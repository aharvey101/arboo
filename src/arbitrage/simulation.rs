use crate::common::revm::{EvmSimulator, Tx};
use ::log::error;
use alloy::eips::BlockId;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy_primitives::aliases::U24;
use alloy_sol_types::SolCall;
use anyhow::Result;
use revm::primitives::{address, Address, Bytecode, U256};
use std::str::FromStr;

pub async fn simulation(
    target_pool: Address,
    token_a: Address,
    token_b: Address,
    amount: U256,
    fee: U24,
    simulator: &mut EvmSimulator<'_>,
    provider: &RootProvider<PubSubFrontend>,
) -> Result<U256> {
    simulation_with_logging(
        target_pool,
        token_a,
        token_b,
        amount,
        fee,
        simulator,
        provider,
        false,
    )
    .await
}

pub async fn simulation_with_logging(
    target_pool: Address,
    token_a: Address,
    token_b: Address,
    amount: U256,
    fee: U24,
    simulator: &mut EvmSimulator<'_>,
    provider: &RootProvider<PubSubFrontend>,
    enable_info_logging: bool,
) -> Result<U256> {
    let simulation_start = std::time::Instant::now();

    if enable_info_logging {
        log::info!(
            "🚀 Starting simulation - Pool: {}, Token A: {}, Token B: {}, Amount: {} wei, Fee: {}",
            target_pool,
            token_a,
            token_b,
            amount,
            fee
        );
    }

    // Cache block data to avoid repeated network calls
    use std::sync::LazyLock;
    use std::sync::Mutex;

    static CACHED_BLOCK_DATA: LazyLock<Mutex<Option<(u64, u64, U256, u64, std::time::Instant)>>> =
        LazyLock::new(|| Mutex::new(None));

    let (latest_gas_limit, latest_gas_price) = {
        // Check cache first and release lock immediately
        let cache_data = {
            let cache = CACHED_BLOCK_DATA.lock().unwrap();
            cache.clone()
        };

        // Use cached data if less than 11 seconds old (one block)
        if let Some((_, _, gas_price, gas_limit, cached_at)) = cache_data {
            if cached_at.elapsed().as_secs() < 11 {
                (gas_limit, gas_price)
            } else {
                // Refresh cache - do network calls without holding lock
                let latest_block_number = provider.get_block_number().await?;
                log::debug!("got block number: {:?}", latest_block_number);

                let block_id = BlockId::from_str(latest_block_number.to_string().as_str())
                    .map_err(|e| anyhow::anyhow!("Invalid block number format: {}", e))?;
                let latest_block = provider
                    .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
                    .await?
                    .ok_or(anyhow::Error::msg("Error getting block"))?;

                let gas_limit = latest_block.header.gas_limit;
                let gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
                let timestamp = latest_block.header.timestamp;

                // Update cache with new data
                {
                    let mut cache = CACHED_BLOCK_DATA.lock().unwrap();
                    *cache = Some((
                        latest_block_number,
                        timestamp,
                        gas_price,
                        gas_limit,
                        std::time::Instant::now(),
                    ));
                }

                log::debug!("Refreshed block cache in: {:?}", simulation_start.elapsed());
                (gas_limit, gas_price)
            }
        } else {
            log::debug!("Getting new block data");
            // Initialize cache - do network calls without holding lock
            let latest_block_number = provider.get_block_number().await?;
            let block_id = BlockId::from_str(latest_block_number.to_string().as_str())
                .map_err(|e| anyhow::anyhow!("Invalid block number format: {}", e))?;
            let latest_block = provider
                .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
                .await?
                .ok_or(anyhow::Error::msg("Error getting block"))?;

            let gas_limit = latest_block.header.gas_limit;
            let gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
            let timestamp = latest_block.header.timestamp;

            // Store in cache
            {
                let mut cache = CACHED_BLOCK_DATA.lock().unwrap();
                *cache = Some((
                    latest_block_number,
                    timestamp,
                    gas_price,
                    gas_limit,
                    std::time::Instant::now(),
                ));
            }

            (gas_limit, gas_price)
        }
    };

    let wallet_address = simulator.owner;

    // Fast initial balance check
    let weth_balance = check_weth_balance_optimized(
        wallet_address,
        simulator,
        &latest_gas_limit,
        &latest_gas_price,
    )
    .await
    .inspect_err(|e| log::debug!("Error getting weth balance {:?}", e))?;

    alloy::sol! {
        #[derive(Debug)]
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
        ) external;
    };

    let function_call = flashSwap_V3_to_V2Call {
        pool0: target_pool,
        fee1: fee,
        tokenIn: token_a,
        tokenOut: token_b,
        amountIn: amount,
    };

    let function_call_data = function_call.abi_encode();

    let caller = simulator.owner;
    let contract_address = simulator.contract_address;

    // Create the transaction
    let new_tx = Tx {
        caller,
        transact_to: contract_address,
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.call(new_tx).inspect_err(|e| {
        let error_str = format!("{:?}", e);
        if error_str.contains("EVM REVERT:") {
            if let Some(start) = error_str.find("0x") {
                if let Some(end) = error_str[start..].find(" / Gas used:") {
                    let hex_data = &error_str[start..start + end];
                    if let Ok(decoded) = crate::common::decode_result::decode_revert_hex(hex_data) {
                        error!("Decoded EVM error: {}", decoded);
                    } else {
                        error!("Failed to decode revert data: {}", hex_data);
                    }
                }
            }
        }
    })?;

    // Fast final balance check
    //    let balance = check_weth_balance_optimized(
    //        wallet_address,
    //        simulator,
    //        &latest_gas_limit,
    //        &latest_gas_price,
    //    )
    //    .await
    //    .inspect_err(|e| log::debug!("Error checking weth balance {e}",))?;
    //
    //    let profit = balance - weth_balance;

    let token = token_b
        .const_eq(&get_address(AddressType::Weth))
        .then_some(token_a)
        .unwrap_or(token_b);

    let token_balance = get_token_balance(simulator, token, simulator.owner).await?;

    let simulation_duration = simulation_start.elapsed();
    if enable_info_logging {
        log::info!(
            "✅ Simulation complete - Profit: {} wei, Duration: {:?}",
            token_balance,
            simulation_duration
        );
    }

    Ok(token_balance)
}

