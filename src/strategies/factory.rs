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

impl DefaultStrategyFactory {
    /// Create arbitrage strategy asynchronously 
    pub async fn create_arbitrage_strategy(&self, config: StrategyConfig, ws_url: String, max_connections: usize) -> Result<UniswapArbitrageStrategy> {
        UniswapArbitrageStrategy::new(config, ws_url, max_connections).await
    }
}

impl StrategyFactory for DefaultStrategyFactory {
    fn create_strategy(&self, strategy_type: &str, config: StrategyConfig) -> Result<Box<dyn MevStrategy>> {
        match strategy_type.to_lowercase().as_str() {
            "sandwich" | "sandwich_attack" | "sandwich-attack" => {
                Ok(Box::new(SandwichStrategy::new(config)))
            }
            "liquidation" | "lending_liquidation" | "lending-liquidation" => {
                Ok(Box::new(LiquidationStrategy::new(config)))
            }
            "arbitrage" | "uniswap_arbitrage" | "uniswap-arbitrage" => {
                // Note: Arbitrage strategy requires async initialization
                // Use create_arbitrage_strategy() method instead
                Err(anyhow::anyhow!("Arbitrage strategy requires async initialization. Use create_arbitrage_strategy() method."))
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
