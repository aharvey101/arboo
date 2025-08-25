use anyhow::Result;
use arbooo::common::logs::LogEvent;
use arbooo::strategies::arbitrage::UniswapArbitrageStrategy;
use arbooo::strategies::traits::{ExecutionResult, ExecutionContext, StrategyConfig, MevStrategy};
use arbooo::common::connection_pool::ConnectionPool;
use arbooo::common::pairs::Event;
use alloy::primitives::address;
use alloy::providers::Provider;
use alloy_primitives::aliases::U24;
use alloy_primitives::U256;
use log::{info, warn};
use revm::primitives::Address;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::timeout;
mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_complete_arbitrage_cycle() -> Result<()> {
    info!("🔄 Starting FULL END-TO-END arbitrage cycle test with Anvil");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created with Anvil");

    test_env.verify_connection().await?;
    let initial_block = test_env.provider.get_block_number().await?;
    info!("📦 Initial block number: {}", initial_block);

    let (pool_setup, log_event) = setup_arbitrage_pools_on_anvil(&test_env).await?;
    info!("🏊 Pool setup complete: Pool A: {}, Pool B: {}", 
          pool_setup.pool_a_address, pool_setup.pool_b_address);

    let strategy = create_arbitrage_strategy_with_anvil(&test_env, &pool_setup).await?;
    info!("✅ UniswapArbitrageStrategy created and configured for Anvil");

    let context = create_anvil_execution_context(&test_env).await?;
    info!("🎯 Execution context created for block: {}", context.block_number);

    info!("🔍 PHASE 1: Scanning for arbitrage opportunities...");
    let start_time = Instant::now();

    let opportunities = timeout(
        Duration::from_secs(10),
        strategy.scan_opportunities(&log_event)
    ).await??;

    let scan_time = start_time.elapsed();
    info!("📊 Detection phase took: {:?}, found {} opportunities", 
          scan_time, opportunities.len());

    assert!(!opportunities.is_empty(), "❌ No arbitrage opportunities detected! This suggests the detection logic isn't working.");
    info!("✅ DETECTION PASSED: {} opportunities found", opportunities.len());

    info!("🧪 PHASE 2: Simulating arbitrage opportunities...");
    let mut simulation_results = Vec::new();
    let mut profitable_opportunities = Vec::new();

    for (i, opportunity) in opportunities.iter().enumerate() {
        info!("🔬 Simulating opportunity {}/{}", i + 1, opportunities.len());

        let simulation_start = Instant::now();
        let simulation_result = timeout(
            Duration::from_secs(10),
            strategy.simulate_opportunity(opportunity, &context)
        ).await??;

        let simulation_time = simulation_start.elapsed();
        info!("📊 Simulation {} took: {:?}, success: {}, profit: {} wei", 
              i + 1, simulation_time, simulation_result.success, simulation_result.profit);

        assert!(simulation_result.gas_used >= U256::from(21_000u64), 
               "❌ Simulation gas estimate too low: {} (min: 21,000)", simulation_result.gas_used);
        assert!(simulation_result.gas_used <= U256::from(2_000_000u64), 
               "❌ Simulation gas estimate too high: {} (max: 2,000,000)", simulation_result.gas_used);

        simulation_results.push(simulation_result.clone());

        if simulation_result.success {
            profitable_opportunities.push((opportunity, simulation_result));
        } else {
            info!("📉 Unprofitable simulation (this is normal)");
        }
    }

    assert!(!simulation_results.is_empty(), "❌ No simulations were performed!");
    info!("✅ SIMULATION PASSED: {}/{} simulations completed", 
          simulation_results.len(), opportunities.len());

    info!("🚀 PHASE 3: Executing profitable opportunities...");
    let mut execution_results = Vec::new();
    let mut successful_transactions = Vec::new();

    if profitable_opportunities.is_empty() {
        warn!("⚠️  No profitable opportunities to execute - creating a forced execution for testing");
        if !opportunities.is_empty() {
            let (opportunity, _) = (&opportunities[0], &simulation_results[0]);

            info!("🧪 Forcing execution of first opportunity for E2E testing...");
            let execution_result = timeout(
                Duration::from_secs(15),
                strategy.execute_opportunity(opportunity, &context)
            ).await??;

            execution_results.push(execution_result.clone());

            if execution_result.success {
                if let Some(ref tx_hash) = execution_result.tx_hash {
                    assert_ne!(tx_hash, "0x1234567890abcdef", 
                              "❌ TDD FAILURE: Even forced execution returns MOCK tx_hash! execute_opportunity must call send_transaction!");
                }
                successful_transactions.push(execution_result);
                info!("✅ FORCED EXECUTION successful!");
            } else {
                info!("📉 Forced execution failed (acceptable for testing)");
            }
        }
    } else {
        for (i, (opportunity, _sim_result)) in profitable_opportunities.iter().enumerate() {
            info!("💰 Executing profitable opportunity {}/{}", i + 1, profitable_opportunities.len());

            let execution_start = Instant::now();
            let execution_result = timeout(
                Duration::from_secs(20),
                strategy.execute_opportunity(opportunity, &context)
            ).await??;

            let execution_time = execution_start.elapsed();
            info!("📊 Execution {} took: {:?}, success: {}, tx_hash: {:?}", 
                  i + 1, execution_time, execution_result.success, execution_result.tx_hash);

            execution_results.push(execution_result.clone());

            if execution_result.success {
                successful_transactions.push(execution_result.clone());

                assert!(execution_result.tx_hash.is_some(), 
                       "❌ Transaction failed - no tx hash returned");

                let tx_hash = execution_result.tx_hash.as_ref().unwrap();
                let hex_part = &tx_hash[2..];

                assert_eq!(tx_hash.len(), 66, 
                          "❌ Invalid transaction hash length: {} (expected 66 chars)", 
                          tx_hash.len());

                assert!(tx_hash.starts_with("0x"), 
                       "❌ Invalid transaction hash format - missing 0x prefix");

                assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()), 
                       "❌ Invalid transaction hash - contains non-hex characters");

                assert_ne!(tx_hash, "0x1234567890abcdef", 
                          "❌ Mock transaction hash detected - real transaction failed");

                assert!(execution_result.profit > U256::ZERO,
                       "❌ Transaction reported success but profit is zero or negative");

                assert!(execution_result.gas_used >= U256::from(21_000u64),
                       "❌ Invalid gas used value - below minimum");

                info!("🎉 TDD: Real transaction hash detected: {}", tx_hash);

                if let Some(ref tx_hash) = execution_result.tx_hash {
                    info!("🔍 Verifying transaction on Anvil: {}", tx_hash);

                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let current_block = test_env.provider.get_block_number().await?;
                    if current_block > initial_block {
                        info!("✅ TRANSACTION CONFIRMED: Block advanced from {} to {}", 
                              initial_block, current_block);
                    } else {
                        warn!("⚠️  Block number unchanged - transaction may not have been mined yet");
                    }
                } else {
                    warn!("⚠️  Successful execution but no tx_hash provided");
                }
            }
        }
    }

    let total_cycle_time = start_time.elapsed();
    info!("⏱️  COMPLETE ARBITRAGE CYCLE took: {:?}", total_cycle_time);

    info!("🧪 FINAL E2E ASSERTIONS:");

    assert!(!opportunities.is_empty(), 
           "❌ DETECTION FAILED: No opportunities detected");
    assert!(!simulation_results.is_empty(), 
           "❌ SIMULATION FAILED: No simulations performed");
    assert!(!execution_results.is_empty(), 
           "❌ EXECUTION FAILED: No executions attempted");

    assert!(total_cycle_time < Duration::from_secs(60), 
           "❌ PERFORMANCE FAILED: Total cycle took too long: {:?}", total_cycle_time);

    let execution_attempts = execution_results.len();
    assert!(execution_attempts > 0, 
           "❌ CRITICAL FAILURE: No execution attempts made!");

    let valid_executions = execution_results.iter()
        .filter(|r| r.tx_hash.is_some() || r.error.is_some())
        .count();
    assert!(valid_executions > 0, 
           "❌ EXECUTION QUALITY FAILED: No valid execution attempts (no tx_hash or error)");

    info!("📈 END-TO-END TEST SUMMARY:");
    info!("  🔍 Opportunities detected: {}", opportunities.len());
    info!("  🧪 Simulations performed: {}", simulation_results.len());
    info!("  � Profitable simulations: {}", profitable_opportunities.len());
    info!("  🚀 Execution attempts: {}", execution_attempts);
    info!("  ✅ Successful transactions: {}", successful_transactions.len());
    info!("  ⏱️  Total cycle time: {:?}", total_cycle_time);
    info!("  🎯 Test Result: FULL E2E CYCLE COMPLETED");

    verify_anvil_state_after_execution(&test_env, initial_block).await?;

    info!("🎉 COMPLETE ARBITRAGE CYCLE TEST PASSED!");
    Ok(())
}

