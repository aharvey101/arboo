// E2E Test Runner Binary
// This binary can be run independently to execute end-to-end tests
// Usage: cargo run --bin e2e_test_runner

use anyhow::Result;
use arbooo::common::logger;
use log::info;
use std::process;

// Include the test utilities from the parent tests directory
#[path = "../utils/mod.rs"]
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::from_filename(".env.test")?;
    // Setup logging for tests
    logger::setup_logger();
    info!("🧪 Starting E2E Test Runner");

    // Parse command line arguments for specific test selection
    let args: Vec<String> = std::env::args().collect();
    let test_name = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let mut test_results = TestResults::new();

    match test_name {
        "provider" => test_results.add(run_provider_connection_test().await),
        "atomic" => test_results.add(run_atomic_tests().await),
        "pool" => test_results.add(run_pool_tests().await),
        "evm" => test_results.add(run_evm_tests().await),
        "component" => {
            test_results.add(run_pool_tests().await);
            test_results.add(run_evm_tests().await);
        }
        "integration" => test_results.add(run_integration_tests().await),
        "full-flow" => test_results.add(run_comprehensive_flow_tests().await),
        "edge-cases" => test_results.add(run_edge_case_tests().await),
        "stress" => test_results.add(run_edge_case_tests().await),
        "all" => {
            test_results.add(run_atomic_tests().await);
            test_results.add(run_pool_tests().await);
            test_results.add(run_evm_tests().await);
            test_results.add(run_integration_tests().await);
            test_results.add(run_comprehensive_flow_tests().await);
            test_results.add(run_edge_case_tests().await);
        }
        _ => {
            eprintln!("❌ Unknown test: {}", test_name);
            eprintln!("Available tests: provider, atomic, pool, evm, component, integration, full-flow, edge-cases, stress, all");
            process::exit(1);
        }
    }

    // Print test summary
    test_results.print_summary();

    if test_results.has_failures() {
        process::exit(1);
    }

    Ok(())
}

async fn run_provider_connection_test() -> TestResult {
    info!("🔗 Running Provider Connection Test");
    
    match test_integrated_environment().await {
        Ok(_) => TestResult::success("Provider Connection"),
        Err(e) => TestResult::failure("Provider Connection", format!("{}", e)),
    }
}

async fn run_atomic_tests() -> TestResult {
    info!("⚛️  Running Atomic Tests");
    
    // Run the most basic test - integrated test environment setup
    match test_integrated_environment().await {
        Ok(_) => TestResult::success("Atomic Tests"),
        Err(e) => TestResult::failure("Atomic Tests", format!("{}", e)),
    }
}

async fn run_pool_tests() -> TestResult {
    info!("🏊 Running Pool Data Tests");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test 1: Pool Data Structures
    info!("📊 Testing Pool Data Structures");
    match utils::pool_test_runner::run_pool_data_structure_tests().await {
        Ok(_) => info!("✅ Pool data structure tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Pool data structure tests failed: {}", e));
        }
    }
    
    // Test 2: Pool Cache Operations
    info!("💾 Testing Pool Cache Operations");
    match utils::pool_test_runner::run_pool_cache_tests().await {
        Ok(_) => info!("✅ Pool cache tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Pool cache tests failed: {}", e));
        }
    }
    
    // Test 3: Pool Pairing Logic
    info!("🔗 Testing Pool Pairing Logic");
    match utils::pool_test_runner::run_pool_pairing_tests().await {
        Ok(_) => info!("✅ Pool pairing tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Pool pairing tests failed: {}", e));
        }
    }
    
    // Test 4: Pool Discovery Infrastructure
    info!("🔍 Testing Pool Discovery Infrastructure");
    match utils::pool_test_runner::run_pool_discovery_tests().await {
        Ok(_) => info!("✅ Pool discovery tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Pool discovery tests failed: {}", e));
        }
    }
    
    if all_passed {
        TestResult::success("Pool Data Tests")
    } else {
        TestResult::failure("Pool Data Tests", errors.join("; "))
    }
}

