use alloy_sol_types::SolCall;
use crate::common::revm::{EvmSimulator, Tx};
use alloy::contract::{ContractInstance, Interface};
use alloy::dyn_abi::DynSolValue;
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::U64;
use alloy_json_abi::JsonAbi;
use alloy_primitives::U16;
use anyhow::Result;
use revm::db::InMemoryDB;
use revm::primitives::{address, AccountInfo, Address, Bytecode, FixedBytes, U256};
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;

pub async fn simulation() -> Result<()> {
    //      - Simulation:
    //       - Simply we are going to, get required info (latest block, pool needed?, adjacent pool?)
    //       - deploy our contract
    //       - Create a transaction to send to our contract
    //       - Execute the transaction
    //       - Log the results
    //       - check if eth balance has increased

    // initialise cache and addresses

    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider: RootProvider<PubSubFrontend, AnyNetwork> = ProviderBuilder::new().network().on_ws(ws_client).await?;
    let provider = Arc::new(provider);

    // Fetch the latest block number
    let latest_block_number = provider.get_block_number().await?;
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full )
        .await?
        .expect("Expected block");

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    let wallet = PrivateKeySigner::random();
    let wallet_address = wallet.address();

    let contract_info = AccountInfo::new(U256::ZERO, 0, FixedBytes::ZERO, arboo_bytecode());
    let mut simulator = EvmSimulator::new(provider.clone(), Some(wallet_address),U64::from(latest_block_number));
    simulator.deploy(wallet_address, contract_info.code.unwrap());

    // println!("weth account info: {:?}", simulator.get_storage(weth_addr()));
    // println!("pool 500 account info: {:?}", simulator.get_storage(pool_500_addr()));
    // println!("pool 3000 account info: {:?}", simulator.get_storage(pool_3000_addr()));
    // for i in 0..10 {
    //     let storage = provider.get_storage_at(usdc_addr().into(), U256::from(i)).await?;
    //     println!("Storage of the test swap: {:?}", storage);
    //     simulator.insert_account_storage(usdc_addr(), U256::from(i), storage);
    // }


    let simple_swap_wallet = PrivateKeySigner::random();
    let simple_swap_wallet_address = simple_swap_wallet.address();
    simulator.deploy(simple_swap_wallet_address, simple_bytecode());

    // set initial eth value;
    let alloy_wallet_address = Address::from_str(wallet.address().to_string().as_str()).unwrap();
    let initial_eth_balance = U256::from(100) * U256::from(10).pow(U256::from(18));
    simulator.set_eth_balance(alloy_wallet_address, initial_eth_balance);

    let fifty_eth = U256::from(50) * U256::from(10).pow(U256::from(18));
    let five_eth = U256::from(5) * U256::from(10).pow(U256::from(18));
    let four_eth = U256::from(4) * U256::from(10).pow(U256::from(18));
    // swap eth to weth
    alloy::sol!{
        function swapEthForWeth(
            address to,
            uint256 deadline
        ) external payable;
    }

    let function_call = swapEthForWethCall {
        to: wallet_address,
        deadline: U256::from(9999999999_u64),
    };

    let function_call_data = function_call.abi_encode();

    // NOTE: to test the simulation I am swapping here to depress the price on the v2 pool
    simulation_swap(&mut simulator,  &latest_gas_price, &latest_gas_limit, &provider).await?;

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: weth_addr(),
        data: function_call_data.into(),
        value: five_eth,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    let result = simulator.call(new_tx)?;

    println!("result from swapping, {:?}", result);

    let fee1 = alloy_primitives::aliases::U24::from(500);

    alloy::sol! {
        function flashSwap_V3_to_V2(
            address pool0,
            uint24 fee1,
            address tokenIn,
            address tokenOut,
            uint256 amountIn,
        ) external;
    };

    let function_call = flashSwap_V3_to_V2Call {
        pool0: pool_3000_addr(),
        fee1: fee1,
        tokenIn: weth_addr(),
        tokenOut: usdc_addr(),
        amountIn: four_eth,
    };
    
    let function_call_data = function_call.abi_encode();

    // Log initial balances
    let initial_balance_target = simulator.get_eth_balance(wallet_address);

    println!(
        "Initial eth balance of Owner: {:?}",
        initial_balance_target
    );

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: simulator.owner,
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    let result = simulator.call(new_tx)?;

    // Log gas used
    println!("Gas Used: {:?}", result.gas_used);

    // withdraw_weth(latest_gas_limit, latest_gas_price, wallet_address, &mut simulator, four_eth)?;

   // check balance of weth
    let eth_balance = simulator.get_eth_balance(wallet_address);
    println!("eth balance after swap: {:?}", eth_balance); 

    // simulator.get_accounts();

    // simulator.get_db();

    Ok(())
}

