use crate::{
    arbitrage::simulation::{one_ether, one_hundred_ether, one_thousand_eth},
    common::revm::EvmSimulator,
};
use alloy::network::Ethereum;
use alloy_sol_types::SolCall;
use futures::StreamExt;
use num_bigint::BigInt;
use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    str::FromStr,
    sync::Arc,
};
use {
    ::log::info,
    alloy::{
        eips::BlockId,
        primitives::{Address, FixedBytes, B256, U256, U64},
        providers::{Provider, ProviderBuilder, RootProvider},
        pubsub::PubSubFrontend,
        rpc::{
            client::WsConnect,
            types::{eth::Filter, BlockTransactionsKind},
        },
        signers::local::PrivateKeySigner,
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
    let cache_file = "cache/.cached-pools.csv";
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
        let mut requests = Vec::new();
        requests.push(tokio::task::spawn(load_uniswap_v2_pools(
            provider.clone(),
            range.0,
            range.1,
        )));
        requests.push(tokio::task::spawn(load_uniswap_v3_pools(
            provider.clone(),
            range.0,
            range.1,
        )));

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

    let (evm, caller_address) = create_evm(provider.clone()).await;
    let evm = Arc::new(tokio::sync::Mutex::new(evm));
    let required_liquidity = BigInt::from_signed_bytes_be(&one_ether().to_be_bytes_vec());

    let mut filtered_pools: Vec<Pool> = vec![];
    let pb = ProgressBar::new(pools.len() as u64);

    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );
    for (_, pool) in pools.clone().iter_mut().enumerate() {
        let has_liquidity = liquidity_test(
            evm.clone(),
            &pool.address,
            required_liquidity.clone(),
            caller_address,
        )
        .await
        .unwrap_or_else(|e| false);
        if !has_liquidity {
            pb.inc(1);
            continue;
        }
        filtered_pools.push(pool.clone());
        pb.inc(1);
    }

    let mut pools = filtered_pools;
    info!("amount of pools after liquidity test: {:?}", pools.len());
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
        .address(UNISWAP_V2_FACTORY)
        .event("PairCreated(address,address,address,uint256)");

    let logs = provider.get_logs(&event_filter).await?;

    for log in logs {
        let block_number = log.block_number.unwrap_or_default();

        let topic0 = FixedBytes::from(log.topics()[1]);
        let topic0 = FixedBytes::<20>::try_from(&topic0[12..32]).unwrap();
        let token0 = Address::from(topic0);

        let token1 = Address::from(
            FixedBytes::<20>::try_from(&FixedBytes::from(log.topics()[2])[12..32]).unwrap(),
        );
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
        .address(UNISWAP_V3_FACTORY)
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
        let pool_address = FixedBytes::<32>::try_from(decoded.1).unwrap();
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

// The is_v3_pool function uses an incorrect/incomplete hash for checking V3 pools
// Should use full bytecode verification or a more reliable method
async fn is_v3_pool(
    address: Address,
    provider: &Arc<RootProvider<PubSubFrontend>>,
) -> Result<bool> {
    let code = provider.get_code_at(address).await.unwrap().to_string();

    // Use full bytecode verification instead of partial hash
    let v3_init_code_hash = "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54";
    let is_v3 = code.contains(v3_init_code_hash);
    Ok(is_v3)
}

// functon that takes in a reference to the evm and reference to a pool address, and an amount of required liquidity
// returns a boolean of if the contract has the required liquidity or not
async fn liquidity_test(
    evm: Arc<tokio::sync::Mutex<EvmSimulator<'static>>>,
    pool_address: &Address,
    required_liquidity: BigInt,
    caller_address: Address,
) -> Result<bool, anyhow::Error> {
    // construct sol call for liquidity:
    evm.lock()
        .await
        .set_eth_balance(
            caller_address,
            U256::from(1000) * U256::from(10).pow(U256::from(18)),
        )
        .await;
    alloy::sol! {
       function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    let params = getReservesCall {}.abi_encode();

    // do call to evm?

    let tx = crate::common::revm::Tx {
        caller: caller_address,
        transact_to: *pool_address,
        value: U256::ZERO,
        gas_price: U256::from(20_000),
        gas_limit: 120_000_000u64,
        data: params.into(),
    };

    let res = evm.lock().await.call(tx)?;

    let output = decode_reserves_call(&res.output).unwrap_or_else(|e| vec![U256::ZERO, U256::ZERO]);

    let output1 = BigInt::from_signed_bytes_be(&output[0].to_be_bytes_vec());

    let output2 = BigInt::from_signed_bytes_be(&output[1].to_be_bytes_vec());

    let liquidity = BigInt::from(output1 * output2);
    let liquidity = liquidity.sqrt();

    if liquidity >= BigInt::from(required_liquidity) {
        return Ok(true);
    }
    Ok(false)
}

// function that creates an evm
async fn create_evm(
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
) -> (EvmSimulator<'static>, Address) {
    let latest_block_number = provider.get_block_number().await.unwrap();

    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    let evm = EvmSimulator::new(
        provider.clone(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    );
    (evm, contract_wallet_address)
}

fn decode_reserves_call(data: &revm::primitives::Bytes) -> Result<Vec<U256>, String> {
    let decoded_data = hex::decode(data.to_string().trim_start_matches("0x"))
        .map_err(|e| format!("Failed to decode data: {}", e))?;
    if decoded_data.len() != 96 {
        return Err("Invalid data length".to_string());
    }
    // Next 32 bytes contain reserves 0
    let reserves_one = U256::from_be_slice(&decoded_data[0..32]);
    // Next 32 btes contain reserves 1
    let reserves_two = U256::from_be_slice(&decoded_data[32..64]);

    Ok(vec![reserves_one, reserves_two])
}
