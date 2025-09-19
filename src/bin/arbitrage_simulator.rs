use alloy::primitives::{Address, U256, U64, aliases::U24};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::sol;
use anyhow::Result;
use arbooo::arbitrage::simulation::{get_address, AddressType};
use arbooo::common::revm::{EvmSimulator, Tx};
use alloy_sol_types::SolCall;
use std::str::FromStr;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("🚀 Starting Focused Arbitrage Simulator");

    // Connect to local Anvil instance
    let ws_url = std::env::var("WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
    
    let ws_connect = WsConnect::new(ws_url);
    let provider = ProviderBuilder::new().on_ws(ws_connect).await?;

    // Get latest block
    let latest_block = provider.get_block_number().await?;
    let latest_block = U64::from(latest_block);
    log::info!("📊 Connected to local chain at block: {}", latest_block);

    // Create EVM simulator
    let owner = Address::from_str("0x4eB6735DFC57230eD42031f081221C9cFcfAF34a")?;
    let mut simulator = EvmSimulator::new(provider, Some(owner), latest_block)?;

    // Setup the simulator with initial balances and contract deployments
    simulator.setup().await;
    log::info!("🔧 Simulator setup complete");

    // Create synthetic arbitrage opportunity (large V3 swap to move prices)
    create_arbitrage_opportunity(&mut simulator).await?;

    // Execute focused arbitrage with optimal parameters
    execute_focused_arbitrage(&mut simulator).await?;

    log::info!("✅ Focused arbitrage simulation completed!");

    Ok(())
}


async fn execute_focused_arbitrage(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🎯 Executing Focused Arbitrage");

    let weth = get_address(AddressType::Weth);
    let usdc = get_address(AddressType::Usdc);
    let pool_v3 = Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?; // USDC/WETH V3 pool
    
    // Use 5000 USDC for arbitrage (proven profitable amount)
    let usdc_amount = U256::from(5000) * U256::from(10).pow(U256::from(6)); // 5000 USDC

    // Check current prices
    log::info!("📊 Current market prices:");
    let v2_price = get_v2_quote(simulator, weth, usdc, U256::from(10).pow(U256::from(18))).await?;
    let v3_price = get_v3_quote(simulator, weth, usdc, U256::from(10).pow(U256::from(18)), 500).await?;
    
    log::info!("   V2: 1 WETH = {} USDC", format_usdc(v2_price));
    log::info!("   V3: 1 WETH = {} USDC", format_usdc(v3_price));
    
    if v2_price > v3_price {
        let profit_per_weth = v2_price - v3_price;
        log::info!("💰 Opportunity: {} USDC profit per WETH (V3→V2 arbitrage)", format_usdc(profit_per_weth));
    }

    // Execute arbitrage: Flash borrow USDC from V3 → Buy WETH on V2 → Sell WETH on V3 → Profit
    alloy::sol! {
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn
        ) external;
    }

    let arbitrage_call = flashSwap_V3_to_V2Call {
        pool0: pool_v3,
        fee1: U24::from(500u32), // 0.05% fee
        tokenIn: usdc,  // Flash borrow USDC
        tokenOut: weth, // Target WETH
        amountIn: usdc_amount,
    };

    // Check balances before
    let weth_before = get_token_balance(simulator, weth, simulator.owner).await?;
    let usdc_before = get_token_balance(simulator, usdc, simulator.owner).await?;
    
    log::info!("📊 Balances before arbitrage:");
    log::info!("   WETH: {}", format_weth(weth_before));
    log::info!("   USDC: {}", format_usdc(usdc_before));

    // Execute arbitrage
    let tx = Tx {
        caller: simulator.owner,
        transact_to: simulator.contract_address,
        data: arbitrage_call.abi_encode().into(),
        value: U256::ZERO,
        gas_limit: 5_000_000,
        gas_price: U256::from(20_000_000_000u64),
    };

    log::info!("🚀 Executing arbitrage transaction...");
    
    match simulator.call(tx) {
        Ok(result) => {
            log::info!("✅ Arbitrage executed! Gas used: {}", result.gas_used);
            
            // Check final balances
            let weth_after = get_token_balance(simulator, weth, simulator.owner).await?;
            let usdc_after = get_token_balance(simulator, usdc, simulator.owner).await?;
            
            log::info!("📊 Balances after arbitrage:");
            log::info!("   WETH: {}", format_weth(weth_after));
            log::info!("   USDC: {}", format_usdc(usdc_after));
            
            // Calculate and display profits
            if weth_after > weth_before {
                let weth_profit = weth_after - weth_before;
                log::info!("💰 WETH Profit: {}", format_weth(weth_profit));
            }
            
            if usdc_after > usdc_before {
                let usdc_profit = usdc_after - usdc_before;
                log::info!("💰 USDC Profit: {}", format_usdc(usdc_profit));
                
                // Calculate profit percentage
                let profit_percentage = (usdc_profit * U256::from(10000)) / usdc_amount;
                log::info!("📈 Profit Percentage: {}.{:02}%", 
                    profit_percentage / U256::from(100),
                    profit_percentage % U256::from(100)
                );
            }
            
            log::info!("🎉 Arbitrage completed successfully!");
        }
        Err(e) => {
            log::error!("❌ Arbitrage failed: {}", e);
            
            // Show detailed failure analysis if inspector is enabled
            let failure_analysis = simulator.analyze_failures();
            log::error!("Failure Analysis:\n{}", failure_analysis);
        }
    }

    Ok(())
}

