use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use alloy_primitives::aliases::U24;
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
    let wallet = EthereumWallet::from(signer.clone());

    let http_url = Url::from_str(http_url)
        .map_err(|e| anyhow::anyhow!("Invalid HTTP_URL format '{}': {}", http_url, e))?;
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet.clone())
        .on_http(http_url);

     let input_as_bytes = revm::primitives::Bytes::from(input);

    // Get the address from the signer instead of using a hardcoded address
    let from_address = signer.address();

    // Get the actual nonce from the provider instead of using a passed-in value
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
    //NOTE:  gas limit should be the amount of gas that was simulated for hte transaction to have taken up

    let tx = TransactionRequest::default()
        .with_from(from_address)
        .with_chain_id(1)
        .with_value(U256::ZERO)
        .with_input(input_as_bytes)
        .with_to(contract_address)
        .with_nonce(actual_nonce)
        // NOTE: this should be gas price?
        .with_max_fee_per_gas(base_fee.ok_or_else(|| anyhow::anyhow!("Base fee is required"))?)
        // NOTE: This too
        .with_max_priority_fee_per_gas(bribe.ok_or_else(|| anyhow::anyhow!("Priority fee (bribe) is required"))?)
        .with_gas_limit(gas_limit.ok_or_else(|| anyhow::anyhow!("Gas limit is required"))?);

    log::debug!("TX: {:?}", tx);

    let envelope = tx.build(&wallet).await?;
    let tx_hash = format!("{:?}", envelope.tx_hash());

    log::debug!("Pending TX Hash: {}", tx_hash);

    let pending = provider
        .send_tx_envelope(envelope)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {}", e))?
        .with_timeout(Some(std::time::Duration::from_secs_f32(20_f32)));

    // Use tokio::time::timeout to wrap the watch() call with a timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(25),
        pending.watch()
    ).await {
        Ok(Ok(receipt)) => {
            log::debug!("Transaction confirmed with receipt: {:?}", receipt);
            Ok(tx_hash)
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
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
        ) external;
    };

    let function_call = flashSwap_V3_to_V2Call {
        pool0: target_pool,
        fee1: fee,
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: amount,
    }
    .abi_encode();

    Ok(function_call)
}
