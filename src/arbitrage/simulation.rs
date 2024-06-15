use crate::common::revm::{EvmSimulator, Tx};
use alloy::contract::{ContractInstance, Interface};
use alloy::dyn_abi::DynSolValue;
use alloy::eips::BlockId;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::wallet::LocalWallet;

use alloy_json_abi::JsonAbi;
use anyhow::Result;
use revm::primitives::{address, AccountInfo, Address, Bytecode, FixedBytes, U256};
use revm::DummyStateDB;
use std::str::FromStr;

pub async fn simulation(target_pool: Address, adjacent_pool: Address) -> Result<()> {
    //      - Simulation:
    //          - Simply we are going to simulate:
    //          - borrowing from the lower priced pool
    //          - selling to higher priced exchange for weth
    //          - using weth gained, by required amount on lower priced exchange
    //          - pay back loan with fee
    //          - revenue will be weth gained - weth used to repay loan

    // initialise cache and addresses
    // let http_url = var("HTTP_URL").expect("No HTTP_URL").to_string();
    // let http_url = Url::from_str(http_url.as_str()).unwrap();
    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider = ProviderBuilder::new().on_ws(ws_client).await?;
    // let mut cache_db = init_cache(http_url).await?;
    let mem_db = DummyStateDB::default();

    let latest_block_number = provider.get_block_number().await?;
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, true)
        .await?
        .expect("Expected block");
    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gast_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    let wallet = LocalWallet::random();
    let wallet_address = wallet.address().to_string();
    let wallet_address = Address::from_str(wallet_address.as_str())?;

    let contract_info = AccountInfo::new(U256::ZERO, 0, FixedBytes::ZERO, arboo_bytecode());
    let mut simulator = EvmSimulator::new_with_db(wallet_address, latest_block_number, mem_db);
    simulator.deploy(wallet_address, contract_info.code.unwrap());

    let abi = json_abi();

    let abi = Interface::new(abi);
    // make Contract from abi
    let contract_address =
        alloy::primitives::Address::from_str(simulator.owner.to_string().as_str()).unwrap();
    let contract = ContractInstance::<Address, RootProvider<PubSubFrontend>, Interface>::new(
        contract_address,
        provider.clone(),
        abi,
    );
    // set initial eth value;
    let alloy_wallet_address = Address::from_str(wallet.address().to_string().as_str()).unwrap();
    let initial_eth_balance = U256::from(100) * U256::from(10).pow(U256::from(18));
    simulator.set_eth_balance(alloy_wallet_address, initial_eth_balance);

    let value1: DynSolValue = U256::from(1000000000000000000 as i64).into();
    let value2: DynSolValue = U256::ZERO.into();
    let function_call = contract.encode_input("flash", &[value1, value2])?;

    let function_call_data = revm::primitives::Bytes::copy_from_slice(&*function_call);

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: simulator.owner,
        data: function_call_data,
        value: U256::default(),
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gast_price,
    };
    // simulate that transaction working
    let gas_used = match simulator.call(new_tx) {
        Ok(result) => result.gas_used,
        Err(_) => 0,
    };

    println!("Gas Used :{:?}", gas_used);

    Ok(())
}
pub fn one_ether() -> U256 {
    "1000000000000000000".parse().unwrap()
}

pub fn me() -> Address {
    address!("0000000000000000000000000000000000000001")
}

pub fn weth_addr() -> Address {
    address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
}

pub fn usdc_addr() -> Address {
    address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
}

pub fn official_quoter_addr() -> Address {
    address!("61fFE014bA17989E743c5F6cB21bF9697530B21e")
}

pub fn custom_quoter_addr() -> Address {
    address!("A5C381211A406b48A073E954e6949B0D49506bc0")
}

// WETH/USDC fee 500
pub fn pool_500_addr() -> Address {
    address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
}

// WETH/USDC fee 3000
pub fn pool_3000_addr() -> Address {
    address!("8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8")
}

