use anyhow::Result;
use arbooo::strategies::arbitrage::UniswapArbitrageStrategy;
use arbooo::strategies::traits::{MevStrategy, ExecutionContext, StrategyConfig};
use arbooo::common::logs::LogEvent;
use arbooo::common::connection_pool::ConnectionPool;
use arbooo::common::pairs::Event;
use alloy::primitives::address;
use alloy::providers::Provider;
use alloy_primitives::aliases::U24;
use alloy_primitives::U256;
use log::info;
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
async fn test_sequential_arbitrage_opportunities() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Testing sequential arbitrage opportunities (simulating concurrent load)");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let num_opportunities = 5;
    let mut durations = Vec::new();
    let start_time = Instant::now();

    for i in 0..num_opportunities {
        info!("🚀 Processing arbitrage opportunity #{}", i);
        let log_event = create_test_arbitrage_opportunity_variant(i).await?;

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(15),
            process_with_strategy(&strategy, &log_event, &context)
        ).await;

        let duration = start.elapsed();
        durations.push(duration);

        match result {
            Ok(_) => info!("✅ Opportunity #{} completed in {:?}", i, duration),
            Err(_) => info!("⏰ Opportunity #{} timed out", i),
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let total_time = start_time.elapsed();
    let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;

    info!("📊 {} opportunities processed in {:?} (avg: {:?})", 
          num_opportunities, total_time, avg_duration);

    assert_eq!(durations.len(), num_opportunities);

    assert!(avg_duration < Duration::from_secs(10), 
           "Average processing time should be reasonable");

    Ok(())
}

