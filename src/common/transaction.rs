use alloy::{
    network::EthereumWallet,
    primitives::{Address, TxKind, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{TransactionInput, TransactionRequest},
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
    max_fee_per_gas: Option<u128>,
    gas_price: Option<u128>,
    input: TransactionInput,
)-> Result<()> {

    let private_key = var("PRIVATE_KEY").unwrap();
    let http_url = var::<&str>("HTTP_URL").unwrap();
    let http_url = http_url.as_str();

    let signer = PrivateKeySigner::from_str(&private_key).unwrap();
    let wallet = EthereumWallet::from(signer);
    let http_url = Url::from_str(http_url).unwrap();
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(http_url);

    let tx = TransactionRequest {
        to: Some(TxKind::Call(contract_address)),
        value: None,
        max_fee_per_gas,
        gas_price,
        input,
        ..Default::default()
    };

    let pending_tx = provider.send_transaction(tx).await.unwrap();

    println!("Pending Tx: {:?}", pending_tx);

    let tx =pending_tx.with_required_confirmations(1).watch().await?;
    

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