pub fn arboo_bytecode() -> Bytecode {
    let bytes = "0x60e060405234801561001057600080fd5b506040516109ba3803806109ba83398101604081905261002f9161013e565b6001600160a01b038116608081905260408051630dfe168160e01b81529051630dfe1681916004808201926020929091908290030181865afa158015610079573d6000803e3d6000fd5b505050506040513d601f19601f8201168201806040525081019061009d919061013e565b6001600160a01b031660a0816001600160a01b0316815250506080516001600160a01b031663d21220a76040518163ffffffff1660e01b8152600401602060405180830381865afa1580156100f6573d6000803e3d6000fd5b505050506040513d601f19601f8201168201806040525081019061011a919061013e565b6001600160a01b031660c05250600080546001600160a01b0319163317905561016e565b60006020828403121561015057600080fd5b81516001600160a01b038116811461016757600080fd5b9392505050565b60805160a05160c0516107fa6101c06000396000818161038b01526104db0152600081816102e301526104040152600081816101160152818161023e01528181610433015261050a01526107fa6000f3fe608060405234801561001057600080fd5b506004361061004c5760003560e01c8063828cc5ce146100515780638da5cb5b14610066578063b29a814014610095578063e9cbafb0146100a8575b600080fd5b61006461005f3660046105b1565b6100bb565b005b600054610079906001600160a01b031681565b6040516001600160a01b03909116815260200160405180910390f35b6100646100a33660046105ef565b610188565b6100646100b6366004610619565b610233565b60408051606080820183528482526020808301858152339385019384528451918201879052518185015291516001600160a01b03908116838301528351808403909201825260808301938490526312439b2f60e21b909352917f0000000000000000000000000000000000000000000000000000000000000000169063490e6cbc90610151903090879087908790608401610699565b600060405180830381600087803b15801561016b57600080fd5b505af115801561017f573d6000803e3d6000fd5b50505050505050565b6000546001600160a01b031633146101d35760405162461bcd60e51b81526020600482015260096024820152682727aa2fa7aba722a960b91b60448201526064015b60405180910390fd5b811580156101e8576001811461021b57505050565b60405163a9059cbb60e01b81523360048201528260248201526000806044836000885af161021557600080fd5b50505050565b60008060008085335af161022e57600080fd5b505050565b336001600160a01b037f0000000000000000000000000000000000000000000000000000000000000000161461029c5760405162461bcd60e51b815260206004820152600e60248201526d1b9bdd08185d5d1a1bdc9a5e995960921b60448201526064016101ca565b60006102aa82840184610706565b905084156103545760408181015190516323b872dd60e01b81526001600160a01b039182166004820152306024820152604481018790527f0000000000000000000000000000000000000000000000000000000000000000909116906323b872dd906064016020604051808303816000875af115801561032e573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906103529190610774565b505b83156103fc5760408181015190516323b872dd60e01b81526001600160a01b039182166004820152306024820152604481018690527f0000000000000000000000000000000000000000000000000000000000000000909116906323b872dd906064016020604051808303816000875af11580156103d6573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906103fa9190610774565b505b84156104d3577f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663a9059cbb7f0000000000000000000000000000000000000000000000000000000000000000878460000151610462919061079d565b6040516001600160e01b031960e085901b1681526001600160a01b03909216600483015260248201526044016020604051808303816000875af11580156104ad573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906104d19190610774565b505b83156105aa577f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031663a9059cbb7f0000000000000000000000000000000000000000000000000000000000000000868460200151610539919061079d565b6040516001600160e01b031960e085901b1681526001600160a01b03909216600483015260248201526044016020604051808303816000875af1158015610584573d6000803e3d6000fd5b505050506040513d601f19601f820116820180604052508101906105a89190610774565b505b5050505050565b600080604083850312156105c457600080fd5b50508035926020909101359150565b80356001600160a01b03811681146105ea57600080fd5b919050565b6000806040838503121561060257600080fd5b61060b836105d3565b946020939093013593505050565b6000806000806060858703121561062f57600080fd5b8435935060208501359250604085013567ffffffffffffffff8082111561065557600080fd5b818701915087601f83011261066957600080fd5b81358181111561067857600080fd5b88602082850101111561068a57600080fd5b95989497505060200194505050565b60018060a01b03851681526000602085602084015284604084015260806060840152835180608085015260005b818110156106e25785810183015185820160a0015282016106c6565b50600060a0828601015260a0601f19601f8301168501019250505095945050505050565b60006060828403121561071857600080fd5b6040516060810181811067ffffffffffffffff8211171561074957634e487b7160e01b600052604160045260246000fd5b80604052508235815260208301356020820152610768604084016105d3565b60408201529392505050565b60006020828403121561078657600080fd5b8151801515811461079657600080fd5b9392505050565b808201808211156107be57634e487b7160e01b600052601160045260246000fd5b9291505056fea26469706673582212201f11bf35a265fe6cf7a7a681e59372eebee6756b5c0d129c7a844608b62a108564736f6c63430008180033".parse().unwrap();
    return Bytecode::new_raw(bytes);
}

pub fn json_abi() -> JsonAbi {
    let abi: JsonAbi = serde_json::from_str(
        r#"
        [
        {
           "type":"constructor",
           "inputs":[
              {
                 "name":"_pool",
                 "type":"address",
                 "internalType":"address"
              }
           ],
           "stateMutability":"nonpayable"
        },
        {
           "type":"function",
           "name":"flash",
           "inputs":[
              {
                 "name":"amount0",
                 "type":"uint256",
                 "internalType":"uint256"
              },
              {
                 "name":"amount1",
                 "type":"uint256",
                 "internalType":"uint256"
              }
           ],
           "outputs":[

           ],
           "stateMutability":"nonpayable"
        },
        {
           "type":"function",
           "name":"owner",
           "inputs":[

           ],
           "outputs":[
              {
                 "name":"",
                 "type":"address",
                 "internalType":"address"
              }
           ],
           "stateMutability":"view"
        },
        {
           "type":"function",
           "name":"recoverToken",
           "inputs":[
              {
                 "name":"token",
                 "type":"address",
                 "internalType":"address"
              },
              {
                 "name":"amount",
                 "type":"uint256",
                 "internalType":"uint256"
              }
           ],
           "outputs":[

           ],
           "stateMutability":"nonpayable"
        },
        {
           "type":"function",
           "name":"uniswapV3FlashCallback",
           "inputs":[
              {
                 "name":"fee0",
                 "type":"uint256",
                 "internalType":"uint256"
              },
              {
                 "name":"fee1",
                 "type":"uint256",
                 "internalType":"uint256"
              },
              {
                 "name":"data",
                 "type":"bytes",
                 "internalType":"bytes"
              }
           ],
           "outputs":[

           ],
           "stateMutability":"nonpayable"
        }
        ]
        "#,
    )
    .unwrap();
    abi
}
