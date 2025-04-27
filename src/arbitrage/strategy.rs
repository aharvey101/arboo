use crate::arbitrage::simulation::{arboo_bytecode, get_address, one_thousand_eth, AddressType};
use crate::arbitrage::simulation::{one_ether, simulation};
use crate::common::transaction::{create_input_data, send_transaction};
use crate::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::{EvmSimulator, Tx},
};
use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Block, BlockTransactionsKind};
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::aliases::U24;
use alloy_primitives::{address, Bytes, U160};
use alloy_sol_types::abi::token;
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
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio::sync::{broadcast::Sender, Mutex as TokioMutex};

pub async fn strategy(
    sender: Sender<LogEvent>,
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
) -> Result<()> {
    let mut event_reciever = sender.subscribe();
    loop {
        match event_reciever.recv().await {
            Ok(message) => {
                // reserves of the target pool to low?
                let is_v2_to_v3 = message.pool_variant == 3;
                //log::debug!("Message: {:?}", message);
                // Calculate optimal amount
                let max_input = U256::MAX - U256::from(10).pow(U256::from(18));

                let latest_block = provider
                    .get_block(BlockId::latest(), BlockTransactionsKind::Full)
                    .await
                    .unwrap()
                    .unwrap();

                let block_base_fee = latest_block.header.base_fee_per_gas.unwrap();

                load_specific_pools(
                    simulator.clone(),
                    message.log_pool_address,
                    message.corresponding_pool_address,
                )
                .await?;

                let time = std::time::Instant::now();

                setup_evm(simulator.clone(), provider.clone()).await?;

                //info!("Message: {:?}", message);
                let optimal_result = match find_optimal_amount_v3_to_v2(
                    message.token0,
                    message.token1,
                    simulator.clone(),
                    max_input,
                    message.fee,
                    latest_block.clone(),
                    message.corresponding_pool_address,
                    provider.clone(),
                )
                .await
                {
                    Ok(res) => res,
                    Err(_) => continue,
                };

                if optimal_result.possible_profit < U256::from(100_000u128) {
                    continue;
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

                if provider.get_block_number().await.unwrap_or_default()
                    > latest_block.header.number
                {
                    info!("Block has passed, opportunity has passed");
                    continue;
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
            }
            Err(err) => {
                info!("Error Recieving message: {err}")
            }
        }
    }
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
    simulator: Arc<TokioMutex<EvmSimulator<'_>>>,
    max_input: U256,
    fee: U24,
    latest_block: Block,
    target_pool: Address,
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;
    let mut left = U256::from(10).pow(U256::from(18)); // 1 token
    let mut right = max_input;

    while left <= right {
        let mid = (left + right) / U256::from(2);
        // Only query once per iteration with mid

        let v3_amount_out = simulation(
            target_pool,
            token_in,
            token_out,
            mid,
            fee,
            simulator.clone(),
            provider.clone(),
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
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
    let mut sim = simulator.lock().await;
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

    let possible_profit = decode_quote_output_v3(res.output).expect("failed to decode output");
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

async fn load_specific_pools<'a>(
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    pool_a: Address,
    pool_b: Address,
) -> Result<()> {
    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path).expect("Error getting File");
    let reader = io::BufReader::new(file);

    let sim = simulator.lock().await;

    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line.expect("Expected Line");
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
                        block_number: fields[6].parse::<u64>().unwrap_or_default(),
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
                    .expect("Failed to load v2 pool state");
            }
            Event::PairCreated(pool) => {
                sim.load_v2_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
                sim.load_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load basic state");
            }
        },
        _ => {}
    };

    match pools_map.get(&pool_b) {
        Some(pool) => match pool {
            Event::PoolCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
            }
            Event::PairCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
            }
        },
        _ => {}
    };

    Ok(())
}

async fn setup_evm(
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
) -> Result<()> {
    let latest_block = provider
        .get_block(
            BlockId::Number(alloy::eips::BlockNumberOrTag::Latest),
            BlockTransactionsKind::Full,
        )
        .await
        .inspect_err(|err| log::error!("Error getting block: {:?}", err))
        .unwrap_or_default()
        .unwrap_or_default();

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    // deploy contract:

    let contract_address = simulator.lock().await.contract_address;
    simulator
        .lock()
        .await
        .deploy_code_at(contract_address, arboo_bytecode())
        .await;

    // set initial eth value;
    let initial_eth_balance = U256::from(1_000_000) * U256::from(10).pow(U256::from(18));

    let wallet = simulator.lock().await.owner;

    simulator
        .lock()
        .await
        .set_eth_balance(wallet, initial_eth_balance)
        .await;

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

    simulator.lock().await.call(new_tx)?;

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

    simulator.lock().await.call(approve_tx)?;
    Ok(())
}
