use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::providers::Provider;
use alloy::signers::local::PrivateKeySigner;
use alloy::{
    network::Ethereum, providers::RootProvider, pubsub::PubSubFrontend,
    rpc::types::BlockTransactionsKind,
};
use log::info;
use tokio::sync::Mutex as TokioMutex;

use crate::arbitrage::simulation::{get_address, one_thousand_eth, simulation, AddressType};
use crate::common::revm::Tx;
use alloy_primitives::aliases::U24;
use alloy_primitives::{address, U160, U256, U64};
use alloy_sol_types::SolCall;
use anyhow::Result;

use super::revm::EvmSimulator;
pub async fn sim_test(
    provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
    simulator: Arc<TokioMutex<EvmSimulator<'_>>>,
) -> Result<()> {
    let latest_block = provider
        .get_block(
            BlockId::Number(alloy::eips::BlockNumberOrTag::Latest),
            BlockTransactionsKind::Full,
        )
        .await
        .inspect_err(|err| log::error!("Error getting block: {:?}", err))
        .unwrap_or_default()
        .unwrap_or_default();

    let latest_gas_limit = latest_block.header.gas_limit;
    let latest_gas_price = U256::from(latest_block.header.base_fee_per_gas.expect("gas"));
    let my_wallet = PrivateKeySigner::random();
    // set initial eth value;
    let initial_eth_balance = U256::from(99_000) * U256::from(10).pow(U256::from(18));

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

    let new_tx = Tx {
        caller: my_wallet.address(),
        transact_to: get_address(AddressType::Weth),
        data: function_call_data.into(),
        value: one_thousand_eth(),
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.lock().await.call(new_tx)?;

    alloy::sol! {
        function approve(address spender, uint256 amount) external returns (bool);
    }
    let approve_data = approveCall {
        spender: get_address(AddressType::V3Router),
        amount: U256::MAX, // Infinite approval, you can set a specific amount instead
    }
    .abi_encode();

    let approve_tx = Tx {
        caller: my_wallet.address(),
        transact_to: get_address(AddressType::Weth),
        data: approve_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit,
        gas_price: latest_gas_price,
    };

    simulator.lock().await.call(approve_tx)?;

    alloy::sol! {
        interface ISwapRouter {
              struct ExactInputSingleParams {
                address tokenIn;
                address tokenOut;
                uint24 fee;
                address recipient;
                uint256 amountIn;
                uint256 amountOutMinimum;
                uint160 sqrtPriceLimitX96;
        }
       function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
    }
    }

    let function_params = ISwapRouter::ExactInputSingleParams {
        tokenIn: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        tokenOut: address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
        fee: U24::from(3000),
        recipient: my_wallet.address(),
        amountIn: one_thousand_eth(),
        amountOutMinimum: U256::ZERO,
        sqrtPriceLimitX96: U160::ZERO,
    };

    let function_call = ISwapRouter::exactInputSingleCall {
        params: function_params,
    };
    let function_call_data = function_call.abi_encode();

    let new_tx = Tx {
        caller: my_wallet.address(),
        transact_to: get_address(AddressType::V3Router),
        data: function_call_data.into(),
        value: U256::ZERO,
        gas_limit: latest_gas_limit * 2,
        gas_price: latest_gas_price,
    };

    match simulator.lock().await.call(new_tx) {
        Ok(res) => {
            info!("Res from mock call, {:?}", res);
        }
        Err(err) => info!("Error doing swap of weth to Link {:?},", err),
    };

    match simulation(
        address!("a6Cc3C2531FdaA6Ae1A3CA84c2855806728693e8"),
        address!("514910771AF9Ca656af840dff83E8264EcF986CA"),
        address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        U256::from(999) * U256::from(10).pow(U256::from(18)),
        U24::from(3000),
        simulator.clone(),
    )
    .await
    {
        Ok(res) => {
            info!("balance of sim eth wallet after sim {res}");
        }
        Err(err) => {
            info!("Simulation Errors: {err}");
        }
    }
    Ok(())
}

//async fn router_token_approve<'a>(
//    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'a>>,
//    latest_gas_price: &alloy_primitives::Uint<256, 4>,
//    latest_gas_limit: &u64,
//    wallet_address: Address,
//    router: Address,
//    token: Address,
//) -> Result<(), anyhow::Error> {
//    // First, approve WETH for the V2 router
//    alloy::sol! {
//        function approve(address spender, uint256 amount) external returns (bool);
//    }
//    let approve_data = approveCall {
//        spender: router,
//        amount: U256::MAX, // Infinite approval, you can set a specific amount instead
//    }
//    .abi_encode();
//
//    let approve_tx = Tx {
//        caller: wallet_address,
//        transact_to: token,
//        data: approve_data.into(),
//        value: U256::ZERO,
//        gas_limit: *latest_gas_limit,
//        gas_price: *latest_gas_price,
//    };
//    info!("Approving {} for V2 router...", token);
//    let approve_result = evm_simulator
//        .call(approve_tx)
//        .expect("Error approving router for token");
//
//    info!("approve_result {:?}", approve_result);
//
//    Ok(())
//}
//async fn check_contract_state(
//    evm_simulator: &mut EvmSimulator<'_>,
//    contract_address: Address,
//) -> Result<()> {
//    let account_info = evm_simulator.get_account(contract_address).await?;
//    info!("Contract at {:?} state:", contract_address);
//    info!("Contract : {:?}", account_info.nonce);
//    info!("Balance: {:?}", account_info.balance);
//    Ok(())
//}
//async fn check_token_balance(
//    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'_>>,
//    token: Address,
//    wallet: Address,
//    latest_gas_limit: &u64,
//    latest_gas_price: &U256,
//) -> Result<U256> {
//    alloy::sol! {
//        function balanceOf(address) external view returns (uint256);
//    }
//
//    let balance_call = balanceOfCall { _0: wallet }.abi_encode();
//
//    let tx = Tx {
//        caller: wallet,
//        transact_to: token,
//        data: balance_call.into(),
//        value: U256::ZERO,
//        gas_limit: *latest_gas_limit,
//        gas_price: *latest_gas_price,
//    };
//
//    let result = evm_simulator.call(tx).expect("Error getting balance");
//    let balance = U256::from_be_slice(&result.output);
//    Ok(balance)
//}
//async fn log_all_balances(
//    evm_simulator: &mut MutexGuard<'_, EvmSimulator<'_>>,
//    wallet: Address,
//    token_a: Address,
//    token_b: Address,
//    latest_gas_limit: &u64,
//    latest_gas_price: &U256,
//) -> Result<()> {
//    let eth_balance = evm_simulator.get_eth_balance(wallet).await;
//    let token_a_balance = check_token_balance(
//        evm_simulator,
//        token_a,
//        wallet,
//        latest_gas_limit,
//        latest_gas_price,
//    )
//    .await?;
//    let token_b_balance = check_token_balance(
//        evm_simulator,
//        token_b,
//        wallet,
//        latest_gas_limit,
//        latest_gas_price,
//    )
//    .await?;
//
//    info!("Balances for wallet {:?}:", wallet);
//    info!("ETH: {:?}", eth_balance);
//    info!("Token A: {:?}", token_a_balance);
//    info!("Token B: {:?}", token_b_balance);
//
//    Ok(())
//}
//
//async fn get_pair_reserves(
//    pair_address: Address,
//    evm_simulator: Arc<TokioMutex<EvmSimulator<'_>>>,
//    caller: Address,
//) -> Result<(U256, U256)> {
//    alloy::sol! {
//        function getReserves() external view returns (uint112,uint112,uint32);
//    };
//    let calldata = getReservesCall {}.abi_encode();
//
//    let tx = Tx {
//        caller,
//        transact_to: pair_address,
//        data: calldata.into(),
//        value: U256::ZERO,
//        gas_price: U256::ZERO,
//        gas_limit: 5000000,
//    };
//
//    let result = evm_simulator.lock().await.call(tx)?;
//
//    if result.output.len() != 96 {
//        return Err(anyhow!("Invalid output length"));
//    }
//
//    let first_32 = &result.output[0..32];
//    let second_32 = &result.output[32..64];
//    let third_32 = &result.output[64..96];
//
//    let reserve0 = U256::from_be_slice(first_32);
//    let reserve1 = U256::from_be_slice(second_32);
//
//    Ok((reserve0, reserve1))
//}
//
//fn log(log_data: String) {
//    info!("{}", log_data);
//}
//fn calculate_reserves(
//    token_is_token0: bool,
//    eth_reserve: U256,
//    token_price_in_eth: U256,
//    timestamp: u64,
//) -> U256 {
//    // Calculate token reserve based on desired price
//    // If price = ETH/token, then token_reserve = eth_reserve / token_price_in_eth
//    let token_reserve = eth_reserve
//        .checked_mul(U256::from(10).pow(U256::from(18)))
//        .unwrap()
//        .checked_div(token_price_in_eth)
//        .unwrap();
//
//    // Storage slot for reserves in Uniswap V2 pair is slot 8
//
//    // Pack the values according to Uniswap V2 storage layout
//    // reserve1 (112 bits) | reserve0 (112 bits) | blockTimestampLast (32 bits)
//    let (reserve0, reserve1) = if token_is_token0 {
//        (token_reserve, eth_reserve)
//    } else {
//        (eth_reserve, token_reserve)
//    };
//
//    // Ensure reserves don't exceed 112 bits
//    let max_112bit = U256::from(2).pow(U256::from(112)) - U256::from(1);
//    assert!(reserve0 <= max_112bit, "reserve0 exceeds 112 bits");
//    assert!(reserve1 <= max_112bit, "reserve1 exceeds 112 bits");
//
//    let timestamp_u256 = U256::from(timestamp);
//    (reserve1 << 112) | reserve0 | timestamp_u256
//}
//
//async fn sim_swap_v2_router<'a>(
//    mut evm_simulator: MutexGuard<'_, EvmSimulator<'a>>,
//    latest_gas_price: &U256,
//    latest_gas_limit: &u64,
//    token_a: Address,
//    token_b: Address,
//) -> Result<()> {
//    // the idea here is to swap a bunch of token_a for token_b to create an arbitrage opp simulation
//
//    let wallet = PrivateKeySigner::random();
//
//    let wallet_address = wallet.address();
//
//    let amount_in = U256::from(600_000) * U256::from(10).pow(U256::from(18));
//
//    evm_simulator
//        .set_eth_balance(wallet_address, amount_in)
//        .await;
//
//    let amount_in = U256::from(1000) * U256::from(10).pow(U256::from(18));
//
//    alloy::sol! {
//        function deposit() external payable;
//        #[derive(Debug)]
//        function withdraw(uint256 amount) external;
//    }
//
//    let deposit_call = depositCall {};
//    let deposit_call_data = deposit_call.abi_encode();
//
//    let deposit_tx = Tx {
//        caller: wallet_address,
//        transact_to: get_address(AddressType::Weth),
//        data: deposit_call_data.into(),
//        value: amount_in,
//        gas_limit: *latest_gas_limit,
//        gas_price: *latest_gas_price,
//    };
//
//    let res = evm_simulator
//        .call(deposit_tx)
//        .expect("Failed to deposit ETH");
//
//    // let res = evm_decoder(res.output);
//
//    //    info!("res {:?}", res);
//
//    alloy::sol! {
//        function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline)
//        external
//        payable
//        returns (uint[] memory amounts);
//    }
//    let input_params = swapExactETHForTokensCall {
//        amountOutMin: U256::from(1),
//        path: vec![get_address(AddressType::Weth), token_b].into(),
//        to: wallet_address,
//        deadline: U256::MAX,
//    }
//    .abi_encode();
//
//    let new_tx = Tx {
//        caller: wallet_address,
//        transact_to: get_address(AddressType::V2Router),
//        data: input_params.into(),
//        value: one_hundred_ether(),
//        gas_limit: *latest_gas_limit,
//        gas_price: *latest_gas_price,
//    };
//
//    match evm_simulator.call(new_tx) {
//        Ok(res) => {
//            info!("res from swapExactEthForTokens call {:?}", res);
//        }
//        Err(err) => info!("Error: {:?}", err),
//    };
//    Ok(())
//}
//async fn is_v2_pool(address: Address, provider: Arc<RootProvider<PubSubFrontend>>) -> Result<bool> {
//    // Get the contract bytecode
//    let code = provider
//        .get_code_at(address)
//        .await
//        .unwrap_or(Default::default())
//        .to_string();
//
//    // You can compare against known V2 pool creation code hash
//    // This is the init code hash for Uniswap V2 pairs
//    let v2_init_code_hash = "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f";
//
//    // Or check specific bytecode patterns unique to V2 pools
//    let is_v2 = code.contains(v2_init_code_hash);
//
//    Ok(is_v2)
//}
//
//// The is_v3_pool function uses an incorrect/incomplete hash for checking V3 pools
//// Should use full bytecode verification or a more reliable method
//async fn is_v3_pool(
//    address: Address,
//    provider: &Arc<RootProvider<PubSubFrontend>>,
//) -> Result<bool> {
//    let code = provider.get_code_at(address).await.unwrap().to_string();
//
//    // Use full bytecode verification instead of partial hash
//    let v3_init_code_hash = "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54";
//    let is_v3 = code.contains(v3_init_code_hash);
//    Ok(is_v3)
//}
//
//// functon that takes in a reference to the evm and reference to a pool address, and an amount of required liquidity
//// returns a boolean of if the contract has the required liquidity or not
//async fn liquidity_test(
//    evm: Arc<tokio::sync::Mutex<EvmSimulator<'static>>>,
//    pool_address: &Address,
//    required_liquidity: BigInt,
//    caller_address: Address,
//) -> Result<bool, anyhow::Error> {
//    // construct sol call for liquidity:
//    evm.lock()
//        .await
//        .set_eth_balance(
//            caller_address,
//            U256::from(1000) * U256::from(10).pow(U256::from(18)),
//        )
//        .await;
//    alloy::sol! {
//       function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
//    }
//
//    let params = getReservesCall {}.abi_encode();
//
//    // do call to evm?
//
//    let tx = crate::common::revm::Tx {
//        caller: caller_address,
//        transact_to: *pool_address,
//        value: U256::ZERO,
//        gas_price: U256::from(20_000),
//        gas_limit: 120_000_000u64,
//        data: params.into(),
//    };
//
//    let res = evm.lock().await.call(tx)?;
//
//    let output = decode_reserves_call(&res.output).unwrap_or_else(|e| vec![U256::ZERO, U256::ZERO]);
//
//    let output1 = BigInt::from_signed_bytes_be(&output[0].to_be_bytes_vec());
//
//    let output2 = BigInt::from_signed_bytes_be(&output[1].to_be_bytes_vec());
//
//    let liquidity = BigInt::from(output1 * output2);
//    let liquidity = liquidity.sqrt();
//
//    if liquidity >= BigInt::from(required_liquidity) {
//        return Ok(true);
//    }
//    Ok(false)
//}
