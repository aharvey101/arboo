use crate::common::abi::{decode_get_amount_out_response, get_amount_out_calldata};
use crate::common::helpers::*;
use alloy::providers::ProviderBuilder;
use alloy::sol;
use anyhow::Result;
use dotenv::var;
use revm::db::{AlloyDB, CacheDB};
use revm::primitives::{address, Address, Bytecode, Bytes, U256};
use std::str::FromStr;
use std::sync::Arc;
use url::Url;
use FlashloanArbitrage::executeArbitrageCall;

sol! {
    interface IUniswapV2Pair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }

    interface IUniswapV3Pool {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
        function swap(address recipient, bool zeroForOne, int256 amountSpecified, uint160 sqrtPriceLimitX96, bytes calldata data) external returns (int256 amount0, int256 amount1);
    }

    interface IERC20 {
        function balanceOf(address owner) external view returns (uint);
        function approve(address spender, uint amount) external returns (bool);
        function transfer(address recipient, uint amount) external returns (bool);
    }
}

sol! {

contract FlashloanArbitrage {
    address public pool1;
    address public pool2;
    address public weth;
    address public usdc;

    constructor(address _pool1, address _pool2, address _weth, address _usdc) {
        pool1 = _pool1;
        pool2 = _pool2;
        weth = _weth;
        usdc = _usdc;
    }

    function executeArbitrage(uint amount) external {
        // Call the flashloan function with the encoded data
        IUniswapV3Pool(pool1).swap(
            address(this),
            true,
            int256(amount),
            0,
            abi.encode(amount)
        );
    }

    function uniswapV3Callback(int256 amount0Delta, int256 amount1Delta, bytes calldata data) external {
        require(msg.sender == pool1 || msg.sender == pool2, "Invalid sender");

        uint amount = abi.decode(data, (uint));

        if (msg.sender == pool1) {
            // Swap WETH for USDC on pool1
            IUniswapV3Pool(pool1).swap(
                address(this),
                true,
                int256(amount),
                0,
                abi.encode(amount)
            );
        } else {
            // Swap USDC for WETH on pool2
            IUniswapV3Pool(pool2).swap(
                address(this),
                false,
                int256(amount),
                0,
                abi.encode(amount)
            );

            // Repay the flashloan
            IERC20(weth).transfer(pool1, amount + (amount / 1000)); // Include a small fee
        }
    }
}
}
pub async fn simulation(target_pool: Address, adjacent_pool: Address) -> Result<()> {
    //      - Simulation:
    //          - Simply we are going to simulate:
    //          - borrowing from the lower priced pool
    //          - selling to higher priced exchange for weth
    //          - using weth gained, by required amount on lower priced exchange
    //          - pay back loan with fee
    //          - revenue will be weth gained - weth used to repay loan
    //          - gas cost is the cost of all 3 transactions

    // initialise cache and addresses
    let http_url = var("HTTP_URL").expect("No HTTP_URL").to_string();
    let http_url = Url::from_str(http_url.as_str()).unwrap();
    let mut cache_db = init_cache(http_url).await?;

    // create transaction data
    //
    let volumes = volumes(U256::from(0), one_ether() / U256::from(10), 100);

    // borrow from pool 1

    let borrow = Bytes::from(executeArbitrageCall {
        amount: one_ether(),
    });
    for volume in volumes.into_iter() {
        // Get call data from first pool
        let calldata = get_amount_out_calldata(pool_500_addr(), weth_addr(), usdc_addr(), volume);
        // Execute calldata on the test evm
        let response = revm_revert(me(), custom_quoter_addr(), calldata, &mut cache_db)?;
        // Get usdc amount out
        let usdc_amount_out = decode_get_amount_out_response(response)?;
        // Get calldata from second pool
        let calldata = get_amount_out_calldata(
            pool_3000_addr(),
            usdc_addr(),
            weth_addr(),
            U256::from(usdc_amount_out),
        );
        // get weth amount out
        let response = revm_revert(me(), custom_quoter_addr(), calldata, &mut cache_db)?;
        let weth_amount_out = decode_get_amount_out_response(response)?;

        println!(
            "{} WETH -> USDC {} -> WETH {}",
            volume, usdc_amount_out, weth_amount_out
        );

        let weth_amount_out = U256::from(volume);
        if weth_amount_out > volume {
            let profit = weth_amount_out - volume;
            println!("WETH profit: {}", profit);
        } else {
            println!("No profit.");
        }
    }
    Ok(())
}

async fn init_cache(http_url: url::Url) -> Result<AlloyCacheDB> {
    let provider = Arc::from(ProviderBuilder::new().on_http(http_url));
    let mut cache_db = CacheDB::new(AlloyDB::new(provider.clone(), Default::default()));

    // init acount with official quoter address:
    let mocked_balance = U256::MAX / U256::from(2);
    insert_mapping_storage_slot(
        weth_addr(),
        U256::from(0),
        pool_3000_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        usdc_addr(),
        U256::from(0),
        pool_3000_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        weth_addr(),
        U256::from(0),
        pool_500_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        usdc_addr(),
        U256::from(0),
        pool_500_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;

    init_account(pool_500_addr(), &mut cache_db, provider.clone()).await?;

    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytes::from_str(mocked_erc20).unwrap();
    let mocked_erc20 = Bytecode::new_raw(mocked_erc20);

    init_account_with_bytecode(weth_addr(), mocked_erc20.clone(), &mut cache_db).await?;
    init_account_with_bytecode(usdc_addr(), mocked_erc20.clone(), &mut cache_db).await?;

    let mocked_balance = U256::MAX / (U256::from(2));

    insert_mapping_storage_slot(
        weth_addr(),
        U256::from(0),
        pool_3000_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        usdc_addr(),
        U256::from(0),
        pool_3000_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        weth_addr(),
        U256::from(0),
        pool_500_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    insert_mapping_storage_slot(
        usdc_addr(),
        U256::from(0),
        pool_500_addr(),
        mocked_balance,
        &mut cache_db,
    )
    .await?;
    Ok(cache_db)
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
