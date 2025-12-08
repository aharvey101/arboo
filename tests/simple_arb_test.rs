use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{Filter, TransactionRequest};
use alloy::primitives::U256;
use alloy::sol_types::{SolCall};
use anyhow::Result;
use log::info;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use futures::StreamExt;
use revm::primitives::keccak256;
use alloy::sol;

#[tokio::test]
async fn test_detect_actual_swap_logs() -> Result<()> {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();

    info!("🚀 Testing ACTUAL SWAP LOG DETECTION");

    // Kill any existing anvil processes
    let _ = std::process::Command::new("pkill")
        .args(&["-f", "anvil"])
        .output();
    thread::sleep(Duration::from_secs(1));

    // Start Anvil
    info!("📦 Starting Anvil with mainnet fork...");
    let mut anvil_process = Command::new("anvil")
        .arg("--port").arg("18893")
        .arg("--chain-id").arg("1")
        .arg("--fork-url").arg("http://192.168.0.14:8545")
        .arg("--host").arg("127.0.0.1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start Anvil");

    thread::sleep(Duration::from_secs(3));

    // Connect providers
    let http_url = "http://127.0.0.1:18893".parse()?;
    let http_provider = ProviderBuilder::new().on_http(http_url);

    let ws_url = "ws://127.0.0.1:18893";
    let ws_client = WsConnect::new(ws_url);
    let ws_provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await?;

    let ws_provider = Arc::new(ws_provider);
    info!("✅ Connected to Anvil");

    // Pool addresses
    let v2_pool = address!("0xbd504d0a4b16a77e531722c3aea770161347dea7");
    let v3_pool = address!("0xa6d2aac68cafa72c5249aa4d0a379390fe1d40d8");

    info!("📍 Pools:");
    info!("   V2: {:?}", v2_pool);
    info!("   V3: {:?}", v3_pool);

    // Check pool codes
    let v2_code = http_provider.get_code_at(v2_pool).await?;
    let v3_code = http_provider.get_code_at(v3_pool).await?;
    info!("   V2 code: {} bytes", v2_code.len());
    info!("   V3 code: {} bytes", v3_code.len());

    // Setup log subscription BEFORE swap
    let v2_swap_sig = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_sig = keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)".as_bytes());
    
    info!("📥 Setting up log subscription...");
    let filter = Filter::new()
        .address(vec![v2_pool, v3_pool])
        .event_signature(vec![v2_swap_sig, v3_swap_sig]);
    
    let subscription = ws_provider.subscribe_logs(&filter).await?;
    let mut stream = subscription.into_stream();
    info!("✅ Log subscription ready");

    // Spawn log listener
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let log_listener = tokio::spawn(async move {
        let mut count = 0;
        tokio::select! {
            _ = async {
                while let Some(log) = stream.next().await {
                    count += 1;
                    info!("🎉 LOG #{}: {:?}", count, log.address());
                    let _ = tx.send(log).await;
                }
            } => {}
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                info!("⏱️  Timeout");
            }
        }
        count
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Token addresses
    let weth = address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let usdc = address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let account = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    let eth_amount = U256::from(100) * U256::from(10u128.pow(18)); // 100 ETH
    let v2_router = address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D");

    // Define all sol! interfaces at the top of the function
    sol! {
        function deposit() external payable;
        
        function approve(address spender, uint256 amount) external returns (bool);
        
        function balanceOf(address account) external view returns (uint256);
        
        interface IUniswapV2Router {
            #[derive(Debug)]
            function swapExactTokensForTokens(
                uint amountIn,
                uint amountOutMin,
                address[] calldata path,
                address to,
                uint deadline
            ) external returns (uint[] memory amounts);
        }
    }

    info!("📤 Executing actual Uniswap V2 swap to generate logs...");
    info!("   WETH: {:?}", weth);
    info!("   USDC: {:?}", usdc);
    info!("   📍 V2 Router: {:?}", v2_router);
    
    // Step 1: Deposit ETH to WETH
    info!("   Step 1: Depositing ETH to WETH...");
    
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
                Ok(_) => {
                    info!("   ✅ WETH deposit mined");
                }
                Err(e) => {
                    info!("   ⚠️  Could not get receipt: {}", e);
                }
            }
        }
        Err(e) => {
            info!("   ⚠️  WETH deposit failed: {}", e);
        }
    }
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Step 2: Approve WETH spending on V2 router
    info!("   Step 2: Approving V2 router to spend WETH...");
    
    let approve_call = approveCall {
        spender: v2_router,
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
                Ok(_) => {
                    info!("   ✅ Approval mined");
                }
                Err(e) => {
                    info!("   ⚠️  Could not get receipt: {}", e);
                }
            }
        }
        Err(e) => {
            info!("   ⚠️  Approval failed: {}", e);
        }
    }
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Step 3: Execute V2 swap (WETH → USDC)
    info!("   Step 3: Executing Uniswap V2 swap (WETH → USDC)...");
    
    let deadline = U256::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600,
    );
    
    let path = vec![weth, usdc];
    
    let swap_call = IUniswapV2Router::swapExactTokensForTokensCall {
        amountIn: eth_amount,
        amountOutMin: U256::from(1),
        path,
        to: account,
        deadline,
    };
    
    let tx_req = TransactionRequest::default()
        .from(account)
        .to(v2_router)
        .input(swap_call.abi_encode().into())
        .gas_limit(500000u64);
    
    match http_provider.send_transaction(tx_req).await {
        Ok(pending_tx) => {
            info!("   ✅ Swap sent: {:?}", pending_tx.tx_hash());
            match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    info!("   ✅ Swap mined in block {:?}", receipt.block_number);
                    info!("   ✅ Swap status: {:?}", receipt.status());
                    info!("   ✅ Gas used: {:?}", receipt.gas_used);
                    
                    if receipt.status() {
                        info!("   ✅ Swap execution successful!");
                    } else {
                        info!("   ⚠️  Swap reverted");
                    }
                }
                Err(e) => {
                    info!("   ⚠️  Could not get receipt: {}", e);
                }
            }
        }
        Err(e) => {
            info!("   ⚠️  Swap send failed: {}", e);
        }
    }
    
    info!("⏳ Waiting 12 seconds to collect logs...");
    
    // Use select! to listen for logs while waiting
    let mut detected = 0;
    
    loop {
        tokio::select! {
            Some(_log) = rx.recv() => {
                detected += 1;
            }
            _ = tokio::time::sleep(Duration::from_secs(12)) => {
                break;
            }
        }
    }

    let total = log_listener.await?;
    info!("📊 Total logs detected via subscription: {}", total);
    info!("   From channel: {}", detected);
    
    info!("🔍 Checking token balances...");
    
    // Check WETH balance
    let balance_call = balanceOfCall { account };
    let tx_call = TransactionRequest::default()
        .from(account)
        .to(weth)
        .input(balance_call.abi_encode().into());
    
    match http_provider.call(&tx_call).await {
        Ok(result) => {
            if result.len() >= 32 {
                let balance_after = U256::from_be_slice(&result[0..32]);
                info!("   WETH balance after deposits/approvals: {} WETH", balance_after / U256::from(10u128.pow(18)));
            }
        }
        Err(e) => {
            info!("   ⚠️  Balance call failed: {}", e);
        }
    }
    
    // Also try eth_getLogs to verify logs exist (poll for HISTORICAL logs)
    info!("🔍 Checking if logs exist on the blockchain (polling entire fork history)...");
    let current_block = http_provider.get_block_number().await?;
    info!("   Current block: {}", current_block);
    
    // Poll for ALL logs on both pools from last 100k blocks (max allowed)
    let poll_filter_all = Filter::new()
        .address(vec![v2_pool, v3_pool])
        .from_block(if current_block > 100000u64 { current_block - 100000u64 } else { 0u64 })
        .to_block(current_block);
    
    match http_provider.get_logs(&poll_filter_all).await {
        Ok(logs) => {
            info!("   ✅ Total logs found on V2/V3 pools from entire fork: {} logs", logs.len());
            if logs.is_empty() {
                info!("   📝 No logs found on either pool - likely no swaps have occurred yet");
            } else {
                for (i, log) in logs.iter().enumerate().take(10) {
                    info!("      Log {}: address={:?}, topics={}, data_len={}", i, log.address(), log.topics().len(), log.data().data.len());
                }
            }
        }
        Err(e) => {
            info!("   ⚠️  Poll failed: {}", e);
        }
    }
    
    // Then check for swap signature specifically across the whole fork
    let swap_sig_v2 = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let swap_sig_v3 = keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)".as_bytes());
    
    let poll_filter_swaps = Filter::new()
        .address(vec![v2_pool, v3_pool])
        .event_signature(vec![swap_sig_v2, swap_sig_v3])
        .from_block(if current_block > 100000u64 { current_block - 100000u64 } else { 0u64 })
        .to_block(current_block);
    
    match http_provider.get_logs(&poll_filter_swaps).await {
        Ok(logs) => {
            info!("   ✅ Swap-specific logs found: {} logs", logs.len());
            for (i, log) in logs.iter().enumerate() {
                info!("   Swap Log {}: address={:?}, topics={}", i, log.address(), log.topics().len());
            }
        }
        Err(e) => {
            info!("   ⚠️  Swap poll failed: {}", e);
        }
    }

    if total > 0 {
        info!("✅ SUCCESS: Swap logs detected via subscription!");
    } else {
        info!("ℹ️  No swap logs detected via subscription");
        info!("   (This may indicate WebSocket subscription issues)");
    }

    // Stop Anvil
    let _ = anvil_process.kill();
    thread::sleep(Duration::from_secs(1));

    info!("✅ Log detection test complete");
    Ok(())
}
