use super::pairs::Event;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use futures::StreamExt;
use revm::primitives::{address, keccak256};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::broadcast::Sender;

pub async fn get_logs(
    client: Arc<RootProvider<PubSubFrontend>>,
    pairs: HashMap<Address, Event>,
    event_sender: Sender<LogEvent>,
) {
    // before we do this we will need a bunch of addresses to filter on.
    // One way of doing this maybe is just having a bunch of filters? not sure
    // we might have to filter the event after it's come in to detect if the
    // address is one that has two uniswap pools

    let v2_swap_signature =
        keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_signature =
        keccak256("Swap(address,address,int256,int256,uint160,uint160,int24)".as_bytes());

    let filter = Filter::new()
        .event_signature(vec![v3_swap_signature, v2_swap_signature])
        .from_block(BlockNumberOrTag::Latest);
    let sub = client.subscribe_logs(&filter).await.unwrap();

    let mut stream = sub.into_stream();

    while let Some(res) = stream.next().await {
        let key = res.address();
        let token0 = res.data().topics()[1];
        let token0 = Address::from_slice(&token0.0[12..32]);
        let token1 = Address::from_slice(&res.data().topics()[2].0[12..32]);

        // The strategy needs both the log pool address and the corresponding other v pool address, they are in hashmap
        if pairs.contains_key::<Address>(&key) {

            match pairs.get(&key).unwrap() {
                Event::PairCreated(pair) => {
                    let value = pairs.values().find(|value| {
                        let test = match value {
                            Event::PoolCreated(v3_pair) => {
                                if v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1
                                    || v3_pair.token1 == token0 && v3_pair.token0 == token1
                                {
                                    return true;
                                }
                                false
                            }
                            _ => false,
                        };
                        test
                    });
                    let v3_address = match value {
                        Some(Event::PoolCreated(pair)) => pair.pair_address,
                        _ => continue,
                    };
                    match event_sender.send(LogEvent {
                        pool_variant:2,
                        corresponding_pool_address: v3_address,
                        log_pool_address: key,
                        token0,
                        token1,
                    }) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                    continue;
                }
                Event::PoolCreated(pair) => {
                    let value = pairs.values().find(|value| {
                        let test = match value {
                            Event::PairCreated(v2_pair) => {
                                if v2_pair.token0 == pair.token0 && v2_pair.token1 == pair.token1
                                    || v2_pair.token1 == token0 && v2_pair.token0 == token1
                                {
                                    return true;
                                }
                                false
                            }
                            _ => false,
                        };
                        test
                    });

                    let v2_address = match value {
                        Some(Event::PoolCreated(pair)) => pair.pair_address,
                        _ => continue,
                    };
                    match event_sender.send(LogEvent {
                        pool_variant: 3,
                        corresponding_pool_address: v2_address,
                        log_pool_address: key,
                        token0,
                        token1,
                    }) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                    continue;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub pool_variant: usize,
    pub corresponding_pool_address: Address,
    pub log_pool_address: Address,
    pub token0: Address,
    pub token1: Address,
}
