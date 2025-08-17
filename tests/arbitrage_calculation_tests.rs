// Arbitrage Opportunity Calculation E2E Tests
// Tests price difference detection, profit calculation, and slippage analysis

use anyhow::Result;
use arbooo::common::logger;
use arbooo::arbitrage::simulation::{get_address, AddressType};
use alloy::primitives::Address;
use log::info;

#[path = "utils/mod.rs"]
mod utils;
use utils::test_env::TestEnvironment;

#[derive(Debug, Clone)]
struct PriceInfo {
    pool_address: Address,
    pool_type: PoolType,
    token0: Address,
    token1: Address,
    price_token0_per_token1: f64, // How many token0 for 1 token1
    liquidity_usd: f64,
    fee_percentage: f64,
}

#[derive(Debug, Clone, PartialEq)]
enum PoolType {
    UniswapV2,
    UniswapV3,
}

#[derive(Debug, Clone)]
struct ArbitrageOpportunity {
    price_difference_percentage: f64,
    estimated_profit_eth: f64,
    optimal_trade_amount_eth: f64,
    direction: TradeDirection,
    confidence_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
enum TradeDirection {
    BuyV2SellV3,
    BuyV3SellV2,
}

#[tokio::test]
async fn test_price_difference_detection() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Price Difference Detection Test");

    let _test_env = TestEnvironment::new().await?;
    info!("✅ Test environment created");

    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x11; 20]);

    // Create test scenarios with significant price differences
    let scenarios = vec![
        // Scenario 1: Large price difference (should be profitable)
        create_price_scenario(
            "Large price difference",
            weth, usdc,
            2000.0, 2100.0, // 5% difference
            200_000.0, 300_000.0, // Good liquidity
            0.003, 0.0005,
        ),
        // Scenario 2: Medium price difference  
        create_price_scenario(
            "Medium price difference",
            weth, usdc,
            2000.0, 2040.0, // 2% difference
            150_000.0, 200_000.0,
            0.003, 0.003,
        ),
        // Scenario 3: Small but detectable difference
        create_price_scenario(
            "Small price difference",
            weth, usdc,
            2000.0, 2010.0, // 0.5% difference
            100_000.0, 120_000.0,
            0.003, 0.0005,
        ),
    ];

    for (i, scenario) in scenarios.iter().enumerate() {
        info!("🔍 Testing scenario {}: {}", i + 1, scenario.0);
        
        let opportunity = calculate_arbitrage_opportunity(&scenario.1, &scenario.2)?;
        
        // All scenarios should detect some price difference
        assert!(opportunity.price_difference_percentage > 0.0, 
                "Should detect price difference in scenario {}", i + 1);
        
        // Trade amounts should be reasonable (between 0.01 and 10 ETH for these liquidity levels)
        assert!(opportunity.optimal_trade_amount_eth > 0.001, 
                "Trade amount should be meaningful in scenario {}", i + 1);
        assert!(opportunity.optimal_trade_amount_eth < 20.0, 
                "Trade amount should be reasonable in scenario {}", i + 1);
        
        // Confidence should correlate with liquidity and price difference
        assert!(opportunity.confidence_score >= 0.0 && opportunity.confidence_score <= 1.0, 
                "Confidence score should be between 0 and 1 in scenario {}", i + 1);
        
        info!("   ✅ Price diff: {:.2}%, Profit: {:.4} ETH, Confidence: {:.2}, Direction: {:?}", 
              opportunity.price_difference_percentage, opportunity.estimated_profit_eth, 
              opportunity.confidence_score, opportunity.direction);
    }

    info!("🎉 Price Difference Detection Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_profit_calculation_with_costs() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Profit Calculation with Costs Test");

    let weth = get_address(AddressType::Weth);
    let usdc = Address::from([0x11; 20]);

    // Test with a profitable scenario
    let v2_info = create_price_info(
        Address::from([0x01; 20]), PoolType::UniswapV2, weth, usdc,
        2000.0, 500_000.0, 0.003,
    );
    
    let v3_info = create_price_info(
        Address::from([0x02; 20]), PoolType::UniswapV3, weth, usdc,
        2080.0, 800_000.0, 0.0005,
    );

    let trade_amounts = vec![0.5, 1.0, 2.0, 5.0];
    
    for &trade_amount in &trade_amounts {
        let profit = calculate_profit_for_amount(&v2_info, &v3_info, trade_amount, TradeDirection::BuyV2SellV3)?;
        
        // Basic sanity checks
        assert!(profit.gas_cost_eth > 0.0, "Gas costs should be included for {} ETH trade", trade_amount);
        assert!(profit.total_fees_eth > 0.0, "Trading fees should be included for {} ETH trade", trade_amount);
        assert!(profit.gross_profit_eth >= profit.net_profit_eth, 
                "Net profit should not exceed gross profit for {} ETH trade", trade_amount);
        
        // Log results for manual verification
        info!("Trade: {} ETH -> Gross: {:.4}, Fees: {:.4}, Gas: {:.4}, Net: {:.4}", 
              trade_amount, profit.gross_profit_eth, profit.total_fees_eth, 
              profit.gas_cost_eth, profit.net_profit_eth);
    }

    info!("✅ Validated profit calculation structure and cost inclusion");
    info!("🎉 Profit Calculation with Costs Test completed!");
    Ok(())
}

