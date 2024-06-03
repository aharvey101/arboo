use std::sync::Arc;

use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use dotenv::dotenv;
use dotenv::var;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let ws_url = var::<&str>("WS_URL").unwrap();

    let ws_client = WsConnect::new(ws_url);

    let provider = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(provider);

    arboo_strategy(provider).await?;
    // 2 - Scan for price changes?

    // to build a arbitrage bot need to do the following:
    // 1. create an arb bot contract that uses other contract but inserts it's own swap function
    // 2. create code that scans for price changes, and simulates transactions
    //      - to test this we could start off by creating a simulation of usdt / usdc swaps
    // 3.
    // 4.

    Ok(())
}

async fn arboo_strategy(provider: Arc<RootProvider<PubSubFrontend>>) -> Result<()> {
    // Each block could have v2 or v3 contract transaction,
    // this denotes a change in price and we can see if there is an arb opp

    let sub = provider.subscribe_blocks().await?;

    let mut stream = sub.into_stream().take(4);

    let handle = tokio::spawn(async move {
        while let Some(block) = stream.next().await {
            println!(
                "Latest block number: {}",
                block.header.number.expect("Failed to get block number")
            );
        }
    });

    handle.await?;

    Ok(())
}
