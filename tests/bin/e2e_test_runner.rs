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
    println!("Commands:");
    println!("  list          - List all available test categories");
    println!("  all           - Run all discovered test categories (default)");
    println!("  <category>    - Run a specific test category");
    println!();
    println!("Options:");
    println!("  -v, --verbose - Show detailed test output and logs");
    println!("  -q, --quiet   - Show only test summaries (default)");
    println!("  -h, --help    - Show this help message");
    println!();
    println!("Examples:");
    println!("  cargo run --bin e2e_test_runner list");
    println!("  cargo run --bin e2e_test_runner unit");
    println!("  cargo run --bin e2e_test_runner --verbose performance");
    println!("  cargo run --bin e2e_test_runner -v all");
    println!();
    println!("Note: Test categories are auto-discovered from the tests/ folder structure.");
}

#[tokio::main]
async fn main() -> Result<()> {

    setup_test_logger();

    let args: Vec<String> = std::env::args().collect();
    let mut verbose = false;

    for arg in args.iter().skip(1) {
        if arg == "--verbose" || arg == "-v" {
            verbose = true;
            break;
        }
    }

    if verbose {
        info!("🧪 Starting E2E Test Runner");
    }

    let overall_reporter = Reporter::new();
    overall_reporter.start_suite("E2E Test Runner - All Test Suites");

    let mut test_name = "all";
    verbose = false;

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

                if !arg.starts_with('-') {
                    test_name = arg;
                }
            }
        }
    }

    if verbose {
        info!("🔍 Running in verbose mode - showing detailed test output");
    } else {

    }

    e2e_test_runner::individual_tests::set_verbose_mode(verbose);

    match test_name {
        "list" => {
            e2e_test_runner::individual_tests::list_all_categories()?;
            return Ok(());
        }
        "all" => {

            let categories = e2e_test_runner::individual_tests::discover_test_categories()?;
            for category in &categories {
                if verbose {
                    info!("Running category: {}", category.name);
                }
                e2e_test_runner::individual_tests::run_test_category(&category.name).await?;
            }
        }

        "provider" => e2e_test_runner::test_categories::run_provider_connection_test().await?,
        "component" => {
            e2e_test_runner::test_categories::run_pool_tests().await?;
            e2e_test_runner::test_categories::run_evm_tests().await?;
        }
        "full-flow" => e2e_test_runner::test_categories::run_comprehensive_flow_tests().await?,
        "stress" => e2e_test_runner::test_categories::run_edge_case_tests().await?,
        category_name => {

            match e2e_test_runner::individual_tests::run_test_category(category_name).await {
                Ok(_) => {

                }
                Err(_) => {
                    eprintln!("❌ Unknown test category: {}", category_name);
                    eprintln!("💡 Run 'cargo run --bin e2e_test_runner list' to see available categories");
                    process::exit(1);
                }
            }
        }
    }

    println!("✅ All selected test categories completed successfully!");
    Ok(())
}

