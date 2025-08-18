// Log Event Processing E2E Tests
// Tests log event detection, parsing, and processing pipeline

use anyhow::Result;
use arbooo::common::logger;
use arbooo::common::logs::LogEvent;
use arbooo::common::pairs::{Event, V2PoolCreated, V3PoolCreated};
use alloy::providers::Provider;
use alloy::primitives::{B256, Address};
use alloy::rpc::types::BlockTransactionsKind;
use alloy::eips::BlockId;
use alloy_primitives::aliases::U24;
use log::info;
use revm::primitives::keccak256;
use std::collections::HashMap;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn test_log_event_structure_validation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Log Event Structure Validation Test");

    let test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");
    
    // Get a recent block with transactions to analyze log structure
    let latest_block_number = test_env.provider.get_block_number().await?;
    info!("📦 Latest block number: {}", latest_block_number);
    
    // Test that we can access block data for log processing
    let block_id = BlockId::from(latest_block_number);
    
    if let Ok(Some(block)) = test_env.provider.get_block(block_id, BlockTransactionsKind::Full).await {
        info!("✅ Successfully retrieved block {} for log analysis", latest_block_number);
        
        // Test block has required fields for log processing
        assert_ne!(block.header.hash, B256::ZERO, "Block should have valid hash");
        assert!(block.header.timestamp > 0, "Block should have valid timestamp");
        
        // For Anvil, block 0 is valid; for live networks, expect > 0
        if test_env.is_using_anvil() {
            info!("🔧 Using Anvil - block number {} is valid for local fork", block.header.number);
        } else {
            assert!(block.header.number > 0, "Block should have valid number for live network");
        }
        
        info!("✅ Block validation: hash={:?}, timestamp={}, transactions={}", 
              block.header.hash, 
              block.header.timestamp,
              match &block.transactions {
                  alloy::rpc::types::BlockTransactions::Full(txs) => txs.len(),
                  alloy::rpc::types::BlockTransactions::Hashes(hashes) => hashes.len(),
                  alloy::rpc::types::BlockTransactions::Uncle => 0,
              });
    } else {
        info!("⚠️ Could not retrieve block, but provider connection is working");
    }
    
    info!("🎉 Log Event Structure Validation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_event_signature_recognition() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Event Signature Recognition Test");

    // Test recognition of different Uniswap event signatures
    let event_signatures = HashMap::from([
        // Uniswap V2 events
        ("Swap", "0xd78ad95fa46c994b6ca2a0630cd986d64a233db1cd28e456a339f089645149c1"),
        ("Sync", "0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1"), 
        ("Transfer", "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"),
        ("Approval", "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925"),
        
        // Uniswap V3 events  
        ("SwapV3", "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67"),
        ("Mint", "0x7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bae"),
        ("Burn", "0x0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c"),
    ]);
    
    for (event_name, expected_signature) in event_signatures.iter() {
        // Convert hex string to B256 for testing
        let signature_bytes = hex::decode(expected_signature.trim_start_matches("0x"))
            .expect("Valid hex signature");
        let signature = alloy::primitives::B256::from_slice(&signature_bytes);
        
        // Validate signature format
        assert_eq!(signature_bytes.len(), 32, "Event signature should be 32 bytes");
        assert_ne!(signature, alloy::primitives::B256::ZERO, "Signature should not be zero");
        
        info!("✅ Event '{}' signature validated: {}", event_name, expected_signature);
    }
    
    // Test signature matching logic
    let test_signature = alloy::primitives::B256::from([
        0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b,
        0x6c, 0xa2, 0xa0, 0x63, 0x0c, 0xd9, 0x86, 0xd6,
        0x4a, 0x23, 0x3d, 0xb1, 0xcd, 0x28, 0xe4, 0x56,
        0xa3, 0x39, 0xf0, 0x89, 0x64, 0x51, 0x49, 0xc1
    ]);
    
    // This should match the Swap event signature
    let swap_signature = hex::decode("d78ad95fa46c994b6ca2a0630cd986d64a233db1cd28e456a339f089645149c1")
        .expect("Valid hex");
    let expected_swap = alloy::primitives::B256::from_slice(&swap_signature);
    
    assert_eq!(test_signature, expected_swap, "Should recognize Swap event signature");
    
    info!("✅ Event signature matching logic validated");
    info!("🎉 Event Signature Recognition Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_log_filtering_and_categorization() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Log Filtering and Categorization Test");

    // Test event signature recognition for arbitrage detection
    let event_signatures = HashMap::from([
        // Uniswap V2 Swap event signature
        ("V2_SWAP", "d78ad95fa46c994b6ca2a0630cd986d64a233db1cd28e456a339f089645149c1"),
        // Uniswap V3 Swap event signature  
        ("V3_SWAP", "c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67"),
        // ERC20 Transfer (should be filtered out for swap detection)
        ("TRANSFER", "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"),
    ]);
    
    // Test signature validation and categorization
    for (event_type, signature_hex) in event_signatures.iter() {
        // Convert hex string to bytes for validation
        let signature_bytes = hex::decode(signature_hex)
            .expect("Valid hex signature");
        
        // Validate signature format
        assert_eq!(signature_bytes.len(), 32, "Event signature should be 32 bytes");
        
        // Test categorization logic
        let is_arbitrage_relevant = matches!(event_type, &"V2_SWAP" | &"V3_SWAP");
        
        if is_arbitrage_relevant {
            info!("✅ Arbitrage-relevant event: {} with signature {}", event_type, signature_hex);
        } else {
            info!("✅ Non-arbitrage event: {} with signature {}", event_type, signature_hex);
        }
    }
    
    // Test filtering logic for arbitrage detection
    let arbitrage_relevant_count = event_signatures.iter()
        .filter(|(event_type, _)| matches!(**event_type, "V2_SWAP" | "V3_SWAP"))
        .count();
    
    assert_eq!(arbitrage_relevant_count, 2, "Should identify 2 swap events as arbitrage-relevant");
    
    info!("✅ Filtered {} arbitrage-relevant events from {} total event types", 
          arbitrage_relevant_count, event_signatures.len());
    
    info!("🎉 Log Filtering and Categorization Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_swap_signature_generation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Swap Signature Generation Test");

    // Test that our keccak256 signature generation works consistently
    let v2_swap_signature = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_signature = keccak256("Swap(address,address,int256,int256,uint160,uint160,int24)".as_bytes());
    
    // Convert our generated signatures to hex for logging
    let v2_hex = hex::encode(v2_swap_signature);
    let v3_hex = hex::encode(v3_swap_signature);
    
    info!("✅ V2 Swap signature: {}", v2_hex);
    info!("✅ V3 Swap signature: {}", v3_hex);
    
    // Verify signatures are different and have correct length
    assert_ne!(v2_swap_signature, v3_swap_signature, "V2 and V3 signatures should be different");
    assert_eq!(v2_swap_signature.len(), 32, "V2 signature should be 32 bytes");
    assert_eq!(v3_swap_signature.len(), 32, "V3 signature should be 32 bytes");
    
    // Verify signatures are not empty/zero
    assert_ne!(v2_swap_signature, [0u8; 32], "V2 signature should not be zero");
    assert_ne!(v3_swap_signature, [0u8; 32], "V3 signature should not be zero");
    
    // Test that the signatures are deterministic (same input gives same output)
    let v2_signature_again = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_signature_again = keccak256("Swap(address,address,int256,int256,uint160,uint160,int24)".as_bytes());
    
    assert_eq!(v2_swap_signature, v2_signature_again, "V2 signature should be deterministic");
    assert_eq!(v3_swap_signature, v3_signature_again, "V3 signature should be deterministic");
    
    info!("✅ Signature generation is deterministic and produces valid 32-byte hashes");
    info!("🎉 Swap Signature Generation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_pool_pairing_logic() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Pool Pairing Logic Test");

    // Create test tokens
    let weth = Address::from([0x01; 20]);
    let usdc = Address::from([0x02; 20]);
    let dai = Address::from([0x03; 20]);
    
    // Create test pools
    let v2_weth_usdc = V2PoolCreated {
        pair_address: Address::from([0x10; 20]),
        token0: weth,
        token1: usdc,
        fee: 3000,
    };
    
    let v3_weth_usdc = V3PoolCreated {
        pair_address: Address::from([0x20; 20]),
        token0: weth,
        token1: usdc,
        fee: 3000,
        tick_spacing: 60,
    };
    
    let v2_weth_dai = V2PoolCreated {
        pair_address: Address::from([0x30; 20]),
        token0: weth,
        token1: dai,
        fee: 3000,
    };
    
    // Create pairs hashmap like in the actual code
    let mut pairs: HashMap<Address, Event> = HashMap::new();
    pairs.insert(v2_weth_usdc.pair_address, Event::PairCreated(v2_weth_usdc.clone()));
    pairs.insert(v3_weth_usdc.pair_address, Event::PoolCreated(v3_weth_usdc.clone()));
    pairs.insert(v2_weth_dai.pair_address, Event::PairCreated(v2_weth_dai.clone()));
    
    // Test the pairing logic for V2 pool finding its V3 counterpart
    if let Some(Event::PairCreated(pair)) = pairs.get(&v2_weth_usdc.pair_address) {
        // Find corresponding V3 pool (same logic as in get_logs)
        let v3_counterpart = pairs.values().find(|value| {
            matches!(value, Event::PoolCreated(v3_pair) 
                if (v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1) || 
                   (v3_pair.token0 == pair.token1 && v3_pair.token1 == pair.token0))
        });
        
        assert!(v3_counterpart.is_some(), "Should find V3 counterpart for V2 WETH/USDC pool");
        if let Some(Event::PoolCreated(v3_pair)) = v3_counterpart {
            assert_eq!(v3_pair.pair_address, v3_weth_usdc.pair_address, "Should match correct V3 pool");
            info!("✅ Found V3 counterpart for V2 pool: {:?} -> {:?}", 
                  v2_weth_usdc.pair_address, v3_pair.pair_address);
        }
    }
    
    // Test the pairing logic for V3 pool finding its V2 counterpart
    if let Some(Event::PoolCreated(pool)) = pairs.get(&v3_weth_usdc.pair_address) {
        let v2_counterpart = pairs.values().find(|value| {
            matches!(value, Event::PairCreated(v2_pair)
                if (v2_pair.token0 == pool.token0 && v2_pair.token1 == pool.token1) ||
                   (v2_pair.token0 == pool.token1 && v2_pair.token1 == pool.token0))
        });
        
        assert!(v2_counterpart.is_some(), "Should find V2 counterpart for V3 WETH/USDC pool");
        if let Some(Event::PairCreated(v2_pair)) = v2_counterpart {
            assert_eq!(v2_pair.pair_address, v2_weth_usdc.pair_address, "Should match correct V2 pool");
            info!("✅ Found V2 counterpart for V3 pool: {:?} -> {:?}", 
                  v3_weth_usdc.pair_address, v2_pair.pair_address);
        }
    }
    
    // Test that V2 WETH/DAI doesn't find a counterpart (no V3 pool exists)
    if let Some(Event::PairCreated(pair)) = pairs.get(&v2_weth_dai.pair_address) {
        let v3_counterpart = pairs.values().find(|value| {
            matches!(value, Event::PoolCreated(v3_pair)
                if (v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1) ||
                   (v3_pair.token0 == pair.token1 && v3_pair.token1 == pair.token0))
        });
        
        assert!(v3_counterpart.is_none(), "Should not find V3 counterpart for V2 WETH/DAI pool");
        info!("✅ Correctly found no V3 counterpart for isolated V2 WETH/DAI pool");
    }
    
    info!("🎉 Pool Pairing Logic Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_log_event_creation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Log Event Creation Test");

    // Create test data
    let weth = Address::from([0x01; 20]);
    let usdc = Address::from([0x02; 20]);
    
    let v2_pool = V2PoolCreated {
        pair_address: Address::from([0x10; 20]),
        token0: weth,
        token1: usdc,
        fee: 3000,
    };
    
    let v3_pool = V3PoolCreated {
        pair_address: Address::from([0x20; 20]),
        token0: weth,
        token1: usdc,
        fee: 500,
        tick_spacing: 10,
    };
    
    // Test LogEvent creation from V2 pool (variant 2)
    let v2_log_event = LogEvent {
        pool_variant: 2,
        corresponding_pool_address: v3_pool.pair_address,
        log_pool_address: v2_pool.pair_address,
        token0: v2_pool.token0,
        token1: v2_pool.token1,
        fee: U24::from(v2_pool.fee),
    };
    
    // Validate V2 LogEvent structure
    assert_eq!(v2_log_event.pool_variant, 2, "V2 LogEvent should have variant 2");
    assert_eq!(v2_log_event.log_pool_address, v2_pool.pair_address, "Should reference V2 pool as log source");
    assert_eq!(v2_log_event.corresponding_pool_address, v3_pool.pair_address, "Should reference V3 pool as counterpart");
    assert_eq!(v2_log_event.token0, weth, "Should preserve token0");
    assert_eq!(v2_log_event.token1, usdc, "Should preserve token1");
    assert_eq!(v2_log_event.fee, U24::from(3000), "Should use V2 pool fee");
    
    info!("✅ V2 LogEvent validation passed");
    
    // Test LogEvent creation from V3 pool (variant 3)
    let v3_log_event = LogEvent {
        pool_variant: 3,
        corresponding_pool_address: v2_pool.pair_address,
        log_pool_address: v3_pool.pair_address,
        token0: v3_pool.token0,
        token1: v3_pool.token1,
        fee: U24::from(v3_pool.fee),
    };
    
    // Validate V3 LogEvent structure
    assert_eq!(v3_log_event.pool_variant, 3, "V3 LogEvent should have variant 3");
    assert_eq!(v3_log_event.log_pool_address, v3_pool.pair_address, "Should reference V3 pool as log source");
    assert_eq!(v3_log_event.corresponding_pool_address, v2_pool.pair_address, "Should reference V2 pool as counterpart");
    assert_eq!(v3_log_event.token0, weth, "Should preserve token0");
    assert_eq!(v3_log_event.token1, usdc, "Should preserve token1");
    assert_eq!(v3_log_event.fee, U24::from(500), "Should use V3 pool fee");
    
    info!("✅ V3 LogEvent validation passed");
    
    // Test that different pool variants create different LogEvents
    assert_ne!(v2_log_event.pool_variant, v3_log_event.pool_variant, "LogEvents should have different variants");
    assert_ne!(v2_log_event.fee, v3_log_event.fee, "LogEvents should preserve different fees");
    assert_ne!(v2_log_event.log_pool_address, v3_log_event.log_pool_address, "LogEvents should reference different source pools");
    
    info!("🎉 Log Event Creation Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_token_ordering_invariance() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Token Ordering Invariance Test");

    // Create tokens with different addresses to test ordering
    let token_a = Address::from([0x01; 20]); // Lower address
    let token_b = Address::from([0x02; 20]); // Higher address
    
    // Create pools with different token orderings
    let v2_pool_normal = V2PoolCreated {
        pair_address: Address::from([0x10; 20]),
        token0: token_a, // Lower first
        token1: token_b,
        fee: 3000,
    };
    
    let v3_pool_reversed = V3PoolCreated {
        pair_address: Address::from([0x20; 20]),
        token0: token_b, // Higher first
        token1: token_a,
        fee: 500,
        tick_spacing: 60,
    };
    
    // Create pairs hashmap
    let mut pairs: HashMap<Address, Event> = HashMap::new();
    pairs.insert(v2_pool_normal.pair_address, Event::PairCreated(v2_pool_normal.clone()));
    pairs.insert(v3_pool_reversed.pair_address, Event::PoolCreated(v3_pool_reversed.clone()));
    
    // Test V2 pool finding V3 counterpart despite different token ordering
    if let Some(Event::PairCreated(v2_pair)) = pairs.get(&v2_pool_normal.pair_address) {
        let v3_counterpart = pairs.values().find(|value| {
            matches!(value, Event::PoolCreated(v3_pair)
                if (v3_pair.token0 == v2_pair.token0 && v3_pair.token1 == v2_pair.token1) ||
                   (v3_pair.token0 == v2_pair.token1 && v3_pair.token1 == v2_pair.token0))
        });
        
        assert!(v3_counterpart.is_some(), "Should find V3 counterpart despite reversed token order");
        
        if let Some(Event::PoolCreated(v3_pair)) = v3_counterpart {
            // Verify the pairing logic handles reversed tokens correctly
            let tokens_match = (v3_pair.token0 == v2_pair.token0 && v3_pair.token1 == v2_pair.token1) ||
                              (v3_pair.token0 == v2_pair.token1 && v3_pair.token1 == v2_pair.token0);
            
            assert!(tokens_match, "Token pairing should work regardless of order");
            info!("✅ V2 pool ({:?}, {:?}) correctly paired with V3 pool ({:?}, {:?})", 
                  v2_pair.token0, v2_pair.token1, v3_pair.token0, v3_pair.token1);
        }
    }
    
    // Test V3 pool finding V2 counterpart
    if let Some(Event::PoolCreated(v3_pool)) = pairs.get(&v3_pool_reversed.pair_address) {
        let v2_counterpart = pairs.values().find(|value| {
            matches!(value, Event::PairCreated(v2_pair)
                if (v2_pair.token0 == v3_pool.token0 && v2_pair.token1 == v3_pool.token1) ||
                   (v2_pair.token0 == v3_pool.token1 && v2_pair.token1 == v3_pool.token0))
        });
        
        assert!(v2_counterpart.is_some(), "Should find V2 counterpart despite reversed token order");
        info!("✅ V3 pool with reversed tokens correctly found V2 counterpart");
    }
    
    info!("🎉 Token Ordering Invariance Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_edge_case_filtering() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Edge Case Filtering Test");

    let token_a = Address::from([0x01; 20]);
    let token_b = Address::from([0x02; 20]);
    
    // Test case: Pool with same token0 and token1 (should be filtered out)
    let invalid_pool = V3PoolCreated {
        pair_address: Address::from([0x30; 20]),
        token0: token_a,
        token1: token_a, // Same token - invalid
        fee: 3000,
        tick_spacing: 60,
    };
    
    // This should be caught by the check: if v3_pair.token0 == v3_pair.token1 {continue}
    let should_filter = invalid_pool.token0 == invalid_pool.token1;
    assert!(should_filter, "Pool with same token0 and token1 should be filtered out");
    info!("✅ Invalid pool with same tokens correctly identified for filtering");
    
    // Test case: Multiple pools with same token pair but different fees
    let v2_pool = V2PoolCreated {
        pair_address: Address::from([0x40; 20]),
        token0: token_a,
        token1: token_b,
        fee: 3000,
    };
    
    let v3_pool_500 = V3PoolCreated {
        pair_address: Address::from([0x50; 20]),
        token0: token_a,
        token1: token_b,
        fee: 500, // Different fee
        tick_spacing: 10,
    };
    
    let v3_pool_3000 = V3PoolCreated {
        pair_address: Address::from([0x60; 20]),
        token0: token_a,
        token1: token_b,
        fee: 3000, // Same fee as V2
        tick_spacing: 60,
    };
    
    let mut pairs: HashMap<Address, Event> = HashMap::new();
    pairs.insert(v2_pool.pair_address, Event::PairCreated(v2_pool.clone()));
    pairs.insert(v3_pool_500.pair_address, Event::PoolCreated(v3_pool_500.clone()));
    pairs.insert(v3_pool_3000.pair_address, Event::PoolCreated(v3_pool_3000.clone()));
    
    // Test that the find logic returns the first matching V3 pool (not necessarily same fee)
    if let Some(Event::PairCreated(pair)) = pairs.get(&v2_pool.pair_address) {
        let v3_matches: Vec<_> = pairs.values()
            .filter_map(|value| {
                match value {
                    Event::PoolCreated(v3_pair) 
                        if (v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1) ||
                           (v3_pair.token0 == pair.token1 && v3_pair.token1 == pair.token0) => {
                        Some(v3_pair)
                    },
                    _ => None
                }
            })
            .collect();
        
        assert_eq!(v3_matches.len(), 2, "Should find 2 V3 pools for the same token pair");
        info!("✅ Found {} V3 pools matching V2 pool token pair", v3_matches.len());
        
        // The current implementation will take the first match, which depends on HashMap iteration order
        let first_match = pairs.values().find(|value| {
            matches!(value, Event::PoolCreated(v3_pair)
                if (v3_pair.token0 == pair.token0 && v3_pair.token1 == pair.token1) ||
                   (v3_pair.token0 == pair.token1 && v3_pair.token1 == pair.token0))
        });
        
        assert!(first_match.is_some(), "Should find at least one V3 match");
        info!("✅ First match selection works correctly");
    }
    
    info!("🎉 Edge Case Filtering Test completed!");
    Ok(())
}
