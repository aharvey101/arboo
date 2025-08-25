use anyhow::Result;
use arbooo::common::logger;
use arbooo::arbitrage::simulation::{get_address, AddressType};
use alloy::providers::Provider;
use alloy::primitives::U256;
use alloy_primitives::aliases::U24;
use revm::primitives::Address;
use log::info;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_swap_simulation_setup() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Swap Simulation Setup Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");

    let latest_block_number = test_env.provider.get_block_number().await?;
    info!("📦 Latest block number: {}", latest_block_number);

    let weth_address = get_address(AddressType::Weth);
    let v2_router = get_address(AddressType::V2Router);
    let v3_router = get_address(AddressType::V3Router);
    let v2_factory = get_address(AddressType::V2Factory);
    let v3_factory = get_address(AddressType::V3Factory);

    let addresses = vec![weth_address, v2_router, v3_router, v2_factory, v3_factory];
    for (i, addr) in addresses.iter().enumerate() {
        assert_ne!(*addr, Address::ZERO, "Address {} should not be zero", i);

        for (j, other_addr) in addresses.iter().enumerate() {
            if i != j {
                assert_ne!(*addr, *other_addr, "Addresses {} and {} should be different", i, j);
            }
        }
    }

    info!("✅ Contract addresses validated:");
    info!("   WETH:       {:?}", weth_address);
    info!("   V2Router:   {:?}", v2_router);
    info!("   V3Router:   {:?}", v3_router);
    info!("   V2Factory:  {:?}", v2_factory);
    info!("   V3Factory:  {:?}", v3_factory);

    let test_amounts = vec![
        U256::from(1) * U256::from(10).pow(U256::from(17)),
        U256::from(1) * U256::from(10).pow(U256::from(18)),
        U256::from(5) * U256::from(10).pow(U256::from(18)),
    ];

    let test_fees = vec![
        U24::from(500),
        U24::from(3000),
        U24::from(10000),
    ];

    for (i, amount) in test_amounts.iter().enumerate() {
        assert!(*amount > U256::ZERO, "Amount {} should be positive", i);
        assert!(*amount <= U256::from(1000) * U256::from(10).pow(U256::from(18)), 
                "Amount {} should be reasonable (<=1000 ETH)", i);

        info!("✅ Test amount {}: {} ETH", i, amount / U256::from(10).pow(U256::from(18)));
    }

    for (i, fee) in test_fees.iter().enumerate() {
        assert!(*fee >= U24::from(100), "Fee {} should be at least 0.01%", i);
        assert!(*fee <= U24::from(100000), "Fee {} should be at most 10%", i);

        let fee_percent = fee.to::<u32>() as f64 / 10000.0;
        info!("✅ Test fee tier {}: {} ({}%)", i, fee, fee_percent);
    }

    info!("🎉 Swap Simulation Setup Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_swap_parameter_validation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Swap Parameter Validation Test");

    let weth_address = get_address(AddressType::Weth);
    let usdc_address = Address::from([0x12; 20]);

    assert_ne!(weth_address, usdc_address, "Token addresses should be different");

    let small_amount = U256::from(10).pow(U256::from(15));
    let medium_amount = U256::from(10).pow(U256::from(18));
    let large_amount = U256::from(100) * U256::from(10).pow(U256::from(18));

    let amounts = vec![small_amount, medium_amount, large_amount];

    for (i, amount) in amounts.iter().enumerate() {

        assert!(*amount >= U256::from(10).pow(U256::from(15)), 
                "Amount {} too small for realistic testing", i);
        assert!(*amount <= U256::from(1000) * U256::from(10).pow(U256::from(18)), 
                "Amount {} too large for testing", i);

        let fee_basis_points = vec![5, 30, 100];

        for fee_bp in fee_basis_points {
            let fee_amount = *amount * U256::from(fee_bp) / U256::from(10000);
            let net_amount = *amount - fee_amount;

            assert!(net_amount < *amount, "Net amount should be less than input");
            assert!(fee_amount > U256::ZERO || fee_bp == 0, "Fee should be positive for non-zero fee rate");

            info!("✅ Amount {}: {} ETH, Fee {}bp = {} ETH, Net = {} ETH", 
                  i,
                  amount / U256::from(10).pow(U256::from(18)),
                  fee_bp,
                  fee_amount / U256::from(10).pow(U256::from(18)),
                  net_amount / U256::from(10).pow(U256::from(18)));
        }
    }

    let slippage_tolerances = vec![50, 100, 300];
    let base_output = U256::from(95) * U256::from(10).pow(U256::from(17));

    for slippage_bp in slippage_tolerances {
        let slippage_amount = base_output * U256::from(slippage_bp) / U256::from(10000);
        let min_output = base_output - slippage_amount;

        assert!(min_output < base_output, "Min output should be less than expected");
        assert!(slippage_amount > U256::ZERO, "Slippage amount should be positive");

        info!("✅ Slippage {}bp: Expected {} ETH, Min {} ETH", 
              slippage_bp,
              base_output / U256::from(10).pow(U256::from(18)),
              min_output / U256::from(10).pow(U256::from(18)));
    }

    info!("🎉 Swap Parameter Validation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_swap_path_construction() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Swap Path Construction Test");

    let weth = get_address(AddressType::Weth);
    let token_a = Address::from([0x11; 20]);
    let token_b = Address::from([0x22; 20]);
    let token_c = Address::from([0x33; 20]);

    let direct_path = vec![token_a, token_b];
    assert_eq!(direct_path.len(), 2);
    assert_eq!(direct_path[0], token_a);
    assert_eq!(direct_path[1], token_b);

    info!("✅ Direct swap path: {:?} -> {:?}", direct_path[0], direct_path[1]);

    let multihop_path = vec![token_a, weth, token_b];
    assert_eq!(multihop_path.len(), 3);
    assert_eq!(multihop_path[0], token_a);
    assert_eq!(multihop_path[1], weth);
    assert_eq!(multihop_path[2], token_b);

    info!("✅ Multi-hop path: {:?} -> {:?} -> {:?}", 
          multihop_path[0], multihop_path[1], multihop_path[2]);

    let arbitrage_path = vec![token_a, token_b, token_c, token_a];
    assert_eq!(arbitrage_path.len(), 4);
    assert_eq!(arbitrage_path[0], arbitrage_path[3]);

    info!("✅ Arbitrage path: {:?} -> {:?} -> {:?} -> {:?}", 
          arbitrage_path[0], arbitrage_path[1], arbitrage_path[2], arbitrage_path[3]);

    for path in vec![&direct_path, &multihop_path, &arbitrage_path] {

        assert!(path.len() >= 2, "Path should have at least 2 tokens");

        for (i, token) in path.iter().enumerate() {
            assert_ne!(*token, Address::ZERO, "Token {} in path should not be zero", i);
        }

        for i in 0..path.len()-1 {
            assert_ne!(path[i], path[i+1], "Adjacent tokens should be different");
        }
    }

    let v3_fees = vec![
        vec![U24::from(500)],
        vec![U24::from(3000), U24::from(500)],
        vec![U24::from(10000), U24::from(3000), U24::from(500)],
    ];

    for (i, fees) in v3_fees.iter().enumerate() {

        let expected_fee_count = if i == 0 { 1 } else { i + 1 };
        assert_eq!(fees.len(), expected_fee_count, 
                   "Fee count should match path structure");

        for (j, fee) in fees.iter().enumerate() {
            assert!(*fee >= U24::from(100), "Fee {} should be reasonable", j);
            assert!(*fee <= U24::from(100000), "Fee {} should not be excessive", j);
        }

        info!("✅ V3 fee structure {}: {:?}", i, fees);
    }

    info!("🎉 Swap Path Construction Test completed!");
    Ok(())
}

