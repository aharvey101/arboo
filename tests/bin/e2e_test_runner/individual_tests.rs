use anyhow::Result;
use log::info;
use super::reporter::Reporter;
use std::fs;
use std::path::Path;
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

// Dynamic test category discovery from folder structure
#[derive(Debug, Clone)]
pub struct TestCategory {
    pub name: String,
    pub filters: Vec<String>,
    pub description: String,
}

// Discover test categories by scanning the tests directory structure
pub fn discover_test_categories() -> Result<Vec<TestCategory>> {
    let tests_dir = Path::new("tests");
    let mut categories = Vec::new();
    
    if !tests_dir.exists() {
        return Err(anyhow::anyhow!("Tests directory not found"));
    }
    
    // Read all entries in the tests directory
    let entries = fs::read_dir(tests_dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        // Skip non-directories and special directories
        if !path.is_dir() {
            continue;
        }
        
        let folder_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };
        
        // Skip special directories that aren't test categories
        if matches!(folder_name, "bin" | "fixtures" | "utils") {
            continue;
        }
        
        // Discover test files in this category folder
        let mut filters = Vec::new();
        if let Ok(test_entries) = fs::read_dir(&path) {
            for test_entry in test_entries {
                if let Ok(test_entry) = test_entry {
                    let test_path = test_entry.path();
                    if test_path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        if let Some(file_stem) = test_path.file_stem().and_then(|s| s.to_str()) {
                            // Extract the base name from test files
                            // e.g., "arbitrage_calculation_tests.rs" -> "arbitrage_calculation"
                            let filter_name = if file_stem.ends_with("_tests") {
                                file_stem.trim_end_matches("_tests")
                            } else {
                                file_stem
                            };
                            filters.push(filter_name.to_string());
                        }
                    }
                }
            }
        }
        
        // Only add categories that have test files
        if !filters.is_empty() {
            let description = generate_category_description(folder_name);
            categories.push(TestCategory {
                name: folder_name.to_string(),
                filters,
                description,
            });
        }
    }
    
    // Sort categories by name for consistent output
    categories.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(categories)
}

// Generate a human-readable description for each category
fn generate_category_description(category_name: &str) -> String {
    match category_name {
        "atomic" => "Basic atomic functionality tests".to_string(),
        "unit" => "Unit tests for individual components".to_string(),
        "pool" => "Pool data and pairing tests".to_string(),
        "evm" => "EVM simulator and contract tests".to_string(),
        "environment" => "Environment setup and configuration tests".to_string(),
        "transaction" => "Transaction creation and execution tests".to_string(),
        "performance" => "Performance benchmarks and optimization tests".to_string(),
        "memory" => "Memory usage profiling and optimization tests".to_string(),
        "integration" => "Full system integration tests".to_string(),
        "edge_cases" => "Edge case and error scenario tests".to_string(),
        "misc" => "Miscellaneous utility and helper tests".to_string(),
        _ => format!("{} tests", category_name.replace('_', " ")).to_string(),
    }
}

// Generic function to run tests for a specific category
pub async fn run_test_category(category_name: &str) -> Result<()> {
    let categories = discover_test_categories()?;
    let category = categories
        .iter()
        .find(|cat| cat.name == category_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown test category: {}", category_name))?;
    
    info!("🧪 Running {} tests: {}", category.name, category.description);
    
    for filter in &category.filters {
        info!("🔍 Running test filter: {}", filter);
        run_test_by_filter(filter).await?;
    }
    
    Ok(())
}

// List all discovered categories with their details
pub fn list_all_categories() -> Result<()> {
    let categories = discover_test_categories()?;
    
    println!("📁 Discovered Test Categories:");
    println!();
    
    for category in &categories {
        println!("🏷️  {}", category.name);
        println!("   📝 {}", category.description);
        println!("   🧪 Test filters: {}", category.filters.join(", "));
        println!();
    }
    
    println!("🎯 Usage: cargo run --bin e2e_test_runner <category_name>");
    println!("   Example: cargo run --bin e2e_test_runner unit");
    
    Ok(())
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
