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
    pub mod reporter;
    
    pub use test_result::{TestResult, TestResults};
    pub use test_environment::*;
    pub use reporter::*;
}

use e2e_test_runner::{TestResults, setup_test_logger, Reporter};
use e2e_test_runner::test_categories::*;

fn print_help() {
    println!("E2E Test Runner");
    println!("Usage: cargo run --bin e2e_test_runner [OPTIONS] [TEST_CATEGORY]");
    println!();
    println!("Test Categories:");
    println!("  provider      - Provider connection tests");
    println!("  atomic        - Atomic/basic functionality tests");
    println!("  unit          - Unit tests");
    println!("  pool          - Pool data tests");
    println!("  evm           - EVM simulator tests");
    println!("  environment   - Environment configuration tests");
    println!("  transaction   - Transaction tests");
    println!("  performance   - Performance benchmarks");
    println!("  memory        - Memory usage tests");
    println!("  component     - Component integration tests");
    println!("  integration   - Integration tests");
    println!("  full-flow     - Comprehensive flow tests");
    println!("  edge-cases    - Edge case and stress tests");
    println!("  stress        - Stress tests");
    println!("  all           - All tests (default)");
    println!();
    println!("Options:");
    println!("  -v, --verbose - Show detailed test output and logs");
    println!("  -q, --quiet   - Show only test summaries (default)");
    println!("  -h, --help    - Show this help message");
    println!();
    println!("Examples:");
    println!("  cargo run --bin e2e_test_runner unit");
    println!("  cargo run --bin e2e_test_runner --verbose performance");
    println!("  cargo run --bin e2e_test_runner -v all");
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup our dedicated test logger
    setup_test_logger();
    info!("🧪 Starting E2E Test Runner");

    // Create a Jest-style reporter for the overall test run
    let overall_reporter = Reporter::new();
    overall_reporter.start_suite("E2E Test Runner - All Test Suites");

    // Parse command line arguments for specific test selection and verbosity
    let args: Vec<String> = std::env::args().collect();
    let mut test_name = "all";
    let mut verbose = false;
    
    // Parse arguments - more flexible approach
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--verbose" | "-v" => {
                verbose = true;
            }
            "--quiet" | "-q" => {
                verbose = false;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                // If it's not a flag, treat it as test_name
                if !arg.starts_with('-') {
                    test_name = arg;
                }
            }
        }
    }
    
    if verbose {
        info!("🔍 Running in verbose mode - showing detailed test output");
    } else {
        info!("📝 Running in quiet mode - showing test summaries only");
        info!("💡 Use --verbose or -v flag to see detailed test output");
    }
    
    // Set global verbosity flag
    e2e_test_runner::individual_tests::set_verbose_mode(verbose);

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
            eprintln!("Available tests: provider, atomic, unit, pool, evm, environment, transaction, performance, memory, component, integration, full-flow, edge-cases, stress, all");
            process::exit(1);
        }
    }

    // Print test summary with Jest-style reporting
    overall_reporter.should("E2E Test Runner - All Test Suites", "execute all selected test categories")
        .assert(|| {
            if test_results.has_failures() {
                Err(anyhow::anyhow!("Some test categories failed"))
            } else {
                Ok(())
            }
        })?;
    
    overall_reporter.end_suite("E2E Test Runner - All Test Suites");
    
    // Print traditional summary as well
    test_results.print_summary();

    if test_results.has_failures() {
        process::exit(1);
    }

    Ok(())
}
