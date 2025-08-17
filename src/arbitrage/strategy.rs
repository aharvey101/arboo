use crate::arbitrage::simulation::{arboo_bytecode, get_address, one_thousand_eth, AddressType};
use crate::arbitrage::simulation::simulation;
use crate::common::transaction::{create_input_data, send_transaction};
use crate::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::{EvmSimulator, Tx},
};
use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider, WsConnect};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Block, BlockTransactionsKind};
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::aliases::U24;
use alloy_primitives::{address, U64};
use alloy_sol_types::SolCall;
use anyhow::Result;
use dotenv::var;
use log::info;
use revm::primitives::{Address, U256};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead},
    path::Path,
    str::FromStr,
};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;

pub struct StrategyWorkerPool {
    sender: mpsc::Sender<LogEvent>,
}
#[allow(clippy::all)]
impl StrategyWorkerPool {
    pub async fn new(sender: Sender<LogEvent>, ws_url: String) {
        let mut event_reciever = sender.subscribe();

        let local = tokio::task::LocalSet::new();

        local.spawn_local(async move {
            while let Ok(res) = event_reciever.recv().await {
                tokio::task::spawn_local(process_strategy(res, ws_url.clone()));
            }
        });

        local.await;
    }

    pub async fn submit_event(&self, event: LogEvent) -> Result<()> {
        // Send with back-pressure (will wait if channel is full)
        self.sender.send(event).await?;
        Ok(())
    }

    pub fn try_submit_event(
        &self,
        event: LogEvent,
    ) -> Result<(), mpsc::error::TrySendError<LogEvent>> {
        // Try to send without waiting
        self.sender.try_send(event)
    }
}

// Usage example in your main code:
pub async fn initialize_strategy_pool(sender: Sender<LogEvent>, ws_url: String) {
    StrategyWorkerPool::new(sender, ws_url).await;
}

