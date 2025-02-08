use crate::arbitrage::simulation::{self, arboo_bytecode, get_address, AddressType};
use crate::common::{
    logs::LogEvent,
    pairs::{Event, V2PoolCreated, V3PoolCreated},
    revm::{EvmSimulator, Tx},
};
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy_primitives::aliases::U24;
use alloy_primitives::U160;
use alloy_sol_types::{SolCall, SolValue};
use anyhow::{anyhow, Result};
use log::info;
use revm::primitives::{address, Address, U256};
use sha3::digest::consts::U2;
use std::env::current_exe;
use std::io::Read;
use std::ops::Add;
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
                let is_v2_to_v3 = message.pool_variant == 2;
                // Calculate optimal amount
                let max_input = U256::from(10_000) * U256::from(10).pow(U256::from(18)); // 1000
                                                                                         // let max_input = U256::MAX;
                let optimal_result = find_optimal_amount_v3_to_v2(
                    message.corresponding_pool_address,
                    message.log_pool_address,
                    message.token0,
                    message.token1,
                    simulator.clone(),
                    max_input,
                    is_v2_to_v3,
                )
                .await
                .expect("Failed");
                info!("optimal_result {:?}", optimal_result);
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
    v2_pool: Address,
    token_in: Address,
    token_out: Address,
    simulator: Arc<TokioMutex<EvmSimulator<'a>>>,
    max_input: U256,
    is_v2_to_v3: bool,
) -> Result<ArbitrageResult> {
    let mut best_profit = U256::ZERO;
    let mut optimal_amount = U256::ZERO;

    // Binary search parameters
    let mut left = U256::from(10).pow(U256::from(18)); // Start with 1 token
    let mut right = max_input;

    while left <= right {
        let mid = (left + right) / U256::from(2);

        let v3_amount_out = match get_amounts_out(
            simulator.clone(),
            left,
            token_in,
            token_out,
            get_address(AddressType::V2Router),
            is_v2_to_v3,
        )
        .await
        {
            Ok(amount) => amount,
            Err(e) => break
        };

        // Calculate potential profit based on output amount
        // Here you might want to adjust the profit calculation based on your specific needs

        let current_profit = v3_amount_out;
        // Update best profit if we found better results
        if current_profit > best_profit {
            best_profit = current_profit;
            optimal_amount = mid;
            info!(
                "New optimal amount found: {} with expected output: {}, profit: {}",
                optimal_amount, v3_amount_out, best_profit
            );
        }

        // Binary search adjustment
        let mid_plus_delta = mid + U256::from(100).pow(U256::from(18)); // 1 token increment

        let v3_amount_out = match get_amounts_out(
            simulator.clone(),
            right,
            get_address(AddressType::Weth),
            get_address(AddressType::Uni),
            get_address(AddressType::V2Router),
            is_v2_to_v3,
        )
        .await
        {
            Ok(amount) => amount,
            Err(e) => break
        };

        let next_profit = v3_amount_out;
        // let next_profit = if v3_amount_out > mid_plus_delta {
        //     v3_amount_out - mid_plus_delta
        // } else {
        //     U256::ZERO
        // };

        if next_profit > current_profit {
            left = mid + U256::from(1);

            info!("next mid after increased profit? {:?}", mid);
        } else {
            right = mid - U256::from(1);
            info!("next right profit not increased? {:?}", right);
        }
    }

    Ok(ArbitrageResult {
        optimal_amount,
        expected_profit: best_profit,
    })
}

