use crate::arbitrage::simulation::{self, one_ether, simulation};
use crate::arbitrage::simulation::{get_address, AddressType};
use crate::common;
use crate::common::transaction::{create_input_data, send_transaction};
use crate::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::{EvmSimulator, Tx},
};
use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Block, BlockTransactionsKind};
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::aliases::U24;
use alloy_primitives::{address, Bytes, U160};
use alloy_sol_types::SolCall;
use anyhow::Result;
use dotenv::var;
use log::{debug, info};
use revm::primitives::{Address, U256};
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

pub async fn strategy(
    sender: Sender<LogEvent>,
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
) -> Result<()> {
    let mut event_reciever = sender.subscribe();
    loop {
        match event_reciever.recv().await {
            // this has to recieve the event
            Ok(message) => {
                // reserves of the target pool to low?
                let is_v2_to_v3 = message.pool_variant == 3;

                // Calculate optimal amount
                let max_input = U256::from(10_000) * U256::from(10).pow(U256::from(18)); // 1000

                let latest_block = provider
                    .get_block(BlockId::latest(), BlockTransactionsKind::Full)
                    .await
                    .unwrap()
                    .expect("Expected block");
                let gas_limit = latest_block.header.gas_limit;
                let block_base_fee = latest_block.header.base_fee_per_gas.unwrap();

                let private_key = var("PRIVATE_KEY").unwrap();
                let signer = PrivateKeySigner::from_str(&private_key).unwrap();

                let nonce = provider
                    .get_transaction_count(signer.address())
                    .await
                    .expect("error getting nonce");

                load_specific_pools(
                    simulator.clone(),
                    message.log_pool_address,
                    message.corresponding_pool_address,
                )
                .await?;

                let time = std::time::Instant::now();

                let optimal_result = match find_optimal_amount_v3_to_v2(
                    message.token0,
                    message.token1,
                    simulator.clone(),
                    max_input,
                    message.fee,
                    latest_block.clone(),
                )
                .await
                {
                    Ok(res) => res,
                    Err(_) => continue,
                };

                if optimal_result.possible_profit < U256::from(10_000_000) {
                    // info!("No arbitrage opportunity found");
                    continue;
                }
                // simulate with optimal amoun in arbooo
                let target_pool = if is_v2_to_v3 {
                    message.log_pool_address
                } else {
                    message.corresponding_pool_address
                };
                info!(
                    "Tike taken to calculate optimal amount: {:?}",
                    time.elapsed()
                );
                info!("Arbitrage opportunity found");
                info!(
                    "Creating and sending TX for optimal amount {} to pool {}",
                    optimal_result.optimal_amount, target_pool
                );

                simulation(
                    target_pool,
                    message.token0,
                    message.token1,
                    optimal_result.optimal_amount,
                    simulator.clone(),
                )
                .await
                .unwrap_or_default();

                log::debug!("Time taken to run sim {:?}", time.elapsed());

                if provider.get_block_number().await.unwrap_or_default()
                    > latest_block.header.number
                {
                    info!("Block has passed, opportunity has passed");
                    continue;
                }
                let transaction = create_input_data(
                    target_pool,
                    message.fee,
                    message.token0,
                    message.token1,
                    optimal_result.optimal_amount,
                )
                .await
                .unwrap();

                let contract_address = var::<&str>("CONTRACT_ADDRESS").unwrap();
                let contract_address = Address::from_str(&contract_address).unwrap();

                let bribe = 800_000_000u128;

                let max_fee_per_gas = u128::from(gas_limit);

                tokio::spawn(send_transaction(
                    contract_address,
                    Some(block_base_fee as u128),
                    Some(4_000_000),
                    Some(bribe + 2_000_000),
                    Some(bribe),
                    transaction,
                    nonce,
                ));
            }
            Err(err) => {
                info!("Error Recieving message: {err}")
            }
        }
    }
}

#[derive(Debug)]
pub struct ArbitrageResult {
    pub optimal_amount: U256,
    pub possible_profit: U256,
}

