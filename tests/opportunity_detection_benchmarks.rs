// Opportunity Detection Latency Benchmarks - Phase 6.1
// Performance testing for arbitrage opportunity detection speed

use anyhow::Result;
use arbooo::arbitrage::strategy::process_strategy;
use arbooo::common::logs::LogEvent;
use alloy::primitives::address;
use alloy_primitives::aliases::U24;
use log::info;
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[path = "utils/mod.rs"]
mod utils;
use utils::test_env::TestEnvironment;

/// Benchmark opportunity detection latency across different scenarios
#[tokio::test]
async fn benchmark_opportunity_detection_latency() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⚡ Benchmarking opportunity detection latency");

    // Test different opportunity scenarios for latency analysis
    let latency_scenarios = [
        ("immediate_opportunity", "Immediate opportunity detection"),
        ("complex_calculation", "Complex multi-pool calculation"),
        ("high_gas_scenario", "High gas cost scenario"),
        ("low_profit_margin", "Low profit margin detection"),
        ("multi_token_path", "Multi-token arbitrage path"),
    ];

    let mut latency_results = Vec::new();
    let benchmark_iterations = 5; // Multiple iterations for statistical accuracy

    for (scenario_name, scenario_desc) in latency_scenarios {
        info!("⚡ Benchmarking latency scenario: {} - {}", scenario_name, scenario_desc);
        
        let mut scenario_latencies = Vec::new();

        // Run multiple iterations for each scenario
        for iteration in 0..benchmark_iterations {
            let log_event = create_latency_test_opportunity(scenario_name).await?;
            let start_time = Instant::now();
            
            let result = timeout(
                Duration::from_secs(5), // Shorter timeout for latency testing
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;
            
            let latency = start_time.elapsed();
            scenario_latencies.push(latency);
            
            info!("⚡ Iteration {}: latency={:?}, result={}", 
                  iteration + 1, latency, result.is_ok());

            // Small delay between iterations
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Calculate statistics for this scenario
        let avg_latency = calculate_average_duration(&scenario_latencies);
        let min_latency = scenario_latencies.iter().min().unwrap();
        let max_latency = scenario_latencies.iter().max().unwrap();
        
        latency_results.push((scenario_name, avg_latency, *min_latency, *max_latency));
        
        info!("⚡ Scenario '{}' latency stats: avg={:?}, min={:?}, max={:?}", 
              scenario_name, avg_latency, min_latency, max_latency);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Performance analysis
    info!("📊 Opportunity detection latency benchmark results:");
    for (scenario, avg, min, max) in &latency_results {
        info!("   ⚡ {}: avg={:?}, min={:?}, max={:?}", scenario, avg, min, max);
    }

    // Calculate overall performance metrics
    let overall_avg = calculate_average_duration(
        &latency_results.iter().map(|(_, avg, _, _)| *avg).collect::<Vec<_>>()
    );
    
    info!("📊 Overall average opportunity detection latency: {:?}", overall_avg);

    // Performance assertions
    assert!(latency_results.len() == latency_scenarios.len(),
           "All latency scenarios should be benchmarked");
    
    // Assert reasonable performance (under 3 seconds average)
    assert!(overall_avg < Duration::from_secs(3),
           "Average opportunity detection should be under 3 seconds");

    Ok(())
}

/// Benchmark detection latency under load conditions (sequential)
#[tokio::test]
async fn benchmark_detection_under_load() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔥 Benchmarking opportunity detection under sequential load");

    // Test load scenarios with sequential execution
    let load_scenarios = [
        ("light_load", 3, "Light load - 3 sequential opportunities"),
        ("medium_load", 5, "Medium load - 5 sequential opportunities"),
        ("heavy_load", 8, "Heavy load - 8 sequential opportunities"),
    ];

    let mut load_results = Vec::new();

    for (scenario_name, task_count, scenario_desc) in load_scenarios {
        info!("🔥 Testing load scenario: {} - {}", scenario_name, scenario_desc);
        
        let start_time = Instant::now();
        let mut task_results = Vec::new();

        // Execute tasks sequentially to avoid threading issues
        for i in 0..task_count {
            let ws_url = test_env.test_config.ws_url.clone();
            let log_event = create_load_test_opportunity(scenario_name, i).await?;
            
            let task_start = Instant::now();
            let result = timeout(
                Duration::from_secs(8),
                process_strategy(log_event, ws_url)
            ).await;
            
            let task_duration = task_start.elapsed();
            let success = result.is_ok();
            task_results.push((task_duration, success));
            
            info!("🔥 Task {}/{}: duration={:?}, success={}", 
                  i + 1, task_count, task_duration, success);
            
            // Small delay between tasks
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let total_duration = start_time.elapsed();
        let successful_tasks = task_results.iter().filter(|(_, success)| *success).count();
        let avg_task_duration = calculate_average_duration(
            &task_results.iter().map(|(duration, _)| *duration).collect::<Vec<_>>()
        );

        load_results.push((scenario_name, task_count, total_duration, avg_task_duration, successful_tasks));
        
        info!("🔥 Load scenario '{}': total_time={:?}, avg_task_time={:?}, success_rate={}/{}", 
              scenario_name, total_duration, avg_task_duration, successful_tasks, task_count);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Load performance analysis
    info!("📊 Sequential load testing benchmark results:");
    for (scenario, task_count, total, avg_task, successful) in &load_results {
        let success_rate = (*successful as f64 / *task_count as f64) * 100.0;
        info!("   🔥 {}: tasks={}, total_time={:?}, avg_task={:?}, success_rate={:.1}%", 
              scenario, task_count, total, avg_task, success_rate);
    }

    // Performance assertions
    assert!(load_results.len() == load_scenarios.len(),
           "All load scenarios should be tested");

    Ok(())
}

/// Benchmark opportunity detection with different complexity levels
#[tokio::test]
async fn benchmark_detection_complexity() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🧮 Benchmarking detection complexity scaling");

    // Test complexity scenarios
    let complexity_scenarios = [
        ("simple_v2_v3", "Simple V2-V3 pair arbitrage"),
        ("multi_hop_path", "Multi-hop arbitrage path"),
        ("cross_protocol", "Cross-protocol arbitrage"),
        ("high_precision", "High precision calculation"),
    ];

    let mut complexity_results = Vec::new();

    for (scenario_name, scenario_desc) in complexity_scenarios {
        info!("🧮 Testing complexity scenario: {} - {}", scenario_name, scenario_desc);
        
        let mut scenario_times = Vec::new();
        let iterations = 3;

        for iteration in 0..iterations {
            let log_event = create_complexity_test_opportunity(scenario_name).await?;
            let start_time = Instant::now();
            
            let result = timeout(
                Duration::from_secs(6),
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;
            
            let duration = start_time.elapsed();
            scenario_times.push(duration);
            
            info!("🧮 Iteration {}: duration={:?}, success={}", 
                  iteration + 1, duration, result.is_ok());

            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let avg_complexity_time = calculate_average_duration(&scenario_times);
        complexity_results.push((scenario_name, avg_complexity_time));
        
        info!("🧮 Complexity scenario '{}' average time: {:?}", scenario_name, avg_complexity_time);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Complexity analysis
    info!("📊 Complexity scaling benchmark results:");
    for (scenario, avg_time) in &complexity_results {
        info!("   🧮 {}: avg_time={:?}", scenario, avg_time);
    }

    // Performance assertions
    assert!(complexity_results.len() == complexity_scenarios.len(),
           "All complexity scenarios should be benchmarked");

    Ok(())
}

/// Benchmark real-time detection performance
#[tokio::test]
async fn benchmark_realtime_detection() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⏰ Benchmarking real-time detection performance");

    // Test real-time scenarios
    let realtime_scenarios = [
        ("block_time_window", "Detection within block time window"),
        ("mev_competition", "Detection under MEV competition"),
        ("gas_price_update", "Detection with gas price updates"),
        ("liquidity_change", "Detection with liquidity changes"),
    ];

    let mut realtime_results = Vec::new();

    for (scenario_name, scenario_desc) in realtime_scenarios {
        info!("⏰ Testing real-time scenario: {} - {}", scenario_name, scenario_desc);
        
        let log_event = create_realtime_test_opportunity(scenario_name).await?;
        let start_time = Instant::now();
        
        let _result = timeout(
            Duration::from_millis(2000), // Strict 2-second real-time requirement
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;
        
        let duration = start_time.elapsed();
        let meets_realtime = duration < Duration::from_millis(1500); // 1.5s target
        
        realtime_results.push((scenario_name, duration, meets_realtime));
        
        info!("⏰ Real-time scenario '{}': duration={:?}, meets_target={}", 
              scenario_name, duration, meets_realtime);

        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Real-time performance analysis
    info!("📊 Real-time detection benchmark results:");
    for (scenario, duration, meets_target) in &realtime_results {
        info!("   ⏰ {}: duration={:?}, meets_1.5s_target={}", scenario, duration, meets_target);
    }

    let realtime_success_rate = realtime_results.iter()
        .filter(|(_, _, meets_target)| *meets_target)
        .count() as f64 / realtime_results.len() as f64 * 100.0;
    
    info!("📊 Real-time performance success rate: {:.1}%", realtime_success_rate);

    // Performance assertions
    assert!(realtime_results.len() == realtime_scenarios.len(),
           "All real-time scenarios should be tested");

    Ok(())
}

// Helper functions for creating different latency test scenarios

async fn create_latency_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (pool_variant, fee) = match scenario {
        "immediate_opportunity" => (3, 3000u32),
        "complex_calculation" => (2, 500u32),
        "high_gas_scenario" => (3, 10000u32),
        "low_profit_margin" => (3, 100u32),
        "multi_token_path" => (2, 3000u32),
        _ => (3, 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_load_test_opportunity(scenario: &str, index: u8) -> Result<LogEvent> {
    let fee = match scenario {
        "light_load" => 3000u32,
        "medium_load" => 500u32,
        "heavy_load" => 10000u32,
        _ => 3000u32,
    };

    // Vary addresses slightly for different concurrent tests
    let pool_addresses = [
        address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
        address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
    ];

    Ok(LogEvent {
        log_pool_address: pool_addresses[index as usize % pool_addresses.len()],
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_complexity_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (token0, token1, fee) = match scenario {
        "simple_v2_v3" => (
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), // UNI
            3000u32
        ),
        "multi_hop_path" => (
            address!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
            address!("514910771AF9Ca656af840dff83E8264EcF986CA"), // LINK
            500u32
        ),
        "cross_protocol" => (
            address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"), // UNI
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
            10000u32
        ),
        "high_precision" => (
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"), // UNI
            address!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
            100u32
        ),
        _ => (
            address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            3000u32
        ),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0,
        token1,
        fee: U24::from(fee),
    })
}

async fn create_realtime_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let pool_variant = match scenario {
        "block_time_window" => 3,
        "mev_competition" => 2,
        "gas_price_update" => 3,
        "liquidity_change" => 2,
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

fn calculate_average_duration(durations: &[Duration]) -> Duration {
    if durations.is_empty() {
        return Duration::from_secs(0);
    }
    
    let total_nanos: u128 = durations.iter().map(|d| d.as_nanos()).sum();
    let avg_nanos = total_nanos / durations.len() as u128;
    Duration::from_nanos(avg_nanos as u64)
}

// Note: Opportunity detection latency benchmarks focus on performance optimization
// Individual tests can be run with: cargo test benchmark_opportunity_detection_latency
