use crate::common::logs::LogEvent;
use crate::strategies::arbitrage::UniswapArbitrageStrategy;
use crate::strategies::traits::*;
use alloy_primitives::{Address, U256};
use anyhow::Result;
use log::{debug, error, info, warn};

/// Central manager for arbitrage strategies
pub struct StrategyManager {
    pub arbitrage_strategy: UniswapArbitrageStrategy,
    execution_context: ExecutionContext,
}

impl StrategyManager {
    pub async fn new(
        ws_url: String,
        max_connections: usize,
        executor_address: Address,
    ) -> Result<Self> {
        // Create default execution context
        let execution_context = ExecutionContext {
            block_number: 0,                          // Will be updated dynamically
            gas_price: U256::from(20_000_000_000u64), // 20 gwei default
            base_fee: U256::from(15_000_000_000u64),  // 15 gwei default
            executor_address,
            max_gas_limit: 2_000_000,
        };

        // Create arbitrage strategy configuration
        let arbitrage_config = StrategyConfig {
            enabled: true,
            priority: 90,
            min_profit_threshold: U256::from(100_000u128), // 0.0001 ETH minimum
            max_gas_price: U256::from(50_000_000_000u64), // 50 gwei max
            max_position_size: U256::from(10) * U256::from(10).pow(U256::from(18)), // 10 ETH max
        };

        // Initialize arbitrage strategy
        let arbitrage_strategy = UniswapArbitrageStrategy::new(
            arbitrage_config,
            ws_url,
            max_connections,
        ).await?;

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
            match self.arbitrage_strategy.identify_opportunities(log_event.clone(), &self.execution_context).await {
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

    /// Simulate an arbitrage opportunity
    pub async fn simulate_opportunity(
        &self,
        opportunity: &MevOpportunity,
    ) -> Result<ExecutionResult> {
        debug!("🧪 Simulating arbitrage opportunity");

        match opportunity {
            MevOpportunity::Arbitrage(_) | MevOpportunity::V2ToV3Arbitrage(_) => {
                // Main arbitrage strategy now handles both V3→V2 and V2→V3 arbitrage
                self.arbitrage_strategy
                    .simulate_opportunity(opportunity, &self.execution_context)
                    .await
            }
            _ => {
                Err(anyhow::anyhow!(
                    "Strategy cannot handle this opportunity type"
                ))
            }
        }
    }

    /// Execute an arbitrage opportunity
    pub async fn execute_opportunity(
        &self,
        opportunity: &MevOpportunity,
    ) -> Result<ExecutionResult> {
        info!("🚀 Executing arbitrage opportunity");

        // Check if opportunity is profitable
        let simulation_result = self.simulate_opportunity(opportunity).await?;

        if !simulation_result.success {
            return Err(anyhow::anyhow!(
                "Opportunity simulation failed: {:?}",
                simulation_result.error
            ));
        }

        // Get minimum profit threshold (same for both arbitrage types)
        let min_profit_threshold = self.arbitrage_strategy.config.min_profit_threshold;

        if simulation_result.profit < min_profit_threshold {
            return Err(anyhow::anyhow!(
                "Opportunity not profitable enough: {} wei < {} wei",
                simulation_result.profit,
                min_profit_threshold
            ));
        }

        // Execute the opportunity using the arbitrage strategy (handles both V3→V2 and V2→V3)
        match opportunity {
            MevOpportunity::Arbitrage(_) | MevOpportunity::V2ToV3Arbitrage(_) => {
                self.arbitrage_strategy
                    .execute_opportunity(opportunity, &self.execution_context)
                    .await
            }
            _ => {
                Err(anyhow::anyhow!("Unknown opportunity type"))
            }
        }
    }

    /// Process a complete arbitrage cycle: scan -> simulate -> execute if profitable
    pub async fn process_arbitrage_cycle(&self, log_event: LogEvent) -> Result<Vec<ExecutionResult>> {
        let mut results = vec![];

        // Scan for opportunities
        let opportunities = self.process_log_event(log_event).await?;

        if opportunities.is_empty() {
            debug!("No arbitrage opportunities found");
            return Ok(results);
        }

        // Process each opportunity
        for opportunity in opportunities {
            match self.simulate_opportunity(&opportunity).await {
                Ok(simulation_result) => {
                    // Get minimum profit threshold (same for both arbitrage types)
                    let min_profit_threshold = self.arbitrage_strategy.config.min_profit_threshold;

                    if simulation_result.success && simulation_result.profit >= min_profit_threshold {
                        info!(
                            "✅ Profitable opportunity found! Profit: {} wei",
                            simulation_result.profit
                        );

                        // Execute the opportunity
                        match self.execute_opportunity(&opportunity).await {
                            Ok(execution_result) => {
                                info!(
                                    "🎯 Arbitrage executed successfully! TX: {:?}",
                                    execution_result.tx_hash
                                );
                                results.push(execution_result);
                            }
                            Err(e) => {
                                error!("❌ Arbitrage execution failed: {}", e);
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
                            "📉 Opportunity not profitable: {} wei",
                            simulation_result.profit
                        );
                    }
                }
                Err(e) => {
                    warn!("❌ Simulation failed: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// Update execution context (typically called when new block arrives)
    pub fn update_execution_context(&mut self, context: ExecutionContext) {
        debug!("📝 Updated execution context for block {}", context.block_number);
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

    /// Get the arbitrage strategy reference
    pub fn get_arbitrage_strategy(&self) -> &UniswapArbitrageStrategy {
        &self.arbitrage_strategy
    }

    /// Get current execution context
    pub fn get_execution_context(&self) -> &ExecutionContext {
        &self.execution_context
    }
}

