use alloy::providers::ProviderBuilder;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use arbooo::arbitrage::simulation::simulation;
use dotenv::dotenv;
use dotenv::var;

use revm::primitives::{Address, U256};
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let ws_url = var::<&str>("WS_URL").unwrap();
    let http_url = var::<&str>("HTTP_URL").unwrap();
    let http_url = http_url.as_str();
    let http_url = url::Url::from_str(http_url).unwrap();
    let ws_client = WsConnect::new(ws_url.clone());

    let provider = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(provider);
    // strategy(provider).await?;
    simulation(Address::default(), Address::default()).await?;
    // let (pool, other) = load_all_pools(ws_url, 10_000_000, 50000).await.unwrap();
    // simple_test(http_url).await?;
    // 2 - Scan for price changes?

    // to build a arbitrage bot need to do the following:
    // 1. create an arb bot contract
    // 2. create code that scans for price changes, and simulates transactions

    //      - to test this we could start off by creating a simulation of usdt / usdc swaps
    // 3.
    // 4.

    Ok(())
}
