use super::pairs::Event;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::Filter;
use futures::StreamExt;
use log::info;
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
        // let token0 = Address::from_slice(&res.data().topics()[1][12..32]);
        // let token1 = Address::from_slice(&res.data().topics()[2][12..32]);
        // info!("Topics : {:?}", res.data().topics());
        // info!("Token 0: {:?}", token0);
        // info!("Token 1 {:?}", token1);

        // The strategy needs both the log pool address and the corresponding other v pool address, they are in hashmap
        if let Some(event) = pairs.get(&key) {

            //So the issue is that we were using the addresses that were intercting with the pool, but we need to use the addresses that are in the pool
            // To do this we need to either use the 


            match event {
            Event::PairCreated(pair) => {
                if let Some(Event::PoolCreated(v3_pair)) = pairs.values().find(|value| {
                matches!(value, Event::PoolCreated(v3_pair) if (v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1) || (v3_pair.token0 == pair.token1 && v3_pair.token1 == pair.token0))
                }) {


                    // NOTE: this shouldn't be so, not sure why it's doing this
                    if v3_pair.token0 == v3_pair.token1 {continue}
    
                    event_sender.send(LogEvent {
                    pool_variant: 2,
                    corresponding_pool_address: v3_pair.pair_address,
                    log_pool_address: key,
                    token0:v3_pair.token0,
                    token1: v3_pair.token1,
                }).expect("Failed to send event");
                }
            }
            Event::PoolCreated(pair) => {
                if let Some(Event::PairCreated(v2_pair)) = pairs.values().find(|value| {
                matches!(value, Event::PairCreated(v2_pair) if (v2_pair.token0 == pair.token0 && v2_pair.token1 == pair.token1) || (v2_pair.token0 == pair.token1 && v2_pair.token1 == pair.token0))
                }) {

                let _ = event_sender.send(LogEvent {
                    pool_variant: 3,
                    corresponding_pool_address: v2_pair.pair_address,
                    log_pool_address: key,
                    token0: v2_pair.token0,
                    token1: v2_pair.token1,
                });
                }
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