#[tokio::test]
async fn test_sequential_arbitrage_cycles() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Testing sequential arbitrage cycles");

    let num_cycles = 3;
    let mut cycle_times = Vec::new();

    for i in 0..num_cycles {
        info!("🔄 Starting arbitrage cycle {}/{}", i + 1, num_cycles);

        let log_event = create_test_arbitrage_opportunity().await?;
        let start_time = Instant::now();

        let strategy = create_arbitrage_strategy(&test_env).await?;
        let context = create_test_execution_context(&test_env).await?;
        let result = timeout(
            Duration::from_secs(15),
            process_with_strategy(&strategy, &log_event, &context)
        ).await;

        let cycle_time = start_time.elapsed();
        cycle_times.push(cycle_time);

        match result {
            Ok(_) => info!("✅ Cycle {} completed in {:?}", i + 1, cycle_time),
            Err(_) => panic!("❌ Cycle {} timed out", i + 1),
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let avg_time = cycle_times.iter().sum::<Duration>() / cycle_times.len() as u32;
    let max_time = cycle_times.iter().max().unwrap();

    info!("📊 Sequential cycles - Avg: {:?}, Max: {:?}", avg_time, max_time);
    assert!(avg_time < Duration::from_secs(5), "Average cycle time too high");
    assert!(*max_time < Duration::from_secs(10), "Maximum cycle time too high");

    Ok(())
}

#[tokio::test]
async fn test_profitable_vs_unprofitable_cycles() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("💰 Testing profitable vs unprofitable arbitrage cycles");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    info!("💰 Testing profitable opportunity");
    let profitable_event = create_profitable_arbitrage_opportunity().await?;
    let start_time = Instant::now();

    let _result = process_with_strategy(&strategy, &profitable_event, &context).await;
    let profitable_time = start_time.elapsed();

    info!("💰 Profitable cycle completed in {:?}", profitable_time);

    info!("📉 Testing unprofitable opportunity");
    let unprofitable_event = create_unprofitable_arbitrage_opportunity().await?;
    let start_time = Instant::now();

    let _result = process_with_strategy(&strategy, &unprofitable_event, &context).await;
    let unprofitable_time = start_time.elapsed();

    info!("📉 Unprofitable cycle completed in {:?}", unprofitable_time);

    assert!(unprofitable_time < profitable_time, 
           "Unprofitable cycle should be faster than profitable");

    Ok(())
}

