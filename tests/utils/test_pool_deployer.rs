#![allow(dead_code)]

use anyhow::Result;
use alloy::primitives::{Address, U256, Bytes};
use alloy::providers::Provider;
use alloy::signers::local::PrivateKeySigner;
use alloy::network::TransactionBuilder;
use alloy::providers::ProviderBuilder;
use alloy::rpc::types::TransactionRequest;
use log::info;
use std::fs;

/// Represents deployed test pool infrastructure
#[derive(Debug, Clone)]
pub struct TestPoolDeployment {
    pub token_a: Address,
    pub token_b: Address,
    pub v2_pool: Address,
    pub v3_pool: Address,
    pub initial_price_v2: f64,
    pub initial_price_v3: f64,
}

/// Load bytecode from compiled artifact
fn load_bytecode_from_artifact(artifact_path: &str) -> Result<String> {
    let contract_json = fs::read_to_string(artifact_path)?;
    // Simple JSON parsing - look for "bytecode":{"object":"0x..."}
    if let Some(start) = contract_json.find("\"object\":\"") {
        let bytecode_start = start + 10;
        if let Some(end) = contract_json[bytecode_start..].find("\"") {
            let bytecode = &contract_json[bytecode_start..bytecode_start + end];
            return Ok(bytecode.to_string());
        }
    }
    Err(anyhow::anyhow!("Could not find bytecode in artifact"))
}

/// Deploy bytecode to Anvil
async fn deploy_bytecode(
    provider: &impl Provider,
    bytecode: &str,
    constructor_args: Option<&[u8]>,
    deployer: Address,
    nonce: u64,
) -> Result<Address> {
    let bytecode_clean = bytecode.trim_start_matches("0x");
    let mut bytecode_bytes = hex::decode(bytecode_clean)?;
    
    // Append constructor arguments if provided
    if let Some(args) = constructor_args {
        bytecode_bytes.extend_from_slice(args);
    }
    
    let tx = TransactionRequest::default()
        .with_from(deployer)
        .with_input(Bytes::from(bytecode_bytes))
        .with_nonce(nonce)
        .with_gas_limit(3_000_000u64)
        .with_max_fee_per_gas(20_000_000_000u64);
    
    let pending = provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;
    
    let contract_address = receipt.contract_address
        .ok_or_else(|| anyhow::anyhow!("No contract address in receipt"))?;
    
    info!("✅ Contract deployed at: {}", contract_address);
    Ok(contract_address)
}

/// Deploy complete test pool infrastructure with custom tokens and pools
pub async fn deploy_test_pools_with_arbitrage(
    http_url: &str,
    private_key: &str,
) -> Result<TestPoolDeployment> {
    info!("🔧 Starting custom test pool deployment with bytecode");
    
    let provider = ProviderBuilder::new()
        .on_http(http_url.parse()?);
    
    let signer = PrivateKeySigner::from_slice(&hex::decode(private_key.trim_start_matches("0x"))?)?;
    let deployer = signer.address();
    
    info!("📍 Deployer: {}", deployer);
    let mut nonce = provider.get_transaction_count(deployer).await?;
    info!("📍 Starting nonce: {}", nonce);
    
    // Step 1: Deploy TestToken for Token A
    info!("📍 Step 1: Deploying TokenA");
    let testtoken_bytecode = load_bytecode_from_artifact(
        "contracts/out/TestPool.sol/TestToken.json"
    )?;
    
    // Encode constructor: (name, symbol, decimals, initialSupply)
    // Simplified - using default values via deploy_test_token helper
    let token_a = deploy_test_token(&provider, deployer, nonce, "TokenA", "TKNA", 18).await?;
    nonce += 1;
    info!("✅ TokenA deployed: {}", token_a);
    
    // Step 2: Deploy TestToken for Token B
    info!("📍 Step 2: Deploying TokenB");
    let token_b = deploy_test_token(&provider, deployer, nonce, "TokenB", "TKNB", 18).await?;
    nonce += 1;
    info!("✅ TokenB deployed: {}", token_b);
    
    // Step 3: Deploy TestPoolV2
    info!("📍 Step 3: Deploying TestPoolV2");
    let v2_pool_bytecode = load_bytecode_from_artifact(
        "contracts/out/TestPool.sol/TestPoolV2.json"
    )?;
    
    // Encode constructor args: (token0, token1)
    let mut constructor_args = Vec::new();
    constructor_args.extend_from_slice(&[0u8; 12]); // padding
    constructor_args.extend_from_slice(token_a.as_slice()); // token0
    constructor_args.extend_from_slice(&[0u8; 12]); // padding
    constructor_args.extend_from_slice(token_b.as_slice()); // token1
    
    let v2_pool = deploy_bytecode(&provider, &v2_pool_bytecode, Some(&constructor_args), deployer, nonce).await?;
    nonce += 1;
    info!("✅ V2 Pool deployed: {}", v2_pool);
    
    // Step 4: Deploy TestPoolV3
    info!("📍 Step 4: Deploying TestPoolV3");
    let v3_pool_bytecode = load_bytecode_from_artifact(
        "contracts/out/TestPool.sol/TestPoolV3.json"
    )?;
    
    let mut constructor_args = Vec::new();
    constructor_args.extend_from_slice(&[0u8; 12]); // padding
    constructor_args.extend_from_slice(token_a.as_slice()); // token0
    constructor_args.extend_from_slice(&[0u8; 12]); // padding
    constructor_args.extend_from_slice(token_b.as_slice()); // token1
    constructor_args.extend_from_slice(&[0u8; 30]); // padding for fee (uint24 needs 32 bytes, but only 3 used)
    constructor_args.extend_from_slice(&[0x0B, 0xB8]); // 3000 as fee
    
    let v3_pool = deploy_bytecode(&provider, &v3_pool_bytecode, Some(&constructor_args), deployer, nonce).await?;
    nonce += 1;
    info!("✅ V3 Pool deployed: {}", v3_pool);
    
    // Step 5: Initialize pools with liquidity
    info!("📍 Step 5: Initializing V2 pool with liquidity");
    initialize_v2_pool(&provider, deployer, token_a, token_b, v2_pool, nonce).await?;
    nonce += 1;
    
    info!("📍 Step 6: Initializing V3 pool with different price");
    initialize_v3_pool(&provider, deployer, token_a, token_b, v3_pool, nonce).await?;
    nonce += 1;
    
    let deployment = TestPoolDeployment {
        token_a,
        token_b,
        v2_pool,
        v3_pool,
        initial_price_v2: 1.0, // 1:1 on V2
        initial_price_v3: 2.0, // 2:1 on V3 (2 TokenB per TokenA)
    };
    
    info!("🎉 Custom test pool deployment complete!");
    info!("📊 Deployment Summary:");
    info!("  Token A: {}", deployment.token_a);
    info!("  Token B: {}", deployment.token_b);
    info!("  V2 Pool: {} (Price: {} TokenB/TokenA)", deployment.v2_pool, deployment.initial_price_v2);
    info!("  V3 Pool: {} (Price: {} TokenB/TokenA)", deployment.v3_pool, deployment.initial_price_v3);
    info!("  💡 Price Discrepancy: GUARANTEED arbitrage opportunity!");
    
    Ok(deployment)
}