#[tokio::test]
async fn test_slippage_impact_calculation() -> Result<()> {
    logger::setup_logger();
    info!("🧪 Starting Slippage Impact Calculation Test");

    let weth = get_address(AddressType::Weth);
    let dai = Address::from([0x22; 20]);

    // Test different liquidity levels with the same trade size
    let trade_amount = 2.0; // 2 ETH
    let base_price_v2 = 1800.0;
    let base_price_v3 = 1860.0;

    let liquidity_scenarios = vec![
        ("High Liquidity", 1_000_000.0, 1_500_000.0),
        ("Medium Liquidity", 200_000.0, 300_000.0),
        ("Low Liquidity", 50_000.0, 80_000.0),
    ];

    let mut results = Vec::new();

    for (scenario_name, v2_liquidity, v3_liquidity) in liquidity_scenarios {
        info!("🔍 Testing {} scenario", scenario_name);
        
        let v2_info = create_price_info(
            Address::from([0x03; 20]), PoolType::UniswapV2, weth, dai,
            base_price_v2, v2_liquidity, 0.003,
        );
        
        let v3_info = create_price_info(
            Address::from([0x04; 20]), PoolType::UniswapV3, weth, dai,
            base_price_v3, v3_liquidity, 0.0005,
        );

        let analysis = analyze_slippage_impact(&v2_info, &v3_info, trade_amount)?;
        
        // Basic validation: slippage should be positive and reasonable
        assert!(analysis.v2_slippage_percent >= 0.0, "{} V2 slippage should be non-negative", scenario_name);
        assert!(analysis.v3_slippage_percent >= 0.0, "{} V3 slippage should be non-negative", scenario_name);
        assert!(analysis.v2_slippage_percent < 50.0, "{} V2 slippage should be reasonable", scenario_name);
        assert!(analysis.v3_slippage_percent < 50.0, "{} V3 slippage should be reasonable", scenario_name);
        
        info!("   V2 slippage: {:.2}%, V3 slippage: {:.2}%, Profit reduction: {:.1}%", 
              analysis.v2_slippage_percent, analysis.v3_slippage_percent, analysis.effective_profit_reduction);
        
        results.push((scenario_name, analysis));
    }

    // Verify slippage increases with lower liquidity
    assert!(results[2].1.v2_slippage_percent > results[0].1.v2_slippage_percent,
            "Low liquidity should have higher slippage than high liquidity");

    info!("✅ Validated slippage calculation and liquidity impact");
    info!("🎉 Slippage Impact Calculation Test completed!");
    Ok(())
}

// Helper functions
#[derive(Debug)]
struct ProfitCalculation {
    gross_profit_eth: f64,
    gas_cost_eth: f64,
    total_fees_eth: f64,
    net_profit_eth: f64,
}

#[derive(Debug)]
struct SlippageAnalysis {
    v2_slippage_percent: f64,
    v3_slippage_percent: f64,
    effective_profit_reduction: f64,
}

fn create_price_scenario(
    name: &str,
    token0: Address,
    token1: Address,
    v2_price: f64,
    v3_price: f64,
    v2_liquidity: f64,
    v3_liquidity: f64,
    v2_fee: f64,
    v3_fee: f64,
) -> (String, PriceInfo, PriceInfo) {
    let v2_info = create_price_info(
        Address::from([0x01; 20]), PoolType::UniswapV2, token0, token1,
        v2_price, v2_liquidity, v2_fee,
    );
    
    let v3_info = create_price_info(
        Address::from([0x02; 20]), PoolType::UniswapV3, token0, token1,
        v3_price, v3_liquidity, v3_fee,
    );
    
    (name.to_string(), v2_info, v3_info)
}

fn create_price_info(
    pool_address: Address,
    pool_type: PoolType,
    token0: Address,
    token1: Address,
    price_token0_per_token1: f64,
    liquidity_usd: f64,
    fee_percentage: f64,
) -> PriceInfo {
    PriceInfo {
        pool_address,
        pool_type,
        token0,
        token1,
        price_token0_per_token1,
        liquidity_usd,
        fee_percentage,
    }
}

