use crate::arbitrage::simulation::{arboo_bytecode, get_address, one_thousand_eth, AddressType};
use crate::arbitrage::simulation::simulation;
use crate::common::connection_pool::ConnectionPool;
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
    sync::Arc,
};
use tokio::sync::{broadcast::Sender, mpsc, RwLock};

/// Strategy worker pool for handling arbitrage opportunities efficiently
/// Uses connection pooling, memory caching, and optimized processing with bounded concurrency
pub struct StrategyWorkerPool {
    #[allow(dead_code)]
    sender: mpsc::Sender<LogEvent>,
    connection_pool: ConnectionPool,
    pools_map: Arc<RwLock<HashMap<Address, Event>>>,
}

impl StrategyWorkerPool {
    pub async fn new(_sender: Sender<LogEvent>, ws_url: String, max_connections: usize) -> Result<Self> {
        let (tx, _rx) = mpsc::channel::<LogEvent>(1000);
        let connection_pool = ConnectionPool::new(ws_url, max_connections);
        
        // Load pools map once and cache it
        let pools_map = Arc::new(RwLock::new(Self::load_pools_map().await?));
        
        // Store components for direct processing - no spawning needed since main app handles this
        // EVM components can't be Send+Sync so we avoid spawning tasks entirely
        Ok(Self {
            sender: tx,
            connection_pool,
            pools_map,
        })
    }

    /// Process a log event directly - use this instead of sending to channel
    pub async fn process_event(&self, log_event: LogEvent) -> Result<()> {
        process_strategy_optimized(log_event, &self.connection_pool, &self.pools_map).await
    }

    async fn load_pools_map() -> Result<HashMap<Address, Event>> {
        let cache_dir = var("CACHE_DIR").unwrap_or_else(|_| "/tmp/arboo-cache".to_string());
        let cache_path = format!("{}/.cached-pools.csv", cache_dir);
        
        let mut pools_map = HashMap::new();
        let path = Path::new(&cache_path);
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        for line in reader.lines().skip(1) {
            let line = line?;
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 6 { continue; }

            match fields[2] {
                "2" => {
                    let pair_address = Address::from_str(fields[1])
                        .map_err(|e| anyhow::anyhow!("Invalid V2 pair address '{}': {}", fields[1], e))?;
                    pools_map.insert(
                        pair_address,
                        Event::PairCreated(V2PoolCreated {
                            pair_address,
                            token0: Address::from_str(fields[3])
                                .map_err(|e| anyhow::anyhow!("Invalid V2 token0 address '{}': {}", fields[3], e))?,
                            token1: Address::from_str(fields[4])
                                .map_err(|e| anyhow::anyhow!("Invalid V2 token1 address '{}': {}", fields[4], e))?,
                            fee: fields[5].parse::<u32>()
                                .map_err(|e| anyhow::anyhow!("Invalid V2 fee '{}': {}", fields[5], e))?,
                        }),
                    );
                }
                "3" => {
                    let pair_address = Address::from_str(fields[1])
                        .map_err(|e| anyhow::anyhow!("Invalid V3 pair address '{}': {}", fields[1], e))?;
                    pools_map.insert(
                        pair_address,
                        Event::PoolCreated(V3PoolCreated {
                            pair_address,
                            token0: Address::from_str(fields[3])
                                .map_err(|e| anyhow::anyhow!("Invalid V3 token0 address '{}': {}", fields[3], e))?,
                            token1: Address::from_str(fields[4])
                                .map_err(|e| anyhow::anyhow!("Invalid V3 token1 address '{}': {}", fields[4], e))?,
                            fee: fields[5].parse::<u32>()
                                .map_err(|e| anyhow::anyhow!("Invalid V3 fee '{}': {}", fields[5], e))?,
                            tick_spacing: 0i32,
                        }),
                    );
                }
                _ => continue,
            }
        }
        
        info!("Loaded {} pools into optimized cache", pools_map.len());
        Ok(pools_map)
    }

    pub async fn start(&self) -> Result<()> {
        info!("StrategyWorkerPool started with {} pools cached", 
               self.pools_map.read().await.len());
        Ok(())
    }
}

#[derive(Debug)]
pub struct ArbitrageResult {
    pub optimal_amount: U256,
    pub possible_profit: U256,
}


