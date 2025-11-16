use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{Filter, TransactionRequest};
use alloy::primitives::U256;
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

    // Pool and token addresses
    let v2_pool = address!("0x0606c53d3ddda7fbcdfea72bbb540ce1cfd29b84"); // DAI/WETH 0.3%
    let v3_pool = address!("0xb9c7807d2428dc9d5fb6dcdd56aec89d204c64a9"); // DAI/WETH 0.01%
    let dai = address!("0x6b175474e89094c44da98b954eedeac495271d0f");
    let weth = address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let account = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"); // Anvil account #0

    info!("📍 Pools and Tokens:");
    info!("   V2 Pool: {:?}", v2_pool);
    info!("   V3 Pool: {:?}", v3_pool);
    info!("   DAI: {:?}", dai);
    info!("   WETH: {:?}", weth);
    info!("   Account: {:?}", account);

    // Setup log subscription BEFORE swap
    info!("📥 Setting up log subscription...");
    let v2_swap_sig = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_sig = keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)".as_bytes());

    let filter = Filter::new()
        .address(vec![v2_pool, v3_pool])
        .event_signature(vec![v2_swap_sig, v3_swap_sig]);

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
        }
    }

    // Get HTTP provider for non-subscription calls
    let http_url = format!("http://127.0.0.1:{}", anvil.port).parse()?;
    let http_provider = ProviderBuilder::new().on_http(http_url);

    // Step 1: Get pool reserves before swap
    info!("\n📊 STEP 1: Checking initial pool reserves...");
    let reserves_call = IUniswapV2Pair::getReservesCall {};
    let reserves_result = http_provider
        .call(
            &TransactionRequest::default()
                .to(v2_pool)
                .input(reserves_call.abi_encode().into()),
        )
        .await?;

    let (reserve0, reserve1, _) = <(u128, u128, u32)>::abi_decode(&reserves_result, false)?;
    info!("   DAI reserves: {} (raw)", reserve0);
    info!("   WETH reserves: {} (raw)", reserve1);

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

    // Step 3: Approve pool to spend WETH
    info!("\n🔐 STEP 3: Approving pool to spend WETH...");
    let approve_call = approveCall {
        spender: v2_pool,
        amount: eth_amount,
    };

    let tx_req = TransactionRequest::default()
        .from(account)
        .to(weth)
        .input(approve_call.abi_encode().into())
        .gas_limit(100000u64);

    match http_provider.send_transaction(tx_req).await {
        Ok(pending_tx) => {
            info!("   ✅ Approval sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(_) => info!("   ✅ Approval mined"),
                Err(e) => info!("   ⚠️  Receipt error: {}", e),
            }
        }
        Err(e) => info!("   ❌ Approval failed: {}", e),
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 4: Transfer WETH to pool
    info!("\n🔄 STEP 4: Transferring WETH to pool...");
    let transfer_call = transferCall {
        to: v2_pool,
        amount: eth_amount / U256::from(2), // Transfer 50 WETH to test
    };

    let tx_req = TransactionRequest::default()
        .from(account)
        .to(weth)
        .input(transfer_call.abi_encode().into())
        .gas_limit(100000u64);

    let transfer_amount = eth_amount / U256::from(2);
    match http_provider.send_transaction(tx_req).await {
        Ok(pending_tx) => {
            info!("   ✅ Transfer sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(_) => info!("   ✅ Transfer mined - {} WETH in pool", transfer_amount / U256::from(10u128.pow(18))),
                Err(e) => info!("   ⚠️  Receipt error: {}", e),
            }
        }
        Err(e) => info!("   ❌ Transfer failed: {}", e),
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 5: Execute direct swap on V2 pool
    info!("\n🔄 STEP 5: Executing DIRECT swap on V2 pool...");
    info!("   Swap: WETH → DAI");

    // Calculate expected DAI output using Uniswap formula
    // amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    let amount_in = transfer_amount;
    let amount_in_with_fee = (amount_in * U256::from(997)) / U256::from(1000);
    let amount_out = (amount_in_with_fee * U256::from(reserve0)) / (U256::from(reserve1) + amount_in_with_fee);

    info!("   Amount in (WETH): {}", amount_in / U256::from(10u128.pow(18)));
    info!("   Expected out (DAI): {}", amount_out / U256::from(10u128.pow(18)));

     // Call swap directly on pool
     // swap(uint amount0Out, uint amount1Out, address to, bytes calldata data)
     // amount0Out = DAI output, amount1Out = 0
     let swap_data = TransactionRequest::default()
         .from(account)
         .to(v2_pool)
         .input(
             IUniswapV2Pair::swapCall {
                 amount0Out: amount_out,
                 amount1Out: U256::ZERO,
                 to: account,
                 data: Vec::new().into(),
             }
             .abi_encode()
             .into(),
         )
         .gas_limit(300000u64);

    match http_provider.send_transaction(swap_data).await {
        Ok(pending_tx) => {
            info!("   ✅ Swap sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    info!("   ✅ Swap mined in block {:?}", receipt.block_number);
                    info!("   ✅ Gas used: {}", receipt.gas_used);
                    if receipt.status() {
                        info!("   ✅ SWAP EXECUTION SUCCESSFUL!");
                    } else {
                        info!("   ⚠️  Swap reverted - insufficient liquidity or slippage?");
                    }
                }
                Err(e) => info!("   ⚠️  Receipt error: {}", e),
            }
        }
        Err(e) => info!("   ❌ Swap failed: {}", e),
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