// Optimized balance check function
async fn check_weth_balance_optimized(
    wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
) -> Result<U256, anyhow::Error> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: get_address(AddressType::Weth),
        data: balanceOfCall {
            account: wallet_address,
        }
        .abi_encode()
        .into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let result = simulator
        .call(new_tx)
        .inspect_err(|e| log::debug!("There was an error {e}"))?;

    let balance = U256::from_be_slice(&result.output);

    Ok(balance)
}

pub fn one_ether() -> U256 {
    U256::from(10).pow(U256::from(18)) // 1e18
}

pub fn one_hundred_ether() -> U256 {
    U256::from(100) * U256::from(10).pow(U256::from(18)) // 100e18
}

pub fn fify_thousand_eth() -> U256 {
    U256::from(50000) * U256::from(10).pow(U256::from(18)) // 50000e18
}

pub fn five_hundred_eth() -> U256 {
    U256::from(500) * U256::from(10).pow(U256::from(18)) // 500e18
}

pub fn one_thousand_eth() -> U256 {
    U256::from(1000) * U256::from(10).pow(U256::from(18)) // 1000e18
}

pub fn five_hundred_thousand_eth() -> U256 {
    U256::from(50000) * U256::from(10).pow(U256::from(18)) // 50000e18
}

pub fn me() -> Address {
    address!("0000000000000000000000000000000000000001")
}

pub enum AddressType {
    Weth,
    V3Router,
    V2Router,
    V2Factory,
    V3Factory,
    V2Quoter,
    V3Quoter,
    UniswapV2Router,
    UniswapV3Router,
    Usdc,
}

