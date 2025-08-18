use super::individual_tests::*;
use super::reporter::Reporter;
use anyhow::Result;

pub async fn run_provider_connection_test() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Provider Connection Test");
    
    let result = reporter.should("Provider Connection Test", "establish provider connection and validate environment")
        .assert_async(|| async {
            test_integrated_environment().await
        }).await;
        
    reporter.end_suite("Provider Connection Test");
    result
}

pub async fn run_atomic_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Atomic Tests Suite");
    
    // Run the most basic test - integrated test environment setup
    let result = reporter.should("Atomic Tests Suite", "run integrated test environment setup")
        .assert_async(|| async {
            test_integrated_environment().await
        }).await;
        
    reporter.end_suite("Atomic Tests Suite");
    result
}

pub async fn run_pool_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Pool Data Tests Suite");
    
    // Test 1: Pool State Loading
    reporter.should("Pool Data Tests Suite", "load pool state")
        .assert_async(|| async {
            run_pool_state_loading_test().await
        }).await?;
    
    // Test 2: Pool Data File Tests
    reporter.should("Pool Data Tests Suite", "run pool data file tests")
        .assert_async(|| async {
            run_pool_data_file_tests().await
        }).await?;
        
    // Test 3: Pool Pairing File Tests
    reporter.should("Pool Data Tests Suite", "run pool pairing file tests")
        .assert_async(|| async {
            run_pool_pairing_file_tests().await
        }).await?;
        
    // Test 4: Pool Strategy Integration
    reporter.should("Pool Data Tests Suite", "run pool strategy integration test")
        .assert_async(|| async {
            run_pool_strategy_integration_test().await
        }).await?;
        
    // Test 5: EVM Pool State Integration
    reporter.should("Pool Data Tests Suite", "run EVM pool state integration test")
        .assert_async(|| async {
            run_evm_pool_state_integration_test().await
        }).await?;
    
    reporter.end_suite("Pool Data Tests Suite");
    Ok(())
}

pub async fn run_evm_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("EVM Simulator Tests Suite");
    
    // Test 1: EVM Simulator Initialization
    reporter.should("EVM Simulator Tests Suite", "initialize EVM simulator")
        .assert_async(|| async {
            run_evm_initialization_test().await
        }).await?;
    
    // Test 2: Transaction Execution
    reporter.should("EVM Simulator Tests Suite", "execute transactions in EVM")
        .assert_async(|| async {
            run_transaction_execution_test().await
        }).await?;
    
    // Test 3: Contract Deployment and Interaction  
    reporter.should("EVM Simulator Tests Suite", "deploy and interact with contracts")
        .assert_async(|| async {
            run_contract_deployment_test().await
        }).await?;
    
    // Test 4: Account Balance Management
    reporter.should("EVM Simulator Tests Suite", "manage account balances")
        .assert_async(|| async {
            run_balance_management_test().await
        }).await?;

    reporter.end_suite("EVM Simulator Tests Suite");
    Ok(())
}

