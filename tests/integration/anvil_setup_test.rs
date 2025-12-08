use anyhow::Result;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

/// Simple test to verify anvil setup works correctly with latest block
#[tokio::test]
async fn test_anvil_setup_with_latest_block() -> Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    info!("🧪 Testing Anvil setup with latest block");

    // Create test environment - this uses latest block by default
    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created successfully");

    // Verify connection
    test_env.verify_connection().await?;
    info!("✅ Connection verified");

    // Get block info
    let block_info = test_env.get_latest_block_info().await?;
    block_info.pretty_print();

    // Basic assertions
    assert!(block_info.number > 0, "Block number should be > 0");
    assert!(block_info.gas_limit > 0, "Gas limit should be > 0");

    info!("✅ Anvil setup test passed!");
    Ok(())
}
