use alloy::{
    hex::encode_prefixed,
    network::{Ethereum, EthereumWallet, NetworkWallet, TransactionBuilder},
    primitives::{Address, TxKind, U256},
    providers::{PendingTransactionBuilder, Provider, ProviderBuilder, SendableTx},
    rpc::types::{TransactionInput, TransactionRequest},
    signers::local::PrivateKeySigner,
    transports::{TransportErrorKind, TransportResult},
};
use alloy_primitives::{address, aliases::U24};
use alloy_sol_types::SolCall;
use anyhow::Result;
use dotenv::var;
use log::info;
use reqwest::Url;
use std::{str::FromStr, time::Duration};

pub async fn send_transaction(
    contract_address: Address,
    gas_price: Option<u128>,
    gas_limit: Option<u64>,
    base_fee: Option<u128>,
    bribe: Option<u128>,
    input: Vec<u8>,
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

    let input_as_bytes = revm::primitives::Bytes::from(input);

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
        bribe.unwrap(),
        nonce
    );
    //NOTE:  gas limit should be the amount of gas that was simulated for hte transaction to have taken up

    let tx = TransactionRequest::default()
        .with_from(address!("5f1F5565561aC146d24B102D9CDC288992Ab2938"))
        .with_chain_id(1)
        .with_value(U256::ZERO)
        .with_input(input_as_bytes)
        .with_to(contract_address)
        .with_nonce(nonce)
        // NOTE: this should be gas price?
        .with_max_fee_per_gas(base_fee.unwrap())
        // NOTE: This too
        .with_max_priority_fee_per_gas(bribe.unwrap())
        .with_gas_limit(gas_limit.unwrap());

    info!("TX: {:?}", tx);

    let envelope = tx.build(&wallet).await?;

    info!("Pending TX Hash: {:?}", envelope.tx_hash());

    let pending = provider
        .send_tx_envelope(envelope)
        .await
        .unwrap()
        .with_timeout(Some(std::time::Duration::from_secs_f32(20_f32)));

    let res = pending.watch().await?;

    info!("Res: {:?}", res);
    Ok(())
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
