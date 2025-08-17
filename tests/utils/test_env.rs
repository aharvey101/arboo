// Test Environment Utilities
// Provides common setup and utilities for E2E tests

use anyhow::Result;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use log::info;
use std::sync::Arc;
use revm::primitives::Address;
use super::anvil_setup::{AnvilInstance, create_mainnet_fork};

pub struct TestEnvironment {
    pub provider: Arc<RootProvider<PubSubFrontend>>,
    pub test_config: TestConfig,
    pub anvil_instance: Option<AnvilInstance>, // Optional anvil instance for local testing
}

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub ws_url: String,
    pub use_local_fork: bool,
    pub fork_block_number: Option<u64>,
    pub test_timeout_secs: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        // Environment variables are loaded externally
        // Don't load .env files here as they may override our test configuration
        
        println!("🔍 Environment Variables Debug:");
        println!("  TEST_WS_URL: {:?}", std::env::var("TEST_WS_URL"));
        println!("  WS_URL: {:?}", std::env::var("WS_URL"));
        println!("  USE_LOCAL_FORK: {:?}", std::env::var("USE_LOCAL_FORK"));
        println!("  MAINNET_RPC_URL: {:?}", std::env::var("MAINNET_RPC_URL"));
        
        let use_local_fork = std::env::var("USE_LOCAL_FORK")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);
        
        // If we're using local fork (anvil), we'll get the URL from anvil later
        // Otherwise, try environment variables
        let ws_url = if use_local_fork {
            "".to_string() // Will be set when anvil starts
        } else {
            std::env::var("TEST_WS_URL")
                .or_else(|_| std::env::var("WS_URL"))
                .unwrap_or_else(|_| "wss://eth.merkle.io".to_string())
        };
        
        println!("  Final ws_url: {}", if ws_url.is_empty() { "[will use anvil]" } else { &ws_url });
        println!("  Final use_local_fork: {}", use_local_fork);
        
        Self {
            ws_url,
            use_local_fork,
            fork_block_number: std::env::var("FORK_BLOCK_NUMBER")
                .ok()
                .and_then(|s| s.parse().ok()),
            test_timeout_secs: std::env::var("TEST_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
        }
    }
}

impl TestEnvironment {
    pub async fn new() -> Result<Self> {
        Self::new_with_config(TestConfig::default()).await
    }
    
    pub async fn new_with_config(config: TestConfig) -> Result<Self> {
        info!("🏗️  Setting up test environment...");
        info!("🔧 Config - use_local_fork: {}", config.use_local_fork);
        info!("🔧 Config - ws_url: {}", config.ws_url);
        info!("🔧 Config - fork_block_number: {:?}", config.fork_block_number);
        
        let (provider, anvil_instance) = if config.use_local_fork {
            info!("🔧 Setting up local anvil fork...");
            let anvil = create_mainnet_fork(config.fork_block_number).await?;
            
            // Connect to the local anvil instance via WebSocket
            let ws_url = format!("ws://127.0.0.1:{}", anvil.port);
            info!("🔗 Connecting to local anvil at: {}", ws_url);
            
            let ws_client = WsConnect::new(ws_url);
            let provider = ProviderBuilder::new().on_ws(ws_client).await?;
            let provider = Arc::new(provider);
            
            (provider, Some(anvil))
        } else {
            info!("🌐 Connecting to external provider: {}", config.ws_url);
            let ws_client = WsConnect::new(config.ws_url.clone());
            let provider = ProviderBuilder::new().on_ws(ws_client).await?;
            let provider = Arc::new(provider);
            
            (provider, None)
        };
        
        info!("✅ Test environment ready");
        
        Ok(Self {
            provider,
            test_config: config,
            anvil_instance,
        })
    }
    
    pub async fn verify_connection(&self) -> Result<()> {
        info!("🔍 Verifying test environment connection...");
        
        let block_number = self.provider.get_block_number().await?;
        if let Some(anvil) = &self.anvil_instance {
            info!("📦 Connected to local anvil (port {}) at block: {}", anvil.port, block_number);
        } else {
            info!("📦 Connected to external provider at block: {}", block_number);
        }
        
        Ok(())
    }
    
    /// Returns true if using local anvil, false if using external provider
    pub fn is_using_anvil(&self) -> bool {
        self.anvil_instance.is_some()
    }
    
    pub async fn get_latest_block_info(&self) -> Result<TestBlockInfo> {
        let block_number = self.provider.get_block_number().await?;
        let block = self.provider
            .get_block(
                alloy::eips::BlockId::latest(), 
                alloy::rpc::types::BlockTransactionsKind::Hashes
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("Could not fetch latest block"))?;
            
        Ok(TestBlockInfo {
            number: block_number,
            hash: block.header.hash,
            timestamp: block.header.timestamp,
            gas_limit: block.header.gas_limit,
            base_fee: block.header.base_fee_per_gas,
            transaction_count: block.transactions.len(),
        })
    }
}

#[derive(Debug)]
pub struct TestBlockInfo {
    pub number: u64,
    pub hash: alloy::primitives::B256,
    pub timestamp: u64,
    pub gas_limit: u64,
    pub base_fee: Option<u64>,
    pub transaction_count: usize,
}

impl TestBlockInfo {
    pub fn pretty_print(&self) {
        println!("📦 Block Info:");
        println!("   Number: {}", self.number);
        println!("   Hash: {:?}", self.hash);
        println!("   Timestamp: {}", self.timestamp);
        println!("   Gas Limit: {}", self.gas_limit);
        println!("   Base Fee: {:?}", self.base_fee);
        println!("   Transactions: {}", self.transaction_count);
    }
}

// Test assertion helpers
pub mod assertions {
    use super::*;
    
    pub fn assert_block_number_increasing(old_block: u64, new_block: u64) -> Result<()> {
        if new_block <= old_block {
            return Err(anyhow::anyhow!(
                "Block number not increasing: {} -> {}", old_block, new_block
            ));
        }
        Ok(())
    }
    
    pub fn assert_reasonable_gas_limit(gas_limit: u64) -> Result<()> {
        const MIN_GAS_LIMIT: u64 = 8_000_000;  // 8M gas
        const MAX_GAS_LIMIT: u64 = 50_000_000; // 50M gas
        
        if gas_limit < MIN_GAS_LIMIT || gas_limit > MAX_GAS_LIMIT {
            return Err(anyhow::anyhow!(
                "Unreasonable gas limit: {} (expected between {} and {})",
                gas_limit, MIN_GAS_LIMIT, MAX_GAS_LIMIT
            ));
        }
        Ok(())
    }
    
    pub fn assert_recent_timestamp(timestamp: u64) -> Result<()> {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        const MAX_BLOCK_AGE_SECONDS: u64 = 3600; // 1 hour
        
        if now.saturating_sub(timestamp) > MAX_BLOCK_AGE_SECONDS {
            return Err(anyhow::anyhow!(
                "Block timestamp too old: {} (current: {})", timestamp, now
            ));
        }
        Ok(())
    }
}
