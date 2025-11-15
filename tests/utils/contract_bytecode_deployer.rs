#![allow(dead_code)]

use anyhow::Result;
use alloy::primitives::{Address, Bytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::signers::local::PrivateKeySigner;
use log::info;
use std::str::FromStr;

/// Deploys contract bytecode to Anvil and returns the deployed contract address
pub async fn deploy_contract_bytecode(
    http_url: &str,
    bytecode: &str,
    private_key: &str,
) -> Result<Address> {
    // Remove '0x' prefix if present
    let bytecode_clean = bytecode.trim_start_matches("0x");
    
    // Decode hex string to bytes
    let bytecode_bytes = hex::decode(bytecode_clean)?;
    
    info!("🔧 Deploying contract with {} bytes of bytecode", bytecode_bytes.len());
    
    // Parse private key
    let signer = PrivateKeySigner::from_str(private_key)?;
    let wallet = EthereumWallet::from(signer.clone());
    let from_address = signer.address();
    
    info!("📍 Deploying from address: {}", from_address);
    
     // Create HTTP provider WITHOUT wallet for deployment
    let url = http_url.parse()?;
    info!("🔧 Creating provider for deployment...");
    let provider = ProviderBuilder::new()
        .on_http(url);
    info!("✅ Provider created successfully");
    
    // Get current nonce before sending transaction
    let nonce_before = provider.get_transaction_count(from_address).await?;
    info!("📍 Current nonce: {}", nonce_before);
    
    // Create deployment transaction - contract creation doesn't need a 'to' field
    // We'll send raw transaction data instead
    let tx_data = alloy::rpc::types::TransactionRequest {
        from: Some(from_address),
        to: None,
        value: Some(alloy::primitives::U256::ZERO),
        input: alloy::rpc::types::TransactionInput::new(Bytes::from(bytecode_bytes)),
        nonce: Some(nonce_before),
        gas: Some(3_000_000),
        max_priority_fee_per_gas: Some(2_000_000_000),
        max_fee_per_gas: Some(10_000_000_000),
        ..Default::default()
    };
    
    // Send the transaction directly without building
    // (the send_transaction method handles all the building internally)
    info!("📤 Sending deployment transaction");
    
    let pending = provider
        .send_transaction(tx_data)
        .await?
        .with_timeout(Some(std::time::Duration::from_secs(30)));
    
    // Wait for receipt
    let _receipt = tokio::time::timeout(
        std::time::Duration::from_secs(40),
        pending.watch()
    ).await??;
    
    info!("✅ Transaction mined");
    
    // Wait a bit for Anvil to update
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // Compute the contract address deterministically
    // For Ethereum, contract_address = keccak256(RLP([sender_address, sender_nonce]))[12:]
    
    // Import the necessary functions for address computation
    use alloy::primitives::keccak256;
    
    // Manually encode [sender, nonce] in RLP format
    let mut rlp_data = Vec::new();
    
    let address_bytes = from_address.as_slice();
    let nonce_bytes = encode_nonce(nonce_before);
    
    // Calculate total length
    let payload_len = 1 + 20 + 1 + nonce_bytes.len();
    let total_len = 1 + payload_len;
    
    // List prefix
    if total_len < 56 {
        rlp_data.push(0xC0 + total_len as u8);
    } else {
        rlp_data.push(0xF7);
        rlp_data.extend_from_slice(&(total_len as u64).to_be_bytes());
    }
    
    // Address encoding: 0x94 (0x80 + 20)
    rlp_data.push(0x94);
    rlp_data.extend_from_slice(address_bytes);
    
    // Nonce encoding
    if nonce_before == 0 {
        rlp_data.push(0x80);
    } else if nonce_before < 128 {
        rlp_data.push(nonce_before as u8);
    } else {
        let nonce_len = nonce_bytes.len() as u8;
        rlp_data.push(0x80 + nonce_len);
        rlp_data.extend_from_slice(&nonce_bytes);
    }
    
    let hash = keccak256(&rlp_data);
    let contract_address = Address::from_slice(&hash[12..]);
    
    info!("✅ Contract deployed at: {} (computed from sender nonce {})", contract_address, nonce_before);
    
    Ok(contract_address)
}

/// Helper function to encode nonce in minimal bytes
fn encode_nonce(nonce: u64) -> Vec<u8> {
    if nonce == 0 {
        vec![]
    } else {
        let mut bytes = Vec::new();
        let mut n = nonce;
        while n > 0 {
            bytes.insert(0, (n & 0xFF) as u8);
            n >>= 8;
        }
        bytes
    }
}

/// Load bytecode from hex file
pub fn load_bytecode_from_file(file_path: &str) -> Result<String> {
    let bytecode = std::fs::read_to_string(file_path)?;
    Ok(bytecode.trim().to_string())
}

/// Deploy both V3 and V2 flash swap contracts
pub async fn deploy_arbitrage_contracts(
    http_url: &str,
    private_key: &str,
) -> Result<(Address, Address)> {
    info!("🔧 Deploying arbitrage contracts to Anvil...");
    
    // Load bytecodes
    let v3_bytecode = load_bytecode_from_file("src/bytecode/uniswapV3flashSwap.hex")?;
    let v2_bytecode = load_bytecode_from_file("src/bytecode/v2_flash_to_v3_swap.hex")?;
    
    // Deploy V3 contract
    info!("📦 Deploying V3 Flash Swap contract...");
    let v3_contract_address = deploy_contract_bytecode(http_url, &v3_bytecode, private_key).await?;
    
    // Deploy V2 contract
    info!("📦 Deploying V2 Flash Swap contract...");
    let v2_contract_address = deploy_contract_bytecode(http_url, &v2_bytecode, private_key).await?;
    
    info!("✅ Both arbitrage contracts deployed:");
    info!("   V3 Flash Swap: {}", v3_contract_address);
    info!("   V2 Flash Swap: {}", v2_contract_address);
    
    Ok((v3_contract_address, v2_contract_address))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_bytecode() -> Result<()> {
        let bytecode = load_bytecode_from_file("src/bytecode/uniswapV3flashSwap.hex")?;
        assert!(!bytecode.is_empty());
        assert!(bytecode.starts_with("0x"));
        Ok(())
    }
}
