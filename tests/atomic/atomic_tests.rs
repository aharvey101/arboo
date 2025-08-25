use anyhow::Result;
use arbooo::common::logger;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::{TestEnvironment, assertions};

#[tokio::test]
async fn test_atomic_provider_connection() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Atomic Provider Connection Test");

    let test_env = TestEnvironment::new().await?;

    if test_env.is_using_anvil() {
        info!("🔧 Using local Anvil fork for testing");
    } else {
        info!("🌐 Using external RPC provider for testing");
    }

    test_env.verify_connection().await?;

    let initial_block = test_env.get_latest_block_info().await?;
    info!("📊 Initial block info:");
    initial_block.pretty_print();

    assertions::assert_reasonable_gas_limit(initial_block.gas_limit)?;
    assertions::assert_recent_timestamp(initial_block.timestamp)?;

    info!("⏱️  Waiting for new blocks...");
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    let new_block = test_env.get_latest_block_info().await?;
    info!("📊 New block info:");
    new_block.pretty_print();

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

    for i in 1..=5 {
        info!("📡 Stability check #{}", i);
        let block_info = test_env.get_latest_block_info().await?;

        assertions::assert_reasonable_gas_limit(block_info.gas_limit)?;
        assertions::assert_recent_timestamp(block_info.timestamp)?;

        info!("  ✅ Check #{} passed (block: {})", i, block_info.number);

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

    info!("🔍 Validating block data integrity...");

    if test_env.is_using_anvil() {
        info!("🔧 Using Anvil - block number {} is valid for local fork", block_info.number);

    } else {

        if block_info.number < 10_000_000 {
            return Err(anyhow::anyhow!("Block number seems too low for live network: {}", block_info.number));
        }
    }

    assertions::assert_reasonable_gas_limit(block_info.gas_limit)?;

    assertions::assert_recent_timestamp(block_info.timestamp)?;

    info!("📊 Block has {} transactions", block_info.transaction_count);

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

