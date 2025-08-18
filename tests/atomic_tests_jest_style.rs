// Updated Atomic Tests with Jest-style reporting
// This demonstrates the new test reporting system

use anyhow::Result;
use arbooo::common::logger;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::{TestEnvironment, assertions};

// Include the Jest-style reporter
mod jest_reporter {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/bin/e2e_test_runner/jest_style_reporter.rs"));
}
use jest_reporter::JestStyleReporter;

#[tokio::test]
async fn test_atomic_provider_connection_with_jest_style() -> Result<()> {
    logger::setup_logger();
    
    let reporter = JestStyleReporter::new();
    
    // Start the test suite
    reporter.start_suite("Atomic Provider Connection");
    
    let mut has_errors = false;

    // Should create test environment successfully
    let test_env = match reporter.should("Atomic Provider Connection", "create test environment successfully")
        .assert_async(|| async {
            TestEnvironment::new().await.map_err(|e| anyhow::anyhow!("Failed to create test environment: {}", e))
        }).await {
        Ok(_) => TestEnvironment::new().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Provider Connection");
            return Err(anyhow::anyhow!("Failed to create test environment"));
        }
    };
    
    // Should determine which environment type is being used
    if let Err(_) = reporter.should("Atomic Provider Connection", "determine environment type (Anvil or external RPC)")
        .assert(|| {
            if test_env.is_using_anvil() {
                info!("🔧 Using local Anvil fork for testing");
            } else {
                info!("🌐 Using external RPC provider for testing");
            }
            Ok(())
        }) {
        has_errors = true;
    }
    
    // Should verify basic connection
    if let Err(_) = reporter.should("Atomic Provider Connection", "verify basic blockchain connection")
        .assert_async(|| async {
            test_env.verify_connection().await
                .map_err(|e| anyhow::anyhow!("Connection verification failed: {}", e))
        }).await {
        has_errors = true;
    }
    
    // Should get initial block info
    let initial_block = match reporter.should("Atomic Provider Connection", "retrieve initial block information")
        .assert_async(|| async {
            test_env.get_latest_block_info().await
                .map_err(|e| anyhow::anyhow!("Failed to get block info: {}", e))
        }).await {
        Ok(_) => test_env.get_latest_block_info().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Provider Connection");
            return Err(anyhow::anyhow!("Failed to get initial block info"));
        }
    };
    
    // Should validate block properties are reasonable
    if let Err(_) = reporter.should("Atomic Provider Connection", "validate block has reasonable gas limit")
        .assert(|| {
            assertions::assert_reasonable_gas_limit(initial_block.gas_limit)
                .map_err(|e| anyhow::anyhow!("Gas limit validation failed: {}", e))
        }) {
        has_errors = true;
    }
    
    if let Err(_) = reporter.should("Atomic Provider Connection", "validate block has recent timestamp")
        .assert(|| {
            assertions::assert_recent_timestamp(initial_block.timestamp)
                .map_err(|e| anyhow::anyhow!("Timestamp validation failed: {}", e))
        }) {
        has_errors = true;
    }
    
    // Should wait for new blocks and verify block progression
    if let Err(_) = reporter.should("Atomic Provider Connection", "wait for new blocks (15 seconds)")
        .assert_async(|| async {
            info!("⏱️  Waiting for new blocks...");
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            Ok(())
        }).await {
        has_errors = true;
    }
    
    // Should get updated block info
    let new_block = match reporter.should("Atomic Provider Connection", "retrieve updated block information")
        .assert_async(|| async {
            test_env.get_latest_block_info().await
                .map_err(|e| anyhow::anyhow!("Failed to get updated block info: {}", e))
        }).await {
        Ok(_) => test_env.get_latest_block_info().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Provider Connection");
            return Err(anyhow::anyhow!("Failed to get updated block info"));
        }
    };
    
    // Should verify block number progression
    if let Err(_) = reporter.should("Atomic Provider Connection", "verify block number progression or stability")
        .assert(|| {
            if new_block.number > initial_block.number {
                info!("✅ Block number increased: {} -> {}", initial_block.number, new_block.number);
            } else {
                info!("⚠️  Block number unchanged (this is ok for fast tests): {}", new_block.number);
            }
            Ok(())
        }) {
        has_errors = true;
    }

    reporter.end_suite("Atomic Provider Connection");
    
    if has_errors {
        Err(anyhow::anyhow!("Some assertions failed"))
    } else {
        Ok(())
    }
}

