use crate::common::revm::{EvmSimulator, Tx};
use ::log::info;
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolValue};
use anyhow::Result;
use revm::primitives::{address, Address, Bytecode, U256};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

pub async fn simulation(
    target_pool: Address,
    token_a: Address,
    token_b: Address,
    amount: U256,
    simulator: Arc<Mutex<EvmSimulator<'_>>>,
) -> Result<U256> {
    //      - Simulation:
    //       - Simply we are going to, get required info (latest block, pool needed?, adjacent pool?)
    //       - deploy our contract
    //       - Create a transaction to send to our contract
    //       - Execute the transaction
    //       - Log the results
    //       - check if eth balance has increased

    
    info!("Starting simulation...");
    // Start a timer
    let start_time = std::time::Instant::now();
    info!(" start_time{:?}", start_time);

    info!("Simulation started at: {:?}", start_time);

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

    // Verify contract states after loading
    // let pool_info = simulator
    //     .lock()
    //     .await
    //     .get_account(get_address(AddressType::V3Pool))
    //     .await?;
    // info!("Pool code loaded: {}", !pool_info.code.unwrap().is_empty());
    // info!("Pool code hash: {}", pool_info.code_hash);

    let my_wallet = PrivateKeySigner::random();
    // set initial eth value;
    let initial_eth_balance = U256::from(1000) * U256::from(10).pow(U256::from(18));
    simulator
        .lock()
        .await
        .set_eth_balance(my_wallet.address(), initial_eth_balance)
        .await;

    check_weth_balance(
        my_wallet.address(),
        &mut *simulator.lock().await,
        &latest_gas_limit,
        &latest_gas_price,
        None,
    )
    .await
    .expect("error checking weth balance");

    let thousand = U256::from(1_000) * U256::from(10).pow(U256::from(18));

    // let trillion_pepe = U256::from(100_000_000_000_000u64) * U256::from(10).pow(U256::from(18));
    // let hundred_thousand_usdc = U256::from(100_000_000) * U256::from(10).pow(U256::from(6));

    // NOTE: to test the simulation I am swapping here to depress the price on the v2 pool

    // simulation_swap(
    //     simulator.lock().await,
    //     &latest_gas_price,
    //     &latest_gas_limit,
    //     token_a,
    //     token_b,
    // )
    // .await
    // .expect("Error doing setup swap");

    // test our hello world function
    // Check pool state

    info!("Checking V3 pool state...");
    simulator
        .lock()
        .await
        .load_v3_pool_state(target_pool)
        .await?;
    let pool_info = simulator.lock().await.get_account(target_pool).await?;
    // info!("Pool state: {:?}", pool_info);

    // Check token states
    // info!("Checking token states...");
    // let token_a_info = simulator.lock().await.get_account(token_a).await?;
    // info!("Token A state: {:?}", token_a_info);
    // let token_b_info = simulator.lock().await.get_account(token_b).await?;
    // info!("Token B state: {:?}", token_b_info);

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
        pool0: target_pool,
        fee1: fee1,
        tokenIn: token_a,
        tokenOut: token_b,
        amountIn: amount,
    };

    let function_call_data = function_call.abi_encode();

    // // Check weth balance just to see if it's all good
    // Note: create the transaction
    let new_tx = Tx {
        caller: my_wallet.address(),
        transact_to: simulator.lock().await.owner,
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    // Call said transaction
    match simulator.lock().await.call(new_tx) {
        Ok(res) => info!("Tx Result: {:?}", res),
        Err(err) => info!("Tx not successful: {:?}", err),
    }

    let balance = check_weth_balance(
        my_wallet.address(),
        &mut *simulator.lock().await,
        &latest_gas_limit,
        &latest_gas_price,
        None,
    )
    .await
    .expect("Error checking weth balance");

    info!("Simulation took: {:?}", start_time.elapsed());
    Ok(balance)
}

