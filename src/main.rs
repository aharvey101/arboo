use std::str::FromStr;
use std::sync::Arc;

use alloy::primitives::TxHash;
use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::eth::Block;
use anyhow::Result;
use arbooo::common::load_all_pools;
use dotenv::dotenv;
use dotenv::var;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;

    let ws_url = var::<&str>("WS_URL").unwrap();

    let ws_client = WsConnect::new(ws_url.clone());

    let provider = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(provider);
    load_all_pools(ws_url, 10000000, 50000).await.unwrap();
    // arboo_strategy(provider).await?;
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

    // get all v2 /v3 pools and write to a file?

    let sub = provider.subscribe_blocks().await?;

    let mut stream = sub.into_stream();

    // do pools?
    while let Some(block) = stream.next().await {
        // so we are streaming blocks
        //
        // lets build a function to see what is in the block
        get_pool_tx(block.clone(), provider.clone()).await?;
        println!(
            "Latest block number: {}",
            block.header.number.expect("Failed to get block number")
        );
    }

    Ok(())
}

// function that tests to see if there was any uniswap v2/uniswap v3 transactions in the block (there will be)
async fn get_pool_tx(block: Block, provider: Arc<RootProvider<PubSubFrontend>>) -> Result<()> {
    // look into block, see what transactions are pool transactions
    // Transactions that are TO uniswap v2/v3 contracts are what we want
    // println!("Block: {:?}", block);
    // get transactions in block?
    let full_block = provider
        .get_block_by_hash(block.header.hash.unwrap(), true)
        .await?
        .unwrap();

    let tx_hashes = full_block.transactions.hashes();

    // for hash in tx_hashes {
    //     // lookup transaction and push into vec??
    //     let tx = provider
    //         .get_transaction_by_hash(hash.to_owned())
    //         .await?
    //         .expect("couldn't get tx");
    //     println!("tx {:?}", tx);
    // }
    let my_hash = "0x24eedb1b5be1cbfeaac50ecaafc7ebe3e93aacfb33521e74ad36d5f9b87ef9c6";
    let hash = TxHash::from_str(my_hash).unwrap();
    let my_tx = provider.get_transaction_by_hash(hash).await?.unwrap();

    println!("MY TX : {:?}  ", my_tx);

    Ok(())
}

// Transaction { hash: 0xfb460dc4f9c1a062e945de66b8f06f3dbeb0c4b1d1c7b9df4c356a5b69a46dc0, nonce: 37317, block_hash: Some(0x84f547ca514c699919e4f0c1d2d4f49ffdc6b460cc7461850615fa6b878c5f01), block_number: Some(20011013), transaction_index: Some(120), from: 0xa83114a443da1cecefc50368531cace9f37fcccb, to: Some(0x388c818ca8b9251b393131c08a736a67ccb19297), value: 0x000000000000000000000000000000000000000000000000017558c72340eb6d_U256, gas_price: Some(10970819645), gas: 22111, max_fee_per_gas: Some(10970819645), max_priority_fee_per_gas: Some(0), max_fee_per_blob_gas: None, input: 0x, signature: Some(Signature { r: 0xef0927fc8e833ed3fcf3bd3c0be36b4ddeb06da9e486bc9fad0b2925095f683f_U256, s: 0x10f4feaf0b502703f235463075fbf7952ab7af22b79d725036c553af19ab893e_U256, v: 0x0000000000000000000000000000000000000000000000000000000000000001_U256, y_parity: Some(Parity(true)) }), chain_id: Some(1), blob_versioned_hashes: None, access_list: Some(AccessList([])), transaction_type: Some(2), other: OtherFields {} }
//
// struct Transaction {
//     hash: H264,
//     block_hash: H264,
//     from: H264,
//     to: H264,
// }
