use anyhow::Result;
use arbooo::common::logger;
use alloy::providers::Provider;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_evm_simulator_module_availability() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting EVM Simulator Module Availability Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");

    let latest_block_number = test_env.provider.get_block_number().await?;
    info!("📦 Latest block number: {}", latest_block_number);

    use arbooo::common::revm::{Tx, VictimTx};
    use alloy::signers::local::PrivateKeySigner;
    use alloy::primitives::U256;
    use revm::primitives::Bytes;

    let test_owner = PrivateKeySigner::random().address();
    let test_target = PrivateKeySigner::random().address();

    let test_tx = Tx {
        caller: test_owner,
        transact_to: test_target,
        data: Bytes::new(),
        value: U256::ZERO,
        gas_price: U256::from(20_000_000_000u128),
        gas_limit: 21_000,
    };

    info!("✅ Transaction structure created: caller {:?}, target {:?}", test_tx.caller, test_tx.transact_to);

    let victim_tx = VictimTx {
        tx_hash: revm::primitives::B256::ZERO,
        from: test_owner,
        to: test_target,
        data: Bytes::new(),
        value: U256::ZERO,
        gas_price: U256::from(20_000_000_000u128),
        gas_limit: Some(21_000),
    };

    let converted_tx = Tx::from(victim_tx);
    assert_eq!(converted_tx.caller, test_owner);
    assert_eq!(converted_tx.transact_to, test_target);
    assert_eq!(converted_tx.gas_limit, 21_000);

    info!("✅ Transaction type conversions work correctly");
    info!("🎉 EVM Simulator Module Availability Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_evm_simulator_types_and_structures() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting EVM Simulator Types and Structures Test");

    use alloy::signers::local::PrivateKeySigner;
    use alloy::primitives::U256;
    use revm::primitives::{Bytes, B256};
    use arbooo::common::revm::{Tx, VictimTx};

    let addresses = vec![
        PrivateKeySigner::random().address(),
        PrivateKeySigner::random().address(),
        PrivateKeySigner::random().address(),
    ];

    let gas_prices = vec![
        U256::from(10_000_000_000u128),
        U256::from(50_000_000_000u128),
        U256::from(100_000_000_000u128),
    ];

    let values = vec![
        U256::ZERO,
        U256::from(1) * U256::from(10).pow(U256::from(18)),
        U256::from(5) * U256::from(10).pow(U256::from(17)),
    ];

    for (i, ((caller, target), (gas_price, value))) in addresses
        .iter()
        .zip(addresses.iter().skip(1))
        .zip(gas_prices.iter().zip(values.iter()))
        .enumerate()
    {
        let test_data = if i == 0 {
            Bytes::new()
        } else {
            Bytes::from(vec![0x60, 0x80, 0x60, 0x40, 0x52])
        };

        let tx = Tx {
            caller: *caller,
            transact_to: *target,
            data: test_data.clone(),
            value: *value,
            gas_price: *gas_price,
            gas_limit: 100_000 + (i as u64 * 50_000),
        };

        assert_eq!(tx.caller, *caller);
        assert_eq!(tx.transact_to, *target);
        assert_eq!(tx.value, *value);
        assert_eq!(tx.gas_price, *gas_price);

        info!("✅ Transaction {} verified: gas_limit={}, value={} wei", 
              i, tx.gas_limit, tx.value);
    }

    let victim_txs = vec![
        VictimTx {
            tx_hash: B256::from([1u8; 32]),
            from: addresses[0],
            to: addresses[1],
            data: Bytes::new(),
            value: values[0],
            gas_price: gas_prices[0],
            gas_limit: Some(21_000),
        },
        VictimTx {
            tx_hash: B256::from([2u8; 32]),
            from: addresses[1],
            to: addresses[2],
            data: Bytes::from(vec![0x60, 0x40]),
            value: values[1],
            gas_price: gas_prices[1],
            gas_limit: None,
        },
    ];

    for (i, victim_tx) in victim_txs.iter().enumerate() {
        let converted = Tx::from(victim_tx.clone());

        assert_eq!(converted.caller, victim_tx.from);
        assert_eq!(converted.transact_to, victim_tx.to);
        assert_eq!(converted.value, victim_tx.value);
        assert_eq!(converted.gas_price, victim_tx.gas_price);

        let expected_gas_limit = victim_tx.gas_limit.unwrap_or(5_000_000);
        assert_eq!(converted.gas_limit, expected_gas_limit);

        info!("✅ VictimTx {} conversion verified: gas_limit={}", i, converted.gas_limit);
    }

    info!("🎉 EVM Simulator Types and Structures Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_evm_simulator_constants_and_addresses() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting EVM Simulator Constants and Addresses Test");

    use arbooo::arbitrage::simulation::{get_address, AddressType};
    use alloy::primitives::Address;

    let weth_address = get_address(AddressType::Weth);
    let v3_router_address = get_address(AddressType::V3Router);
    let v2_router_address = get_address(AddressType::V2Router);
    let v3_factory_address = get_address(AddressType::V3Factory);
    let v2_factory_address = get_address(AddressType::V2Factory);
    let v2_quoter_address = get_address(AddressType::V2Quoter);

    assert_ne!(weth_address, Address::ZERO, "WETH address should not be zero");
    assert_ne!(v3_router_address, Address::ZERO, "V3Router address should not be zero");
    assert_ne!(v2_router_address, Address::ZERO, "V2Router address should not be zero");
    assert_ne!(v3_factory_address, Address::ZERO, "V3Factory address should not be zero");
    assert_ne!(v2_factory_address, Address::ZERO, "V2Factory address should not be zero");
    assert_ne!(v2_quoter_address, Address::ZERO, "V2Quoter address should not be zero");

    assert_ne!(weth_address, v3_router_address, "WETH and V3Router should have different addresses");
    assert_ne!(v2_router_address, v3_router_address, "V2Router and V3Router should have different addresses");
    assert_ne!(v2_factory_address, v3_factory_address, "V2Factory and V3Factory should have different addresses");

    info!("✅ Contract addresses verified:");
    info!("   WETH:       {:?}", weth_address);
    info!("   V3Router:   {:?}", v3_router_address);
    info!("   V2Router:   {:?}", v2_router_address);
    info!("   V3Factory:  {:?}", v3_factory_address);
    info!("   V2Factory:  {:?}", v2_factory_address);
    info!("   V2Quoter:   {:?}", v2_quoter_address);

    use arbooo::arbitrage::simulation::arboo_bytecode;

    let bytecode = arboo_bytecode();
    assert!(!bytecode.is_empty(), "Contract bytecode should not be empty");

    info!("✅ Contract bytecode available: {} bytes", bytecode.len());

    info!("🎉 EVM Simulator Constants and Addresses Test completed!");
    Ok(())
}

