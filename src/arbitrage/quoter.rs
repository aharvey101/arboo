use crate::arbitrage::simulation::arboo_bytecode;
use crate::common::revm::{EvmSimulator, Tx};
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::primitives::U64;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
use anyhow::Result;
use revm::primitives::{Address, Bytecode, U256};
use std::str::FromStr;
use std::sync::Arc;

sol! {
        function getUniswapV2Quote(address tokenIn, address tokenOut, uint256 amountIn) external view returns (uint256);
        function getUniswapV3Quote(address[] memory path, uint256 amountIn) external returns (uint256);
}

pub async fn execute_quotes(amount: U256, token_a: Address, token_b: Address) -> Result<()> {
    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));

    let provider: RootProvider<PubSubFrontend, AnyNetwork> = ProviderBuilder::new()
        .network()
        .on_ws(ws_client)
        .await
        .unwrap();

    let provider = Arc::new(provider);

    let latest_block_number = provider.get_block_number().await.unwrap();
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
        .await?
        .expect("Expected block");

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    let contract_wallet = PrivateKeySigner::random();
    let quoter_address = contract_wallet.address();

    let mut simulator = EvmSimulator::new(
        provider.clone(),
        Some(quoter_address),
        U64::from(latest_block_number),
    );

    // do v2 swap

    let call = getUniswapV2QuoteCall {
        tokenIn: token_a,
        tokenOut: token_b,
        amountIn: amount,
    };
    let calldata = call.abi_encode();

    let tx = Tx {
        caller: Address::ZERO,
        transact_to: quoter_address,
        data: calldata.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price.clone(),
        gas_limit: latest_gas_limit.clone(),
    };

    let result = simulator
        .staticcall(tx)
        .expect("shouldn't have failed static_call");
    println!("v2 result: {:?}", result);

    // do v3 swap
    simulator.deploy(arboo_bytecode());

    let call = getUniswapV3QuoteCall {
        path: vec![token_a, token_b],
        amountIn: amount,
    };
    let calldata = call.abi_encode();

    let tx = Tx {
        caller: Address::ZERO,
        transact_to: quoter_address,
        data: calldata.into(),
        value: U256::ZERO,
        gas_price: latest_gas_price.clone(),
        gas_limit: latest_gas_limit.clone(),
    };

    let result = simulator
        .staticcall(tx)
        .expect("shouldn't have failed static_call");
    println!("result: {:?}", result);

    Ok(())
}

pub fn quoter_bytecode() -> Bytecode {
    let bytes = hex::decode("608060405234801561000f575f80fd5b5060043610610034575f3560e01c8063049a1c3714610038578063827f09f61461005d575b5f80fd5b61004b61004636600461024e565b610070565b60405190815260200160405180910390f35b61004b61006b3660046102f4565b610197565b6040805160028082526060820183525f928392919060208301908036833701905050905084815f815181106100a7576100a7610394565b60200260200101906001600160a01b031690816001600160a01b03168152505083816001815181106100db576100db610394565b6001600160a01b039092166020928302919091019091015260405163d06ca61f60e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d9063d06ca61f9061012e90879086906004016103a8565b5f60405180830381865afa158015610148573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f1916820160405261016f91908101906103ff565b90508060018151811061018457610184610394565b6020026020010151925050509392505050565b5f80546040516001600160a01b039091169063cdca1753906101bd90869060200161048b565b604051602081830303815290604052846040518363ffffffff1660e01b81526004016101ea9291906104c9565b5f604051808303815f875af1158015610205573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f1916820160405261022c9190810190610593565b509195945050505050565b6001600160a01b038116811461024b575f80fd5b50565b5f805f60608486031215610260575f80fd5b833561026b81610237565b9250602084013561027b81610237565b929592945050506040919091013590565b634e487b7160e01b5f52604160045260245ffd5b604051601f8201601f1916810167ffffffffffffffff811182821017156102c9576102c961028c565b604052919050565b5f67ffffffffffffffff8211156102ea576102ea61028c565b5060051b60200190565b5f8060408385031215610305575f80fd5b823567ffffffffffffffff81111561031b575f80fd5b8301601f8101851361032b575f80fd5b8035602061034061033b836102d1565b6102a0565b82815260059290921b8301810191818101908884111561035e575f80fd5b938201935b8385101561038557843561037681610237565b82529382019390820190610363565b98969091013596505050505050565b634e487b7160e01b5f52603260045260245ffd5b5f604082018483526020604060208501528185518084526060860191506020870193505f5b818110156103f25784516001600160a01b0316835293830193918301916001016103cd565b5090979650505050505050565b5f6020808385031215610410575f80fd5b825167ffffffffffffffff811115610426575f80fd5b8301601f81018513610436575f80fd5b805161044461033b826102d1565b81815260059190911b82018301908381019087831115610462575f80fd5b928401925b8284101561048057835182529284019290840190610467565b979650505050505050565b81515f9082906020808601845b838110156104bd5781516001600160a01b031685529382019390820190600101610498565b50929695505050505050565b604081525f83518060408401525f5b818110156104f557602081870181015160608684010152016104d8565b505f606082850101526060601f19601f8301168401019150508260208301529392505050565b5f82601f83011261052a575f80fd5b8151602061053a61033b836102d1565b8083825260208201915060208460051b87010193508684111561055b575f80fd5b602086015b8481101561058857805163ffffffff8116811461057b575f80fd5b8352918301918301610560565b509695505050505050565b5f805f80608085870312156105a6575f80fd5b8451935060208086015167ffffffffffffffff808211156105c5575f80fd5b818801915088601f8301126105d8575f80fd5b81516105e661033b826102d1565b81815260059190911b8301840190848101908b831115610604575f80fd5b938501935b8285101561062b57845161061c81610237565b82529385019390850190610609565b60408b01519098509450505080831115610643575f80fd5b50506106518782880161051b565b60609690960151949793965050505056fea2646970667358221220b31e298fe7b7a1a236c2d9482eef573d7f97a30f9f6ced679d9a5e26f571933f64736f6c63430008180033").unwrap();
    return Bytecode::new_raw(bytes.into());
}
