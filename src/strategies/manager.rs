use crate::common::logs::LogEvent;
use crate::strategies::arbitrage::UniswapArbitrageStrategy;
use crate::strategies::traits::*;
use alloy_primitives::U256;
use anyhow::Result;
use log::{debug, error, info};

/// Central manager for arbitrage strategies
pub struct StrategyManager {
    pub arbitrage_strategy: UniswapArbitrageStrategy,
    execution_context: ExecutionContext,
}

impl StrategyManager {
    pub async fn new(ws_url: String, max_connections: usize) -> Result<Self> {
        // Create default execution context
        let execution_context = ExecutionContext {
            block_number: 0,                          // Will be updated dynamically
            gas_price: U256::from(20_000_000_000u64), // 20 gwei default
            base_fee: U256::from(15_000_000_000u64),  // 15 gwei default
            max_gas_limit: 2_000_000,
        };

        // Create arbitrage strategy configuration
        let arbitrage_config = StrategyConfig {
            enabled: true,
            priority: 90,
            min_profit_threshold: U256::from(100_000u128), // 0.0001 ETH minimum
            max_gas_price: U256::from(50_000_000_000u64),  // 50 gwei max
            max_position_size: U256::from(10) * U256::from(10).pow(U256::from(18)), // 10 ETH max
        };

        // Initialize arbitrage strategy
        let arbitrage_strategy =
            UniswapArbitrageStrategy::new(arbitrage_config, ws_url, max_connections).await?;

        info!("✅ Strategy Manager initialized with arbitrage strategy");

        Ok(Self {
            arbitrage_strategy,
            execution_context,
        })
    }

    /// Process a log event and scan for arbitrage opportunities
    pub async fn process_log_event(&self, log_event: LogEvent) -> Result<Vec<MevOpportunity>> {
        debug!(
            "🔍 Processing log event from pool: {}",
            log_event.log_pool_address
        );

        let mut all_opportunities = vec![];

        // Check main arbitrage strategy - now handles both V3→V2 and V2→V3 arbitrage
        if self.arbitrage_strategy.config.enabled {
            match self
                .arbitrage_strategy
                .identify_opportunities(log_event.clone(), &self.execution_context)
                .await
            {
                Ok(opportunities) => {
                    debug!("📊 Found {} arbitrage opportunities", opportunities.len());
                    all_opportunities.extend(opportunities);
                }
                Err(e) => {
                    error!("❌ Failed to scan arbitrage opportunities: {}", e);
                }
            }
        }

        debug!("📊 Total opportunities found: {}", all_opportunities.len());
        Ok(all_opportunities)
    }

    /// Update execution context (typically called when new block arrives)
    pub fn update_execution_context(&mut self, context: ExecutionContext) {
        debug!(
            "📝 Updated execution context for block {}",
            context.block_number
        );
        self.execution_context = context;
    }

    /// Get strategy configuration
    pub fn get_strategy_config(&self) -> &StrategyConfig {
        &self.arbitrage_strategy.config
    }

    /// Enable/disable the arbitrage strategy
    pub fn configure_strategy(&mut self, enabled: bool) -> Result<()> {
        // Update the config directly since we don't have update_config method
        self.arbitrage_strategy.config.enabled = enabled;
        info!(
            "📝 Arbitrage strategy {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Complete arbitrage cycle using enhanced multi-contract simulation
    /// This is now the primary arbitrage cycle method
    pub async fn process_arbitrage_cycle(
        &mut self,
        log_event: LogEvent,
    ) -> Result<Vec<ExecutionResult>> {
        let mut results = vec![];

        // Scan for opportunities using the standard method
        let opportunities = self.process_log_event(log_event).await?;

        if opportunities.is_empty() {
            debug!("No arbitrage opportunities found");
            return Ok(results);
        }

        debug!(
            "🔍 Found {} opportunities, processing with enhanced simulation",
            opportunities.len()
        );

        // Process each opportunity with enhanced capabilities
        for opportunity in opportunities {
            match self
                .arbitrage_strategy
                .simulate_opportunity(&opportunity, &self.execution_context)
                .await
            {
                Ok(simulation_result) => {
                    if simulation_result.success
                        && simulation_result.profit
                            >= self.arbitrage_strategy.config.min_profit_threshold
                    {
                        info!(
                            "✅ Enhanced simulation successful! Profit: {} wei",
                            simulation_result.profit
                        );

                        // Execute the opportunity
                        match self
                            .arbitrage_strategy
                            .execute_opportunity(&opportunity, &self.execution_context)
                            .await
                        {
                            Ok(execution_result) => {
                                info!(
                                    "🎯 Enhanced execution completed! TX: {:?}",
                                    execution_result.tx_hash
                                );
                                results.push(execution_result);
                            }
                            Err(e) => {
                                error!("❌ Enhanced execution failed: {}", e);
                                results.push(ExecutionResult {
                                    success: false,
                                    profit: U256::ZERO,
                                    gas_used: simulation_result.gas_used,
                                    tx_hash: None,
                                    error: Some(e.to_string()),
                                });
                            }
                        }
                    } else {
                        debug!(
                            "📉 Enhanced simulation - opportunity not profitable: {} wei",
                            simulation_result.profit
                        );
                    }
                }
                Err(e) => {
                    error!("❌ Enhanced simulation failed: {}", e);
                    results.push(ExecutionResult {
                        success: false,
                        profit: U256::ZERO,
                        gas_used: U256::from(100_000),
                        tx_hash: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(results)
    }
}
