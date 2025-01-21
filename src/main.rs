use alloy::providers::ProviderBuilder;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use arbooo::arbitrage::liquidity;
use arbooo::arbitrage::simple_swap_sim::simple_swap_simulation;
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

        let provider = ProviderBuilder::new().on_ws(ws_client).await.unwrap();
    let provider = Arc::new(provider);
    // strategy(provider).await?;
    // simulation(Address::default(), Address::default()).await?;
    // liquidity::liquidity().await?;
    simulation().await?;
    // simple_swap_simulation().await?;

    // 2 - Scan for price changes?

    // to build a arbitrage bot need to do the following:
    // 1. create an arb bot contract
    // 2. create code that scans for price changes, and simulates transactions

    //      - to test this we could start off by creating a simulation of usdt / usdc swaps
    // 3.
    // 4.

    Ok(())
}
