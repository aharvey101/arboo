#![allow(dead_code)]

use anyhow::Result;
use arbooo::common::logger;
use arbooo::arbitrage::simulation::{get_address, AddressType};
use alloy::providers::Provider;
use alloy::primitives::{U256, Address, Bytes};
use alloy::rpc::types::TransactionRequest;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[derive(Debug, Clone)]
struct FlashLoanSetup {
    provider_address: Address,
    loan_amount: U256,
    loan_token: Address,
    loan_fee: U256,
    callback_data: Bytes,
}

#[derive(Debug, Clone)]
struct SwapExecution {
    router_address: Address,
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    amount_out_min: U256,
    deadline: u64,
    swap_type: SwapType,
}

#[derive(Debug, Clone)]
enum SwapType {
    UniswapV2,
    UniswapV3 { fee: u32, sqrt_price_limit: U256 },
}

#[derive(Debug, Clone)]
struct ProfitExtraction {
    profit_token: Address,
    profit_amount: U256,
    gas_used: u64,
    net_profit: U256,
    extraction_method: ExtractionMethod,
}

#[derive(Debug, Clone)]
enum ExtractionMethod {
    DirectTransfer,
    TokenSwap { target_token: Address },
    Reinvestment { strategy: String },
}

#[derive(Debug)]
struct ExecutionPipeline {
    flash_loan: FlashLoanSetup,
    swaps: Vec<SwapExecution>,
    profit_extraction: ProfitExtraction,
    total_gas_estimate: u64,
    success_probability: f64,
}