pub async fn process_strategy(message: LogEvent, ws_url: String) -> Result<()> {
    let time = std::time::Instant::now();
    // log the runtime
    let ws_client = WsConnect::new(ws_url.clone());
    info!("websocket url: {:?}", ws_url);
    let provider = ProviderBuilder::new().on_ws(ws_client).await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;

    info!("Time to make provider: {:?}", time.elapsed());
    let latest_block_number = provider
        .get_block_number()
        .await
        .map_err(|e| anyhow::anyhow!("Error getting block number: {}", e))?;

    let latest_block = provider
        .get_block(BlockId::latest(), BlockTransactionsKind::Full)
        .await
        .map_err(|e| anyhow::anyhow!("Error getting latest block: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?;
    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    let mut simulator = EvmSimulator::new(
        provider.clone(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    ).map_err(|e| anyhow::anyhow!("Failed to create EVM simulator: {}", e))?;

    info!("Time to make evm: {:?}", time.elapsed());
    // reserves of the target pool to low?
    let is_v2_to_v3 = message.pool_variant == 3;
    // Calculate optimal amount
    let max_input = U256::MAX - U256::from(10).pow(U256::from(18));

    let block_base_fee = latest_block.header.base_fee_per_gas
        .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?;
    info!("loading the pools");
    load_specific_pools(
        &mut simulator,
        message.log_pool_address,
        message.corresponding_pool_address,
    )
    .await?;
    info!("Pools loaded");
    let time = std::time::Instant::now();

    setup_evm(&mut simulator, &provider).await?;

    info!("Setup evm {:?}", time.elapsed());
    let optimal_result = find_optimal_amount_v3_to_v2(
        message.token0,
        message.token1,
        &mut simulator,
        max_input,
        message.fee,
        latest_block.clone(),
        message.corresponding_pool_address,
        &provider,
    )
    .await?;

    info!("Calculated Optimal Result");
    if optimal_result.possible_profit < U256::from(100_000u128) {
        info!("no profit");
        return Ok(());
    }
    // simulate with optimal amoun in arbooo
    let target_pool = if is_v2_to_v3 {
        message.log_pool_address
    } else {
        message.corresponding_pool_address
    };
    log::debug!(
        "Tike taken to calculate optimal amount: {:?}",
        time.elapsed()
    );
    info!("Arbitrage opportunity found");
    info!(
        "Creating and sending TX for optimal amount {} to pool {}",
        optimal_result.optimal_amount, target_pool
    );

    if provider.get_block_number().await.unwrap_or_default() > latest_block.header.number {
        info!("Block has passed, opportunity has passed");
    }

    let transaction = create_input_data(
        target_pool,
        message.fee,
        message.token1,
        message.token0,
        optimal_result.optimal_amount,
    )
    .await
    .inspect(|e| info!("Error creating input data: {:?}", e))?;

    let contract_address = var::<&str>("CONTRACT_ADDRESS")?;
    let contract_address = Address::from_str(&contract_address)?;

    let nonce = provider
        .get_transaction_count(address!("5f1F5565561aC146d24B102D9CDC288992Ab2938"))
        .await
        .inspect(|e| info!("error getting nonce, {:?}", e))?;

    tokio::spawn(send_transaction(
        contract_address,
        Some(block_base_fee as u128),
        Some(1_500_000),
        Some(block_base_fee as u128),
        Some(2_000_000),
        transaction,
        nonce,
    ));
    Ok(())
}

#[derive(Debug)]
pub struct ArbitrageResult {
    pub optimal_amount: U256,
    pub possible_profit: U256,
}

// lets do a really slow way to see if it's the binary search that is the problem?

pub async fn find_optimal_amount_v3_to_v2(
    token_in: Address,
    token_out: Address,
    simulator: &mut EvmSimulator,
    max_input: U256,
    fee: U24,
    latest_block: Block,
    target_pool: Address,
    provider: &RootProvider<PubSubFrontend, Ethereum>,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;
    let mut left = U256::from(10).pow(U256::from(18)); // 1 token
    let mut right = max_input;

    while left <= right {
        let mid = (left + right) / U256::from(2);
        // Only query once per iteration with mid
        info!("doing sim with mid: {:?}", mid);
        let v3_amount_out = simulation(
            target_pool,
            token_in,
            token_out,
            mid,
            fee,
            simulator,
            provider,
        )
        .await
        .unwrap_or(U256::ZERO);

        // Calculate profit based on mid amount
        let current_profit = v3_amount_out;

        // Update best profit if better
        if current_profit > best_profit {
            best_profit = current_profit;
            optimal_amount = mid;
            // If profit is increasing, search upper half
            left = mid + U256::from(1);
        } else {
            // If profit is decreasing, search lower half
            right = mid - U256::from(1);
        }
    }
    if best_profit == U256::ZERO {
        return Ok(ArbitrageResult {
            optimal_amount: U256::ZERO,
            possible_profit: U256::ZERO,
        });
    }

    // convert optimal amount to weth

    alloy::sol! {
        #[derive(Debug)]
        function quoteExactInput(
            bytes memory path,
            uint256 amountIn
        ) external returns (uint256 amountOut, uint160[] sqrtPriceX96AfterList, uint32[] initializedTicksCrossedList, uint256 gasEstimate);
    }

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas
        .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?);
    let sim = simulator;
    let mut path = Vec::new();
    path.extend_from_slice(token_in.as_slice());
    path.extend_from_slice(&U24::from(3000).to_be_bytes_vec());
    path.extend_from_slice(get_address(AddressType::Weth).as_slice());
    let path = alloy::primitives::Bytes::from(path);

    let tx_data = quoteExactInputCall {
        path,
        amountIn: best_profit,
    }
    .abi_encode();

    let tx = Tx {
        caller: sim.owner,
        transact_to: get_address(AddressType::V2Quoter),
        data: tx_data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let res = sim.call(tx)?;

    let possible_profit = decode_quote_output_v3(res.output)
        .map_err(|e| anyhow::anyhow!("Failed to decode V3 quoter output: {}", e))?;
    log::debug!("possible_profit {possible_profit}");
    Ok(ArbitrageResult {
        optimal_amount,
        possible_profit,
    })
}
// Helper function to decode V3 quoter output
fn decode_quote_output_v3(output: revm::primitives::Bytes) -> Result<U256> {
    let output = hex::decode(output.to_string().trim_start_matches("0x"))?;

    let number = U256::from_be_slice(&output[0..32]);

    Ok(number)
}

async fn load_specific_pools(
    simulator: &mut EvmSimulator,
    pool_a: Address,
    pool_b: Address,
) -> Result<()> {
    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path)
        .map_err(|e| anyhow::anyhow!("Error opening cached pools file: {}", e))?;
    let reader = io::BufReader::new(file);

    let sim = simulator;

    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line
            .map_err(|e| anyhow::anyhow!("Error reading line from cached pools file: {}", e))?;
        let fields: Vec<&str> = line.split(',').collect();
        match fields[2] {
            "2" => {
                let pair_address = Address::from_str(fields[1]).unwrap_or_default();
                pools_map.insert(
                    pair_address,
                    Event::PairCreated(V2PoolCreated {
                        pair_address: Address::from_str(fields[1]).unwrap_or_default(),
                        token0: Address::from_str(fields[3]).unwrap_or_default(),
                        token1: Address::from_str(fields[4]).unwrap_or_default(),
                        fee: fields[5].parse::<u32>().unwrap_or_default(),
                        //block_number: fields[6].parse::<u64>().unwrap_or_default(),
                    }),
                );
            }
            "3" => {
                let pair_address = Address::from_str(fields[1]).unwrap_or_default();
                pools_map.insert(
                    pair_address,
                    Event::PoolCreated(V3PoolCreated {
                        pair_address: Address::from_str(fields[1]).unwrap_or_default(),
                        token0: Address::from_str(fields[3]).unwrap_or_default(),
                        token1: Address::from_str(fields[4]).unwrap_or_default(),
                        fee: fields[5].parse::<u32>().unwrap_or_default(),
                        tick_spacing: 0i32,
                    }),
                );
            }
            &_ => continue,
        };
    }
    match pools_map.get(&pool_a) {
        Some(pool) => match pool {
            Event::PoolCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load v3 pool state for {}: {}", pool.pair_address, e))?;
            }
            Event::PairCreated(pool) => {
                sim.load_v2_pool_state(pool.pair_address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load v2 pool state for {}: {}", pool.pair_address, e))?;
                sim.load_pool_state(pool.pair_address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load basic pool state for {}: {}", pool.pair_address, e))?;
            }
        },
        _ => {}
    };

    match pools_map.get(&pool_b) {
        Some(pool) => match pool {
            Event::PoolCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load v3 pool state for {}: {}", pool.pair_address, e))?;
            }
            Event::PairCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to load v3 pool state for {}: {}", pool.pair_address, e))?;
            }
        },
        _ => {}
    };

    Ok(())
}