fn check_weth_balance(wallet_address: Address, simulator: &mut EvmSimulator<'_>, latest_gas_limit: &u64, latest_gas_price: &U256) -> Result<(), anyhow::Error> {

    alloy::sol!{
        function balanceOf(address account) external view returns (uint256);
    }

    let function_call = balanceOfCall {
        account: wallet_address,
    };

    let function_call_data = function_call.abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: weth_addr(),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit.clone() as u64,
        gas_price: *latest_gas_price,
    };

    let result = simulator.call(new_tx)?;

    println!("WETH balance: {:?}", result);
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
//Uniswap SwapRouter02:
pub fn v3_router_addr()-> Address {
    address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
}

pub fn v2_router_addr()-> Address {
    address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
}

pub fn random_addr()-> Address {
    address!("60d023f1b06edcEAcC4799a73865D1baaBBd355f")
}
pub fn arboo_bytecode() -> Bytecode {
    let bytes = "0x608060405234801561000f575f80fd5b5061121f8061001d5f395ff3fe608060405234801561000f575f80fd5b506004361061004a575f3560e01c806310d1e85c1461004e5780637bd0416514610063578063f0cc68c514610076578063fa461e3314610089575b5f80fd5b61006161005c366004610bce565b61009c565b005b610061610071366004610c4a565b610240565b610061610084366004610c4a565b610338565b610061610097366004610ca8565b610531565b6001600160a01b03851630146100e65760405162461bcd60e51b815260206004820152600a6024820152693737ba1039b2b73232b960b11b60448201526064015b60405180910390fd5b5f808080806100f786880188610cf7565b945094509450945094505f61010f85878685876106fa565b9050855f6103e56101218c6003610d60565b61012b9190610d7d565b610136906001610d9c565b90505f610143828d610d9c565b6040516323b872dd60e01b81526001600160a01b038a8116600483015230602483015260448201839052919250908416906323b872dd906064016020604051808303815f875af1158015610199573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101bd9190610dbc565b5060405163a9059cbb60e01b81526001600160a01b038f811660048301526024820186905284169063a9059cbb906044015b6020604051808303815f875af115801561020b573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061022f9190610dbc565b505050505050505050505050505050565b6001600160a01b03808316908416105f8161027957610274600173fffd8963efd1fc6a506488495d951d5263988d26610dde565b610289565b6102896401000276a36001610e05565b90505f338888888888886040516020016102a99796959493929190610e25565b60408051601f1981840301815290829052630251596160e31b825291506001600160a01b0389169063128acb08906102ed9030908790899088908890600401610eb0565b60408051808303815f875af1158015610308573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061032c9190610eea565b50505050505050505050565b6001600160a01b03808316908416105f816103715761036c600173fffd8963efd1fc6a506488495d951d5263988d26610dde565b610381565b6103816401000276a36001610e05565b90505f338888888888886040516020016103a19796959493929190610e25565b60408051601f1981840301815260028084526060808501845291945090929160208301908036833701905050905085815f815181106103e2576103e2610f20565b60200260200101906001600160a01b031690816001600160a01b031681525050868160018151811061041657610416610f20565b6001600160a01b039092166020928302919091019091015260405163d06ca61f60e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d9063d06ca61f906104699089908690600401610f77565b5f60405180830381865afa158015610483573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f191682016040526104aa9190810190610f97565b9050896001600160a01b031663022c0d9f87835f815181106104ce576104ce610f20565b602002602001015130876040518563ffffffff1660e01b81526004016104f79493929190611050565b5f604051808303815f87803b15801561050e575f80fd5b505af1158015610520573d5f803e3d5ffd5b5050505050505050505b5050505050565b5f808080808080610544888a018a611086565b9650965096509650965096509650856001600160a01b0316336001600160a01b0316146105a05760405162461bcd60e51b815260206004820152600a6024820152693737ba1039b2b73232b960b11b60448201526064016100dd565b5f816105b4576105af8c61110c565b6105bd565b6105bd8b61110c565b90505f6105cc85878487610842565b90505f6105d98583611126565b90505f81116106175760405162461bcd60e51b815260206004820152600a602482015269070726f666974203d20360b41b60448201526064016100dd565b60405163a9059cbb60e01b81526001600160a01b038a811660048301526024820187905288169063a9059cbb906044016020604051808303815f875af1158015610663573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906106879190610dbc565b506001600160a01b03871673c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2146106bc576106b7878b836109f2565b610520565b60405163a9059cbb60e01b81523060048201526024810182905273c02aaa39b223fe8d0a0e5c4f27ead9083c756cc29063a9059cbb906044016101ef565b60405163095ea7b360e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc456004820152602481018390525f906001600160a01b0387169063095ea7b3906044016020604051808303815f875af115801561075a573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061077e9190610dbc565b506040805160e0810182526001600160a01b0380891682528716602082015262ffffff8616818301523060608201526080810185905260a081018490525f60c082015290516304e45aaf60e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc45906304e45aaf906107f7908490600401611139565b6020604051808303815f875af1158015610813573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906108379190611197565b979650505050505050565b60405163095ea7b360e01b8152737a250d5630b4cf539739df2c5dacb4c659f2488d6004820152602481018390525f906001600160a01b0386169063095ea7b3906044016020604051808303815f875af11580156108a2573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906108c69190610dbc565b50604080516002808252606080830184529260208301908036833701905050905085815f815181106108fa576108fa610f20565b60200260200101906001600160a01b031690816001600160a01b031681525050848160018151811061092e5761092e610f20565b6001600160a01b03909216602092830291909101909101526040516338ed173960e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d906338ed17399061098790889088908790309042906004016111ae565b5f604051808303815f875af11580156109a2573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f191682016040526109c99190810190610f97565b9050806001815181106109de576109de610f20565b602002602001015192505050949350505050565b60405163095ea7b360e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc456004820152602481018290526001600160a01b0384169063095ea7b3906044016020604051808303815f875af1158015610a50573d5f803e3d5ffd5b505050506040513d601f19601f82011682018060405250810190610a749190610dbc565b505f6040518060e00160405280856001600160a01b0316815260200173c02aaa39b223fe8d0a0e5c4f27ead9083c756cc26001600160a01b031681526020016101f462ffffff168152602001846001600160a01b031681526020018381526020015f81526020016401000276a36001610aed9190610e05565b6001600160a01b031690526040516304e45aaf60e01b81529091507368b3465833fb72a70ecdf485e0e4c7bd8665fc45906304e45aaf90610b32908490600401611139565b6020604051808303815f875af1158015610b4e573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061052a9190611197565b6001600160a01b0381168114610b86575f80fd5b50565b5f8083601f840112610b99575f80fd5b50813567ffffffffffffffff811115610bb0575f80fd5b602083019150836020828501011115610bc7575f80fd5b9250929050565b5f805f805f60808688031215610be2575f80fd5b8535610bed81610b72565b94506020860135935060408601359250606086013567ffffffffffffffff811115610c16575f80fd5b610c2288828901610b89565b969995985093965092949392505050565b803562ffffff81168114610c45575f80fd5b919050565b5f805f805f60a08688031215610c5e575f80fd5b8535610c6981610b72565b9450610c7760208701610c33565b93506040860135610c8781610b72565b92506060860135610c9781610b72565b949793965091946080013592915050565b5f805f8060608587031215610cbb575f80fd5b8435935060208501359250604085013567ffffffffffffffff811115610cdf575f80fd5b610ceb87828801610b89565b95989497509550505050565b5f805f805f60a08688031215610d0b575f80fd5b8535610d1681610b72565b94506020860135610d2681610b72565b9350610d3460408701610c33565b94979396509394606081013594506080013592915050565b634e487b7160e01b5f52601160045260245ffd5b8082028115828204841417610d7757610d77610d4c565b92915050565b5f82610d9757634e487b7160e01b5f52601260045260245ffd5b500490565b80820180821115610d7757610d77610d4c565b8015158114610b86575f80fd5b5f60208284031215610dcc575f80fd5b8151610dd781610daf565b9392505050565b6001600160a01b03828116828216039080821115610dfe57610dfe610d4c565b5092915050565b6001600160a01b03818116838216019080821115610dfe57610dfe610d4c565b6001600160a01b039788168152958716602087015262ffffff9490941660408601529185166060850152909316608083015260a082019290925290151560c082015260e00190565b5f81518084525f5b81811015610e9157602081850181015186830182015201610e75565b505f602082860101526020601f19601f83011685010191505092915050565b6001600160a01b0386811682528515156020830152604082018590528316606082015260a0608082018190525f9061083790830184610e6d565b5f8060408385031215610efb575f80fd5b505080516020909101519092909150565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f52603260045260245ffd5b5f815180845260208085019450602084015f5b83811015610f6c5781516001600160a01b031687529582019590820190600101610f47565b509495945050505050565b828152604060208201525f610f8f6040830184610f34565b949350505050565b5f6020808385031215610fa8575f80fd5b825167ffffffffffffffff80821115610fbf575f80fd5b818501915085601f830112610fd2575f80fd5b815181811115610fe457610fe4610f0c565b8060051b604051601f19603f8301168101818110858211171561100957611009610f0c565b604052918252848201925083810185019188831115611026575f80fd5b938501935b828510156110445784518452938501939285019261102b565b98975050505050505050565b84815283602082015260018060a01b0383166040820152608060608201525f61107c6080830184610e6d565b9695505050505050565b5f805f805f805f60e0888a03121561109c575f80fd5b87356110a781610b72565b965060208801356110b781610b72565b95506110c560408901610c33565b945060608801356110d581610b72565b935060808801356110e581610b72565b925060a0880135915060c08801356110fc81610daf565b8091505092959891949750929550565b5f600160ff1b820161112057611120610d4c565b505f0390565b81810381811115610d7757610d77610d4c565b81516001600160a01b03908116825260208084015182169083015260408084015162ffffff16908301526060808401518216908301526080808401519083015260a0838101519083015260c092830151169181019190915260e00190565b5f602082840312156111a7575f80fd5b5051919050565b85815284602082015260a060408201525f6111cc60a0830186610f34565b6001600160a01b039490941660608301525060800152939250505056fea2646970667358221220aa242453086aefc424a3697bd0dedd3fe36bce88c6e213608e8ecc8b9255ec4464736f6c63430008180033".parse().unwrap();
    return Bytecode::new_raw(bytes);
}

