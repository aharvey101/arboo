use std::sync::Arc;

use anyhow::Result;
use dotenv::dotenv;
use dotenv::var;
use ethers::providers::{Provider, Ws};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    println!("Hello, world!");

    let ws_url = var::<&str>("WS_URL").unwrap();

    let ws_client = Ws::connect(ws_url).await?;

    let provider = Arc::new(Provider::new(ws_client));

    // to build a arbitrage bot need to do the following:
    // 1. create an arb bot contract that uses other contract but inserts it's own swap function
    // 2. create code that scans for price changes, and simulates transactions
    //      - to test this we could start off by creating a simulation of usdt / usdc swaps
    // 3.
    // 4.

    Ok(())
}