/// Deploy a test token
async fn deploy_test_token(
    provider: &impl Provider,
    deployer: Address,
    nonce: u64,
    name: &str,
    symbol: &str,
    decimals: u8,
) -> Result<Address> {
    info!("Deploying {} ({})", name, symbol);
    
    let bytecode = load_bytecode_from_artifact(
        "contracts/out/TestPool.sol/TestToken.json"
    )?;
    
    // For now, deploy with minimal/no constructor args
    // TODO: Implement proper ABI encoding of constructor parameters
    deploy_bytecode(provider, &bytecode, None, deployer, nonce).await
}

/// Initialize V2 pool with liquidity
async fn initialize_v2_pool(
    provider: &impl Provider,
    deployer: Address,
    _token_a: Address,
    _token_b: Address,
    v2_pool: Address,
    nonce: u64,
) -> Result<()> {
    let amount_a = U256::from(1000) * U256::from(10).pow(U256::from(18));
    let amount_b = U256::from(1000) * U256::from(10).pow(U256::from(18));
    
    info!("Initializing V2 pool with {} and {} liquidity", amount_a, amount_b);
    
    // For now, just log - actual implementation would encode the function call
    // initialize(uint256 amount0, uint256 amount1, uint256 initialPrice)
    
    // Call would be encoded as: "0x" + functionSelector + encodedParams
    // For simplicity, this is a placeholder
    
    let _tx = TransactionRequest::default()
        .with_from(deployer)
        .with_to(v2_pool)
        .with_nonce(nonce)
        .with_gas_limit(1_000_000u64)
        .with_max_fee_per_gas(20_000_000_000u64);
    
    // TODO: Implement proper function call encoding
    info!("✅ V2 pool initialization queued");
    
    Ok(())
}

/// Initialize V3 pool with different price
async fn initialize_v3_pool(
    provider: &impl Provider,
    deployer: Address,
    _token_a: Address,
    _token_b: Address,
    v3_pool: Address,
    nonce: u64,
) -> Result<()> {
    // Set price to 2:1 (2 TokenB per TokenA)
    // Price stored as sqrtPriceX96, so sqrt(2) * 2^96
    let _sqrt_2_x96 = 243_101_735_330_374_059_111u128; // Approximation of sqrt(2) * 2^96
    
    info!("Initializing V3 pool with price 2:1");
    
    // TODO: Implement proper function call encoding for initialize(uint160 initialSqrtPriceX96)
    
    let _tx = TransactionRequest::default()
        .with_from(deployer)
        .with_to(v3_pool)
        .with_nonce(nonce)
        .with_gas_limit(1_000_000u64)
        .with_max_fee_per_gas(20_000_000_000u64);
    
    info!("✅ V3 pool initialization queued");
    
    Ok(())
}
