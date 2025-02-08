use alloy::{
    primitives::{Address, TxKind, U256},
    providers::{ext::AnvilApi, Provider, ProviderBuilder, RootProvider},
    pubsub::PubSubFrontend,
    rpc::types::{TransactionInput, TransactionRequest}, 
    signers::local::LocalSigner,
};
use reqwest::Url;
use alloy_primitives::aliases::U24;
use anyhow::Result;
use std::{str::FromStr, sync::Arc};
use alloy_sol_types::{SolCall, SolValue};
use dotenv::var;
// 1. Create the transaction Input Data
// 2. Send the transaction

pub async fn send_transaction(
    provider: Arc<RootProvider<PubSubFrontend>>,
    amount: U256,
    contract_address: Address,
    target_pool: Address,
    max_fee_per_gas: Option<u128>,
    gas_price: Option<u128>,
    input: TransactionInput,
) -> Result<()> {
    // create our own http provider
    // get wallet from my local private key
    let private_key = var("PRIVATE_KEY").unwrap();
    let wallet  = LocalSigner::from_str(&private_key).unwrap();


    let wallet_address = wallet.address();
   

    let http_url = var::<&str>("HTTP_URL").unwrap();
    let http_url = http_url.as_str();
    let http_url = Url::from_str(http_url).unwrap();
    let provider = ProviderBuilder::new().on_http(http_url);

    // Create the transaction request
    let tx = TransactionRequest {
        from: Some(wallet_address),
        to: Some(TxKind::Call(contract_address)),
        value: Some(amount),
        max_fee_per_gas,
        gas_price,
        input,
        ..Default::default()
    };

    // Sign the transaction

    // let signed_tx = wallet.sign_transaction_sync(&tx).unwrap();



    // .to(target_pool)
    // .value(amount)
    // .gas_price(provider.get_gas_price().await?)
    // .gas(21000); // Basic ETH transfer gas limit

    // Send the transaction
    // let pending_tx = provider.send_transaction(tx).await?;

    // Wait for transaction to be mined

    // println!("Transaction successful with hash: {:?}", receipt.transaction_hash);

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
    }.abi_encode();

    let bytes = TransactionInput{
        input: Some(alloy_primitives::Bytes::from(function_call)),
        data: None,
    };

    Ok(bytes)
}
