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
                  //pub block_number: u64,
}

impl From<StringRecord> for Pool {
    fn from(record: StringRecord) -> Self {
        let version = match record.get(2).and_then(|v| v.parse().ok()).unwrap_or(2) {
            2 => DexVariant::UniswapV2,
            3 => DexVariant::UniswapV3,
            _ => DexVariant::UniswapV2, // Default to V2 for unknown versions
        };
        Self {
            id: record.get(0).and_then(|v| v.parse().ok()).unwrap_or(0),
            address: record
                .get(1)
                .and_then(|v| Address::from_str(v).ok())
                .unwrap_or_default(),
            version,
            token0: record
                .get(3)
                .and_then(|v| Address::from_str(v).ok())
                .unwrap_or_default(),
            token1: record
                .get(4)
                .and_then(|v| Address::from_str(v).ok())
                .unwrap_or_default(),
            fee: record.get(5).and_then(|v| v.parse().ok()).unwrap_or(3000),
            //block_number: record.get(6).and_then(|v| v.parse().ok()).unwrap_or(0),
        }
    }
}

impl Pool {
    pub fn cache_row(&self) -> (i64, String, i32, String, String, u32) {
        (
            self.id,
            format!("{:?}", self.address),
            self.version.num() as i32,
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.fee,
            //self.block_number,
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
    cache_path: &String,
) -> Result<(Vec<Pool>, i64)> {
    create_dir_all("cache")
        .map_err(|e| anyhow::anyhow!("Error creating cache directory: {}", e))?;
    info!("Creating cache file");

    let file_path = Path::new(cache_path);
    let file_exists = file_path.exists();
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(cache_path)
        .map_err(|error| anyhow::anyhow!("Error opening cache file '{}': {}", cache_path, error))?;
    let mut writer = csv::Writer::from_writer(file);

    let mut pools = Vec::new();

    let mut v2_pool_cnt = 0;

    if file_exists {
        let mut reader = csv::Reader::from_path(cache_path)?;

        for row in reader.records() {
            let row = row.map_err(|e| anyhow::anyhow!("Error reading CSV row: {}", e))?;
            let pool = Pool::from(row);
            if let DexVariant::UniswapV2 = pool.version {
                v2_pool_cnt += 1
            }
            pools.push(pool);
        }
    } else {
        log::debug!("Writing");
        writer.write_record([
            "id", "address", "version", "token0", "token1", "fee",
            //"block_number",
        ])?;
    }
    info!("Pools loaded: {:?}", pools.len());
    info!("V2 pools: {:?}", v2_pool_cnt);
    let ws_client = WsConnect::new(wss_url);
    let ws = ProviderBuilder::new().on_ws(ws_client).await?;
    let provider = Arc::new(ws);

    let mut id = if !pools.is_empty() {
        pools.last().map(|p| p.id).unwrap_or(-1)
    } else {
        -1
    };
    let last_id = id;

    //    let from_block = if id != -1 {
    //        pools.last().as_ref().unwrap().block_number + 1
    //    } else {
    //        from_block
    //    };

     let to_block = provider
         .get_block_number()
         .await
         .map_err(|e| anyhow::anyhow!("Failed to get latest block number: {}", e))?;

     info!("Current block number: {}", to_block);

     let mut blocks_processed = 0;

     let mut block_range = Vec::new();

     // For forked testnets, scan backwards from current block
     // Most pools will be in recent blocks on a fork
     let actual_from_block = if from_block > to_block {
         // If requested from_block is higher than current block (fork scenario)
         // Scan from current block backwards with chunk size, but at least scan recent blocks
         info!("⚠️ Requested start block {} is beyond current block {}. Scanning recent blocks instead.", from_block, to_block);
         if to_block > 100_000 {
             to_block.saturating_sub(100_000) // Scan last 100k blocks
         } else {
             0 // Scan from genesis on small forks
         }
     } else {
         from_block
     };

     info!("📡 Pool scanning range: {} to {}", actual_from_block, to_block);

     loop {
         let start_idx = actual_from_block + blocks_processed;
         let mut end_idx = start_idx + chunk - 1;
         if end_idx > to_block {
             end_idx = to_block;
             block_range.push((start_idx, end_idx));
             break;
         }
         block_range.push((start_idx, end_idx));
         blocks_processed += chunk;
     }
     log::debug!("Block range: {:?}", block_range);

    let pb = ProgressBar::new(block_range.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
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
                 match response {
                     Ok(pools_response) => {
                         pools.extend(pools_response);
                     }
                     Err(e) => {
                         log::error!("⚠️ Error fetching pools for range: {}", e);
                     }
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

     info!("🏁 Total pools discovered: {}", pools.len());
     log::debug!("amount of pools before liquidity test: {:?}", pools.len());

    //    let (evm, caller_address) = create_evm(provider.clone()).await;
    //let evm = Arc::new(tokio::sync::Mutex::new(evm));
    //let required_liquidity = BigInt::from_signed_bytes_be(&one_ether().to_be_bytes_vec());

    //#let mut filtered_pools: Vec<Pool> = vec![];
    let pb = ProgressBar::new(pools.len() as u64);

    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
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
     //pools.sort_by_key(|p| p.block_number);
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
     // Finalize the CSV writer - this flushes and closes the underlying file properly
     writer.flush()?;
     drop(writer);
     log::debug!("Added {:?} new pools to cache", added);
     info!("💾 Cache file persisted at: {}", cache_path);

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
        .address(vec![UNISWAP_V2_FACTORY])
        .event("PairCreated(address,address,address,uint256)");

    info!("🔍 Scanning V2 pools from block {} to {} on factory {}", from_block, to_block, UNISWAP_V2_FACTORY);
    let logs = provider.get_logs(&event_filter).await?;
    info!("📊 Found {} V2 PairCreated events", logs.len());

    for log in logs {
        //let block_number = log.block_number.unwrap_or_default();

        let topic0 = log.topics()[1];
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32])
            .map_err(|e| anyhow::anyhow!("Invalid topic0 format in V2 log: {}", e))?;
        let token0 = Address::from(topic0);

        let token1 = Address::from(
            FixedBytes::<20>::try_from(&log.topics()[2][12..32])
                .map_err(|e| anyhow::anyhow!("Invalid topic2 format in V2 log: {}", e))?,
        );
        let log_data = log.inner.data.data.to_vec();
        let log_data = log_data.as_slice();
        let decoded: (Address, B256) = SolValue::abi_decode(log_data, false)
            .map_err(|e| anyhow::anyhow!("Failed to decode V2 log data: {}", e))?;

        let pool_data = Pool {
            id: -1,
            address: decoded.0,
            version: DexVariant::UniswapV2,
            token0,
            token1,
             fee: 300,
             //block_number,
         };
         pools.push(pool_data);
     }

     info!("✅ Successfully loaded {} V2 pools", pools.len());
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
         .address(vec![UNISWAP_V3_FACTORY])
         .event("PoolCreated(address,address,uint24,int24,address)");

     info!("🔍 Scanning V3 pools from block {} to {} on factory {}", from_block, to_block, UNISWAP_V3_FACTORY);
     let logs = provider.get_logs(&event_filter).await?;
     info!("📊 Found {} V3 PoolCreated events", logs.len());
     for log in logs {
        if log.topics()[1].is_zero() {
            info!("V3 log 1 empty");
            continue;
        }
        //let block_number = log.block_number.unwrap_or_default();

        let topic0 = log.topics()[1];
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32])
            .map_err(|e| anyhow::anyhow!("Invalid topic0 format in V3 log: {}", e))?;
        let token0 = Address::from(topic0);

        let topic1 = log.topics()[2];
        let topic1 = FixedBytes::<20>::try_from(&topic1[12..32])
            .map_err(|e| anyhow::anyhow!("Invalid topic1 format in V3 log: {}", e))?;
        let token1 = Address::from(topic1);

        // Decode the log data - V3 PoolCreated event has (uint24 fee, int24 tickSpacing, address pool)
        let log_data = &log.inner.data.data;
        let decoded: (U256, i32, Address) = SolValue::abi_decode(log_data, false)
            .map_err(|e| anyhow::anyhow!("Failed to decode V3 log data: {}", e))?;

        let fee = decoded.0.to::<u32>(); // fee is uint24, can safely convert to u32
        let pool_address = decoded.2; // pool address is the third field

        // info!("is v3: {:?}", is_v3);
        let pool_data = Pool {
            id: -1,
            address: pool_address,
             version: DexVariant::UniswapV3,
             token0,
             token1,
             fee,
             //block_number,
         };
         pools.push(pool_data);
     }

