use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    network::{EthereumWallet, TransactionBuilder},
};
use alloy_primitives::{aliases::U24, TxKind};
use alloy_sol_types::SolCall;
use anyhow::Result;
use dotenv::var;

use reqwest::Url;
use std::str::FromStr;

pub async fn send_transaction(
    contract_address: Address,
    gas_price: Option<u128>,
    gas_limit: Option<u64>,
    base_fee: Option<u128>,
    bribe: Option<u128>,
    input: Vec<u8>,
    _nonce: u64,
) -> Result<String> {
    let http_url = var::<&str>("HTTP_URL")
        .map_err(|e| anyhow::anyhow!("HTTP_URL environment variable not set: {}", e))?;
    let http_url = http_url.as_str();

    let private_key = var("PRIVATE_KEY")
        .map_err(|e| anyhow::anyhow!("PRIVATE_KEY environment variable not set: {}", e))?;
    let signer = PrivateKeySigner::from_str(&private_key)
        .map_err(|e| anyhow::anyhow!("Invalid private key format: {}", e))?;

    let http_url = Url::from_str(http_url)
        .map_err(|e| anyhow::anyhow!("Invalid HTTP_URL format '{}': {}", http_url, e))?;
    // Create provider WITHOUT wallet - we'll sign manually
    let provider = ProviderBuilder::new()
        .on_http(http_url.clone());

     let input_as_bytes = revm::primitives::Bytes::from(input);

    // Get the address from the signer
    let from_address = signer.address();

    // Get the actual nonce from the provider
    let actual_nonce = provider
        .get_transaction_count(from_address)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get transaction nonce: {}", e))?;

    log::debug!(
        "Sending transaction with parameters:\n\
         contract_address: {}\n\
         from_address: {}\n\
         actual_nonce: {}\n\
         gas_price: {:?}\n\
         gas_limit: {:?}\n\
         base_fee: {:?}\n\
         bribe: {:?}",
        contract_address,
        from_address,
        actual_nonce,
        gas_price,
        gas_limit,
        base_fee,
        bribe.unwrap_or(0)
    );

    // Build transaction - provide all required fields to avoid filler issues
    let tx_req = alloy::rpc::types::TransactionRequest {
        from: Some(from_address),
        to: Some(TxKind::Call(contract_address)),
        value: Some(U256::ZERO),
        input: alloy::rpc::types::TransactionInput::new(input_as_bytes),
        nonce: Some(actual_nonce),
        gas: Some(gas_limit.ok_or_else(|| anyhow::anyhow!("Gas limit is required"))?),
        max_priority_fee_per_gas: Some(bribe.ok_or_else(|| anyhow::anyhow!("Priority fee (bribe) is required"))?),
        max_fee_per_gas: Some(base_fee.ok_or_else(|| anyhow::anyhow!("Base fee is required"))?),
        chain_id: Some(1),
        ..Default::default()
    };

    log::debug!("TX Request built: {:?}", tx_req);

    // Create a provider with wallet to sign the transaction
    let provider_with_wallet = ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer))
        .on_http(http_url.clone());

    // Sign the transaction 
    let signed_tx = provider_with_wallet
        .send_transaction(tx_req)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {}", e))?;

    let tx_hash_str = signed_tx.tx_hash().to_string();
    log::debug!("Signed TX Hash: {}", tx_hash_str);

    let pending = signed_tx
        .with_timeout(Some(std::time::Duration::from_secs_f32(20_f32)));

    // Use tokio::time::timeout to wrap the watch() call with a timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(25),
        pending.watch()
    ).await {
        Ok(Ok(receipt)) => {
            log::debug!("Transaction confirmed with receipt: {:?}", receipt);
            Ok(tx_hash_str)
        }
        Ok(Err(e)) => {
            log::error!("Transaction failed with error: {:?}", e);
            Err(anyhow::anyhow!("Transaction execution failed: {}", e))
        }
        Err(_) => {
            log::error!("Transaction confirmation timeout - no receipt received within 25 seconds");
            Err(anyhow::anyhow!("Transaction timeout: failed to confirm within 25 seconds"))
        }
    }
}

pub async fn create_input_data(
    target_pool: Address,
    fee: U24,
    token_in: Address,
    token_out: Address,
    amount: U256,
) -> Result<Vec<u8>> {
    alloy::sol! {
        #[derive(Debug)]
        function flashSwap_V2_to_V3(
            address v2Pool,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
            uint24 v3Fee
        ) external;
    };

    let function_call = flashSwap_V2_to_V3Call {
        v2Pool: target_pool,
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: amount,
        v3Fee: fee,
    }
    .abi_encode();

    Ok(function_call)
}