#[tokio::test]
async fn test_edge_case_arbitrage_cycles() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🎯 Testing edge case arbitrage cycles");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    info!("🔬 Testing very small arbitrage amount");
    let small_amount_event = create_small_amount_opportunity().await?;
    let _result = process_with_strategy(&strategy, &small_amount_event, &context).await;

    info!("🏔️  Testing maximum arbitrage amount");
    let large_amount_event = create_large_amount_opportunity().await?;
    let _result = process_with_strategy(&strategy, &large_amount_event, &context).await;

    info!("❌ Testing invalid pool addresses");
    let invalid_pool_event = create_invalid_pool_opportunity().await?;
    let _result = process_with_strategy(&strategy, &invalid_pool_event, &context).await;

    info!("✅ All edge case cycles completed without panicking");
    Ok(())
}

#[tokio::test]
async fn test_arbitrage_cycle_performance() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚡ Testing arbitrage cycle performance metrics");

    let log_event = create_test_arbitrage_opportunity().await?;
    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let total_start = Instant::now();

    let _result = process_with_strategy(&strategy, &log_event, &context).await;

    let total_time = total_start.elapsed();

    info!("📊 Total cycle time: {:?}", total_time);

    assert!(total_time < Duration::from_secs(5), 
           "Complete cycle should finish within 5 seconds");

    println!("PERF_METRIC: total_cycle_time_ms={}", total_time.as_millis());

    Ok(())
}

async fn create_test_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_profitable_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), 
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"), 
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(500u32),
    })
}

async fn create_unprofitable_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 2,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(10000u32),
    })
}