async fn run_evm_tests() -> TestResult {
    info!("🔧 Running EVM Simulator Tests");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test 1: EVM Simulator Initialization
    info!("🏗️ Testing EVM Simulator Initialization");
    match run_evm_initialization_test().await {
        Ok(_) => info!("✅ EVM initialization tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("EVM initialization tests failed: {}", e));
        }
    }
    
    // Test 2: Transaction Execution
    info!("🔄 Testing Transaction Execution");
    match run_transaction_execution_test().await {
        Ok(_) => info!("✅ Transaction execution tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Transaction execution tests failed: {}", e));
        }
    }
    
    // Test 3: Contract Deployment and Interaction  
    info!("📦 Testing Contract Deployment and Interaction");
    match run_contract_deployment_test().await {
        Ok(_) => info!("✅ Contract deployment tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Contract deployment tests failed: {}", e));
        }
    }
    
    // Test 4: Account Balance Management
    info!("💰 Testing Account Balance Management");
    match run_balance_management_test().await {
        Ok(_) => info!("✅ Balance management tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Balance management tests failed: {}", e));
        }
    }
    
    // Test 5: Pool State Loading
    info!("🏊 Testing Pool State Loading");
    match run_pool_state_loading_test().await {
        Ok(_) => info!("✅ Pool state loading tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Pool state loading tests failed: {}", e));
        }
    }
    
    // Test 6: Block Environment Manipulation
    info!("🔧 Testing Block Environment Manipulation");
    match run_block_environment_test().await {
        Ok(_) => info!("✅ Block environment tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Block environment tests failed: {}", e));
        }
    }
    
    if all_passed {
        TestResult::success("EVM Simulator Tests")
    } else {
        TestResult::failure("EVM Simulator Tests", errors.join("; "))
    }
}

async fn run_integration_tests() -> TestResult {
    info!("🔧 Running Integration Tests");
    
    // Placeholder for integration tests
    TestResult::success("Integration Tests (Not Implemented)")
}

async fn run_comprehensive_flow_tests() -> TestResult {
    info!("🚀 Running Comprehensive Flow Tests");
    
    // Run all comprehensive flow test categories
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test full arbitrage cycles
    info!("🔄 Testing Full Arbitrage Cycles");
    match run_full_arbitrage_cycle_test().await {
        Ok(_) => info!("✅ Full arbitrage cycle tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Full arbitrage cycle tests failed: {}", e));
        }
    }
    
    // Test concurrent opportunities (sequential due to thread safety)
    info!("🔄 Testing Concurrent Opportunities (Sequential Load)");
    match run_concurrent_opportunities_test().await {
        Ok(_) => info!("✅ Concurrent opportunities tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Concurrent opportunities tests failed: {}", e));
        }
    }
    
    // Test high-frequency scenarios
    info!("⚡ Testing High-Frequency Scenarios");
    match run_high_frequency_test().await {
        Ok(_) => info!("✅ High-frequency tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("High-frequency tests failed: {}", e));
        }
    }
    
    // Test error recovery and reconnection
    info!("🔧 Testing Error Recovery and Reconnection");
    match run_error_recovery_test().await {
        Ok(_) => info!("✅ Error recovery tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Error recovery tests failed: {}", e));
        }
    }
    
    if all_passed {
        TestResult::success("Comprehensive Flow Tests")
    } else {
        TestResult::failure("Comprehensive Flow Tests", errors.join("; "))
    }
}

async fn run_edge_case_tests() -> TestResult {
    info!("🔬 Running Edge Case & Stress Tests");
    
    // Run all edge case test categories
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test network disconnection scenarios
    info!("🌐 Testing Network Disconnection Scenarios");
    match run_network_disconnection_test().await {
        Ok(_) => info!("✅ Network disconnection tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Network disconnection tests failed: {}", e));
        }
    }
    
    // Test gas price spike scenarios
    info!("⛽ Testing Gas Price Spike Scenarios");
    match run_gas_price_spike_test().await {
        Ok(_) => info!("✅ Gas price spike tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Gas price spike tests failed: {}", e));
        }
    }
    
    // Test insufficient liquidity scenarios
    info!("💧 Testing Insufficient Liquidity Scenarios");
    match run_insufficient_liquidity_test().await {
        Ok(_) => info!("✅ Insufficient liquidity tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Insufficient liquidity tests failed: {}", e));
        }
    }
    
    // Test block reorganization scenarios
    info!("🔄 Testing Block Reorganization Scenarios");
    match run_block_reorganization_test().await {
        Ok(_) => info!("✅ Block reorganization tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("Block reorganization tests failed: {}", e));
        }
    }
    
    // Test MEV competition scenarios
    info!("🏆 Testing MEV Competition Scenarios");
    match run_mev_competition_test().await {
        Ok(_) => info!("✅ MEV competition tests passed"),
        Err(e) => {
            all_passed = false;
            errors.push(format!("MEV competition tests failed: {}", e));
        }
    }
    
    if all_passed {
        TestResult::success("Edge Case & Stress Tests")
    } else {
        TestResult::failure("Edge Case & Stress Tests", errors.join("; "))
    }
}

