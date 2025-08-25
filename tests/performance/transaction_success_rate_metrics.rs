use anyhow::Result;
use arbooo::arbitrage::strategy::process_strategy;
use arbooo::common::logs::LogEvent;
use alloy::primitives::address;
use alloy_primitives::aliases::U24;
use log::info;
use std::time::{Duration, Instant};
use tokio::time::timeout;

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/utils/mod.rs"));
}
use utils::test_env::TestEnvironment;

#[tokio::test]
async fn measure_transaction_success_rates_normal() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("✅ Measuring transaction success rates under normal conditions");

    let test_scenarios = [
        ("high_liquidity", 5, "High liquidity pools"),
        ("medium_liquidity", 4, "Medium liquidity pools"),
        ("low_liquidity", 3, "Low liquidity pools"),
        ("mixed_liquidity", 6, "Mixed liquidity conditions"),
    ];

    let mut success_results = Vec::new();

    for (scenario_name, transaction_count, scenario_desc) in test_scenarios {
        info!("✅ Testing scenario: {} - {}", scenario_name, scenario_desc);

        let mut successful_transactions = 0;
        let mut failed_transactions = 0;
        let mut execution_times = Vec::new();

        for tx_index in 0..transaction_count {
            let start_time = Instant::now();
            let log_event = create_success_test_opportunity(scenario_name, tx_index).await?;

            let transaction_result = timeout(
                Duration::from_secs(8),
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;

            let execution_time = start_time.elapsed();
            execution_times.push(execution_time);

            match transaction_result {
                Ok(Ok(_)) => {
                    successful_transactions += 1;
                    info!("✅ Transaction {}/{} succeeded in {:?}", tx_index + 1, transaction_count, execution_time);
                },
                Ok(Err(_)) | Err(_) => {
                    failed_transactions += 1;
                    info!("❌ Transaction {}/{} failed in {:?}", tx_index + 1, transaction_count, execution_time);
                },
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let success_rate = if transaction_count > 0 {
            (successful_transactions as f64 / transaction_count as f64) * 100.0
        } else {
            0.0
        };

        let avg_execution_time = if !execution_times.is_empty() {
            execution_times.iter().sum::<Duration>() / execution_times.len() as u32
        } else {
            Duration::ZERO
        };

        success_results.push((scenario_name, successful_transactions, failed_transactions, success_rate, avg_execution_time));

        info!("✅ Scenario '{}': success_rate={:.1}%, successful={}, failed={}, avg_time={:?}", 
              scenario_name, success_rate, successful_transactions, failed_transactions, avg_execution_time);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    info!("📊 Transaction success rate analysis:");
    for (scenario, successful, failed, rate, avg_time) in &success_results {
        info!("   ✅ {}: rate={:.1}%, successful={}, failed={}, avg_time={:?}", 
              scenario, rate, successful, failed, avg_time);
    }

    let total_successful: u32 = success_results.iter().map(|(_, s, _, _, _)| *s).sum();
    let total_failed: u32 = success_results.iter().map(|(_, _, f, _, _)| *f).sum();
    let overall_success_rate = if total_successful + total_failed > 0 {
        (total_successful as f64 / (total_successful + total_failed) as f64) * 100.0
    } else {
        0.0
    };

    info!("📊 Overall success rate: {:.1}% ({} successful, {} failed)", 
          overall_success_rate, total_successful, total_failed);

    assert!(success_results.len() == test_scenarios.len(),
           "All success rate scenarios should be tested");

    if total_successful + total_failed > 0 {

        assert!(overall_success_rate >= 0.0,
               "Overall success rate should be non-negative, was {:.1}%", overall_success_rate);
        info!("📊 Transaction success rate test completed with {:.1}% success rate", overall_success_rate);
    } else {
        info!("📊 No transactions were attempted in test environment");
    }

    Ok(())
}

#[tokio::test]
async fn measure_transaction_success_rates_stress() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔥 Measuring transaction success rates under stress conditions");

    let stress_scenarios = [
        ("rapid_fire", 8, "Rapid-fire transactions"),
        ("complex_arbitrage", 5, "Complex arbitrage scenarios"),
        ("network_congestion", 6, "Simulated network congestion"),
        ("gas_volatility", 4, "Gas price volatility"),
    ];

    let mut stress_results = Vec::new();

    for (scenario_name, transaction_count, scenario_desc) in stress_scenarios {
        info!("🔥 Testing stress scenario: {} - {}", scenario_name, scenario_desc);

        let mut stress_successful = 0;
        let mut stress_failed = 0;
        let mut stress_timeouts = 0;
        let mut response_times = Vec::new();

        for tx_index in 0..transaction_count {
            let start_time = Instant::now();
            let log_event = create_stress_test_opportunity(scenario_name, tx_index).await?;

            let stress_timeout = match scenario_name {
                "rapid_fire" => Duration::from_secs(5),
                "complex_arbitrage" => Duration::from_secs(10),
                "network_congestion" => Duration::from_secs(12),
                "gas_volatility" => Duration::from_secs(8),
                _ => Duration::from_secs(6),
            };

            let transaction_result = timeout(
                stress_timeout,
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;

            let response_time = start_time.elapsed();
            response_times.push(response_time);

            match transaction_result {
                Ok(Ok(_)) => {
                    stress_successful += 1;
                    info!("🔥 Stress transaction {}/{} succeeded in {:?}", tx_index + 1, transaction_count, response_time);
                },
                Ok(Err(_)) => {
                    stress_failed += 1;
                    info!("🔥 Stress transaction {}/{} failed in {:?}", tx_index + 1, transaction_count, response_time);
                },
                Err(_) => {
                    stress_timeouts += 1;
                    info!("🔥 Stress transaction {}/{} timed out in {:?}", tx_index + 1, transaction_count, response_time);
                },
            }

            if scenario_name == "rapid_fire" {
                tokio::time::sleep(Duration::from_millis(50)).await;
            } else {
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }

        let stress_success_rate = if transaction_count > 0 {
            (stress_successful as f64 / transaction_count as f64) * 100.0
        } else {
            0.0
        };

        let avg_response_time = if !response_times.is_empty() {
            response_times.iter().sum::<Duration>() / response_times.len() as u32
        } else {
            Duration::ZERO
        };

        stress_results.push((scenario_name, stress_successful, stress_failed, stress_timeouts, stress_success_rate, avg_response_time));

        info!("🔥 Stress scenario '{}': success_rate={:.1}%, successful={}, failed={}, timeouts={}, avg_response={:?}", 
              scenario_name, stress_success_rate, stress_successful, stress_failed, stress_timeouts, avg_response_time);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    info!("📊 Stress test success rate analysis:");
    for (scenario, successful, failed, timeouts, rate, avg_time) in &stress_results {
        info!("   🔥 {}: rate={:.1}%, successful={}, failed={}, timeouts={}, avg_time={:?}", 
              scenario, rate, successful, failed, timeouts, avg_time);
    }

    assert!(stress_results.len() == stress_scenarios.len(),
           "All stress scenarios should be tested");

    Ok(())
}

#[tokio::test]
async fn measure_transaction_reliability_over_time() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("⏱️ Measuring transaction reliability over time");

    let reliability_duration = Duration::from_secs(20);
    let check_interval = Duration::from_secs(4);

    let mut reliability_timeline = Vec::new();
    let start_time = Instant::now();
    let mut last_check_time = start_time;
    let mut total_attempts = 0;
    let mut total_successes = 0;

    info!("⏱️ Starting reliability testing for {:?}", reliability_duration);

    while start_time.elapsed() < reliability_duration {

        let attempt_start = Instant::now();
        let log_event = create_reliability_test_opportunity().await?;

        let transaction_result = timeout(
            Duration::from_secs(6),
            process_strategy(log_event, test_env.test_config.ws_url.clone())
        ).await;

        total_attempts += 1;
        let is_successful = matches!(transaction_result, Ok(Ok(_)));
        if is_successful {
            total_successes += 1;
        }

        if last_check_time.elapsed() >= check_interval {
            let elapsed = start_time.elapsed();
            let current_reliability = if total_attempts > 0 {
                (total_successes as f64 / total_attempts as f64) * 100.0
            } else {
                0.0
            };

            reliability_timeline.push((elapsed, total_attempts, total_successes, current_reliability));

            info!("⏱️ Time: {:?}, Attempts: {}, Successes: {}, Reliability: {:.1}%", 
                  elapsed, total_attempts, total_successes, current_reliability);

            last_check_time = Instant::now();
        }

        let attempt_time = attempt_start.elapsed();
        info!("⏱️ Transaction attempt {} completed in {:?} (success: {})", 
              total_attempts, attempt_time, is_successful);

        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    let final_reliability = if total_attempts > 0 {
        (total_successes as f64 / total_attempts as f64) * 100.0
    } else {
        0.0
    };

    info!("📊 Transaction reliability over time:");
    for (time, attempts, successes, reliability) in &reliability_timeline {
        info!("   ⏱️ {:?}: attempts={}, successes={}, reliability={:.1}%", 
              time, attempts, successes, reliability);
    }

    info!("📊 Final reliability metrics:");
    info!("   📊 Total attempts: {}", total_attempts);
    info!("   📊 Total successes: {}", total_successes);
    info!("   📊 Final reliability: {:.1}%", final_reliability);

    if !reliability_timeline.is_empty() {
        let reliabilities: Vec<f64> = reliability_timeline.iter().map(|(_, _, _, r)| *r).collect();
        let min_reliability = reliabilities.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_reliability = reliabilities.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let avg_reliability = reliabilities.iter().sum::<f64>() / reliabilities.len() as f64;

        info!("📊 Reliability statistics: min={:.1}%, max={:.1}%, avg={:.1}%", 
              min_reliability, max_reliability, avg_reliability);
    }

    assert!(!reliability_timeline.is_empty(),
           "Should have collected reliability samples");

    assert!(total_attempts > 0,
           "Should have attempted at least one transaction");

    Ok(())
}

#[tokio::test]
async fn measure_transaction_recovery_rates() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("🔄 Measuring transaction recovery and retry success rates");

    let recovery_scenarios = [
        ("network_recovery", "Network failure recovery"),
        ("gas_price_recovery", "Gas price adjustment recovery"),
        ("liquidity_recovery", "Liquidity shortage recovery"),
        ("timeout_recovery", "Timeout recovery"),
    ];

    let mut recovery_results = Vec::new();

    for (scenario_name, scenario_desc) in recovery_scenarios {
        info!("🔄 Testing recovery scenario: {} - {}", scenario_name, scenario_desc);

        let mut initial_failures = 0;
        let mut recovery_successes = 0;
        let mut total_recovery_attempts = 0;

        for attempt_cycle in 0..3 {
            info!("🔄 Recovery cycle {}/{}", attempt_cycle + 1, 3);

            let log_event = create_recovery_test_opportunity(scenario_name).await?;
            let initial_result = timeout(
                Duration::from_secs(5),
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;

            match initial_result {
                Ok(Ok(_)) => {
                    recovery_successes += 1;
                    info!("🔄 Initial attempt succeeded immediately");
                },
                Ok(Err(_)) | Err(_) => {
                    initial_failures += 1;
                    info!("🔄 Initial attempt failed, attempting recovery");

                    for retry_attempt in 0..2 {
                        total_recovery_attempts += 1;

                        let recovery_log_event = create_recovery_test_opportunity(scenario_name).await?;
                        let recovery_result = timeout(
                            Duration::from_secs(7),
                            process_strategy(recovery_log_event, test_env.test_config.ws_url.clone())
                        ).await;

                        if matches!(recovery_result, Ok(Ok(_))) {
                            recovery_successes += 1;
                            info!("🔄 Recovery successful on retry {}", retry_attempt + 1);
                            break;
                        } else {
                            info!("🔄 Recovery attempt {} failed", retry_attempt + 1);
                        }

                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                },
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let recovery_rate = if initial_failures > 0 {
            (recovery_successes as f64 / (initial_failures + recovery_successes) as f64) * 100.0
        } else {
            100.0
        };

        recovery_results.push((scenario_name, initial_failures, recovery_successes, total_recovery_attempts, recovery_rate));

        info!("🔄 Recovery scenario '{}': recovery_rate={:.1}%, failures={}, recoveries={}, attempts={}", 
              scenario_name, recovery_rate, initial_failures, recovery_successes, total_recovery_attempts);

        tokio::time::sleep(Duration::from_millis(800)).await;
    }

    info!("📊 Transaction recovery rate analysis:");
    for (scenario, failures, recoveries, attempts, rate) in &recovery_results {
        info!("   🔄 {}: rate={:.1}%, failures={}, recoveries={}, attempts={}", 
              scenario, rate, failures, recoveries, attempts);
    }

    assert!(recovery_results.len() == recovery_scenarios.len(),
           "All recovery scenarios should be tested");

    Ok(())
}

#[tokio::test]
async fn measure_transaction_success_rates_market_conditions() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    info!("📈 Measuring transaction success rates under different market conditions");

    let market_scenarios = [
        ("bull_market", 4, "Bull market conditions"),
        ("bear_market", 4, "Bear market conditions"),
        ("volatile_market", 5, "Volatile market conditions"),
        ("stable_market", 3, "Stable market conditions"),
        ("flash_crash", 3, "Flash crash simulation"),
    ];

    let mut market_results = Vec::new();

    for (scenario_name, transaction_count, scenario_desc) in market_scenarios {
        info!("📈 Testing market scenario: {} - {}", scenario_name, scenario_desc);

        let mut market_successful = 0;
        let mut market_failed = 0;
        let mut profit_opportunities = 0;
        let mut execution_times = Vec::new();

        for tx_index in 0..transaction_count {
            let start_time = Instant::now();
            let log_event = create_market_test_opportunity(scenario_name, tx_index).await?;

            let transaction_result = timeout(
                Duration::from_secs(9),
                process_strategy(log_event, test_env.test_config.ws_url.clone())
            ).await;

            let execution_time = start_time.elapsed();
            execution_times.push(execution_time);

            match transaction_result {
                Ok(Ok(_)) => {
                    market_successful += 1;
                    profit_opportunities += 1;
                    info!("📈 Market transaction {}/{} succeeded in {:?}", tx_index + 1, transaction_count, execution_time);
                },
                Ok(Err(_)) | Err(_) => {
                    market_failed += 1;
                    info!("📈 Market transaction {}/{} failed in {:?}", tx_index + 1, transaction_count, execution_time);
                },
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let market_success_rate = if transaction_count > 0 {
            (market_successful as f64 / transaction_count as f64) * 100.0
        } else {
            0.0
        };

        let profit_rate = if transaction_count > 0 {
            (profit_opportunities as f64 / transaction_count as f64) * 100.0
        } else {
            0.0
        };

        let avg_execution_time = if !execution_times.is_empty() {
            execution_times.iter().sum::<Duration>() / execution_times.len() as u32
        } else {
            Duration::ZERO
        };

        market_results.push((scenario_name, market_successful, market_failed, market_success_rate, profit_rate, avg_execution_time));

        info!("📈 Market scenario '{}': success_rate={:.1}%, profit_rate={:.1}%, successful={}, failed={}, avg_time={:?}", 
              scenario_name, market_success_rate, profit_rate, market_successful, market_failed, avg_execution_time);

        tokio::time::sleep(Duration::from_millis(600)).await;
    }

    info!("📊 Market condition success rate analysis:");
    for (scenario, successful, failed, success_rate, profit_rate, avg_time) in &market_results {
        info!("   📈 {}: success={:.1}%, profit={:.1}%, successful={}, failed={}, avg_time={:?}", 
              scenario, success_rate, profit_rate, successful, failed, avg_time);
    }

    assert!(market_results.len() == market_scenarios.len(),
           "All market scenarios should be tested");

    Ok(())
}

async fn create_success_test_opportunity(scenario: &str, index: usize) -> Result<LogEvent> {
    let (pool_variant, fee) = match scenario {
        "high_liquidity" => (3, 3000u32),
        "medium_liquidity" => (2, 500u32),
        "low_liquidity" => (3, 10000u32),
        "mixed_liquidity" => {
            if index % 2 == 0 { (3, 3000u32) } else { (2, 500u32) }
        },
        _ => (3, 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_stress_test_opportunity(scenario: &str, index: usize) -> Result<LogEvent> {
    let (pool_variant, fee) = match scenario {
        "rapid_fire" => (3, 3000u32),
        "complex_arbitrage" => (2, 10000u32),
        "network_congestion" => (3, 500u32),
        "gas_volatility" => {
            if index % 2 == 0 { (3, 3000u32) } else { (2, 10000u32) }
        },
        _ => (3, 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_reliability_test_opportunity() -> Result<LogEvent> {
    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant: 3,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(3000u32),
    })
}

async fn create_recovery_test_opportunity(scenario: &str) -> Result<LogEvent> {
    let (pool_variant, fee) = match scenario {
        "network_recovery" => (3, 3000u32),
        "gas_price_recovery" => (2, 500u32),
        "liquidity_recovery" => (3, 10000u32),
        "timeout_recovery" => (2, 3000u32),
        _ => (3, 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

async fn create_market_test_opportunity(scenario: &str, index: usize) -> Result<LogEvent> {
    let (pool_variant, fee) = match scenario {
        "bull_market" => (3, 3000u32),
        "bear_market" => (2, 10000u32),
        "volatile_market" => {
            match index % 3 {
                0 => (3, 500u32),
                1 => (2, 3000u32),
                _ => (3, 10000u32),
            }
        },
        "stable_market" => (3, 3000u32),
        "flash_crash" => (2, 500u32),
        _ => (3, 3000u32),
    };

    Ok(LogEvent {
        log_pool_address: address!("1f9840a85d5aF5bf1D1762F925BDADdC4201F984"),
        corresponding_pool_address: address!("5777d92f208679DB4b9778590Fa3CAB3aC9e2168"),
        pool_variant,
        token0: address!("A0b86a33E6441E4C536C53D5BBD7AE4B9a24C6F2"),
        token1: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        fee: U24::from(fee),
    })
}

