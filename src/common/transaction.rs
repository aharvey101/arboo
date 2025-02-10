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
    max_fee_per_gas: Option<u128>,
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


    let tx = TransactionRequest::default()
        .with_from(signer.clone().address())
        .with_input(input_as_bytes)
        .with_to(contract_address)
        .with_nonce(nonce)
        .with_max_fee_per_gas(max_fee_per_gas.unwrap())
        .with_max_priority_fee_per_gas(gas_price.unwrap())
        .with_gas_limit(gas_limit.unwrap());

    let is_1559 = tx.complete_1559().unwrap();

    info!("Transaction: {:?}", is_1559);
    // Sign the transaction. 
    let envelope = tx.build(&wallet).await?;

    // Send the raw transaction. The transaction is sent to the Flashbots relay and, if valid, will
    // be included in a block by a Flashbots builder. Note that the transaction request, as defined,
    // is invalid and will not be included in the blockchain.
    let pending = provider.send_tx_envelope(envelope).await?.register().await?;

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
