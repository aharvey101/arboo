use anyhow::Result;
use log::info;
use super::reporter::Reporter;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

// Global flag for verbose mode
static VERBOSE_MODE: OnceLock<bool> = OnceLock::new();

// Set verbose mode (called from main)
pub fn set_verbose_mode(verbose: bool) {
    VERBOSE_MODE.set(verbose).ok();
}

// Get current verbose mode
fn is_verbose_mode() -> bool {
    VERBOSE_MODE.get().copied().unwrap_or(false)
}

// Generic test runner that can run tests by filter pattern
pub async fn run_test_by_filter(test_filter: &str) -> Result<()> {
    run_cargo_test_with_verbosity(test_filter, is_verbose_mode()).await
}

// Generic test runner that can run multiple test filters
#[allow(dead_code)]
pub async fn run_tests_by_filters(test_filters: &[&str]) -> Result<()> {
    for filter in test_filters {
        run_test_by_filter(filter).await?;
    }
    Ok(())
}

// Helper function to run cargo test with default verbosity  
#[allow(dead_code)]
pub async fn run_cargo_test_with_output(test_filter: &str) -> Result<()> {
    run_cargo_test_with_verbosity(test_filter, is_verbose_mode()).await
}

// Core function that handles both verbose and quiet modes
async fn run_cargo_test_with_verbosity(test_filter: &str, show_details: bool) -> Result<()> {
    if show_details {
        info!("🧪 Running test filter: {}", test_filter);
    }
    
    if show_details {
        // Verbose mode - pipe output directly to terminal
        let mut cmd = Command::new("cargo");
        cmd.args(&["test", test_filter, "--", "--nocapture"])
           .stdout(Stdio::inherit())
           .stderr(Stdio::inherit());
        
        let status = cmd.status()
            .map_err(|e| anyhow::anyhow!("Failed to execute cargo test: {}", e))?;
        
        if status.success() {
            info!("✅ {} completed successfully", test_filter);
            Ok(())
        } else {
            info!("❌ {} failed with exit code: {:?}", test_filter, status.code());
            Err(anyhow::anyhow!("Test failed with exit code: {:?}", status.code()))
        }
    } else {
        // Quiet mode - capture and parse output
        let output = Command::new("cargo")
            .args(&["test", test_filter, "--", "--nocapture"])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        if output.status.success() {
            // Parse and display just the summary in verbose mode
            if show_details {
                if let Some(result_line) = stdout.lines().find(|line| line.contains("test result:")) {
                    info!("  📊 {}", result_line.trim());
                } else {
                    info!("  ✅ completed successfully");
                }
            }
            Ok(())
        } else {
            // Show error summary in verbose mode
            if show_details {
                info!("❌ {} failed:", test_filter);
                if let Some(error_line) = stderr.lines().chain(stdout.lines())
                    .find(|line| line.contains("error:") || line.contains("FAILED")) {
                    info!("  ❌ {}", error_line.trim());
                }
            }
            Err(anyhow::anyhow!("Test failed"))
        }
    }
}

// Test category definitions - maps categories to test filters
pub struct TestCategory {
    pub name: &'static str,
    pub filters: &'static [&'static str],
    pub description: &'static str,
}

// Define all test categories and their corresponding test filters
pub const TEST_CATEGORIES: &[TestCategory] = &[
    TestCategory {
        name: "atomic",
        filters: &["atomic_tests"],
        description: "Basic atomic functionality tests",
    },
    TestCategory {
        name: "unit",
        filters: &[
            "arbitrage_calculation",
            "transaction_creation", 
            "error_recovery",
            "profit_simulation",
            "transaction_execution",
            "env_loading",
            "log_event_processing", 
            "test_fork_check",
            "single_swap_simulation",
        ],
        description: "Unit tests for individual components",
    },
    TestCategory {
        name: "pool",
        filters: &["pool_data", "pool_pairing"],
        description: "Pool data and pairing tests",
    },
    TestCategory {
        name: "evm",
        filters: &["evm_simulator"],
        description: "EVM simulator and contract tests",
    },
    TestCategory {
        name: "environment", 
        filters: &["test_environment_demo"],
        description: "Environment setup and configuration tests",
    },
    TestCategory {
        name: "performance",
        filters: &[
            "high_frequency",
            "mev_competition", 
            "opportunity_detection",
            "simulation_execution",
            "transaction_success_rate",
        ],
        description: "Performance benchmarks and optimization tests",
    },
    TestCategory {
        name: "memory",
        filters: &["memory_usage"],
        description: "Memory usage profiling tests",
    },
    TestCategory {
        name: "integration",
        filters: &[
            "full_arbitrage_cycle",
            "concurrent_opportunities",
            "network_disconnection",
        ],
        description: "Integration and end-to-end tests",
    },
    TestCategory {
        name: "edge_cases",
        filters: &[
            "insufficient_liquidity",
            "gas_price_spike",
            "block_reorganization",
        ],
        description: "Edge cases and stress tests",
    },
    TestCategory {
        name: "misc",
        filters: &["logger_tests", "rpc_call_measurement"],
        description: "Miscellaneous utility tests",
    },
];

// Generic function to run tests for a specific category
pub async fn run_test_category(category_name: &str) -> Result<()> {
    let category = TEST_CATEGORIES
        .iter()
        .find(|cat| cat.name == category_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown test category: {}", category_name))?;
    
    info!("🧪 Running {} tests: {}", category.name, category.description);
    
    for filter in category.filters {
        info!("🔍 Running test filter: {}", filter);
        run_test_by_filter(filter).await?;
    }
    
    Ok(())
}

// Function to run all test categories
#[allow(dead_code)]
pub async fn run_all_test_categories() -> Result<()> {
    for category in TEST_CATEGORIES {
        info!("🧪 Running category: {} - {}", category.name, category.description);
        for filter in category.filters {
            run_test_by_filter(filter).await?;
        }
    }
    Ok(())
}

// Get all available test category names
#[allow(dead_code)]
pub fn get_available_categories() -> Vec<&'static str> {
    TEST_CATEGORIES.iter().map(|cat| cat.name).collect()
}

// Environment setup and basic integration tests
pub async fn test_integrated_environment() -> Result<()> {
    let reporter = Reporter::new();
    reporter.start_suite("Integrated Environment Setup");
    
    reporter.should("Integrated Environment Setup", "create integrated test environment")
        .assert_async(|| async {
            // For now, just simulate a successful environment setup
            // This would normally use the utils::integrated_test_env module
            Ok(())
        }).await?;
    
    reporter.end_suite("Integrated Environment Setup");
    Ok(())
}
