use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::Filter;
use alloy_primitives::keccak256;
use anyhow::Result;
use futures::StreamExt;
use hex;
use log::info;
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let ws_url = env::var("WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    
    info!("🔍 Log Listener Diagnostic Tool");
    info!("📡 Connecting to WebSocket: {}", ws_url);
    
    // Connect to WebSocket provider
    let ws_client = WsConnect::new(&ws_url);
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;

    info!("✅ Connected to provider");

    // Get current block
    let current_block = provider.get_block_number().await?;
    info!("📦 Current block: {}", current_block);

    // Create V2 and V3 Swap event signatures
    let v2_swap_signature =
        keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_signature =
        keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)".as_bytes());

    info!("🔍 V2 Swap signature: 0x{}", hex::encode(v2_swap_signature));
    info!("🔍 V3 Swap signature: 0x{}", hex::encode(v3_swap_signature));

    // Create filter for Swap events from current block
    let filter = Filter::new()
        .event_signature(vec![v2_swap_signature, v3_swap_signature])
        .from_block(alloy::eips::BlockNumberOrTag::Number(current_block));

    info!("📋 Creating filter for Swap events from block: {}", current_block);

    // Subscribe to logs
    info!("🔗 Subscribing to logs...");
    let subscription = provider.subscribe_logs(&filter).await
        .map_err(|e| anyhow::anyhow!("Failed to subscribe to logs: {}", e))?;

    let mut stream = subscription.into_stream();
    let start_time = Instant::now();
    let mut log_count = 0u64;

    info!("✅ Subscription established! Listening for logs...");
    info!("⏳ Press Ctrl+C to stop\n");

    loop {
        tokio::select! {
            Some(log) = stream.next() => {
                log_count += 1;
                let elapsed = start_time.elapsed().as_secs_f64();
                
                info!(
                    "📥 LOG #{} [{}s] - Block: {:?}, Address: {:?}",
                    log_count,
                    elapsed as u64,
                    log.block_number,
                    log.address()
                );
                
                // Print log details
                info!("   Topics: {}", log.topics().len());
                for (i, topic) in log.topics().iter().enumerate() {
                    info!("     Topic[{}]: 0x{}", i, hex::encode(topic));
                }
                info!("   Data: {} bytes", log.data().data.len());
                info!("   Transaction: {:?}", log.transaction_hash);
                info!("");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("\n🛑 Shutting down...");
                info!("📊 Total logs received: {}", log_count);
                info!("⏱️  Runtime: {:.2}s", start_time.elapsed().as_secs_f64());
                break;
            }
        }
    }

    Ok(())
}
