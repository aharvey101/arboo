use alloy::eips::BlockId;
use alloy::hex;
use alloy::primitives::Address;
use alloy::primitives::B256;
use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::eth::{Block, Filter};
use alloy_json_abi::Event;
use alloy_sol_types::abi;
use alloy_sol_types::SolValue;
use anyhow::Result;
use csv::StringRecord;
use home::home_dir;
use indicatif::{ProgressBar, ProgressStyle};
use sha3::{Digest, Keccak256};

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DexVariant {
    UniswapV2, // 2
}

impl DexVariant {
    pub fn num(&self) -> u8 {
        match self {
            DexVariant::UniswapV2 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pool {
    pub id: i64,
    pub address: Address,
    pub version: DexVariant,
    pub token0: Address,
    pub token1: Address,
    pub fee: u32, // uniswap v3 specific
    pub block_number: u64,
    pub timestamp: u64,
}

impl From<StringRecord> for Pool {
    fn from(record: StringRecord) -> Self {
        let version = match record.get(2).unwrap().parse().unwrap() {
            2 => DexVariant::UniswapV2,
            _ => DexVariant::UniswapV2,
        };
        Self {
            id: record.get(0).unwrap().parse().unwrap(),
            address: Address::from_str(record.get(1).unwrap()).unwrap(),
            version,
            token0: Address::from_str(record.get(3).unwrap()).unwrap(),
            token1: Address::from_str(record.get(4).unwrap()).unwrap(),
            fee: record.get(5).unwrap().parse().unwrap(),
            block_number: record.get(6).unwrap().parse().unwrap(),
            timestamp: record.get(7).unwrap().parse().unwrap(),
        }
    }
}

impl Pool {
    pub fn cache_row(&self) -> (i64, String, i32, String, String, u32, u64, u64) {
        (
            self.id,
            format!("{:?}", self.address),
            self.version.num() as i32,
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.fee,
            self.block_number,
            self.timestamp,
        )
    }

    pub fn trades(&self, token_a: Address, token_b: Address) -> bool {
        let is_zero_for_one = self.token0 == token_a && self.token1 == token_b;
        let is_one_for_zero = self.token1 == token_a && self.token0 == token_b;
        is_zero_for_one || is_one_for_zero
    }

    pub fn pretty_msg(&self) -> String {
        format!(
            "[{:?}] {:?}: {:?} --> {:?}",
            self.version, self.address, self.token0, self.token1
        )
    }

    pub fn pretty_print(&self) {
        println!("{}", self.pretty_msg());
    }
}

pub async fn get_touched_pools(
    provider: &Arc<RootProvider<PubSubFrontend>>,
    block_number: u64,
) -> Result<Vec<Address>> {
    let v2_swap_event = "Swap(address,uint256,uint256,uint256,uint256,address)";
    let event_filter = Filter::new()
        .from_block(block_number)
        .to_block(block_number)
        .events(vec![v2_swap_event]);
    let logs = provider.get_logs(&event_filter).await?;
    let touched_pools: Vec<Address> = logs.iter().map(|log| log.address()).collect();
    Ok(touched_pools)
}

pub async fn load_all_pools(
    wss_url: String,
    from_block: u64,
    chunk: u64,
) -> Result<(Vec<Pool>, i64)> {
    match create_dir_all("cache") {
        _ => {
            println!("Created directory")
        }
    }

    let cache_file = "cache/.cached-pools.csv";
    let file_path = Path::new(cache_file);
    let file_exists = file_path.exists();
    let file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(&cache_file)
        .unwrap();
    let mut writer = csv::Writer::from_writer(file);

    let mut pools = Vec::new();

    let mut v2_pool_cnt = 0;

    if file_exists {
        let mut reader = csv::Reader::from_path(cache_file)?;

        for row in reader.records() {
            let row = row.unwrap();
            let pool = Pool::from(row);
            match pool.version {
                DexVariant::UniswapV2 => v2_pool_cnt += 1,
            }
            pools.push(pool);
        }
    } else {
        println!("Writing");
        writer.write_record(&[
            "id",
            "address",
            "version",
            "token0",
            "token1",
            "fee",
            "block_number",
            "timestamp",
        ])?;
    }
    println!("Pools loaded: {:?}", pools.len());
    println!("V2 pools: {:?}", v2_pool_cnt);
    let ws_client = WsConnect::new(wss_url);
    let ws = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(ws);

    // Uniswap V2
    let pair_created_event = "event PairCreated(address,address,address,uint256)";

    let mut hasher = Keccak256::default();
    hasher.update(pair_created_event.to_string().as_bytes());
    println!("hasher: {:?}", hasher.clone());
    let result = hasher.finalize();
    let hash_bytes: [u8; 32] = result.as_slice().try_into()?;
    let hasher = B256::from(hash_bytes);

    let mut id = if pools.len() > 0 {
        pools.last().as_ref().unwrap().id as i64
    } else {
        -1
    };
    let last_id = id as i64;

    let from_block = if id != -1 {
        pools.last().as_ref().unwrap().block_number + 1
    } else {
        from_block
    };
    let to_block = provider.get_block_number().await.unwrap();
    let mut blocks_processed = 0;

    let mut block_range = Vec::new();

    loop {
        let start_idx = from_block + blocks_processed;
        let mut end_idx = start_idx + chunk - 1;
        if end_idx > to_block {
            end_idx = to_block;
            block_range.push((start_idx, end_idx));
            break;
        }
        block_range.push((start_idx, end_idx));
        blocks_processed += chunk;
    }
    println!("Block range: {:?}", block_range);

    let pb = ProgressBar::new(block_range.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    for range in block_range {
        let mut requests = Vec::new();
        requests.push(tokio::task::spawn(load_uniswap_v2_pools(
            provider.clone(),
            range.0,
            range.1,
            pair_created_event,
            hasher,
        )));
        let results = futures::future::join_all(requests).await;
        for result in results {
            match result {
                Ok(response) => match response {
                    Ok(pools_response) => {
                        pools.extend(pools_response);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        pb.inc(1);
    }

    let mut added = 0;
    pools.sort_by_key(|p| p.block_number);
    for pool in pools.iter_mut() {
        if pool.id == -1 {
            id += 1;
            pool.id = id;
        }
        if (pool.id as i64) > last_id {
            writer.serialize(pool.cache_row())?;
            added += 1;
        }
    }
    writer.flush()?;
    println!("Added {:?} new pools", added);

    Ok((pools, last_id))
}

pub async fn load_uniswap_v2_pools(
    provider: Arc<RootProvider<PubSubFrontend>>,
    from_block: u64,
    to_block: u64,
    event: &str,
    signature: B256,
) -> Result<Vec<Pool>> {
    println!("Loading v2 pool");
    let mut pools = Vec::new();
    let mut timestamp_map = HashMap::new();

    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        .event("PairCreated(address,address,address,uint256)");
    println!("Event Filter: {:?}", event_filter);
    let logs = provider.get_logs(&event_filter).await?;
    println!("Logs? {:?}", logs.clone());
    println!("signature: {:?}", signature);
    for log in logs {
        let topic = log.topics()[0];
        let block_number = log.block_number.unwrap_or_default();
        println!("Topic:  {:?}", topic);
        if topic != signature {
            continue;
        }
        println!("Block: {:?}", block_number);
        println!(
            "Block as BlockId : {:?}",
            BlockId::from(block_number.clone())
        );
        let timestamp = if !timestamp_map.contains_key(&block_number) {
            let block = provider
                .get_block(BlockId::from(block_number), false)
                .await
                .unwrap()
                .unwrap();
            let timestamp = block.header.timestamp;
            timestamp_map.insert(block_number, timestamp);
            timestamp
        } else {
            let timestamp = *timestamp_map.get(&block_number).unwrap();
            timestamp
        };
        let token0 = Address::from_str(log.topics()[1].to_string().as_str()).unwrap();
        let token1 = Address::from_str(log.topics()[2].to_string().as_str()).unwrap();
        println!("{:?}, {:?}", token0, token1);
        // let token0_as_Address: [u8; 20] ;

        let log_data = log.inner.data.data.to_vec();
        let log_data = log_data.as_slice();

        // let log_data = log.data().to_owned().to_vec().as_slice();
        println!("Decoding");
        let decoded: (Address, B256) = SolValue::abi_decode(log_data, false).unwrap();
        let pool_data = Pool {
            id: -1,
            address: decoded.0,
            version: DexVariant::UniswapV2,
            token0,
            token1,
            fee: 300,
            block_number,
            timestamp,
        };
        pools.push(pool_data);
        println!("decoded: {:?}", decoded);
    }

    Ok(pools)
}