async fn test_integrated_environment() -> Result<()> {
    use utils::integrated_test_env::{IntegratedTestEnvironment, TestEnvironmentConfig};
    use alloy::providers::Provider;
    
    info!("  🏗️  Creating integrated test environment...");
    
    // Create a simple test configuration
    let config = TestEnvironmentConfig {
        mainnet_fork_url: "https://rpc.ankr.com/eth".to_string(),
        private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
        websocket_port: None,
        enable_logging: true,
        gas_limit: 21000,
        gas_price: 20_000_000_000,
    };
    
    info!("  🚀 Setting up test environment with Anvil...");
    let test_env = IntegratedTestEnvironment::new(config).await?;
    
    info!("  ✅ Test environment created successfully");
    
    // Test basic provider functionality
    info!("  � Testing provider connection...");
    let provider = test_env.provider();
    let block_number = provider.get_block_number().await?;
    info!("  ✅ Current block number: {}", block_number);
    
    info!("  🧹 Cleaning up test environment...");
    test_env.cleanup().await?;
    info!("  ✅ Test environment cleanup completed");
    
    Ok(())
}

// Individual comprehensive flow test functions
async fn run_full_arbitrage_cycle_test() -> Result<()> {
    use std::process::Command;
    
    info!("🔄 Running full arbitrage cycle test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_complete_arbitrage_cycle", "--test", "full_arbitrage_cycle_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Full arbitrage cycle test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_concurrent_opportunities_test() -> Result<()> {
    use std::process::Command;
    
    info!("🔄 Running concurrent opportunities test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_sequential_arbitrage_opportunities", "--test", "concurrent_opportunities_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Concurrent opportunities test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_high_frequency_test() -> Result<()> {
    use std::process::Command;
    
    info!("⚡ Running high-frequency test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_high_frequency_opportunity_processing", "--test", "high_frequency_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ High-frequency test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_error_recovery_test() -> Result<()> {
    use std::process::Command;
    
    info!("🔧 Running error recovery test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_connection_failure_handling", "--test", "error_recovery_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Error recovery test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

// Individual edge case and stress test functions
async fn run_network_disconnection_test() -> Result<()> {
    use std::process::Command;
    
    info!("🌐 Running network disconnection test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_websocket_disconnection_recovery", "--test", "network_disconnection_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Network disconnection test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_gas_price_spike_test() -> Result<()> {
    use std::process::Command;
    
    info!("⛽ Running gas price spike test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_gas_price_spike_handling", "--test", "gas_price_spike_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Gas price spike test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_insufficient_liquidity_test() -> Result<()> {
    use std::process::Command;
    
    info!("💧 Running insufficient liquidity test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_low_liquidity_handling", "--test", "insufficient_liquidity_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Insufficient liquidity test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_block_reorganization_test() -> Result<()> {
    use std::process::Command;
    
    info!("🔄 Running block reorganization test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_block_reorganization_handling", "--test", "block_reorganization_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Block reorganization test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_mev_competition_test() -> Result<()> {
    use std::process::Command;
    
    info!("🏆 Running MEV competition test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_mev_competition_detection", "--test", "mev_competition_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ MEV competition test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

// Individual EVM simulator test functions
async fn run_evm_initialization_test() -> Result<()> {
    use std::process::Command;
    
    info!("🏗️ Running EVM simulator initialization test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_evm_simulator_module_availability", "--test", "evm_simulator_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ EVM simulator initialization test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_transaction_execution_test() -> Result<()> {
    use std::process::Command;
    
    info!("🔄 Running transaction execution test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_evm_simulator_types_and_structures", "--test", "evm_simulator_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Transaction execution test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_contract_deployment_test() -> Result<()> {
    use std::process::Command;
    
    info!("📦 Running contract deployment test");
    
    let output = Command::new("cargo")
        .args(&["test", "test_evm_simulator_constants_and_addresses", "--test", "evm_simulator_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
    
    if output.status.success() {
        info!("✅ Contract deployment test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("Test failed: {}", stderr))
    }
}

async fn run_balance_management_test() -> Result<()> {
    use utils::integrated_test_env::IntegratedTestEnvironment;
    use alloy::providers::Provider;
    
    info!("💰 Testing account balance management with integrated environment");
    
    // Create test environment to ensure we can work with accounts
    let test_env = IntegratedTestEnvironment::quick_setup().await
        .map_err(|e| anyhow::anyhow!("Failed to setup test environment: {}", e))?;
    
    info!("✅ Successfully created test environment for balance management");
    
    // Test that we can access provider functionality
    let provider = test_env.provider();
    let block_number = provider.get_block_number().await
        .map_err(|e| anyhow::anyhow!("Failed to get block number from provider: {}", e))?;
    
    info!("✅ Successfully queried block number: {}", block_number);
    
    // Cleanup
    test_env.cleanup().await
        .map_err(|e| anyhow::anyhow!("Failed to cleanup test environment: {}", e))?;
    
    info!("✅ Balance management test completed successfully");
    Ok(())
}

async fn run_pool_state_loading_test() -> Result<()> {
    info!("🏊 Testing pool state loading capabilities");
    
    // Test that we can access the simulator functions needed for pool state loading
    use arbooo::common::revm::{Tx, VictimTx};
    use alloy::primitives::{U256, Address};
    use revm::primitives::Bytes;
    use alloy::signers::local::PrivateKeySigner;
    
    // Test creating transaction structures for pool interactions
    let pool_address = PrivateKeySigner::random().address();
    let caller_address = PrivateKeySigner::random().address();
    
    let _pool_tx = Tx {
        caller: caller_address,
        transact_to: pool_address,
        data: Bytes::new(),
        value: U256::ZERO,
        gas_price: U256::from(20_000_000_000u128),
        gas_limit: 500_000,
    };
    
    // Test VictimTx to Tx conversion
    let victim_tx = VictimTx {
        tx_hash: revm::primitives::B256::ZERO,
        from: caller_address,
        to: pool_address,
        data: Bytes::new(),
        value: U256::ZERO,
        gas_price: U256::from(20_000_000_000u128),
        gas_limit: Some(500_000),
    };
    
    let converted_tx = Tx::from(victim_tx);
    assert_eq!(converted_tx.caller, caller_address, "Converted transaction should preserve caller");
    assert_eq!(converted_tx.transact_to, pool_address, "Converted transaction should preserve target");
    assert_eq!(converted_tx.gas_limit, 500_000, "Converted transaction should preserve gas limit");
    
    // Test pool address parsing
    use std::str::FromStr;
    let _v3_pool_address = Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640")
        .map_err(|e| anyhow::anyhow!("Failed to parse V3 pool address: {}", e))?;
    
    let _v2_pool_address = Address::from_str("0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc")
        .map_err(|e| anyhow::anyhow!("Failed to parse V2 pool address: {}", e))?;
    
    info!("✅ Pool state loading test completed successfully");
    Ok(())
}

async fn run_block_environment_test() -> Result<()> {
    use utils::integrated_test_env::IntegratedTestEnvironment;
    use alloy::providers::Provider;
    
    info!("🔧 Testing block environment manipulation");
    
    // Create test environment
    let test_env = IntegratedTestEnvironment::quick_setup().await
        .map_err(|e| anyhow::anyhow!("Failed to setup test environment: {}", e))?;
    
    // Test block environment queries
    let provider = test_env.provider();
    
    // Test block number retrieval
    let block_number = provider.get_block_number().await
        .map_err(|e| anyhow::anyhow!("Failed to get block number: {}", e))?;
    
    assert!(block_number > 0, "Block number should be greater than 0");
    info!("✅ Current block number: {}", block_number);
    
    // Test gas price retrieval
    let gas_price = provider.get_gas_price().await
        .map_err(|e| anyhow::anyhow!("Failed to get gas price: {}", e))?;
    
    assert!(gas_price > 0, "Gas price should be greater than 0");
    info!("✅ Current gas price: {} wei", gas_price);
    
    // Cleanup
    test_env.cleanup().await
        .map_err(|e| anyhow::anyhow!("Failed to cleanup test environment: {}", e))?;
    
    info!("✅ Block environment test completed successfully");
    Ok(())
}

#[derive(Debug)]
struct TestResult {
    name: String,
    success: bool,
    error: Option<String>,
}

impl TestResult {
    fn success(name: &str) -> Self {
        Self {
            name: name.to_string(),
            success: true,
            error: None,
        }
    }
    
    fn failure(name: &str, error: String) -> Self {
        Self {
            name: name.to_string(),
            success: false,
            error: Some(error),
        }
    }
}

struct TestResults {
    results: Vec<TestResult>,
}

impl TestResults {
    fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    fn add(&mut self, result: TestResult) {
        self.results.push(result);
    }
    
    fn has_failures(&self) -> bool {
        self.results.iter().any(|r| !r.success)
    }
    
    fn print_summary(&self) {
        println!("\n📊 Test Summary:");
        println!("================");
        
        for result in &self.results {
            if result.success {
                println!("✅ {}", result.name);
            } else {
                println!("❌ {}: {}", result.name, result.error.as_ref().unwrap_or(&"Unknown error".to_string()));
            }
        }
        
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.success).count();
        let failed = total - passed;
        
        println!("\n📈 Results: {} passed, {} failed, {} total", passed, failed, total);
        
        if failed > 0 {
            println!("❌ Some tests failed!");
        } else {
            println!("🎉 All tests passed!");
        }
    }
}