pub fn get_address(address_type: AddressType) -> Address {
    match address_type {
        AddressType::Weth => address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        AddressType::V3Router => address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
        AddressType::V2Router => address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        AddressType::UniswapV2Router => address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        AddressType::UniswapV3Router => address!("E592427A0AEce92De3Edee1F18E0157C05861564"),
        AddressType::V3Factory => address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
        AddressType::V2Factory => address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
        AddressType::V2Quoter => address!("61fFE014bA17989E743c5F6cB21bF9697530B21e"),
        AddressType::V3Quoter => address!("61fFE014bA17989E743c5F6cB21bF9697530B21e"),
        AddressType::Usdc => address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
    }
}

pub enum MockAddress {
    UniV2,
    UniV3,
}

pub fn mock_addresses(address_type: MockAddress) -> Address {
    match address_type {
        MockAddress::UniV2 => address!("d3d2E2692501A5c9Ca623199D38826e513033a17"),
        MockAddress::UniV3 => address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801"),
    }
}

pub fn arboo_bytecode() -> Bytecode {
    // Read bytecode from hex file
    let hex_content = std::fs::read_to_string("src/bytecode/updated_arbitrage.hex")
        .expect("Failed to read arbitrage contract bytecode from hex file");

    // Remove 0x prefix if present and any whitespace
    let hex_str = hex_content.trim().trim_start_matches("0x");

    let bytes = hex::decode(hex_str).expect("Invalid hex string in bytecode file");
    Bytecode::new_raw(bytes.into())
}

pub fn v2_flash_to_v3_swap_bytecode() -> Bytecode {
    // Read bytecode from hex file
    let hex_content = std::fs::read_to_string("src/bytecode/v2_flash_to_v3_swap.hex")
        .expect("Failed to read V2FlashToV3Swap contract bytecode from hex file");

    // Remove 0x prefix if present and any whitespace
    let hex_str = hex_content.trim().trim_start_matches("0x");

    let bytes = hex::decode(hex_str).expect("Invalid hex string in bytecode file");
    Bytecode::new_raw(bytes.into())
}

pub async fn check_weth_balance(
    wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
    caller: Option<Address>,
) -> Result<U256, anyhow::Error> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    let function_call = balanceOfCall {
        account: wallet_address,
    };

    let function_call_data = function_call.abi_encode();

    let caller = caller.unwrap_or(wallet_address);

    let new_tx = Tx {
        caller,
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let result = simulator
        .call(new_tx)
        .inspect_err(|e| log::debug!("There was an error {e}"))?;

    let balance = U256::from_be_slice(&result.output);

    Ok(balance)
}

#[derive(Debug)]
pub enum ParserType {
    UTF8,
    U256,
}

#[derive(Debug)]
pub struct ParserInput<'a> {
    parser_type: ParserType,
    data: &'a [u8],
}

pub fn parse_data(inputs: Vec<ParserInput>) -> Vec<String> {
    inputs
        .iter()
        .map(|input| match input.parser_type {
            ParserType::UTF8 => String::from_utf8(input.data.to_vec())
                .unwrap_or_else(|_| "Invalid UTF-8".to_string()),
            ParserType::U256 => U256::from_be_slice(input.data).to_string(),
        })
        .collect()
}
async fn get_token_balance(
    simulator: &mut EvmSimulator<'_>,
    token: Address,
    account: Address,
) -> Result<U256> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    let balance_call = balanceOfCall { account };
    let call_data = balance_call.abi_encode();

    let tx = Tx {
        caller: simulator.owner,
        transact_to: token,
        data: call_data.into(),
        value: U256::ZERO,
        gas_limit: 100_000,
        gas_price: U256::from(20_000_000_000u64),
    };

    match simulator.staticcall(tx) {
        Ok(result) => {
            if result.output.len() >= 32 {
                Ok(U256::from_be_slice(&result.output[..32]))
            } else {
                Ok(U256::ZERO)
            }
        }
        Err(_) => Ok(U256::ZERO),
    }
}