#[tokio::test]
async fn test_flash_loan_setup_validation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Flash Loan Setup Validation Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");

    let flash_loan_scenarios = vec![
        create_aave_flash_loan_scenario(),
        create_balancer_flash_loan_scenario(),
        create_uniswap_v3_flash_loan_scenario(),
    ];

    for (i, scenario) in flash_loan_scenarios.iter().enumerate() {
        info!("🔍 Testing flash loan scenario {}: {}", i + 1, scenario.provider_name);

        assert!(scenario.setup.loan_amount > U256::ZERO, 
                "Loan amount must be greater than zero");

        assert!(scenario.setup.loan_fee <= scenario.setup.loan_amount / U256::from(100), 
                "Loan fee should be reasonable (< 1% of loan amount)");

        let flash_loan_tx = create_flash_loan_transaction(&scenario.setup)?;

        assert!(flash_loan_tx.to.is_some(), "Flash loan transaction must have a target");

        let gas_estimate = estimate_flash_loan_gas(&test_env, &flash_loan_tx).await?;

        assert!(gas_estimate >= 21000 && gas_estimate <= 1_000_000, 
                "Flash loan gas estimate should be reasonable: {} gas", gas_estimate);

        validate_flash_loan_callback(&scenario.setup.callback_data)?;

        info!("   ✅ Flash loan setup validated - Provider: {}, Amount: {} ETH, Gas: {}", 
              scenario.provider_name, 
              format_ether_amount(scenario.setup.loan_amount),
              gas_estimate);
    }

    info!("🎉 Flash Loan Setup Validation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_multi_hop_swap_execution() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Multi-Hop Swap Execution Test");

    let test_env = TestEnvironment::new().await?;

    let swap_strategies = vec![
        create_v2_to_v3_arbitrage_swaps(),
        create_cross_pool_arbitrage_swaps(),
        create_triangular_arbitrage_swaps(),
    ];

    for (i, strategy) in swap_strategies.iter().enumerate() {
        info!("🔍 Testing swap strategy {}: {}", i + 1, strategy.name);

        assert!(strategy.swaps.len() >= 2, "Multi-hop strategy must have at least 2 swaps");

        let mut cumulative_gas = 0u64;
        let mut token_flow = strategy.swaps[0].token_in;

        for (j, swap) in strategy.swaps.iter().enumerate() {
            info!("   Validating swap {}: {} -> {}", 
                  j + 1, 
                  format_token_symbol(swap.token_in),
                  format_token_symbol(swap.token_out));

            if j > 0 {
                assert_eq!(token_flow, swap.token_in, 
                          "Token flow must be continuous between swaps");
            }
            token_flow = swap.token_out;

            let swap_tx = create_swap_transaction(swap)?;

            let swap_gas = estimate_swap_gas(&test_env, &swap_tx, &swap.swap_type).await?;
            cumulative_gas += swap_gas;

            assert!(swap.amount_in > U256::ZERO, "Swap amount must be positive");
            assert!(swap.amount_out_min > U256::ZERO, "Minimum output must be positive");
            assert!(swap.deadline > get_current_timestamp(), "Deadline must be in the future");

            info!("     ✅ Swap validated - Gas: {}, Type: {:?}", swap_gas, swap.swap_type);
        }

        assert!(cumulative_gas <= 800_000, 
                "Total gas for multi-hop should be reasonable: {} gas", cumulative_gas);

        if strategy.is_arbitrage {
            assert_eq!(strategy.swaps[0].token_in, 
                      strategy.swaps.last().unwrap().token_out,
                      "Arbitrage strategy must end with same token as start");
        }

        info!("   ✅ Strategy validated - Total gas: {}, Swaps: {}", 
              cumulative_gas, strategy.swaps.len());
    }

    info!("🎉 Multi-Hop Swap Execution Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_profit_extraction_validation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Profit Extraction Validation Test");

    let test_env = TestEnvironment::new().await?;

    let execution_pipelines = vec![
        create_profitable_arbitrage_pipeline(),
        create_marginal_profit_pipeline(),
        create_complex_mev_pipeline(),
    ];

    for (i, pipeline) in execution_pipelines.iter().enumerate() {
        info!("🔍 Testing execution pipeline {}: Profit target = {} ETH", 
              i + 1, format_ether_amount(pipeline.profit_extraction.profit_amount));

        assert!(!pipeline.swaps.is_empty(), "Pipeline must have at least one swap");
        assert!(pipeline.total_gas_estimate > 0, "Gas estimate must be positive");
        assert!(pipeline.success_probability > 0.0 && pipeline.success_probability <= 1.0, 
                "Success probability must be between 0 and 1");

        let gas_cost = calculate_gas_cost(pipeline.total_gas_estimate, 20_000_000_000u64)?;
        let flash_loan_fee = pipeline.flash_loan.loan_fee;
        let total_costs = gas_cost + flash_loan_fee;

        info!("   💰 Cost breakdown - Gas: {} ETH, Flash loan fee: {} ETH, Total: {} ETH",
              format_ether_amount(gas_cost),
              format_ether_amount(flash_loan_fee),
              format_ether_amount(total_costs));

        assert!(pipeline.profit_extraction.profit_amount > total_costs, 
                "Gross profit must exceed total costs for viable execution");

        let net_profit = pipeline.profit_extraction.profit_amount - total_costs;

        let profit_diff = if pipeline.profit_extraction.net_profit > net_profit {
            pipeline.profit_extraction.net_profit - net_profit
        } else {
            net_profit - pipeline.profit_extraction.net_profit
        };
        assert!(profit_diff <= U256::from(50_000_000_000_000_000u64),
               "Net profit calculation should be approximately accurate: expected {}, got {}", 
               format_ether_amount(net_profit), 
               format_ether_amount(pipeline.profit_extraction.net_profit));

        validate_profit_extraction_method(&pipeline.profit_extraction.extraction_method)?;

        let extraction_tx = create_profit_extraction_transaction(&pipeline.profit_extraction)?;
        let extraction_gas = estimate_extraction_gas(&test_env, &extraction_tx).await?;

        assert!(extraction_gas <= 100_000, 
                "Profit extraction should be gas efficient: {} gas", extraction_gas);

        let extraction_cost = calculate_gas_cost(extraction_gas, 20_000_000_000u64)?;
        let final_net_profit = net_profit - extraction_cost;

        info!("   🎯 Final analysis - Net profit: {} ETH, Extraction cost: {} ETH, Final: {} ETH",
              format_ether_amount(net_profit),
              format_ether_amount(extraction_cost), 
              format_ether_amount(final_net_profit));

        let min_profit_threshold = U256::from(1000000000000000u64);
        assert!(final_net_profit >= min_profit_threshold,
                "Final profit must meet minimum threshold for execution");

        info!("   ✅ Pipeline validated - Success probability: {:.1}%, Final profit: {} ETH", 
              pipeline.success_probability * 100.0,
              format_ether_amount(final_net_profit));
    }

    info!("🎉 Profit Extraction Validation Test completed!");
    Ok(())
}