pub fn simple_bytecode() -> Bytecode {
    Bytecode::new_raw("0x608060405234801561000f575f80fd5b5060405161028138038061028183398101604081905261002e91610052565b5f80546001600160a01b0319166001600160a01b039290921691909117905561007f565b5f60208284031215610062575f80fd5b81516001600160a01b0381168114610078575f80fd5b9392505050565b6101f58061008c5f395ff3fe608060405260043610610028575f3560e01c80633fc8cef31461002c578063bc1cbce814610066575b5f80fd5b348015610037575f80fd5b505f5461004a906001600160a01b031681565b6040516001600160a01b03909116815260200160405180910390f35b61006e610070565b005b5f34116100c35760405162461bcd60e51b815260206004820152601e60248201527f4d7573742073656e642045544820746f207377617020666f7220574554480000604482015260640160405180910390fd5b5f8054906101000a90046001600160a01b03166001600160a01b031663d0e30db0346040518263ffffffff1660e01b81526004015f604051808303818588803b15801561010e575f80fd5b505af1158015610120573d5f803e3d5ffd5b50505f5460405163a9059cbb60e01b81523360048201523460248201526001600160a01b03909116935063a9059cbb925060440190506020604051808303815f875af1158015610172573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101969190610199565b50565b5f602082840312156101a9575f80fd5b815180151581146101b8575f80fd5b939250505056fea2646970667358221220f0f0607cc27cce1c340b07295c24aac37d16af628068c79e3774273bc80e621f64736f6c63430008180033".parse().unwrap())
}

    //          - Simply we are going to simulate:
    //          - borrowing from the lower priced pool
    //          - selling to higher priced exchange for weth
    //          - using weth gained, by required amount on lower priced exchange
    //          - pay back loan with fee
    //          - revenue will be weth gained - weth used to repay loan


    // Create a function that takes in the Evm Simulator arc mutex and then does a massive swap transaction so that we have a transaction to simulate with

    async fn simulation_swap<'a>(evm_simulator: &mut EvmSimulator<'a>,  latest_gas_price: &U256, latest_gas_limit: &u64, provider: &Arc<RootProvider<PubSubFrontend, AnyNetwork>>) -> Result<()>
{
    // let abi = serde_json::from_str(include_str!("v2_3000.json")).unwrap();

    // let contract = ContractInstance::<Address, Arc<RootProvider<PubSubFrontend, AnyNetwork>>, Interface>::new(
    //     evm_simulator.owner,
    //     provider.clone(),
    //     Interface::new(abi),
    // );


    // let amount_out_min: DynSolValue = U256::from(1).into();
    // let path: DynSolValue = DynSolValue::Array(vec![usdc_addr().into(), weth_addr().into()]);
    // let to: DynSolValue = wallet_address.into();
    // let deadline: DynSolValue = U256::from(9999999999_u64).into();

    // let swap_params = vec![
    //     amount_out_min,
    //     path,
    //     to,
    //     deadline,
    // ];

    // let swap_params = contract.encode_input("swapExactETHForTokens", &swap_params)?;

    let wallet = PrivateKeySigner::random();

    let wallet_address = wallet.address();

    // Initial balance:
    // put the balance into the wallet
    let amount_in = U256::from(100_000) * U256::from(10).pow(U256::from(18));
    evm_simulator.set_eth_balance(wallet_address, amount_in);

    // Log initial balances
    let initial_balance_target = evm_simulator.get_eth_balance(wallet_address);
    println!(
        "Initial Balance of the test swap: {:?}",
        initial_balance_target
    );

    
    let fifty_eth = U256::from(50_000) * U256::from(10).pow(U256::from(18));

    alloy::sol!{
        function swapEthForWeth(
            address to,
            uint256 deadline
        ) external payable;
    }

    let function_call = swapEthForWethCall {
        to: wallet_address,
        deadline: U256::from(9999999999_u64),
    };

    let function_call_data = function_call.abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: weth_addr(),
        data: function_call_data.into(),
        value: fifty_eth,
        gas_limit: *latest_gas_limit as u64,
        gas_price: *latest_gas_price,
    };

    let result = evm_simulator.call(new_tx)?;

    println!("result from swapping, {:?}", result);

    // create sol! for swapping weth for usdc

    alloy::sol! {
        function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
        external
        payable
        returns (uint[] memory amounts);
    }

    let swap_params = swapExactETHForTokensCall {
        amountOutMin: U256::from(1),
        path: vec!(weth_addr(),usdc_addr()),
        to: wallet_address,
        deadline: U256::from(9999999999_u64),
    };

    let swap_params = swap_params.abi_encode();

    // Execute the transaction
    let new_tx = Tx {
        caller: wallet_address,
        transact_to: v2_router_addr(),
        data: swap_params.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    evm_simulator.call(new_tx)?;

    // Log final balances
    let final_balance_target = evm_simulator.get_eth_balance(wallet_address);
    
    println!("Eth Balance of the fake swaap: {:?}", final_balance_target);
    
    Ok(())


    }

fn withdraw_weth(latest_gas_limit: u64, latest_gas_price: alloy_primitives::Uint<256, 4>, wallet_address: Address, simulator: &mut EvmSimulator<'_>, four_eth: alloy_primitives::Uint<256, 4>) -> Result<(), anyhow::Error> {

    alloy::sol!{
        function balanceOf(address account) external view returns (uint256);
        function deposit(uint256 amount) external payable;
        function withdraw(uint256 amount) external; 
    }

    let function_call = withdrawCall{
        amount: four_eth,
    };

    let function_call_data = function_call.abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: weth_addr(),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    simulator.call(new_tx)?;
    Ok(())
    }