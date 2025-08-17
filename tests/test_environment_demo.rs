#![allow(unused_imports)]

// Comprehensive Test Environment Demo
// Demonstrates the complete test infrastructure setup and usage

use anyhow::Result;
use log::{info, warn};
use std::time::Duration;

// Import our test infrastructure
mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mod.rs"));
}

use utils::integrated_test_env::{IntegratedTestEnvironment, TestEnvironmentConfig, quick_setup};
use fixtures::test_scenarios::{PredefinedScenarios, ScenarioType};

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
    
    let env = quick_setup::simple_test_env().await?;
    
    // Verify environment is working
    env.test_env.verify_connection().await?;
    
    // Get some basic info
    let block_info = env.test_env.get_latest_block_info().await?;
    block_info.pretty_print();
    
    // Verify deployed contracts
    info!("📜 Deployed {} test tokens and {} test pools", 
          env.deployed_tokens.len(), env.deployed_pools.len());
    
    info!("✅ Simple environment demo completed");
    Ok(())
}

/// Demo 2: Mainnet fork environment
async fn demo_mainnet_fork_environment() -> Result<()> {
    info!("\n🌐 Demo 2: Mainnet Fork Environment");
    
    // Fork from a recent block (you can specify a block number)
    let env = quick_setup::mainnet_fork_env(Some(18_500_000)).await?;
    
    // Test connection to forked mainnet
    env.test_env.verify_connection().await?;
    
    let block_info = env.test_env.get_latest_block_info().await?;
    info!("📦 Forked mainnet at block: {}", block_info.number);
    
    info!("✅ Mainnet fork demo completed");
    Ok(())
}

/// Demo 3: Mock WebSocket scenarios
async fn demo_mock_websocket_scenarios() -> Result<()> {
    info!("\n🎭 Demo 3: Mock WebSocket Scenarios");
    
    let env = quick_setup::mock_env_with_scenario(
        "normal_operation".to_string()
    ).await?;
    
    if let Some(ref mock_ws) = env.mock_ws {
        info!("🎬 Starting normal operation scenario...");
        mock_ws.start_scenario("normal_operation").await?;
        
        // Subscribe to events and demonstrate receiving them
        let mut event_receiver = mock_ws.subscribe();
        
        // Wait for a few events
        let mut event_count = 0;
        let timeout_duration = Duration::from_secs(5);
        let start_time = std::time::Instant::now();
        
        while event_count < 3 && start_time.elapsed() < timeout_duration {
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), event_receiver.recv()).await {
                match event {
                    Ok(mock_event) => {
                        info!("📨 Received mock event: {:?}", mock_event);
                        event_count += 1;
                    }
                    Err(_) => break,
                }
            }
        }
        
        // Test network instability scenario
        info!("🌪️  Testing network instability...");
        mock_ws.start_scenario("network_instability").await?;
        
        // Simulate connection errors
        mock_ws.simulate_connection_error(
            utils::mock_websocket::ConnectionErrorType::NetworkTimeout,
            "Simulated network timeout"
        )?;
    }
    
    info!("✅ Mock WebSocket demo completed");
    Ok(())
}

/// Demo 4: Predefined test scenarios
async fn demo_test_scenarios() -> Result<()> {
    info!("\n📋 Demo 4: Predefined Test Scenarios");
    
    // Load all predefined scenarios
    let all_scenarios = PredefinedScenarios::all_scenarios();
    info!("📊 Available scenarios: {}", all_scenarios.len());
    
    for scenario in &all_scenarios {
        info!("  📋 {} ({}): {}", 
              scenario.name, 
              format!("{:?}", scenario.scenario_type),
              scenario.description);
    }
    
    // Demonstrate filtering scenarios by type
    let profitable_scenarios = PredefinedScenarios::scenarios_by_type(ScenarioType::ProfitableArbitrage);
    info!("💰 Profitable arbitrage scenarios: {}", profitable_scenarios.len());
    
    let edge_case_scenarios = PredefinedScenarios::scenarios_by_type(ScenarioType::EdgeCase);
    info!("⚠️  Edge case scenarios: {}", edge_case_scenarios.len());
    
    // Demonstrate scenario serialization
    let sample_scenario = PredefinedScenarios::profitable_weth_usdc_arbitrage();
    let json = serde_json::to_string_pretty(&sample_scenario)?;
    info!("📄 Sample scenario JSON structure:\n{}", &json[0..300.min(json.len())]);
    
    info!("✅ Test scenarios demo completed");
    Ok(())
}