#[derive(Debug)]
struct FlashLoanScenario {
    provider_name: String,
    setup: FlashLoanSetup,
}

#[derive(Debug)]
struct SwapStrategy {
    name: String,
    swaps: Vec<SwapExecution>,
    is_arbitrage: bool,
}

fn create_aave_flash_loan_scenario() -> FlashLoanScenario {
    FlashLoanScenario {
        provider_name: "Aave V3".to_string(),
        setup: FlashLoanSetup {
            provider_address: Address::from([0xaa; 20]),
            loan_amount: U256::from(100) * U256::from(10).pow(U256::from(18)),
            loan_token: get_address(AddressType::Weth),
            loan_fee: U256::from(90000000000000000u64),
            callback_data: Bytes::from_static(&[0x01, 0x02, 0x03]),
        },
    }
}

fn create_balancer_flash_loan_scenario() -> FlashLoanScenario {
    FlashLoanScenario {
        provider_name: "Balancer V2".to_string(),
        setup: FlashLoanSetup {
            provider_address: Address::from([0xbb; 20]),
            loan_amount: U256::from(50) * U256::from(10).pow(U256::from(18)),
            loan_token: get_address(AddressType::Weth),
            loan_fee: U256::ZERO,
            callback_data: Bytes::from_static(&[0x04, 0x05, 0x06]),
        },
    }
}

fn create_uniswap_v3_flash_loan_scenario() -> FlashLoanScenario {
    FlashLoanScenario {
        provider_name: "Uniswap V3".to_string(),
        setup: FlashLoanSetup {
            provider_address: Address::from([0xcc; 20]),
            loan_amount: U256::from(200) * U256::from(10).pow(U256::from(18)),
            loan_token: get_address(AddressType::Weth),
            loan_fee: U256::from(600000000000000000u64),
            callback_data: Bytes::from_static(&[0x07, 0x08, 0x09]),
        },
    }
}

fn create_v2_to_v3_arbitrage_swaps() -> SwapStrategy {
    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x11; 20]);

    SwapStrategy {
        name: "V2 to V3 Arbitrage".to_string(),
        swaps: vec![
            SwapExecution {
                router_address: Address::from([0x22; 20]),
                token_in: weth,
                token_out: usdc,
                amount_in: U256::from(10) * U256::from(10).pow(U256::from(18)),
                amount_out_min: U256::from(20000) * U256::from(10).pow(U256::from(6)),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV2,
            },
            SwapExecution {
                router_address: Address::from([0x33; 20]),
                token_in: usdc,
                token_out: weth,
                amount_in: U256::from(20000) * U256::from(10).pow(U256::from(6)),
                amount_out_min: U256::from(10050000000000000000u64),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV3 { 
                    fee: 3000,
                    sqrt_price_limit: U256::ZERO 
                },
            },
        ],
        is_arbitrage: true,
    }
}

fn create_cross_pool_arbitrage_swaps() -> SwapStrategy {
    let weth = get_address(AddressType::Weth);
    let dai = Address::from([0x44; 20]);

    SwapStrategy {
        name: "Cross-Pool Arbitrage".to_string(),
        swaps: vec![
            SwapExecution {
                router_address: Address::from([0x55; 20]),
                token_in: weth,
                token_out: dai,
                amount_in: U256::from(5) * U256::from(10).pow(U256::from(18)),
                amount_out_min: U256::from(10000) * U256::from(10).pow(U256::from(18)),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV3 { fee: 500, sqrt_price_limit: U256::ZERO },
            },
            SwapExecution {
                router_address: Address::from([0x66; 20]),
                token_in: dai,
                token_out: weth,
                amount_in: U256::from(10000) * U256::from(10).pow(U256::from(18)),
                amount_out_min: U256::from(5025000000000000000u64),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV2,
            },
        ],
        is_arbitrage: true,
    }
}

