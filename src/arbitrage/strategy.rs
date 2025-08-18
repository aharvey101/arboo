use crate::arbitrage::simulation::{arboo_bytecode, get_address, one_thousand_eth, AddressType};
use crate::arbitrage::simulation::simulation_with_logging;
use crate::common::cache::{EvmSimulatorCache, BlockDataCache};
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
use tokio::sync::{broadcast::Sender, mpsc, RwLock, Semaphore};

/// Strategy worker pool for handling arbitrage opportunities efficiently
/// Uses connection pooling, memory caching, and optimized processing with bounded concurrency
pub struct StrategyWorkerPool {
    #[allow(dead_code)]
    sender: mpsc::Sender<LogEvent>,
    connection_pool: ConnectionPool,
    pools_map: Arc<RwLock<HashMap<Address, Event>>>,
    // Add semaphore for limiting concurrent tasks
    task_semaphore: Arc<Semaphore>,
    // Add EVM simulator cache for better performance
    evm_cache: EvmSimulatorCache,
    // Block data cache to reduce network calls
    block_cache: BlockDataCache,
}

impl StrategyWorkerPool {
    pub async fn new(sender: Sender<LogEvent>, ws_url: String, max_connections: usize) -> Result<Self> {
        let (tx, _rx) = mpsc::channel::<LogEvent>(1000);
        let connection_pool = ConnectionPool::new(ws_url, max_connections);
        
        // Load pools map once and cache it
        let pools_map = Arc::new(RwLock::new(Self::load_pools_map().await?));
        
        // Create semaphore to limit concurrent strategy tasks (prevent thread explosion)
        let max_concurrent_tasks = (max_connections * 2).min(32); // Reasonable limit
        let task_semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));
        
        // Initialize caches
        let evm_cache = EvmSimulatorCache::new(16, 30); // Cache up to 16 simulators for 30 seconds
        let block_cache = BlockDataCache::new(10); // Cache block data for 10 seconds
        
        info!("Strategy worker pool configured with max {} concurrent tasks", max_concurrent_tasks);
        
        // Use standard spawn with bounded concurrency
        let pool_clone = connection_pool.clone();
        let pools_map_clone = pools_map.clone();
        let semaphore_clone = task_semaphore.clone();
        let mut event_receiver = sender.subscribe();

        tokio::spawn(async move {
            info!("Starting bounded strategy worker with {} max concurrent tasks", max_concurrent_tasks);
            while let Ok(log_event) = event_receiver.recv().await {
                let pool_clone = pool_clone.clone();
                let pools_map_clone = pools_map_clone.clone();
                let semaphore_clone_inner = semaphore_clone.clone();
                
                // Use async spawn instead of spawn_blocking to handle semaphore properly
                tokio::spawn(async move {
                    // Acquire permit asynchronously - this blocks if no permits available
                    let _permit = match semaphore_clone_inner.try_acquire() {
                        Ok(permit) => permit,
                        Err(_) => {
                            log::warn!("Max concurrent tasks reached, dropping event");
                            return;
                        }
                    };
                    
                    // Run CPU-intensive work in blocking context
                    let result = tokio::task::spawn_blocking(move || {
                        tokio::runtime::Handle::current().block_on(async move {
                            process_strategy_optimized(log_event, &pool_clone, &pools_map_clone).await
                        })
                    }).await;
                    
                    if let Ok(Err(e)) = result {
                        log::error!("Strategy processing error: {}", e);
                    }
                    // Permit automatically released when _permit goes out of scope
                });
            }
            info!("Strategy worker stopped");
        });
        
        Ok(Self {
            sender: tx,
            connection_pool,
            pools_map,
            task_semaphore,
            evm_cache,
            block_cache,
        })
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
    
    log::info!("🔍 Starting arbitrage analysis for pool: {} (variant: {})", 
               message.log_pool_address, message.pool_variant);
    
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
        log::info!("❌ Arbitrage analysis complete - Not profitable ({}), skipping. Duration: {:?}", 
                   optimal_result.possible_profit, start_time.elapsed());
        return Ok(());
    }

    // Check if block is still current
    let current_block = provider.get_block_number().await.unwrap_or_default();
    if current_block > latest_block.header.number {
        log::info!("⏰ Arbitrage analysis complete - Block expired ({} > {}), opportunity missed. Duration: {:?}", 
                   current_block, latest_block.header.number, start_time.elapsed());
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

    log::info!("📊 Arbitrage opportunity details - Token0: {}, Token1: {}, Fee: {}", 
               message.token0, message.token1, message.fee);

    // Run final simulation with detailed logging for the optimal amount
    let _final_profit = simulation_with_logging(
        target_pool,
        message.token1,
        message.token0,
        optimal_result.optimal_amount,
        message.fee,
        &mut simulator,
        provider,
        true, // Enable detailed logging for this final run
    ).await.unwrap_or(U256::ZERO);

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

    log::info!("✅ Arbitrage analysis complete - Transaction submitted. Total duration: {:?}", start_time.elapsed());
    Ok(())
}