     info!("✅ Successfully loaded {} V3 pools", pools.len());
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

/// Append a single dynamically discovered pool to the CSV cache
/// This function handles proper ID generation and maintains thread safety
pub async fn append_pool_to_cache(
    event: &crate::common::pairs::Event,
    cache_path: &str,
) -> Result<i64> {
    use tokio::sync::Mutex;
    use csv::Writer;
    
    // Thread-safe static for managing the next available ID
    static NEXT_ID: tokio::sync::OnceCell<Mutex<i64>> = tokio::sync::OnceCell::const_new();
    
    // Get or initialize the next available ID
    let pool_id = {
        let mutex = NEXT_ID.get_or_init(|| async {
            // Initialize by reading the last ID from the cache file
            let last_id = get_last_id_from_cache(cache_path).await.unwrap_or(-1);
            Mutex::new(last_id)
        }).await;
        
        let mut next_id = mutex.lock().await;
        *next_id += 1;
        *next_id
    };
    
    // Create the Pool struct from the Event
    let pool = Pool {
        id: pool_id,
        address: match event {
            crate::common::pairs::Event::PairCreated(pair) => pair.pair_address,
            crate::common::pairs::Event::PoolCreated(pool) => pool.pair_address,
        },
        version: match event {
            crate::common::pairs::Event::PairCreated(_) => DexVariant::UniswapV2,
            crate::common::pairs::Event::PoolCreated(_) => DexVariant::UniswapV3,
        },
        token0: match event {
            crate::common::pairs::Event::PairCreated(pair) => pair.token0,
            crate::common::pairs::Event::PoolCreated(pool) => pool.token0,
        },
        token1: match event {
            crate::common::pairs::Event::PairCreated(pair) => pair.token1,
            crate::common::pairs::Event::PoolCreated(pool) => pool.token1,
        },
        fee: match event {
            crate::common::pairs::Event::PairCreated(pair) => pair.fee,
            crate::common::pairs::Event::PoolCreated(pool) => pool.fee,
        },
    };
    
    // Ensure the cache directory exists
    if let Some(parent) = Path::new(cache_path).parent() {
        create_dir_all(parent)?;
    }
    
    // Check if file exists to determine if we need to write headers
    let file_exists = Path::new(cache_path).exists();
    
    // Open file in append mode
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(cache_path)?;
    
    let mut writer = Writer::from_writer(file);
    
    // Write headers if this is a new file
    if !file_exists {
        writer.write_record([
            "id", "address", "version", "token0", "token1", "fee",
        ])?;
    }
    
    // Serialize and write the pool
    writer.serialize(pool.cache_row())?;
    writer.flush()?;
    
    log::info!(
        "✅ Persisted discovered pool to cache: {} (id: {}, version: {}, tokens: {} -> {})",
        pool.address,
        pool.id,
        pool.version.num(),
        pool.token0,
        pool.token1
    );
    
    Ok(pool_id)
}

/// Helper function to read the last ID from the cache file
async fn get_last_id_from_cache(cache_path: &str) -> Result<i64> {
    if !Path::new(cache_path).exists() {
        return Ok(-1);
    }
    
    let mut reader = csv::Reader::from_path(cache_path)?;
    let mut last_id = -1;
    
    for result in reader.records() {
        if let Ok(record) = result {
            if let Some(id_str) = record.get(0) {
                if let Ok(id) = id_str.parse::<i64>() {
                    last_id = last_id.max(id);
                }
            }
        }
    }
    
    Ok(last_id)
}
