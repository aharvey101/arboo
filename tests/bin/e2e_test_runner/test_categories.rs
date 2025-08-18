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
    run_test_category("pool").await
}

pub async fn run_evm_tests() -> Result<()> {
    run_test_category("evm").await
}

pub async fn run_unit_tests() -> Result<()> {
    run_test_category("unit").await
}

pub async fn run_performance_tests() -> Result<()> {
    run_test_category("performance").await
}

pub async fn run_memory_tests() -> Result<()> {
    run_test_category("memory").await
}

pub async fn run_environment_tests() -> Result<()> {
    run_test_category("environment").await
}

pub async fn run_transaction_tests() -> Result<()> {
    // Transaction tests are spread across unit tests
    let transaction_filters = &[
        "transaction_creation",
        "transaction_execution",
        "single_swap_simulation",
    ];
    
    for filter in transaction_filters {
        run_test_by_filter(filter).await?;
    }
    Ok(())
}

pub async fn run_integration_tests() -> Result<()> {
    run_test_category("integration").await
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
    run_test_category("edge_cases").await
}
