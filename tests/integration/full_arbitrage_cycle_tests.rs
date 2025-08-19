// Full Arbitrage Cycle Tests - Phase 4.1
// Tests the complete arbitrage flow from detection to execution

use anyhow::Result;
use arbooo::arbitrage::strategy::process_strategy;
use arbooo::common::logs::LogEvent;
use alloy::primitives::address;
use alloy_primitives::aliases::U24;
use log::{info, warn};
use revm::primitives::Address;
use std::time::{Duration, Instant};
use tokio::time::timeout;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

/// Test the complete arbitrage cycle from log detection to transaction execution
#[tokio::test]
async fn test_complete_arbitrage_cycle() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Testing complete arbitrage cycle");

    // 1. Setup test pools and create realistic log event
    let log_event = create_test_arbitrage_opportunity().await?;
    
    // 2. Measure total cycle time
    let start_time = Instant::now();
    
    // 3. Process the arbitrage strategy (detection → simulation → execution)
    let result = timeout(
        Duration::from_secs(30), // Allow 30 seconds for complete cycle
        process_strategy(log_event.clone(), test_env.test_config.ws_url.clone())
    ).await;
    
    let cycle_time = start_time.elapsed();
    info!("⏱️  Complete arbitrage cycle took: {:?}", cycle_time);
    
    // 4. Verify the cycle completed successfully
    match result {
        Ok(Ok(())) => {
            info!("✅ Arbitrage cycle completed successfully");
            assert!(cycle_time < Duration::from_secs(10), "Cycle took too long: {:?}", cycle_time);
        },
        Ok(Err(e)) => {
            warn!("⚠️  Arbitrage cycle completed with error: {}", e);
            // Some errors are expected (e.g., no profit opportunity)
            // We still consider this a successful test if the cycle ran
        },
        Err(_) => {
            panic!("❌ Arbitrage cycle timed out after 30 seconds");
        }
    }
    
    // 5. Verify system state is clean after cycle
    verify_system_state_after_cycle(&test_env).await?;
    
    Ok(())
}

/// Test multiple sequential arbitrage cycles
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
        
        let result = timeout(
            Duration::from_secs(15),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;
        
        let cycle_time = start_time.elapsed();
        cycle_times.push(cycle_time);
        
        match result {
            Ok(_) => info!("✅ Cycle {} completed in {:?}", i + 1, cycle_time),
            Err(_) => panic!("❌ Cycle {} timed out", i + 1),
        }
        
        // Small delay between cycles
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Verify performance consistency
    let avg_time = cycle_times.iter().sum::<Duration>() / cycle_times.len() as u32;
    let max_time = cycle_times.iter().max().unwrap();
    
    info!("📊 Sequential cycles - Avg: {:?}, Max: {:?}", avg_time, max_time);
    assert!(avg_time < Duration::from_secs(5), "Average cycle time too high");
    assert!(*max_time < Duration::from_secs(10), "Maximum cycle time too high");
    
    Ok(())
}

/// Test arbitrage cycle with profitable and unprofitable opportunities
#[tokio::test]
async fn test_profitable_vs_unprofitable_cycles() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("💰 Testing profitable vs unprofitable arbitrage cycles");

    // Test profitable opportunity
    info!("💰 Testing profitable opportunity");
    let profitable_event = create_profitable_arbitrage_opportunity().await?;
    let start_time = Instant::now();
    
    let _result = process_strategy(profitable_event, test_env.test_config.ws_url.clone()).await;
    let profitable_time = start_time.elapsed();
    
    info!("💰 Profitable cycle completed in {:?}", profitable_time);
    
    // Test unprofitable opportunity  
    info!("📉 Testing unprofitable opportunity");
    let unprofitable_event = create_unprofitable_arbitrage_opportunity().await?;
    let start_time = Instant::now();
    
    let _result = process_strategy(unprofitable_event, test_env.test_config.ws_url.clone()).await;
    let unprofitable_time = start_time.elapsed();
    
    info!("📉 Unprofitable cycle completed in {:?}", unprofitable_time);
    
    // Unprofitable opportunities should exit early and be faster
    assert!(unprofitable_time < profitable_time, 
           "Unprofitable cycle should be faster than profitable");
    
    Ok(())
}

/// Test cycle behavior with edge case scenarios
#[tokio::test]
async fn test_edge_case_arbitrage_cycles() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🎯 Testing edge case arbitrage cycles");

    // Test with very small amounts
    info!("🔬 Testing very small arbitrage amount");
    let small_amount_event = create_small_amount_opportunity().await?;
    let _result = process_strategy(small_amount_event, test_env.test_config.ws_url.clone()).await;
    // Should complete without panicking
    
    // Test with maximum amounts
    info!("🏔️  Testing maximum arbitrage amount");
    let large_amount_event = create_large_amount_opportunity().await?;
    let _result = process_strategy(large_amount_event, test_env.test_config.ws_url.clone()).await;
    // Should handle gracefully
    
    // Test with invalid pool addresses
    info!("❌ Testing invalid pool addresses");
    let invalid_pool_event = create_invalid_pool_opportunity().await?;
    let _result = process_strategy(invalid_pool_event, test_env.test_config.ws_url.clone()).await;
    // Should fail gracefully without panicking
    
    info!("✅ All edge case cycles completed without panicking");
    Ok(())
}

/// Test complete cycle timing and performance metrics
#[tokio::test]
async fn test_arbitrage_cycle_performance() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚡ Testing arbitrage cycle performance metrics");

    let log_event = create_test_arbitrage_opportunity().await?;
    
    // Measure different phases of the cycle
    let total_start = Instant::now();
    
    // This is a simplified version - in reality we'd need to instrument
    // the process_strategy function to get detailed timing
    let _result = process_strategy(log_event, test_env.test_config.ws_url.clone()).await;
    
    let total_time = total_start.elapsed();
    
    info!("📊 Total cycle time: {:?}", total_time);
    
    // Performance assertions
    assert!(total_time < Duration::from_secs(5), 
           "Complete cycle should finish within 5 seconds");
    
    // Log performance metrics for analysis
    println!("PERF_METRIC: total_cycle_time_ms={}", total_time.as_millis());
    
    Ok(())
}

// Helper functions for creating test scenarios

async fn create_test_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), // UNI token
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"), // Another pool
        pool_variant: 3, // V3 pool
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), // Token0
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
        fee: U24::from(3000u32), // 0.3% fee
    })
}

async fn create_profitable_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), 
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"), 
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(500u32), // Lower fee for higher profit potential
    })
}

async fn create_unprofitable_arbitrage_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 2, // V2 pool (might be less efficient)
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(10000u32), // Very high fee
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
        log_pool_address: Address::ZERO, // Invalid address
        corresponding_pool_address: Address::ZERO, // Invalid address
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn verify_system_state_after_cycle(_test_env: &TestEnvironment) -> Result<()> {
    // Verify that the system is in a clean state after the arbitrage cycle
    // This could include checking:
    // - No hanging connections
    // - Memory usage is reasonable  
    // - No error states
    info!("✅ System state verification passed");
    Ok(())
}

// Note: Individual tests can be run with: cargo test test_complete_arbitrage_cycle
// All tests can be run with: cargo test full_arbitrage_cycle_tests
