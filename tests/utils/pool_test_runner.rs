#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use anyhow::Result;
use log::info;

pub async fn run_pool_data_structure_tests() -> Result<()> {
    use std::process::Command;

    info!("📊 Running pool data structure tests");

    let output = Command::new("cargo")
        .args(&["test", "test_pool_data_structures", "--test", "pool_data_tests", "--", "--nocapture"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run test: {}", e))?;

    if output.status.success() {
        info!("✅ Pool data structure test passed");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(anyhow::anyhow!("Test failed: {}\nStdout: {}", stderr, stdout))
    }
}

pub async fn run_pool_cache_tests() -> Result<()> {
    use std::process::Command;

    info!("💾 Running pool cache tests");

    let tests = vec![
        "test_pool_cache_creation",
        "test_pool_cache_file_operations"
    ];

    for test_name in tests {
        info!("  🧪 Running {}", test_name);
        let output = Command::new("cargo")
            .args(&["test", test_name, "--test", "pool_data_tests", "--", "--nocapture"])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run test {}: {}", test_name, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!("Test {} failed: {}\nStdout: {}", test_name, stderr, stdout));
        }
    }

    info!("✅ All pool cache tests passed");
    Ok(())
}

pub async fn run_pool_pairing_tests() -> Result<()> {
    use std::process::Command;

    info!("🔗 Running pool pairing tests");

    let tests = vec![
        "test_pool_pairing_structure",
        "test_arbitrage_pair_identification"
    ];

    for test_name in tests {
        info!("  🧪 Running {}", test_name);
        let output = Command::new("cargo")
            .args(&["test", test_name, "--test", "pool_pairing_tests", "--", "--nocapture"])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run test {}: {}", test_name, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!("Test {} failed: {}\nStdout: {}", test_name, stderr, stdout));
        }
    }

    info!("✅ All pool pairing tests passed");
    Ok(())
}

pub async fn run_pool_discovery_tests() -> Result<()> {
    info!("🔍 Running pool discovery infrastructure tests");

    match test_pool_discovery_infrastructure().await {
        Ok(_) => {
            info!("✅ Pool discovery infrastructure tests passed");
            Ok(())
        }
        Err(e) => {
            Err(anyhow::anyhow!("Pool discovery infrastructure test failed: {}", e))
        }
    }
}

async fn test_pool_discovery_infrastructure() -> Result<()> {
    use super::integrated_test_env::{IntegratedTestEnvironment, TestEnvironmentConfig};
    use arbooo::common::pools::{Pool, DexVariant};
    use alloy::providers::Provider;
    use std::collections::HashMap;

    info!("  🏗️  Setting up test environment for pool discovery...");

    let config = TestEnvironmentConfig {
        mainnet_fork_url: "https://rpc.ankr.com/eth".to_string(),
        private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
        websocket_port: None,
        enable_logging: false,
        gas_limit: 21000,
        gas_price: 20_000_000_000,
    };

    let test_env = IntegratedTestEnvironment::new(config).await?;
    let provider = test_env.provider();

    info!("  📡 Testing provider connectivity...");
    let latest_block = provider.get_block_number().await?;
    info!("    ✅ Connected to provider, latest block: {}", latest_block);

    info!("  📊 Testing pool data structures...");
    let test_pools = create_test_pool_set();

    for (i, pool) in test_pools.iter().enumerate() {

        if pool.token0 == pool.token1 {
            return Err(anyhow::anyhow!("Pool {}: tokens should be different", i));
        }

        if pool.fee == 0 && pool.version == DexVariant::UniswapV3 {
            return Err(anyhow::anyhow!("Pool {}: V3 pools should have non-zero fee", i));
        }

        let trades_correctly = pool.trades(pool.token0, pool.token1) && 
                              pool.trades(pool.token1, pool.token0);
        if !trades_correctly {
            return Err(anyhow::anyhow!("Pool {}: trades() method not working correctly", i));
        }

        let cache_row = pool.cache_row();
        if cache_row.0 != pool.id {
            return Err(anyhow::anyhow!("Pool {}: cache serialization incorrect", i));
        }
    }

    info!("    ✅ Validated {} test pools", test_pools.len());

    info!("  🔗 Testing pool pairing logic...");
    let arbitrage_pairs = identify_arbitrage_pairs(&test_pools);
    info!("    ✅ Found {} potential arbitrage pairs", arbitrage_pairs.len());

    info!("  💾 Testing memory usage...");
    let pool_map: HashMap<alloy::primitives::Address, Pool> = test_pools
        .iter()
        .map(|pool| (pool.address, *pool))
        .collect();

    if pool_map.len() != test_pools.len() {
        return Err(anyhow::anyhow!("Pool map creation failed: duplicate addresses"));
    }

    info!("    ✅ Pool map created successfully with {} entries", pool_map.len());

    info!("  🔍 Testing pool filtering...");
    let v2_pools: Vec<_> = test_pools
        .iter()
        .filter(|pool| pool.version == DexVariant::UniswapV2)
        .collect();

    let v3_pools: Vec<_> = test_pools
        .iter()
        .filter(|pool| pool.version == DexVariant::UniswapV3)
        .collect();

    info!("    ✅ Filtered {} V2 pools and {} V3 pools", v2_pools.len(), v3_pools.len());

    test_env.cleanup().await?;

    Ok(())
}

