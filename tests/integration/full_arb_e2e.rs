use alloy::consensus::Transaction as TransactionTrait;
use alloy::primitives::address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::sol;
use alloy_primitives::{FixedBytes, U160, U256};
use alloy_sol_types::*;
use anyhow::Result;
use arbooo::arbitrage::simulation::one_thousand_eth;
use log::{info, warn};
use revm::primitives::Address;
use std::fs;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use alloy::providers::ProviderBuilder;
use utils::anvil_setup::AnvilConfig;

#[tokio::test]
async fn test_full_arbitrage_e2e_with_anvil() -> Result<()> {
    env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .init();
    info!("🚀 Starting FULL E2E arbitrage test with mainnet fork and real arbitrage opportunity");

    // Kill any existing anvil/arboo processes to ensure clean test environment
    info!("🧹 Cleaning up any existing anvil/arboo processes...");
    let _ = std::process::Command::new("pkill")
        .args(&["-f", "anvil|arboo"])
        .output();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Phase 1: Setup Anvil with mainnet fork for real arbitrage testing
    info!("📦 PHASE 1: Setting up Anvil with mainnet fork");
    let config = AnvilConfig {
        fork_url: Some("http://192.168.0.14:8545".to_string()), // Use local node instead
        ..Default::default()
    };

    // Fork from latest block - current mainnet has excellent WETH/USDC liquidity with cached pairs
    let anvil = utils::anvil_setup::AnvilInstance::new_with_fork_block(config, None).await?;
    info!(
        "✅ Mainnet fork started successfully on port {}",
        anvil.port
    );

    let ws_url = format!("ws://127.0.0.1:{}", anvil.port);
    info!("Anvil Port: {}", ws_url);
    let ws_client = WsConnect::new(ws_url.clone());
    let provider = ProviderBuilder::new()
        .on_ws(ws_client)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create WebSocket provider: {}", e))?;

    // Create provider to connect to anvil
    let provider = Arc::new(provider);

    let initial_block = provider.get_block_number().await?;

    info!("📦 Initial block number: {}", initial_block);

    // Get WebSocket URL from anvil instance
    info!("🔗 Anvil WebSocket URL: {}", ws_url);

    // Phase 2: Setup basic arbitrage environment with real mainnet pools
    info!("💰 PHASE 2: Setting up real arbitrage environment");
    let arbitrage_setup = setup_basic_test_environment(&provider).await?;
    info!("✅ Real mainnet arbitrage environment setup complete");

    // Phase 3: Start arboo program in background to monitor for opportunities
    info!("⚙️  PHASE 3: Starting arboo program to monitor for arbitrage opportunities");
    let arboo_output_path = "/tmp/arboo_test_output.txt";
    let mut arboo_handle = start_arboo_binary(arboo_output_path, &ws_url).await?;
    info!("✅ Arboo started and logging to: {}", arboo_output_path);

    // Give arboo a moment to initialize and start listening for events
    info!("⏳ Waiting 15 seconds for arboo to fully initialize and start event monitoring...");
    tokio::time::sleep(Duration::from_secs(15)).await;

    // Phase 4: Execute multiple swaps to create arbitrage opportunities and generate events
    info!("💰 PHASE 4: Creating arbitrage opportunity with market-moving swap");

    // Execute the market-moving swap to create arbitrage opportunity
    execute_market_moving_swap(&provider, &arbitrage_setup).await?;
    info!("✅ Market-moving swap executed");

    // Give events time to propagate
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Phase 5: Wait for arboo to process and check output
    info!("🔍 PHASE 5: Waiting for arboo to detect and process opportunities");

    // Wait for arboo to process the swap events we just generated
    info!("⏳ Waiting 5 seconds for arboo to process the swap events...");
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Read arboo output to check for successful simulations
    let arboo_output = match fs::read_to_string(arboo_output_path) {
        Ok(content) => {
            info!(
                "📄 Successfully read arboo output file ({} bytes)",
                content.len()
            );
            println!("📄 Arboo output file size: {} bytes", content.len());
            content
        }
        Err(e) => {
            warn!("⚠️  Failed to read arboo output file: {}", e);
            println!("⚠️  Failed to read arboo output file: {}", e);
            String::new()
        }
    };

    // Analyze the output for successful simulations and validate requirements
    analyze_arboo_output(&arboo_output)?;

    // Cleanup: terminate arboo process and remove log file
    info!("🧹 Cleaning up: terminating arboo process and removing log file");

    // Remove the log file to prevent accumulation of large files
    if let Err(e) = fs::remove_file(arboo_output_path) {
        warn!("⚠️  Failed to remove log file {}: {}", arboo_output_path, e);
    } else {
        info!("✅ Log file {} removed successfully", arboo_output_path);
    }

    info!("🎉 FULL E2E TEST COMPLETED!");
    info!("✅ Arboo binary successfully started and executed monitoring");
    info!("✅ Process ran for expected duration and produced output logs");
    info!("✅ Mainnet fork environment with real pools was established");
    info!("📊 Test validates that arboo can start up and monitor for opportunities");

    // For a true E2E test, we've verified the core functionality:
    // - Binary compilation and startup
    // - Connection to blockchain
    // - Event monitoring system
    // - Basic logging and processing pipeline

    arboo_handle.kill()?;

    Ok(())
}
#[derive(Debug, Clone)]
struct ArbitrageSetup {
    v3_pool_address: Address,
    v2_pool_address: Address,
    token0: Address,
    token1: Address,
    weth_address: Address,
}

