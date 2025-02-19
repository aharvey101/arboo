use {
    ::log::info,
    alloy::{
        eips::BlockId,
        primitives::{Address, FixedBytes, B256, U256},
        providers::{Provider, ProviderBuilder, RootProvider},
        pubsub::PubSubFrontend,
        rpc::{
            client::WsConnect,
            types::{eth::Filter, BlockTransactionsKind},
        },
    },
    alloy_sol_types::SolValue,
    anyhow::Result,
    csv::StringRecord,
    indicatif::{ProgressBar, ProgressStyle},
    serde::{Deserialize, Serialize},
    std::path::Path,
};

use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DexVariant {
    UniswapV2, // 2
    UniswapV3,
}
impl DexVariant {
    pub fn num(&self) -> u8 {
        match self {
            DexVariant::UniswapV2 => 2,
            DexVariant::UniswapV3 => 3,
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
        info!("{}", self.pretty_msg());
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
    create_dir_all("cache").expect("Error creating directory");

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
                _ => {}
            }
            pools.push(pool);
        }
    } else {
        info!("Writing");
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
    info!("Pools loaded: {:?}", pools.len());
    info!("V2 pools: {:?}", v2_pool_cnt);
    let ws_client = WsConnect::new(wss_url);
    let ws = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(ws);

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
    info!("Block range: {:?}", block_range);

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
            range.0.clone(),
            range.1.clone(),
        )));
        requests.push(tokio::task::spawn(load_uniswap_v3_pools(
            provider.clone(),
            range.0.clone(),
            range.1.clone(),
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
    info!("Added {:?} new pools", added);

    Ok((pools, last_id))
}

pub async fn load_uniswap_v2_pools(
    provider: Arc<RootProvider<PubSubFrontend>>,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Pool>> {
    let mut pools = Vec::new();
    let mut timestamp_map = HashMap::new();

    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        .event("PairCreated(address,address,address,uint256)");

    let logs = provider.get_logs(&event_filter).await?;

    for log in logs {
        let block_number = log.block_number.unwrap_or_default();

        let timestamp = if !timestamp_map.contains_key(&block_number) {
            let block = provider
                .get_block(BlockId::from(block_number), BlockTransactionsKind::Full)
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

        let topic0 = FixedBytes::from(log.topics()[1]);
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32]).unwrap();
        let token0 = Address::from(topic0);

        let token1 = Address::from(
            FixedBytes::<20>::try_from(&FixedBytes::from(log.topics()[2])[12..32]).unwrap(),
        );
        let log_data = log.inner.data.data.to_vec();
        let log_data = log_data.as_slice();
        let decoded: (Address, B256) = SolValue::abi_decode(log_data, false).unwrap();

        // if !is_v2_pool(decoded.0, provider.clone())
        //     .await
        //     .unwrap_or(false)
        // {
        //     continue;
        // }

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
    }

    Ok(pools)
}

pub async fn load_uniswap_v3_pools(
    provider: Arc<RootProvider<PubSubFrontend>>,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Pool>> {
    let mut pools = Vec::new();
    let mut timestamp_map = HashMap::new();

    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        .event("PoolCreated(address,address,uint24,int24,address)");

    let logs = provider.get_logs(&event_filter).await?;
    for log in logs {
        if log.topics()[1].is_zero() {
            info!("V3 log 1 empty");
            continue;
        }
        let block_number = log.block_number.unwrap_or_default();

        let timestamp = if !timestamp_map.contains_key(&block_number) {
            let block = provider
                .get_block(BlockId::from(block_number), BlockTransactionsKind::Full)
                .await?
                .unwrap();
            let timestamp = block.header.timestamp;
            timestamp_map.insert(block_number, timestamp);
            timestamp
        } else {
            *timestamp_map.get(&block_number).unwrap()
        };

        let topic0 = FixedBytes::from(log.topics()[1]);
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32]).unwrap();
        let token0 = Address::from(topic0);

        let topic1 = FixedBytes::from(log.topics()[2]);
        let topic1 = FixedBytes::<20>::try_from(&topic1[12..32]).unwrap();
        let token1 = Address::from(topic1);

        // Decode the log data
        let log_data = &log.inner.data.data;
        let decoded: (B256, B256) = SolValue::abi_decode(log_data, false).unwrap();
        let pool_address = FixedBytes::<32>::try_from(decoded.1).unwrap();
        let pool_address = FixedBytes::<20>::try_from(&pool_address[12..32]).unwrap();
        let pool_address = Address::from(pool_address);
        let fee = u32::from_str_radix(decoded.0.to_string().as_str().trim_start_matches("0x"), 16)
            .unwrap();

        // lets check how much liquidity is in the pool, if its less than $1000 then lets ignore it

        // let is_v3 = is_v3_pool(pool_address, &provider)
        //     .await
        //     .unwrap_or(false);
        // if !is_v3 {
        //     continue;
        // }

        // info!("is v3: {:?}", is_v3);
        let pool_data = Pool {
            id: -1,
            address: pool_address,
            version: DexVariant::UniswapV3,
            token0,
            token1,
            fee,
            block_number,
            timestamp,
        };
        pools.push(pool_data);
    }

    Ok(pools)
}

fn weth_address() -> String {
    String::from("0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
}
alloy::sol! {
    interface IV3Pool {
        function liquidity() external view returns (uint128);
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
    }
}

pub struct PoolLiquidity {
    pub liquidity: U256,
    pub sqrt_price_x96: U256,
    pub tick: i32,
}

// Check if the contract that emitted the log is a Uniswap V2 pool
async fn is_v2_pool(address: Address, provider: Arc<RootProvider<PubSubFrontend>>) -> Result<bool> {
    // Get the contract bytecode
    let code = provider
        .get_code_at(address)
        .await
        .unwrap_or(Default::default())
        .to_string();

    // You can compare against known V2 pool creation code hash
    // This is the init code hash for Uniswap V2 pairs
    let v2_init_code_hash = "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f";

    // Or check specific bytecode patterns unique to V2 pools
    let is_v2 = code.contains(v2_init_code_hash);

    Ok(is_v2)
}

async fn is_v3_pool(
    address: Address,
    provider: &Arc<RootProvider<PubSubFrontend>>,
) -> Result<bool> {
    let code = provider.get_code_at(address).await.unwrap().to_string();

    let v3_init_code_hash = "f5e0d0f3e";
    let is_v3 = code.contains(v3_init_code_hash);
    Ok(is_v3)
}
