use crate::common::revm::{EvmSimulator, Tx};
use ::log::info;
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::WsConnect;
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::Bytes;
use alloy_sol_types::{SolCall, SolValue};
use anyhow::{anyhow, Result};
use revm::primitives::{address, Address, Bytecode, U256};
use std::ops::RangeBounds;
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
    simulator.lock().await.deploy(arboo_bytecode()).await;

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

    let my_wallet = PrivateKeySigner::random();
    // set initial eth value;
    let initial_eth_balance = U256::from(1000) * U256::from(10).pow(U256::from(18));
    simulator
        .lock()
        .await
        .set_eth_balance(my_wallet.address(), initial_eth_balance)
        .await;

    alloy::sol! {
        function swapEthForWeth(
            address to,
            uint256 deadline
        ) external payable;
    };
    let function_call = swapEthForWethCall {
        to: my_wallet.address(),
        deadline: U256::from(9999999999_u64),
    };

    let function_call_data = function_call.abi_encode();

    // NOTE: to test the simulation I am swapping here to depress the price on the v2 pool

    let new_tx = Tx {
        caller: my_wallet.address(),
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: U256::from(10) * U256::from(10).pow(U256::from(18)),
        gas_limit: latest_gas_limit as u64,
        gas_price: latest_gas_price,
    };

    let result = simulator.lock().await.call(new_tx)?;

    info!("result from swapping, {:?}", result);
    check_weth_balance(
        my_wallet.address(),
        &mut *simulator.lock().await,
        &latest_gas_limit,
        &latest_gas_price,
        None,
    )
    .await
    .unwrap();
    // Load WETH contract state
    // simulator.lock().await.load_account(token_a).await;
    // simulator.lock().await.load_account(token_b).await;
    // simulator
    //     .lock()
    //     .await
    //     .load_account(get_address(AddressType::Weth))
    //     .await;
    // simulator
    //     .lock()
    //     .await
    //     .load_v3_pool_state(target_pool)
    //     .await
    //     .unwrap();
    let code_address = simulator.lock().await.owner;
    info!(
        "res from get account A {:?}",
        simulator
            .lock()
            .await
            .get_account(token_a)
            .await
            .expect("Failed to get account")
    );
    info!(
        "res from get account B {:?}",
        simulator
            .lock()
            .await
            .get_account(token_b)
            .await
            .expect("Failed to get account")
    );
    info!(
        "res from get account WETH{:?}",
        simulator
            .lock()
            .await
            .get_account(get_address(AddressType::Weth))
            .await
            .expect("Failed to get account")
    );
    info!(
        "res from get account TARGET_POOL{:?}",
        simulator
            .lock()
            .await
            .get_account(target_pool)
            .await
            .expect("Failed to get account")
    );

    info!(
        "res from get account TARGET_POOL{:?}",
        simulator
            .lock()
            .await
            .get_account(code_address)
            .await
            .expect("Failed to get account")
    );

    // info!("res from another func {:?}",simulator.lock().await.get_contract(target_pool).await.expect("error"));

    let reserves = get_pair_reserves(target_pool, simulator.clone(), my_wallet.address())
        .await
        .expect("failed to get reserves");

    info!("reserves {:?}", reserves);

    check_weth_balance(
        my_wallet.address(),
        &mut *simulator.lock().await,
        &latest_gas_limit,
        &latest_gas_price,
        None,
    )
    .await
    .expect("error checking weth balance");

    sim_swap_v2_router(
        simulator.lock().await,
        &latest_gas_price,
        &latest_gas_limit,
        token_b,
        token_a,
    )
    .await
    .expect("error");

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

    info!("function_call {:?}", function_call);

    let function_call_data = function_call.abi_encode();

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
        Ok(res) => {
            evm_decoder(res.output).unwrap();
        }
        Err(err) => return Ok(U256::ZERO),
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

    Ok(balance)
}
async fn sim_swap_v2_router<'a>(
    mut evm_simulator: MutexGuard<'_, EvmSimulator<'a>>,
    latest_gas_price: &U256,
    latest_gas_limit: &u64,
    token_a: Address,
    token_b: Address,
) -> Result<()> {
    // the idea here is to swap a bunch of token_a for token_b to create an arbitrage opp simulation

    let wallet = PrivateKeySigner::random();

    let wallet_address = wallet.address();

    let amount_in = U256::from(600_000) * U256::from(10).pow(U256::from(18));

    evm_simulator
        .set_eth_balance(wallet_address, amount_in)
        .await;

    let amount_in = U256::from(1000) * U256::from(10).pow(U256::from(18));

    alloy::sol! {
        function deposit() external payable;
        #[derive(Debug)]
        function withdraw(uint256 amount) external;
    }

    let deposit_call = depositCall {};
    let deposit_call_data = deposit_call.abi_encode();

    info!("deposit_call ");

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

    let res = evm_decoder(res.output);

    info!("res {:?}", res);

    alloy::sol! {
        function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
        external
        payable
        returns (uint[] memory amounts);
    }
    let input_params = swapExactETHForTokensCall {
        amountOutMin: U256::from(1),
        path: vec![get_address(AddressType::Weth), token_a].into(),
        to: wallet_address,
        deadline: U256::MAX,
    }
    .abi_encode();

    let new_tx = Tx {
        caller: wallet_address,
        transact_to: get_address(AddressType::V2Router),
        data: input_params.into(),
        value: one_hundred_ether(),
        gas_limit: *latest_gas_limit,
        gas_price: *latest_gas_price,
    };

    match evm_simulator.call(new_tx) {
        Ok(res) => {
            evm_decoder(res.output).expect("Error decoding");
        }
        Err(err) => info!("Error: {:?}", err),
    };
    Ok(())
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

    info!("res {:?}", res);

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

    let weth_balance = check_weth_balance(
        wallet_address,
        &mut evm_simulator,
        latest_gas_limit,
        latest_gas_price,
        None,
    )
    .await
    .expect("Error checking balance");

    info!("weth_balance {:?}", weth_balance);

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
    let bytes = hex::decode("608060405234801561000f575f80fd5b5060043610610034575f3560e01c80637bd0416514610038578063fa461e331461004d575b5f80fd5b61004b610046366004610780565b610060565b005b61004b61005b3660046107de565b610187565b6001600160a01b03808316908416105f8161009957610094600173fffd8963efd1fc6a506488495d951d5263988d2661086b565b6100a9565b6100a96401000276a36001610892565b604080513360208201526001600160a01b03808b169282019290925262ffffff89166060820152818816608082015290861660a082015260c0810185905283151560e08201529091505f906101000160408051601f1981840301815290829052630251596160e31b825291506001600160a01b0389169063128acb089061013c90309087908990889088906004016108b2565b60408051808303815f875af1158015610157573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061017b919061092a565b50505050505050505050565b5f80808080808061019a888a018a610959565b9650965096509650965096509650856001600160a01b0316336001600160a01b0316146101e157604051630fa958bb60e31b81523360048201526024015b60405180910390fd5b5f816101f5576101f08c6109df565b6101fe565b6101fe8b6109df565b90505f61020e8587846001610397565b905083811161023a57604051638b02883f60e01b815260048101829052602481018590526044016101d8565b5f61024585836109f9565b9050805f0361026757604051635c1822b160e01b815260040160405180910390fd5b60405163a9059cbb60e01b81526001600160a01b038a811660048301526024820187905288169063a9059cbb906044016020604051808303815f875af11580156102b3573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906102d79190610a12565b506001600160a01b03871673c02aaa39b223fe8d0a0e5c4f27ead9083c756cc21461030c57610307878b83610582565b610387565b60405163a9059cbb60e01b81523060048201526024810182905273c02aaa39b223fe8d0a0e5c4f27ead9083c756cc29063a9059cbb906044016020604051808303815f875af1158015610361573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906103859190610a12565b505b5050505050505050505050505050565b60405163095ea7b360e01b8152737a250d5630b4cf539739df2c5dacb4c659f2488d6004820152602481018390525f906001600160a01b0386169063095ea7b3906044016020604051808303815f875af11580156103f7573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061041b9190610a12565b50604080516002808252606080830184529260208301908036833701905050905085815f8151811061044f5761044f610a48565b60200260200101906001600160a01b031690816001600160a01b031681525050848160018151811061048357610483610a48565b6001600160a01b03909216602092830291909101909101526040516338ed173960e01b81525f90737a250d5630b4cf539739df2c5dacb4c659f2488d906338ed1739906104dc9088908890879030904290600401610a5c565b5f604051808303815f875af11580156104f7573d5f803e3d5ffd5b505050506040513d5f823e601f3d908101601f1916820160405261051e9190810190610acd565b90505f8160018151811061053457610534610a48565b6020026020010151101561055b5760405163820bf1e560e01b815260040160405180910390fd5b8060018151811061056e5761056e610a48565b602002602001015192505050949350505050565b60405163095ea7b360e01b81527368b3465833fb72a70ecdf485e0e4c7bd8665fc456004820152602481018290526001600160a01b0384169063095ea7b3906044016020604051808303815f875af11580156105e0573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906106049190610a12565b505f6040518060e00160405280856001600160a01b0316815260200173c02aaa39b223fe8d0a0e5c4f27ead9083c756cc26001600160a01b031681526020016101f462ffffff168152602001846001600160a01b031681526020018381526020015f81526020016401000276a3600161067d9190610892565b6001600160a01b03908116909152604080516304e45aaf60e01b81528351831660048201526020840151831660248201529083015162ffffff1660448201526060830151821660648201526080830151608482015260a083015160a482015260c083015190911660c48201529091507368b3465833fb72a70ecdf485e0e4c7bd8665fc45906304e45aaf9060e4016020604051808303815f875af1158015610727573d5f803e3d5ffd5b505050506040513d601f19601f8201168201806040525081019061074b9190610b86565b5050505050565b6001600160a01b0381168114610766575f80fd5b50565b803562ffffff8116811461077b575f80fd5b919050565b5f805f805f60a08688031215610794575f80fd5b853561079f81610752565b94506107ad60208701610769565b935060408601356107bd81610752565b925060608601356107cd81610752565b949793965091946080013592915050565b5f805f80606085870312156107f1575f80fd5b8435935060208501359250604085013567ffffffffffffffff80821115610816575f80fd5b818701915087601f830112610829575f80fd5b813581811115610837575f80fd5b886020828501011115610848575f80fd5b95989497505060200194505050565b634e487b7160e01b5f52601160045260245ffd5b6001600160a01b0382811682821603908082111561088b5761088b610857565b5092915050565b6001600160a01b0381811683821601908082111561088b5761088b610857565b5f60018060a01b03808816835260208715156020850152866040850152818616606085015260a06080850152845191508160a08501525f5b828110156109065785810182015185820160c0015281016108ea565b50505f60c0828501015260c0601f19601f8301168401019150509695505050505050565b5f806040838503121561093b575f80fd5b505080516020909101519092909150565b8015158114610766575f80fd5b5f805f805f805f60e0888a03121561096f575f80fd5b873561097a81610752565b9650602088013561098a81610752565b955061099860408901610769565b945060608801356109a881610752565b935060808801356109b881610752565b925060a0880135915060c08801356109cf8161094c565b8091505092959891949750929550565b5f600160ff1b82016109f3576109f3610857565b505f0390565b81810381811115610a0c57610a0c610857565b92915050565b5f60208284031215610a22575f80fd5b8151610a2d8161094c565b9392505050565b634e487b7160e01b5f52604160045260245ffd5b634e487b7160e01b5f52603260045260245ffd5b5f60a08201878352602087602085015260a0604085015281875180845260c0860191506020890193505f5b81811015610aac5784516001600160a01b031683529383019391830191600101610a87565b50506001600160a01b03969096166060850152505050608001529392505050565b5f6020808385031215610ade575f80fd5b825167ffffffffffffffff80821115610af5575f80fd5b818501915085601f830112610b08575f80fd5b815181811115610b1a57610b1a610a34565b8060051b604051601f19603f83011681018181108582111715610b3f57610b3f610a34565b604052918252848201925083810185019188831115610b5c575f80fd5b938501935b82851015610b7a57845184529385019392850192610b61565b98975050505050505050565b5f60208284031215610b96575f80fd5b505191905056fea26469706673582212208ac3eb45161f0ace562f3f2dc9ffb00efd25a492d735bc0e080ca4e71e5ed80364736f6c63430008180033").unwrap();
    return Bytecode::new_raw(bytes.into());
}

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