/// Main strategy processing with connection pooling and caching optimizations
pub async fn process_strategy_optimized(
    message: LogEvent,
    connection_pool: &ConnectionPool,
    pools_map: &Arc<RwLock<HashMap<Address, Event>>>,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    
    // Get pooled provider - much faster than creating new connection
    let pooled_provider = connection_pool.get_provider().await?;
    let provider = pooled_provider.provider();
    
    log::debug!("Time to get pooled provider: {:?}", start_time.elapsed());
    
    let latest_block_number = provider.get_block_number().await?;

    // Use Arc to avoid expensive block cloning
    let latest_block = Arc::new(
        provider
            .get_block(BlockId::latest(), BlockTransactionsKind::Full)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?
    );
    
    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    // Create EVM simulator with pooled provider
    let pooled_provider_clone = connection_pool.get_provider().await?;
    let simulator_provider = pooled_provider_clone.into_provider();
    let mut simulator = EvmSimulator::new(
        simulator_provider,
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    )?;

    log::debug!("Time to create EVM: {:?}", start_time.elapsed());

    let block_base_fee = latest_block.header.base_fee_per_gas
        .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?;

    // Use cached pools map - no file I/O per request
    let pools_map_guard = pools_map.read().await;
    load_specific_pools_optimized(
        &mut simulator,
        message.log_pool_address,
        message.corresponding_pool_address,
        &pools_map_guard,
    ).await?;
    drop(pools_map_guard); // Release lock immediately
    
    log::debug!("Pools loaded in: {:?}", start_time.elapsed());

    // Setup EVM state
    setup_evm_optimized(&mut simulator, provider).await?;
    log::debug!("Setup EVM in: {:?}", start_time.elapsed());

    // Find optimal arbitrage amount
    let max_input = U256::MAX - U256::from(10).pow(U256::from(18));
    let optimal_result = find_optimal_amount_optimized(
        message.token0,
        message.token1,
        &mut simulator,
        max_input,
        message.fee,
        &latest_block,
        message.corresponding_pool_address,
        provider,
    ).await?;

    log::debug!("Calculated optimal result in: {:?}", start_time.elapsed());

    // Early exit for unprofitable opportunities
    if optimal_result.possible_profit < U256::from(100_000u128) {
        log::debug!("Opportunity not profitable ({}), skipping", optimal_result.possible_profit);
        return Ok(());
    }

    // Check if block is still current
    let current_block = provider.get_block_number().await.unwrap_or_default();
    if current_block > latest_block.header.number {
        log::debug!("Block {} passed (current: {}), opportunity expired", 
              latest_block.header.number, current_block);
        return Ok(());
    }

    // Determine target pool based on variant
    let is_v2_to_v3 = message.pool_variant == 3;
    let target_pool = if is_v2_to_v3 {
        message.log_pool_address
    } else {
        message.corresponding_pool_address
    };

    info!(
        "🎯 Profitable arbitrage! Profit: {} wei, Amount: {}, Target: {}",
        optimal_result.possible_profit, optimal_result.optimal_amount, target_pool
    );

    // Create transaction data
    let transaction = create_input_data(
        target_pool,
        message.fee,
        message.token1,
        message.token0,
        optimal_result.optimal_amount,
    ).await?;

    let contract_address = var::<&str>("CONTRACT_ADDRESS")?;
    let contract_address = Address::from_str(&contract_address)?;

    let nonce = provider
        .get_transaction_count(address!("5f1F5565561aC146d24B102D9CDC288992Ab2938"))
        .await?;

    // Send transaction asynchronously to avoid blocking
    tokio::spawn(send_transaction(
        contract_address,
        Some(block_base_fee as u128),
        Some(1_500_000),
        Some(block_base_fee as u128),
        Some(2_000_000),
        transaction,
        nonce,
    ));

    log::debug!("⚡ Total processing time: {:?}", start_time.elapsed());
    Ok(())
}

async fn load_specific_pools_optimized(
    simulator: &mut EvmSimulator<'_>,
    pool_a: Address,
    pool_b: Address,
    pools_map: &HashMap<Address, Event>,
) -> Result<()> {
    // Load pool A
    if let Some(pool) = pools_map.get(&pool_a) {
        match pool {
            Event::PoolCreated(pool) => {
                simulator.load_v3_pool_state(pool.pair_address).await?;
            }
            Event::PairCreated(pool) => {
                simulator.load_v2_pool_state(pool.pair_address).await?;
                simulator.load_pool_state(pool.pair_address).await?;
            }
        }
    }

    // Load pool B
    if let Some(pool) = pools_map.get(&pool_b) {
        match pool {
            Event::PoolCreated(pool) => {
                simulator.load_v3_pool_state(pool.pair_address).await?;
            }
            Event::PairCreated(pool) => {
                simulator.load_v3_pool_state(pool.pair_address).await?;
            }
        }
    }

    Ok(())
}

