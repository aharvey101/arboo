use crate::arbitrage::simulation::{self, arboo_bytecode, get_address, AddressType};
use crate::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::{EvmSimulator, Tx},
};
use anyhow::{anyhow, Result};
use log::info;
use revm::primitives::{address, Address, U256};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead},
    path::Path,
    str::FromStr,
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio::sync::{broadcast::Sender, Mutex as TokioMutex};

pub async fn strategy(sender: Sender<LogEvent>, simulator: Arc<Mutex<EvmSimulator<'_>>>) {
    // simulator.lock().await.deploy(arboo_bytecode()).await;

    // we want to load all of the contracts into memory
    info!("Loading pools");
    // load_pools(simulator.clone())
    //     .await;
    // info!("Pools Loaded")

    let mut event_reciever = sender.subscribe();

    loop {
        match event_reciever.recv().await {
            // this has to recieve the event
            Ok(message) => {
                info!("The Message: {:?}", message);
                if message.pool_variant == 2 {
                    // THis will be removed
                    ////////////////////////////////////////////////////////////////////////////////
                    simulator.lock().await.load_account(message.token0).await;
                    simulator.lock().await.load_account(message.token1).await;
                    simulator
                        .lock()
                        .await
                        .load_account(message.log_pool_address)
                        .await;
                    // simulator
                    //     .lock()
                    //     .await
                    //     .load_v2_pool_state(message.log_pool_address)
                    //     .await
                    //     .unwrap();
                    simulator
                        .lock()
                        .await
                        .load_account(message.corresponding_pool_address)
                        .await;
                    // simulator
                    //     .lock()
                    //     .await
                    //     .load_v3_pool_state(message.corresponding_pool_address)
                    //     .await
                    //     .expect("Failed");
                    //////////////////////////////////////////////////////////////////////////////

                    let max_input = U256::from(1_000_000_000) * U256::from(10).pow(U256::from(18)); // 1000
                    let optimal_result = find_optimal_amount_v3_to_v2(
                        message.log_pool_address,
                        message.token1,
                        message.token0,
                        simulator.clone(),
                        max_input,
                    )
                    .await
                    .expect("Failed");
                    info!("optimal_result {:?}", optimal_result);
                }

                if message.pool_variant == 3 {
                    // THis will be removed
                    ////////////////////////////////////////////////////////////////////////////////
                    simulator.lock().await.load_account(message.token0).await;
                    simulator.lock().await.load_account(message.token1).await;
                    simulator
                        .lock()
                        .await
                        .load_account(message.corresponding_pool_address)
                        .await;
                    // simulator
                    //     .lock()
                    //     .await
                    //     .load_v2_pool_state(message.corresponding_pool_address)
                    //     .await
                    //     .unwrap();
                    simulator
                        .lock()
                        .await
                        .load_account(message.log_pool_address)
                        .await;
                    // simulator
                    //     .lock()
                    //     .await
                    //     .load_v3_pool_state(message.log_pool_address)
                    //     .await
                    //     .expect("Failed");
                    ////////////////////////////////////////////////////////////////////////////////

                    // Calculate optimal amount
                    let max_input = U256::from(1000) * U256::from(10).pow(U256::from(18)); // 1000
                    let optimal_result = find_optimal_amount_v3_to_v2(
                        message.corresponding_pool_address,
                        message.token0,
                        message.token1,
                        simulator.clone(),
                        max_input,
                    )
                    .await
                    .expect("Failed");
                    info!("optimal_result {:?}", optimal_result);
                }
            }
            Err(err) => {
                info!("OOP")
            }
        }
    }
}
async fn load_pools<'a>(simulator: Arc<Mutex<EvmSimulator<'a>>>) {
    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path).expect("Error getting File");
    let reader = io::BufReader::new(file);

    for line in reader.lines().skip(1) {
        // Skip the header line
        let line = line.expect("Expected Line");
        let fields: Vec<&str> = line.split(',').collect();
        match fields[2] {
            "2" => {
                let pair_address = Address::from_str(fields[1]).expect("error");
                pools_map.insert(
                    pair_address,
                    Event::PairCreated(V2PoolCreated {
                        pair_address: Address::from_str(fields[1]).expect("error"),
                        token0: Address::from_str(fields[3]).expect("error"),
                        token1: Address::from_str(fields[4]).expect("error"),
                        fee: fields[5].parse::<u32>().expect("error"),
                        block_number: fields[6].parse::<u64>().expect("error"),
                    }),
                );
            }
            "3" => {
                let pair_address = Address::from_str(fields[1]).expect("error");
                pools_map.insert(
                    pair_address,
                    Event::PoolCreated(V3PoolCreated {
                        pair_address: Address::from_str(fields[1]).expect("error"),
                        token0: Address::from_str(fields[3]).expect("error"),
                        token1: Address::from_str(fields[4]).expect("error"),
                        fee: fields[5].parse::<u32>().expect("error"),
                        tick_spacing: 0i32,
                    }),
                );
            }
            &_ => continue,
        };
    }
    let mut sim = simulator.lock().await;
    for event in pools_map.iter() {
        info!("Loading pool, {:?}, number:", event.0);
        sim.load_account(*event.0).await;
        sim.load_pool_state(*event.0)
            .await
            .expect("Failed to load basic state");
        if let (_, Event::PairCreated(V2PoolCreated { .. })) = event {
            sim.load_v2_pool_state(*event.0)
                .await
                .expect("Failed to load v2 pool state");
        } else {
            sim.load_v3_pool_state(*event.0)
                .await
                .expect("Failed to load v3 state");
        }
    }
}