async fn simulation_swap<'a>(
    mut evm_simulator: MutexGuard<'_, EvmSimulator<'a>>,
    latest_gas_price: &U256,
    latest_gas_limit: &u64,
    token_a: Address,
    token_b: Address,
) -> Result<()> {
    // the idea here is to swap a bunch of token_a for token_b to create an arbitrage opp simulation
    // Load router state

    // Load pool state
    let pool_address = address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801");
    check_contract_state(&mut evm_simulator, pool_address).await?;
    let wallet = PrivateKeySigner::random();

    let wallet_address = wallet.address();

    let amount_in = U256::from(600_000) * U256::from(10).pow(U256::from(18));

    evm_simulator
        .set_eth_balance(wallet_address, amount_in)
        .await;

    let amount_in = U256::from(100) * U256::from(10).pow(U256::from(18));

    alloy::sol! {
        function deposit() external payable;
        #[derive(Debug)]
        function withdraw(uint256 amount) external;
    }

    let deposit_call = depositCall {};
    let deposit_call_data = deposit_call.abi_encode();

    let deposit_tx = Tx {
        caller: wallet_address,
        transact_to: get_address(AddressType::Weth),
        data: deposit_call_data.into(),
        value: amount_in,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let res = evm_simulator
        .call(deposit_tx)
        .expect("Failed to deposit ETH");

    //     let withdraw_call = withdrawCall { amount: amount_in };
    //     let withdraw_call_data = withdraw_call.abi_encode();

    //     let withdraw_tx = Tx {
    //         caller: wallet_address,
    //         transact_to: get_address(AddressType::Weth),
    //         data: withdraw_call_data.into(),
    //         value: U256::ZERO,
    //         gas_limit: *latest_gas_limit,
    //         gas_price: *latest_gas_price,
    //     };

    //     let result = evm_simulator
    //         .call(withdraw_tx)
    //         .expect("Failed to withdraw WETH");

    //    info!("result {:?}", result );

    // // Before V2 swap
    // log_all_balances(
    //     &mut evm_simulator,
    //     wallet_address,
    //     token_a,
    //     token_b,
    //     latest_gas_limit,
    //     latest_gas_price,
    // )
    // .await?;

    // alloy::sol! {
    //     function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
    //     external
    //     payable
    //     returns (uint[] memory amounts);
    // }
    // let input_params = swapExactETHForTokensCall {
    //     amountOutMin: U256::from(1),
    //     path: vec![weth_addr(), token_a].into(),
    //     to: wallet_address,
    //     deadline: U256::MAX,
    // }
    // .abi_encode();

    // let new_tx = Tx {
    //     caller: wallet_address,
    //     transact_to: v2_router_addr(),
    //     data: input_params.into(),
    //     value: one_hundred_ether(),
    //     gas_limit: *latest_gas_limit,
    //     gas_price: *latest_gas_price,
    // };

    // match evm_simulator.call(new_tx) {
    //     Ok(res) => info!("Swap successful: {:?}", res),
    //     Err(err) => info!("Error: {:?}", err),
    // };

    // info!("Swapped eth for token a");

    check_weth_balance(
        wallet_address,
        &mut *evm_simulator,
        latest_gas_limit,
        latest_gas_price,
        None,
    )
    .await
    .unwrap();
    // After V2 swap
    log_all_balances(
        &mut evm_simulator,
        wallet_address,
        token_a,
        token_b,
        latest_gas_limit,
        latest_gas_price,
    )
    .await?;

    // Do approvals

    router_token_approve(
        &mut evm_simulator,
        latest_gas_price,
        latest_gas_limit,
        wallet_address,
        get_address(AddressType::V3Router),
        get_address(AddressType::Weth),
    )
    .await
    .unwrap();

    // Inside simulation_swap function
    alloy::sol! {
        #[derive(Debug)]
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        #[derive(Debug)]
        function exactInputSingle(
            ExactInputSingleParams calldata params
        ) external returns (uint256 amountOut);
    }

    let amount_in = U256::from(10) * U256::from(10).pow(U256::from(18));

    let params = ExactInputSingleParams {
        tokenIn: get_address(AddressType::Weth),
        tokenOut: token_a,
        fee: alloy_primitives::aliases::U24::from(3000),
        recipient: wallet_address,
        deadline: U256::MAX,
        amountIn: amount_in,
        amountOutMinimum: U256::from(1),
        sqrtPriceLimitX96: alloy::primitives::U160::MAX,
    };

    let f_call = exactInputSingleCall { params: params };
    info!("params as bytes: {:?}", f_call);
    let f_call = f_call.abi_encode();

    // let exact_input_data = exactInputSingleCall {
    //     params: input_params,
    // }
    // .abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: get_address(AddressType::V3Router),
        data: f_call.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    evm_simulator.call(new_tx).expect("Failed to do sim swap");

    // After V3 swap
    log_all_balances(
        &mut evm_simulator,
        wallet_address,
        token_a,
        token_b,
        latest_gas_limit,
        latest_gas_price,
    )
    .await?;

    // router_token_approve(
    //     &mut evm_simulator,
    //     latest_gas_price,
    //     latest_gas_limit,
    //     wallet_address,
    //     v2_router_addr(),
    //     token_b,
    // )
    // .await
    // .unwrap();

    Ok(())
}

async fn router_token_approve<'a>(
    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'a>>,
    latest_gas_price: &alloy_primitives::Uint<256, 4>,
    latest_gas_limit: &u64,
    wallet_address: Address,
    router: Address,
    token: Address,
) -> Result<(), anyhow::Error> {
    // First, approve WETH for the V2 router
    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }
    let approve_data = approveCall {
        spender: router,
        amount: U256::MAX, // Infinite approval, you can set a specific amount instead
    }
    .abi_encode();

    let approve_tx = Tx {
        caller: wallet_address,
        transact_to: token,
        data: approve_data.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };
    info!("Approving {} for V2 router...", token);
    let approve_result = evm_simulator
        .call(approve_tx)
        .expect("Error approving router for token");

    info!("approve_result {:?}", approve_result);

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