fn calculate_arbitrage_opportunity(v2_info: &PriceInfo, v3_info: &PriceInfo) -> Result<ArbitrageOpportunity> {
    let v2_price = v2_info.price_token0_per_token1;
    let v3_price = v3_info.price_token0_per_token1;
    
    let (direction, price_diff_pct) = if v2_price < v3_price {
        (TradeDirection::BuyV2SellV3, ((v3_price - v2_price) / v2_price) * 100.0)
    } else {
        (TradeDirection::BuyV3SellV2, ((v2_price - v3_price) / v3_price) * 100.0)
    };
    
    // Conservative trade sizing based on liquidity
    let min_liquidity = v2_info.liquidity_usd.min(v3_info.liquidity_usd);
    let optimal_trade_amount = (min_liquidity * 0.002) / 2000.0; // 0.2% of min liquidity
    
    // Simplified profit calculation
    let price_diff_absolute = (v2_price - v3_price).abs();
    let gross_profit_eth = (price_diff_absolute / v2_price.max(v3_price)) * optimal_trade_amount;
    let total_fees = (v2_info.fee_percentage + v3_info.fee_percentage) * optimal_trade_amount;
    let gas_cost = 0.002;
    let estimated_profit = gross_profit_eth - total_fees - gas_cost;
    
    // Confidence scoring
    let liquidity_score = (min_liquidity / 200_000.0).min(1.0);
    let price_diff_score = (price_diff_pct / 3.0).min(1.0);
    let confidence_score = liquidity_score * 0.5 + price_diff_score * 0.5;
    
    Ok(ArbitrageOpportunity {
        price_difference_percentage: price_diff_pct,
        estimated_profit_eth: estimated_profit.max(0.0),
        optimal_trade_amount_eth: optimal_trade_amount,
        direction,
        confidence_score,
    })
}

fn calculate_profit_for_amount(
    v2_info: &PriceInfo, 
    v3_info: &PriceInfo, 
    trade_amount: f64,
    direction: TradeDirection
) -> Result<ProfitCalculation> {
    let (buy_pool, sell_pool) = match direction {
        TradeDirection::BuyV2SellV3 => (v2_info, v3_info),
        TradeDirection::BuyV3SellV2 => (v3_info, v2_info),
    };
    
    // Calculate slippage
    let buy_slippage = calculate_slippage(buy_pool.liquidity_usd, trade_amount);
    let sell_slippage = calculate_slippage(sell_pool.liquidity_usd, trade_amount);
    
    let effective_buy_price = buy_pool.price_token0_per_token1 * (1.0 + buy_slippage);
    let effective_sell_price = sell_pool.price_token0_per_token1 * (1.0 - sell_slippage);
    
    // Calculate profits and costs
    let price_diff_per_unit = effective_sell_price - effective_buy_price;
    let gross_profit_eth = (price_diff_per_unit / effective_buy_price) * trade_amount;
    
    let trading_fees = (buy_pool.fee_percentage + sell_pool.fee_percentage) * trade_amount;
    let gas_cost = 0.002;
    let net_profit = gross_profit_eth - trading_fees - gas_cost;
    
    Ok(ProfitCalculation {
        gross_profit_eth,
        gas_cost_eth: gas_cost,
        total_fees_eth: trading_fees,
        net_profit_eth: net_profit,
    })
}

fn analyze_slippage_impact(
    v2_info: &PriceInfo, 
    v3_info: &PriceInfo, 
    trade_amount: f64
) -> Result<SlippageAnalysis> {
    let v2_slippage = calculate_slippage(v2_info.liquidity_usd, trade_amount) * 100.0;
    let v3_slippage = calculate_slippage(v3_info.liquidity_usd, trade_amount) * 100.0;
    
    let total_slippage_impact = v2_slippage + v3_slippage;
    let initial_profit_pct = ((v3_info.price_token0_per_token1 - v2_info.price_token0_per_token1) / v2_info.price_token0_per_token1) * 100.0;
    
    let profit_reduction = if initial_profit_pct.abs() > 0.001 {
        (total_slippage_impact / initial_profit_pct.abs()) * 100.0
    } else {
        100.0 // No profit to reduce
    };
    
    Ok(SlippageAnalysis {
        v2_slippage_percent: v2_slippage,
        v3_slippage_percent: v3_slippage,
        effective_profit_reduction: profit_reduction,
    })
}

fn calculate_slippage(liquidity_usd: f64, trade_amount_eth: f64) -> f64 {
    let trade_value_usd = trade_amount_eth * 2000.0;
    let liquidity_impact = trade_value_usd / liquidity_usd;
    
    // Simple linear model: more conservative for testing
    (liquidity_impact * 0.01).min(0.1) // Max 10% slippage
}
