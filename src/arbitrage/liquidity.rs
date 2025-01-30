use crate::common::revm::{EvmSimulator, Tx};
use alloy::eips::BlockId;
use alloy::network::primitives::BlockTransactionsKind;
use alloy::network::AnyNetwork;
use alloy::primitives::{address as alloy_address, Address as alloy_Address, Signed, U160, U64};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol_types::SolCall;
use anyhow::Result;
use revm::db::{CacheDB, EmptyDB};
use revm::primitives::{address, AccountInfo, Address, Bytecode, Bytes, FixedBytes, U256};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub async fn liquidity() -> Result<()> {
    // Initialize WebSocket provider
    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider: RootProvider<PubSubFrontend, AnyNetwork> =
        ProviderBuilder::new().network().on_ws(ws_client).await?;

    let provider = Arc::new(provider);
    let mut mem_db = CacheDB::new(EmptyDB::default());

    // Fetch the latest block number
    let latest_block_number = provider.get_block_number().await?;
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, BlockTransactionsKind::Full)
        .await?;
    // println!("Latest Block: {:?}", latest_block);

    // Initialize the database with the current state
    let latest_block = provider
        .get_block(latest_block_number.into(), BlockTransactionsKind::Full)
        .await?
        .unwrap();
    let pool_500_addr_parsed =
        alloy::primitives::Address::from_str(pool_500_addr().to_string().as_str()).unwrap();
    let contract_code = provider.get_code_at(pool_500_addr_parsed).await?;
    let contract_code = contract_code.0;

    let account_info_500 = AccountInfo::new(
        U256::ZERO,
        0,
        FixedBytes::ZERO,
        Bytecode::new_raw(revm::precompile::Bytes(contract_code)),
    );
    mem_db.insert_account_info(pool_500_addr(), account_info_500);

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    println!("Latest Block Number: {:?}", latest_block_number);
    println!("Latest gas price : {:?}", latest_gas_price);
    let wallet = PrivateKeySigner::random();
    let wallet_address = wallet.address();
    let wallet_address_revm =
        revm::primitives::Address::from_str(wallet_address.to_string().as_str()).unwrap();

    // Create the EVM simulator
    let simulator = Arc::new(Mutex::new(EvmSimulator::new(
        provider,
        Some(wallet_address_revm),
        U64::from(latest_block_number),
    )));

    // // Set initial ETH value
    let initial_eth_balance = U256::from(1_000_000_000_000_000_000 as u64); // Example initial balance (1 ETH)
    simulator
        .clone()
        .lock()
        .unwrap()
        .set_eth_balance(wallet_address, initial_eth_balance);

    let amount_in = Signed::<256, 4>::from_str("1_000_000_000_000_000_000").unwrap();
    // Create a uint160 value for sqrtPriceLimitX96
    let sqrt_price_limit_x96 = U160::MAX; // Use U160 directly

    // get the balance of my wallet
    let balance = simulator
        .clone()
        .lock()
        .unwrap()
        .get_eth_balance(wallet_address_revm);

    println!("Balance: {:?}", balance);

    alloy::sol! {
        interface IUniswapV3Pool {
            function slot0()
                external
                view
                returns (
                    uint160 sqrtPriceX96,
                    int24 tick,
                    uint16 observationIndex,
                    uint16 observationCardinality,
                    uint16 observationCardinalityNext,
                    uint8 feeProtocol,
                    bool unlocked
                );

            function liquidity() external view returns (uint128);

            function ticks(int24 tick) external view returns (
                uint128 liquidityGross,
                int128 liquidityNet,
                uint256 feeGrowthOutside0X128,
                uint256 feeGrowthOutside1X128,
                int56 tickCumulativeOutside,
                uint160 secondsPerLiquidityOutsideX128,
                uint32 secondsOutside,
                bool initialized
            );
        }
    }

    // Call the liquidity function
    let liquidity_call = IUniswapV3Pool::liquidityCall {};
    let liquidity_call_data = liquidity_call.abi_encode();

    let tx = Tx {
        caller: wallet_address_revm,
        transact_to: pool_500_addr(),
        data: Bytes::from(liquidity_call_data),
        value: U256::ZERO,
        gas_price: latest_gas_price,
        gas_limit: latest_gas_limit,
    };

    let liquidity = simulator.clone().lock().unwrap().call(tx).unwrap();
    println!("Liquidity: {:?}", liquidity);

    Ok(())
}

// WETH/USDC fee 500
pub fn pool_500_addr() -> Address {
    address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
}

pub fn weth_address() -> alloy_Address {
    alloy_address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
}

pub fn usdc_address() -> alloy_Address {
    alloy_address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

pub fn usdc_weth_pool_500_addr() -> alloy_Address {
    alloy_address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
}