async fn load_specific_pools_optimized(
    simulator: &mut EvmSimulator,
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
    simulator: &mut EvmSimulator,
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
    simulator: &mut EvmSimulator,
    max_input: U256,
    fee: U24,
    latest_block: &Block,
    target_pool: Address,
    _provider: &RootProvider<PubSubFrontend, Ethereum>,
) -> Result<ArbitrageResult> {
    let optimization_start = std::time::Instant::now();
    
    // Pre-calculate reusable values to avoid repeated work
    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas
        .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?);
    let wallet_address = simulator.owner;
    let contract_address = simulator.contract_address;
    
    // Get initial WETH balance once and reuse across iterations
    let initial_weth_balance = check_weth_balance_fast(
        wallet_address,
        simulator,
        &latest_gas_limit,
        &latest_gas_price,
    ).await?;
    
    log::debug!("Initial balance fetch: {:?}", optimization_start.elapsed());
    
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;
    
    // Use binary search but with optimized simulation calls
    let mut left = U256::from(10).pow(U256::from(18)); // Start with 1 token
    let mut right = max_input;
    let mut iteration_count = 0;
    
    while left <= right && iteration_count < 20 { // Limit iterations to prevent infinite loops
        iteration_count += 1;
        let mid = (left + right) / U256::from(2);
        let iteration_start = std::time::Instant::now();
        
        // Fast simulation without network calls
        let profit = simulate_arbitrage_fast(
            target_pool,
            token_in,
            token_out,
            mid,
            fee,
            simulator,
            contract_address,
            wallet_address,
            &latest_gas_limit,
            &latest_gas_price,
            initial_weth_balance,
        ).await.unwrap_or(U256::ZERO);

        log::debug!("Binary search iteration {} amount {} profit {} took: {:?}", 
                   iteration_count, mid, profit, iteration_start.elapsed());

        if profit > best_profit {
            best_profit = profit;
            optimal_amount = mid;
            left = mid + U256::from(1);
        } else {
            right = mid - U256::from(1);
        }
        
        // Early exit if we find a good amount and iterations are taking too long
        if iteration_count > 10 && best_profit > U256::ZERO {
            let total_time = optimization_start.elapsed();
            if total_time.as_millis() > 5000 { // 5 second timeout
                log::debug!("Early exit due to time limit reached");
                break;
            }
        }
    }

    log::debug!("Binary search complete after {} iterations in {:?}", 
               iteration_count, optimization_start.elapsed());

    if best_profit == U256::ZERO {
        return Ok(ArbitrageResult {
            optimal_amount: U256::ZERO,
            possible_profit: U256::ZERO,
        });
    }

    // Fast profit calculation without additional network calls
    let possible_profit = estimate_weth_profit_fast(token_in, best_profit);

    Ok(ArbitrageResult {
        optimal_amount,
        possible_profit,
    })
}

// Fast simulation that reuses EVM state and minimizes allocations
async fn simulate_arbitrage_fast(
    target_pool: Address,
    token_a: Address,
    token_b: Address,
    amount: U256,
    fee: U24,
    simulator: &mut EvmSimulator,
    contract_address: Address,
    wallet_address: Address,
    gas_limit: &u64,
    gas_price: &U256,
    initial_balance: U256,
) -> Result<U256> {
    // Pre-encoded function call to avoid repeated encoding
    alloy::sol! {
        #[derive(Debug)]
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
        ) external;
    }

    let function_call = flashSwap_V3_to_V2Call {
        pool0: target_pool,
        fee1: fee,
        tokenIn: token_a,
        tokenOut: token_b,
        amountIn: amount,
    };

    let tx = Tx {
        caller: wallet_address,
        transact_to: contract_address,
        data: function_call.abi_encode().into(),
        value: U256::ZERO,
        gas_limit: *gas_limit,
        gas_price: *gas_price,
    };

    // Execute simulation without balance checks
    simulator.call(tx)?;

    // Fast balance check
    let final_balance = check_weth_balance_fast(
        wallet_address,
        simulator,
        gas_limit,
        gas_price,
    ).await?;

    Ok(final_balance.saturating_sub(initial_balance))
}

// Optimized balance check without redundant operations
async fn check_weth_balance_fast(
    wallet_address: Address,
    simulator: &mut EvmSimulator,
    gas_limit: &u64,
    gas_price: &U256,
) -> Result<U256> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    let tx = Tx {
        caller: wallet_address,
        transact_to: get_address(AddressType::Weth),
        data: balanceOfCall { account: wallet_address }.abi_encode().into(),
        value: U256::ZERO,
        gas_limit: *gas_limit,
        gas_price: *gas_price,
    };

    let result = simulator.call(tx)?;
    Ok(U256::from_be_slice(&result.output))
}

// Fast profit estimation without network calls
fn estimate_weth_profit_fast(token_in: Address, amount: U256) -> U256 {
    // Use approximate conversion rates for common tokens to avoid network calls
    // This is a fast estimation - for exact values, the final simulation will be accurate
    
    let weth_address = get_address(AddressType::Weth);
    if token_in == weth_address {
        return amount; // Already WETH
    }
    
    // For other tokens, use conservative estimate
    // In production, you'd maintain a cache of recent token prices
    amount * U256::from(95) / U256::from(100) // Assume ~5% slippage
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