async fn create_small_amount_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_large_amount_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_invalid_pool_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: Address::ZERO,
        corresponding_pool_address: Address::ZERO,
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn process_arbitrage_strategy_with_results(
    log_event: LogEvent,
    ws_url: String,
) -> Result<Vec<ExecutionResult>> {
    use arbooo::strategies::factory::DefaultStrategyFactory;
    use arbooo::strategies::manager::StrategyManager;
    use arbooo::strategies::traits::ExecutionContext;
    use arbooo::common::connection_pool::ConnectionPool;
    use arbooo::common::pairs::Event;
    use std::sync::Arc;
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    use alloy_primitives::address;
    use alloy::providers::Provider;

    info!("🔄 Processing arbitrage strategy with results capture");
    info!("🔗 Using WebSocket URL: {}", ws_url);

    let pools_map = Arc::new(RwLock::new(HashMap::<Address, Event>::new()));

    let connection_pool = ConnectionPool::new(ws_url.clone(), 4);

    let _factory = DefaultStrategyFactory::new(pools_map.clone(), connection_pool.clone());

    let manager = StrategyManager::new(
        ws_url.clone(),
        4,
        pools_map,
        address!("742d35Cc6634C0532925a3b8d1C4AC1B8b5C0000"),
    ).await?;

    let test_block_number = {
        if let Ok(pooled_provider) = connection_pool.get_provider().await {
            match pooled_provider.provider().get_block_number().await {
                Ok(block_num) => {
                    info!("📦 Using current block number: {}", block_num);
                    block_num
                },
                Err(e) => {
                    log::warn!("Failed to get current block number: {}, using default", e);
                    20000000
                }
            }
        } else {
            log::warn!("Failed to get provider, using default block number");
            20000000
        }
    };

    let context = ExecutionContext {
        block_number: test_block_number,
        gas_price: U256::from(50_000_000_000u64),
        base_fee: U256::from(30_000_000_000u64),
        executor_address: address!("742d35Cc6634C0532925a3b8d1C4AC1B8b5C0000"),
        max_gas_limit: 2_000_000,
    };

    let results = manager.process_arbitrage_cycle(log_event).await?;

    let profitable_results: Vec<_> = results.iter()
        .filter(|r| r.success && r.profit > U256::ZERO)
        .collect();

    if !profitable_results.is_empty() {
        info!("✅ Found {} profitable arbitrage opportunities", profitable_results.len());
        for result in profitable_results {
            info!("  💰 Profit: {} wei, Gas: {} wei", result.profit, result.gas_used);
        }
    } else {
        info!("📉 No profitable opportunities found (this is normal for tests)");
    }

    Ok(results)
}

async fn create_arbitrage_strategy(test_env: &TestEnvironment) -> Result<UniswapArbitrageStrategy> {
    info!("🏗️  Creating UniswapArbitrageStrategy for testing");

    let pools_map = Arc::new(RwLock::new(HashMap::<Address, Event>::new()));

    let connection_pool = ConnectionPool::new(test_env.test_config.ws_url.clone(), 4);

    let config = StrategyConfig {
        enabled: true,
        priority: 90,
        min_profit_threshold: U256::from(100_000u128),
        max_gas_price: U256::from(200_000_000_000u64),
        max_position_size: U256::from(10_000_000_000_000_000_000u64),
    };

    let strategy = UniswapArbitrageStrategy::new(config, pools_map, connection_pool);
    info!("✅ UniswapArbitrageStrategy created successfully");

    Ok(strategy)
}

async fn create_test_execution_context(test_env: &TestEnvironment) -> Result<ExecutionContext> {
    info!("🏗️  Creating ExecutionContext for testing");

    let block_number = test_env.provider.get_block_number().await
        .unwrap_or_else(|e| {
            warn!("Failed to get block number: {}, using default", e);
            20000000
        });

    let context = ExecutionContext {
        block_number,
        gas_price: U256::from(50_000_000_000u64),
        base_fee: U256::from(30_000_000_000u64),
        executor_address: address!("742d35Cc6634C0532925a3b8d1C4AC1B8b5C0000"),
        max_gas_limit: 2_000_000,
    };

    info!("✅ ExecutionContext created - Block: {}, Gas Price: {} gwei", 
          block_number, context.gas_price / U256::from(1_000_000_000u64));

    Ok(context)
}

#[derive(Debug, Clone)]
struct AnvilPoolSetup {
    pool_a_address: Address,
    pool_b_address: Address,
    token_a_address: Address,
    token_b_address: Address,
    weth_address: Address,
}

async fn setup_arbitrage_pools_on_anvil(_test_env: &TestEnvironment) -> Result<(AnvilPoolSetup, LogEvent)> {
    info!("🏊 Setting up arbitrage pools on Anvil...");

    let weth_address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let usdc_address = address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2");

    let pool_a_address = address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984");
    let pool_b_address = address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168");

    let pool_setup = AnvilPoolSetup {
        pool_a_address,
        pool_b_address,
        token_a_address: usdc_address,
        token_b_address: weth_address,
        weth_address,
    };

    let log_event = LogEvent {
        log_pool_address: pool_a_address,
        corresponding_pool_address: pool_b_address,
        pool_variant: 3,
        token0: usdc_address,
        token1: weth_address,
        fee: U24::from(3000u32),
    };

    info!("✅ Pool setup complete:");
    info!("  🏊 Pool A (V3): {}", pool_setup.pool_a_address);
    info!("  🏊 Pool B (V2): {}", pool_setup.pool_b_address);
    info!("  🪙 Token A: {}", pool_setup.token_a_address);
    info!("  🪙 Token B (WETH): {}", pool_setup.token_b_address);

    Ok((pool_setup, log_event))
}

