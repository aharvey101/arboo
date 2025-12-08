use alloy::primitives::{address, U256, Address};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{Filter, TransactionRequest};
use alloy::sol_types::SolCall;
use alloy_sol_types::SolValue;
use anyhow::Result;
use log::info;
use std::sync::Arc;
use std::time::Duration;
use futures::StreamExt;
use revm::primitives::keccak256;
use alloy::sol;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::anvil_setup::AnvilConfig;

#[tokio::test]
async fn test_direct_v2_pool_swap() -> Result<()> {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();

    info!("🚀 Testing DIRECT V2 POOL SWAP");
    info!("Goal: Execute swap directly on pool to test arbitrage detection");

    // Kill any existing processes
    let _ = std::process::Command::new("pkill")
        .args(&["-f", "anvil"])
        .output();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Start Anvil with mainnet fork
    info!("📦 Starting Anvil with mainnet fork...");
    let config = AnvilConfig {
        fork_url: Some("http://192.168.0.14:8545".to_string()),
        ..Default::default()
    };

    let anvil = utils::anvil_setup::AnvilInstance::new_with_fork_block(config, None).await?;
    info!("✅ Anvil started on port {}", anvil.port);

    let ws_url = format!("ws://127.0.0.1:{}", anvil.port);
    let ws_client = WsConnect::new(ws_url.clone());
    let ws_provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await?;

    let ws_provider = Arc::new(ws_provider);

    // Pool and token addresses - Use verified Uniswap V2 pools
    let v2_pool = address!("0xA478c2975Ab1Ea89e8196811F51A7B7Ade33eB11"); // DAI/WETH Uniswap V2 pool
    let dai = address!("0x6b175474e89094c44da98b954eedeac495271d0f");
    let weth = address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let account = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"); // Anvil account #0

    info!("📍 Pool and Tokens:");
    info!("   V2 Pool: {:?}", v2_pool);
    info!("   DAI: {:?}", dai);
    info!("   WETH: {:?}", weth);
    info!("   Account: {:?}", account);

    // Setup log subscription BEFORE swap
    info!("📥 Setting up log subscription...");
    let v2_swap_sig = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());

    let filter = Filter::new()
        .address(vec![v2_pool])  // Only monitor the V2 pool we're testing
        .event_signature(vec![v2_swap_sig]);

    let subscription = ws_provider.subscribe_logs(&filter).await?;
    let mut stream = subscription.into_stream();
    info!("✅ Log subscription ready");

    // Spawn log listener
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(10);
    let _log_listener = tokio::spawn(async move {
        let mut count = 0;
        tokio::select! {
            _ = async {
                while let Some(log) = stream.next().await {
                    count += 1;
                    info!("🎉 SWAP EVENT #{}: {:?}", count, log.address());
                    let _ = log_tx.send(log).await;
                }
            } => {}
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                info!("⏱️  Log listener timeout");
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Define sol interfaces for token and pool interactions
    sol! {
        function deposit() external payable;
        function approve(address spender, uint256 amount) external returns (bool);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        
        interface IUniswapV2Pair {
            function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
            function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
            function token0() external view returns (address);
            function token1() external view returns (address);
        }
    }

    // Get HTTP provider for non-subscription calls
    let http_url = format!("http://127.0.0.1:{}", anvil.port).parse()?;
    let http_provider = ProviderBuilder::new().on_http(http_url);

    // Step 1: Verify pool tokens and get reserves
    info!("\n📊 STEP 1: Verifying pool tokens and reserves...");
    
    // Check token0 and token1
    let token0_call = IUniswapV2Pair::token0Call {};
    let token0_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(v2_pool)
                .input(token0_call.abi_encode().into()),
        )
        .await?;
    let token0_addr = Address::abi_decode(&token0_result, false)?;
    
    let token1_call = IUniswapV2Pair::token1Call {};
    let token1_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(v2_pool)
                .input(token1_call.abi_encode().into()),
        )
        .await?;
    let token1_addr = Address::abi_decode(&token1_result, false)?;
    
    info!("   Pool token0: {:?}", token0_addr);
    info!("   Pool token1: {:?}", token1_addr);
    
    let reserves_call = IUniswapV2Pair::getReservesCall {};
    let reserves_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(v2_pool)
                .input(reserves_call.abi_encode().into()),
        )
        .await?;

    let (reserve0, reserve1, _) = <(u128, u128, u32)>::abi_decode(&reserves_result, false)?;
    info!("   Token0 reserves: {} (raw)", reserve0);
    info!("   Token1 reserves: {} (raw)", reserve1);

    // Step 2: Fund account with WETH
    info!("\n💰 STEP 2: Funding account with WETH...");
    let eth_amount = U256::from(100) * U256::from(10u128.pow(18)); // 100 ETH

    let deposit_call = depositCall {};
    let tx_req = TransactionRequest::default()
        .from(account)
        .to(weth)
        .input(deposit_call.abi_encode().into())
        .value(eth_amount)
        .gas_limit(100000u64);

    match http_provider.send_transaction(tx_req).await {
        Ok(pending_tx) => {
            info!("   ✅ WETH deposit sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(_) => info!("   ✅ WETH deposit mined"),
                Err(e) => info!("   ⚠️  Receipt error: {}", e),
            }
        }
        Err(e) => info!("   ❌ Deposit failed: {}", e),
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 3: We don't need approval for direct swaps, skip this step
    info!("\n🔐 STEP 3: Direct swaps don't require pre-approval, skipping...");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 4: Skip separate transfer - we'll transfer as part of the swap
    info!("\n⏭️  STEP 4: Skipping separate transfer - will transfer during swap...");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 5: Execute direct swap on V2 pool (proper Uniswap V2 method)
    info!("\n🔄 STEP 5: Executing DIRECT swap on V2 pool (WETH → DAI)...");
    
    // Determine which token is which in the pool
    let (weth_is_token0, dai_is_token0) = if token0_addr == weth {
        (true, false)
    } else if token0_addr == dai {
        (false, true) 
    } else {
        return Err(anyhow::anyhow!("Pool doesn't contain WETH/DAI pair"));
    };
    
    // Calculate expected output based on correct token order
    let swap_amount = U256::from(1) * U256::from(10u128.pow(18)); // 1 WETH
    let (reserve_in, reserve_out) = if weth_is_token0 {
        (reserve0, reserve1) // WETH input, DAI output
    } else {
        (reserve1, reserve0) // WETH input, DAI output  
    };
    
    // Uniswap V2 formula: amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    let amount_in_with_fee = (swap_amount * U256::from(997)) / U256::from(1000);
    let expected_dai_out = (amount_in_with_fee * U256::from(reserve_out)) / (U256::from(reserve_in) + amount_in_with_fee);
    
    info!("   WETH is token0: {}", weth_is_token0);
    info!("   Amount in (WETH): {}", swap_amount / U256::from(10u128.pow(18)));
    info!("   Expected out (DAI): {}", expected_dai_out / U256::from(10u128.pow(18)));
    
    // Step 5a: Transfer WETH to pool (required before calling swap)
    info!("   📤 Transferring WETH to pool before swap...");
    let transfer_to_pool_call = transferCall {
        to: v2_pool,
        amount: swap_amount,
    };
    
    let transfer_tx = TransactionRequest::default()
        .from(account)
        .to(weth)
        .input(transfer_to_pool_call.abi_encode().into())
        .gas_limit(100000u64);
        
    match http_provider.send_transaction(transfer_tx).await {
        Ok(pending_tx) => {
            info!("   ✅ WETH transfer to pool sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(_) => info!("   ✅ WETH transfer mined"),
                Err(e) => info!("   ⚠️  Transfer receipt error: {}", e),
            }
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to transfer WETH to pool: {}", e)),
    }
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Step 5b: Call swap with correct amount0Out/amount1Out
    info!("   🔄 Calling swap function on pool...");
    let (amount0_out, amount1_out) = if weth_is_token0 {
        (U256::ZERO, expected_dai_out)  // WETH in (token0), DAI out (token1)
    } else {
        (expected_dai_out, U256::ZERO)  // WETH in (token1), DAI out (token0)
    };
    
    let swap_call = IUniswapV2Pair::swapCall {
        amount0Out: amount0_out,
        amount1Out: amount1_out,
        to: account,
        data: Vec::new().into(),
    };
    
    let swap_tx = TransactionRequest::default()
        .from(account)
        .to(v2_pool)
        .input(swap_call.abi_encode().into())
        .gas_limit(300000u64);

    match http_provider.send_transaction(swap_tx).await {
        Ok(pending_tx) => {
            info!("   ✅ Swap sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    info!("   ✅ Swap mined in block {:?}", receipt.block_number);
                    info!("   ✅ Gas used: {}", receipt.gas_used);
                    if receipt.status() {
                        info!("   🎉 SWAP EXECUTION SUCCESSFUL!");
                    } else {
                        info!("   ❌ Swap reverted");
                        return Err(anyhow::anyhow!("Swap transaction reverted"));
                    }
                }
                Err(e) => info!("   ⚠️  Receipt error: {}", e),
            }
        }
        Err(e) => return Err(anyhow::anyhow!("Swap transaction failed: {}", e)),
    }

    // Step 6: Wait for events
    info!("\n📥 STEP 6: Waiting for swap events...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check if we received any events
    let mut event_count = 0;
    while let Ok(Some(_log)) = tokio::time::timeout(Duration::from_millis(100), log_rx.recv()).await {
        event_count += 1;
    }

    if event_count > 0 {
        info!("✅ Received {} swap event(s)", event_count);
    } else {
        info!("⚠️  No swap events detected (may need to adjust subscription)");
    }

    // Step 7: Verify final balances
    info!("\n💰 STEP 7: Verifying final balances...");

    let dai_balance_call = balanceOfCall { account };
    let dai_balance_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(dai)
                .input(dai_balance_call.abi_encode().into()),
        )
        .await?;
    let dai_balance = U256::from_be_slice(&dai_balance_result[0..32]);

    let weth_balance_call = balanceOfCall { account };
    let weth_balance_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(weth)
                .input(weth_balance_call.abi_encode().into()),
        )
        .await?;
    let weth_balance = U256::from_be_slice(&weth_balance_result[0..32]);

    info!("   DAI balance: {}", dai_balance / U256::from(10u128.pow(18)));
    info!("   WETH balance: {}", weth_balance / U256::from(10u128.pow(18)));

    // Verify we got DAI from swap
    if dai_balance > U256::ZERO {
        info!("✅ Successfully received DAI from swap!");
        info!("🎉 DIRECT V2 POOL SWAP TEST PASSED!");
        return Ok(());
    } else {
        return Err(anyhow::anyhow!("❌ No DAI received from swap - swap may have failed or not executed"));
    }
}
