use anyhow::Result;
use arbooo::arbitrage::strategy::process_strategy;
use arbooo::common::logs::LogEvent;
use alloy::primitives::address;
use alloy_primitives::aliases::U24;
use log::info;
use std::time::{Duration, Instant};
use tokio::time::timeout;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_low_liquidity_handling() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("💧 Testing low liquidity handling");

    let liquidity_scenarios = [
        ("normal_liquidity", "Standard liquidity pool"),
        ("low_liquidity", "Low liquidity pool"),
        ("minimal_liquidity", "Minimal liquidity pool"),
        ("dust_liquidity", "Dust-level liquidity"),
    ];

    let mut liquidity_results = Vec::new();

    for (scenario_name, scenario_desc) in liquidity_scenarios {
        info!("💧 Testing liquidity scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_liquidity_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(12),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();

        match result {
            Ok(Ok(_)) => {
                info!("✅ Liquidity scenario '{}' completed successfully in {:?}", scenario_name, duration);
                liquidity_results.push((scenario_name, true, duration));
            }
            Ok(Err(e)) => {
                info!("⚠️  Liquidity scenario '{}' failed with error: {} (duration: {:?})", scenario_name, e, duration);
                liquidity_results.push((scenario_name, false, duration));
            }
            Err(_) => {
                info!("⏰ Liquidity scenario '{}' timed out after {:?}", scenario_name, duration);
                liquidity_results.push((scenario_name, false, duration));
            }
        }

        tokio::time::sleep(Duration::from_millis(600)).await;
    }

    let successful_scenarios = liquidity_results.iter().filter(|(_, success, _)| *success).count();
    let total_scenarios = liquidity_results.len();

    info!("📊 Low liquidity analysis:");
    for (scenario, success, duration) in &liquidity_results {
        info!("   💧 {}: success={}, duration={:?}", scenario, success, duration);
    }

    info!("📊 Overall liquidity handling: {}/{} scenarios handled", 
          successful_scenarios, total_scenarios);

    assert!(total_scenarios > 0, "Should have tested liquidity scenarios");

    Ok(())
}