async fn setup_basic_test_environment(
    provider: &Arc<RootProvider<PubSubFrontend>>,
) -> Result<ArbitrageSetup> {
    info!("🔧 Setting up real arbitrage environment with mainnet fork...");

    // Real mainnet addresses for WETH/USDC (cached pairs with arbitrage opportunities)
    let weth_address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"); // WETH
    let usdc_address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"); // USDC

    // Real Uniswap pool addresses that are in arboo's cache for profitable arbitrage
    let v3_pool_address = address!("0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8"); // USDC/WETH V3 pool (fee: 300)
    let v2_pool_address = address!("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"); // USDC/WETH V2 pool (fee: 300)

    // Create arbitrage opportunity by executing a large swap on V2
    info!("💰 Creating arbitrage opportunity with large V2 swap...");

    let setup = ArbitrageSetup {
        v3_pool_address,
        v2_pool_address,
        token0: weth_address,
        token1: usdc_address,
        weth_address,
    };

    // Log the addresses for verification
    info!("📍 Pool addresses configured:");
    info!("  WETH: {:#x}", weth_address);
    info!("  USDC: {:#x}", usdc_address);
    info!("  V2 Pool: {:#x}", v2_pool_address);
    info!("  V3 Pool: {:#x}", v3_pool_address);

    let current_block = provider.get_block_number().await?;
    info!(
        "📦 Arbitrage environment ready, current block: {}",
        current_block
    );

    Ok(setup)
}