/// Demo 5: Complete arbitrage test cycle
async fn demo_complete_arbitrage_cycle() -> Result<()> {
    info!("\n🔄 Demo 5: Complete Arbitrage Test Cycle");
    
    // Set up environment with a profitable scenario
    let mut env = quick_setup::simple_test_env().await?;
    
    // Apply profitable arbitrage scenario
    let scenario = PredefinedScenarios::profitable_weth_usdc_arbitrage();
    info!("🎬 Applying scenario: {}", scenario.name);
    env.apply_scenario(scenario.clone()).await?;
    
    // Run complete test cycle
    info!("🚀 Running complete arbitrage test cycle...");
    let result = env.run_arbitrage_test_cycle().await?;
    
    // Print detailed results
    result.print_report();
    
    // Check if results meet expectations
    if result.meets_expectations(&scenario.expected_outcomes) {
        info!("✅ Test results meet expected outcomes!");
    } else {
        warn!("⚠️  Test results do not meet expected outcomes");
    }
    
    // Test with different scenarios
    info!("\n🧪 Testing with different scenario types...");
    
    let scenarios_to_test = vec![
        PredefinedScenarios::low_liquidity_scenario(),
        PredefinedScenarios::gas_price_spike(),
        PredefinedScenarios::mev_competition(),
    ];
    
    for test_scenario in scenarios_to_test {
        info!("🎯 Testing scenario: {}", test_scenario.name);
        env.apply_scenario(test_scenario.clone()).await?;
        
        let test_result = env.run_arbitrage_test_cycle().await?;
        
        info!("📊 Results: Detection={}, Simulation={}, Execution={}", 
              test_result.detection.opportunity_detected,
              test_result.simulation.is_some(),
              test_result.execution.is_some());
              
        if test_result.meets_expectations(&test_scenario.expected_outcomes) {
            info!("✅ {} passed expectations", test_scenario.name);
        } else {
            warn!("⚠️  {} did not meet expectations", test_scenario.name);
        }
    }
    
    info!("✅ Complete arbitrage cycle demo completed");
    Ok(())
}

/// Performance benchmarking demo
#[allow(dead_code)]
async fn demo_performance_benchmarks() -> Result<()> {
    info!("\n⚡ Demo: Performance Benchmarks");
    
    let env = quick_setup::simple_test_env().await?;
    
    // Benchmark environment setup time
    let setup_start = std::time::Instant::now();
    let _new_env = quick_setup::simple_test_env().await?;
    let setup_time = setup_start.elapsed();
    info!("🏗️  Environment setup time: {:?}", setup_time);
    
    // Benchmark detection cycles
    let detection_times: Vec<Duration> = futures::future::join_all(
        (0..10).map(|_| async {
            let start = std::time::Instant::now();
            let _result = env.run_arbitrage_test_cycle().await;
            start.elapsed()
        })
    ).await;
    
    let avg_detection_time = detection_times.iter().sum::<Duration>() / detection_times.len() as u32;
    let min_time = detection_times.iter().min().unwrap();
    let max_time = detection_times.iter().max().unwrap();
    
    info!("📊 Detection Performance:");
    info!("   Average: {:?}", avg_detection_time);
    info!("   Min: {:?}", min_time);
    info!("   Max: {:?}", max_time);
    
    Ok(())
}

/// Error handling demo
#[allow(dead_code)]
async fn demo_error_handling() -> Result<()> {
    info!("\n🚨 Demo: Error Handling and Recovery");
    
    // Test with invalid configuration
    info!("🧪 Testing invalid configuration...");
    let invalid_config = TestEnvironmentConfig {
        use_anvil: false,
        use_mainnet_fork: true, // This should fail - can't fork without Anvil
        deploy_test_contracts: true,
        ..Default::default()
    };
    
    match IntegratedTestEnvironment::new(invalid_config).await {
        Ok(_) => warn!("⚠️  Expected failure but got success"),
        Err(e) => info!("✅ Correctly caught error: {}", e),
    }
    
    // Test network failure scenarios
    info!("🌐 Testing network failure scenarios...");
    let env = quick_setup::mock_env_with_scenario("network_instability".to_string()).await?;
    
    if let Some(ref mock_ws) = env.mock_ws {
        // Simulate various connection failures
        mock_ws.simulate_connection_error(
            utils::mock_websocket::ConnectionErrorType::DnsFailure,
            "DNS resolution failed"
        )?;
        
        mock_ws.simulate_connection_error(
            utils::mock_websocket::ConnectionErrorType::ConnectionRefused,
            "Connection refused"
        )?;
        
        mock_ws.simulate_connection_error(
            utils::mock_websocket::ConnectionErrorType::UnexpectedDisconnect,
            "Unexpected disconnect"
        )?;
    }
    
    info!("✅ Error handling demo completed");
    Ok(())
}
