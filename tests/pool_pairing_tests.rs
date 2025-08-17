// Pool Pairing Logic E2E Tests
// Tests V2 ↔ V3 pool matching and arbitrage pair identification

use anyhow::Result;
use arbooo::common::logger;
use arbooo::arbitrage::simulation::{get_address, AddressType};
use alloy::providers::Provider;
use alloy::primitives::{U256, Address};
use alloy_primitives::aliases::U24;
use log::info;
use std::collections::HashMap;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[derive(Debug, Clone, PartialEq)]
struct MockPool {
    address: Address,
    pool_type: PoolType,
    token0: Address,
    token1: Address,
    fee: U24,
    reserves: Option<(U256, U256)>, // For V2 pools
    liquidity: Option<U256>,        // For V3 pools
}

#[derive(Debug, Clone, PartialEq)]
enum PoolType {
    UniswapV2,
    UniswapV3,
}

#[tokio::test]
async fn test_pool_pairing_structure() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Pool Pairing Structure Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");
    
    // Get latest block for context
    let latest_block_number = test_env.provider.get_block_number().await?;
    info!("📦 Latest block number: {}", latest_block_number);
    
    // Create test tokens
    let weth = get_address(AddressType::Weth);
    let token_a = Address::from([0x11; 20]); // Mock token A
    let token_b = Address::from([0x22; 20]); // Mock token B
    
    // Create mock pools for testing
    let mut test_pools = Vec::new();
    
    // WETH/Token_A pools (arbitrage pair)
    let v2_weth_tokena = MockPool {
        address: Address::from([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc]),
        pool_type: PoolType::UniswapV2,
        token0: weth,
        token1: token_a,
        fee: U24::from(3000), // 0.3% (standardized for comparison)
        reserves: Some((
            U256::from(100) * U256::from(10).pow(U256::from(18)), // 100 WETH
            U256::from(200) * U256::from(10).pow(U256::from(18)), // 200 Token_A
        )),
        liquidity: None,
    };
    
    let v3_weth_tokena = MockPool {
        address: Address::from([0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
        pool_type: PoolType::UniswapV3,
        token0: weth,
        token1: token_a,
        fee: U24::from(3000), // 0.3%
        reserves: None,
        liquidity: Some(U256::from(500) * U256::from(10).pow(U256::from(18))),
    };
    
    test_pools.push(v2_weth_tokena.clone());
    test_pools.push(v3_weth_tokena.clone());
    
    // WETH/Token_B pools (another arbitrage pair)
    let v2_weth_tokenb = MockPool {
        address: Address::from([0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00]),
        pool_type: PoolType::UniswapV2,
        token0: weth,
        token1: token_b,
        fee: U24::from(3000),
        reserves: Some((
            U256::from(150) * U256::from(10).pow(U256::from(18)), // 150 WETH
            U256::from(300) * U256::from(10).pow(U256::from(18)), // 300 Token_B
        )),
        liquidity: None,
    };
    
    let v3_weth_tokenb = MockPool {
        address: Address::from([0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22]),
        pool_type: PoolType::UniswapV3,
        token0: weth,
        token1: token_b,
        fee: U24::from(500), // 0.05% (different fee)
        reserves: None,
        liquidity: Some(U256::from(750) * U256::from(10).pow(U256::from(18))),
    };
    
    test_pools.push(v2_weth_tokenb.clone());
    test_pools.push(v3_weth_tokenb.clone());
    
    // Single pool (no arbitrage opportunity)
    let v2_tokena_tokenb = MockPool {
        address: Address::from([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44]),
        pool_type: PoolType::UniswapV2,
        token0: token_a,
        token1: token_b,
        fee: U24::from(3000),
        reserves: Some((
            U256::from(50) * U256::from(10).pow(U256::from(18)),
            U256::from(75) * U256::from(10).pow(U256::from(18)),
        )),
        liquidity: None,
    };
    
    test_pools.push(v2_tokena_tokenb.clone());
    
    // Test pool structure validation
    for (i, pool) in test_pools.iter().enumerate() {
        // Validate pool has required fields
        assert_ne!(pool.address, Address::ZERO, "Pool {} should have valid address", i);
        assert_ne!(pool.token0, Address::ZERO, "Pool {} should have valid token0", i);
        assert_ne!(pool.token1, Address::ZERO, "Pool {} should have valid token1", i);
        assert_ne!(pool.token0, pool.token1, "Pool {} tokens should be different", i);
        assert!(pool.fee > U24::from(0), "Pool {} should have positive fee", i);
        
        // Type-specific validation
        match pool.pool_type {
            PoolType::UniswapV2 => {
                assert!(pool.reserves.is_some(), "V2 pool {} should have reserves", i);
                assert!(pool.liquidity.is_none(), "V2 pool {} should not have liquidity field", i);
                
                if let Some((reserve0, reserve1)) = pool.reserves {
                    assert!(reserve0 > U256::ZERO, "V2 pool {} reserve0 should be positive", i);
                    assert!(reserve1 > U256::ZERO, "V2 pool {} reserve1 should be positive", i);
                }
            },
            PoolType::UniswapV3 => {
                assert!(pool.reserves.is_none(), "V3 pool {} should not have reserves field", i);
                assert!(pool.liquidity.is_some(), "V3 pool {} should have liquidity", i);
                
                if let Some(liquidity) = pool.liquidity {
                    assert!(liquidity > U256::ZERO, "V3 pool {} liquidity should be positive", i);
                }
            },
        }
        
        info!("✅ Pool {} validated: {:?} for {}-{} pair", 
              i, pool.pool_type, 
              format!("{:?}", pool.token0)[0..6].to_string(),
              format!("{:?}", pool.token1)[0..6].to_string());
    }
    
    info!("✅ Validated {} pools successfully", test_pools.len());
    info!("🎉 Pool Pairing Structure Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_arbitrage_pair_identification() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Arbitrage Pair Identification Test");

    // Create comprehensive test pool set
    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x11; 20]); // Mock USDC
    let dai = Address::from([0x22; 20]);  // Mock DAI
    let wbtc = Address::from([0x33; 20]); // Mock WBTC
    
    let mut pools = Vec::new();
    
    // WETH/USDC arbitrage pair (both V2 and V3)
    pools.push(create_mock_pool(
        Address::from([0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01]), PoolType::UniswapV2, weth, usdc, U24::from(3000),
        Some((U256::from(100_000), U256::from(200_000_000))), None
    ));
    pools.push(create_mock_pool(
        Address::from([0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02]), PoolType::UniswapV3, weth, usdc, U24::from(500),
        None, Some(U256::from(1_000_000))
    ));
    
    // WETH/DAI arbitrage pair
    pools.push(create_mock_pool(
        Address::from([0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03]), PoolType::UniswapV2, weth, dai, U24::from(3000),
        Some((U256::from(75_000), U256::from(150_000_000))), None
    ));
    pools.push(create_mock_pool(
        Address::from([0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]), PoolType::UniswapV3, weth, dai, U24::from(3000),
        None, Some(U256::from(800_000))
    ));
    
    // USDC/DAI (only one version, no arbitrage)
    pools.push(create_mock_pool(
        Address::from([0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05]), PoolType::UniswapV2, usdc, dai, U24::from(3000),
        Some((U256::from(1_000_000), U256::from(1_000_000))), None
    ));
    
    // WETH/WBTC (only V3, no arbitrage)
    pools.push(create_mock_pool(
        Address::from([0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06]), PoolType::UniswapV3, weth, wbtc, U24::from(3000),
        None, Some(U256::from(500_000))
    ));
    
    // Test arbitrage pair identification logic
    let arbitrage_pairs = find_arbitrage_pairs(&pools);
    
    // Validate results
    assert_eq!(arbitrage_pairs.len(), 2, "Should find 2 arbitrage pairs");
    
    // Check WETH/USDC pair
    let weth_usdc_pair = arbitrage_pairs.iter()
        .find(|pair| pair.has_tokens(weth, usdc))
        .expect("Should find WETH/USDC arbitrage pair");
    
    assert_eq!(weth_usdc_pair.v2_pools.len(), 1, "WETH/USDC should have 1 V2 pool");
    assert_eq!(weth_usdc_pair.v3_pools.len(), 1, "WETH/USDC should have 1 V3 pool");
    
    // Check WETH/DAI pair  
    let weth_dai_pair = arbitrage_pairs.iter()
        .find(|pair| pair.has_tokens(weth, dai))
        .expect("Should find WETH/DAI arbitrage pair");
    
    assert_eq!(weth_dai_pair.v2_pools.len(), 1, "WETH/DAI should have 1 V2 pool");
    assert_eq!(weth_dai_pair.v3_pools.len(), 1, "WETH/DAI should have 1 V3 pool");
    
    info!("✅ Found {} arbitrage pairs:", arbitrage_pairs.len());
    for (i, pair) in arbitrage_pairs.iter().enumerate() {
        info!("   Pair {}: {}-{} (V2: {}, V3: {})", 
              i, 
              format!("{:?}", pair.token0)[0..6].to_string(),
              format!("{:?}", pair.token1)[0..6].to_string(),
              pair.v2_pools.len(), 
              pair.v3_pools.len());
    }
    
    info!("🎉 Arbitrage Pair Identification Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_pool_liquidity_filtering() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Pool Liquidity Filtering Test");

    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x11; 20]);
    
    // Create pools with different liquidity levels
    let pools = vec![
        // High liquidity pool (good for arbitrage)
        create_mock_pool(
            Address::from([0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01]), PoolType::UniswapV2, weth, usdc, U24::from(3000),
            Some((
                U256::from(1000) * U256::from(10).pow(U256::from(18)), // 1000 WETH
                U256::from(2_000_000) * U256::from(10).pow(U256::from(6)), // 2M USDC
            )), None
        ),
        // Medium liquidity pool
        create_mock_pool(
            Address::from([0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02]), PoolType::UniswapV3, weth, usdc, U24::from(500),
            None, Some(U256::from(500) * U256::from(10).pow(U256::from(18)))
        ),
        // Low liquidity pool (should be filtered out)
        create_mock_pool(
            Address::from([0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03]), PoolType::UniswapV2, weth, usdc, U24::from(3000),
            Some((
                U256::from(1) * U256::from(10).pow(U256::from(18)), // 1 WETH
                U256::from(2000) * U256::from(10).pow(U256::from(6)), // 2K USDC
            )), None
        ),
        // Very low liquidity pool (should be filtered out)
        create_mock_pool(
            Address::from([0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]), PoolType::UniswapV3, weth, usdc, U24::from(3000),
            None, Some(U256::from(1) * U256::from(10).pow(U256::from(17))) // 0.1 liquidity
        ),
    ];
    
    // Test liquidity filtering
    let min_weth_liquidity = U256::from(10) * U256::from(10).pow(U256::from(18)); // 10 WETH minimum
    let filtered_pools = filter_pools_by_liquidity(&pools, min_weth_liquidity);
    
    // Should keep high and medium liquidity pools, filter out low liquidity
    assert_eq!(filtered_pools.len(), 2, "Should keep 2 pools with sufficient liquidity");
    
    // Verify high liquidity pool is kept
    assert!(filtered_pools.iter().any(|p| p.address == Address::from([0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01])), 
            "High liquidity pool should be kept");
    
    // Verify medium liquidity pool is kept
    assert!(filtered_pools.iter().any(|p| p.address == Address::from([0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02])), 
            "Medium liquidity pool should be kept");
    
    // Verify low liquidity pools are filtered out
    assert!(!filtered_pools.iter().any(|p| p.address == Address::from([0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03])), 
            "Low liquidity pool should be filtered out");
    assert!(!filtered_pools.iter().any(|p| p.address == Address::from([0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04])), 
            "Very low liquidity pool should be filtered out");
    
    info!("✅ Liquidity filtering: {} pools retained from {} total", 
          filtered_pools.len(), pools.len());
    
    for pool in filtered_pools.iter() {
        let liquidity_desc = match pool.pool_type {
            PoolType::UniswapV2 => {
                if let Some((reserve0, reserve1)) = pool.reserves {
                    format!("Reserves: {} / {}", reserve0, reserve1)
                } else {
                    "No reserves".to_string()
                }
            },
            PoolType::UniswapV3 => {
                if let Some(liquidity) = pool.liquidity {
                    format!("Liquidity: {}", liquidity)
                } else {
                    "No liquidity".to_string()
                }
            },
        };
        
        info!("   Pool {:?}: {:?} - {}", 
              pool.address, pool.pool_type, liquidity_desc);
    }
    
    info!("🎉 Pool Liquidity Filtering Test completed!");
    Ok(())
}