pub enum AddressType {
    Weth,
    Usdc,
    OfficialQuoter,
    CustomQuoter,
    Pool3000Pepe,
    Uni,
    Pepe,
    Pool500,
    Pool3000,
    V3Router,
    V2Router,
    Random,
    V2Pool,
    V3Pool,
    Factory,
}

pub fn get_address(address_type: AddressType) -> Address {
    match address_type {
        AddressType::Weth => address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        AddressType::Usdc => address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        AddressType::OfficialQuoter => address!("61fFE014bA17989E743c5F6cB21bF9697530B21e"),
        AddressType::CustomQuoter => address!("A5C381211A406b48A073E954e6949B0D49506bc0"),
        AddressType::Pool3000Pepe => address!("11950d141EcB863F01007AdD7D1A342041227b58"),
        AddressType::Uni => address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        AddressType::Pepe => address!("6982508145454Ce325dDbE47a25d4ec3d2311933"),
        AddressType::Pool500 => address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
        AddressType::Pool3000 => address!("8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8"),
        AddressType::V3Router => address!("68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
        AddressType::V2Router => address!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D"),
        AddressType::Random => address!("60d023f1b06edcEAcC4799a73865D1baaBBd355f"),
        AddressType::V3Pool => address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801"),
        AddressType::V2Pool => address!("d3d2E2692501A5c9Ca623199D38826e513033a17"),
        AddressType::Factory => address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
    }
}
pub fn arboo_bytecode() -> Bytecode {
    let bytes = hex::decode("608060405234801561000f575f80fd5b506004361061003f575f3560e01c80637bd04165146100435780639476f92214610058578063fa461e331461008e575b5f80fd5b610056610051366004610855565b6100a1565b005b604080518082018252600b81526a12195b1b1bc815dbdc9b1960aa1b6020820152905161008591906108f6565b60405180910390f35b61005661009c36600461090f565b6101c8565b6001600160a01b03808316908416105f816100da576100d5600173fffd8963efd1fc6a506488495d951d5263988d2661099c565b6100ea565b6100ea6401000276a360016109c3565b604080513360208201526001600160a01b03808b169282019290925262ffffff89166060820152818816608082015290861660a082015260c0810185905283151560e08201529091505f906101000160408051601f1981840301815290829052630251596160e31b825291506001600160a01b0389169063128acb089061017d90309087908990889088906004016109e3565b60408051808303815f875af1158015610198573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906101bc9190610a28565b50505050505050505050565b5f8080808080806101db888a018a610a57565b9650965096509650965096509650856001600160a01b0316336001600160a01b03161461023c5760405162461bcd60e51b815260206004820152600a6024820152693737ba1039b2b73232b960b11b60448201526064015b60405180910390fd5b5f816102505761024b8c610add565b610259565b6102598b610add565b90505f6102698587846001610433565b90508381116102ba5760405162461bcd60e51b815260206004820152601860248201527f6275794261636b416d6f756e74203c20616d6f756e74496e00000000000000006044820152606401610233565b5f6102c58583610af7565b90505f81116103035760405162461bcd60e51b815260206004820152600a602482015269070726f666974203d20360b41b6044820152606401610233565b60405163a9059cbb60e01b81526001600160a01b038a811660048301526024820187905288169063a9059cbb906044016020604051808303815f875af115801561034f573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906103739190610b10565b506001600160a01b03871673c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2146103a8576103a3878b83610657565b610423565b60405163a9059cbb60e01b81523060048201526024810182905273c02aaa39b223fe8d0a0e5c4f27ead9083c756cc29063a9059cbb906044016020604051808303815f875af11580156103fd573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906104219190610b10565b505b5050505050505050505050505050565b60405163095ea7b360e01b8152737a250d5630b4cf539739df2c5dacb4c659f2488d6004820152602481018390525f906001600160a01b0386169063095ea7b3906044016020604051808303815f875af1158015610493573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906104b79190610b10565b50604080516002808252606080830184529260208301908036833701905050905085815f815181106104eb576104eb610b3f565b60200260200101906001600160a01b031690816001600160a01b031681525050848160018151811061051f5761051f610b3f565b6001600160a01b03909216602092830291909101909101526040516338ed173960e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d906338ed1739906105789088908890879030904290600401610b53565b5f604051808303815f875af1158015610593573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f191682016040526105ba9190810190610bc4565b90505f816001815181106105d0576105d0610b3f565b602002602001015110156106305760405162461bcd60e51b815260206004820152602160248201527f616d6f756e74312061667465722076325f73776170206c657373207468616e206044820152600360fc1b6064820152608401610233565b8060018151811061064357610643610b3f565b602002602001015192505050949350505050565b60405163095ea7b360e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc456004820152602481018290526001600160a01b0384169063095ea7b3906044016020604051808303815f875af11580156106b5573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906106d99190610b10565b505f6040518060e00160405280856001600160a01b0316815260200173c02aaa39b223fe8d0a0e5c4f27ead9083c756cc26001600160a01b031681526020016101f462ffffff168152602001846001600160a01b031681526020018381526020015f81526020016401000276a3600161075291906109c3565b6001600160a01b03908116909152604080516304e45aaf60e01b81528351831660048201526020840151831660248201529083015162ffffff1660448201526060830151821660648201526080830151608482015260a083015160a482015260c083015190911660c48201529091507368b3465833fb72a70ecdf485e0e4c7bd8665fc45906304e45aaf9060e4016020604051808303815f875af11580156107fc573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906108209190610c7d565b5050505050565b6001600160a01b038116811461083b575f80fd5b50565b803562ffffff81168114610850575f80fd5b919050565b5f805f805f60a08688031215610869575f80fd5b853561087481610827565b94506108826020870161083e565b9350604086013561089281610827565b925060608601356108a281610827565b949793965091946080013592915050565b5f81518084525f5b818110156108d7576020818501810151868301820152016108bb565b505f602082860101526020601f19601f83011685010191505092915050565b602081525f61090860208301846108b3565b9392505050565b5f805f8060608587031215610922575f80fd5b8435935060208501359250604085013567ffffffffffffffff80821115610947575f80fd5b818701915087601f83011261095a575f80fd5b813581811115610968575f80fd5b886020828501011115610979575f80fd5b95989497505060200194505050565b634e487b7160e01b5f52601160045260245ffd5b6001600160a01b038281168282160390808211156109bc576109bc610988565b5092915050565b6001600160a01b038181168382160190808211156109bc576109bc610988565b6001600160a01b0386811682528515156020830152604082018590528316606082015260a0608082018190525f90610a1d908301846108b3565b979650505050505050565b5f8060408385031215610a39575f80fd5b505080516020909101519092909150565b801515811461083b575f80fd5b5f805f805f805f60e0888a031215610a6d575f80fd5b8735610a7881610827565b96506020880135610a8881610827565b9550610a966040890161083e565b94506060880135610aa681610827565b93506080880135610ab681610827565b925060a0880135915060c0880135610acd81610a4a565b8091505092959891949750929550565b5f600160ff1b8201610af157610af1610988565b505f0390565b81810381811115610b0a57610b0a610988565b92915050565b5f60208284031215610b20575f80fd5b815161090881610a4a565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f52603260045260245ffd5b5f60a08201878352602087602085015260a0604085015281875180845260c0860191506020890193505f5b81811015610ba35784516001600160a01b031683529383019391830191600101610b7e565b50506001600160a01b03969096166060850152505050608001529392505050565b5f6020808385031215610bd5575f80fd5b825167ffffffffffffffff80821115610bec575f80fd5b818501915085601f830112610bff575f80fd5b815181811115610c1157610c11610b2b565b8060051b604051601f19603f83011681018181108582111715610c3657610c36610b2b565b604052918252848201925083810185019188831115610c53575f80fd5b938501935b82851015610c7157845184529385019392850192610c58565b98975050505050505050565b5f60208284031215610c8d575f80fd5b505191905056fea26469706673582212203ae4e0c5f8b2e03c2e8dcddd59bc9d153e80f7ec778f7eea043b99ba0a738ba964736f6c63430008180033").unwrap();
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
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    simulator.call(new_tx)?;
    Ok(())
}

async fn check_weth_balance(
    wallet_address: Address,
    simulator: &mut EvmSimulator<'_>,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
    caller: Option<Address>,
) -> Result<U256, anyhow::Error> {
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
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let result = simulator.call(new_tx)?;

    let balance = U256::from_be_slice(&result.output);

    let balance = balance / U256::from(10).pow(U256::from(18));

    info!("WETH balance: {:?}", balance);
    Ok(balance)
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
async fn check_contract_state(
    evm_simulator: &mut EvmSimulator<'_>,
    contract_address: Address,
) -> Result<()> {
    let account_info = evm_simulator.get_account(contract_address).await?;
    info!("Contract at {:?} state:", contract_address);
    info!("Contract : {:?}", account_info.nonce);
    info!("Balance: {:?}", account_info.balance);
    Ok(())
}
async fn check_token_balance(
    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'_>>,
    token: Address,
    wallet: Address,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
) -> Result<U256> {
    alloy::sol! {
        function balanceOf(address) external view returns (uint256);
    }

    let balance_call = balanceOfCall { _0: wallet }.abi_encode();

    let tx = Tx {
        caller: wallet,
        transact_to: token,
        data: balance_call.into(),
        value: U256::ZERO,
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    let result = evm_simulator.call(tx).expect("Error getting balance");
    let balance = U256::from_be_slice(&result.output);
    Ok(balance)
}
async fn log_all_balances(
    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'_>>,
    wallet: Address,
    token_a: Address,
    token_b: Address,
    latest_gas_limit: &u64,
    latest_gas_price: &U256,
) -> Result<()> {
    let eth_balance = evm_simulator.get_eth_balance(wallet).await;
    let token_a_balance = check_token_balance(
        evm_simulator,
        token_a,
        wallet,
        latest_gas_limit,
        latest_gas_price,
    )
    .await?;
    let token_b_balance = check_token_balance(
        evm_simulator,
        token_b,
        wallet,
        latest_gas_limit,
        latest_gas_price,
    )
    .await?;

    info!("Balances for wallet {:?}:", wallet);
    info!("ETH: {:?}", eth_balance);
    info!("Token A: {:?}", token_a_balance);
    info!("Token B: {:?}", token_b_balance);

    Ok(())
}