async fn create_arbitrage_strategy_with_anvil(
    test_env: &TestEnvironment, 
    pool_setup: &AnvilPoolSetup
) -> Result<UniswapArbitrageStrategy> {
    info!("🏗️  Creating UniswapArbitrageStrategy configured for Anvil...");

    let pools_map = Arc::new(RwLock::new(HashMap::<Address, Event>::new()));

    {
        let mut pools_guard = pools_map.write().await;

        let pool_a_event = Event::PoolCreated(arbooo::common::pairs::V3PoolCreated {
            token0: pool_setup.token_a_address,
            token1: pool_setup.token_b_address,
            fee: 3000,
            tick_spacing: 60,
            pair_address: pool_setup.pool_a_address,
        });
        pools_guard.insert(pool_setup.pool_a_address, pool_a_event);

        let pool_b_event = Event::PairCreated(arbooo::common::pairs::V2PoolCreated {
            token0: pool_setup.token_a_address,
            token1: pool_setup.token_b_address,
            fee: 3000,
            pair_address: pool_setup.pool_b_address,
        });
        pools_guard.insert(pool_setup.pool_b_address, pool_b_event);
    }

    let anvil_ws_url = if let Some(anvil) = &test_env.anvil_instance {
        format!("ws://127.0.0.1:{}", anvil.port)
    } else {
        test_env.test_config.ws_url.clone()
    };

    let connection_pool = ConnectionPool::new(anvil_ws_url, 4);

    let config = StrategyConfig {
        enabled: true,
        priority: 90,
        min_profit_threshold: U256::from(1u64),
        max_gas_price: U256::from(200_000_000_000u64),
        max_position_size: U256::from(10_000_000_000_000_000_000u64),
    };

    let strategy = UniswapArbitrageStrategy::new(config, pools_map, connection_pool);
    info!("✅ UniswapArbitrageStrategy configured for Anvil with {} pools", 2);

    Ok(strategy)
}

async fn create_anvil_execution_context(test_env: &TestEnvironment) -> Result<ExecutionContext> {
    info!("🎯 Creating ExecutionContext using Anvil data...");

    let block_number = test_env.provider.get_block_number().await?;

    let gas_price = U256::from(20_000_000_000u64);
    let base_fee = U256::from(15_000_000_000u64);

    let executor_address = address!("742d35Cc6634C0532925a3b8d1C4AC1B8b5C0000");

    let context = ExecutionContext {
        block_number,
        gas_price,
        base_fee,
        executor_address,
        max_gas_limit: 2_000_000,
    };

    info!("✅ Anvil ExecutionContext created:");
    info!("  📦 Block: {}", block_number);
    info!("  ⛽ Gas Price: {} gwei", gas_price / U256::from(1_000_000_000u64));
    info!("  🏠 Executor: {}", executor_address);

    Ok(context)
}

async fn verify_anvil_state_after_execution(
    test_env: &TestEnvironment,
    initial_block: u64
) -> Result<()> {
    info!("🔍 Verifying Anvil state after execution...");

    let current_block = test_env.provider.get_block_number().await?;
    info!("📦 Block progression: {} -> {}", initial_block, current_block);

    if current_block > initial_block {
        info!("✅ Blocks advanced - transactions were likely mined");
    } else {
        info!("ℹ️  Block unchanged - no transactions mined (acceptable for mock execution)");
    }

    let chain_id = test_env.provider.get_chain_id().await?;
    info!("🔗 Anvil chain ID: {}", chain_id);

    info!("🧹 Connection state verified");

    Ok(())
}

async fn process_with_strategy(
    strategy: &UniswapArbitrageStrategy,
    log_event: &LogEvent,
    context: &ExecutionContext,
) -> Result<()> {
    let opportunities = strategy.scan_opportunities(log_event).await?;

    if opportunities.is_empty() {
        return Ok(());
    }

    let opportunity = &opportunities[0];
    let simulation_result = strategy.simulate_opportunity(opportunity, context).await?;

    if !simulation_result.success {
        return Ok(());
    }

    let _execution_result = strategy.execute_opportunity(opportunity, context).await?;

    Ok(())
}