async fn get_amounts_out(
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
    amount: U256,
    token_a: Address,
    token_b: Address,
    v2_router: Address,
    is_v2_to_v3: bool,
) -> Result<U256> {
    // This tests if there is a price discrepancy between v2 and v3

    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider: RootProvider<PubSubFrontend, AnyNetwork> = ProviderBuilder::new()
        .network()
        .on_ws(ws_client)
        .await
        .expect("Error getting ws client");
    let provider = Arc::new(provider);

    let latest_block_number = provider
        .get_block_number()
        .await
        .expect("error getting block number");
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
        .await
        .unwrap()
        .expect("Expected block");

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
    let mut sim = simulator.lock().await;
    let sim_owner = sim.owner;

    // sim.load_v3_pool_state(get_address(AddressType::UniV3Pool))
    //     .await
    //     .unwrap();

    sim.set_eth_balance(
        sim_owner,
        U256::from(1000) * U256::from(10).pow(U256::from(18)),
    )
    .await;
    let mut profit = U256::ZERO;
    alloy::sol! {
        #[derive(Debug)]
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    };

    let tx_call: getAmountsOutCall = getAmountsOutCall {
        amountIn: amount,
        path: vec![token_a, token_b].into(),
    };
    let data = tx_call.abi_encode();

    let tx = Tx {
        caller: sim.owner,
        transact_to: v2_router,
        data: data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price.clone(),
        gas_limit: latest_gas_limit.clone(),
    };

    let res = sim
        .call(tx)
        .inspect_err(|e| info!("Failed to call v2 swap, {:?}", e))?;

    let v2_amount_out = decode_uniswap_v2_quote(&res.output).expect("failed to decode output");

    info!("v2_amount_out {:?}", v2_amount_out);

    if v2_amount_out == U256::ZERO {
        profit = U256::ZERO;
    }
    alloy::sol! {
        #[derive(Debug)]
        function quoteExactInput(
            bytes memory path,
            uint256 amountIn
        ) external returns (uint256 amountOut, uint160[] sqrtPriceX96AfterList, uint32[] initializedTicksCrossedList, uint256 gasEstimate);
    }

    let mut path = Vec::new();
    path.extend_from_slice(token_a.as_slice());
    path.extend_from_slice(&U24::from(3000).to_be_bytes_vec());
    path.extend_from_slice(token_b.as_slice());
    let path = alloy::primitives::Bytes::from(path);

    let tx_data = quoteExactInputCall {
        path: path,
        amountIn: amount,
    }
    .abi_encode();

    let tx = Tx {
        caller: sim.owner,
        transact_to: get_address(AddressType::Quoter),
        data: tx_data.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price.clone(),
        gas_limit: latest_gas_limit.clone(),
    };

    let res = sim.call(tx)?;

    let v3_amount_out = decode_quote_output_v3(res.output).expect("failed to decode output");

    info!("v3_amount_out {:?}", v3_amount_out);

    if v3_amount_out == U256::ZERO {
        profit = U256::ZERO;
    }
    if is_v2_to_v3 && v3_amount_out > v2_amount_out {
        profit = v3_amount_out - v2_amount_out;
    } else if !is_v2_to_v3 && v2_amount_out > v3_amount_out {
        profit = v2_amount_out - v3_amount_out;
    }

    Ok(profit)
}

fn decode_uniswap_v2_quote(data: &revm::primitives::Bytes) -> Result<U256, String> {
    let decoded_data =
        hex::decode(data.to_string().trim_start_matches("0x")).expect("failed to decode data");

    // First 32 bytes is the offset to the array data
    let offset = U256::from_be_slice(&decoded_data[0..32]);
    assert_eq!(offset, U256::from(32), "Unexpected offset");

    // Next 32 bytes contain the array length
    let array_length = U256::from_be_slice(&decoded_data[32..64]);
    assert_eq!(array_length, U256::from(2), "Expected array length 2");

    // Next 32 bytes contain the input amount
    let amount_in = U256::from_be_slice(&decoded_data[64..96]);
    // assert_eq!(amount_in, U256::from(10).pow(U256::from(18)), "Expected input amount 1");

    let amount_out = U256::from_be_slice(&decoded_data[96..128]);
    let amount_out = amount_out;

    Ok(amount_out)
}

fn decode_quote_output_v3(output: revm::primitives::Bytes) -> Result<U256> {
    let output = hex::decode(output.to_string().trim_start_matches("0x"))?;

    let number = U256::from_be_slice(&output[0..32]);

    Ok(number)
}