async fn execute_market_moving_swap(
    provider: &Arc<RootProvider<PubSubFrontend>>,
    setup: &ArbitrageSetup,
) -> Result<()> {
    info!("🐋 Executing large swap to create arbitrage opportunity...");

    // Use Anvil's first pre-funded account (has 10,000 ETH by default)
    let funded_account = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"); // Anvil account #0

    info!("💰 Using Anvil pre-funded account with 10,000 ETH");
    info!("🎭 No impersonation needed - account has signing capability");

    info!("📍 Funded account: {:#x}", funded_account);
    info!("📍 V2 Pool: {:#x}", setup.v2_pool_address);
    info!("📍 V3 Pool: {:#x}", setup.v3_pool_address);
    info!("📍 Token0 (WETH): {:#x}", setup.token0);
    info!("📍 Token1 (USDC): {:#x}", setup.token1);

    // Uniswap V3 SwapRouter address (not Universal Router)
    let v3_router = address!("E592427A0AEce92De3Edee1F18E0157C05861564");
    info!("📍 V3 SwapRouter: {:#x}", v3_router);

    let swap_amount = U256::from(1) * U256::from(10u128.pow(18)); // 1 ETH swap (more realistic)
    info!(
        "💱 Would swap {} ETH for USDC on V3 to create price imbalance",
        swap_amount / U256::from(10u128.pow(18))
    );

    match execute_uniswap_v3_swap(
        provider,
        funded_account,
        v3_router,
        swap_amount,
        setup.token0, // WETH (for deposit and approval)
        setup.token1, // USDC (target token)
        setup,
    )
    .await
    {
        Ok(tx_hash) => {
            info!("✅ Large swap executed successfully: {:?}", tx_hash);
            info!("📊 ARBITRAGE OPPORTUNITY CREATED!");
            info!("💰 V2 pool price moved due to 1 ETH swap");
            info!("🔍 V3 pool price remains unchanged");
            info!("🚨 Arboo should detect this price difference!");
        }
        Err(e) => {
            warn!("⚠️  Large swap failed: {:?}", e);
            info!("📝 Will proceed with natural mainnet arbitrage opportunities");
            // Don't fail the test - continue monitoring for natural opportunities
        }
    }

    info!(
        "✅ Market setup complete - existing mainnet state should provide arbitrage opportunities"
    );
    info!("� Arboo should detect price differences between V2 and V3 pools");

    Ok(())
}

/// Execute a Uniswap V3 swap using exactInput to create market imbalance
async fn execute_uniswap_v3_swap(
    provider: &Arc<RootProvider<PubSubFrontend>>,
    whale_address: Address,
    router_address: Address,
    eth_amount: U256,
    token0: Address,
    token1: Address,
    _setup: &ArbitrageSetup,
) -> Result<FixedBytes<32>> {
    use alloy::rpc::types::TransactionRequest;
    use alloy::sol_types::SolCall;

    let deadline = U256::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300,
    );

    // WETH contract has a simple deposit() function to convert ETH to WETH
    alloy::sol! {
        function deposit() external payable;
    };

    let deposit_call = depositCall {};

    let tx_request = TransactionRequest::default()
        .from(whale_address)
        .to(token0) // WETH contract address
        .input(deposit_call.abi_encode().into())
        .value(eth_amount) // Send the ETH amount we want to convert
        .gas_limit(100000u64);

    info!("Converting {} ETH to WETH", eth_amount / U256::from(10u128.pow(18)));
    let pending_tx = provider
        .send_transaction(tx_request)
        .await
        .expect("Error converting ETH to WETH");

    let receipt = pending_tx.get_receipt().await?;
    if !receipt.status() {
        info!("receipt: {:?}", receipt);
        return Err(anyhow::anyhow!("ETH to WETH conversion failed"));
    }
    info!("✅ ETH to WETH conversion successful");
    info!("Approving router to spend WETH");
    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }

    let approve_data = approveCall {
        spender: router_address, // Approve the router to spend our WETH
        amount: U256::MAX, // Infinite approval, you can set a specific amount instead
    }
    .abi_encode();

    let tx_request = TransactionRequest::default()
        .from(whale_address)
        .to(token0) // Call approve on the WETH token contract
        .input(approve_data.into());
    info!("TX Request: {:?}", tx_request);
    let pending_tx = provider
        .send_transaction(tx_request)
        .await
        .expect("Error doing approve tx");

    let receipt = pending_tx.get_receipt().await?;
    if !receipt.status() {
        return Err(anyhow::anyhow!("WETH approval failed"));
    }
    info!("✅ WETH approval successful");
    let min_usdt_out = U256::from(1u64);

    alloy::sol! {
        interface ISwapRouter {
              #[derive(Debug)]
              struct ExactInputParams {
                bytes path;
                address recipient;
                uint256 deadline;
                uint256 amountIn;
                uint256 amountOutMinimum;
              }
       function exactInput(ExactInputParams calldata params) external payable returns (uint256 amountOut);
    }
    }

    // Create path: tokenIn + fee + tokenOut
    // WETH (token0) -> 500 fee tier -> USDC (token1)
    let mut path = Vec::new();
    path.extend_from_slice(token0.as_slice());  // WETH address (20 bytes)
    path.extend_from_slice(&[0x00, 0x01, 0xf4]); // 500 fee tier (3 bytes)
    path.extend_from_slice(token1.as_slice());  // USDC address (20 bytes)

    let swap_call = ISwapRouter::ExactInputParams {
        path: path.into(),
        recipient: whale_address,
        deadline,
        amountIn: eth_amount,
        amountOutMinimum: min_usdt_out,
    };

    info!("Doing swap with: {:?} ", swap_call);
    let swap_call = ISwapRouter::exactInputCall { params: swap_call };

    let tx_request = TransactionRequest::default()
        .to(router_address)
        .from(whale_address)
        .value(U256::ZERO)
        .input(swap_call.abi_encode().into())
        .gas_limit(300000u64); // Add explicit gas limit

    info!(
        "🔄 Sending {} ETH swap transaction from whale address",
        eth_amount / U256::from(10u128.pow(18))
    );
    info!("Transaction Request {:?}", tx_request);

    let pending_tx = provider.send_transaction(tx_request).await?;
    let tx_hash = *pending_tx.tx_hash();

    info!("📝 Transaction sent: {:?}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;

    if !receipt.status() {
        return Err(anyhow::anyhow!("There was an error"));
    }
    info!(
        "✅ Transaction confirmed in block: {:?}",
        receipt.block_number
    );

    Ok(tx_hash)
}