async fn setup_evm(
    simulator: &mut EvmSimulator,
    provider: &RootProvider<PubSubFrontend, Ethereum>,
) -> Result<()> {
    let latest_block = provider
        .get_block(
            BlockId::Number(alloy::eips::BlockNumberOrTag::Latest),
            BlockTransactionsKind::Full,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Error getting latest block: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?;

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    // deploy contract:

    let contract_address = simulator.contract_address;
    simulator
        .deploy_code_at(contract_address, arboo_bytecode())
        .await;

    // set initial eth value;
    let initial_eth_balance = U256::from(1_000_000) * U256::from(10).pow(U256::from(18));

    let wallet = simulator.owner;

    simulator.set_eth_balance(wallet, initial_eth_balance).await;

    alloy::sol! {
        function swapEthForWeth(
            address to,
            uint256 deadline
        ) external payable;
    };

    let function_call = swapEthForWethCall {
        to: wallet,
        deadline: U256::from(9999999999_u64),
    };

    let function_call_data = function_call.abi_encode();

    let new_tx = Tx {
        caller: wallet,
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: one_thousand_eth() * U256::from(10),
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.call(new_tx)?;

    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }
    let approve_data = approveCall {
        spender: get_address(AddressType::V3Router),
        amount: U256::MAX, // Infinite approval, you can set a specific amount instead
    }
    .abi_encode();

    let approve_tx = Tx {
        caller: wallet,
        transact_to: get_address(AddressType::Weth),
        data: approve_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.call(approve_tx)?;
    Ok(())
}
