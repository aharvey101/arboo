use crate::strategies::traits::*;
use crate::common::{
    logs::LogEvent,
    pairs::Event,
    connection_pool::ConnectionPool,
};
use crate::strategies::PoolVersion;
use async_trait::async_trait;
use anyhow::Result;
use alloy_primitives::{Address, U256};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use log::{info, debug, warn};

/// Implementation of LogEvent as an MevEvent
impl MevEvent for LogEvent {
    fn event_type(&self) -> &str {
        match self.pool_variant {
            2 => "uniswap_v2_swap",
            3 => "uniswap_v3_swap", 
            _ => "unknown_swap",
        }
    }
    
    fn block_number(&self) -> u64 {
        // LogEvent doesn't currently have block number, returning 0 for now
        // TODO: Add block_number field to LogEvent
        0
    }
    
    fn transaction_index(&self) -> Option<u64> {
        None
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn clone_boxed(&self) -> Box<dyn MevEvent> {
        Box::new(self.clone())
    }
}

/// Uniswap V2/V3 Arbitrage Strategy
#[derive(Debug)]
pub struct UniswapArbitrageStrategy {
    config: StrategyConfig,
    pools_map: Arc<RwLock<HashMap<Address, Event>>>,
    connection_pool: ConnectionPool,
}

impl UniswapArbitrageStrategy {
    pub fn new(
        config: StrategyConfig,
        pools_map: Arc<RwLock<HashMap<Address, Event>>>,
        connection_pool: ConnectionPool,
    ) -> Self {
        Self {
            config,
            pools_map,
            connection_pool,
        }
    }
    
    async fn extract_log_event(event: &dyn MevEvent) -> Option<&LogEvent> {
        event.as_any().downcast_ref::<LogEvent>()
    }
    
    async fn convert_to_arbitrage_opportunity(&self, log_event: &LogEvent) -> Result<ArbitrageOpportunity> {
        let pools_guard = self.pools_map.read().await;
        
        let pool_a_info = pools_guard.get(&log_event.log_pool_address);
        let pool_b_info = pools_guard.get(&log_event.corresponding_pool_address);
        
        let (pool_variant_a, fee_a) = match pool_a_info {
            Some(Event::PairCreated(v2_pool)) => (PoolVersion::UniswapV2, v2_pool.fee),
            Some(Event::PoolCreated(v3_pool)) => (PoolVersion::UniswapV3, v3_pool.fee),
            None => return Err(anyhow::anyhow!("Pool A not found in pools map")),
        };
        
        let (pool_variant_b, fee_b) = match pool_b_info {
            Some(Event::PairCreated(v2_pool)) => (PoolVersion::UniswapV2, v2_pool.fee),
            Some(Event::PoolCreated(v3_pool)) => (PoolVersion::UniswapV3, v3_pool.fee),
            None => return Err(anyhow::anyhow!("Pool B not found in pools map")),
        };
        
        let test_amount = U256::from(100) * U256::from(10).pow(U256::from(18));
        
        Ok(ArbitrageOpportunity {
            token_in: log_event.token1,
            token_out: log_event.token0,
            pool_a: log_event.log_pool_address,
            pool_b: log_event.corresponding_pool_address,
            amount_in: test_amount,
            expected_profit: U256::ZERO, // Will be calculated during simulation
            pool_variant_a,
            pool_variant_b,
            fee_a,
            fee_b,
        })
    }
}

#[async_trait]
impl MevStrategy for UniswapArbitrageStrategy {
    fn name(&self) -> &str {
        "UniswapArbitrage"
    }
    
    fn config(&self) -> &StrategyConfig {
        &self.config
    }
    
    fn update_config(&mut self, config: StrategyConfig) {
        self.config = config;
    }
    
    async fn scan_opportunities(&self, event: &dyn MevEvent) -> Result<Vec<MevOpportunity>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        
        let log_event = match Self::extract_log_event(event).await {
            Some(event) => event,
            None => {
                debug!("Event is not a LogEvent, skipping");
                return Ok(vec![]);
            }
        };
        
        info!("🔍 Scanning for arbitrage opportunities: {} (variant: {})", 
              log_event.log_pool_address, log_event.pool_variant);
        
        let arbitrage_opportunity = self.convert_to_arbitrage_opportunity(log_event).await?;
        
        Ok(vec![MevOpportunity::Arbitrage(arbitrage_opportunity)])
    }
    
    async fn simulate_opportunity(
        &self,
        opportunity: &MevOpportunity,
        _context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let arbitrage_opp = match opportunity {
            MevOpportunity::Arbitrage(opp) => opp,
            _ => return Err(anyhow::anyhow!("Not an arbitrage opportunity")),
        };
        
        debug!("🧪 Simulating arbitrage opportunity with semaphore pattern");
        
        // Simplified simulation to test the semaphore pattern first
        // Then we can add the full EVM simulation back with proper Send/Sync handling
        
        // Simple heuristic: check if the amount is reasonable and pools exist
        let pools_guard = self.pools_map.read().await;
        let pool_a_exists = pools_guard.contains_key(&arbitrage_opp.pool_a);
        let pool_b_exists = pools_guard.contains_key(&arbitrage_opp.pool_b);
        drop(pools_guard);
        
        if !pool_a_exists || !pool_b_exists {
            return Ok(ExecutionResult {
                success: false,
                profit: U256::ZERO,
                gas_used: U256::from(500_000),
                tx_hash: None,
                error: Some("One or more pools not found".to_string()),
            });
        }
        
        // Simple profit estimation (mock)
        let estimated_profit = if arbitrage_opp.amount_in > U256::from(10).pow(U256::from(18)) {
            U256::from(500_000) // Mock 0.0005 ETH profit for larger amounts
        } else {
            U256::from(50_000) // Mock 0.00005 ETH profit for smaller amounts
        };
        
        let success = estimated_profit >= self.config.min_profit_threshold;
        
        if success {
            info!("✅ Simulation successful with semaphore! Estimated Profit: {} wei", estimated_profit);
        } else {
            debug!("❌ Simulation unprofitable: {} wei (threshold: {} wei)", 
                   estimated_profit, self.config.min_profit_threshold);
        }
        
        Ok(ExecutionResult {
            success,
            profit: estimated_profit,
            gas_used: U256::from(500_000), // Estimated
            tx_hash: None,
            error: if success { None } else { Some("Insufficient profit".to_string()) },
        })
    }
    
    async fn execute_opportunity(
        &self,
        opportunity: &MevOpportunity,
        _context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let _arbitrage_opp = match opportunity {
            MevOpportunity::Arbitrage(opp) => opp,
            _ => return Err(anyhow::anyhow!("Not an arbitrage opportunity")),
        };
        
        // TODO: Implement actual transaction execution
        // For now, return a mock result
        warn!("🚧 Arbitrage execution not yet implemented - would execute transaction here");
        
        Ok(ExecutionResult {
            success: true,
            profit: U256::from(1000000), // Mock profit
            gas_used: U256::from(500000),
            tx_hash: Some("0x1234567890abcdef".to_string()),
            error: None,
        })
    }
    
    fn can_handle(&self, opportunity: &MevOpportunity) -> bool {
        matches!(opportunity, MevOpportunity::Arbitrage(_))
    }
}
