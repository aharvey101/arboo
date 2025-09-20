use alloy::providers::{ProviderBuilder, RootProvider, WsConnect};
use alloy::pubsub::PubSubFrontend;
use alloy::network::Ethereum;
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, warn};

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pool: Arc<Mutex<VecDeque<RootProvider<PubSubFrontend, Ethereum>>>>,
    ws_url: String,
    max_connections: usize,
    current_size: Arc<Mutex<usize>>,
}

impl ConnectionPool {
    pub fn new(ws_url: String, max_connections: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::new())),
            ws_url,
            max_connections,
            current_size: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn get_provider(&self) -> Result<PooledProvider> {
        let mut pool = self.pool.lock().await;
        
        if let Some(provider) = pool.pop_front() {
            info!("Reusing existing WebSocket connection");
            return Ok(PooledProvider {
                provider: Some(provider),
                pool: self.clone(),
            });
        }
        
        drop(pool); // Release lock early
        
        let mut current_size = self.current_size.lock().await;
        if *current_size >= self.max_connections {
            warn!("Connection pool exhausted, creating temporary connection");
            return self.create_temporary_provider().await;
        }
        
        *current_size += 1;
        drop(current_size);
        
        info!("Creating new WebSocket connection for pool");
        let provider = self.create_provider().await?;
        
        Ok(PooledProvider {
            provider: Some(provider),
            pool: self.clone(),
        })
    }

    async fn create_provider(&self) -> Result<RootProvider<PubSubFrontend, Ethereum>> {
        let ws_client = WsConnect::new(&self.ws_url);
        ProviderBuilder::new()
            .on_ws(ws_client)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))
    }

    async fn create_temporary_provider(&self) -> Result<PooledProvider> {
        let provider = self.create_provider().await?;
        Ok(PooledProvider {
            provider: Some(provider),
            pool: self.clone(),
        })
    }

    async fn return_provider(&self, provider: RootProvider<PubSubFrontend, Ethereum>) {
        let mut pool = self.pool.lock().await;
        if pool.len() < self.max_connections {
            pool.push_back(provider);
            info!("Provider returned to pool");
        } else {
            info!("Pool full, dropping provider");
        }
    }
}

pub struct PooledProvider {
    provider: Option<RootProvider<PubSubFrontend, Ethereum>>,
    pool: ConnectionPool,
}

impl PooledProvider {
    pub fn provider(&self) -> &RootProvider<PubSubFrontend, Ethereum> {
        self.provider.as_ref().expect("Provider already consumed")
    }

    pub fn into_provider(mut self) -> RootProvider<PubSubFrontend, Ethereum> {
        self.provider.take().expect("Provider already consumed")
    }
}

impl Drop for PooledProvider {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.return_provider(provider).await;
            });
        }
    }
}
