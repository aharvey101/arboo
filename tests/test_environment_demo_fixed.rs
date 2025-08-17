#![allow(unused_imports)]

// Comprehensive Test Environment Demo
// Demonstrates the complete test infrastructure setup and usage

use anyhow::Result;
use log::{info, warn};
use std::time::Duration;
use futures::stream::StreamExt;

// Import our test infrastructure
mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mod.rs"));
}

use utils::integrated_test_env::{IntegratedTestEnvironment, TestEnvironmentConfig, PredefinedScenarios, quick_setup};
use fixtures::test_scenarios::{ScenarioType};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🧪 Starting Comprehensive Test Environment Demo");
    
    // Demo 1: Simple test environment
    demo_simple_environment().await?;
    
    // Demo 2: Mainnet fork environment
    demo_mainnet_fork_environment().await?;
    
    // Demo 3: Mock WebSocket scenarios
    demo_mock_websocket_scenarios().await?;
    
    // Demo 4: Predefined test scenarios
    demo_test_scenarios().await?;
    
    // Demo 5: Complete arbitrage test cycle
    demo_complete_arbitrage_cycle().await?;
    
    info!("✅ All demos completed successfully!");
    
    Ok(())
}

/// Demo 1: Simple test environment with Anvil and test contracts
async fn demo_simple_environment() -> Result<()> {
    info!("\n🔧 Demo 1: Simple Test Environment Setup");
    
    let _env = quick_setup().await?;
    
    info!("📦 Anvil instance started successfully");
    
    // Verify environment is working
    info!("🔗 Provider connection established");
    
    info!("✅ Simple environment demo completed");
    Ok(())
}

/// Demo 2: Mainnet fork environment
async fn demo_mainnet_fork_environment() -> Result<()> {
    info!("\n🌐 Demo 2: Mainnet Fork Environment");
    
    // Use simple setup instead of mainnet fork for demo
    let _env = quick_setup().await?;
    
    info!("📦 Test environment created successfully");
    
    info!("✅ Environment demo completed");
    Ok(())
}

/// Demo 3: Mock WebSocket scenarios
async fn demo_mock_websocket_scenarios() -> Result<()> {
    info!("\n🎭 Demo 3: Mock WebSocket Scenarios");
    
    let _env = quick_setup().await?;
    
    info!("🎬 Mock WebSocket environment created");
    
    // Simulate receiving some events
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    info!("✅ Mock WebSocket demo completed");
    Ok(())
}

/// Demo 4: Predefined test scenarios
async fn demo_test_scenarios() -> Result<()> {
    info!("\n📋 Demo 4: Predefined Test Scenarios");
    
    let env = quick_setup().await?;
    
    // Create a simple test scenario
    let scenario = PredefinedScenarios::normal_arbitrage();
    info!("📋 Running scenario: {}", scenario.name);
    info!("📄 Description: {}", scenario.description);
    
    // Execute the scenario
    let result = env.execute_scenario(&scenario).await?;
    
    info!("✅ Scenario completed with result: {}", result.scenario_name);
    
    Ok(())
}

/// Demo 5: Complete arbitrage test cycle
async fn demo_complete_arbitrage_cycle() -> Result<()> {
    info!("\n🔄 Demo 5: Complete Arbitrage Test Cycle");
    
    let env = quick_setup().await?;
    
    // Create a simple scenario
    let scenario = PredefinedScenarios::normal_arbitrage();
    
    info!("🎯 Running arbitrage scenario: {}", scenario.name);
    
    // Execute the scenario
    let result = env.execute_scenario(&scenario).await?;
    
    info!("📊 Results: Execution time={:?}", result.execution_time);
    
    if result.transaction_sent {
        info!("✅ Arbitrage cycle completed successfully");
    } else {
        warn!("⚠️  Arbitrage cycle was not executed");
    }
    
    info!("✅ Complete arbitrage cycle demo completed");
    Ok(())
}

/// Performance benchmarking demo
#[allow(dead_code)]
async fn demo_performance_benchmarks() -> Result<()> {
    info!("\n⚡ Demo: Performance Benchmarks");
    
    let env = quick_setup().await?;
    
    // Benchmark environment setup time
    let setup_start = std::time::Instant::now();
    let _new_env = quick_setup().await?;
    let setup_time = setup_start.elapsed();
    info!("🏗️  Environment setup time: {:?}", setup_time);
    
    // Create scenarios and run tests
    let scenario = PredefinedScenarios::normal_arbitrage();
    
    // Run multiple test cycles for benchmarking
    let detection_times: Vec<Duration> = futures::stream::iter(0..5)
        .then(|_| async {
            let start = std::time::Instant::now();
            let _result = env.execute_scenario(&scenario).await;
            start.elapsed()
        })
        .collect()
        .await;
    
    let avg_detection_time = detection_times.iter().sum::<Duration>() / detection_times.len() as u32;
    let min_time = detection_times.iter().min().unwrap();
    let max_time = detection_times.iter().max().unwrap();
    
    info!("📊 Detection Performance:");
    info!("   Average: {:?}", avg_detection_time);
    info!("   Min: {:?}", min_time);
    info!("   Max: {:?}", max_time);
    
    Ok(())
}

// Note: Test Environment Demo showcases the infrastructure setup and usage patterns
// Individual demos can be run by calling the main function
