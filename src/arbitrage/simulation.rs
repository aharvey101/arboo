use crate::common::revm::{EvmSimulator, Tx};
use alloy::consensus::TxEnvelope;
use alloy::dyn_abi::{DynSolType, DynSolValue};
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::primitives::U64;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::{Bytes, B256};
use alloy_sol_types::SolCall;
use anyhow::Result;
use revm::primitives::{address, AccountInfo, Address, Bytecode, FixedBytes, U256};
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;

pub async fn simulation(pool_a: Address, pool_b: Address) -> Result<()> {
    //      - Simulation:
    //       - Simply we are going to, get required info (latest block, pool needed?, adjacent pool?)
    //       - deploy our contract
    //       - Create a transaction to send to our contract
    //       - Execute the transaction
    //       - Log the results
    //       - check if eth balance has increased

    let ws_client = WsConnect::new(std::env::var("WS_URL").expect("no ws url"));
    let provider: RootProvider<PubSubFrontend, AnyNetwork> =
        ProviderBuilder::new().network().on_ws(ws_client).await?;
    let provider = Arc::new(provider);

    let latest_block_number = provider.get_block_number().await?;
    let block_id = BlockId::from_str(latest_block_number.to_string().as_str()).unwrap();
    let latest_block = provider
        .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
        .await?
        .expect("Expected block");

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));

    let contract_wallet = PrivateKeySigner::random();
    let contract_wallet_address = contract_wallet.address();

    let mut simulator = EvmSimulator::new(
        provider.clone(),
        Some(contract_wallet_address),
        U64::from(latest_block_number),
    );

    let my_wallet = PrivateKeySigner::random();
    // set initial eth value;
    let initial_eth_balance = U256::from(1000) * U256::from(10).pow(U256::from(18));
    simulator.set_eth_balance(my_wallet.address(), initial_eth_balance);

    simulator.deploy(arboo_bytecode());

    let hundred_thousand_usdc = U256::from(100_000) * U256::from(10).pow(U256::from(6));
    let trillion_pepe = U256::from(100_000_000_000_000u64) * U256::from(10).pow(U256::from(18));
    // let hundred_thousand_usdc = U256::from(100_000_000) * U256::from(10).pow(U256::from(6));

    // NOTE: to test the simulation I am swapping here to depress the price on the v2 pool
    // simulation_swap(
    //     &mut simulator,
    //     &latest_gas_price,
    //     &latest_gas_limit,
    //     &provider,
    // )
    // .await?;

    // test our hello world function

    let fee1 = alloy_primitives::aliases::U24::from(500);

    // Note: Setup the swap abi
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

    // Note: create the params
    let function_call = flashSwap_V3_to_V2Call {
        pool0: pool_500_addr(),
        fee1: fee1,
        tokenIn: pool_a,
        tokenOut: pool_b,
        amountIn: trillion_pepe,
    };

    let function_call_data = function_call.abi_encode();

    // // Check weth balance just to see if it's all good
    // Note: create the transaction
    let new_tx = Tx {
        caller: my_wallet.address(),
        transact_to: contract_wallet_address,
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    // Call said transaction
    let result = simulator.call(new_tx)?;


    check_weth_balance(
        my_wallet.address(),
        &mut simulator,
        &latest_gas_limit,
        &latest_gas_price,
        None,
    )?;


    // Log gas used
    println!("Gas Used: {:?}", result.gas_used);

    Ok(())
}

fn hello_world(
    latest_gas_limit: u64,
    latest_gas_price: alloy_primitives::Uint<256, 4>,
    contract_wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    my_wallet_address: &Address,
) -> Result<(), anyhow::Error> {
    alloy::sol! {
        #[derive(Debug)]
        function hello_world() public pure returns(string memory);
    }
    let params = hello_worldCall {}.abi_encode();
    let tx_hello_world = Tx {
        caller: *my_wallet_address,
        transact_to: contract_wallet_address,
        data: params.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };
    let res = simulator.call(tx_hello_world)?;
    println!("res: {:?}", res);
    println!(
        "Res from hello world, {:?}",
        hello_worldCall::abi_decode_returns(&res.output, false)?
    );
    Ok(())
}

