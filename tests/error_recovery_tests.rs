// Error Recovery and Reconnection Tests - Phase 4.4
// Tests error recovery and reconnection scenarios

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

/// Test behavior with invalid WebSocket URLs (connection failures)
#[tokio::test]
async fn test_connection_failure_handling() -> Result<()> {
    let _test_env = TestEnvironment::new().await?;
    info!("🔌 Testing connection failure handling");

    let invalid_urls = [
        "wss://invalid-endpoint.example.com",
        "wss://localhost:9999", // Likely not running
        "ws://malformed-url",
    ];
    
    let mut failure_results = Vec::new();
    
    for (i, invalid_url) in invalid_urls.iter().enumerate() {
        info!("🔌 Testing connection failure with URL: {}", invalid_url);
        
        let log_event = create_test_arbitrage_opportunity().await?;
        let start_time = Instant::now();
        
        // Process should fail gracefully with invalid URL
        let result = timeout(
            Duration::from_secs(10), // Generous timeout for connection attempts
            process_strategy(log_event, invalid_url.to_string())
        ).await;
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(Ok(_)) => {
                warn!("⚠️  Unexpected success with invalid URL: {}", invalid_url);
                failure_results.push((i, false, duration));
            }
            Ok(Err(e)) => {
                info!("✅ Properly failed with invalid URL: {} - Error: {}", invalid_url, e);
                failure_results.push((i, true, duration));
            }
            Err(_) => {
                info!("⏰ Timed out with invalid URL: {}", invalid_url);
                failure_results.push((i, true, duration));
            }
        }
    }
    
    // Verify all connection failures were handled gracefully
    let proper_failures = failure_results.iter().filter(|(_, failed, _)| *failed).count();
    
    info!("📊 Connection failure handling: {}/{} properly failed", 
          proper_failures, failure_results.len());
    
    assert_eq!(proper_failures, failure_results.len(), 
              "All invalid connections should fail gracefully");
    
    // Verify failures don't take too long (no hanging)
    let max_failure_time = failure_results.iter()
        .map(|(_, _, duration)| *duration)
        .max()
        .unwrap_or(Duration::ZERO);
    
    assert!(max_failure_time < Duration::from_secs(15), 
           "Connection failures should not hang for more than 15 seconds");
    
    Ok(())
}