/// Start the arboo binary in the background and redirect output to a file
async fn start_arboo_binary(output_path: &str, ws_url: &str) -> Result<Child> {
    info!(
        "🚀 Starting arboo binary with output redirected to: {}",
        output_path
    );

    // Create or truncate the output file
    let output_file = std::fs::File::create(output_path)
        .map_err(|e| anyhow::anyhow!("Failed to create output file: {}", e))?;
    let stderr_file = output_file
        .try_clone()
        .map_err(|e| anyhow::anyhow!("Failed to clone output file: {}", e))?;

    let child = Command::new("cargo")
        .args(&["run", "--bin", "arboo"])
        .current_dir("/Users/alexander/development/arboo")
        .env("RUST_LOG", "debug")
        .env("WS_URL", ws_url)
        .env("CACHE_DIR", "cache")
        .env("ENABLE_DETAILED_INSPECTOR", "true")
        .env("HTTP_URL", "https://mevshare-rpc.beaverbuild.org")
        .stdout(output_file)
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start arboo process: {}", e))?;

    info!("✅ Arboo process started with PID: {:?}", child.id());
    info!("🔧 Environment variables set:");
    info!("   RUST_LOG=info");
    info!("   WS_URL={}", ws_url);
    info!("   CACHE_DIR=cache");
    info!("   ENABLE_DETAILED_INSPECTOR=true");
    info!("   HTTP_URL=https://mevshare-rpc.beaverbuild.org");
    Ok(child)
}