async fn simulation_swap<'a>(
    evm_simulator: &mut EvmSimulator<'a>,
    latest_gas_price: &U256,
    latest_gas_limit: &u64,
    provider: &Arc<RootProvider<PubSubFrontend, AnyNetwork>>,
) -> Result<()> {
    let wallet = PrivateKeySigner::random();

    let wallet_address = wallet.address();

    let amount_in = U256::from(600_000) * U256::from(10).pow(U256::from(18));
    evm_simulator.set_eth_balance(wallet_address, amount_in);

    alloy::sol! {
        function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
        external
        payable
        returns (uint[] memory amounts);
    }

    alloy::sol! {
        function exactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            address recipient,
            uint256 amountIn,
            uint256 amountOutMinimum,
            uint160 sqrtPriceLimitX96
        ) external payable returns (uint256 amountOut);
    }

    let exact_input_params = exactInputSingleCall {
        tokenIn: weth_addr(),
        tokenOut: pepe_addr(),
        fee: alloy_primitives::aliases::U24::from(3000),
        recipient: wallet_address,
        amountIn: one_hundred_ether(),
        amountOutMinimum: U256::from(1),
        sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
    };

    let exact_input_data = exact_input_params.abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: v2_router_addr(),
        data: exact_input_data.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    evm_simulator.call(new_tx)?;

    // let swap_params = swapExactETHForTokensCall {
    //     amountOutMin: U256::from(1),
    //     path: vec![weth_addr(), usdc_addr()],
    //     to: wallet_address,
    //     deadline: U256::from(9999999999_u64),
    // };

    // let swap_params = swap_params.abi_encode();

    // // Execute the transaction
    // let new_tx = Tx {
    //     caller: wallet_address,
    //     transact_to: v2_router_addr(),
    //     data: swap_params.into(),
    //     value: one_thousand_eth(),
    //     gas_limit: *latest_gas_limit,
    //     gas_price: *latest_gas_price,
    // };

    // evm_simulator.call(new_tx)?;

    Ok(())
}

pub fn one_ether() -> U256 {
    "1000000000000000000".parse().unwrap()
}

pub fn one_hundred_ether() -> U256 {
    "100000000000000000000".parse().unwrap()
}

pub fn fify_thousand_eth() -> U256 {
    "50000000000000000000000".parse().unwrap()
}

pub fn five_hundred_eth() -> U256 {
    "500000000000000000000".parse().unwrap()
}

pub fn one_thousand_eth() -> U256 {
    "1000000000000000000000".parse().unwrap()
}

