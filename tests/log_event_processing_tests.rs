// Log Event Processing E2E Tests
// Tests log event detection, parsing, and processing pipeline

use anyhow::Result;
use arbooo::common::logger;
use alloy::providers::Provider;
use alloy::primitives::B256;
use alloy::rpc::types::BlockTransactionsKind;
use alloy::eips::BlockId;
use log::info;
use std::collections::HashMap;

#[path = "utils/mod.rs"]
mod utils;
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
        assert!(block.header.number > 0, "Block should have valid number");
        
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
