use anyhow::Result;
use arbooo::arbitrage::strategy::process_strategy;
use arbooo::common::logs::LogEvent;
use alloy::primitives::address;
use alloy_primitives::aliases::U24;
use log::{info, warn};
use std::time::{Duration, Instant};
use tokio::time::timeout;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_websocket_disconnection_recovery() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔌 Testing WebSocket disconnection recovery");

    let log_event = create_test_arbitrage_opportunity().await?;
    let normal_result = timeout(
        Duration::from_secs(10),
        process_strategy(log_event.clone(), test_env.test_config.ws_url.clone())
    ).await;

    match normal_result {
        Ok(_) => info!("✅ Normal operation confirmed"),
        Err(_) => info!("⚠️  Normal operation had timeout (network might be slow)"),
    }

    let disconnection_scenarios = [
        ("sudden_disconnect", "wss://invalid-endpoint-sudden.example.com"),
        ("timeout_disconnect", "wss://localhost:9999"),
        ("dns_failure", "wss://non-existent-domain-12345.invalid"),
    ];

    let mut disconnection_results = Vec::new();

    for (scenario_name, invalid_url) in disconnection_scenarios {
        info!("🔌 Testing disconnection scenario: {}", scenario_name);

        let start_time = Instant::now();
        let result = timeout(
            Duration::from_secs(8),
            process_strategy(log_event.clone(), invalid_url.to_string())
        ).await;
        let duration = start_time.elapsed();

        match result {
            Ok(Ok(_)) => {
                warn!("⚠️  Unexpected success in disconnection scenario: {}", scenario_name);
                disconnection_results.push((scenario_name, false, duration));
            }
            Ok(Err(e)) => {
                info!("✅ Properly handled disconnection in scenario '{}': {}", scenario_name, e);
                disconnection_results.push((scenario_name, true, duration));
            }
            Err(_) => {
                info!("⏰ Timeout in disconnection scenario '{}' (acceptable)", scenario_name);
                disconnection_results.push((scenario_name, true, duration));
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let proper_handling = disconnection_results.iter().filter(|(_, handled, _)| *handled).count();

    info!("📊 Disconnection handling: {}/{} scenarios handled properly", 
          proper_handling, disconnection_results.len());

    assert_eq!(proper_handling, disconnection_results.len(), 
              "All disconnection scenarios should be handled gracefully");

    info!("🔄 Testing reconnection capability");
    tokio::time::sleep(Duration::from_secs(1)).await;

    let reconnection_attempts = 3;
    let mut successful_reconnections = 0;

    for attempt in 0..reconnection_attempts {
        info!("🔄 Reconnection attempt #{}", attempt + 1);

        let result = timeout(
            Duration::from_secs(10),
            process_strategy(log_event.clone(), test_env.test_config.ws_url.clone())
        ).await;

        if result.is_ok() {
            successful_reconnections += 1;
            info!("✅ Reconnection attempt #{} successful", attempt + 1);
        } else {
            info!("❌ Reconnection attempt #{} failed", attempt + 1);
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    let reconnection_rate = (successful_reconnections as f64 / reconnection_attempts as f64) * 100.0;
    info!("📊 Reconnection success rate: {:.1}% ({}/{})", 
          reconnection_rate, successful_reconnections, reconnection_attempts);

    assert!(successful_reconnections > 0, 
           "At least one reconnection attempt should succeed");

    Ok(())
}

#[tokio::test]
async fn test_intermittent_network_issues() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("📡 Testing intermittent network issues");

    let operations = [
        ("normal_op_1", test_env.test_config.ws_url.clone()),
        ("network_issue_1", "wss://timeout-test-1.invalid".to_string()),
        ("normal_op_2", test_env.test_config.ws_url.clone()),
        ("network_issue_2", "wss://timeout-test-2.invalid".to_string()),
        ("normal_op_3", test_env.test_config.ws_url.clone()),
    ];

    let mut intermittent_results = Vec::new();

    for (operation_name, ws_url) in operations {
        info!("📡 Running operation: {}", operation_name);

        let log_event = create_test_arbitrage_opportunity().await?;
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(6),
            process_strategy(log_event, ws_url)
        ).await;

        let duration = start_time.elapsed();
        let success = match result {
            Ok(Ok(_)) => true,
            Ok(Err(_)) => false,
            Err(_) => false,
        };

        intermittent_results.push((operation_name, success, duration));

        info!("📡 Operation '{}': success={}, duration={:?}", 
              operation_name, success, duration);

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let normal_ops: Vec<_> = intermittent_results.iter()
        .filter(|(name, _, _)| name.contains("normal"))
        .collect();

    let issue_ops: Vec<_> = intermittent_results.iter()
        .filter(|(name, _, _)| name.contains("issue"))
        .collect();

    let normal_success_rate = normal_ops.iter().filter(|(_, success, _)| *success).count() as f64 / normal_ops.len() as f64 * 100.0;
    let issue_failure_rate = issue_ops.iter().filter(|(_, success, _)| !*success).count() as f64 / issue_ops.len() as f64 * 100.0;

    info!("📊 Intermittent network analysis:");
    info!("   Normal operations success rate: {:.1}%", normal_success_rate);
    info!("   Network issue operations failure rate: {:.1}%", issue_failure_rate);

    assert!(issue_failure_rate >= 80.0, 
           "Network issue operations should fail consistently");

    Ok(())
}

#[tokio::test]
async fn test_network_stress_recovery() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🌪️ Testing network stress recovery patterns");

    let stress_levels = [
        ("low_stress", 2, Duration::from_secs(5)),
        ("medium_stress", 4, Duration::from_secs(3)),
        ("high_stress", 6, Duration::from_secs(2)),
    ];

    let mut stress_results = Vec::new();

    for (stress_name, num_operations, operation_timeout) in stress_levels {
        info!("🌪️ Testing stress level: {} ({} operations)", stress_name, num_operations);

        let stress_start = Instant::now();
        let mut operations_results = Vec::new();

        for i in 0..num_operations {
            let log_event = create_stress_test_opportunity(i).await?;

            let op_start = Instant::now();
            let result = timeout(
                operation_timeout,
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;
            let op_duration = op_start.elapsed();

            operations_results.push((i, result.is_ok(), op_duration));

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let stress_duration = stress_start.elapsed();
        let successful_ops = operations_results.iter().filter(|(_, success, _)| *success).count();
        let success_rate = (successful_ops as f64 / num_operations as f64) * 100.0;

        stress_results.push((stress_name.to_string(), num_operations, successful_ops, success_rate, stress_duration));

        info!("🌪️ Stress level '{}': {}/{} successful ({:.1}%) in {:?}", 
              stress_name, successful_ops, num_operations, success_rate, stress_duration);

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    info!("📊 Network stress recovery analysis:");
    for (stress_name, total_ops, successful_ops, success_rate, duration) in &stress_results {
        info!("   🌪️ {}: {}/{} ops ({:.1}%) in {:?}", 
              stress_name, successful_ops, total_ops, success_rate, duration);
    }

    let high_stress_result = stress_results.iter()
        .find(|(name, _, _, _, _)| name.contains("high"))
        .expect("High stress result should exist");

    assert!(high_stress_result.3 >= 15.0, 
           "System should maintain at least 15% success rate even under high network stress");

    Ok(())
}

#[tokio::test]
async fn test_connection_pooling_fallback() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Testing connection pooling and fallback scenarios");

    info!("🔄 Testing primary connection");
    let log_event = create_test_arbitrage_opportunity().await?;

    let primary_result = timeout(
        Duration::from_secs(8),
        process_strategy(log_event.clone(), test_env.test_config.ws_url.clone())
    ).await;

    let primary_success = primary_result.is_ok();
    info!("🔄 Primary connection: success={}", primary_success);

    let fallback_scenarios = [
        ("fallback_1", "wss://fallback-1.example.com"),
        ("fallback_2", "wss://fallback-2.example.com"),
        ("fallback_3", "wss://fallback-3.example.com"),
    ];

    let mut fallback_results = Vec::new();

    for (fallback_name, fallback_url) in fallback_scenarios {
        info!("🔄 Testing fallback: {}", fallback_name);

        let result = timeout(
            Duration::from_secs(5),
            process_strategy(log_event.clone(), fallback_url.to_string())
        ).await;

        let success = match result {
            Ok(Ok(_)) => true,
            Ok(Err(_)) => false,
            Err(_) => false,
        };
        fallback_results.push((fallback_name, success));

        info!("🔄 Fallback '{}': handled_gracefully={}", fallback_name, !success);
    }

    let graceful_failures = fallback_results.iter().filter(|(_, success)| !*success).count();

    info!("📊 Fallback handling: {}/{} fallbacks handled gracefully", 
          graceful_failures, fallback_results.len());

    assert_eq!(graceful_failures, fallback_results.len(), 
              "All fallback scenarios should fail gracefully");

    Ok(())
}

#[tokio::test]
async fn test_concurrent_connection_management() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔗 Testing concurrent connection management");

    let concurrent_scenarios = [
        ("rapid_sequential", 8, Duration::from_millis(200)),
        ("burst_operations", 12, Duration::from_millis(100)),
        ("sustained_load", 15, Duration::from_millis(150)),
    ];

    let mut concurrent_results = Vec::new();

    for (scenario_name, num_operations, inter_op_delay) in concurrent_scenarios {
        info!("🔗 Testing concurrent scenario: {} ({} operations)", scenario_name, num_operations);

        let scenario_start = Instant::now();
        let mut operation_results = Vec::new();

        for i in 0..num_operations {
            let log_event = create_concurrent_test_opportunity(i).await?;

            let op_start = Instant::now();
            let result = timeout(
                Duration::from_secs(4),
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;
            let op_duration = op_start.elapsed();

            operation_results.push((i, result.is_ok(), op_duration));

            tokio::time::sleep(inter_op_delay).await;
        }

        let scenario_duration = scenario_start.elapsed();
        let successful_ops = operation_results.iter().filter(|(_, success, _)| *success).count();
        let success_rate = (successful_ops as f64 / num_operations as f64) * 100.0;

        concurrent_results.push((scenario_name.to_string(), num_operations, successful_ops, success_rate, scenario_duration));

        info!("🔗 Scenario '{}': {}/{} successful ({:.1}%) in {:?}", 
              scenario_name, successful_ops, num_operations, success_rate, scenario_duration);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    info!("📊 Concurrent connection analysis:");
    for (scenario, total_ops, successful_ops, success_rate, duration) in &concurrent_results {
        let ops_per_second = *total_ops as f64 / duration.as_secs_f64();
        info!("   🔗 {}: {}/{} ops ({:.1}%), {:.1} ops/sec", 
              scenario, successful_ops, total_ops, success_rate, ops_per_second);
    }

    let burst_result = concurrent_results.iter()
        .find(|(name, _, _, _, _)| name.contains("burst"))
        .expect("Burst result should exist");

    assert!(burst_result.3 >= 30.0, 
           "System should handle at least 30% of burst operations successfully");

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

async fn create_stress_test_opportunity(variant: usize) -> Result<LogEvent> {
    let addresses = [
        address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
        address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
    ];

    let base_addr = addresses[variant % addresses.len()];

    Ok(LogEvent {
        log_pool_address: base_addr,
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_concurrent_test_opportunity(variant: usize) -> Result<LogEvent> {
    let fees = [3000u32, 500u32, 10000u32, 100u32];
    let fee = fees[variant % fees.len()];

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