#[derive(Debug)]
pub struct ArbitrageResult {
    pub optimal_amount: U256,
    pub expected_profit: U256,
}

pub async fn find_optimal_amount_v3_to_v2<'a>(
    v3_pool: Address,
    token_in: Address,
    token_out: Address,
    simulator: Arc<TokioMutex<EvmSimulator<'a>>>,
    max_input: U256,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;

    // Binary search parameters
    let mut left = U256::from(10).pow(U256::from(18)); // Start with 1 token
    let mut right = max_input;

    let v3_out = simulation::simulation(v3_pool, token_in, token_out, left, simulator.clone())
        .await
        .expect("failed to simulate");
    info!("Out: v3_out {:?}",v3_out );
    // while left <= right {
    //     let mid = (left + right) / U256::from(2);

    //     let v3_out = simulation::simulation(v3_pool, token_in, token_out, left, simulator.clone())
    //         .await
    //         .expect("failed to simulate");

    //     // Calculate potential profit based on output amount
    //     // Here you might want to adjust the profit calculation based on your specific needs
    //     let current_profit = if v3_out > mid {
    //         v3_out - mid
    //     } else {
    //         U256::ZERO
    //     };

    //     // Update best profit if we found better results
    //     if current_profit > best_profit {
    //         best_profit = current_profit;
    //         optimal_amount = mid;
    //         info!(
    //             "New optimal amount found: {} with expected output: {}, profit: {}",
    //             optimal_amount, v3_out, best_profit
    //         );
    //     }

    //     // Binary search adjustment
    //     let mid_plus_delta = mid + U256::from(10).pow(U256::from(17)); // 0.1 token increment
    //     let next_v3_out =
    //         simulation::simulation(v3_pool, token_in, token_out, right, simulator.clone())
    //             .await
    //             .expect("Failed to simulate");

    //     let next_profit = if next_v3_out > mid_plus_delta {
    //         next_v3_out - mid_plus_delta
    //     } else {
    //         U256::ZERO
    //     };

    //     if next_profit > current_profit {
    //         left = mid + U256::from(1);
    //     } else {
    //         right = mid - U256::from(1);
    //     }
    // }

    Ok(ArbitrageResult {
        optimal_amount,
        expected_profit: best_profit,
    })
}

// let target_pool = pools_map
//     .iter()
//     .find(|(_, event)| match event {
//         Event::PairCreated(V2PoolCreated { token0, token1, .. })
//         | Event::PoolCreated(V3PoolCreated { token0, token1, .. }) => {
//             (token0 == &get_address(AddressType::Uni)
//                 && token1 == &get_address(AddressType::Weth))
//                 || (token0 == &get_address(AddressType::Weth)
//                     && token1 == &get_address(AddressType::Uni))
//         }
//         _ => false,
//     })
//     .map(|(address, _)| address)
//     .expect("UNI-ETH pool not found");

// let test_pools: HashMap<Address, Event> = pools_map
//     .iter()
//     .filter(|(_, event)| match event {
//         Event::PairCreated(V2PoolCreated { token0, token1, .. })
//         | Event::PoolCreated(V3PoolCreated { token0, token1, .. }) => {
//             (token0 == &get_address(AddressType::Uni)
//                 && token1 == &get_address(AddressType::Weth))
//                 || (token0 == &get_address(AddressType::Weth)
//                     && token1 == &get_address(AddressType::Uni))
//         }
//         _ => false,
//     })
//     .map(|(address, event)| (*address, event.clone()))
//     .collect();

// info!("pools {:?}", test_pools);