fn create_test_pool_set() -> Vec<arbooo::common::pools::Pool> {
    use arbooo::common::pools::{Pool, DexVariant};
    use arbooo::arbitrage::simulation::{get_address, AddressType};

    let weth = get_address(AddressType::Weth);
    let dai = alloy::primitives::address!("6B175474E89094C44Da98b954EedeAC495271d0F");
    let usdc = alloy::primitives::address!("A0b86a33E6441c8D4c2f544d5f4e2dE6A2B3f6d7");
    let wbtc = alloy::primitives::address!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");

    vec![

        Pool {
            id: 1,
            address: alloy::primitives::address!("A478c2975Ab1Ea89e8196811F51A7B7Ade33eB11"),
            version: DexVariant::UniswapV2,
            token0: weth,
            token1: dai,
            fee: 3000,
        },
        Pool {
            id: 2,
            address: alloy::primitives::address!("C36442b4a4522E871399CD717aBDD847Ab11FE88"),
            version: DexVariant::UniswapV3,
            token0: weth,
            token1: dai,
            fee: 3000,
        },

        Pool {
            id: 3,
            address: alloy::primitives::address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            version: DexVariant::UniswapV2,
            token0: weth,
            token1: usdc,
            fee: 3000,
        },
        Pool {
            id: 4,
            address: alloy::primitives::address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"),
            version: DexVariant::UniswapV3,
            token0: weth,
            token1: usdc,
            fee: 500,
        },

        Pool {
            id: 5,
            address: alloy::primitives::address!("4585FE77225b41b697C938B018E2Ac67Ac5a20c0"),
            version: DexVariant::UniswapV3,
            token0: wbtc,
            token1: weth,
            fee: 3000,
        },

        Pool {
            id: 6,
            address: alloy::primitives::address!("AE461cA67B15dc8dc81CE7615e0320dA1A9aB8D5"),
            version: DexVariant::UniswapV2,
            token0: dai,
            token1: usdc,
            fee: 3000,
        },
    ]
}

fn identify_arbitrage_pairs(pools: &[arbooo::common::pools::Pool]) -> Vec<(arbooo::common::pools::Pool, arbooo::common::pools::Pool)> {
    use std::collections::HashMap;
    use arbooo::common::pools::Pool;

    let mut token_pairs: HashMap<(alloy::primitives::Address, alloy::primitives::Address), Vec<Pool>> = HashMap::new();

    for pool in pools {
        let (token0, token1) = if pool.token0 < pool.token1 {
            (pool.token0, pool.token1)
        } else {
            (pool.token1, pool.token0)
        };

        token_pairs.entry((token0, token1)).or_default().push(*pool);
    }

    let mut arbitrage_pairs = Vec::new();

    for (_, pool_group) in token_pairs {
        if pool_group.len() >= 2 {

            for i in 0..pool_group.len() {
                for j in (i + 1)..pool_group.len() {
                    let pool1 = pool_group[i];
                    let pool2 = pool_group[j];

                    if pool1.version != pool2.version {
                        arbitrage_pairs.push((pool1, pool2));
                    }
                }
            }
        }
    }

    arbitrage_pairs
}