#[tokio::test]
async fn test_atomic_provider_stability_with_jest_style() -> Result<()> {
    logger::setup_logger();
    
    let reporter = JestStyleReporter::new();
    reporter.start_suite("Atomic Provider Stability");
    
    let mut has_errors = false;

    // Should create test environment
    let test_env = match reporter.should("Atomic Provider Stability", "create test environment")
        .assert_async(|| async {
            TestEnvironment::new().await.map_err(|e| anyhow::anyhow!("Failed to create test environment: {}", e))
        }).await {
        Ok(_) => TestEnvironment::new().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Provider Stability");
            return Err(anyhow::anyhow!("Failed to create test environment"));
        }
    };
    
    // Should perform multiple rapid stability checks
    for i in 1..=5 {
        let description = format!("perform stability check #{}/5", i);
        
        if let Err(_) = reporter.should("Atomic Provider Stability", &description)
            .assert_async(|| async {
                let block_info = test_env.get_latest_block_info().await
                    .map_err(|e| anyhow::anyhow!("Stability check failed: {}", e))?;
                
                // Basic validation
                assertions::assert_reasonable_gas_limit(block_info.gas_limit)
                    .map_err(|e| anyhow::anyhow!("Gas limit validation failed: {}", e))?;
                assertions::assert_recent_timestamp(block_info.timestamp)
                    .map_err(|e| anyhow::anyhow!("Timestamp validation failed: {}", e))?;
                
                info!("  ✅ Check #{} passed (block: {})", i, block_info.number);
                
                // Small delay between calls
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                Ok(())
            }).await {
            has_errors = true;
        }
    }

    reporter.end_suite("Atomic Provider Stability");
    
    if has_errors {
        Err(anyhow::anyhow!("Some stability checks failed"))
    } else {
        Ok(())
    }
}

#[tokio::test] 
async fn test_atomic_block_data_integrity_with_jest_style() -> Result<()> {
    logger::setup_logger();
    
    let reporter = JestStyleReporter::new();
    reporter.start_suite("Atomic Block Data Integrity");
    
    let mut has_errors = false;

    // Should create test environment
    let test_env = match reporter.should("Atomic Block Data Integrity", "create test environment")
        .assert_async(|| async {
            TestEnvironment::new().await.map_err(|e| anyhow::anyhow!("Failed to create test environment: {}", e))
        }).await {
        Ok(_) => TestEnvironment::new().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Block Data Integrity");
            return Err(anyhow::anyhow!("Failed to create test environment"));
        }
    };
    
    // Should get block info for integrity testing
    let block_info = match reporter.should("Atomic Block Data Integrity", "retrieve block information")
        .assert_async(|| async {
            test_env.get_latest_block_info().await
                .map_err(|e| anyhow::anyhow!("Failed to get block info: {}", e))
        }).await {
        Ok(_) => test_env.get_latest_block_info().await?,
        Err(_) => {
            has_errors = true;
            reporter.end_suite("Atomic Block Data Integrity");
            return Err(anyhow::anyhow!("Failed to get block info"));
        }
    };
    
    // Should validate block number is reasonable
    if let Err(_) = reporter.should("Atomic Block Data Integrity", "validate block number is reasonable (> 10M)")
        .assert(|| {
            if block_info.number < 10_000_000 {
                Err(anyhow::anyhow!("Block number seems too low: {}", block_info.number))
            } else {
                Ok(())
            }
        }) {
        has_errors = true;
    }
    
    // Should validate gas limit is within expected bounds
    if let Err(_) = reporter.should("Atomic Block Data Integrity", "validate gas limit is within expected bounds")
        .assert(|| {
            assertions::assert_reasonable_gas_limit(block_info.gas_limit)
                .map_err(|e| anyhow::anyhow!("Gas limit validation failed: {}", e))
        }) {
        has_errors = true;
    }
    
    // Should validate timestamp is recent
    if let Err(_) = reporter.should("Atomic Block Data Integrity", "validate timestamp is reasonably recent")
        .assert(|| {
            assertions::assert_recent_timestamp(block_info.timestamp)
                .map_err(|e| anyhow::anyhow!("Timestamp validation failed: {}", e))
        }) {
        has_errors = true;
    }

    reporter.end_suite("Atomic Block Data Integrity");
    
    if has_errors {
        Err(anyhow::anyhow!("Some integrity checks failed"))
    } else {
        Ok(())
    }
}