pub async fn find_optimal_amount_v3_to_v2(
    token_in: Address,
    token_out: Address,
    simulator: Arc<TokioMutex<EvmSimulator<'_>>>,
    max_input: U256,
    fee: U24,
    latest_block: Block,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;
    let mut left = U256::from(10).pow(U256::from(18)); // 1 token
    let mut right = max_input;

    while left <= right {
        let mid = (left + right) / U256::from(2);

        // Only query once per iteration with mid
        let v3_amount_out = match get_v3_to_v2_arbitrage_profit(
            simulator.clone(),
            mid,
            token_in,
            token_out,
            fee,
            get_address(AddressType::V2Router),
            latest_block.clone(),
        )
        .await
        {
            Ok(amount) => amount,
            Err(_) => break,
        };

        // Calculate profit based on mid amount
        let current_profit = v3_amount_out;

        // Update best profit if better
        if current_profit > best_profit {
            best_profit = current_profit;
            optimal_amount = mid;
            // If profit is increasing, search upper half
            left = mid + U256::from(1);
        } else {
            // If profit is decreasing, search lower half
            right = mid - U256::from(1);
        }
    }

    if best_profit == U256::ZERO {
        return Ok(ArbitrageResult {
            optimal_amount: U256::ZERO,
            possible_profit: U256::ZERO,
        });
    }

    // convert optimal amount to weth

    alloy::sol! {
        #[derive(Debug)]
        function quoteExactInput(
            bytes memory path,
            uint256 amountIn
        ) external returns (uint256 amountOut, uint160[] sqrtPriceX96AfterList, uint32[] initializedTicksCrossedList, uint256 gasEstimate);
    }

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
    let mut sim = simulator.lock().await;
    let mut path = Vec::new();
    path.extend_from_slice(token_in.as_slice());
    path.extend_from_slice(&U24::from(3000).to_be_bytes_vec());
    path.extend_from_slice(get_address(AddressType::Weth).as_slice());
    let path = alloy::primitives::Bytes::from(path);

    let tx_data = quoteExactInputCall {
        path,
        amountIn: best_profit,
    }
    .abi_encode();

    let tx = Tx {
        caller: sim.owner,
        transact_to: get_address(AddressType::V2Quoter),
        data: tx_data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let res = sim.call(tx)?;

    let possible_profit = decode_quote_output_v3(res.output).expect("failed to decode output");
    info!("possible_profit {possible_profit}");
    Ok(ArbitrageResult {
        optimal_amount,
        possible_profit,
    })
}

async fn get_v3_to_v2_arbitrage_profit(
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    amount_in: U256,
    token_a: Address,
    token_b: Address,
    fee: U24,
    v2_router: Address,
    latest_block: Block,
) -> Result<U256> {
    // Setup the simulation environment
    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
    let mut sim = simulator.lock().await;
    let sim_owner = sim.owner;

    // Set a large ETH balance for testing
    sim.set_eth_balance(
        sim_owner,
        U256::from(1000) * U256::from(10).pow(U256::from(18)),
    )
    .await;

    // Step 1: Calculate how many tokenB we get from borrowing tokenA on Uniswap V3
    // We'll use the V3 Quoter contract to simulate this
    alloy::sol! {
        #[derive(Debug)]
        function quoteExactInput(
            bytes memory path,
            uint256 amountIn
        ) external returns (uint256 amountOut, uint160[] sqrtPriceX96AfterList, uint32[] initializedTicksCrossedList, uint256 gasEstimate);
    }
    let v3_quoter_address = get_address(AddressType::V2Quoter);

    // Construct the path for token swap (token_a -> token_b with fee)
    let mut path = Vec::with_capacity(43);

    path.extend_from_slice(token_a.as_slice());
    path.extend_from_slice(&U24::from(3000).to_be_bytes_vec());
    path.extend_from_slice(token_b.as_slice());

    let path = alloy::primitives::Bytes::from(path);

    let tx_data = quoteExactInputCall {
        path,
        amountIn: amount_in,
    };

    let tx_data = tx_data.abi_encode();
    let tx = Tx {
        caller: sim_owner,
        transact_to: v3_quoter_address,
        data: tx_data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let res = match sim.call(tx) {
        Ok(res) => res,
        Err(e) => {
            // info!("Failed to call V3 quote: {:?}", e);
            return Ok(U256::ZERO); // Return zero profit if V3 quote fails
        }
    };

    // Extract the exact tokenB amount we'd receive from V3
    let v3_amount_out = match decode_quote_exact_input_single_output(res.output) {
        Ok(amount_out) => amount_out,
        Err(e) => {
            info!("Failed to decode V3 quote output: {:?}", e);
            return Ok(U256::ZERO);
        }
    };

    if v3_amount_out == U256::ZERO {
        info!("V3 swap would result in zero tokens, no arbitrage possible");
        return Ok(U256::ZERO);
    }

    // Step 2: Calculate how many tokenA we get back by swapping tokenB on Uniswap V2
    alloy::sol! {
        #[derive(Debug)]
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    };

    let tx_call: getAmountsOutCall = getAmountsOutCall {
        amountIn: v3_amount_out,
        path: vec![token_b, token_a],
    };

    let data = tx_call.abi_encode();
    let tx = Tx {
        caller: sim_owner,
        transact_to: v2_router,
        data: data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let res = match sim.call(tx) {
        Ok(res) => res,
        Err(e) => {
            //info!("Failed to call V2 getAmountsOut: {:?}", e);
            return Ok(U256::ZERO);
        }
    };

    let v2_buy_back_amount = match decode_uniswap_v2_quote(&res.output) {
        Ok(amount_out) => amount_out,
        Err(e) => {
            //info!("Failed to decode V2 quote output: {}", e);
            return Ok(U256::ZERO);
        }
    };

    if v2_buy_back_amount == U256::ZERO {
        //info!("V2 swap would result in zero tokens, no arbitrage possible");
        return Ok(U256::ZERO);
    }

    // Step 3: Check if profitable
    if v2_buy_back_amount <= amount_in {
        //info!(
        //    "Not profitable: buy back amount {} <= amount in {}",
        //    v2_buy_back_amount, amount_in
        //);
        return Ok(U256::ZERO);
    }

    let profit = v2_buy_back_amount - amount_in;

    // Step 4: If tokenA is not WETH, calculate how much WETH we'd get by swapping profit
    let weth = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
        .expect("WETH address should be valid");

    if token_a != weth {
        // Calculate WETH equivalent of profit
        let tx_call: getAmountsOutCall = getAmountsOutCall {
            amountIn: profit,
            path: vec![token_a, weth],
        };

        let data = tx_call.abi_encode();
        let tx = Tx {
            caller: sim_owner,
            transact_to: v2_router,
            data: data.into(),
            value: U256::ZERO,
            gas_price: latest_gas_price,
            gas_limit: latest_gas_limit,
        };

        let res = match sim.call(tx) {
            Ok(res) => res,
            Err(e) => {
                info!("Failed to calculate WETH profit: {:?}", e);
                return Ok(profit); // Return the token profit if WETH calc fails
            }
        };

        let weth_profit = match decode_uniswap_v2_quote(&res.output) {
            Ok(amount_out) => amount_out,
            Err(e) => {
                info!("Failed to decode WETH profit quote: {}", e);
                return Ok(profit);
            }
        };
        return Ok(weth_profit);
    }

    // If tokenA is already WETH, just return the profit
    Ok(profit)
}

// Helper function to decode V3 quoter output
fn decode_quote_exact_input_single_output(output: Bytes) -> Result<U256> {
    let output = hex::decode(output.to_string().trim_start_matches("0x"))?;
    // Just return the first value (amountOut) which is all we need
    let amount_out = U256::from_be_slice(&output[0..32]);
    Ok(amount_out)
}

// Helper function to decode V2 getAmountsOut output
fn decode_uniswap_v2_quote(output: &Bytes) -> Result<U256, String> {
    let decoded_data =
        hex::decode(output.to_string().trim_start_matches("0x")).expect("failed to decode data");

    // First 32 bytes is the offset to the array data
    let offset = U256::from_be_slice(&decoded_data[0..32]);
    assert_eq!(offset, U256::from(32), "Unexpected offset");

    // Next 32 bytes contain the array length
    let array_length = U256::from_be_slice(&decoded_data[32..64]);
    assert_eq!(array_length, U256::from(2), "Expected array length 2");

    // We want the second element (index 1) which is at position 96..128
    let amount_out = U256::from_be_slice(&decoded_data[96..128]);
    Ok(amount_out)
}
fn decode_quote_output_v3(output: revm::primitives::Bytes) -> Result<U256> {
    let output = hex::decode(output.to_string().trim_start_matches("0x"))?;

    let number = U256::from_be_slice(&output[0..32]);

    Ok(number)
}
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use mockall::*;

//     mock! {
//         SimulatorWrapper {
//             async fn get_amounts_out(
//                 &self,
//                 amount: U256,
//                 token_in: Address,
//                 token_out: Address,
//                 v2_router: Address,
//                 is_v2_to_v3: bool,
//             ) -> Result<U256>;
//         }
//     }

//     #[tokio::test]
//     async fn test_find_optimal_amount() {
//         let mut mock_sim = MockSimulatorWrapper::new();

//         // Mock sequence of get_amounts_out calls
//         mock_sim
//             .expect_get_amounts_out()
//             .times(2) // We expect 2 calls based on binary search
//             .returning(|amount, _, _, _, _| {
//                 // Simulate different profits for different amounts
//                 if amount < U256::from(500) * U256::from(10).pow(U256::from(18)) {
//                     Ok(U256::from(100) * U256::from(10).pow(U256::from(18)))
//                 } else {
//                     Ok(U256::from(50) * U256::from(10).pow(U256::from(18)))
//                 }
//             });

//         let simulator = Arc::new(TokioMutex::new(mock_sim));

//         let token_in = Address::from_str("0x1000000000000000000000000000000000000000").unwrap();
//         let token_out = Address::from_str("0x2000000000000000000000000000000000000000").unwrap();
//         let max_input = U256::from(1000) * U256::from(10).pow(U256::from(18));

//         let result = find_optimal_amount_v3_to_v2(
//             token_in,
//             token_out,
//             simulator,
//             max_input,
//             true
//         ).await.unwrap();

//         assert!(result.optimal_amount > U256::ZERO);
//         assert!(result.expected_profit > U256::ZERO);
//         assert!(result.optimal_amount <= max_input);
//     }

//     #[tokio::test]
//     async fn test_find_optimal_amount_zero_profit() {
//         let mut mock_sim = MockSimulatorWrapper::new();

//         mock_sim
//             .expect_get_amounts_out()
//             .returning(|_, _, _, _, _| Ok(U256::ZERO));

//         let simulator = Arc::new(TokioMutex::new(mock_sim));

//         let token_in = Address::from_str("0x1000000000000000000000000000000000000000").unwrap();
//         let token_out = Address::from_str("0x2000000000000000000000000000000000000000").unwrap();
//         let max_input = U256::from(1000) * U256::from(10).pow(U256::from(18));

//         let result = find_optimal_amount_v3_to_v2(
//             token_in,
//             token_out,
//             simulator,
//             max_input,
//             true
//         ).await.unwrap();

//         assert_eq!(result.optimal_amount, U256::ZERO);
//         assert_eq!(result.expected_profit, U256::ZERO);
//     }

//     #[tokio::test]
//     async fn test_find_optimal_amount_error() {
//         let mut mock_sim = MockSimulatorWrapper::new();

//         mock_sim
//             .expect_get_amounts_out()
//             .returning(|_, _, _, _, _| Err(anyhow!("Simulation failed")));

//         let simulator = Arc::new(TokioMutex::new(mock_sim));

//         let token_in = Address::from_str("0x1000000000000000000000000000000000000000").unwrap();
//         let token_out = Address::from_str("0x2000000000000000000000000000000000000000").unwrap();
//         let max_input = U256::from(1000) * U256::from(10).pow(U256::from(18));

//         let result = find_optimal_amount_v3_to_v2(
//             token_in,
//             token_out,
//             simulator,
//             max_input,
//             true
//         ).await.unwrap();

//         assert_eq!(result.optimal_amount, U256::ZERO);
//         assert_eq!(result.expected_profit, U256::ZERO);
//     }
// }
//

async fn load_specific_pools<'a>(
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    pool_a: Address,
    pool_b: Address,
) -> Result<()> {
    let mut pools_map: HashMap<Address, Event> = HashMap::new();
    let path = Path::new("cache/.cached-pools.csv");
    let file = File::open(&path).expect("Error getting File");
    let reader = io::BufReader::new(file);

    let sim = simulator.lock().await;

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
    match pools_map.get(&pool_a) {
        Some(pool) => match pool {
            Event::PoolCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
            }
            Event::PairCreated(pool) => {
                sim.load_v2_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
                sim.load_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load basic state");
            }
        },
        _ => {}
    };

    match pools_map.get(&pool_b) {
        Some(pool) => match pool {
            Event::PoolCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
            }
            Event::PairCreated(pool) => {
                sim.load_v3_pool_state(pool.pair_address)
                    .await
                    .expect("Failed to load v2 pool state");
            }
        },
        _ => {}
    };

    Ok(())
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
