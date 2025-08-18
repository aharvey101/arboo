// E2E Test Runner Binary
// This binary can be run independently to execute end-to-end tests
// Usage: cargo run --bin e2e_test_runner

use anyhow::Result;
use log::info;
use std::process;

mod e2e_test_runner {
    pub mod test_categories;
    pub mod individual_tests;
    pub mod test_environment;
    pub mod reporter;
    
    pub use test_environment::*;
    pub use reporter::*;
}

use e2e_test_runner::{setup_test_logger, Reporter};

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
    
    // Parse command line arguments first to check verbose mode
    let args: Vec<String> = std::env::args().collect();
    let mut verbose = false;
    
    // Quick scan for verbose flag
    for arg in args.iter().skip(1) {
        if arg == "--verbose" || arg == "-v" {
            verbose = true;
            break;
        }
    }
    
    if verbose {
        info!("🧪 Starting E2E Test Runner");
    }

    // Create a Jest-style reporter for the overall test run
    let overall_reporter = Reporter::new();
    overall_reporter.start_suite("E2E Test Runner - All Test Suites");

    // Parse command line arguments for specific test selection and verbosity
    let mut test_name = "all";
    verbose = false; // Reset and parse properly
    
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
        // Only show this reminder in quiet mode, but make it less prominent
        // info!("💡 Use --verbose or -v flag to see detailed test output");
    }
    
    // Set global verbosity flag
    e2e_test_runner::individual_tests::set_verbose_mode(verbose);

    match test_name {
        "provider" => e2e_test_runner::test_categories::run_provider_connection_test().await?,
        "atomic" => e2e_test_runner::test_categories::run_atomic_tests().await?,
        "pool" => e2e_test_runner::test_categories::run_pool_tests().await?,
        "evm" => e2e_test_runner::test_categories::run_evm_tests().await?,
        "unit" => e2e_test_runner::test_categories::run_unit_tests().await?,
        "performance" => e2e_test_runner::test_categories::run_performance_tests().await?,
        "memory" => e2e_test_runner::test_categories::run_memory_tests().await?,
        "environment" => e2e_test_runner::test_categories::run_environment_tests().await?,
        "transaction" => e2e_test_runner::test_categories::run_transaction_tests().await?,
        "component" => {
            e2e_test_runner::test_categories::run_pool_tests().await?;
            e2e_test_runner::test_categories::run_evm_tests().await?;
        }
        "integration" => e2e_test_runner::test_categories::run_integration_tests().await?,
        "full-flow" => e2e_test_runner::test_categories::run_comprehensive_flow_tests().await?,
        "edge-cases" => e2e_test_runner::test_categories::run_edge_case_tests().await?,
        "stress" => e2e_test_runner::test_categories::run_edge_case_tests().await?,
        "all" => {
            e2e_test_runner::test_categories::run_atomic_tests().await?;
            e2e_test_runner::test_categories::run_unit_tests().await?;
            e2e_test_runner::test_categories::run_pool_tests().await?;
            e2e_test_runner::test_categories::run_evm_tests().await?;
            e2e_test_runner::test_categories::run_environment_tests().await?;
            e2e_test_runner::test_categories::run_transaction_tests().await?;
            e2e_test_runner::test_categories::run_performance_tests().await?;
            e2e_test_runner::test_categories::run_memory_tests().await?;
            e2e_test_runner::test_categories::run_integration_tests().await?;
            e2e_test_runner::test_categories::run_comprehensive_flow_tests().await?;
            e2e_test_runner::test_categories::run_edge_case_tests().await?;
        }
        _ => {
            eprintln!("❌ Unknown test: {}", test_name);
            eprintln!("Available tests: provider, atomic, unit, pool, evm, environment, transaction, performance, memory, component, integration, full-flow, edge-cases, stress, all");
            process::exit(1);
        }
    }

    println!("✅ All selected test categories completed successfully!");
    Ok(())
}
