use alloy::{
    network::{EthereumWallet, NetworkWallet, TransactionBuilder},
    primitives::{Address, TxKind, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{TransactionInput, TransactionRequest},
    signers::local::PrivateKeySigner,
};
use log::info;
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
    input: TransactionInput,
    nonce: u64,
) -> Result<()> {
    let http_url = var::<&str>("HTTP_URL").unwrap();
    let http_url = http_url.as_str();

    let private_key = var("PRIVATE_KEY").unwrap();
    let signer = PrivateKeySigner::from_str(&private_key).unwrap();
    let wallet = EthereumWallet::from(signer.clone());
    let http_url = Url::from_str(http_url).unwrap();
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet.clone())
        .on_http(http_url);

    let input_as_bytes = input.input.as_ref().unwrap().0.clone();

    info!(
        "Sending transaction with parameters:\n\
        contract_address: {}\n\
        gas_price: {:?}\n\
        gas_limit: {:?}\n\
        base_fee: {:?}\n\
        bribe: {:?}\n\
        nonce: {}",
        contract_address,
        gas_price,
        gas_limit,
        base_fee,
        6000000000000000u128,
        nonce
    );
   // gas limit should be the amount of gas that was simulated for hte transaction to have taken up 
   //  
    let tx = TransactionRequest::default()
        .with_from(signer.clone().address())
        .with_input(input_as_bytes)
        .with_to(contract_address)
        .with_nonce(nonce)
        .with_max_fee_per_gas(base_fee.unwrap())
        .with_max_priority_fee_per_gas(bribe.unwrap())
        .with_gas_limit(gas_limit.unwrap());

    // Sign the transaction. 
    let envelope = tx.build(&wallet).await?;

    info!("Signed transaction: {}", envelope.tx_hash());

    // Send the raw transaction. The transaction is sent to the Flashbots relay and, if valid, will
    // be included in a block by a Flashbots builder. Note that the transaction request, as defined,
    // is invalid and will not be included in the blockchain.
    let pending = match provider.send_tx_envelope(envelope).await {
        Ok(tx) => match tx.register().await {
            Ok(p) => p,
            Err(e) => {
                info!("Failed to register transaction: {}", e);
                return Err(e.into());
            }
        },
        Err(e) => {
            info!("Failed to send transaction: {}", e);
            return Err(e.into());
        }
    };

    info!("Sent transaction: {}", pending.tx_hash());


    Ok(())
}

pub async fn create_input_data(
    target_pool: Address,
    fee: U24,
    token_in: Address,
    token_out: Address,
    amount: U256,
) -> Result<TransactionInput> {
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

    let bytes = TransactionInput {
        input: Some(alloy_primitives::Bytes::from(function_call)),
        data: None,
    };

    Ok(bytes)
}
