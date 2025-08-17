// E2E Test Runner Binary
// This binary can be run independently to execute end-to-end tests
// Usage: cargo run --bin e2e_test_runner

use anyhow::Result;
use log::info;
use std::process;

mod e2e_test_runner {
    pub mod test_result;
    pub mod test_categories;
    pub mod individual_tests;
    pub mod test_environment;
    
    pub use test_result::{TestResult, TestResults};
    pub use test_environment::*;
}

use e2e_test_runner::{TestResults, setup_test_logger};
use e2e_test_runner::test_categories::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup our dedicated test logger
    setup_test_logger();
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
        "unit" => test_results.add(run_unit_tests().await),
        "performance" => test_results.add(run_performance_tests().await),
        "memory" => test_results.add(run_memory_tests().await),
        "environment" => test_results.add(run_environment_tests().await),
        "transaction" => test_results.add(run_transaction_tests().await),
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
            test_results.add(run_unit_tests().await);
            test_results.add(run_pool_tests().await);
            test_results.add(run_evm_tests().await);
            test_results.add(run_environment_tests().await);
            test_results.add(run_transaction_tests().await);
            test_results.add(run_performance_tests().await);
            test_results.add(run_memory_tests().await);
            test_results.add(run_integration_tests().await);
            test_results.add(run_comprehensive_flow_tests().await);
            test_results.add(run_edge_case_tests().await);
        }
        _ => {
            eprintln!("❌ Unknown test: {}", test_name);
            eprintln!("Available tests: provider, atomic, unit, pool, evm, environment, transaction, performance, memory, component, integration, full-flow, edge-cases, gas-estimation, stress, all");
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
