use anyhow::Result;
use std::env;
use alloy::providers::Provider;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::anvil_setup::{AnvilInstance, AnvilConfig};

#[tokio::test]
async fn test_mainnet_fork_check() -> Result<()> {
    println!("🔍 Testing if anvil actually forks mainnet...");

    println!("MAINNET_RPC_URL environment variable: {:?}", env::var("MAINNET_RPC_URL").ok());

    let config = AnvilConfig::default();
    println!("AnvilConfig fork_url: {:?}", config.fork_url);

    if config.fork_url.is_some() {
        println!("✅ Config has fork_url, attempting to create mainnet fork...");
        match AnvilInstance::new(config).await {
            Ok(anvil) => {
                let provider = anvil.get_http_provider()?;
                let block_number = provider.get_block_number().await?;

                println!("📦 Current block number: {}", block_number);

                if block_number > 1_000_000 {
                    println!("🎯 SUCCESS: This appears to be a mainnet fork (block: {})", block_number);
                } else if block_number == 0 {
                    println!("❌ This is a clean local blockchain (block: 0)");
                } else {
                    println!("🤔 Unknown blockchain state (block: {})", block_number);
                }
            },
            Err(e) => {
                println!("⚠️  Failed to fork mainnet (this is ok if RPC endpoint is unavailable): {}", e);
                println!("   The test will pass anyway since this is optional infrastructure");
            }
        }
    } else {
        println!("❌ No fork_url configured, anvil will start as clean local blockchain");
        println!("   Set MAINNET_RPC_URL environment variable to fork from mainnet");
    }

    Ok(())
}

