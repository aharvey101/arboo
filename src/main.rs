use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Hello, world!");
    // to build a arbitrage bot need to do the follwing:
    // 1. create an arb bot contract that uses other contract but inserts it's own swap function
    // 2. create code that scans for price changes, and simulates transactions
    //      - to test this we could start off by creating a simulation of usdt / usdc swaps
    // 3.
    // 4.
    Ok(())
}