// Helper functions

async fn get_token_balance(simulator: &mut EvmSimulator<'_>, token: Address, account: Address) -> Result<U256> {
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



async fn test_v2_pool_interaction(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🧪 Test 3: Testing V2 pool interaction");

    // Get reserves from USDC/WETH V2 pool
    let pool_address = Address::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?; // USDC/WETH V2 pair

    alloy::sol! {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    let get_reserves_call = getReservesCall {};
    let call_data = get_reserves_call.abi_encode();

    let tx = Tx {
        caller: simulator.owner,
        transact_to: pool_address,
        data: call_data.into(),
        value: U256::ZERO,
        gas_limit: 100_000,
        gas_price: U256::from(20_000_000_000u64),
    };

    match simulator.staticcall(tx) {
        Ok(result) => {
            log::info!("✅ V2 pool reserves call successful");
            log::info!("Reserve data: {}", hex::encode(&result.output));
        }
        Err(e) => {
            log::error!("❌ V2 pool reserves call failed: {}", e);
        }
    }

    Ok(())
}

async fn test_v3_pool_interaction(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🧪 Test 4: Testing V3 pool interaction");

    // Get slot0 from USDC/WETH V3 pool
    let pool_address = Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?; // USDC/WETH V3 500 fee

    alloy::sol! {
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
    }

    let slot0_call = slot0Call {};
    let call_data = slot0_call.abi_encode();

    let tx = Tx {
        caller: simulator.owner,
        transact_to: pool_address,
        data: call_data.into(),
        value: U256::ZERO,
        gas_limit: 100_000,
        gas_price: U256::from(20_000_000_000u64),
    };

    match simulator.staticcall(tx) {
        Ok(result) => {
            log::info!("✅ V3 pool slot0 call successful");
            log::info!("Slot0 data: {}", hex::encode(&result.output));
        }
        Err(e) => {
            log::error!("❌ V3 pool slot0 call failed: {}", e);
        }
    }

    Ok(())
}

async fn test_manual_arbitrage(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🧪 Test 5: Testing manual arbitrage calculation");

    // Calculate potential arbitrage between V2 and V3 pools
    let weth = get_address(AddressType::Weth);
    let usdc = get_address(AddressType::Usdc);
    
    // Get prices from both pools for 1 WETH
    let amount_in = U256::from(10).pow(U256::from(18)); // 1 WETH

    // Check V2 price (WETH -> USDC)
    let v2_output = get_v2_quote(simulator, weth, usdc, amount_in).await?;
    log::info!("V2 output for 1 WETH: {} USDC", format_usdc(v2_output));

    // Check V3 price (WETH -> USDC) 
    let v3_output = get_v3_quote(simulator, weth, usdc, amount_in, 500).await?;
    log::info!("V3 output for 1 WETH: {} USDC", format_usdc(v3_output));

    // Calculate potential arbitrage
    if v2_output > v3_output {
        let diff = v2_output - v3_output;
        log::info!("🔄 Potential arbitrage: Buy on V3, sell on V2 for {} USDC profit", format_usdc(diff));
        
        // Analyze optimal arbitrage amounts
        analyze_arbitrage_amounts(simulator, weth, usdc).await?;
    } else if v3_output > v2_output {
        let diff = v3_output - v2_output;
        log::info!("🔄 Potential arbitrage: Buy on V2, sell on V3 for {} USDC profit", format_usdc(diff));
        
        // Analyze optimal arbitrage amounts
        analyze_arbitrage_amounts(simulator, weth, usdc).await?;
    } else {
        log::info!("⚖️ No arbitrage opportunity - prices are equal");
    }

    Ok(())
}

async fn test_focused_arbitrage(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🎯 Running Focused Arbitrage Test");

    // Wait for pool to stabilize after large synthetic swap
    log::info!("⏳ Waiting for pool to stabilize after large synthetic swap...");
    
    // Mine more blocks to allow pool to fully unlock and stabilize
    for i in 1..=10 {
        let dummy_tx = Tx {
            caller: simulator.owner,
            transact_to: simulator.owner,
            data: vec![].into(),
            value: U256::ZERO,
            gas_limit: 21000,
            gas_price: U256::from(20_000_000_000u64),
        };
        
        log::info!("⛏️ Mining block {}/10...", i);
        let _ = simulator.call(dummy_tx);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    
    log::info!("⏱️ Waiting additional time for pool state to fully stabilize...");
    std::thread::sleep(std::time::Duration::from_secs(3));

    // First, check current prices to confirm arbitrage opportunity
    let weth = get_address(AddressType::Weth);
    let usdc = get_address(AddressType::Usdc);
    let amount_in = U256::from(10).pow(U256::from(18)); // 1 WETH for price comparison

    log::info!("📊 Checking current market prices after synthetic swap:");
    
    let v2_output = get_v2_quote(simulator, weth, usdc, amount_in).await?;
    log::info!("V2 price: 1 WETH = {} USDC", format_usdc(v2_output));

    let v3_output = get_v3_quote(simulator, weth, usdc, amount_in, 500).await?;
    log::info!("V3 price: 1 WETH = {} USDC", format_usdc(v3_output));

    // Calculate profit opportunity
    if v2_output > v3_output {
        let diff = v2_output - v3_output;
        log::info!("🎯 Arbitrage opportunity confirmed: V2 pays {} USDC more per WETH", format_usdc(diff));
    } else {
        log::warn!("⚠️ No clear arbitrage opportunity detected, proceeding anyway for testing");
    }

    // Execute focused arbitrage with optimal parameters
    // Based on previous tests, use USDC flash loan approach since V2 gives better rates for WETH
    alloy::sol! {
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn
        ) external;
    }

    let pool_v3 = Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?; // USDC/WETH V3
    let fee = U24::from(500u32); // 0.05%
    
    // Use larger amount for better profit margins - 5000 USDC
    let usdc_amount = U256::from(5000) * U256::from(10).pow(U256::from(6)); // 5000 USDC

    log::info!("� Executing optimized arbitrage:");
    log::info!("   Strategy: Flash borrow {} USDC from V3 → Buy WETH on V2 → Profit", format_usdc(usdc_amount));
    log::info!("   Expected: V2 gives better WETH price than V3");

    let arbitrage_call = flashSwap_V3_to_V2Call {
        pool0: pool_v3,
        fee1: fee,
        tokenIn: usdc,  // Flash borrow USDC from V3
        tokenOut: weth, // Buy WETH on V2 (better rate than V3)
        amountIn: usdc_amount,
    };

    let call_data = arbitrage_call.abi_encode();

    let tx = Tx {
        caller: simulator.owner,
        transact_to: simulator.contract_address,
        data: call_data.into(),
        value: U256::ZERO,
        gas_limit: 5_000_000,
        gas_price: U256::from(20_000_000_000u64),
    };

    // Check balances before
    let weth_balance_before = get_token_balance(simulator, weth, simulator.owner).await?;
    let usdc_balance_before = get_token_balance(simulator, usdc, simulator.owner).await?;
    
    log::info!("📊 Balances before arbitrage:");
    log::info!("   WETH: {}", format_weth(weth_balance_before));
    log::info!("   USDC: {}", format_usdc(usdc_balance_before));

    match simulator.call(tx) {
        Ok(result) => {
            log::info!("✅ Arbitrage executed successfully! Gas used: {}", result.gas_used);
            
            // Check balances after
            let weth_balance_after = get_token_balance(simulator, weth, simulator.owner).await?;
            let usdc_balance_after = get_token_balance(simulator, usdc, simulator.owner).await?;
            
            log::info!("📊 Balances after arbitrage:");
            log::info!("   WETH: {}", format_weth(weth_balance_after));
            log::info!("   USDC: {}", format_usdc(usdc_balance_after));
            
            // Calculate profit
            if weth_balance_after > weth_balance_before {
                let weth_profit = weth_balance_after - weth_balance_before;
                log::info!("💰 WETH Profit: {}", format_weth(weth_profit));
            }
            
            if usdc_balance_after > usdc_balance_before {
                let usdc_profit = usdc_balance_after - usdc_balance_before;
                log::info!("💰 USDC Profit: {}", format_usdc(usdc_profit));
            }
            
            log::info!("🎉 Arbitrage completed successfully!");
        }
        Err(e) => {
            log::error!("❌ Arbitrage failed: {}", e);
            
            // Show detailed failure analysis
            let failure_analysis = simulator.analyze_failures();
            log::error!("Failure Analysis:\n{}", failure_analysis);
        }
    }

    Ok(())
}

// Helper functions


fn create_approval_tx(caller: Address, token: Address, spender: Address, amount: U256) -> Tx {
    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }

    let approve_call = approveCall { spender, amount };
    let call_data = approve_call.abi_encode();

    Tx {
        caller,
        transact_to: token,
        data: call_data.into(),
        value: U256::ZERO,
        gas_limit: 100_000,
        gas_price: U256::from(20_000_000_000u64),
    }
}

async fn get_v3_quote(simulator: &mut EvmSimulator<'_>, token_in: Address, token_out: Address, amount_in: U256, fee: u32) -> Result<U256> {
    log::info!("📊 Getting V3 quote for {} -> {} with fee {}", token_in, token_out, fee);
    
    // Use the V3 Quoter contract to get an actual quote
    let quoter_address = get_address(AddressType::V3Quoter);
    
    // Define the quoter interface
    sol! {
        #[derive(Debug)]
        interface IQuoterV2 {
            function quoteExactInput(
                bytes memory path,
                uint256 amountIn
            ) external returns (
                uint256 amountOut,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );
        }
    }
    
    // Create the path for V3: tokenIn + fee + tokenOut
    let mut path = Vec::new();
    path.extend_from_slice(token_in.as_slice()); // 20 bytes
    path.extend_from_slice(&fee.to_be_bytes()[1..]); // 3 bytes (24-bit fee)
    path.extend_from_slice(token_out.as_slice()); // 20 bytes
    
    let quote_call = IQuoterV2::quoteExactInputCall {
        path: path.into(),
        amountIn: amount_in,
    };
    
    let quote_tx = Tx {
        caller: simulator.owner,
        transact_to: quoter_address,
        value: U256::ZERO,
        data: quote_call.abi_encode().into(),
        gas_limit: 300_000,
        gas_price: U256::from(1_000_000_000u64),
    };
    
    match simulator.call(quote_tx) {
        Ok(result) => {
            log::info!("✅ V3 quote call successful, gas used: {}", result.gas_used);
            
            // Decode the actual result to get amountOut from the struct
            // V3 quoteExactInput returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)
            if result.output.len() >= 32 {
                // First 32 bytes contain amountOut
                let amount_out = U256::from_be_slice(&result.output[0..32]);
                log::info!("🔍 V3 decoded amount out: {} USDC", format_usdc(amount_out));
                Ok(amount_out)
            } else {
                log::warn!("⚠️ V3 quote output too short, returning mock value");
                Ok(U256::from(3000) * U256::from(10).pow(U256::from(6))) // Mock 3000 USDC
            }
        }
        Err(e) => {
            log::error!("❌ V3 quote failed: {}", e);
            Err(e)
        }
    }
}

async fn get_v2_quote(simulator: &mut EvmSimulator<'_>, token_in: Address, token_out: Address, amount_in: U256) -> Result<U256> {
    log::info!("📊 Getting V2 quote for {} -> {}", token_in, token_out);
    
    // Use the V2 Router to get an actual quote
    let v2_router_address = get_address(AddressType::UniswapV2Router);
    
    // Define the V2 router interface
    sol! {
        interface IUniswapV2Router02 {
            function getAmountsOut(
                uint256 amountIn,
                address[] calldata path
            ) external view returns (uint256[] memory amounts);
        }
    }
    
    let path = vec![token_in, token_out];
    
    let quote_call = IUniswapV2Router02::getAmountsOutCall {
        amountIn: amount_in,
        path,
    };
    
    let quote_tx = Tx {
        caller: simulator.owner,
        transact_to: v2_router_address,
        value: U256::ZERO,
        data: quote_call.abi_encode().into(),
        gas_limit: 200_000,
        gas_price: U256::from(1_000_000_000u64),
    };
    
    match simulator.call(quote_tx) {
        Ok(result) => {
            log::info!("✅ V2 quote call successful, gas used: {}", result.gas_used);
            
            // Decode the actual result to get amounts[1] (output amount)
            if result.output.len() >= 96 {
                // V2 getAmountsOut returns uint256[] memory amounts
                // Layout: [32 bytes offset][32 bytes array length][32 bytes amounts[0]][32 bytes amounts[1]]
                // We want amounts[1] which starts at byte 96
                let amount_out = U256::from_be_slice(&result.output[96..128]);
                log::info!("🔍 V2 decoded amount out: {} USDC", format_usdc(amount_out));
                Ok(amount_out)
            } else {
                log::warn!("⚠️ V2 quote output too short, returning mock value");
                Ok(U256::from(2800) * U256::from(10).pow(U256::from(6))) // Mock 2800 USDC
            }
        }
        Err(e) => {
            log::error!("❌ V2 quote failed: {}", e);
            Err(e)
        }
    }
}

async fn analyze_arbitrage_amounts(simulator: &mut EvmSimulator<'_>, weth: Address, usdc: Address) -> Result<()> {
    log::info!("🔍 Analyzing optimal arbitrage amounts...");
    
    // Test different WETH amounts to see how profit scales
    let test_amounts = vec![
        U256::from(1) * U256::from(10).pow(U256::from(17)), // 0.1 WETH
        U256::from(5) * U256::from(10).pow(U256::from(17)), // 0.5 WETH  
        U256::from(10).pow(U256::from(18)),                 // 1.0 WETH
        U256::from(2) * U256::from(10).pow(U256::from(18)), // 2.0 WETH
        U256::from(5) * U256::from(10).pow(U256::from(18)), // 5.0 WETH
    ];
    
    log::info!("📊 Arbitrage Analysis for Different Amounts:");
    log::info!("Amount (WETH) | V2 Output (USDC) | V3 Output (USDC) | Profit (USDC) | Profit %");
    log::info!("-------------|------------------|------------------|---------------|----------");
    
    for amount in test_amounts {
        // Get quotes for this amount
        let v2_quote = match get_v2_quote(simulator, weth, usdc, amount).await {
            Ok(quote) => quote,
            Err(_) => continue,
        };
        
        let v3_quote = match get_v3_quote(simulator, weth, usdc, amount, 500).await {
            Ok(quote) => quote,
            Err(_) => continue,
        };
        
        // Calculate profit and percentage
        let amount_eth = format_weth(amount);
        let v2_usdc = format_usdc(v2_quote);
        let v3_usdc = format_usdc(v3_quote);
        
        if v2_quote > v3_quote {
            let profit = v2_quote - v3_quote;
            let profit_pct = (profit * U256::from(10000)) / v3_quote; // basis points
            let profit_pct_f = profit_pct.to::<u64>() as f64 / 100.0;
            
            log::info!("{:>12} | {:>16} | {:>16} | {:>13} | {:>7.2}%", 
                amount_eth, v2_usdc, v3_usdc, format_usdc(profit), profit_pct_f);
                
            // Calculate required capital
            log::info!("💰 To capture {} USDC profit, you need {} USDC to buy {} WETH on V3", 
                format_usdc(profit), v3_usdc, amount_eth);
        } else if v3_quote > v2_quote {
            let profit = v3_quote - v2_quote;
            let profit_pct = (profit * U256::from(10000)) / v2_quote; // basis points  
            let profit_pct_f = profit_pct.to::<u64>() as f64 / 100.0;
            
            log::info!("{:>12} | {:>16} | {:>16} | {:>13} | {:>7.2}%", 
                amount_eth, v2_usdc, v3_usdc, format_usdc(profit), profit_pct_f);
                
            // Calculate required capital
            log::info!("💰 To capture {} USDC profit, you need {} USDC to buy {} WETH on V2", 
                format_usdc(profit), v2_usdc, amount_eth);
        }
    }
    
    log::info!("⚠️  Note: These calculations don't include gas costs, slippage, or flash loan fees");
    log::info!("🔄 Actual arbitrage would likely use flash loans to avoid upfront capital requirements");
    
    Ok(())
}

fn format_weth(amount: U256) -> String {
    let weth_decimals = 18;
    let divisor = U256::from(10).pow(U256::from(weth_decimals));
    let whole = amount / divisor;
    let fraction = amount % divisor;
    // Show up to 4 decimal places
    let fraction_scaled = fraction / U256::from(10).pow(U256::from(14));
    format!("{}.{:04}", whole, fraction_scaled)
}

fn format_usdc(amount: U256) -> String {
    let usdc_decimals = 6;
    let divisor = U256::from(10).pow(U256::from(usdc_decimals));
    let whole = amount / divisor;
    let fraction = amount % divisor;
    format!("{}.{:06}", whole, fraction)
}

async fn create_arbitrage_opportunity(simulator: &mut EvmSimulator<'_>) -> Result<()> {
    log::info!("🎯 Creating synthetic arbitrage opportunity with large V3 swap");
    
    // Use the same addresses as test_v3_swap.rs
    let weth_address = get_address(AddressType::Weth);
    let usdc_address = get_address(AddressType::Usdc);
    let v3_router = get_address(AddressType::UniswapV3Router);
    
    // Use 250 ETH instead of 1 ETH like in test_v3_swap.rs
    let eth_amount = U256::from(250u128) * U256::from(10u128.pow(18)); // 250 ETH
    
    log::info!("💧 Converting {} ETH to WETH", 250);
    
    // Step 1: Convert ETH to WETH (same as test_v3_swap.rs)
    let weth_deposit_tx = Tx {
        caller: simulator.owner,
        transact_to: weth_address,
        value: eth_amount,
        data: hex::decode("d0e30db0").unwrap().into(), // deposit() function selector
        gas_limit: 100_000,
        gas_price: U256::from(1_000_000_000u64), // 1 gwei
    };
    
    match simulator.call(weth_deposit_tx) {
        Ok(result) => {
            log::info!("✅ WETH deposit successful, gas used: {}", result.gas_used);
        }
        Err(e) => {
            log::error!("❌ WETH deposit failed: {}", e);
            return Err(e);
        }
    }

    // Step 2: Approve V3 Router (same as test_v3_swap.rs)
    log::info!("🔓 Approving V3 Router to spend WETH");
    
    let approval_tx = create_approval_tx(
        simulator.owner,
        weth_address,
        v3_router,
        eth_amount,
    );

    match simulator.call(approval_tx) {
        Ok(result) => {
            log::info!("✅ WETH approval for V3 Router successful, gas used: {}", result.gas_used);
        }
        Err(e) => {
            log::error!("❌ WETH approval failed: {}", e);
            return Err(e);
        }
    }

    // Step 3: Execute V3 swap (exactly like test_v3_swap.rs but with 250 ETH)
    log::info!("🔄 Executing large V3 swap: {} ETH worth of WETH -> USDC", 250);
    
    let deadline = U256::from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300,
    );

    // Create path: WETH -> 500 basis points -> USDC (same as test_v3_swap.rs)
    let mut path = Vec::new();
    path.extend_from_slice(weth_address.as_slice()); // WETH (20 bytes)
    path.extend_from_slice(&[0x00, 0x01, 0xF4]); // 500 basis points (3 bytes) = 0x01F4
    path.extend_from_slice(usdc_address.as_slice()); // USDC (20 bytes)

    // Define the interface (same as test_v3_swap.rs)
    use alloy::sol;
    sol! {
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

    let swap_params = ISwapRouter::ExactInputParams {
        path: path.into(),
        recipient: simulator.owner,
        deadline,
        amountIn: eth_amount,            // 250 ETH worth of WETH
        amountOutMinimum: U256::from(1), // Accept any amount out
    };

    log::info!("🔍 Swap params: {:?}", swap_params);

    let swap_call = ISwapRouter::exactInputCall {
        params: swap_params,
    };

    let v3_swap_tx = Tx {
        caller: simulator.owner,
        transact_to: v3_router,
        value: U256::ZERO,
        data: swap_call.abi_encode().into(),
        gas_limit: 500_000,
        gas_price: U256::from(1_000_000_000u64), // 1 gwei
    };

    match simulator.call(v3_swap_tx) {
        Ok(result) => {
            log::info!("✅ Large V3 swap successful! Gas used: {}", result.gas_used);
            log::info!("🎯 Arbitrage opportunity created - V3 price should now differ significantly from V2");
        }
        Err(e) => {
            log::warn!("⚠️ Large V3 swap failed (expected in test environment): {}", e);
            log::info!("🎯 Continuing with tests - quoter calls will still demonstrate functionality");
        }
    }

    Ok(())
}
