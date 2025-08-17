// Anvil Local Blockchain Fork Setup
// Provides utilities for setting up and managing local Anvil forks for testing

use anyhow::{Result, Context};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use alloy::providers::{Provider, ProviderBuilder};
use log::{info, warn, debug};
use portpicker::pick_unused_port;

pub struct AnvilInstance {
    process: Child,
    pub rpc_url: String,
    pub ws_url: String,
    pub port: u16,
    pub ws_port: u16,
    pub chain_id: u64,
}

pub struct AnvilConfig {
    pub fork_url: Option<String>,
    pub fork_block_number: Option<u64>,
    pub chain_id: u64,
    pub accounts: u32,
    pub balance: u64, // in ETH
    pub gas_limit: u64,
    pub gas_price: u64,
    pub base_fee: u64,
}

impl Default for AnvilConfig {
    fn default() -> Self {
        Self {
            fork_url: std::env::var("MAINNET_RPC_URL").ok(),
            fork_block_number: None,
            chain_id: 31337, // Anvil default
            accounts: 10,
            balance: 10000, // 10k ETH per account
            gas_limit: 30_000_000,
            gas_price: 20_000_000_000, // 20 gwei
            base_fee: 1_000_000_000,   // 1 gwei
        }
    }
}

impl AnvilInstance {
    /// Start a new Anvil instance with the given configuration
    pub async fn new(config: AnvilConfig) -> Result<Self> {
        let port = pick_unused_port()
            .context("Failed to find unused port for Anvil")?;

        info!("🔧 Starting Anvil instance on port {}", port);

        let mut cmd = Command::new("anvil");
        
        // Basic configuration with verbose logging
        cmd.arg("--port").arg(port.to_string())
           .arg("--chain-id").arg(config.chain_id.to_string())
           .arg("--accounts").arg(config.accounts.to_string())
           .arg("--balance").arg(config.balance.to_string())
           .arg("--gas-limit").arg(config.gas_limit.to_string())
           .arg("--gas-price").arg(config.gas_price.to_string())
           .arg("--base-fee").arg(config.base_fee.to_string())
           .arg("--host").arg("127.0.0.1");

        // Fork configuration if provided
        if let Some(fork_url) = &config.fork_url {
            cmd.arg("--fork-url").arg(fork_url);
            
            if let Some(block) = config.fork_block_number {
                cmd.arg("--fork-block-number").arg(block.to_string());
                info!("📡 Forking from {} at specific block {}", fork_url, block);
            } else {
                info!("📡 Forking from {} at latest block", fork_url);
            }
        }

        // Debug: print the exact command
        info!("🔧 Starting Anvil with command: {:?}", cmd);
        info!("🎯 Anvil will be available at http://127.0.0.1:{} and ws://127.0.0.1:{}", port, port);

        // Start the process - inherit stdout/stderr so we can see anvil logs
        let process = cmd
            .stdout(Stdio::inherit()) // Show anvil output directly
            .stderr(Stdio::inherit()) // Show anvil errors directly
            .spawn()
            .context("Failed to start Anvil process")?;

        debug!("Anvil process started with PID: {:?}", process.id());

        let rpc_url = format!("http://127.0.0.1:{}", port);
        let ws_url = format!("ws://127.0.0.1:{}", port); // Same port for both

        let instance = Self {
            process,
            rpc_url: rpc_url.clone(),
            ws_url: ws_url.clone(),
            port,
            ws_port: port, // Same port for WebSocket
            chain_id: config.chain_id,
        };

        // Wait for Anvil to be ready
        instance.wait_for_ready().await?;
        
        info!("✅ Anvil instance ready at {} (WS: {})", rpc_url, ws_url);
        
        Ok(instance)
    }

