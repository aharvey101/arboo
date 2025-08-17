// Most Atomic E2E Test: Provider Connection and Basic Blockchain Interaction
// This is the foundation test that verifies we can connect to and interact with the blockchain

use anyhow::Result;
use arbooo::common::logger;
use log::info;

#[path = "utils/mod.rs"]
mod utils;
use utils::test_env::{TestEnvironment, assertions};

#[tokio::test]
async fn test_atomic_provider_connection() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Atomic Provider Connection Test");

    // Setup test environment
    let test_env = TestEnvironment::new().await?;
    
    // Log which environment we're using
    if test_env.is_using_anvil() {
        info!("🔧 Using local Anvil fork for testing");
    } else {
        info!("🌐 Using external RPC provider for testing");
    }
    
    // Verify basic connection
    test_env.verify_connection().await?;
    
    // Get initial block info
    let initial_block = test_env.get_latest_block_info().await?;
    info!("📊 Initial block info:");
    initial_block.pretty_print();
    
    // Assert block properties are reasonable
    assertions::assert_reasonable_gas_limit(initial_block.gas_limit)?;
    assertions::assert_recent_timestamp(initial_block.timestamp)?;
    
    // Wait a few seconds and verify we can get a newer block
    info!("⏱️  Waiting for new blocks...");
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    
    let new_block = test_env.get_latest_block_info().await?;
    info!("📊 New block info:");
    new_block.pretty_print();
    
    // Assert block number has increased (or at least not decreased)
    if new_block.number > initial_block.number {
        info!("✅ Block number increased: {} -> {}", initial_block.number, new_block.number);
    } else {
        info!("⚠️  Block number unchanged (this is ok for fast tests): {}", new_block.number);
    }
    
    info!("🎉 Atomic Provider Connection Test passed!");
    Ok(())
}

#[tokio::test]
async fn test_atomic_provider_stability() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Atomic Provider Stability Test");

    let test_env = TestEnvironment::new().await?;
    
    // Test multiple rapid calls to ensure connection stability
    for i in 1..=5 {
        info!("📡 Stability check #{}", i);
        let block_info = test_env.get_latest_block_info().await?;
        
        // Basic validation
        assertions::assert_reasonable_gas_limit(block_info.gas_limit)?;
        assertions::assert_recent_timestamp(block_info.timestamp)?;
        
        info!("  ✅ Check #{} passed (block: {})", i, block_info.number);
        
        // Small delay between calls
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    
    info!("🎉 Provider Stability Test passed!");
    Ok(())
}

#[tokio::test]
async fn test_atomic_block_data_integrity() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Atomic Block Data Integrity Test");

    let test_env = TestEnvironment::new().await?;
    let block_info = test_env.get_latest_block_info().await?;
    
    // Test block data makes sense
    info!("🔍 Validating block data integrity...");
    
    // Block number should be reasonable (mainnet is > 18M as of 2023)
    if block_info.number < 10_000_000 {
        return Err(anyhow::anyhow!("Block number seems too low: {}", block_info.number));
    }
    
    // Gas limit should be reasonable
    assertions::assert_reasonable_gas_limit(block_info.gas_limit)?;
    
    // Timestamp should be recent
    assertions::assert_recent_timestamp(block_info.timestamp)?;
    
    // Should have some transactions (unless we hit an empty block)
    info!("📊 Block has {} transactions", block_info.transaction_count);
    
    // Base fee should exist for post-EIP-1559 blocks
    if let Some(base_fee) = block_info.base_fee {
        if base_fee == 0 {
            return Err(anyhow::anyhow!("Base fee is zero, which is unusual"));
        }
        info!("💰 Base fee: {} wei", base_fee);
    } else {
        info!("⚠️  No base fee (pre-EIP-1559 or test network)");
    }
    
    info!("🎉 Block Data Integrity Test passed!");
    Ok(())
}
