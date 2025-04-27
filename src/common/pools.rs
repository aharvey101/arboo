use std::{
    fs::{create_dir_all, OpenOptions},
    str::FromStr,
    sync::Arc,
};
use {
    ::log::info,
    alloy::{
        primitives::{Address, FixedBytes, B256, U256},
        providers::{Provider, ProviderBuilder, RootProvider},
        pubsub::PubSubFrontend,
        rpc::{client::WsConnect, types::eth::Filter},
    },
    alloy_sol_types::SolValue,
    anyhow::Result,
    csv::StringRecord,
    indicatif::{ProgressBar, ProgressStyle},
    serde::{Deserialize, Serialize},
    std::path::Path,
};
pub const UNISWAP_V2_FACTORY: Address = Address::new([
    0x5C, 0x69, 0xbE, 0xe7, 0x01, 0xef, 0x81, 0x4a, 0x2B, 0x6a, 0x3E, 0xDD, 0x4B, 0x16, 0x52, 0xCB,
    0x9c, 0xc5, 0xaA, 0x6f,
]);

pub const UNISWAP_V3_FACTORY: Address = Address::new([
    0x1F, 0x98, 0x43, 0x1c, 0x8a, 0xD9, 0x85, 0x23, 0x63, 0x1A, 0xE4, 0xa5, 0x9f, 0x26, 0x73, 0x46,
    0xea, 0x31, 0xF9, 0x84,
]);

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
        }
    }
}

impl Pool {
    pub fn cache_row(&self) -> (i64, String, i32, String, String, u32, u64) {
        (
            self.id,
            format!("{:?}", self.address),
            self.version.num() as i32,
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.fee,
            self.block_number,
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
    info!("Creating cache file");
    let cache_file = "~/cache/.cached-pools.csv";
    let file_path = Path::new(cache_file);
    let file_exists = file_path.exists();
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(cache_file)
        .unwrap();
    let mut writer = csv::Writer::from_writer(file);

    let mut pools = Vec::new();

    let mut v2_pool_cnt = 0;

    if file_exists {
        let mut reader = csv::Reader::from_path(cache_file)?;

        for row in reader.records() {
            let row = row.unwrap();
            let pool = Pool::from(row);
            if let DexVariant::UniswapV2 = pool.version {
                v2_pool_cnt += 1
            }
            pools.push(pool);
        }
    } else {
        info!("Writing");
        writer.write_record([
            "id",
            "address",
            "version",
            "token0",
            "token1",
            "fee",
            "block_number",
        ])?;
    }
    info!("Pools loaded: {:?}", pools.len());
    info!("V2 pools: {:?}", v2_pool_cnt);
    let ws_client = WsConnect::new(wss_url);
    let ws = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(ws);

    let mut id = if !pools.is_empty() {
        pools.last().as_ref().unwrap().id
    } else {
        -1
    };
    let last_id = id;

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
        let requests = vec![
            tokio::task::spawn(load_uniswap_v2_pools(provider.clone(), range.0, range.1)),
            tokio::task::spawn(load_uniswap_v3_pools(provider.clone(), range.0, range.1)),
        ];

        let results = futures::future::join_all(requests).await;
        results.into_iter().for_each(|result| {
            if let Ok(response) = result {
                if let Ok(pools_response) = response {
                    pools.extend(pools_response);
                }
            }
        });
        // now that we have all the pools, what we need to do is make sure they have atleast 5 eth of liquidity
        // to do this we need to setup an evm, get the storage for the contract,
        // Then query the balance of the contract for the token0 and token1
        // if either of them are less than 5 eth, we will skip the pool
        // if they are more than 5 eth, we will add the pool to the list

        pb.inc(1);
    }

    info!("amount of pools before liquidity test: {:?}", pools.len());

    //    let (evm, caller_address) = create_evm(provider.clone()).await;
    //let evm = Arc::new(tokio::sync::Mutex::new(evm));
    //let required_liquidity = BigInt::from_signed_bytes_be(&one_ether().to_be_bytes_vec());

    //#let mut filtered_pools: Vec<Pool> = vec![];
    let pb = ProgressBar::new(pools.len() as u64);

    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );
    //    for (_, pool) in pools.clone().iter_mut().enumerate() {
    //        let has_liquidity = liquidity_test(
    //            evm.clone(),
    //            &pool.address,
    //            required_liquidity.clone(),
    //            caller_address,
    //        )
    //        .await
    //        .unwrap_or_else(|e| false);
    //        if !has_liquidity {
    //            pb.inc(1);
    //            continue;
    //        }
    //        filtered_pools.push(pool.clone());
    //        pb.inc(1);
    //    }
    //
    //let mut pools = filtered_pools;
    //info!("amount of pools after liquidity test: {:?}", pools.len());
    let mut added = 0;
    pools.sort_by_key(|p| p.block_number);
    for pool in pools.iter_mut() {
        if pool.id == -1 {
            id += 1;
            pool.id = id;
        }
        if pool.id > last_id {
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

    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        //        .address(UNISWAP_V2_FACTORY)
        .event("PairCreated(address,address,address,uint256)");

    let logs = provider.get_logs(&event_filter).await?;

    for log in logs {
        let block_number = log.block_number.unwrap_or_default();

        let topic0 = log.topics()[1];
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32]).unwrap();
        let token0 = Address::from(topic0);

        let token1 = Address::from(FixedBytes::<20>::try_from(&log.topics()[2][12..32]).unwrap());
        let log_data = log.inner.data.data.to_vec();
        let log_data = log_data.as_slice();
        let decoded: (Address, B256) = SolValue::abi_decode(log_data, false).unwrap();

        let pool_data = Pool {
            id: -1,
            address: decoded.0,
            version: DexVariant::UniswapV2,
            token0,
            token1,
            fee: 300,
            block_number,
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

    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        //        .address(UNISWAP_V3_FACTORY)
        .event("PoolCreated(address,address,uint24,int24,address)");

    let logs = provider.get_logs(&event_filter).await?;
    for log in logs {
        if log.topics()[1].is_zero() {
            info!("V3 log 1 empty");
            continue;
        }
        let block_number = log.block_number.unwrap_or_default();

        let topic0 = log.topics()[1];
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32]).unwrap();
        let token0 = Address::from(topic0);

        let topic1 = log.topics()[2];
        let topic1 = FixedBytes::<20>::try_from(&topic1[12..32]).unwrap();
        let token1 = Address::from(topic1);

        // Decode the log data
        let log_data = &log.inner.data.data;
        let decoded: (B256, B256) = SolValue::abi_decode(log_data, false).unwrap();
        let pool_address = decoded.1;
        let pool_address = FixedBytes::<20>::try_from(&pool_address[12..32]).unwrap();
        let pool_address = Address::from(pool_address);
        let fee = u32::from_str_radix(decoded.0.to_string().as_str().trim_start_matches("0x"), 16)
            .unwrap();

        // info!("is v3: {:?}", is_v3);
        let pool_data = Pool {
            id: -1,
            address: pool_address,
            version: DexVariant::UniswapV3,
            token0,
            token1,
            fee,
            block_number,
        };
        pools.push(pool_data);
    }

    Ok(pools)
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
