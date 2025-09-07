use alloy::providers::ProviderBuilder;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use arbooo::common::logger;
use arbooo::common::logs;
use arbooo::common::pools;
use arbooo::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
};
use arbooo::strategies::StrategyManager;
use dotenv::dotenv;
use dotenv::var;
use log::{info, debug, warn};
use revm::primitives::Address;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::RwLock;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    logger::setup_logger();
    info!("🚀 Starting Generalized MEV Bot");

    let ws_url = var::<&str>("WS_URL")
        .map_err(|e| anyhow::anyhow!("WS_URL environment variable not set: {}", e))?;
    let cache_dir = var("CACHE_DIR").unwrap_or_else(|_| "/tmp/arboo-cache".to_string());

    let ws_client = WsConnect::new(ws_url.clone());
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await
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

    // Create channels for different event types
    let (log_event_sender, _): (Sender<LogEvent>, _) = broadcast::channel(512);

    // Load pools map
    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new(&cache_path);
    let file = File::open(path).map_err(|e| anyhow::anyhow!("Failed to load pools file {}", e))?;
    let reader = io::BufReader::new(file);

    for line in reader.lines().skip(1) {
        let line = line?;
        let fields: Vec<&str> = line.split(',').collect();

        match fields[2] {
            "2" => {
                let pair_address = Address::from_str(fields[1]).map_err(|e| {
                    anyhow::anyhow!("Invalid V2 pair address '{}': {}", fields[1], e)
                })?;
                pools_map.insert(
                    pair_address,
                    Event::PairCreated(V2PoolCreated {
                        pair_address,
                        token0: Address::from_str(fields[3]).map_err(|e| {
                            anyhow::anyhow!("Invalid V2 token0 address '{}': {}", fields[3], e)
                        })?,
                        token1: Address::from_str(fields[4]).map_err(|e| {
                            anyhow::anyhow!("Invalid V2 token1 address '{}': {}", fields[4], e)
                        })?,
                        fee: fields[5].parse::<u32>().map_err(|e| {
                            anyhow::anyhow!("Invalid V2 fee '{}': {}", fields[5], e)
                        })?,
                    }),
                );
            }
            "3" => {
                let pair_address = Address::from_str(fields[1]).map_err(|e| {
                    anyhow::anyhow!("Invalid V3 pair address '{}': {}", fields[1], e)
                })?;
                pools_map.insert(
                    pair_address,
                    Event::PoolCreated(V3PoolCreated {
                        pair_address,
                        token0: Address::from_str(fields[3]).map_err(|e| {
                            anyhow::anyhow!("Invalid V3 token0 address '{}': {}", fields[3], e)
                        })?,
                        token1: Address::from_str(fields[4]).map_err(|e| {
                            anyhow::anyhow!("Invalid V3 token1 address '{}': {}", fields[4], e)
                        })?,
                        fee: fields[5].parse::<u32>().map_err(|e| {
                            anyhow::anyhow!("Invalid V3 fee '{}': {}", fields[5], e)
                        })?,
                        tick_spacing: 0i32,
                    }),
                );
            }
            _ => continue,
        };
    }

    let pools_map = Arc::new(RwLock::new(pools_map));
    info!(
        "📊 Loaded {} pools into cache",
        pools_map.read().await.len()
    );

    // Initialize the generalized strategy manager
    let executor_address = Address::from_str(
        &var("EXECUTOR_ADDRESS")
            .unwrap_or_else(|_| "0x5f1F5565561aC146d24B102D9CDC288992Ab2938".to_string()),
    )?;

    let strategy_manager = StrategyManager::new(
        ws_url.clone(),
        30000, // max connections
        executor_address,
    )
    .await?;

    // Create a bridge task to convert LogEvents to MevEvents (not needed for now)
    let mut log_receiver = log_event_sender.subscribe();

    // Start log listener - need to pass the HashMap, not Arc<RwLock<HashMap>>
    let pools_map_for_logs = {
        let guard = pools_map.read().await;
        guard.clone()
    };
    set.spawn(logs::get_logs(
        provider.clone(),
        pools_map_for_logs,
        log_event_sender,
    ));

    // Start strategy manager on main thread (non-Send types can't cross thread boundaries)
    let strategy_manager_task = async move {
        info!("🚀 Starting Strategy Manager event processing loop");
        
        while let Ok(log_event) = log_receiver.recv().await {
            // Process each log event for arbitrage opportunities
            match strategy_manager.process_arbitrage_cycle(log_event).await {
                Ok(results) => {
                    if !results.is_empty() {
                        info!("📊 Processed arbitrage cycle with {} results", results.len());
                        for result in results {
                            if result.success {
                                info!("✅ Successful arbitrage execution: Profit {} wei", result.profit);
                            } else {
                                debug!("📉 Unprofitable arbitrage attempt");
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("⚠️ Arbitrage cycle processing failed: {}", e);
                }
            }
        }
        
        warn!("📡 Log receiver channel closed, stopping strategy manager");
    };

    info!("🎯 Generalized MEV Bot started successfully!");
    info!("📈 Available strategies: Arbitrage, Sandwich (disabled), Liquidation (disabled)");
    info!("🔧 Press Ctrl+C to shutdown gracefully");

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
        // Wait for strategy manager to complete
        _ = strategy_manager_task => {
            info!("Strategy manager completed");
        }
        // Wait for Ctrl+C signal
        _ = signal::ctrl_c() => {
            info!("🛑 Received Ctrl+C, shutting down gracefully...");
            set.abort_all();

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            info!("✅ Shutdown complete");
        }
    }

    Ok(())
}
