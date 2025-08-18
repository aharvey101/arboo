use alloy::providers::ProviderBuilder;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use arbooo::arbitrage::strategy::initialize_strategy_pool;
use arbooo::common::logger;
use arbooo::common::logs;
use arbooo::common::pools;
use arbooo::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
};
use dotenv::dotenv;
use dotenv::var;
use log::info;
use revm::primitives::Address;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    logger::setup_logger();
    info!("Logger setup");
    let ws_url = var::<&str>("WS_URL")
        .map_err(|e| anyhow::anyhow!("WS_URL environment variable not set: {}", e))?;
    let cache_dir = var("CACHE_DIR")
        .unwrap_or_else(|_| "/tmp/arboo-cache".to_string());
    
    let ws_client = WsConnect::new(ws_url.clone());

    let provider = ProviderBuilder::new().on_ws(ws_client).await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;
    let provider = Arc::new(provider);

    let cache_path = format!("{}/.cached-pools.csv", cache_dir);
    if !Path::new(&cache_path).try_exists()? {
        info!("Cache doesn't exist, crawling blocks for pools");
        pools::load_all_pools(ws_url.clone(), 100_000, 50_000)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load pools: {}", e))?;
    }

    let mut set = JoinSet::new();

    let (sender, _): (Sender<LogEvent>, _) = broadcast::channel(512);

    // 1. Get all pools

    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new(&cache_path);
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    // id,address,version,token0,oken1,fee,block_number,timestamp
    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line?;
        let fields: Vec<&str> = line.split(',').collect();

        match fields[2] {
            "2" => {
                let pair_address = Address::from_str(fields[1])
                    .map_err(|e| anyhow::anyhow!("Invalid V2 pair address '{}': {}", fields[1], e))?;
                pools_map.insert(
                    pair_address,
                    Event::PairCreated(V2PoolCreated {
                        pair_address: Address::from_str(fields[1])
                            .map_err(|e| anyhow::anyhow!("Invalid V2 pair address '{}': {}", fields[1], e))?,
                        token0: Address::from_str(fields[3])
                            .map_err(|e| anyhow::anyhow!("Invalid V2 token0 address '{}': {}", fields[3], e))?,
                        token1: Address::from_str(fields[4])
                            .map_err(|e| anyhow::anyhow!("Invalid V2 token1 address '{}': {}", fields[4], e))?,
                        fee: fields[5].parse::<u32>()
                            .map_err(|e| anyhow::anyhow!("Invalid V2 fee '{}': {}", fields[5], e))?,
                        //block_number: fields[6].parse::<u64>().map_err(|e| anyhow::anyhow!("Invalid V2 block number '{}': {}", fields[6], e))?,
                    }),
                );
            }
            "3" => {
                let pair_address = Address::from_str(fields[1])
                    .map_err(|e| anyhow::anyhow!("Invalid V3 pair address '{}': {}", fields[1], e))?;
                pools_map.insert(
                    pair_address,
                    Event::PoolCreated(V3PoolCreated {
                        pair_address: Address::from_str(fields[1])
                            .map_err(|e| anyhow::anyhow!("Invalid V3 pair address '{}': {}", fields[1], e))?,
                        token0: Address::from_str(fields[3])
                            .map_err(|e| anyhow::anyhow!("Invalid V3 token0 address '{}': {}", fields[3], e))?,
                        token1: Address::from_str(fields[4])
                            .map_err(|e| anyhow::anyhow!("Invalid V3 token1 address '{}': {}", fields[4], e))?,
                        fee: fields[5].parse::<u32>()
                            .map_err(|e| anyhow::anyhow!("Invalid V3 fee '{}': {}", fields[5], e))?,
                        tick_spacing: 0i32,
                    }),
                );
            }
            &_ => continue,
        };
    }

    // 2. Listen for logs on pools
    set.spawn(logs::get_logs(provider.clone(), pools_map, sender.clone()));

    info!("Spawning optimized EVM strategy with {} worker threads", 16);
    let _strategy_pool = initialize_strategy_pool(sender, ws_url, 16).await?;
    
    // Add graceful shutdown handling
    info!("Arbitrage bot started. Press Ctrl+C to shutdown gracefully.");
    
    tokio::select! {
        // Wait for tasks to complete
        _ = async {
            while let Some(res) = set.join_next().await {
                log::debug!("{:?}", res);
                if res.is_err() {
                    log::error!("Task failed: {:?}", res);
                }
            }
        } => {
            info!("All tasks completed");
        }
        // Wait for Ctrl+C signal
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
            set.abort_all(); // Abort all spawned tasks
            
            // Give tasks a moment to cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            info!("Shutdown complete");
        }
    }

    Ok(())
}

// MVP What is left to do:
// [] Fix up all the decoding so that we can understand the errors
// [x] Create an Inspector
// [x] Make it take profitable Arbitrages :shrug:
// [ ] Make a ETH usdt ETH USD bot