    /// Wait for Anvil to become ready by polling the RPC endpoint
    async fn wait_for_ready(&self) -> Result<()> {
        debug!("⏳ Waiting for Anvil to become ready...");
        
        // Give Anvil a bit more time to fully start up
        sleep(Duration::from_millis(1000)).await;
        
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 30;
        const DELAY_MS: u64 = 200;

        while attempts < MAX_ATTEMPTS {
            // Use a simple HTTP client to test connectivity
            let client = reqwest::Client::new();
            let response = client
                .post(&self.rpc_url)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_blockNumber",
                    "params": [],
                    "id": 1
                }))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) if json.get("result").is_some() => {
                            debug!("🎯 Anvil ready! Connected successfully");
                            return Ok(());
                        }
                        Ok(_) => {
                            debug!("Attempt {}/{}: Got response but no result field", attempts + 1, MAX_ATTEMPTS);
                        }
                        Err(e) => {
                            debug!("Attempt {}/{}: Failed to parse response: {}", attempts + 1, MAX_ATTEMPTS, e);
                        }
                    }
                }
                Ok(resp) => {
                    debug!("Attempt {}/{}: Got HTTP {} response", attempts + 1, MAX_ATTEMPTS, resp.status());
                }
                Err(e) => {
                    debug!("Attempt {}/{}: Connection failed: {}", attempts + 1, MAX_ATTEMPTS, e);
                }
            }
            
            attempts += 1;
            sleep(Duration::from_millis(DELAY_MS)).await;
        }

        Err(anyhow::anyhow!(
            "Anvil failed to become ready after {} attempts at {}", MAX_ATTEMPTS, self.rpc_url
        ))
    }

    /// Get a provider connected to this Anvil instance (HTTP)
    pub fn get_http_provider(&self) -> Result<alloy::providers::RootProvider<alloy::transports::http::Http<reqwest::Client>>> {
        let url = self.rpc_url.parse()?;
        Ok(ProviderBuilder::new().on_http(url))
    }

    /// Get a provider connected to this Anvil instance (WebSocket)
    pub async fn get_ws_provider(&self) -> Result<alloy::providers::RootProvider<alloy::pubsub::PubSubFrontend>> {
        let ws_client = alloy::rpc::client::WsConnect::new(self.ws_url.clone());
        let provider = ProviderBuilder::new().on_ws(ws_client).await?;
        Ok(provider)
    }

    /// Mine a number of blocks
    pub async fn mine_blocks(&self, count: u64) -> Result<()> {
        let provider = self.get_http_provider()?;
        
        for _ in 0..count {
            // Use anvil_mine RPC method
            let _: serde_json::Value = provider
                .client()
                .request("anvil_mine", ())
                .await?;
        }
        
        debug!("⛏️  Mined {} blocks", count);
        Ok(())
    }

    /// Set the next block timestamp
    pub async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<()> {
        let provider = self.get_http_provider()?;
        
        let _: serde_json::Value = provider
            .client()
            .request("anvil_setNextBlockTimestamp", (timestamp,))
            .await?;
            
        debug!("🕐 Set next block timestamp to {}", timestamp);
        Ok(())
    }

    /// Increase time by the given number of seconds
    pub async fn increase_time(&self, seconds: u64) -> Result<()> {
        let provider = self.get_http_provider()?;
        
        let _: serde_json::Value = provider
            .client()
            .request("anvil_increaseTime", (seconds,))
            .await?;
            
        debug!("⏰ Increased time by {} seconds", seconds);
        Ok(())
    }

    /// Reset the fork to a specific block number
    pub async fn reset_fork(&self, block_number: Option<u64>) -> Result<()> {
        let provider = self.get_http_provider()?;
        
        let params = match block_number {
            Some(block) => serde_json::json!({
                "blockNumber": format!("0x{:x}", block)
            }),
            None => serde_json::json!({}),
        };
        
        let _: serde_json::Value = provider
            .client()
            .request("anvil_reset", (params,))
            .await?;
            
        debug!("🔄 Reset fork to block {:?}", block_number);
        Ok(())
    }

    /// Get account addresses and private keys
    pub async fn get_accounts(&self) -> Result<Vec<AccountInfo>> {
        let provider = self.get_http_provider()?;
        
        let accounts: Vec<String> = provider
            .client()
            .request("eth_accounts", ())
            .await?;
            
        let private_keys: Vec<String> = provider
            .client()
            .request("anvil_dumpState", ())
            .await
            .ok()
            .and_then(|state: serde_json::Value| {
                state.get("accounts")?.as_object()?
                    .values()
                    .filter_map(|acc| acc.get("secretKey")?.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .into()
            })
            .unwrap_or_default();

        let mut account_infos = Vec::new();
        for (i, address) in accounts.iter().enumerate() {
            let private_key = private_keys.get(i).cloned();
            account_infos.push(AccountInfo {
                address: address.clone(),
                private_key,
                index: i,
            });
        }

        Ok(account_infos)
    }

    /// Stop the Anvil instance
    pub async fn stop(mut self) -> Result<()> {
        info!("🔥 Stopping Anvil instance on port {}", self.port);
        let _ = self.process.kill();
        let _ = self.process.wait();
        debug!("Anvil instance stopped");
        Ok(())
    }
}

impl Drop for AnvilInstance {
    fn drop(&mut self) {
        info!("🔥 Shutting down Anvil instance (ports {}/{})", self.port, self.ws_port);
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub address: String,
    pub private_key: Option<String>,
    pub index: usize,
}

impl AccountInfo {
    pub fn address_as_alloy(&self) -> Result<alloy::primitives::Address> {
        self.address.parse()
            .context("Failed to parse address")
    }
}

/// Helper function to create a test Anvil instance with mainnet fork
pub async fn create_mainnet_fork(block_number: Option<u64>) -> Result<AnvilInstance> {
    let fork_url = std::env::var("MAINNET_RPC_URL").ok();
    println!("fork url: {:?}", fork_url);
    let config = AnvilConfig {
        fork_url,
        fork_block_number: block_number,
        ..Default::default()
    };
    println!("config: {:?}", config.fork_url);
    AnvilInstance::new(config).await
}

/// Helper function to create a clean Anvil instance without forking
pub async fn create_clean_anvil() -> Result<AnvilInstance> {
    let config = AnvilConfig {
        fork_url: None,
        ..Default::default()
    };

    AnvilInstance::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anvil_startup() -> Result<()> {
        let anvil = create_clean_anvil().await?;
        let provider = anvil.get_http_provider()?;
        
        let block_number = provider.get_block_number().await?;
        assert_eq!(block_number, 0);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_anvil_mining() -> Result<()> {
        let anvil = create_clean_anvil().await?;
        let provider = anvil.get_http_provider()?;
        
        let initial_block = provider.get_block_number().await?;
        anvil.mine_blocks(3).await?;
        let final_block = provider.get_block_number().await?;
        
        assert_eq!(final_block, initial_block + 3);
        
        Ok(())
    }
}
