use alloy::providers::ProviderBuilder;
use alloy::rpc::client::WsConnect;
use anyhow::Result;
use arbooo::arbitrage::strategy::strategy;
use arbooo::common::logs;
use arbooo::common::logs::LogEvent;
use arbooo::common::pairs::Event;
use arbooo::common::pairs::V2PoolCreated;
use arbooo::common::pairs::V3PoolCreated;
use arbooo::common::pools;
use dotenv::dotenv;
use dotenv::var;
use revm::primitives::{Address, U256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;

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

    if !Path::new("cache/.cached-pools.csv").try_exists()? {
        pools::load_all_pools(ws_url, 10_000_000, 50_000)
            .await
            .unwrap();
    }
    
    let mut set = JoinSet::new();

    let (sender, _): (Sender<LogEvent>, _) = broadcast::channel(512);

    set.spawn(strategy(provider.clone(), sender.clone()));

    // 1. Get all pools

    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path)?;
    let reader = io::BufReader::new(file);
    // id,address,version,token0,token1,fee,block_number,timestamp
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
    set.spawn(logs::get_logs(provider.clone(), pools_map, sender));

    // 3. If a log has a pool in a hashmap, it could be a buy or sell on that pool
    // 4. do corresponding simulations (if buy, then price increased, so check if sim creates profit by selling on other pool and vice versa)
    // 5.

    while let Some(res) = set.join_next().await {
        println!("{:?}", res);
    }

    Ok(())
}