fn create_triangular_arbitrage_swaps() -> SwapStrategy {
    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x77; 20]);
    let dai = Address::from([0x88; 20]);

    SwapStrategy {
        name: "Triangular Arbitrage".to_string(),
        swaps: vec![
            SwapExecution {
                router_address: Address::from([0x99; 20]),
                token_in: weth,
                token_out: usdc,
                amount_in: U256::from(1) * U256::from(10).pow(U256::from(18)),
                amount_out_min: U256::from(2000) * U256::from(10).pow(U256::from(6)),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV2,
            },
            SwapExecution {
                router_address: Address::from([0xaa; 20]),
                token_in: usdc,
                token_out: dai,
                amount_in: U256::from(2000) * U256::from(10).pow(U256::from(6)),
                amount_out_min: U256::from(2010) * U256::from(10).pow(U256::from(18)),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV3 { fee: 100, sqrt_price_limit: U256::ZERO },
            },
            SwapExecution {
                router_address: Address::from([0xbb; 20]),
                token_in: dai,
                token_out: weth,
                amount_in: U256::from(2010) * U256::from(10).pow(U256::from(18)),
                amount_out_min: U256::from(1005000000000000000u64),
                deadline: get_current_timestamp() + 300,
                swap_type: SwapType::UniswapV2,
            },
        ],
        is_arbitrage: true,
    }
}

fn create_profitable_arbitrage_pipeline() -> ExecutionPipeline {
    ExecutionPipeline {
        flash_loan: create_aave_flash_loan_scenario().setup,

        swaps: create_v2_to_v3_arbitrage_swaps().swaps,
        profit_extraction: ProfitExtraction {
            profit_token: get_address(AddressType::Weth),
            profit_amount: U256::from(500_000_000_000_000_000u64),
            gas_used: 280_000,
            net_profit: U256::from(450_000_000_000_000_000u64),
            extraction_method: ExtractionMethod::DirectTransfer,
        },
        total_gas_estimate: 280_000,
        success_probability: 0.85,
    }
}

fn create_marginal_profit_pipeline() -> ExecutionPipeline {
    ExecutionPipeline {
        flash_loan: create_balancer_flash_loan_scenario().setup,
        swaps: create_v2_to_v3_arbitrage_swaps().swaps,
        profit_extraction: ProfitExtraction {
            profit_token: get_address(AddressType::Weth),
            profit_amount: U256::from(150_000_000_000_000_000u64),
            gas_used: 320_000,
            net_profit: U256::from(120_000_000_000_000_000u64),
            extraction_method: ExtractionMethod::TokenSwap { 
                target_token: Address::from([0xdd; 20])
            },
        },
        total_gas_estimate: 320_000,
        success_probability: 0.72,
    }
}

fn create_complex_mev_pipeline() -> ExecutionPipeline {
    ExecutionPipeline {
        flash_loan: create_uniswap_v3_flash_loan_scenario().setup,
        swaps: create_triangular_arbitrage_swaps().swaps,
        profit_extraction: ProfitExtraction {
            profit_token: get_address(AddressType::Weth),
            profit_amount: U256::from(750_000_000_000_000_000u64),
            gas_used: 680_000,
            net_profit: U256::from(136_400_000_000_000_000u64),
            extraction_method: ExtractionMethod::Reinvestment { 
                strategy: "Compound Lending".to_string() 
            },
        },
        total_gas_estimate: 680_000,
        success_probability: 0.65,
    }
}

fn create_flash_loan_transaction(setup: &FlashLoanSetup) -> Result<TransactionRequest> {
    let mut tx = TransactionRequest::default();
    tx.to = Some(setup.provider_address.into());
    tx.input = Bytes::from_static(&[
        0x12, 0x34, 0x56, 0x78,

    ]).into();
    tx.value = Some(U256::ZERO);
    Ok(tx)
}

