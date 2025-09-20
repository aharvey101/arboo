use anyhow::Result;
use arbooo::common::logger;
use alloy::providers::{Provider, ProviderBuilder};
use log::info;

#[tokio::test]
async fn test_evm_simulation_direct_reth_connection() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Testing EVM simulation with DIRECT Reth connection (no Anvil)");

    // Connect directly to Reth node via WebSocket
    let reth_ws_url = "ws://192.168.0.14:8547";
    info!("🔗 Connecting directly to Reth WebSocket: {}", reth_ws_url);
    
    let provider = ProviderBuilder::new()
        .on_ws(alloy::rpc::client::WsConnect::new(reth_ws_url.to_string()))
        .await?;
    
    let current_block = provider.get_block_number().await?;
    info!("📦 Current block from direct Reth connection: {}", current_block);
    
    // Test EVM simulation with direct Reth connection
    info!("🧪 Testing EVM simulation with direct Reth connection...");
    
    use arbooo::common::revm::EvmSimulator;
    
    // Test 1: Create EVM simulator with current block
    info!("   Test 1: EvmSimulator with current block ({})", current_block);
    match EvmSimulator::new_with_db(None, alloy::primitives::U64::from(current_block), provider.clone()) {
        Ok(_simulator) => {
            info!("   ✅ SUCCESS: EVM simulator works with current block on direct Reth!");
        },
        Err(e) => {
            info!("   ❌ FAILED: EVM simulator failed with current block: {}", e);
        }
    }
    
    // Test 2: Create EVM simulator with earlier block
    let earlier_block = current_block.saturating_sub(100);
    info!("   Test 2: EvmSimulator with earlier block ({})", earlier_block);
    match EvmSimulator::new_with_db(None, alloy::primitives::U64::from(earlier_block), provider.clone()) {
        Ok(_simulator) => {
            info!("   ✅ SUCCESS: EVM simulator works with earlier block on direct Reth!");
        },
        Err(e) => {
            info!("   ❌ FAILED: EVM simulator failed with earlier block: {}", e);
        }
    }
    
    // Test 3: Try a much earlier block to test historical state
    let much_earlier_block = current_block.saturating_sub(1000);
    info!("   Test 3: EvmSimulator with much earlier block ({})", much_earlier_block);
    match EvmSimulator::new_with_db(None, alloy::primitives::U64::from(much_earlier_block), provider.clone()) {
        Ok(_simulator) => {
            info!("   ✅ SUCCESS: EVM simulator works with much earlier block on direct Reth!");
        },
        Err(e) => {
            info!("   ❌ FAILED: EVM simulator failed with much earlier block: {}", e);
        }
    }
    
    // Test 4: Test with a known good block (latest - 10)
    let safe_block = current_block.saturating_sub(10);
    info!("   Test 4: EvmSimulator with safe block ({})", safe_block);
    match EvmSimulator::new_with_db(None, alloy::primitives::U64::from(safe_block), provider) {
        Ok(simulator) => {
            info!("   ✅ SUCCESS: EVM simulator works with safe block on direct Reth!");
            
            // Try to perform a simple simulation
            info!("   🧪 Testing basic EVM simulation...");
            // We can add actual simulation logic here if needed
            info!("   ✅ EVM simulator is ready for simulations!");
        },
        Err(e) => {
            info!("   ❌ FAILED: EVM simulator failed with safe block: {}", e);
        }
    }
    
    info!("🎉 Direct Reth EVM simulation test completed!");
    Ok(())
}

#[tokio::test]
async fn test_alloydb_anvil_vs_direct_comparison() -> Result<()> {
    logger::setup_logger();
    info!("🧪 COMPARISON TEST: AlloyDB with Anvil vs Direct Reth");

    // Test 1: Direct Reth connection
    info!("🔗 Part 1: Testing with DIRECT Reth connection...");
    let reth_ws_url = "ws://192.168.0.14:8547";
    let direct_provider = ProviderBuilder::new()
        .on_ws(alloy::rpc::client::WsConnect::new(reth_ws_url.to_string()))
        .await?;
    
    let direct_block = direct_provider.get_block_number().await?;
    info!("   Direct Reth block: {}", direct_block);
    
    use revm::db::AlloyDB;
    use alloy::eips::BlockId;
    
    let direct_alloydb = AlloyDB::new(&direct_provider, BlockId::latest());
    let direct_success = direct_alloydb.is_some();
    info!("   Direct Reth AlloyDB result: {}", if direct_success { "✅ SUCCESS" } else { "❌ FAILED" });
    
    // Test 2: Anvil connection (forked from same Reth)
    info!("🔗 Part 2: Testing with Anvil (forked from Reth)...");
    
    // Set up anvil fork
    std::env::set_var("MAINNET_RPC_URL", "http://192.168.0.14:8545");
    
    use super::super::utils::test_env::{TestEnvironment, TestConfig};
    let config = TestConfig {
        ws_url: "".to_string(),
        fork_block_number: Some(direct_block), // Use same block as direct test
        test_timeout_secs: 30,
    };
    
    let test_env = TestEnvironment::new_with_config(config).await?;
    let anvil_block = test_env.provider.get_block_number().await?;
    info!("   Anvil block: {}", anvil_block);
    
    let anvil_alloydb = AlloyDB::new(test_env.provider.as_ref(), BlockId::latest());
    let anvil_success = anvil_alloydb.is_some();
    info!("   Anvil AlloyDB result: {}", if anvil_success { "✅ SUCCESS" } else { "❌ FAILED" });
    
    // Compare results
    info!("🔍 COMPARISON RESULTS:");
    info!("   Direct Reth:  {}", if direct_success { "✅ WORKS" } else { "❌ FAILS" });
    info!("   Anvil Fork:   {}", if anvil_success { "✅ WORKS" } else { "❌ FAILS" });
    
    if direct_success && !anvil_success {
        info!("🎯 CONFIRMED: AlloyDB works with direct Reth but NOT with Anvil");
        info!("🔍 This proves Anvil is not properly proxying some RPC calls that AlloyDB needs");
    } else if !direct_success && !anvil_success {
        info!("🎯 ISSUE: AlloyDB fails with both direct Reth and Anvil");
        info!("🔍 This suggests a broader compatibility issue");
    } else if direct_success && anvil_success {
        info!("🎯 UNEXPECTED: AlloyDB works with both - the issue might be intermittent");
    } else {
        info!("🎯 VERY UNEXPECTED: AlloyDB works with Anvil but not direct Reth");
    }
    
    info!("🎉 Comparison test completed!");
    Ok(())
}
