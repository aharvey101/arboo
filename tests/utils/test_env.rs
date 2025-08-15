// Test Environment Utilities
// Provides common setup and utilities for E2E tests

use anyhow::Result;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use log::info;
use std::sync::Arc;
use revm::primitives::Address;

pub struct TestEnvironment {
    pub provider: Arc<RootProvider<PubSubFrontend>>,
    pub test_config: TestConfig,
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
        // Try to load .env.test file first, then fall back to .env
        let _ = dotenv::from_filename(".env.test").or_else(|_| dotenv::dotenv());
        
        Self {
            ws_url: std::env::var("TEST_WS_URL")
                .unwrap_or_else(|_| "wss://eth.merkle.io".to_string()),
            use_local_fork: std::env::var("USE_LOCAL_FORK")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
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
        
        let ws_client = WsConnect::new(config.ws_url.clone());
        let provider = ProviderBuilder::new().on_ws(ws_client).await?;
        let provider = Arc::new(provider);
        
        info!("✅ Test environment ready with provider: {}", config.ws_url);
        
        Ok(Self {
            provider,
            test_config: config,
        })
    }
    
    pub async fn verify_connection(&self) -> Result<()> {
        info!("🔍 Verifying test environment connection...");
        
        let block_number = self.provider.get_block_number().await?;
        info!("📦 Connected to block: {}", block_number);
        
        Ok(())
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