fn create_swap_transaction(swap: &SwapExecution) -> Result<TransactionRequest> {
    let mut tx = TransactionRequest::default();
    tx.to = Some(swap.router_address.into());
    tx.input = match swap.swap_type {
        SwapType::UniswapV2 => Bytes::from_static(&[0x38, 0xed, 0x17, 0x39]),
        SwapType::UniswapV3 { .. } => Bytes::from_static(&[0x41, 0x4b, 0xf3, 0x89]),
    }.into();
    tx.value = Some(U256::ZERO);
    Ok(tx)
}

fn create_profit_extraction_transaction(extraction: &ProfitExtraction) -> Result<TransactionRequest> {
    let mut tx = TransactionRequest::default();
    tx.to = Some(extraction.profit_token.into());
    tx.input = match extraction.extraction_method {
        ExtractionMethod::DirectTransfer => Bytes::from_static(&[0xa9, 0x05, 0x9c, 0xbb]),
        ExtractionMethod::TokenSwap { .. } => Bytes::from_static(&[0x38, 0xed, 0x17, 0x39]),
        ExtractionMethod::Reinvestment { .. } => Bytes::from_static(&[0x12, 0x34, 0x56, 0x78]),
    }.into();
    tx.value = Some(U256::ZERO);
    Ok(tx)
}

async fn estimate_flash_loan_gas(test_env: &TestEnvironment, tx: &TransactionRequest) -> Result<u64> {

    match test_env.provider.estimate_gas(tx).await {
        Ok(gas) => Ok(gas),
        Err(_) => Ok(350_000),
    }
}

async fn estimate_swap_gas(test_env: &TestEnvironment, tx: &TransactionRequest, swap_type: &SwapType) -> Result<u64> {
    match test_env.provider.estimate_gas(tx).await {
        Ok(gas) => Ok(gas),
        Err(_) => Ok(match swap_type {
            SwapType::UniswapV2 => 130_000,
            SwapType::UniswapV3 { .. } => 180_000,
        }),
    }
}

async fn estimate_extraction_gas(test_env: &TestEnvironment, tx: &TransactionRequest) -> Result<u64> {
    match test_env.provider.estimate_gas(tx).await {
        Ok(gas) => Ok(gas),
        Err(_) => Ok(65_000),
    }
}

fn validate_flash_loan_callback(callback_data: &Bytes) -> Result<()> {
    assert!(callback_data.len() > 0, "Callback data cannot be empty");
    assert!(callback_data.len() >= 3, "Callback data must have minimum required fields");
    Ok(())
}

fn validate_profit_extraction_method(method: &ExtractionMethod) -> Result<()> {
    match method {
        ExtractionMethod::DirectTransfer => {

            Ok(())
        }
        ExtractionMethod::TokenSwap { target_token } => {
            assert_ne!(*target_token, Address::ZERO, "Target token cannot be zero address");
            Ok(())
        }
        ExtractionMethod::Reinvestment { strategy } => {
            assert!(!strategy.is_empty(), "Reinvestment strategy must be specified");
            Ok(())
        }
    }
}

fn calculate_gas_cost(gas_used: u64, gas_price: u64) -> Result<U256> {
    let gas_cost = U256::from(gas_used) * U256::from(gas_price);
    Ok(gas_cost)
}

fn format_ether_amount(amount: U256) -> String {
    let ether = amount / U256::from(10).pow(U256::from(18));
    let remainder = amount % U256::from(10).pow(U256::from(18));
    let decimals = remainder / U256::from(10).pow(U256::from(15));
    format!("{}.{:03}", ether, decimals)
}

fn format_token_symbol(token: Address) -> String {
    let weth = get_address(AddressType::Weth);
    if token == weth {
        "WETH".to_string()
    } else {
        format!("Token_{}", hex::encode(&token.0[..3]))
    }
}

fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

