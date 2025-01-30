use crate::common::pairs::Event;
use crate::common::pairs::V2PoolCreated;
use crate::common::pairs::V3PoolCreated;
use crate::common::revm::{EvmSimulator, Tx};
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::primitives::U64;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use revm::primitives::{address, Address, Bytecode, U256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex as TokioMutex;

pub async fn threaded_evm(
    sender: Sender<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider: RootProvider<PubSubFrontend, AnyNetwork> = ProviderBuilder::new()
        .network()
        .on_ws(ws_client)
        .await
        .expect("Provider failed to build");
    let provider = Arc::new(provider);

    let latest_block_number = provider
        .get_block_number()
        .await
        .expect("Error getting block number");
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
        .await
        .expect("Error getting latest block")
        .expect("Expected block");

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    let simulator = EvmSimulator::new(
        provider.clone(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    );

    let simulator = Arc::new(TokioMutex::new(simulator));

    // Laod all pools:

    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path).expect("Error getting File");
    let reader = io::BufReader::new(file);
    // id,address,version,token0,token1,fee,block_number,timestamp
    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line.expect("Expected Line");
        let fields: Vec<&str> = line.split(',').collect();
        match fields[2] {
            "2" => {
                let pair_address = Address::from_str(fields[1]).unwrap();
                pools_map.insert(
                    pair_address,
                    Event::PairCreated(V2PoolCreated {
                        pair_address: Address::from_str(fields[1]).unwrap(),
                        token0: Address::from_str(fields[3]).unwrap(),
                        token1: Address::from_str(fields[4]).unwrap(),
                        fee: fields[5].parse::<u32>().unwrap(),
                        block_number: fields[6].parse::<u64>().unwrap(),
                    }),
                );
            }
            "3" => {
                let pair_address = Address::from_str(fields[1]).unwrap();
                pools_map.insert(
                    pair_address,
                    Event::PoolCreated(V3PoolCreated {
                        pair_address: Address::from_str(fields[1]).unwrap(),
                        token0: Address::from_str(fields[3]).unwrap(),
                        token1: Address::from_str(fields[4]).unwrap(),
                        fee: fields[5].parse::<u32>().unwrap(),
                        tick_spacing: 0i32,
                    }),
                );
            }
            &_ => continue,
        };
    }
    // we want to load all of the contracts into memory
    println!("Loading pools");
    for (addr, event) in pools_map.iter(){
        println!("Loading pool, {:?}, number:", addr);
        let mut sim = simulator.lock().await;
        sim.load_account(*addr).await;
        sim.load_pool_state(*addr).await.expect("Failed to load basic state");
        if let Event::PairCreated(V2PoolCreated { .. }) = event {
            sim.load_v2_pool_state(*addr).await.expect("Failed to load v2 pool state");
        } else {
            sim.load_v3_pool_state(*addr).await.expect("Failed to load v3 state");
        }
    }

    let mut event_reciever = sender.subscribe();
    loop {
        match event_reciever.recv().await {
            Ok(message) => {

            }
            Err(err) => {
                println!("OOP")
            }
        }
    }
}

pub fn start_evm_thread(sender: Sender<()>) {
    tokio::spawn(threaded_evm(sender));
}
