use anyhow::Result;
use arbooo::common::logger;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::WsConnect;
use log::info;

#[tokio::test]
async fn test_alloydb_version_compatibility() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Testing AlloyDB version compatibility");

    // Print version information
    info!("📋 Version Information:");
    info!("   REVM version: 19.7.0");
    info!("   Alloy version: 0.7.3");
    
    // Test with direct Reth connection using HTTP instead of WebSocket
    info!("🔗 Testing with HTTP connection to Reth...");
    let reth_http_url = "http://192.168.0.14:8545";
    
    let provider = ProviderBuilder::new()
        .on_builtin(reth_http_url)
        .await?;
    
    let current_block = provider.get_block_number().await?;
    info!("📦 Current block from Reth (HTTP): {}", current_block);
    
    // Test basic AlloyDB imports and types
    info!("🔍 Testing AlloyDB imports...");
    use revm::db::{AlloyDB, CacheDB};
    use alloy::eips::BlockId;
    
    info!("✅ AlloyDB types imported successfully");
    
    // Try to create AlloyDB with different approaches
    info!("🧪 Testing AlloyDB creation approaches...");
    
    // Approach 1: Direct AlloyDB creation
    info!("   Approach 1: Direct AlloyDB::new()");
    let alloy_db_result = AlloyDB::new(&provider, BlockId::latest());
    match alloy_db_result {
        Some(db) => {
            info!("   ✅ AlloyDB::new() succeeded!");
            let _cached_db = CacheDB::new(db);
            info!("   ✅ CacheDB wrapper also works!");
        },
        None => {
            info!("   ❌ AlloyDB::new() returned None");
            
            // Try with specific block
            info!("   🔄 Trying with specific block...");
            let specific_block = BlockId::number(current_block);
            let alloy_db_specific = AlloyDB::new(&provider, specific_block);
            match alloy_db_specific {
                Some(_db) => info!("   ✅ AlloyDB works with specific block!"),
                None => info!("   ❌ AlloyDB fails even with specific block"),
            }
        }
    }
    
    // Test if the issue is with the provider or the block
    info!("🔍 Testing provider methods that AlloyDB might use...");
    
    // Test eth_getCode (AlloyDB needs this)
    use alloy::primitives::Address;
    let test_address = Address::from([0u8; 20]); // Zero address
    match provider.get_code_at(test_address).await {
        Ok(code) => info!("   ✅ eth_getCode works (length: {})", code.len()),
        Err(e) => info!("   ❌ eth_getCode failed: {}", e),
    }
    
    // Test eth_getBalance (AlloyDB needs this)
    match provider.get_balance(test_address).await {
        Ok(balance) => info!("   ✅ eth_getBalance works: {}", balance),
        Err(e) => info!("   ❌ eth_getBalance failed: {}", e),
    }
    
    // Test eth_getStorageAt (AlloyDB needs this)
    use alloy::primitives::U256;
    match provider.get_storage_at(test_address, U256::ZERO).await {
        Ok(storage) => info!("   ✅ eth_getStorageAt works: {:?}", storage),
        Err(e) => info!("   ❌ eth_getStorageAt failed: {}", e),
    }
    
    // Test if the issue is specific to the AlloyDB constructor
    info!("🔍 Investigating AlloyDB constructor behavior...");
    
    // Let's try to understand what AlloyDB::new() actually checks
    // Try with a contract address that definitely exists
    let weth_address = Address::from_slice(&[0xC0, 0x2a, 0xaA, 0x39, 0xb2, 0x23, 0xFE, 0x8D, 0x0A, 0x0e, 0x5C, 0x4F, 0x27, 0xeA, 0xD9, 0x08, 0x3C, 0x75, 0x6C, 0xc2]);
    let weth_code = provider.get_code_at(weth_address).await?;
    info!("   WETH contract code length: {}", weth_code.len());
    
    // Check if the block has the required data
    let block = provider.get_block(BlockId::latest(), alloy::rpc::types::BlockTransactionsKind::Hashes).await?;
    if let Some(block) = block {
        info!("   ✅ Block data available:");
        info!("      Block number: {}", block.header.number);
        info!("      Block hash: {:?}", block.header.hash);
        info!("      Gas limit: {}", block.header.gas_limit);
        info!("      Transaction count: {}", block.transactions.len());
    } else {
        info!("   ❌ Block data not available");
    }
    
    // Test chain ID (AlloyDB might check this)
    match provider.get_chain_id().await {
        Ok(chain_id) => info!("   ✅ Chain ID: {}", chain_id),
        Err(e) => info!("   ❌ Chain ID failed: {}", e),
    }
    
    info!("💡 HYPOTHESIS: AlloyDB might be checking something that your Reth node doesn't support");
    info!("   or the AlloyDB version might have stricter requirements than the basic RPC methods");
    
    info!("🎉 AlloyDB compatibility test completed!");
    Ok(())
}
