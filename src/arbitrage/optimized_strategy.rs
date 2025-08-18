use crate::common::connection_pool::ConnectionPool;
use crate::common::{
    logs::LogEvent,
    revm::EvmSimulator,
};
use alloy::eips::BlockId;
use alloy::providers::Provider;
use alloy::rpc::types::BlockTransactionsKind;
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::U64;
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;

pub struct OptimizedStrategyWorkerPool {
    sender: mpsc::Sender<LogEvent>,
    #[allow(dead_code)]
    connection_pool: ConnectionPool,
}

impl OptimizedStrategyWorkerPool {
    pub fn new(sender: Sender<LogEvent>, ws_url: String, max_connections: usize) -> Self {
        let (tx, _rx) = mpsc::channel::<LogEvent>(1000);
        let connection_pool = ConnectionPool::new(ws_url, max_connections);
        
        // Clone for the worker tasks
        let pool_clone = connection_pool.clone();
        
        // Spawn worker tasks
        for worker_id in 0..max_connections {
            let mut event_receiver = sender.subscribe();
            let pool = pool_clone.clone();
            
            tokio::spawn(async move {
                info!("Starting strategy worker {}", worker_id);
                while let Ok(log_event) = event_receiver.recv().await {
                    if let Err(e) = process_strategy_optimized(log_event, &pool).await {
                        log::error!("Worker {} failed to process strategy: {}", worker_id, e);
                    }
                }
            });
        }
        
        Self {
            sender: tx,
            connection_pool,
        }
    }

    pub async fn submit_event(&self, event: LogEvent) -> Result<()> {
        self.sender.send(event).await
            .map_err(|e| anyhow::anyhow!("Failed to submit event: {}", e))?;
        Ok(())
    }
}

pub async fn process_strategy_optimized(
    _message: LogEvent, 
    connection_pool: &ConnectionPool
) -> Result<()> {
    let start_time = std::time::Instant::now();
    
    // Get a pooled provider instead of creating a new connection
    let pooled_provider = connection_pool.get_provider().await?;
    let provider = pooled_provider.provider();
    
    info!("Time to get pooled provider: {:?}", start_time.elapsed());
    
    let latest_block_number = provider
        .get_block_number()
        .await
        .map_err(|e| anyhow::anyhow!("Error getting block number: {}", e))?;

    // Use Arc to avoid cloning the entire block data
    let _latest_block = Arc::new(
        provider
            .get_block(BlockId::latest(), BlockTransactionsKind::Full)
            .await
            .map_err(|e| anyhow::anyhow!("Error getting latest block: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("Latest block not found"))?
    );
    
    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    // Create EVM simulator with the pooled provider
    // Note: We need to convert the provider appropriately
    let _simulator = EvmSimulator::new(
        // This might need adjustment based on your provider type
        pooled_provider.into_provider(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    ).map_err(|e| anyhow::anyhow!("Failed to create EVM simulator: {}", e))?;

    info!("Time to create optimized EVM: {:?}", start_time.elapsed());
    
    // Continue with the rest of your strategy logic...
    // The rest of the function remains the same but uses Arc<Block> instead of cloning
    
    Ok(())
}

// Keep the original function for backward compatibility
pub async fn process_strategy(message: LogEvent, ws_url: String) -> Result<()> {
    // This is the old inefficient version - should be replaced with optimized version
    log::warn!("Using non-optimized process_strategy - consider switching to optimized version");
    
    // Create a temporary connection pool for this single operation
    let pool = ConnectionPool::new(ws_url, 1);
    process_strategy_optimized(message, &pool).await
}

// Usage in your main function
pub async fn initialize_optimized_strategy_pool(
    sender: Sender<LogEvent>, 
    ws_url: String,
    max_connections: usize
) -> OptimizedStrategyWorkerPool {
    OptimizedStrategyWorkerPool::new(sender, ws_url, max_connections)
}