async fn setup_evm_optimized(
    simulator: &mut EvmSimulator<'_>,
    provider: &RootProvider<PubSubFrontend, Ethereum>,
) -> Result<()> {
    let latest_block = provider
        .get_block(
            BlockId::Number(alloy::eips::BlockNumberOrTag::Latest),
            BlockTransactionsKind::Full,
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?;

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas
        .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?);

    // Deploy arbitrage contract
    let contract_address = simulator.contract_address;
    simulator.deploy_code_at(contract_address, arboo_bytecode()).await;

    // Fund wallet with ETH
    let initial_eth_balance = U256::from(1_000_000) * U256::from(10).pow(U256::from(18));
    let wallet = simulator.owner;
    simulator.set_eth_balance(wallet, initial_eth_balance).await;

    // Convert ETH to WETH
    alloy::sol! {
        function swapEthForWeth(address to, uint256 deadline) external payable;
    };

    let function_call = swapEthForWethCall {
        to: wallet,
        deadline: U256::from(9999999999_u64),
    };

    let eth_to_weth_tx = Tx {
        caller: wallet,
        transact_to: get_address(AddressType::Weth),
        data: function_call.abi_encode().into(),
        value: one_thousand_eth() * U256::from(10),
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.call(eth_to_weth_tx)?;

    // Approve router to spend WETH
    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }
    
    let approve_data = approveCall {
        spender: get_address(AddressType::V3Router),
        amount: U256::MAX,
    }.abi_encode();

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

async fn find_optimal_amount_optimized(
    token_in: Address,
    token_out: Address,
    simulator: &mut EvmSimulator<'_>,
    max_input: U256,
    fee: U24,
    latest_block: &Block,
    target_pool: Address,
    provider: &RootProvider<PubSubFrontend, Ethereum>,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;
    let mut left = U256::from(10).pow(U256::from(18)); // Start with 1 token
    let mut right = max_input;

    // Binary search for optimal amount
    while left <= right {
        let mid = (left + right) / U256::from(2);
        
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

        if v3_amount_out > best_profit {
            best_profit = v3_amount_out;
            optimal_amount = mid;
            left = mid + U256::from(1);
        } else {
            right = mid - U256::from(1);
        }
    }

    if best_profit == U256::ZERO {
        return Ok(ArbitrageResult {
            optimal_amount: U256::ZERO,
            possible_profit: U256::ZERO,
        });
    }

    // Calculate profit in WETH terms
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

    // Create path for token -> WETH conversion
    let mut path = Vec::new();
    path.extend_from_slice(token_in.as_slice());
    path.extend_from_slice(&U24::from(3000).to_be_bytes_vec());
    path.extend_from_slice(get_address(AddressType::Weth).as_slice());
    let path = alloy::primitives::Bytes::from(path);

    let tx_data = quoteExactInputCall {
        path,
        amountIn: best_profit,
    }.abi_encode();

    let quote_tx = Tx {
        caller: simulator.owner,
        transact_to: get_address(AddressType::V2Quoter),
        data: tx_data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let result = simulator.call(quote_tx)?;
    let possible_profit = decode_quote_output_v3(result.output)?;

    Ok(ArbitrageResult {
        optimal_amount,
        possible_profit,
    })
}

fn decode_quote_output_v3(output: revm::primitives::Bytes) -> Result<U256> {
    let output_str = output.to_string();
    let hex_str = output_str.trim_start_matches("0x");
    let output_bytes = hex::decode(hex_str)?;
    
    if output_bytes.len() < 32 {
        return Err(anyhow::anyhow!("Output too short: {} bytes", output_bytes.len()));
    }
    
    let number = U256::from_be_slice(&output_bytes[0..32]);
    Ok(number)
}

/// Initialize the strategy pool
pub async fn initialize_strategy_pool(
    sender: Sender<LogEvent>, 
    ws_url: String,
    max_connections: usize,
) -> Result<StrategyWorkerPool> {
    StrategyWorkerPool::new(sender, ws_url, max_connections).await
}

/// Legacy function for backward compatibility
pub async fn process_strategy(message: LogEvent, ws_url: String) -> Result<()> {
    log::warn!("Using legacy process_strategy - switch to optimized version for better performance");
    
    let pool = ConnectionPool::new(ws_url, 1);
    let pools_map = Arc::new(RwLock::new(
        StrategyWorkerPool::load_pools_map().await?
    ));
    process_strategy_optimized(message, &pool, &pools_map).await
}