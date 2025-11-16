use alloy::providers::Provider;
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
use log::{debug, info, warn};
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
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Hello World");
    dotenv()?;
    logger::setup_logger();
    info!("🚀 Starting Generalized MEV Bot");

    let ws_url = var::<&str>("WS_URL")
        .map_err(|e| anyhow::anyhow!("WS_URL environment variable not set: {}", e))?;
    info!("WS URL {:?}", ws_url);
    let cache_dir = var("CACHE_DIR").unwrap_or_else(|_| "/tmp/arboo-cache".to_string());

    let ws_client = WsConnect::new(ws_url.clone());
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;
    let block_number = provider.get_block_number().await?;

    info!("Block Number: {:?}", block_number);

    let provider = Arc::new(provider);

     let cache_path = format!("{}/.cached-pools.csv", cache_dir);
     
     // Check if cache exists and has pools
     let should_scan_blocks = if !Path::new(&cache_path).try_exists()? {
         info!("Cache doesn't exist, crawling blocks for pools");
         true
     } else {
         // Check if cache file is empty (only has headers)
         let file = File::open(&cache_path).map_err(|e| anyhow::anyhow!("Failed to open pools file {}", e))?;
         let reader = io::BufReader::new(file);
         let line_count = reader.lines().count();
         
         if line_count <= 1 {
             // Only header or completely empty
             info!("Cache file is empty (only {} lines), crawling blocks for pools", line_count);
             true
         } else {
             info!("Cache file exists with {} lines, using cached pools", line_count);
             false
         }
     };

     if should_scan_blocks {
         // Start from block 100k (when most pools were created) with 50k block chunks
         pools::load_all_pools(ws_url.clone(), 100_000, 50_000, &cache_path)
             .await
             .map_err(|e| anyhow::anyhow!("Failed to load pools: {}", e))?;
     }

     let mut set = JoinSet::new();

     // Create cancellation token for graceful shutdown
     let cancellation_token = CancellationToken::new();

     // Create channels for different event types
     let (log_event_sender, _): (Sender<LogEvent>, _) = broadcast::channel(512);

     // Load pools map
     let mut pools_map: HashMap<Address, Event> = HashMap::new();
     let path = Path::new(&cache_path);
     info!("📂 Opening cache file at: {}", cache_path);
     let file = File::open(path).map_err(|e| anyhow::anyhow!("Failed to load pools file {}", e))?;
     //info!("File: {:?}", file);
     let reader = io::BufReader::new(file);

     let mut v2_count = 0;
     let mut v3_count = 0;
     let mut skipped_count = 0;
     
     info!("🔍 Starting to parse pools from cache file - checking field lengths");
     
     // Define our target tokens for detailed logging
     let dai_addr = "0x6b175474e89094c44da98b954eedeac495271d0f".to_lowercase();
     let weth_addr = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_lowercase();

     for line in reader.lines().skip(1) {
          let line = line?;
          let fields: Vec<&str> = line.split(',').collect();
          
          // Debug: check fields count and version field
          if fields.len() < 6 {
              warn!("⚠️ Skipping malformed line with {} fields: {}", fields.len(), line);
              skipped_count += 1;
              continue;
          }
          
          let version_str = fields[2];
          if fields.len() < 6 || fields.len() > 8 {
              warn!("⚠️ Unexpected field count: {} fields (expected 6-8)", fields.len());
          }
          
          if version_str != "2" && version_str != "3" {
              warn!("⚠️ Unknown version string: '{}' (len={})", version_str, version_str.len());
          }
          
          // Get token addresses for logging
          let token0_lower = fields.get(3).map(|t| t.to_lowercase()).unwrap_or_default();
          let token1_lower = fields.get(4).map(|t| t.to_lowercase()).unwrap_or_default();
          let is_dai_weth = (token0_lower == dai_addr && token1_lower == weth_addr) ||
                            (token0_lower == weth_addr && token1_lower == dai_addr);
          
          match version_str {
               "2" => {
                   v2_count += 1;
                   if is_dai_weth {
                       info!("✅ Found V2 DAI/WETH pool: address={}, token0={}, token1={}, fee={}", 
                           fields[1], fields[3], fields[4], fields[5]);
                   }
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
                  v3_count += 1;
                  if is_dai_weth {
                      info!("✅ Found V3 DAI/WETH pool: address={}, token0={}, token1={}, fee={}", 
                          fields[1], fields[3], fields[4], fields[5]);
                  }
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
              _ => {
                  skipped_count += 1;
                  warn!(
                      "⚠️ Skipping unknown pool version '{}' (len={}) for address {}",
                      version_str, version_str.len(), fields[1]
                  );
                  if skipped_count <= 10 {
                      debug!("Full line: {}", line);
                  }
                  continue;
              }
          };
     }

    let pools_map = Arc::new(RwLock::new(pools_map));
    info!(
        "📊 Loaded {} pools into cache (V2: {}, V3: {}, Skipped: {})",
        pools_map.read().await.len(),
        v2_count,
        v3_count,
        skipped_count
    );

    let mut strategy_manager = StrategyManager::new(
        ws_url.clone(),
        30000, // max connections
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
        cancellation_token.clone(),
    ));

    // Start strategy manager on main thread (non-Send types can't cross thread boundaries)
    let cancellation_token_strategy = cancellation_token.clone();
    let strategy_manager_task = async move {
        info!("🚀 Starting Strategy Manager event processing loop");

        loop {
            tokio::select! {
                log_event = log_receiver.recv() => {
                    match log_event {
                        Ok(log_event) => {
                            info!("📥 Received log event from pool: {}", log_event.log_pool_address);
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
                        Err(_) => {
                            warn!("📡 Log receiver channel closed, stopping strategy manager");
                            break;
                        }
                    }
                }
                _ = cancellation_token_strategy.cancelled() => {
                    info!("Strategy manager shutdown requested");
                    break;
                }
            }
        }

        info!("Strategy manager task completed");
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

            // Trigger cancellation for all tasks
            cancellation_token.cancel();

            // Give tasks time to shutdown gracefully
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Abort any remaining tasks
            set.abort_all();

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            info!("✅ Shutdown complete");
        }
    }

    Ok(())
}