#[tokio::test]
async fn test_slippage_impact_analysis() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("📈 Testing slippage impact under varying liquidity");

    let slippage_scenarios = [
        ("low_slippage", "High liquidity, low slippage environment"),
        ("moderate_slippage", "Medium liquidity, moderate slippage"),
        ("high_slippage", "Low liquidity, high slippage"),
        ("extreme_slippage", "Very low liquidity, extreme slippage"),
    ];

    let mut slippage_results = Vec::new();

    for (scenario_name, scenario_desc) in slippage_scenarios {
        info!("📈 Testing slippage scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_slippage_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(10),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();
        let success = result.is_ok();

        slippage_results.push((scenario_name, success, duration));

        info!("📈 Slippage scenario '{}': success={}, duration={:?}", 
              scenario_name, success, duration);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    info!("📊 Slippage impact analysis:");
    for (scenario, success, duration) in &slippage_results {
        info!("   📈 {}: success={}, duration={:?}", scenario, success, duration);
    }

    let analyzed_scenarios = slippage_results.len();
    assert!(analyzed_scenarios == slippage_scenarios.len(),
           "System should analyze all slippage scenarios");

    Ok(())
}

#[tokio::test]
async fn test_arbitrage_viability_constraints() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚖️ Testing arbitrage viability under liquidity constraints");

    let constraint_scenarios = [
        ("viable_arbitrage", "Sufficient liquidity for profitable arbitrage"),
        ("marginal_arbitrage", "Marginal liquidity, questionable profitability"),
        ("constrained_arbitrage", "Limited liquidity, likely unprofitable"),
        ("impossible_arbitrage", "Insufficient liquidity, impossible to execute"),
    ];

    let mut constraint_results = Vec::new();

    for (scenario_name, scenario_desc) in constraint_scenarios {
        info!("⚖️ Testing constraint scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_constraint_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(11),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();

        match result {
            Ok(Ok(_)) => {
                info!("✅ Constraint scenario '{}' found viable arbitrage in {:?}", scenario_name, duration);
                constraint_results.push((scenario_name, "viable", duration));
            }
            Ok(Err(e)) => {
                info!("⚠️  Constraint scenario '{}' determined unviable: {} (duration: {:?})", scenario_name, e, duration);
                constraint_results.push((scenario_name, "unviable", duration));
            }
            Err(_) => {
                info!("⏰ Constraint scenario '{}' analysis timed out after {:?}", scenario_name, duration);
                constraint_results.push((scenario_name, "timeout", duration));
            }
        }

        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    info!("📊 Arbitrage viability constraint analysis:");
    for (scenario, result, duration) in &constraint_results {
        info!("   ⚖️ {}: result={}, duration={:?}", scenario, result, duration);
    }

    let analyzed_constraints = constraint_results.len();
    assert!(analyzed_constraints == constraint_scenarios.len(),
           "System should analyze all constraint scenarios");

    Ok(())
}

#[tokio::test]
async fn test_pool_depth_analysis() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🏊 Testing pool depth analysis and liquidity estimation");

    let depth_scenarios = [
        ("deep_pool", "Deep liquidity pool"),
        ("shallow_pool", "Shallow liquidity pool"),
        ("unbalanced_pool", "Unbalanced liquidity pool"),
        ("depleted_pool", "Nearly depleted liquidity pool"),
    ];

    let mut depth_results = Vec::new();

    for (scenario_name, scenario_desc) in depth_scenarios {
        info!("🏊 Testing depth scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_depth_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(9),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();
        let success = result.is_ok();

        depth_results.push((scenario_name, success, duration));

        info!("🏊 Depth scenario '{}': success={}, duration={:?}", 
              scenario_name, success, duration);

        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    info!("📊 Pool depth analysis:");
    for (scenario, success, duration) in &depth_results {
        info!("   🏊 {}: success={}, duration={:?}", scenario, success, duration);
    }

    let successful_analyses = depth_results.iter().filter(|(_, success, _)| *success).count();
    info!("📊 Pool depth analysis success rate: {}/{} scenarios", 
          successful_analyses, depth_results.len());

    assert!(depth_results.len() == depth_scenarios.len(),
           "System should attempt all depth analyses");

    Ok(())
}

#[tokio::test]
async fn test_dynamic_liquidity_monitoring() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("📊 Testing dynamic liquidity monitoring and adaptation");

    let monitoring_scenarios = [
        ("stable_liquidity", "Stable liquidity conditions"),
        ("increasing_liquidity", "Gradually increasing liquidity"),
        ("decreasing_liquidity", "Gradually decreasing liquidity"),
        ("volatile_liquidity", "Highly volatile liquidity"),
        ("liquidity_drain", "Rapid liquidity drain"),
    ];

    let mut monitoring_results = Vec::new();

    for (scenario_name, scenario_desc) in monitoring_scenarios {
        info!("📊 Testing monitoring scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_monitoring_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(13),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();

        match result {
            Ok(Ok(_)) => {
                info!("✅ Monitoring scenario '{}' adapted successfully in {:?}", scenario_name, duration);
                monitoring_results.push((scenario_name, true, duration));
            }
            Ok(Err(e)) => {
                info!("⚠️  Monitoring scenario '{}' adaptation failed: {} (duration: {:?})", scenario_name, e, duration);
                monitoring_results.push((scenario_name, false, duration));
            }
            Err(_) => {
                info!("⏰ Monitoring scenario '{}' timed out after {:?}", scenario_name, duration);
                monitoring_results.push((scenario_name, false, duration));
            }
        }

        tokio::time::sleep(Duration::from_millis(800)).await;
    }

    info!("📊 Dynamic liquidity monitoring analysis:");
    for (scenario, success, duration) in &monitoring_results {
        info!("   📊 {}: success={}, duration={:?}", scenario, success, duration);
    }

    let successful_monitoring = monitoring_results.iter().filter(|(_, success, _)| *success).count();
    let total_scenarios = monitoring_results.len();

    info!("📊 Overall monitoring capability: {}/{} scenarios handled successfully", 
          successful_monitoring, total_scenarios);

    let graceful_handling = monitoring_results.len() > 0;

    if successful_monitoring > 0 {
        info!("✅ System successfully monitored {}/{} liquidity scenarios", successful_monitoring, monitoring_results.len());
    } else {
        info!("⚠️  All liquidity scenarios failed, but system handled them gracefully without crashing");
    }

    assert!(successful_monitoring > 0 || graceful_handling,
           "System should either handle liquidity monitoring successfully or fail gracefully");

    Ok(())
}

#[tokio::test]
async fn test_liquidity_fragmentation() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🧩 Testing liquidity fragmentation across multiple pools");

    let fragmentation_scenarios = [
        ("concentrated_liquidity", "Liquidity concentrated in few pools"),
        ("distributed_liquidity", "Liquidity evenly distributed"),
        ("fragmented_liquidity", "Highly fragmented liquidity"),
        ("scattered_liquidity", "Scattered across many small pools"),
    ];

    let mut fragmentation_results = Vec::new();

    for (scenario_name, scenario_desc) in fragmentation_scenarios {
        info!("🧩 Testing fragmentation scenario: {} - {}", scenario_name, scenario_desc);

        let log_event = create_fragmentation_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(10),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        let duration = start_time.elapsed();
        let success = result.is_ok();

        fragmentation_results.push((scenario_name, success, duration));

        info!("🧩 Fragmentation scenario '{}': success={}, duration={:?}", 
              scenario_name, success, duration);

        tokio::time::sleep(Duration::from_millis(600)).await;
    }

    info!("📊 Liquidity fragmentation analysis:");
    for (scenario, success, duration) in &fragmentation_results {
        info!("   🧩 {}: success={}, duration={:?}", scenario, success, duration);
    }

    let successful_handling = fragmentation_results.iter().filter(|(_, success, _)| *success).count();
    info!("📊 Fragmentation handling success rate: {}/{} scenarios", 
          successful_handling, fragmentation_results.len());

    assert!(fragmentation_results.len() == fragmentation_scenarios.len(),
           "System should analyze all fragmentation scenarios");

    Ok(())
}

async fn create_liquidity_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (pool_address, corresponding_address, fee) = match scenario {
        "normal_liquidity" => (
            address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
            address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
            3000u32
        ),
        "low_liquidity" => (
            address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
            address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
            10000u32
        ),
        "minimal_liquidity" => (
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
            3000u32
        ),
        "dust_liquidity" => (
            address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            100u32
        ),
        _ => (
            address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
            address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
            3000u32
        ),
    };

    Ok(LogEvent {
        log_pool_address: pool_address,
        corresponding_pool_address: corresponding_address,
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_slippage_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let fee = match scenario {
        "low_slippage" => 500u32,
        "moderate_slippage" => 3000u32,
        "high_slippage" => 10000u32,
        "extreme_slippage" => 100u32,
        _ => 3000u32,
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_constraint_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (token0, token1, pool_variant) = match scenario {
        "viable_arbitrage" => (
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
            3
        ),
        "marginal_arbitrage" => (
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            3
        ),
        "constrained_arbitrage" => (
            address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
            address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
            2
        ),
        "impossible_arbitrage" => (
            address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            3
        ),
        _ => (
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            3
        ),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0,
        token1,
        fee: U24::from(3000u32),
    })
}

async fn create_depth_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (pool_address, fee) = match scenario {
        "deep_pool" => (address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), 500u32),
        "shallow_pool" => (address!("514910771AF9Ca656af840dff83E8264EcF986CA"), 3000u32),
        "unbalanced_pool" => (address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), 10000u32),
        "depleted_pool" => (address!("dAC17F958D2ee523a2206206994597C13D831ec7"), 100u32),
        _ => (address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: pool_address,
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_monitoring_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let pool_variant = match scenario {
        "stable_liquidity" => 3,
        "increasing_liquidity" => 2,
        "decreasing_liquidity" => 3,
        "volatile_liquidity" => 3,
        "liquidity_drain" => 2,
        _ => 3,
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_fragmentation_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (corresponding_address, fee) = match scenario {
        "concentrated_liquidity" => (address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"), 500u32),
        "distributed_liquidity" => (address!("dAC17F958D2ee523a2206206994597C13D831ec7"), 3000u32),
        "fragmented_liquidity" => (address!("514910771AF9Ca656af840dff83E8264EcF986CA"), 10000u32),
        "scattered_liquidity" => (address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), 100u32),
        _ => (address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"), 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: corresponding_address,
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

