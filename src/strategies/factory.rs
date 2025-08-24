use crate::strategies::traits::*;
use crate::strategies::arbitrage::UniswapArbitrageStrategy;
use crate::strategies::sandwich::SandwichStrategy;
use crate::strategies::liquidation::LiquidationStrategy;
use crate::common::pairs::Event;
use crate::common::connection_pool::ConnectionPool;
use anyhow::Result;
use revm::primitives::Address;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Factory for creating different strategy instances
pub struct DefaultStrategyFactory {
    pools_map: Arc<RwLock<HashMap<Address, Event>>>,
    connection_pool: ConnectionPool,
}

impl DefaultStrategyFactory {
    pub fn new(
        pools_map: Arc<RwLock<HashMap<Address, Event>>>,
        connection_pool: ConnectionPool,
    ) -> Self {
        Self {
            pools_map,
            connection_pool,
        }
    }
}

impl StrategyFactory for DefaultStrategyFactory {
    fn create_strategy(&self, strategy_type: &str, config: StrategyConfig) -> Result<Box<dyn MevStrategy>> {
        match strategy_type.to_lowercase().as_str() {
            "arbitrage" | "uniswap_arbitrage" | "uniswap-arbitrage" => {
                Ok(Box::new(UniswapArbitrageStrategy::new(
                    config,
                    self.pools_map.clone(),
                    self.connection_pool.clone(),
                )))
            }
            "sandwich" | "sandwich_attack" | "sandwich-attack" => {
                Ok(Box::new(SandwichStrategy::new(config)))
            }
            "liquidation" | "lending_liquidation" | "lending-liquidation" => {
                Ok(Box::new(LiquidationStrategy::new(config)))
            }
            _ => Err(anyhow::anyhow!("Unknown strategy type: {}", strategy_type)),
        }
    }
    
    fn supported_strategies(&self) -> Vec<String> {
        vec![
            "arbitrage".to_string(),
            "sandwich".to_string(),
            "liquidation".to_string(),
        ]
    }
}
