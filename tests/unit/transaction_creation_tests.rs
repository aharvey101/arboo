use anyhow::Result;
use arbooo::common::logger;
use arbooo::common::revm::{Tx, VictimTx};
use arbooo::arbitrage::simulation::{get_address, AddressType};
use alloy::providers::Provider;
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::U256;
use revm::primitives::{Bytes, B256};
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_basic_transaction_creation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Basic Transaction Creation Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");

    let latest_block_number = test_env.provider.get_block_number().await?;
    info!("📦 Latest block number: {}", latest_block_number);

    let sender = PrivateKeySigner::random().address();
    let recipient = PrivateKeySigner::random().address();

    let eth_transfer_tx = Tx {
        caller: sender,
        transact_to: recipient,
        data: Bytes::new(),
        value: U256::from(1) * U256::from(10).pow(U256::from(18)),
        gas_price: U256::from(20_000_000_000u128),
        gas_limit: 21_000,
    };

    assert_eq!(eth_transfer_tx.caller, sender);
    assert_eq!(eth_transfer_tx.transact_to, recipient);
    assert_eq!(eth_transfer_tx.value, U256::from(1) * U256::from(10).pow(U256::from(18)));
    assert_eq!(eth_transfer_tx.gas_limit, 21_000);
    assert!(eth_transfer_tx.data.is_empty());

    info!("✅ ETH transfer transaction created: {} ETH from {:?} to {:?}", 
          eth_transfer_tx.value / U256::from(10).pow(U256::from(18)), 
          eth_transfer_tx.caller, 
          eth_transfer_tx.transact_to);

    let weth_address = get_address(AddressType::Weth);

    let deposit_data = Bytes::from(hex::decode("d0e30db0").unwrap());

    let weth_deposit_tx = Tx {
        caller: sender,
        transact_to: weth_address,
        data: deposit_data.clone(),
        value: U256::from(5) * U256::from(10).pow(U256::from(17)),
        gas_price: U256::from(25_000_000_000u128),
        gas_limit: 50_000,
    };

    assert_eq!(weth_deposit_tx.caller, sender);
    assert_eq!(weth_deposit_tx.transact_to, weth_address);
    assert_eq!(weth_deposit_tx.value, U256::from(5) * U256::from(10).pow(U256::from(17)));
    assert_eq!(weth_deposit_tx.gas_limit, 50_000);
    assert_eq!(weth_deposit_tx.data, deposit_data);

    info!("✅ WETH deposit transaction created: {} ETH to {:?}", 
          weth_deposit_tx.value / U256::from(10).pow(U256::from(18)), 
          weth_deposit_tx.transact_to);

    let high_value_tx = Tx {
        caller: sender,
        transact_to: recipient,
        data: Bytes::new(),
        value: U256::from(100) * U256::from(10).pow(U256::from(18)),
        gas_price: U256::from(100_000_000_000u128),
        gas_limit: 21_000,
    };

    assert_eq!(high_value_tx.value, U256::from(100) * U256::from(10).pow(U256::from(18)));
    assert_eq!(high_value_tx.gas_price, U256::from(100_000_000_000u128));

    info!("✅ High-value transaction created: {} ETH with {} gwei gas price", 
          high_value_tx.value / U256::from(10).pow(U256::from(18)),
          high_value_tx.gas_price / U256::from(1_000_000_000u128));

    info!("🎉 Basic Transaction Creation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_victim_transaction_parsing() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Victim Transaction Parsing Test");

    let test_addresses = vec![
        PrivateKeySigner::random().address(),
        PrivateKeySigner::random().address(),
        get_address(AddressType::V3Router),
        get_address(AddressType::Weth),
    ];

    let victim_transactions = vec![

        VictimTx {
            tx_hash: B256::from([1u8; 32]),
            from: test_addresses[0],
            to: test_addresses[1],
            data: Bytes::new(),
            value: U256::from(2) * U256::from(10).pow(U256::from(18)),
            gas_price: U256::from(30_000_000_000u128),
            gas_limit: Some(21_000),
        },

        VictimTx {
            tx_hash: B256::from([2u8; 32]),
            from: test_addresses[0],
            to: test_addresses[2],
            data: Bytes::from(vec![0xa4, 0x15, 0x84, 0xc2]),
            value: U256::ZERO,
            gas_price: U256::from(45_000_000_000u128),
            gas_limit: Some(200_000),
        },

        VictimTx {
            tx_hash: B256::from([3u8; 32]),
            from: test_addresses[1],
            to: test_addresses[3],
            data: Bytes::from(hex::decode("2e1a7d4d").unwrap()),
            value: U256::ZERO,
            gas_price: U256::from(20_000_000_000u128),
            gas_limit: None,
        },
    ];

    for (i, victim_tx) in victim_transactions.iter().enumerate() {
        let converted_tx = Tx::from(victim_tx.clone());

        assert_eq!(converted_tx.caller, victim_tx.from);
        assert_eq!(converted_tx.transact_to, victim_tx.to);
        assert_eq!(converted_tx.data, victim_tx.data);
        assert_eq!(converted_tx.value, victim_tx.value);
        assert_eq!(converted_tx.gas_price, victim_tx.gas_price);

        let expected_gas_limit = victim_tx.gas_limit.unwrap_or(5_000_000);
        assert_eq!(converted_tx.gas_limit, expected_gas_limit);

        info!("✅ Victim transaction {} converted: hash={:?}, gas_limit={}", 
              i, victim_tx.tx_hash, converted_tx.gas_limit);
    }

    let unique_hashes: std::collections::HashSet<_> = victim_transactions
        .iter()
        .map(|tx| tx.tx_hash)
        .collect();

    assert_eq!(unique_hashes.len(), victim_transactions.len(), 
               "All transaction hashes should be unique");

    info!("✅ All victim transactions have unique hashes");
    info!("🎉 Victim Transaction Parsing Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_arbitrage_transaction_structure() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Arbitrage Transaction Structure Test");

    let bot_address = PrivateKeySigner::random().address();
    let v2_router = get_address(AddressType::V2Router);
    let v3_router = get_address(AddressType::V3Router);
    let weth_address = get_address(AddressType::Weth);

    let weth_unwrap_tx = Tx {
        caller: bot_address,
        transact_to: weth_address,
        data: Bytes::from(hex::decode("2e1a7d4d").unwrap()),
        value: U256::ZERO,
        gas_price: U256::from(50_000_000_000u128),
        gas_limit: 60_000,
    };

    let v2_swap_tx = Tx {
        caller: bot_address,
        transact_to: v2_router,
        data: Bytes::from(hex::decode("38ed1739").unwrap()),
        value: U256::ZERO,
        gas_price: U256::from(50_000_000_000u128),
        gas_limit: 150_000,
    };

    let v3_swap_tx = Tx {
        caller: bot_address,
        transact_to: v3_router,
        data: Bytes::from(hex::decode("414bf389").unwrap()),
        value: U256::ZERO,
        gas_price: U256::from(50_000_000_000u128),
        gas_limit: 200_000,
    };

    let transactions = vec![&weth_unwrap_tx, &v2_swap_tx, &v3_swap_tx];
    for tx in &transactions {
        assert_eq!(tx.caller, bot_address);
        assert_eq!(tx.gas_price, U256::from(50_000_000_000u128));
        assert_eq!(tx.value, U256::ZERO);
    }

    assert_eq!(weth_unwrap_tx.transact_to, weth_address);
    assert_eq!(v2_swap_tx.transact_to, v2_router);
    assert_eq!(v3_swap_tx.transact_to, v3_router);

    assert!(weth_unwrap_tx.gas_limit < v2_swap_tx.gas_limit);
    assert!(v2_swap_tx.gas_limit < v3_swap_tx.gas_limit);

    info!("✅ WETH unwrap transaction: gas_limit={}", weth_unwrap_tx.gas_limit);
    info!("✅ V2 swap transaction: gas_limit={}", v2_swap_tx.gas_limit);
    info!("✅ V3 swap transaction: gas_limit={}", v3_swap_tx.gas_limit);

    let total_gas_cost = transactions.iter()
        .map(|tx| U256::from(tx.gas_limit) * tx.gas_price)
        .sum::<U256>();

    info!("✅ Total arbitrage gas cost: {} wei ({} ETH)", 
          total_gas_cost, 
          total_gas_cost / U256::from(10).pow(U256::from(18)));

    let max_reasonable_cost = U256::from(10).pow(U256::from(17));
    assert!(total_gas_cost < max_reasonable_cost, 
            "Total gas cost should be reasonable for arbitrage");

    info!("🎉 Arbitrage Transaction Structure Test completed!");
    Ok(())
}

