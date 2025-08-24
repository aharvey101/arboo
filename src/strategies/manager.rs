use crate::strategies::traits::*;
use crate::strategies::arbitrage::UniswapArbitrageStrategy;
use crate::strategies::sandwich::SandwichStrategy;
use crate::strategies::liquidation::LiquidationStrategy;
use crate::strategies::PoolVersion;
use crate::common::pairs::Event;
use crate::common::logs::LogEvent;
use crate::common::connection_pool::ConnectionPool;
use anyhow::Result;
use alloy_primitives::{Address, U256};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Semaphore, broadcast};
use log::{info, debug, error, warn};

/// Central manager for all MEV strategies
pub struct StrategyManager {
    strategies: Vec<Box<dyn MevStrategy>>,
    opportunity_queue: Arc<RwLock<OpportunityQueue>>,
    task_semaphore: Arc<Semaphore>,
    opportunity_broadcaster: OpportunityBroadcaster,
    execution_context: ExecutionContext,
    connection_pool: ConnectionPool,
}

impl StrategyManager {
    pub async fn new(
        ws_url: String,
        max_connections: usize,
        pools_map: Arc<RwLock<HashMap<Address, Event>>>,
        executor_address: Address,
    ) -> Result<Self> {
        let max_concurrent_tasks = (max_connections * 2).min(32);
        let task_semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));
        let opportunity_queue = Arc::new(RwLock::new(OpportunityQueue::new()));
        let (opportunity_broadcaster, _) = broadcast::channel(1000);
        
        // Create connection pool
        let connection_pool = ConnectionPool::new(ws_url, max_connections);
        
        // Create default execution context
        let execution_context = ExecutionContext {
            block_number: 0, // Will be updated dynamically
            gas_price: U256::from(20_000_000_000u64), // 20 gwei default
            base_fee: U256::from(15_000_000_000u64), // 15 gwei default
            executor_address,
            max_gas_limit: 2_000_000,
        };
        
        // Initialize strategies
        let mut strategies: Vec<Box<dyn MevStrategy>> = vec![];
        
        // 1. Arbitrage Strategy (highest priority)
        let arbitrage_config = StrategyConfig {
            enabled: true,
            priority: 90,
            min_profit_threshold: U256::from(100_000u128), // 0.0001 ETH minimum
            ..Default::default()
        };
        strategies.push(Box::new(UniswapArbitrageStrategy::new(
            arbitrage_config,
            pools_map,
            connection_pool.clone(),
        )));
        
        // 2. Sandwich Strategy (medium priority)
        let sandwich_config = StrategyConfig {
            enabled: false, // Disabled by default - requires mempool monitoring
            priority: 70,
            min_profit_threshold: U256::from(500_000u128), // 0.0005 ETH minimum
            ..Default::default()
        };
        strategies.push(Box::new(SandwichStrategy::new(sandwich_config)));
        
        // 3. Liquidation Strategy (lower priority)
        let liquidation_config = StrategyConfig {
            enabled: false, // Disabled by default - requires lending protocol integration
            priority: 50,
            min_profit_threshold: U256::from(1_000_000u128), // 0.001 ETH minimum
            ..Default::default()
        };
        strategies.push(Box::new(LiquidationStrategy::new(liquidation_config)));
        
        info!("✅ Strategy Manager initialized with {} strategies", strategies.len());
        for strategy in &strategies {
            info!("  📈 Strategy: {} (enabled: {}, priority: {})", 
                  strategy.name(), 
                  strategy.config().enabled,
                  strategy.config().priority);
        }
        
        Ok(Self {
            strategies,
            opportunity_queue,
            task_semaphore,
            opportunity_broadcaster,
            execution_context,
            connection_pool,
        })
    }
    
    /// Start the strategy manager with event processing
    pub async fn start(&self, mut log_event_receiver: broadcast::Receiver<LogEvent>) -> Result<()> {
        info!("🚀 Starting Strategy Manager");
        
        // Spawn opportunity processor task
        let _opportunity_processor = self.start_opportunity_processor().await?;
        
        // Main event processing loop
        while let Ok(log_event) = log_event_receiver.recv().await {
            let opportunity_broadcaster = self.opportunity_broadcaster.clone();
            let task_semaphore = self.task_semaphore.clone();
            
            // Process event with all enabled strategies
            for strategy in &self.strategies {
                if !strategy.config().enabled {
                    continue;
                }
                
                let strategy_name = strategy.name().to_string();
                let broadcaster = opportunity_broadcaster.clone();
                let semaphore = task_semaphore.clone();
                // Clone the log event for each strategy
                let log_event_clone = log_event.clone();
                
                tokio::spawn(async move {
                    let _permit = match semaphore.acquire().await {
                        Ok(permit) => permit,
                        Err(_) => {
                            error!("Failed to acquire semaphore permit for {}", strategy_name);
                            return;
                        }
                    };
                    
                    // Use the cloned event as MevEvent
                    let event: &dyn MevEvent = &log_event_clone;
                    
                    Self::process_single_strategy_event(
                        event,
                        &strategy_name,
                        broadcaster,
                    ).await;
                });
            }
        }
        
        Ok(())
    }
    
    /// Process a single event with a single strategy
    async fn process_single_strategy_event(
        event: &dyn MevEvent,
        strategy_name: &str,
        opportunity_broadcaster: OpportunityBroadcaster,
    ) {
        debug!("🔍 Processing event: {} with strategy: {}", event.event_type(), strategy_name);
        
        // For now, just create a mock opportunity for arbitrage strategy
        if strategy_name == "UniswapArbitrage" {
            if let Some(log_event) = event.as_any().downcast_ref::<LogEvent>() {
                // Create a mock arbitrage opportunity
                let arbitrage_opp = ArbitrageOpportunity {
                    token_in: log_event.token1,
                    token_out: log_event.token0,
                    pool_a: log_event.log_pool_address,
                    pool_b: log_event.corresponding_pool_address,
                    amount_in: U256::from(100) * U256::from(10).pow(U256::from(18)),
                    expected_profit: U256::ZERO,
                    pool_variant_a: PoolVersion::UniswapV2,
                    pool_variant_b: PoolVersion::UniswapV3,
                    fee_a: 3000,
                    fee_b: 500,
                };
                
                let opportunity = MevOpportunity::Arbitrage(arbitrage_opp);
                
                debug!("📊 Found opportunity with {} strategy", strategy_name);
                
                // Broadcast opportunity to other components
                if let Err(e) = opportunity_broadcaster.send(opportunity) {
                    debug!("No receivers for opportunity broadcast: {}", e);
                }
            }
        }
    }
    
    /// Process a single event with all enabled strategies (legacy method)
    async fn process_event_with_strategies(
        event: &dyn MevEvent,
        strategies: &[Box<dyn MevStrategy>],
        opportunity_broadcaster: OpportunityBroadcaster,
    ) {
        debug!("🔍 Processing event: {} at block {}", event.event_type(), event.block_number());
        
        for strategy in strategies {
            if !strategy.config().enabled {
                continue;
            }
            
            match strategy.scan_opportunities(event).await {
                Ok(opportunities) => {
                    for opportunity in opportunities {
                        debug!("📊 Found opportunity with {} strategy", strategy.name());
                        
                        // Broadcast opportunity to other components
                        if let Err(e) = opportunity_broadcaster.send(opportunity) {
                            debug!("No receivers for opportunity broadcast: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Strategy {} failed to scan opportunities: {}", strategy.name(), e);
                }
            }
        }
    }
    
    /// Start the opportunity processor task
    async fn start_opportunity_processor(&self) -> Result<tokio::task::JoinHandle<()>> {
        let mut opportunity_receiver = self.opportunity_broadcaster.subscribe();
        let opportunity_queue = self.opportunity_queue.clone();
        let strategies = self.get_strategy_references();
        let execution_context = self.execution_context.clone();
        let task_semaphore = self.task_semaphore.clone();
        
        let handle = tokio::spawn(async move {
            info!("🎯 Opportunity processor started");
            
            while let Ok(opportunity) = opportunity_receiver.recv().await {
                let opportunity_queue_clone = opportunity_queue.clone();
                let strategies_clone = strategies.clone();
                let context = execution_context.clone();
                let semaphore_clone = task_semaphore.clone();
                
                // Use the same pattern as the original strategy implementation
                tokio::spawn(async move {
                    // Acquire permit with timeout to avoid indefinite blocking
                    let _permit = match tokio::time::timeout(
                        tokio::time::Duration::from_millis(100),
                        semaphore_clone.acquire()
                    ).await {
                        Ok(Ok(permit)) => permit,
                        Ok(Err(_)) => {
                            error!("Semaphore acquisition failed (semaphore closed)");
                            return;
                        },
                        Err(_) => {
                            warn!("Max concurrent tasks reached, dropping opportunity (timeout)");
                            return;
                        }
                    };
                    
                    // Run the CPU-intensive EVM work in a separate thread pool
                    // This matches the original strategy implementation pattern
                    let simulation_result = tokio::task::spawn_blocking(move || {
                        // Create a new runtime for this thread if needed
                        let rt = tokio::runtime::Handle::try_current()
                            .unwrap_or_else(|_| {
                                tokio::runtime::Runtime::new().unwrap().handle().clone()
                            });
                        
                        rt.block_on(async move {
                            Self::process_opportunity(
                                opportunity,
                                opportunity_queue_clone,
                                strategies_clone,
                                context,
                            ).await
                        })
                    }).await;
                    
                    if let Err(e) = simulation_result {
                        error!("EVM simulation task failed: {}", e);
                    }
                });
            }
        });
        
        Ok(handle)
    }
    
    /// Process a single opportunity
    async fn process_opportunity(
        opportunity: MevOpportunity,
        opportunity_queue: Arc<RwLock<OpportunityQueue>>,
        strategies: Vec<StrategyReference>,
        context: ExecutionContext,
    ) {
        // Find the strategy that can handle this opportunity
        let handler = strategies.iter()
            .find(|s| s.can_handle(&opportunity));
            
        let handler = match handler {
            Some(h) => h,
            None => {
                warn!("⚠️  No strategy can handle opportunity type");
                return;
            }
        };
        
        // Simulate the opportunity first
        match handler.simulate_opportunity(&opportunity, &context).await {
            Ok(simulation_result) => {
                if simulation_result.success && 
                   simulation_result.profit >= handler.config().min_profit_threshold {
                    
                    info!("✅ Opportunity simulation successful! Profit: {} wei", simulation_result.profit);
                    
                    // Add to priority queue
                    let mut queue = opportunity_queue.write().await;
                    queue.push(opportunity, handler.config().priority);
                    debug!("📋 Added opportunity to queue (size: {})", queue.len());
                    drop(queue);
                    
                    // TODO: Execute the opportunity
                    // For now, we just log it
                    warn!("🚧 Opportunity execution not yet implemented");
                    
                } else {
                    debug!("❌ Opportunity not profitable: {} wei (threshold: {} wei)", 
                           simulation_result.profit, handler.config().min_profit_threshold);
                }
            }
            Err(e) => {
                error!("❌ Opportunity simulation failed: {}", e);
            }
        }
    }
    
    /// Get strategy references for async processing
    fn get_strategy_references(&self) -> Vec<StrategyReference> {
        self.strategies.iter().map(|s| StrategyReference {
            name: s.name().to_string(),
            config: s.config().clone(),
        }).collect()
    }
    
    /// Update execution context (typically called when new block arrives)
    pub fn update_execution_context(&mut self, context: ExecutionContext) {
        self.execution_context = context;
    }
    
    /// Get current queue status
    pub async fn get_queue_status(&self) -> (usize, Vec<String>) {
        let queue = self.opportunity_queue.read().await;
        let size = queue.len();
        let strategies: Vec<String> = self.strategies.iter()
            .map(|s| format!("{} ({})", s.name(), if s.config().enabled { "enabled" } else { "disabled" }))
            .collect();
        (size, strategies)
    }
    
    /// Enable/disable a specific strategy
    pub fn configure_strategy(&mut self, strategy_name: &str, enabled: bool) -> Result<()> {
        for strategy in &mut self.strategies {
            if strategy.name() == strategy_name {
                let mut config = strategy.config().clone();
                config.enabled = enabled;
                strategy.update_config(config);
                info!("📝 Strategy {} {}", strategy_name, if enabled { "enabled" } else { "disabled" });
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("Strategy '{}' not found", strategy_name))
    }
    
    /// Process a single MEV event with semaphore-controlled concurrency
    /// This matches the original strategy's tokio::spawn + semaphore pattern
    pub async fn process_mev_event_with_semaphore(
        &self,
        event: &dyn MevEvent,
        context: &ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        debug!("🚀 Processing MEV event with semaphore pattern");
        
        // Try to acquire semaphore permit with timeout (100ms like original strategy)
        let permit = match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            self.task_semaphore.acquire()
        ).await {
            Ok(Ok(permit)) => {
                debug!("✅ Acquired semaphore permit for MEV processing");
                permit
            },
            Ok(Err(e)) => {
                warn!("❌ Failed to acquire semaphore permit: {}", e);
                return Err(anyhow::anyhow!("Failed to acquire semaphore: {}", e));
            },
            Err(_) => {
                warn!("⏱️ Timeout acquiring semaphore permit (100ms)");
                return Err(anyhow::anyhow!("Semaphore acquisition timeout"));
            }
        };
        
        // Process with CPU-intensive work in spawn_blocking (like original strategy)
        let strategies_clone = self.get_strategy_references();
        let event_data = self.extract_event_data(event);
        let context_clone = context.clone();
        
        let processing_result = tokio::task::spawn_blocking(move || {
            // This runs in the blocking thread pool like the original strategy
            debug!("🧠 Processing event in spawn_blocking with {} strategies", strategies_clone.len());
            
            // Create a simple tokio runtime for the blocking context
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut all_results = vec![];
                
                for strategy_ref in &strategies_clone {
                    if !strategy_ref.config().enabled {
                        continue;
                    }
                    
                    debug!("🔄 Processing with strategy: {}", strategy_ref.name);
                    
                    // Create mock opportunity based on event data
                    let mock_opportunity = create_mock_opportunity_from_event(&event_data);
                    
                    // Simulate the opportunity (this would be the real simulation in production)
                    match strategy_ref.simulate_opportunity(&mock_opportunity, &context_clone).await {
                        Ok(result) => {
                            if result.success {
                                info!("🎯 Strategy {} simulation successful! Profit: {} wei", 
                                      strategy_ref.name, result.profit);
                            } else {
                                debug!("📉 Strategy {} simulation unprofitable", strategy_ref.name);
                            }
                            all_results.push(result);
                        },
                        Err(e) => {
                            warn!("❌ Strategy {} simulation failed: {}", strategy_ref.name, e);
                            all_results.push(ExecutionResult {
                                success: false,
                                profit: U256::ZERO,
                                gas_used: U256::from(500_000),
                                tx_hash: None,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                
                all_results
            })
        }).await;
        
        // Release permit (automatically done when permit is dropped)
        drop(permit);
        debug!("🔓 Released semaphore permit");
        
        match processing_result {
            Ok(results) => {
                info!("✅ MEV event processing completed with {} results", results.len());
                Ok(results)
            },
            Err(e) => {
                error!("❌ MEV event processing failed in spawn_blocking: {}", e);
                Err(anyhow::anyhow!("spawn_blocking failed: {}", e))
            }
        }
    }
    
    /// Extract serializable data from event for cross-thread usage
    fn extract_event_data(&self, event: &dyn MevEvent) -> EventData {
        EventData {
            event_type: event.event_type().to_string(),
            block_number: event.block_number(),
            transaction_index: event.transaction_index(),
        }
    }
}

/// Simplified strategy reference for async processing
#[derive(Debug, Clone)]
struct StrategyReference {
    name: String,
    config: StrategyConfig,
}

/// Serializable event data for cross-thread processing
#[derive(Debug, Clone)]
struct EventData {
    event_type: String,
    block_number: u64,
    transaction_index: Option<u64>,
}

/// Helper to create mock opportunity from event data (for testing semaphore pattern)
fn create_mock_opportunity_from_event(event_data: &EventData) -> MevOpportunity {
    // Based on event type, create appropriate mock opportunity
    match event_data.event_type.as_str() {
        "uniswap_v2_swap" | "uniswap_v3_swap" => {
            MevOpportunity::Arbitrage(ArbitrageOpportunity {
                token_in: Address::from([1; 20]),
                token_out: Address::from([2; 20]),
                pool_a: Address::from([10; 20]),
                pool_b: Address::from([11; 20]),
                amount_in: U256::from(1000) * U256::from(10).pow(U256::from(18)),
                expected_profit: U256::from(500_000),
                pool_variant_a: PoolVersion::UniswapV2,
                pool_variant_b: PoolVersion::UniswapV3,
                fee_a: 300,
                fee_b: 3000,
            })
        },
        _ => {
            // Default to arbitrage for testing
            MevOpportunity::Arbitrage(ArbitrageOpportunity {
                token_in: Address::from([1; 20]),
                token_out: Address::from([2; 20]),
                pool_a: Address::from([10; 20]),
                pool_b: Address::from([11; 20]),
                amount_in: U256::from(100) * U256::from(10).pow(U256::from(18)),
                expected_profit: U256::from(250_000),
                pool_variant_a: PoolVersion::UniswapV3,
                pool_variant_b: PoolVersion::UniswapV3,
                fee_a: 3000,
                fee_b: 500,
            })
        }
    }
}

impl StrategyReference {
    fn can_handle(&self, opportunity: &MevOpportunity) -> bool {
        match (&self.name[..], opportunity) {
            ("UniswapArbitrage", MevOpportunity::Arbitrage(_)) => true,
            ("Sandwich", MevOpportunity::Sandwich(_)) => true,
            ("Liquidation", MevOpportunity::Liquidation(_)) => true,
            _ => false,
        }
    }
    
    fn config(&self) -> &StrategyConfig {
        &self.config
    }
    
    // Mock methods for simulation - in real implementation, these would delegate to actual strategies
    async fn simulate_opportunity(&self, _opportunity: &MevOpportunity, _context: &ExecutionContext) -> Result<ExecutionResult> {
        // TODO: This should delegate to actual strategy simulation
        Ok(ExecutionResult {
            success: true,
            profit: U256::from(500_000),
            gas_used: U256::from(200_000),
            tx_hash: None,
            error: None,
        })
    }
}
