use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::{
    network::Ethereum, primitives::U64, pubsub::PubSubFrontend, rpc::client::WsConnect,
    signers::local::PrivateKeySigner,
};
use anyhow::Result;
use arbooo::arbitrage::strategy::strategy;
use arbooo::common::logger;
use arbooo::common::logs;
use arbooo::common::pools;
use arbooo::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::EvmSimulator,
};
use dotenv::dotenv;
use dotenv::var;
use log::info;
use revm::primitives::Address;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv = dotenv()?;
    logger::setup_logger();
    info!("Logger setup");
    let ws_url = var::<&str>("WS_URL").unwrap();
    let ws_client = WsConnect::new(ws_url.clone());

    let provider = ProviderBuilder::new().on_ws(ws_client).await.unwrap();
    let provider = Arc::new(provider);

    if !Path::new("cache/.cached-pools.csv").try_exists()? {
        info!("Cache doesn't exist, crawling blocks for pools");
        pools::load_all_pools(ws_url, 100_000, 50_000)
            .await
            .unwrap();
    }

    let mut set = JoinSet::new();

    let (sender, _): (Sender<LogEvent>, _) = broadcast::channel(512);

    // 1. Get all pools

    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("~/cache/.cached-pools.csv");
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    // id,address,version,token0,oken1,fee,block_number,timestamp
    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line?;
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

    // 2. Listen for logs on pools
    set.spawn(logs::get_logs(provider.clone(), pools_map, sender.clone()));

    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));

    let provider: RootProvider<PubSubFrontend, Ethereum> = ProviderBuilder::new()
        .network()
        .on_ws(ws_client)
        .await
        .expect("Provider failed to build");

    let provider = Arc::new(provider);

    let latest_block_number = provider
        .get_block_number()
        .await
        .expect("Error getting block number");

    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    let simulator = EvmSimulator::new(
        provider.clone(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    );

    let simulator: Arc<TokioMutex<EvmSimulator<'_>>> = Arc::new(TokioMutex::new(simulator));

    info!("Spawning evm");

    strategy(sender, simulator.clone(), provider.clone())
        .await
        .unwrap();

    while let Some(res) = set.join_next().await {
        info!("{:?}", res);
    }

    Ok(())
}

// MVP What is left to do:
// [ ] Fix up all the decoding so that we can understand the errors
// [x] Create an Inspector
// [ ] Make it take profitable Arbitrages :shrug:
// [ ]
