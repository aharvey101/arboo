use alloy::primitives::address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy_primitives::U256;
use anyhow::Result;
use log::info;
use std::sync::Arc;
use std::time::Duration;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}

use alloy::providers::ProviderBuilder;
use utils::anvil_setup::AnvilConfig;

#[tokio::test]
async fn test_simple_arbitrage_detection() -> Result<()> {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    info!("🚀 Starting SIMPLE arbitrage detection test");

    // Kill any existing processes
    let _ = std::process::Command::new("pkill")
        .args(&["-f", "anvil|arboo"])
        .output();
    std::thread::sleep(Duration::from_secs(2));

    // Setup Anvil with mainnet fork
    info!("📦 Setting up Anvil with mainnet fork");
    let config = AnvilConfig {
        fork_url: Some("http://192.168.0.14:8545".to_string()),
        ..Default::default()
    };

    let anvil = utils::anvil_setup::AnvilInstance::new_with_fork_block(config, None).await?;
    info!("✅ Anvil started on port {}", anvil.port);

    let ws_url = format!("ws://127.0.0.1:{}", anvil.port);
    let ws_client = WsConnect::new(ws_url.clone());
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;

    let provider = Arc::new(provider);

    // Step 1: Check initial block
    let initial_block = provider.get_block_number().await?;
    info!("📍 Initial block: {}", initial_block);

    // Step 2: Check WETH/USDC pools on mainnet fork
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

    info!("🔍 STEP 1: Check pool reserves");
    info!("  WETH: {:#x}", weth);
    info!("  USDC: {:#x}", usdc);

    // Step 3: Try to find WETH-USDC pools from cache
    // V2 WETH-USDC pool from cache (ID 4202): 0xbd504d0a4b16a77e531722c3aea770161347dea7
    // V3 WETH-USDC pool from cache (ID 23331): 0xa6d2aac68cafa72c5249aa4d0a379390fe1d40d8
    let uniswap_v2_weth_usdc = address!("bd504d0a4b16a77e531722c3aea770161347dea7");
    let uniswap_v3_weth_usdc_10 = address!("a6d2aac68cafa72c5249aa4d0a379390fe1d40d8");

    info!("🔍 STEP 2: Check if pools exist on fork");
    info!("  V2 WETH-USDC (from cache): {:#x}", uniswap_v2_weth_usdc);
    info!("  V3 WETH-USDC 0.001% (from cache): {:#x}", uniswap_v3_weth_usdc_10);

    // Check V2 pool code
    let v2_code = provider.get_code_at(uniswap_v2_weth_usdc).await?;
    info!("  V2 pool code size: {} bytes", v2_code.len());

    // Check V3 pool code
    let v3_code = provider.get_code_at(uniswap_v3_weth_usdc_10).await?;
    info!("  V3 pool code size: {} bytes", v3_code.len());

    // Step 4: Check pool reserves by calling getReserves
    info!("🔍 STEP 3: Query pool reserves");

    // Get V2 reserves
    let v2_reserves = get_v2_reserves(&provider, uniswap_v2_weth_usdc).await?;
    info!("  V2 Reserves: reserve0={}, reserve1={}", v2_reserves.0, v2_reserves.1);

    // Get V3 reserves
    let v3_reserves = get_v3_reserves(&provider, uniswap_v3_weth_usdc_10).await?;
    info!("  V3 Reserves: reserve0={}, reserve1={}", v3_reserves.0, v3_reserves.1);

    // Step 5: Calculate price difference
    info!("🔍 STEP 4: Calculate price ratios");
    if v2_reserves.0 > U256::ZERO && v3_reserves.0 > U256::ZERO {
        let v2_price = (v2_reserves.1 * U256::from(1e18 as u128)) / v2_reserves.0;
        let v3_price = (v3_reserves.1 * U256::from(1e18 as u128)) / v3_reserves.0;

        info!("  V2 price (USDC/WETH): {}", v2_price);
        info!("  V3 price (USDC/WETH): {}", v3_price);

        let difference = if v2_price > v3_price {
            ((v2_price - v3_price) * U256::from(10000)) / v2_price
        } else {
            ((v3_price - v2_price) * U256::from(10000)) / v3_price
        };

        let diff_basis_points = if difference > U256::from(u64::MAX) {
            10000.0 // Cap at 100%
        } else {
            difference.to::<u64>() as f64
        };
        info!("  Price difference: {:.2}%", diff_basis_points / 100.0);

        if difference > U256::from(100) {
            info!("✅ Significant price difference detected (>1%)!");
        } else {
            info!("⚠️  Price difference is small (<1%)");
        }
    }

    info!("✅ SIMPLE TEST COMPLETE");
    Ok(())
}

async fn get_v2_reserves(
    provider: &Arc<RootProvider<PubSubFrontend>>,
    pool_address: alloy::primitives::Address,
) -> Result<(U256, U256)> {
    use alloy::sol_types::SolCall;

    alloy::sol! {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    let call_data = getReservesCall {}.abi_encode();

    let result = provider
        .call(
            &alloy::rpc::types::TransactionRequest::default()
                .to(pool_address)
                .input(call_data.into()),
        )
        .await?;

    if result.len() >= 32 {
        let reserve0 = U256::from_be_slice(&result[0..32]);
        let reserve1 = U256::from_be_slice(&result[32..64]);
        Ok((reserve0, reserve1))
    } else {
        Err(anyhow::anyhow!("Invalid reserves response"))
    }
}

async fn get_v3_reserves(
    provider: &Arc<RootProvider<PubSubFrontend>>,
    pool_address: alloy::primitives::Address,
) -> Result<(U256, U256)> {
    use alloy::sol_types::SolCall;

    alloy::sol! {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
    }

    let call_data = slot0Call {}.abi_encode();

    let result = provider
        .call(
            &alloy::rpc::types::TransactionRequest::default()
                .to(pool_address)
                .input(call_data.into()),
        )
        .await?;

    // For V3, we'll approximate reserves from liquidity
    // Just return dummy values for now to see if we can get here
    Ok((U256::from(1), U256::from(1)))
}