#[tokio::test]
async fn test_rapid_fire_opportunities() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚡ Testing rapid-fire arbitrage opportunities");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let num_opportunities = 10;
    let mut successful = 0;
    let mut total_time = Duration::ZERO;

    for i in 0..num_opportunities {
        let log_event = create_test_arbitrage_opportunity_variant(i).await?;

        let start = Instant::now();

        let result = timeout(
            Duration::from_secs(5),
            process_with_strategy(&strategy, &log_event, &context)
        ).await;

        let duration = start.elapsed();
        total_time += duration;

        match result {
            Ok(_) => {
                successful += 1;
                info!("✅ Rapid opportunity #{} completed in {:?}", i, duration);
            }
            Err(_) => {
                info!("⏰ Rapid opportunity #{} timed out", i);
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let avg_time = total_time / num_opportunities as u32;
    let success_rate = (successful as f64 / num_opportunities as f64) * 100.0;

    info!("📊 Rapid-fire results: {}/{} successful ({:.1}%), avg time: {:?}", 
          successful, num_opportunities, success_rate, avg_time);

    assert!(success_rate >= 60.0, 
           "Success rate under rapid-fire should be at least 60%");

    Ok(())
}

#[tokio::test]
async fn test_mixed_priority_opportunities() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚡ Testing mixed priority opportunities");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let mut results = Vec::new();

    for i in 0..3 {
        let log_event = create_high_profit_opportunity(i).await?;

        let start = Instant::now();
        let result = process_with_strategy(&strategy, &log_event, &context).await;
        let duration = start.elapsed();

        results.push(("high_profit", i, result.is_ok(), duration));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    for i in 0..3 {
        let log_event = create_low_profit_opportunity(i).await?;

        let start = Instant::now();
        let result = process_with_strategy(&strategy, &log_event, &context).await;
        let duration = start.elapsed();

        results.push(("low_profit", i, result.is_ok(), duration));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let high_profit_count = results.iter()
        .filter(|(ptype, _, success, _)| ptype == &"high_profit" && *success)
        .count();
    let low_profit_count = results.iter()
        .filter(|(ptype, _, success, _)| ptype == &"low_profit" && *success)
        .count();

    info!("📊 Mixed priority results: {} high-profit, {} low-profit completed", 
          high_profit_count, low_profit_count);

    for (ptype, id, success, duration) in results {
        info!("📈 {}_{}({}): completed={}, duration={:?}", 
              ptype, id, id, success, duration);
    }

    Ok(())
}

#[tokio::test]
async fn test_opportunities_with_network_delays() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🐌 Testing opportunities with simulated network delays");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let delays = [50, 100, 150, 200, 250];
    let mut results = Vec::new();

    for (i, delay_ms) in delays.iter().enumerate() {
        info!("⏳ Processing opportunity with {}ms delay", delay_ms);

        tokio::time::sleep(Duration::from_millis(*delay_ms)).await;

        let log_event = create_test_arbitrage_opportunity_variant(i).await?;

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(10),
            process_with_strategy(&strategy, &log_event, &context)
        ).await;
        let duration = start.elapsed();

        results.push((*delay_ms, result.is_ok(), duration));

        info!("📊 Delay {}ms: success={}, processing_time={:?}", 
              delay_ms, result.is_ok(), duration);
    }

    let successful_count = results.iter().filter(|(_, success, _)| *success).count();
    info!("� {}/{} opportunities completed with network delays", 
          successful_count, results.len());

    Ok(())
}

#[tokio::test]
async fn test_sustained_load() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔧 Testing system under sustained load");

    let strategy = create_arbitrage_strategy(&test_env).await?;
    let context = create_test_execution_context(&test_env).await?;

    let rounds = 3;
    let opportunities_per_round = 4;
    let mut round_results = Vec::new();

    for round in 0..rounds {
        info!("🔄 Starting load test round {}/{}", round + 1, rounds);

        let round_start = Instant::now();
        let mut round_successes = 0;

        for i in 0..opportunities_per_round {
            let log_event = create_test_arbitrage_opportunity_variant(i).await?;

            let result = timeout(
                Duration::from_secs(8),
                process_with_strategy(&strategy, &log_event, &context)
            ).await;

            if result.is_ok() {
                round_successes += 1;
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let round_duration = round_start.elapsed();
        round_results.push((round, round_successes, opportunities_per_round, round_duration));

        info!("✅ Round {} completed: {}/{} successful in {:?}", 
              round + 1, round_successes, opportunities_per_round, round_duration);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let total_opportunities = rounds * opportunities_per_round;
    let total_successes: usize = round_results.iter().map(|(_, s, _, _)| s).sum();
    let success_rate = (total_successes as f64 / total_opportunities as f64) * 100.0;

    info!("📊 Sustained load test: {}/{} total successes ({:.1}%)", 
          total_successes, total_opportunities, success_rate);

    assert!(success_rate >= 50.0, 
           "System should maintain at least 50% success rate under sustained load");

    Ok(())
}

async fn create_arbitrage_strategy(test_env: &TestEnvironment) -> Result<UniswapArbitrageStrategy> {
    let config = StrategyConfig {
        enabled: true,
        max_gas_price: U256::from(100_000_000_000u64),
        min_profit_threshold: U256::from(1_000_000u64),
        max_position_size: U256::from(10_000_000_000_000_000_000u64),
        priority: 90,
    };

    let strategy = UniswapArbitrageStrategy::new(config, test_env.test_config.ws_url.clone(), 4).await?;

    Ok(strategy)
}

async fn create_test_execution_context(test_env: &TestEnvironment) -> Result<ExecutionContext> {
    let block_number = test_env.provider.get_block_number().await
        .unwrap_or(20_000_000);

    Ok(ExecutionContext {
        block_number,
        gas_price: U256::from(50_000_000_000u64),
        base_fee: U256::from(30_000_000_000u64),
        max_gas_limit: 2_000_000,
    })
}

async fn process_with_strategy(
    strategy: &UniswapArbitrageStrategy,
    log_event: &LogEvent,
    context: &ExecutionContext,
) -> Result<()> {

    let opportunities = strategy.identify_opportunities(log_event, context).await?;

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

async fn create_test_arbitrage_opportunity_variant(variant: usize) -> Result<LogEvent> {
    let base_addresses = [
        (address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168")),
        (address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
        (address!("dAC17F958D2ee523a2206206994597C13D831ec7"), address!("6B175474E89094C44Da98b954EedeAC495271d0F")),
        (address!("514910771AF9Ca656af840dff83E8264EcF986CA"), address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
        (address!("7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9"), address!("95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE")),
    ];

    let (log_pool, corresponding_pool) = base_addresses[variant % base_addresses.len()];

    Ok(LogEvent {
        log_pool_address: log_pool,
        corresponding_pool_address: corresponding_pool,
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_high_profit_opportunity(variant: usize) -> Result<LogEvent> {
    let mut event = create_test_arbitrage_opportunity_variant(variant).await?;
    event.fee = U24::from(500u32);
    Ok(event)
}

async fn create_low_profit_opportunity(variant: usize) -> Result<LogEvent> {
    let mut event = create_test_arbitrage_opportunity_variant(variant).await?;
    event.fee = U24::from(10000u32);
    Ok(event)
}