pub fn five_hundred_thousand_eth() -> U256 {
    "50000000000000000000000".parse().unwrap()
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

pub fn pool_3000_pepe_addr() -> Address {
    address!("11950d141EcB863F01007AdD7D1A342041227b58")
}

pub fn pepe_addr() -> Address {
    address!("6982508145454Ce325dDbE47a25d4ec3d2311933")
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
pub fn v3_router_addr() -> Address {
    address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45")
}

pub fn v2_router_addr() -> Address {
    address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
}

pub fn random_addr() -> Address {
    address!("60d023f1b06edcEAcC4799a73865D1baaBBd355f")
}
pub fn arboo_bytecode() -> Bytecode {
    let bytes = hex::decode("608060405234801561000f575f80fd5b506004361061003f575f3560e01c80637bd04165146100435780639476f92214610058578063fa461e331461008e575b5f80fd5b6100566100513660046109ab565b6100a1565b005b604080518082018252600b81526a12195b1b1bc815dbdc9b1960aa1b602082015290516100859190610a56565b60405180910390f35b61005661009c366004610a6f565b6101c8565b6001600160a01b03808316908416105f816100da576100d5600173fffd8963efd1fc6a506488495d951d5263988d26610afc565b6100ea565b6100ea6401000276a36001610b23565b604080513360208201526001600160a01b03808b169282019290925262ffffff89166060820152818816608082015290861660a082015260c0810185905283151560e08201529091505f906101000160408051601f1981840301815290829052630251596160e31b825291506001600160a01b0389169063128acb089061017d9030908790899088908890600401610b43565b60408051808303815f875af1158015610198573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101bc9190610b88565b50505050505050505050565b5f8080808080806101db888a018a610bb7565b9650965096509650965096509650856001600160a01b0316336001600160a01b03161461023c5760405162461bcd60e51b815260206004820152600a6024820152693737ba1039b2b73232b960b11b60448201526064015b60405180910390fd5b5f816102505761024b8c610c3d565b610259565b6102598b610c3d565b90505f6102698587846001610484565b9050838110156102bc5761027c816106a8565b610285856106a8565b604051602001610296929190610c57565b60408051601f198184030181529082905262461bcd60e51b825261023391600401610a56565b83811161030b5760405162461bcd60e51b815260206004820152601860248201527f6275794261636b416d6f756e74203c20616d6f756e74496e00000000000000006044820152606401610233565b5f6103168583610cb2565b90505f81116103545760405162461bcd60e51b815260206004820152600a602482015269070726f666974203d20360b41b6044820152606401610233565b60405163a9059cbb60e01b81526001600160a01b038a811660048301526024820187905288169063a9059cbb906044016020604051808303815f875af11580156103a0573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906103c49190610ccb565b506001600160a01b03871673c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2146103f9576103f4878b836107ad565b610474565b60405163a9059cbb60e01b81523060048201526024810182905273c02aaa39b223fe8d0a0e5c4f27ead9083c756cc29063a9059cbb906044016020604051808303815f875af115801561044e573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906104729190610ccb565b505b5050505050505050505050505050565b60405163095ea7b360e01b8152737a250d5630b4cf539739df2c5dacb4c659f2488d6004820152602481018390525f906001600160a01b0386169063095ea7b3906044016020604051808303815f875af11580156104e4573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906105089190610ccb565b50604080516002808252606080830184529260208301908036833701905050905085815f8151811061053c5761053c610cfa565b60200260200101906001600160a01b031690816001600160a01b031681525050848160018151811061057057610570610cfa565b6001600160a01b03909216602092830291909101909101526040516338ed173960e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d906338ed1739906105c99088908890879030904290600401610d0e565b5f604051808303815f875af11580156105e4573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f1916820160405261060b9190810190610d7f565b90505f8160018151811061062157610621610cfa565b602002602001015110156106815760405162461bcd60e51b815260206004820152602160248201527f616d6f756e74312061667465722076325f73776170206c657373207468616e206044820152600360fc1b6064820152608401610233565b8060018151811061069457610694610cfa565b602002602001015192505050949350505050565b6060815f036106ce5750506040805180820190915260018152600360fc1b602082015290565b815f5b81156106f757806106e181610e38565b91506106f09050600a83610e64565b91506106d1565b5f8167ffffffffffffffff81111561071157610711610ce6565b6040519080825280601f01601f19166020018201604052801561073b576020820181803683370190505b5090505b84156107a557610750600183610cb2565b915061075d600a86610e77565b610768906030610e8a565b60f81b81838151811061077d5761077d610cfa565b60200101906001600160f81b03191690815f1a90535061079e600a86610e64565b945061073f565b949350505050565b60405163095ea7b360e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc456004820152602481018290526001600160a01b0384169063095ea7b3906044016020604051808303815f875af115801561080b573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061082f9190610ccb565b505f6040518060e00160405280856001600160a01b0316815260200173c02aaa39b223fe8d0a0e5c4f27ead9083c756cc26001600160a01b031681526020016101f462ffffff168152602001846001600160a01b031681526020018381526020015f81526020016401000276a360016108a89190610b23565b6001600160a01b03908116909152604080516304e45aaf60e01b81528351831660048201526020840151831660248201529083015162ffffff1660448201526060830151821660648201526080830151608482015260a083015160a482015260c083015190911660c48201529091507368b3465833fb72a70ecdf485e0e4c7bd8665fc45906304e45aaf9060e4016020604051808303815f875af1158015610952573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906109769190610e9d565b5050505050565b6001600160a01b0381168114610991575f80fd5b50565b803562ffffff811681146109a6575f80fd5b919050565b5f805f805f60a086880312156109bf575f80fd5b85356109ca8161097d565b94506109d860208701610994565b935060408601356109e88161097d565b925060608601356109f88161097d565b949793965091946080013592915050565b5f5b83811015610a23578181015183820152602001610a0b565b50505f910152565b5f8151808452610a42816020860160208601610a09565b601f01601f19169290920160200192915050565b602081525f610a686020830184610a2b565b9392505050565b5f805f8060608587031215610a82575f80fd5b8435935060208501359250604085013567ffffffffffffffff80821115610aa7575f80fd5b818701915087601f830112610aba575f80fd5b813581811115610ac8575f80fd5b886020828501011115610ad9575f80fd5b95989497505060200194505050565b634e487b7160e01b5f52601160045260245ffd5b6001600160a01b03828116828216039080821115610b1c57610b1c610ae8565b5092915050565b6001600160a01b03818116838216019080821115610b1c57610b1c610ae8565b6001600160a01b0386811682528515156020830152604082018590528316606082015260a0608082018190525f90610b7d90830184610a2b565b979650505050505050565b5f8060408385031215610b99575f80fd5b505080516020909101519092909150565b8015158114610991575f80fd5b5f805f805f805f60e0888a031215610bcd575f80fd5b8735610bd88161097d565b96506020880135610be88161097d565b9550610bf660408901610994565b94506060880135610c068161097d565b93506080880135610c168161097d565b925060a0880135915060c0880135610c2d81610baa565b8091505092959891949750929550565b5f600160ff1b8201610c5157610c51610ae8565b505f0390565b7002bb7bab632103ab73232b9333637bb9d1607d1b81525f8351610c82816011850160208801610a09565b630103b39960e51b6011918401918201528351610ca6816015840160208801610a09565b01601501949350505050565b81810381811115610cc557610cc5610ae8565b92915050565b5f60208284031215610cdb575f80fd5b8151610a6881610baa565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f52603260045260245ffd5b5f60a08201878352602087602085015260a0604085015281875180845260c0860191506020890193505f5b81811015610d5e5784516001600160a01b031683529383019391830191600101610d39565b50506001600160a01b03969096166060850152505050608001529392505050565b5f6020808385031215610d90575f80fd5b825167ffffffffffffffff80821115610da7575f80fd5b818501915085601f830112610dba575f80fd5b815181811115610dcc57610dcc610ce6565b8060051b604051601f19603f83011681018181108582111715610df157610df1610ce6565b604052918252848201925083810185019188831115610e0e575f80fd5b938501935b82851015610e2c57845184529385019392850192610e13565b98975050505050505050565b5f60018201610e4957610e49610ae8565b5060010190565b634e487b7160e01b5f52601260045260245ffd5b5f82610e7257610e72610e50565b500490565b5f82610e8557610e85610e50565b500690565b80820180821115610cc557610cc5610ae8565b5f60208284031215610ead575f80fd5b505191905056fea2646970667358221220bec7cf65fcc722d1cc99449a558e05bda79d0ad08d1526f3a66a2ad6950490f564736f6c63430008180033").unwrap();
    return Bytecode::new_raw(bytes.into());
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

fn withdraw_weth(
    latest_gas_limit: u64,
    latest_gas_price: alloy_primitives::Uint<256, 4>,
    wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    four_eth: alloy_primitives::Uint<256, 4>,
) -> Result<(), anyhow::Error> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
        function deposit(uint256 amount) external payable;
        function withdraw(uint256 amount) external;
    }

    let function_call = withdrawCall { amount: four_eth };

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

fn check_weth_balance(
    wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
    caller: Option<Address>,
) -> Result<(), anyhow::Error> {
    alloy::sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    let function_call = balanceOfCall {
        account: wallet_address,
    };

    let function_call_data = function_call.abi_encode();

    let caller = caller.unwrap_or(wallet_address);

    let new_tx = Tx {
        caller: caller,
        transact_to: weth_addr(),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let result = simulator.call(new_tx)?;

    let balance = U256::from_be_slice(&result.output);

    let balance = balance / U256::from(10).pow(U256::from(18));

    println!("WETH balance: {:?}", balance);
    Ok(())
}
