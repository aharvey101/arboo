use alloy::primitives::{keccak256, Address};
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Filter, Log};
use anyhow::Result;
use log::info;
use revm::primitives::{B256, U256};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn get_pairs(
    client: Arc<RootProvider<PubSubFrontend>>,
) -> Result<HashMap<Address, Event>> {
    // NOTE: fee's are still broken
    let latest_block = client.get_block_number().await.unwrap();
    let from_block = latest_block - 100_000;

    let to_block = latest_block;
    let v2_filter = Filter::new()
        .address(uniswap_v2_factory_address())
        .from_block(from_block)
        .to_block(to_block);

    let v3_filter = Filter::new()
        .address(uniswap_v3_factory_address())
        .from_block(from_block)
        .to_block(to_block);

    // Get the logs
    let mut logs = client.get_logs(&v3_filter).await?;
    logs.extend(client.get_logs(&v2_filter).await?);
    // Process the logs into a hashmap
    let mut pools = vec![];
    for log in logs {
        if let Some(event) = decode_event(&log).await {
            pools.push(event);
        }
    }
    pools.sort();
    // info!("pools? {pools:?}");
    let mut result = HashMap::<Address, Event>::new();

    let mut iter = pools.into_iter().peekable();

    while let Some(current) = iter.next() {
        if let Some(next) = iter.peek() {
            if *next == current {
                result.insert(current.get_address(), current);
                let next = iter.next().unwrap();
                result.insert(next.get_address(), next);
            }
        }
    }
    // we need to map through the pairs and filter out the low market cap pairs, shall we start with 10m?
    // My thoughts are we could just get the data of each uniswap pair and get the product of the two tokens

    // get_pair_data(client, result.clone()).await.unwrap();
    info!("hash map size: {:?}", result.capacity());
    Ok(result)
}

async fn decode_event(log: &Log) -> Option<Event> {
    let uniswap_v2_pool_create_sig =
        keccak256("PairCreated(address,address,address,uint256)".as_bytes()).to_vec();
    let uniswap_v3_pool_create_sig =
        keccak256("PoolCreated(address,address,uint24,int24,address)".as_bytes()).to_vec();

    if log.address().len() > 0 {
        if log.topics()[0] == B256::from_slice(&uniswap_v2_pool_create_sig) {
            let address = Address::from_slice(&log.data().data.0[12..32]);
            let topic_1_address_slice = &log.topics()[1].0.as_slice()[12..32];
            let topic_2_address_slice = &log.topics()[2].0.as_slice()[12..32];
            let token0 = Address::from_slice(topic_1_address_slice);
            let token1 = Address::from_slice(topic_2_address_slice);

            return Some(Event::PairCreated(V2PoolCreated {
                pair_address: address,
                token0,
                token1,
                block_number: log.block_number.unwrap(),
                fee: 500,
            }));
        } else if log.topics()[0] == B256::from_slice(&uniswap_v3_pool_create_sig) {
            let address = Address::from_slice(&log.data().data.0[44..64]);
            let token0 = Address::from_slice(&log.topics()[1].0.as_slice()[12..32]);
            let token1 = Address::from_slice(&log.topics()[1].0.as_slice()[12..32]);

            return Some(Event::PoolCreated(V3PoolCreated {
                pair_address: address,
                token0,
                token1,
                fee: 500,
                tick_spacing: 100,
            }));
        }
    }
    None
}

async fn get_pair_data(
    provider: Arc<RootProvider<PubSubFrontend>>,
    pairs: HashMap<Address, Event>,
) -> Result<()> {
    for pair in pairs {
        // info!("pair: {pair:?}");
        let storage = provider
            .get_storage_at(pair.0, U256::from(2))
            .await
            .unwrap();

        // info!("Storage: {storage:?}");
    }

    Ok(())
}

fn uniswap_v3_factory_address() -> Address {
    "0x1F98431c8aD98523631AE4a59f267346ea31F984"
        .parse()
        .unwrap()
}

fn uniswap_v2_factory_address() -> Address {
    "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"
        .parse()
        .unwrap()
}

#[derive(Debug, Clone)]
pub enum Event {
    PairCreated(V2PoolCreated),
    PoolCreated(V3PoolCreated),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2PoolCreated {
    pub token0: Address,
    pub token1: Address,
    pub pair_address: Address,
    pub block_number: u64,
    pub fee: u32,
}
// Uniswap V3 PoolCreated event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3PoolCreated {
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub tick_spacing: i32,
    pub pair_address: Address,
}

trait CommonFields {
    fn token0(&self) -> Address;
}
trait GetAddress {
    fn get_address(&self) -> Address;
}

// Implement the trait for both structs
impl CommonFields for V3PoolCreated {
    fn token0(&self) -> Address {
        self.token0
    }
}
impl CommonFields for V2PoolCreated {
    fn token0(&self) -> Address {
        self.token0
    }
}

// Implement Ord and PartialOrd for sorting impl Ord for Pools{
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.token0()
            .cmp(&other.token0())
            .then_with(|| self.token0().cmp(&other.token0()))
    }
}
impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.token0() == other.token0()
    }
}

impl Eq for Event {}
// Implement the CommonFields trait for MyEnum
impl CommonFields for Event {
    fn token0(&self) -> Address {
        match self {
            Event::PairCreated(a) => a.token0(),
            Event::PoolCreated(b) => b.token0(),
        }
    }
}

impl GetAddress for Event {
    fn get_address(&self) -> Address {
        match self {
            Event::PairCreated(a) => a.pair_address,
            Event::PoolCreated(b) => b.pair_address,
        }
    }
}
