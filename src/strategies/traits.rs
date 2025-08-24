use async_trait::async_trait;
use anyhow::Result;
use alloy_primitives::U256;
use revm::primitives::Address;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tokio::sync::broadcast;

/// Generic MEV opportunity that can represent different types of strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MevOpportunity {
    /// Arbitrage between different DEX pools
    Arbitrage(ArbitrageOpportunity),
    /// Sandwich attack opportunity
    Sandwich(SandwichOpportunity),
    /// Liquidation opportunity
    Liquidation(LiquidationOpportunity),
    /// Custom strategy opportunity
    Custom {
        strategy_type: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub token_in: Address,
    pub token_out: Address,
    pub pool_a: Address,
    pub pool_b: Address,
    pub amount_in: U256,
    pub expected_profit: U256,
    pub pool_variant_a: PoolVersion,
    pub pool_variant_b: PoolVersion,
    pub fee_a: u32,
    pub fee_b: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichOpportunity {
    pub target_tx_hash: String,
    pub token_in: Address,
    pub token_out: Address,
    pub pool: Address,
    pub frontrun_amount: U256,
    pub expected_profit: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationOpportunity {
    pub protocol: String,
    pub borrower: Address,
    pub collateral_token: Address,
    pub debt_token: Address,
    pub liquidation_amount: U256,
    pub expected_profit: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolVersion {
    UniswapV2,
    UniswapV3,
    SushiswapV2,
    BalancerV2,
    CurveV1,
}

/// Result of executing an MEV strategy
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub profit: U256,
    pub gas_used: U256,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
}

/// Configuration for a strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub enabled: bool,
    pub max_gas_price: U256,
    pub min_profit_threshold: U256,
    pub max_position_size: U256,
    pub priority: u8, // Higher number = higher priority
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_gas_price: U256::from(100_000_000_000u64), // 100 gwei
            min_profit_threshold: U256::from(100_000u128),
            max_position_size: U256::from(1000) * U256::from(10).pow(U256::from(18)),
            priority: 50,
        }
    }
}

/// Generic trait that all MEV strategies must implement
#[async_trait]
pub trait MevStrategy: Send + Sync + Debug {
    /// Name of the strategy
    fn name(&self) -> &str;
    
    /// Strategy configuration
    fn config(&self) -> &StrategyConfig;
    
    /// Update strategy configuration
    fn update_config(&mut self, config: StrategyConfig);
    
    /// Scan for opportunities based on incoming events
    async fn scan_opportunities(
        &self,
        event: &dyn MevEvent,
    ) -> Result<Vec<MevOpportunity>>;
    
    /// Simulate the execution of an opportunity
    async fn simulate_opportunity(
        &self,
        opportunity: &MevOpportunity,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult>;
    
    /// Execute the opportunity if profitable
    async fn execute_opportunity(
        &self,
        opportunity: &MevOpportunity,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult>;
    
    /// Check if this strategy can handle the given opportunity type
    fn can_handle(&self, opportunity: &MevOpportunity) -> bool;
}

/// Context for strategy execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub block_number: u64,
    pub gas_price: U256,
    pub base_fee: U256,
    pub executor_address: Address,
    pub max_gas_limit: u64,
}

/// Generic event trait that different event types can implement
pub trait MevEvent: Send + Sync + Debug + std::any::Any {
    fn event_type(&self) -> &str;
    fn block_number(&self) -> u64;
    fn transaction_index(&self) -> Option<u64>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn clone_boxed(&self) -> Box<dyn MevEvent>;
}

/// Strategy factory for creating different strategy instances
pub trait StrategyFactory: Send + Sync {
    fn create_strategy(&self, strategy_type: &str, config: StrategyConfig) -> Result<Box<dyn MevStrategy>>;
    fn supported_strategies(&self) -> Vec<String>;
}

/// Priority queue for managing opportunities
#[derive(Debug)]
pub struct OpportunityQueue {
    opportunities: Vec<(MevOpportunity, u8)>, // (opportunity, priority)
}

impl OpportunityQueue {
    pub fn new() -> Self {
        Self {
            opportunities: Vec::new(),
        }
    }
    
    pub fn push(&mut self, opportunity: MevOpportunity, priority: u8) {
        self.opportunities.push((opportunity, priority));
        // Sort by priority (higher priority first)
        self.opportunities.sort_by(|a, b| b.1.cmp(&a.1));
    }
    
    pub fn pop(&mut self) -> Option<MevOpportunity> {
        self.opportunities.pop().map(|(opp, _)| opp)
    }
    
    pub fn len(&self) -> usize {
        self.opportunities.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.opportunities.is_empty()
    }
}

/// Channel for broadcasting opportunities between strategies
pub type OpportunityBroadcaster = broadcast::Sender<MevOpportunity>;
pub type OpportunityReceiver = broadcast::Receiver<MevOpportunity>;
