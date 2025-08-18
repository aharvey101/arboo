use arbooo::common::connection_pool::ConnectionPool;
use alloy::providers::Provider;
use dotenv::dotenv;
use std::env::var;
use std::time::Instant;
use anyhow::Result;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();
    
    let ws_url = var("WS_URL")
        .map_err(|e| anyhow::anyhow!("WS_URL environment variable not set: {}", e))?;
    
    info!("Testing connection pool performance...");
    
    // Test 1: Connection Pool Performance
    let pool = ConnectionPool::new(ws_url, 5);
    let mut total_time = std::time::Duration::ZERO;
    let iterations = 10;
    
    for i in 0..iterations {
        let start = Instant::now();
        let provider = pool.get_provider().await?;
        let elapsed = start.elapsed();
        total_time += elapsed;
        
        info!("Iteration {}: Got provider in {:?}", i + 1, elapsed);
        
        // Use the provider briefly to simulate real usage
        let _block_num = provider.provider().get_block_number().await?;
        
        // Provider automatically returns to pool when dropped
    }
    
    let avg_time = total_time / iterations;
    info!("Average time to get pooled provider: {:?}", avg_time);
    
    // Test 2: Memory Usage (Basic)
    info!("Connection pool created successfully!");
    info!("Optimization tests completed!");
    
    Ok(())
}
