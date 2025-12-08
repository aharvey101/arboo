use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::eth::Filter;
use anyhow::Result;

#[tokio::test]
#[ignore]
async fn test_pool_creation_events_detection() -> Result<()> {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();

    let ws_url = std::env::var("WS_URL").expect("WS_URL not set");
    println!("📡 Connecting to: {}", ws_url);

    let ws_client = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await?;

    let latest_block = provider.get_block_number().await?;
    println!("📍 Latest block: {}", latest_block);

    // Test 1: Try to find V2 PairCreated events in recent blocks
    println!("\n🔍 TEST 1: Searching for V2 PairCreated events in last 1000 blocks...");
    let v2_filter = Filter::new()
        .from_block(latest_block.saturating_sub(1000))
        .to_block(latest_block)
        .event("PairCreated(address,address,address,uint256)");

    let v2_logs = provider.get_logs(&v2_filter).await?;
    println!("✅ Found {} V2 PairCreated events", v2_logs.len());

    // Test 2: Try to find V3 PoolCreated events in recent blocks
    println!("\n🔍 TEST 2: Searching for V3 PoolCreated events in last 1000 blocks...");
    let v3_filter = Filter::new()
        .from_block(latest_block.saturating_sub(1000))
        .to_block(latest_block)
        .event("PoolCreated(address,address,uint24,int24,address)");

    let v3_logs = provider.get_logs(&v3_filter).await?;
    println!("✅ Found {} V3 PoolCreated events", v3_logs.len());

    // Test 3: Try from genesis but with smaller chunk (first 10k blocks)
    println!("\n🔍 TEST 3: Searching for V2 PairCreated events in first 10000 blocks (genesis)...");
    let genesis_v2_filter = Filter::new()
        .from_block(0)
        .to_block(9999)
        .event("PairCreated(address,address,address,uint256)");

    let genesis_v2_logs = provider.get_logs(&genesis_v2_filter).await?;
    println!("✅ Found {} V2 PairCreated events from genesis", genesis_v2_logs.len());

    Ok(())
}