// Helper functions
fn create_mock_pool(
    address: Address,
    pool_type: PoolType,
    token0: Address,
    token1: Address,
    fee: U24,
    reserves: Option<(U256, U256)>,
    liquidity: Option<U256>,
) -> MockPool {
    MockPool {
        address,
        pool_type,
        token0,
        token1,
        fee,
        reserves,
        liquidity,
    }
}

#[derive(Debug)]
struct ArbitragePair {
    token0: Address,
    token1: Address,
    v2_pools: Vec<MockPool>,
    v3_pools: Vec<MockPool>,
}

impl ArbitragePair {
    fn has_tokens(&self, token_a: Address, token_b: Address) -> bool {
        (self.token0 == token_a && self.token1 == token_b) ||
        (self.token0 == token_b && self.token1 == token_a)
    }
}

fn find_arbitrage_pairs(pools: &[MockPool]) -> Vec<ArbitragePair> {
    let mut pairs = HashMap::new();
    
    // Group pools by token pair
    for pool in pools {
        let (token0, token1) = if pool.token0 < pool.token1 {
            (pool.token0, pool.token1)
        } else {
            (pool.token1, pool.token0)
        };
        
        let key = (token0, token1);
        pairs.entry(key).or_insert_with(Vec::new).push(pool.clone());
    }
    
    // Find pairs that have both V2 and V3 pools
    let mut arbitrage_pairs = Vec::new();
    
    for ((token0, token1), pool_group) in pairs {
        let v2_pools: Vec<_> = pool_group.iter()
            .filter(|p| matches!(p.pool_type, PoolType::UniswapV2))
            .cloned()
            .collect();
        
        let v3_pools: Vec<_> = pool_group.iter()
            .filter(|p| matches!(p.pool_type, PoolType::UniswapV3))
            .cloned()
            .collect();
        
        // Only include if we have both V2 and V3 pools
        if !v2_pools.is_empty() && !v3_pools.is_empty() {
            arbitrage_pairs.push(ArbitragePair {
                token0,
                token1,
                v2_pools,
                v3_pools,
            });
        }
    }
    
    arbitrage_pairs
}

fn filter_pools_by_liquidity(pools: &[MockPool], min_liquidity: U256) -> Vec<MockPool> {
    pools.iter()
        .filter(|pool| {
            match pool.pool_type {
                PoolType::UniswapV2 => {
                    if let Some((reserve0, reserve1)) = pool.reserves {
                        // For V2, check if either reserve meets minimum
                        reserve0 >= min_liquidity || reserve1 >= min_liquidity
                    } else {
                        false
                    }
                },
                PoolType::UniswapV3 => {
                    if let Some(liquidity) = pool.liquidity {
                        liquidity >= min_liquidity
                    } else {
                        false
                    }
                },
            }
        })
        .cloned()
        .collect()
}