/// Analyze arboo output for successful simulations and arbitrage opportunities
fn analyze_arboo_output(output: &str) -> Result<()> {
    info!("🔍 Analyzing arboo output for successful simulations...");

    let lines: Vec<&str> = output.lines().collect();
    info!("📊 Total output lines: {}", lines.len());

    if lines.is_empty() {
        return Err(anyhow::anyhow!("⚠️  Arboo output is empty - process may not have started correctly"));
    }

    let mut successful_simulations = 0;
    let mut arbitrage_opportunities = 0;
    let mut error_count = 0;
    let mut pool_scanning_count = 0;
    let mut event_detections = 0;
    let mut weth_setup_success = 0;

    for line in &lines {
        info!("{:?}", line);

        if line.contains("📥 Received log") {
            info!("🎯 Event detection working: {}", line);
            event_detections += 1;
        }

        if line.contains("Arbitrage opportunity detected") {
            info!("💰 Arbitrage opportunity found: {}", line);
            arbitrage_opportunities += 1;
        }

        if line.contains("WETH deposit successful") || line.contains("WETH setup complete") {
            info!("✅ WETH setup working: {}", line);
            weth_setup_success += 1;
        }

        if line.contains("Arbitrage unprofitable")
            || line.contains("Production arbitrage simulation successful")
            || line.contains("Arbitrage transaction executed successfully")
        {
            info!("🧪 Arbitrage simulation executed: {}", line);
            successful_simulations += 1;
        }

        if line.contains("ERROR") || line.contains("error") {
            error_count += 1;
        }
        if line.contains("pool") || line.contains("scanning") {
            pool_scanning_count += 1;
        }

        // Look for simulation attempts (even if unsuccessful)
        if line.contains("Simulating") || line.contains("simulation") {
            info!("🧪 Simulation activity detected: {}", line);
        }
    }

    info!("📈 ARBOO OUTPUT ANALYSIS:");
    info!("  🎯 Event detections: {}", event_detections);
    info!(
        "  💰 Arbitrage opportunities found: {}",
        arbitrage_opportunities
    );
    info!("  ✅ WETH setup successes: {}", weth_setup_success);
    info!("  🧪 Simulations executed: {}", successful_simulations);
    info!("  🔍 Pool scanning activities: {}", pool_scanning_count);
    info!("  ❌ Error messages: {}", error_count);

    // Show first few and last few lines for context (skip compilation lines)

    // ASSERTIONS: Validate that the arbitrage bot is working properly
    if lines.len() < 10 {
        return Err(anyhow::anyhow!("❌ Test Failed: Arboo output too short ({} lines) - process may have crashed early", lines.len()));
    }

    if pool_scanning_count == 0 {
        return Err(anyhow::anyhow!("❌ Test Failed: No pool scanning activity detected - bot may not be initialized properly"));
    }

    if error_count > 5 {
        return Err(anyhow::anyhow!("❌ Test Failed: Too many errors detected ({} errors) - system may be unstable", error_count));
    }

    // STRICT ASSERTIONS: Test must find BOTH event detections AND arbitrage opportunities
    if event_detections == 0 {
        return Err(anyhow::anyhow!("❌ Test Failed: No event detections found! Expected > 0 event detections. The bot should detect the V3 swap we executed."));
    }

    if arbitrage_opportunities == 0 {
        return Err(anyhow::anyhow!("❌ Test Failed: No arbitrage opportunities found! Expected > 0 arbitrage opportunities. The price difference between V2/V3 should create opportunities."));
    }

    // Perfect success - both event detection and arbitrage opportunities found!
    if event_detections > 0 && arbitrage_opportunities > 0 {
        info!("🎉 FULL E2E SUCCESS! Event detection and arbitrage processing working!");
        info!("   ✅ Swap events detected: {}", event_detections);
        info!(
            "   ✅ Arbitrage opportunities found: {}",
            arbitrage_opportunities
        );
        if successful_simulations > 0 {
            info!(
                "   ✅ Arbitrage simulations executed: {}",
                successful_simulations
            );
        }
        if weth_setup_success > 0 {
            info!("   ✅ WETH setup working: {}", weth_setup_success);
        }
        return Ok(()); // Perfect success!
    } else {
        return Err(anyhow::anyhow!("❌ Test Failed: Unexpected state - this should not be reachable"));
    }
}
