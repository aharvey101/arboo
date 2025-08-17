#![allow(dead_code)]
#![allow(unused_imports)]

// Test Data Fixtures and Scenarios
// Provides predefined test data for consistent and comprehensive testing

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Comprehensive test scenario that combines multiple components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub scenario_type: ScenarioType,
    pub setup: ScenarioSetup,
    pub expected_outcomes: ExpectedOutcomes,
    pub cleanup: Option<ScenarioCleanup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScenarioType {
    ProfitableArbitrage,
    UnprofitableArbitrage,
    HighSlippage,
    LowLiquidity,
    GasSpike,
    NetworkCongestion,
    MEVCompetition,
    FlashLoanFailure,
    EdgeCase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSetup {
    pub initial_block: u64,
    pub tokens: Vec<TestTokenConfig>,
    pub pools: Vec<TestPoolConfig>,
    pub market_conditions: MarketConditions,
    pub gas_conditions: GasConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTokenConfig {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub total_supply: String, // String to handle large numbers
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPoolConfig {
    pub pool_type: String, // "UniswapV2" or "UniswapV3"
    pub address: String,
    pub token0: String,
    pub token1: String,
    pub fee: u32,
    pub reserves: PoolReserves,
    pub price: String, // Current price as string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolReserves {
    pub token0_reserve: String,
    pub token1_reserve: String,
    pub liquidity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    pub volatility: f64, // 0.0 to 1.0
    pub trend: MarketTrend,
    pub liquidity_factor: f64, // multiplier for base liquidity
    pub price_impact_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Sideways,
    Volatile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasConditions {
    pub base_fee: u64, // in wei
    pub max_fee: u64,
    pub priority_fee: u64,
    pub gas_limit: u64,
    pub congestion_level: CongestionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CongestionLevel {
    Low,     // Fast confirmation
    Medium,  // Normal confirmation
    High,    // Slow confirmation
    Extreme, // Very slow confirmation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutcomes {
    pub should_detect_opportunity: bool,
    pub expected_profit_range: Option<(f64, f64)>, // min, max profit in USD
    pub should_execute_transaction: bool,
    pub expected_gas_used: Option<u64>,
    pub max_execution_time_ms: u64,
    pub risk_assessment: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Extreme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCleanup {
    pub reset_pools: bool,
    pub reset_balances: bool,
    pub reset_gas_conditions: bool,
}

/// Predefined test scenarios
pub struct PredefinedScenarios;

impl PredefinedScenarios {
    /// High-profit arbitrage opportunity (ideal conditions)
    pub fn profitable_weth_usdc_arbitrage() -> TestScenario {
        TestScenario {
            name: "profitable_weth_usdc_arbitrage".to_string(),
            description: "Clear arbitrage opportunity between WETH/USDC pools with significant price difference".to_string(),
            scenario_type: ScenarioType::ProfitableArbitrage,
            setup: ScenarioSetup {
                initial_block: 18_500_000,
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WETH".to_string(),
                        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000".to_string(),
                    },
                    TestTokenConfig {
                        symbol: "USDC".to_string(),
                        address: "0xA0b86a33E6411c1F94d1F6b7E5c5c47f3e2d8B23".to_string(),
                        decimals: 6,
                        total_supply: "1000000000000".to_string(),
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0x123...v2".to_string(),
                        token0: "WETH".to_string(),
                        token1: "USDC".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "1000000000000000000000".to_string(), // 1000 WETH
                            token1_reserve: "3000000000000".to_string(), // 3M USDC
                            liquidity: "54772255750516612".to_string(),
                        },
                        price: "3000.0".to_string(), // $3000 per ETH
                    },
                    TestPoolConfig {
                        pool_type: "UniswapV3".to_string(),
                        address: "0x456...v3".to_string(),
                        token0: "WETH".to_string(),
                        token1: "USDC".to_string(),
                        fee: 500,
                        reserves: PoolReserves {
                            token0_reserve: "500000000000000000000".to_string(), // 500 WETH
                            token1_reserve: "1530000000000".to_string(), // 1.53M USDC
                            liquidity: "38729833462074168851".to_string(),
                        },
                        price: "3060.0".to_string(), // $3060 per ETH (2% difference)
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 0.3,
                    trend: MarketTrend::Bullish,
                    liquidity_factor: 1.0,
                    price_impact_factor: 0.02,
                },
                gas_conditions: GasConditions {
                    base_fee: 20_000_000_000, // 20 gwei
                    max_fee: 50_000_000_000,  // 50 gwei
                    priority_fee: 2_000_000_000, // 2 gwei
                    gas_limit: 500_000,
                    congestion_level: CongestionLevel::Low,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: true,
                expected_profit_range: Some((50.0, 200.0)), // $50-$200 profit
                should_execute_transaction: true,
                expected_gas_used: Some(350_000),
                max_execution_time_ms: 5000,
                risk_assessment: RiskLevel::Low,
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }

    /// Low liquidity scenario that should be avoided
    pub fn low_liquidity_scenario() -> TestScenario {
        TestScenario {
            name: "low_liquidity_scenario".to_string(),
            description: "High slippage scenario due to low liquidity pools".to_string(),
            scenario_type: ScenarioType::LowLiquidity,
            setup: ScenarioSetup {
                initial_block: 18_500_100,
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WETH".to_string(),
                        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000".to_string(),
                    },
                    TestTokenConfig {
                        symbol: "RARE".to_string(),
                        address: "0x12345678901234567890123456789012345678ab".to_string(),
                        decimals: 18,
                        total_supply: "100000000000000000000000".to_string(),
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0x789...v2low".to_string(),
                        token0: "WETH".to_string(),
                        token1: "RARE".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "10000000000000000000".to_string(), // 10 WETH
                            token1_reserve: "1000000000000000000000".to_string(), // 1000 RARE
                            liquidity: "31622776601683793319".to_string(),
                        },
                        price: "100.0".to_string(),
                    },
                    TestPoolConfig {
                        pool_type: "UniswapV3".to_string(),
                        address: "0xabc...v3low".to_string(),
                        token0: "WETH".to_string(),
                        token1: "RARE".to_string(),
                        fee: 3000,
                        reserves: PoolReserves {
                            token0_reserve: "5000000000000000000".to_string(), // 5 WETH
                            token1_reserve: "520000000000000000000".to_string(), // 520 RARE
                            liquidity: "16124515496597247758".to_string(),
                        },
                        price: "104.0".to_string(), // 4% difference but low liquidity
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 0.8,
                    trend: MarketTrend::Volatile,
                    liquidity_factor: 0.1, // Very low liquidity
                    price_impact_factor: 0.15, // High price impact
                },
                gas_conditions: GasConditions {
                    base_fee: 25_000_000_000,
                    max_fee: 60_000_000_000,
                    priority_fee: 3_000_000_000,
                    gas_limit: 500_000,
                    congestion_level: CongestionLevel::Medium,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: true, // Detects but shouldn't execute
                expected_profit_range: Some((-50.0, 10.0)), // Likely unprofitable
                should_execute_transaction: false, // Too risky
                expected_gas_used: None,
                max_execution_time_ms: 3000,
                risk_assessment: RiskLevel::High,
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }

    /// Gas price spike scenario
    pub fn gas_price_spike() -> TestScenario {
        TestScenario {
            name: "gas_price_spike".to_string(),
            description: "Network congestion causing gas prices to spike during arbitrage".to_string(),
            scenario_type: ScenarioType::GasSpike,
            setup: ScenarioSetup {
                initial_block: 18_500_200,
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WETH".to_string(),
                        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000".to_string(),
                    },
                    TestTokenConfig {
                        symbol: "DAI".to_string(),
                        address: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000000".to_string(),
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0xdef...v2gas".to_string(),
                        token0: "WETH".to_string(),
                        token1: "DAI".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "800000000000000000000".to_string(), // 800 WETH
                            token1_reserve: "2400000000000000000000000".to_string(), // 2.4M DAI
                            liquidity: "43817805383034611231".to_string(),
                        },
                        price: "3000.0".to_string(),
                    },
                    TestPoolConfig {
                        pool_type: "UniswapV3".to_string(),
                        address: "0x321...v3gas".to_string(),
                        token0: "WETH".to_string(),
                        token1: "DAI".to_string(),
                        fee: 500,
                        reserves: PoolReserves {
                            token0_reserve: "600000000000000000000".to_string(), // 600 WETH
                            token1_reserve: "1845000000000000000000000".to_string(), // 1.845M DAI
                            liquidity: "33281279921715669949".to_string(),
                        },
                        price: "3075.0".to_string(), // 2.5% difference
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 0.4,
                    trend: MarketTrend::Bullish,
                    liquidity_factor: 1.0,
                    price_impact_factor: 0.03,
                },
                gas_conditions: GasConditions {
                    base_fee: 150_000_000_000, // 150 gwei (very high)
                    max_fee: 300_000_000_000,  // 300 gwei
                    priority_fee: 20_000_000_000, // 20 gwei
                    gas_limit: 500_000,
                    congestion_level: CongestionLevel::Extreme,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: true,
                expected_profit_range: Some((75.0, 150.0)), // Good profit but high gas
                should_execute_transaction: false, // Gas too expensive
                expected_gas_used: Some(400_000),
                max_execution_time_ms: 8000, // Slower due to congestion
                risk_assessment: RiskLevel::High,
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }

    /// MEV competition scenario
    pub fn mev_competition() -> TestScenario {
        TestScenario {
            name: "mev_competition".to_string(),
            description: "Multiple arbitrageurs competing for the same opportunity".to_string(),
            scenario_type: ScenarioType::MEVCompetition,
            setup: ScenarioSetup {
                initial_block: 18_500_300,
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WETH".to_string(),
                        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000".to_string(),
                    },
                    TestTokenConfig {
                        symbol: "USDC".to_string(),
                        address: "0xA0b86a33E6411c1F94d1F6b7E5c5c47f3e2d8B23".to_string(),
                        decimals: 6,
                        total_supply: "1000000000000".to_string(),
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0x654...v2mev".to_string(),
                        token0: "WETH".to_string(),
                        token1: "USDC".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "1200000000000000000000".to_string(), // 1200 WETH
                            token1_reserve: "3600000000000".to_string(), // 3.6M USDC
                            liquidity: "65727320076075600".to_string(),
                        },
                        price: "3000.0".to_string(),
                    },
                    TestPoolConfig {
                        pool_type: "UniswapV3".to_string(),
                        address: "0x987...v3mev".to_string(),
                        token0: "WETH".to_string(),
                        token1: "USDC".to_string(),
                        fee: 500,
                        reserves: PoolReserves {
                            token0_reserve: "700000000000000000000".to_string(), // 700 WETH
                            token1_reserve: "2184000000000".to_string(), // 2.184M USDC
                            liquidity: "44159041652537165527".to_string(),
                        },
                        price: "3120.0".to_string(), // 4% difference (big opportunity)
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 0.5,
                    trend: MarketTrend::Volatile,
                    liquidity_factor: 1.2, // Good liquidity
                    price_impact_factor: 0.02,
                },
                gas_conditions: GasConditions {
                    base_fee: 30_000_000_000, // 30 gwei
                    max_fee: 100_000_000_000, // 100 gwei
                    priority_fee: 15_000_000_000, // 15 gwei (high due to competition)
                    gas_limit: 500_000,
                    congestion_level: CongestionLevel::High,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: true,
                expected_profit_range: Some((100.0, 300.0)), // Large opportunity
                should_execute_transaction: true,
                expected_gas_used: Some(380_000),
                max_execution_time_ms: 2000, // Must be fast to win
                risk_assessment: RiskLevel::Medium, // Competitive but profitable
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }

    /// Edge case: Extreme decimal differences
    pub fn extreme_decimal_differences() -> TestScenario {
        TestScenario {
            name: "extreme_decimal_differences".to_string(),
            description: "Testing with tokens that have very different decimal places".to_string(),
            scenario_type: ScenarioType::EdgeCase,
            setup: ScenarioSetup {
                initial_block: 18_500_400,
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WBTC".to_string(),
                        address: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(),
                        decimals: 8, // Bitcoin has 8 decimals
                        total_supply: "2100000000000000".to_string(), // 21M WBTC
                    },
                    TestTokenConfig {
                        symbol: "SHIB".to_string(),
                        address: "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE".to_string(),
                        decimals: 18, // High decimal token
                        total_supply: "1000000000000000000000000000000000".to_string(), // 1 quadrillion
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0xaaa...v2decimal".to_string(),
                        token0: "WBTC".to_string(),
                        token1: "SHIB".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "100000000".to_string(), // 1 WBTC (8 decimals)
                            token1_reserve: "4000000000000000000000000000".to_string(), // 4B SHIB
                            liquidity: "20000000000000000".to_string(),
                        },
                        price: "40000000000000000000000000.0".to_string(), // SHIB per WBTC
                    },
                    TestPoolConfig {
                        pool_type: "UniswapV3".to_string(),
                        address: "0xbbb...v3decimal".to_string(),
                        token0: "WBTC".to_string(),
                        token1: "SHIB".to_string(),
                        fee: 3000,
                        reserves: PoolReserves {
                            token0_reserve: "50000000".to_string(), // 0.5 WBTC
                            token1_reserve: "2040000000000000000000000000".to_string(), // 2.04B SHIB
                            liquidity: "14142135623730950488".to_string(),
                        },
                        price: "40800000000000000000000000.0".to_string(), // 2% difference
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 0.9, // Very volatile due to SHIB
                    trend: MarketTrend::Volatile,
                    liquidity_factor: 0.3, // Lower liquidity for this exotic pair
                    price_impact_factor: 0.08,
                },
                gas_conditions: GasConditions {
                    base_fee: 25_000_000_000,
                    max_fee: 60_000_000_000,
                    priority_fee: 3_000_000_000,
                    gas_limit: 600_000, // Higher gas for complex calculations
                    congestion_level: CongestionLevel::Medium,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: true,
                expected_profit_range: Some((5.0, 25.0)), // Smaller profit due to complexity
                should_execute_transaction: true,
                expected_gas_used: Some(450_000),
                max_execution_time_ms: 7000, // Slower due to complex math
                risk_assessment: RiskLevel::Medium,
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }

    /// Get all predefined scenarios
    pub fn all_scenarios() -> Vec<TestScenario> {
        vec![
            Self::profitable_weth_usdc_arbitrage(),
            Self::low_liquidity_scenario(),
            Self::gas_price_spike(),
            Self::mev_competition(),
            Self::extreme_decimal_differences(),
        ]
    }

    /// Get scenarios by type
    pub fn scenarios_by_type(scenario_type: ScenarioType) -> Vec<TestScenario> {
        Self::all_scenarios()
            .into_iter()
            .filter(|s| std::mem::discriminant(&s.scenario_type) == std::mem::discriminant(&scenario_type))
            .collect()
    }
}

/// Historical market data replays
pub struct HistoricalReplays;

impl HistoricalReplays {
    /// Black Thursday (March 12, 2020) - crypto market crash
    pub fn black_thursday_crash() -> TestScenario {
        TestScenario {
            name: "black_thursday_crash".to_string(),
            description: "Recreating the market conditions during the March 2020 crypto crash".to_string(),
            scenario_type: ScenarioType::EdgeCase,
            setup: ScenarioSetup {
                initial_block: 9796778, // Around Black Thursday block
                tokens: vec![
                    TestTokenConfig {
                        symbol: "WETH".to_string(),
                        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000".to_string(),
                    },
                    TestTokenConfig {
                        symbol: "DAI".to_string(),
                        address: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
                        decimals: 18,
                        total_supply: "1000000000000000000000000000".to_string(),
                    },
                ],
                pools: vec![
                    TestPoolConfig {
                        pool_type: "UniswapV2".to_string(),
                        address: "0xhistorical1".to_string(),
                        token0: "WETH".to_string(),
                        token1: "DAI".to_string(),
                        fee: 300,
                        reserves: PoolReserves {
                            token0_reserve: "200000000000000000000".to_string(), // 200 WETH
                            token1_reserve: "32000000000000000000000".to_string(), // 32k DAI (ETH crashed to $160)
                            liquidity: "8000000000000000000000".to_string(),
                        },
                        price: "160.0".to_string(), // Crashed price
                    },
                ],
                market_conditions: MarketConditions {
                    volatility: 1.0, // Maximum volatility
                    trend: MarketTrend::Bearish,
                    liquidity_factor: 0.2, // Liquidity dried up
                    price_impact_factor: 0.25, // Massive slippage
                },
                gas_conditions: GasConditions {
                    base_fee: 100_000_000_000, // 100 gwei (network congested from panic)
                    max_fee: 500_000_000_000,
                    priority_fee: 50_000_000_000,
                    gas_limit: 500_000,
                    congestion_level: CongestionLevel::Extreme,
                },
            },
            expected_outcomes: ExpectedOutcomes {
                should_detect_opportunity: false, // Too chaotic
                expected_profit_range: None,
                should_execute_transaction: false,
                expected_gas_used: None,
                max_execution_time_ms: 10000,
                risk_assessment: RiskLevel::Extreme,
            },
            cleanup: Some(ScenarioCleanup {
                reset_pools: true,
                reset_balances: true,
                reset_gas_conditions: true,
            }),
        }
    }
}

/// Utility functions for working with test scenarios
impl TestScenario {
    /// Save scenario to JSON file
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load scenario from JSON file
    pub fn load_from_file(path: &str) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let scenario = serde_json::from_str(&json)?;
        Ok(scenario)
    }

    /// Get estimated execution time based on scenario complexity
    pub fn estimated_execution_time(&self) -> u64 {
        let base_time = 1000; // 1 second base
        let complexity_factor = match self.scenario_type {
            ScenarioType::ProfitableArbitrage => 1.0,
            ScenarioType::LowLiquidity => 1.5,
            ScenarioType::GasSpike => 2.0,
            ScenarioType::MEVCompetition => 0.5, // Must be fast
            ScenarioType::EdgeCase => 3.0,
            _ => 1.0,
        };
        
        (base_time as f64 * complexity_factor) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predefined_scenarios() {
        let scenarios = PredefinedScenarios::all_scenarios();
        assert!(!scenarios.is_empty());
        
        for scenario in scenarios {
            assert!(!scenario.name.is_empty());
            assert!(!scenario.description.is_empty());
            assert!(!scenario.setup.tokens.is_empty());
            assert!(!scenario.setup.pools.is_empty());
        }
    }

    #[test]
    fn test_scenario_serialization() {
        let scenario = PredefinedScenarios::profitable_weth_usdc_arbitrage();
        let json = serde_json::to_string(&scenario).unwrap();
        let deserialized: TestScenario = serde_json::from_str(&json).unwrap();
        
        assert_eq!(scenario.name, deserialized.name);
        assert_eq!(scenario.description, deserialized.description);
    }

    #[test]
    fn test_scenarios_by_type() {
        let profitable = PredefinedScenarios::scenarios_by_type(ScenarioType::ProfitableArbitrage);
        assert!(!profitable.is_empty());
        
        let edge_cases = PredefinedScenarios::scenarios_by_type(ScenarioType::EdgeCase);
        assert!(!edge_cases.is_empty());
    }

    #[test]
    fn test_historical_replays() {
        let black_thursday = HistoricalReplays::black_thursday_crash();
        assert_eq!(black_thursday.name, "black_thursday_crash");
        assert_eq!(black_thursday.expected_outcomes.risk_assessment, RiskLevel::Extreme);
    }
}