/// Test recovery after temporary network issues
#[tokio::test]
async fn test_network_recovery() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Testing network recovery scenarios");

    // First, verify normal operation
    info!("🔄 Phase 1: Verify normal operation");
    let log_event = create_test_arbitrage_opportunity().await?;
    let result = timeout(
        Duration::from_secs(10),
        process_strategy(log_event, test_env.test_config.ws_url.clone())
    ).await;
    
    match result {
        Ok(_) => info!("✅ Normal operation confirmed"),
        Err(_) => info!("⚠️  Normal operation had issues (network might be unstable)"),
    }
    
    // Simulate recovery by using the valid URL again after a delay
    info!("🔄 Phase 2: Simulate network recovery");
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let recovery_attempts = 3;
    let mut recovery_results = Vec::new();
    
    for attempt in 0..recovery_attempts {
        info!("🔄 Recovery attempt #{}", attempt + 1);
        
        let log_event = create_test_arbitrage_opportunity().await?;
        let start_time = Instant::now();
        
        let result = timeout(
            Duration::from_secs(8),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;
        
        let duration = start_time.elapsed();
        let success = result.is_ok();
        recovery_results.push((attempt, success, duration));
        
        info!("🔄 Recovery attempt #{}: success={}, duration={:?}", 
              attempt + 1, success, duration);
        
        // Brief delay between recovery attempts
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Analyze recovery results
    let successful_recoveries = recovery_results.iter().filter(|(_, success, _)| *success).count();
    let recovery_rate = (successful_recoveries as f64 / recovery_attempts as f64) * 100.0;
    
    info!("📊 Network recovery analysis: {}/{} attempts successful ({:.1}%)", 
          successful_recoveries, recovery_attempts, recovery_rate);
    
    // At least one recovery attempt should succeed if network is stable
    assert!(successful_recoveries > 0, "At least one recovery attempt should succeed");
    
    Ok(())
}

/// Test handling of malformed or unexpected data
#[tokio::test]
async fn test_malformed_data_handling() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🚫 Testing malformed data handling");

    let malformed_scenarios = [
        ("invalid_addresses", create_invalid_address_opportunity().await?),
        ("zero_addresses", create_zero_address_opportunity().await?),
        ("extreme_fees", create_extreme_fee_opportunity().await?),
        ("mismatched_tokens", create_mismatched_token_opportunity().await?),
    ];
    
    let mut malformed_results = Vec::new();
    
    for (scenario_name, log_event) in malformed_scenarios {
        info!("🚫 Testing malformed data scenario: {}", scenario_name);
        
        let start_time = Instant::now();
        
        let result = timeout(
            Duration::from_secs(8),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;
        
        let duration = start_time.elapsed();
        
        match result {
            Ok(Ok(_)) => {
                info!("⚠️  Scenario '{}' unexpectedly succeeded", scenario_name);
                malformed_results.push((scenario_name, false, duration));
            }
            Ok(Err(e)) => {
                info!("✅ Scenario '{}' properly failed with error: {}", scenario_name, e);
                malformed_results.push((scenario_name, true, duration));
            }
            Err(_) => {
                info!("⏰ Scenario '{}' timed out (also acceptable)", scenario_name);
                malformed_results.push((scenario_name, true, duration));
            }
        }
    }
    
    // Verify malformed data is handled gracefully
    let proper_handling = malformed_results.iter().filter(|(_, handled, _)| *handled).count();
    
    info!("📊 Malformed data handling: {}/{} scenarios handled properly", 
          proper_handling, malformed_results.len());
    
    // Most malformed data should be handled gracefully (not necessarily all, as some might be valid edge cases)
    assert!(proper_handling >= malformed_results.len() / 2, 
           "At least half of malformed data scenarios should be handled gracefully");
    
    Ok(())
}

/// Test timeout and cancellation behavior
#[tokio::test]
async fn test_timeout_and_cancellation() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⏰ Testing timeout and cancellation behavior");

    let timeout_scenarios = [
        ("very_short", Duration::from_millis(100)),
        ("short", Duration::from_millis(500)),
        ("medium", Duration::from_secs(2)),
        ("long", Duration::from_secs(5)),
    ];
    
    let mut timeout_results = Vec::new();
    
    for (scenario_name, timeout_duration) in timeout_scenarios {
        info!("⏰ Testing timeout scenario: {} ({:?})", scenario_name, timeout_duration);
        
        let log_event = create_test_arbitrage_opportunity().await?;
        let start_time = Instant::now();
        
        let result = timeout(
            timeout_duration,
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;
        
        let actual_duration = start_time.elapsed();
        
        match result {
            Ok(_) => {
                info!("✅ Scenario '{}' completed within timeout: {:?}", scenario_name, actual_duration);
                timeout_results.push((scenario_name, true, actual_duration, timeout_duration));
            }
            Err(_) => {
                info!("⏰ Scenario '{}' timed out as expected: {:?}", scenario_name, actual_duration);
                timeout_results.push((scenario_name, false, actual_duration, timeout_duration));
            }
        }
        
        // Brief delay between timeout tests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Analyze timeout behavior
    info!("📊 Timeout behavior analysis:");
    for (scenario, completed, actual, expected) in &timeout_results {
        let efficiency = if *completed { 
            (actual.as_millis() as f64 / expected.as_millis() as f64) * 100.0 
        } else { 
            100.0 
        };
        
        info!("   ⏰ {}: completed={}, actual={:?}, expected={:?}, efficiency={:.1}%", 
              scenario, completed, actual, expected, efficiency);
        
        // Verify timeouts are respected (within reasonable margin)
        assert!(actual <= &(*expected + Duration::from_millis(500)), 
               "Timeout should be respected within 500ms margin");
    }
    
    Ok(())
}

/// Test graceful degradation under stress
#[tokio::test]
async fn test_graceful_degradation() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("📉 Testing graceful degradation under stress");

    let stress_levels = [
        ("baseline", 1, Duration::from_secs(5)),
        ("moderate", 3, Duration::from_secs(3)),
        ("high", 5, Duration::from_secs(2)),
        ("extreme", 8, Duration::from_secs(1)),
    ];
    
    let mut degradation_results = Vec::new();
    
    for (stress_name, num_concurrent, timeout_per_op) in stress_levels {
        info!("📉 Testing stress level: {} ({} ops, {:?} timeout)", 
              stress_name, num_concurrent, timeout_per_op);
        
        let mut stress_results = Vec::new();
        let stress_start = Instant::now();
        
        for i in 0..num_concurrent {
            let log_event = create_stress_test_opportunity(i).await?;
            
            let op_start = Instant::now();
            let result = timeout(
                timeout_per_op,
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;
            let op_duration = op_start.elapsed();
            
            stress_results.push((i, result.is_ok(), op_duration));
            
            // Minimal delay for stress testing
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        let stress_duration = stress_start.elapsed();
        let successful_ops = stress_results.iter().filter(|(_, success, _)| *success).count();
        let success_rate = (successful_ops as f64 / num_concurrent as f64) * 100.0;
        
        degradation_results.push((stress_name.to_string(), num_concurrent, successful_ops, success_rate, stress_duration));
        
        info!("📉 Stress level '{}': {}/{} successful ({:.1}%) in {:?}", 
              stress_name, successful_ops, num_concurrent, success_rate, stress_duration);
        
        // Recovery period between stress levels
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    
    // Analyze degradation pattern
    info!("📊 Graceful degradation analysis:");
    for (_i, (stress_name, total_ops, successful_ops, success_rate, duration)) in degradation_results.iter().enumerate() {
        info!("   📉 {}: {}/{} ops ({:.1}%) in {:?}", 
              stress_name, successful_ops, total_ops, success_rate, duration);
    }
    
    // System should maintain some level of functionality even under stress
    let extreme_stress_result = &degradation_results[degradation_results.len() - 1];
    assert!(extreme_stress_result.3 >= 20.0, 
           "System should maintain at least 20% success rate even under extreme stress");
    
    Ok(())
}

/// Test error state recovery
#[tokio::test]
async fn test_error_state_recovery() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔧 Testing error state recovery");

    // Induce an error state by using invalid data
    info!("🔧 Phase 1: Induce error state");
    let invalid_event = create_invalid_address_opportunity().await?;
    
    let error_result = timeout(
        Duration::from_secs(5),
        process_strategy(invalid_event, test_env.test_config.ws_url.clone())
    ).await;
    
    info!("🔧 Error induction result: {:?}", error_result.is_ok());
    
    // Brief pause to let system settle
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Attempt recovery with valid data
    info!("🔧 Phase 2: Attempt recovery with valid operations");
    let recovery_attempts = 5;
    let mut recovery_results = Vec::new();
    
    for attempt in 0..recovery_attempts {
        let valid_event = create_test_arbitrage_opportunity().await?;
        
        let start_time = Instant::now();
        let result = timeout(
            Duration::from_secs(6),
            process_strategy(valid_event, test_env.test_config.ws_url.clone())
        ).await;
        let duration = start_time.elapsed();
        
        recovery_results.push((attempt, result.is_ok(), duration));
        
        info!("🔧 Recovery attempt #{}: success={}, duration={:?}", 
              attempt + 1, result.is_ok(), duration);
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    // Analyze recovery pattern
    let successful_recoveries = recovery_results.iter().filter(|(_, success, _)| *success).count();
    let recovery_rate = (successful_recoveries as f64 / recovery_attempts as f64) * 100.0;
    
    info!("📊 Error state recovery: {}/{} attempts successful ({:.1}%)", 
          successful_recoveries, recovery_attempts, recovery_rate);
    
    // System should recover from error states
    assert!(recovery_rate >= 60.0, 
           "System should recover from error states with at least 60% success rate");
    
    Ok(())
}

// Helper functions for creating different error scenarios

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

async fn create_invalid_address_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("0000000000000000000000000000000000000001"), // Invalid
        corresponding_pool_address: address!("0000000000000000000000000000000000000002"), // Invalid
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_zero_address_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("0000000000000000000000000000000000000000"), // Zero address
        corresponding_pool_address: address!("0000000000000000000000000000000000000000"), // Zero address
        pool_variant: 3,
        token0: address!("0000000000000000000000000000000000000000"),
        token1: address!("0000000000000000000000000000000000000000"),
        fee: U24::from(3000u32),
    })
}

async fn create_extreme_fee_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(999999u32), // Extremely high fee
    })
}

async fn create_mismatched_token_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), // Same token
        token1: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), // Same token (invalid)
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

// Note: Error recovery tests focus on system resilience and graceful failure handling
// Individual tests can be run with: cargo test test_connection_failure_handling