fn evm_decoder(error_data: Bytes) -> Result<String> {
    if error_data.len() < 1 {
        return Err(anyhow!("No Error Message"));
    }
    // The next 32 bytes is the offset to where the string data starts
    // The next 32 bytes after that is the length of the string
    // Then comes the actual string data
    let string_hex = &error_data[64..]; // Skip the first two 32-byte chunks

    // Convert hex to string
    let decoded_string = String::from_utf8(
        hex::decode(string_hex)
            .expect("Error Decoding")
            .into_iter()
            .filter(|&x| x != 0) // Remove null terminators
            .collect::<Vec<u8>>(),
    )
    .expect("Invalid UTF-8");

    println!("Decoded error message: {}", decoded_string);
    Ok(decoded_string)
    // Output: "UniswapV2Router: INVALID_PATH"
}

async fn get_pair_reserves(
    pair_address: Address,
    evm_simulator: Arc<Mutex<EvmSimulator<'_>>>,
    caller: Address,
) -> Result<(U256, U256)> {
    alloy::sol! {
        function getReserves() external view returns (uint112,uint112,uint32);
    };
    let calldata = getReservesCall {}.abi_encode();

    let tx = Tx {
        caller,
        transact_to: pair_address,
        data: calldata.into(),
        value: U256::ZERO,
        gas_price: U256::ZERO,
        gas_limit: 5000000,
    };

    let result = evm_simulator
        .lock()
        .await
        .call(tx)
        .expect("Error getting pair reserves");

    info!("result {:?}", result);
    // Ok((out.0, out.1))
    Ok((U256::ZERO, U256::ZERO))
}

fn log(log_data: String) {
    info!("{}", log_data);
}
