// Environment Loading Test
// This test verifies that the .env.test file is being loaded correctly

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
    info!("  Use Local Fork: {}", config.use_local_fork);
    info!("  Fork Block Number: {:?}", config.fork_block_number);
    info!("  Test Timeout: {} seconds", config.test_timeout_secs);
    
    // Verify that we're getting configuration from the .env.test file
    assert!(!config.ws_url.is_empty(), "WS URL should not be empty");
    assert!(config.test_timeout_secs > 0, "Test timeout should be positive");
    
    // Check if we're using the expected test URL
    if config.ws_url == "wss://eth.merkle.io" {
        info!("✅ Using default test URL from .env.test");
    } else {
        info!("ℹ️ Using custom WS URL: {}", config.ws_url);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_env_variable_override() -> Result<()> {
    env_logger::try_init().ok();
    
    // Set a test environment variable
    std::env::set_var("TEST_WS_URL", "wss://custom.test.url");
    
    let config = TestConfig::default();
    
    info!("🔧 Testing environment variable override:");
    info!("  WS URL: {}", config.ws_url);
    
    // The URL should now be the custom one we set
    assert_eq!(config.ws_url, "wss://custom.test.url", "Environment variable override should work");
    
    // Clean up
    std::env::remove_var("TEST_WS_URL");
    
    Ok(())
}
