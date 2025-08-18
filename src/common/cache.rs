use crate::common::revm::EvmSimulator;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::network::Ethereum;
use revm::primitives::{Address, U256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

// EVM simulator cache to avoid recreation overhead
#[derive(Clone)]
pub struct EvmSimulatorCache {
    cache: Arc<RwLock<HashMap<String, (String, Instant)>>>, // Store cache keys instead of simulators
    max_age: Duration,
    max_size: usize,
}

impl EvmSimulatorCache {
    pub fn new(max_size: usize, max_age_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_age: Duration::from_secs(max_age_seconds),
            max_size,
        }
    }

    // For now, just create new simulators since EvmSimulator doesn't implement Clone
    // In a production environment, you'd implement proper caching or make EvmSimulator cloneable
    pub async fn get_or_create_simulator(
        &self,
        _cache_key: String,
        provider: RootProvider<PubSubFrontend, Ethereum>,
        owner: Option<Address>,
        block_number: alloy::primitives::U64,
    ) -> anyhow::Result<EvmSimulator> {
        log::debug!("Creating new EVM simulator (caching not yet implemented for EvmSimulator)");
        let mut new_simulator = EvmSimulator::new(provider, owner, block_number)?;
        
        // Pre-setup the simulator with common state
        self.setup_simulator_optimized(&mut new_simulator).await?;
        Ok(new_simulator)
    }

    async fn setup_simulator_optimized(&self, simulator: &mut EvmSimulator) -> anyhow::Result<()> {
        // Setup common state that all simulations need
        simulator.setup().await;
        log::debug!("EVM simulator setup completed");
        Ok(())
    }

    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        
        cache.retain(|_, (_, created_at)| {
            now.duration_since(*created_at) < self.max_age
        });
        
        log::debug!("Cleaned up expired cache entries, remaining: {}", cache.len());
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        log::debug!("Cleared all cache entries");
    }

    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

// Block data cache to avoid repeated network calls
#[derive(Clone)]
pub struct BlockDataCache {
    latest_block_data: Arc<RwLock<Option<CachedBlockData>>>,
    cache_duration: Duration,
}

#[derive(Clone)]
struct CachedBlockData {
    block_number: u64,
    gas_limit: u64,
    gas_price: U256,
    base_fee: u64,
    timestamp: u64,
    cached_at: Instant,
}

impl BlockDataCache {
    pub fn new(cache_duration_seconds: u64) -> Self {
        Self {
            latest_block_data: Arc::new(RwLock::new(None)),
            cache_duration: Duration::from_secs(cache_duration_seconds),
        }
    }

    pub async fn get_latest_block_data(
        &self,
        provider: &RootProvider<PubSubFrontend, Ethereum>,
    ) -> anyhow::Result<(u64, U256, u64)> {
        // Check cache first
        {
            let cache = self.latest_block_data.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.cached_at.elapsed() < self.cache_duration {
                    return Ok((cached.gas_limit, cached.gas_price, cached.base_fee));
                }
            }
        }

        // Fetch fresh data
        log::debug!("Fetching fresh block data");
        let block_number = provider.get_block_number().await?;
        let latest_block = provider
            .get_block(
                alloy::eips::BlockId::latest(),
                alloy::rpc::types::BlockTransactionsKind::Hashes,
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?;

        let gas_limit = latest_block.header.gas_limit;
        let base_fee = latest_block.header.base_fee_per_gas
            .ok_or_else(|| anyhow::anyhow!("Block missing base_fee_per_gas"))?;
        let gas_price = U256::from(base_fee);

        // Cache the data
        {
            let mut cache = self.latest_block_data.write().await;
            *cache = Some(CachedBlockData {
                block_number,
                gas_limit,
                gas_price,
                base_fee,
                timestamp: latest_block.header.timestamp,
                cached_at: Instant::now(),
            });
        }

        Ok((gas_limit, gas_price, base_fee))
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.latest_block_data.write().await;
        *cache = None;
        log::debug!("Cleared block data cache");
    }
}