pub async fn run_unit_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Unit Tests Suite");
    
    // Run cargo test based unit tests
    reporter.should("Unit Tests Suite", "run arbitrage calculation tests")
        .assert_async(|| async {
            run_cargo_test_with_output("arbitrage_calculation").await
        }).await?;
    
    reporter.should("Unit Tests Suite", "run transaction creation tests")
        .assert_async(|| async {
            run_cargo_test_with_output("transaction_creation").await
        }).await?;
    
    reporter.should("Unit Tests Suite", "run error recovery tests")
        .assert_async(|| async {
            run_cargo_test_with_output("error_recovery").await
        }).await?;
        
    // Additional unit test functions
    reporter.should("Unit Tests Suite", "run profit calculation tests")
        .assert_async(|| async {
            run_profit_calculation_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run transaction execution tests")
        .assert_async(|| async {
            run_transaction_execution_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run profit simulation tests")
        .assert_async(|| async {
            run_profit_simulation_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run atomic component tests")
        .assert_async(|| async {
            run_atomic_component_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run environment loading test")
        .assert_async(|| async {
            run_environment_loading_test().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run log event processing tests")
        .assert_async(|| async {
            run_log_event_processing_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run fork check tests")
        .assert_async(|| async {
            run_fork_check_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run single swap simulation tests")
        .assert_async(|| async {
            run_single_swap_simulation_tests().await
        }).await?;
        
    reporter.should("Unit Tests Suite", "run gas estimation validation test")
        .assert_async(|| async {
            run_gas_estimation_validation_test().await
        }).await?;

    reporter.end_suite("Unit Tests Suite");
    Ok(())
}

pub async fn run_performance_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Performance Tests Suite");
    
    // Run performance test functions
    reporter.should("Performance Tests Suite", "run high frequency tests")
        .assert_async(|| async {
            run_high_frequency_test().await
        }).await?;
    
    reporter.should("Performance Tests Suite", "run MEV competition tests")
        .assert_async(|| async {
            run_mev_competition_test().await
        }).await?;
        
    // Performance benchmarks
    reporter.should("Performance Tests Suite", "run opportunity detection benchmarks")
        .assert_async(|| async {
            run_opportunity_detection_benchmarks().await
        }).await?;
        
    reporter.should("Performance Tests Suite", "run simulation execution benchmarks")
        .assert_async(|| async {
            run_simulation_execution_benchmarks().await
        }).await?;
        
    reporter.should("Performance Tests Suite", "run transaction success rate metrics")
        .assert_async(|| async {
            run_transaction_success_rate_metrics().await
        }).await?;

    reporter.end_suite("Performance Tests Suite");
    Ok(())
}

pub async fn run_memory_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Memory Usage Tests");
    
    let result = reporter.should("Memory Usage Tests", "profile memory usage under different conditions")
        .assert_async(|| async {
            run_memory_usage_profiling().await
        }).await;
        
    reporter.end_suite("Memory Usage Tests");
    result
}

pub async fn run_environment_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Environment Tests");
    
    // Run environment tests  
    reporter.should("Environment Tests", "run block environment test")
        .assert_async(|| async {
            run_block_environment_test().await
        }).await?;
        
    reporter.should("Environment Tests", "run test environment demo")
        .assert_async(|| async {
            run_test_environment_demo().await
        }).await?;
    
    reporter.end_suite("Environment Tests");
    Ok(())
}

pub async fn run_transaction_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Transaction Tests");
    
    // Run transaction execution test
    reporter.should("Transaction Tests", "execute transaction tests")
        .assert_async(|| async {
            run_transaction_execution_test().await
        }).await?;
        
    reporter.should("Transaction Tests", "run transaction creation tests")
        .assert_async(|| async {
            run_transaction_creation_tests().await
        }).await?;
    
    reporter.end_suite("Transaction Tests");
    Ok(())
}

pub async fn run_integration_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Integration Tests");
    
    // Run integration tests
    reporter.should("Integration Tests", "run full arbitrage cycle test")
        .assert_async(|| async {
            run_full_arbitrage_cycle_test().await
        }).await?;
        
    reporter.should("Integration Tests", "run concurrent opportunities test")
        .assert_async(|| async {
            run_concurrent_opportunities_test().await
        }).await?;
        
    reporter.should("Integration Tests", "run network disconnection test")
        .assert_async(|| async {
            run_network_disconnection_test().await
        }).await?;
        
    // More integration tests
    reporter.should("Integration Tests", "run E2E arbitrage pipeline test")
        .assert_async(|| async {
            run_e2e_arbitrage_pipeline_test().await
        }).await?;
        
    reporter.should("Integration Tests", "run provider pipeline integration test")
        .assert_async(|| async {
            run_provider_pipeline_integration_test().await
        }).await?;
        
    reporter.should("Integration Tests", "run strategy processing integration test")
        .assert_async(|| async {
            run_strategy_processing_integration_test().await
        }).await?;
        
    reporter.should("Integration Tests", "run multi component integration test")
        .assert_async(|| async {
            run_multi_component_integration_test().await
        }).await?;
    
    reporter.end_suite("Integration Tests");
    Ok(())
}

pub async fn run_comprehensive_flow_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Comprehensive Flow Tests");
    
    // Run all major test categories for full coverage
    run_pool_tests().await?;
    run_evm_tests().await?;
    run_unit_tests().await?;
    run_performance_tests().await?;
    run_integration_tests().await?;
    
    reporter.end_suite("Comprehensive Flow Tests");
    Ok(())
}

pub async fn run_edge_case_tests() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Edge Case Tests");
    
    // Run edge case scenarios
    reporter.should("Edge Case Tests", "run insufficient liquidity test")
        .assert_async(|| async {
            run_insufficient_liquidity_test().await
        }).await?;
        
    reporter.should("Edge Case Tests", "run gas price spike test")
        .assert_async(|| async {
            run_gas_price_spike_test().await
        }).await?;
        
    reporter.should("Edge Case Tests", "run block reorganization test")
        .assert_async(|| async {
            run_block_reorganization_test().await
        }).await?;
        
    reporter.should("Edge Case Tests", "run error recovery test")
        .assert_async(|| async {
            run_error_recovery_test().await
        }).await?;
    
    reporter.end_suite("Edge Case Tests");
    Ok(())
}
