use anyhow::Result;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestConfig;

#[tokio::test]
async fn test_env_loading() -> Result<()> {
    env_logger::try_init().ok();

    let config = TestConfig::default();

    info!("📊 Test Configuration:");
    info!("  WS URL: {}", config.ws_url);
    info!("  Fork Block Number: {:?}", config.fork_block_number);
    info!("  Test Timeout: {} seconds", config.test_timeout_secs);

    assert!(config.test_timeout_secs > 0, "Test timeout should be positive");

    if config.ws_url.is_empty() {
        info!("✅ Will use local anvil fork");
    } else {
        info!("✅ Using configured URL: {}", config.ws_url);
    }

    Ok(())
}

#[tokio::test]
async fn test_env_variable_override() -> Result<()> {
    env_logger::try_init().ok();

    std::env::set_var("FORK_BLOCK_NUMBER", "12345678");

    let config = TestConfig::default();

    info!("🔧 Testing environment variable override:");
    info!("  Fork Block Number: {:?}", config.fork_block_number);

    assert_eq!(config.fork_block_number, Some(12345678), "Environment variable override should work");

    std::env::remove_var("FORK_BLOCK_NUMBER");

    Ok(())
}

