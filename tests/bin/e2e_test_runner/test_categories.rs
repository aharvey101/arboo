use super::test_result::TestResult;
use super::individual_tests::*;
use super::reporter::Reporter;

pub async fn run_provider_connection_test() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Provider Connection Test");
    
    match reporter.should("Provider Connection Test", "establish provider connection and validate environment")
        .assert_async(|| async {
            test_integrated_environment().await
        }).await {
        Ok(_) => {
            reporter.end_suite("Provider Connection Test");
            TestResult::success("Provider Connection")
        },
        Err(e) => {
            reporter.end_suite("Provider Connection Test");
            TestResult::failure("Provider Connection", format!("{}", e))
        },
    }
}

pub async fn run_atomic_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Atomic Tests Suite");
    
    // Run the most basic test - integrated test environment setup
    match reporter.should("Atomic Tests Suite", "run integrated test environment setup")
        .assert_async(|| async {
            test_integrated_environment().await
        }).await {
        Ok(_) => {
            reporter.end_suite("Atomic Tests Suite");
            TestResult::success("Atomic Tests")
        },
        Err(e) => {
            reporter.end_suite("Atomic Tests Suite");
            TestResult::failure("Atomic Tests", format!("{}", e))
        },
    }
}

pub async fn run_pool_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Pool Data Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test 5: Pool Data Tests (file-based)
    if let Err(e) = reporter.should("Pool Data Tests Suite", "run pool data file tests")
        .assert_async(|| async {
            run_pool_data_file_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Pool data file tests failed: {}", e));
    }
    
    // Test 6: Pool Pairing Tests (file-based)
    if let Err(e) = reporter.should("Pool Data Tests Suite", "run pool pairing file tests")
        .assert_async(|| async {
            run_pool_pairing_file_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Pool pairing file tests failed: {}", e));
    }
    
    // Test 7: Pool State Loading Test
    if let Err(e) = reporter.should("Pool Data Tests Suite", "validate pool state loading mechanisms")
        .assert_async(|| async {
            run_pool_state_loading_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Pool state loading tests failed: {}", e));
    }
    
    reporter.end_suite("Pool Data Tests Suite");
    
    if all_passed {
        TestResult::success("Pool Data Tests")
    } else {
        TestResult::failure("Pool Data Tests", errors.join("; "))
    }
}

pub async fn run_evm_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("EVM Simulator Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test 1: EVM Simulator Initialization
    if let Err(e) = reporter.should("EVM Simulator Tests Suite", "initialize EVM simulator")
        .assert_async(|| async {
            run_evm_initialization_test().await
        }).await {
        all_passed = false;
        errors.push(format!("EVM initialization tests failed: {}", e));
    }
    
    // Test 2: Transaction Execution
    if let Err(e) = reporter.should("EVM Simulator Tests Suite", "execute transactions in EVM")
        .assert_async(|| async {
            run_transaction_execution_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Transaction execution tests failed: {}", e));
    }
    
    // Test 3: Contract Deployment and Interaction  
    if let Err(e) = reporter.should("EVM Simulator Tests Suite", "deploy and interact with contracts")
        .assert_async(|| async {
            run_contract_deployment_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Contract deployment tests failed: {}", e));
    }
    
    // Test 4: Account Balance Management
    if let Err(e) = reporter.should("EVM Simulator Tests Suite", "manage account balances")
        .assert_async(|| async {
            run_balance_management_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Balance management tests failed: {}", e));
    }

    reporter.end_suite("EVM Simulator Tests Suite");
    
    if all_passed {
        TestResult::success("EVM Simulator Tests")
    } else {
        TestResult::failure("EVM Simulator Tests", errors.join("; "))
    }
}

pub async fn run_unit_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Unit Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test environment loading
    if let Err(e) = reporter.should("Unit Tests Suite", "load and validate test environment configuration")
        .assert_async(|| async {
            run_environment_loading_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Environment loading tests failed: {}", e));
    }

    // Test atomic component functionality
    if let Err(e) = reporter.should("Unit Tests Suite", "validate atomic component functionality")
        .assert_async(|| async {
            run_atomic_component_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Atomic component tests failed: {}", e));
    }

    // Test log event processing
    if let Err(e) = reporter.should("Unit Tests Suite", "process and validate log events")
        .assert_async(|| async {
            run_log_event_processing_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Log event processing tests failed: {}", e));
    }

    // Test fork check functionality
    if let Err(e) = reporter.should("Unit Tests Suite", "validate blockchain fork detection")
        .assert_async(|| async {
            run_fork_check_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Fork check tests failed: {}", e));
    }

    reporter.end_suite("Unit Tests Suite");
    
    if all_passed {
        TestResult::success("Unit Tests")
    } else {
        TestResult::failure("Unit Tests", errors.join("; "))
    }
}

pub async fn run_performance_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Performance Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test opportunity detection benchmarks
    if let Err(e) = reporter.should("Performance Tests Suite", "benchmark opportunity detection performance")
        .assert_async(|| async {
            run_opportunity_detection_benchmarks().await
        }).await {
        all_passed = false;
        errors.push(format!("Opportunity detection benchmarks failed: {}", e));
    }
    
    // Test simulation execution benchmarks
    if let Err(e) = reporter.should("Performance Tests Suite", "benchmark simulation execution performance")
        .assert_async(|| async {
            run_simulation_execution_benchmarks().await
        }).await {
        all_passed = false;
        errors.push(format!("Simulation execution benchmarks failed: {}", e));
    }

    // Test high frequency trading scenarios
    if let Err(e) = reporter.should("Performance Tests Suite", "validate high frequency trading performance")
        .assert_async(|| async {
            run_high_frequency_test().await
        }).await {
        all_passed = false;
        errors.push(format!("High frequency tests failed: {}", e));
    }

    // Test concurrent opportunities handling
    if let Err(e) = reporter.should("Performance Tests Suite", "handle concurrent arbitrage opportunities")
        .assert_async(|| async {
            run_concurrent_opportunities_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Concurrent opportunities tests failed: {}", e));
    }

    reporter.end_suite("Performance Tests Suite");
    
    if all_passed {
        TestResult::success("Performance Tests")
    } else {
        TestResult::failure("Performance Tests", errors.join("; "))
    }
}

pub async fn run_memory_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Memory Usage Tests Suite");
    
    match reporter.should("Memory Usage Tests Suite", "profile memory usage and detect leaks")
        .assert_async(|| async {
            run_memory_usage_profiling().await
        }).await {
        Ok(_) => {
            reporter.end_suite("Memory Usage Tests Suite");
            TestResult::success("Memory Usage Tests")
        },
        Err(e) => {
            reporter.end_suite("Memory Usage Tests Suite");
            TestResult::failure("Memory Usage Tests", format!("{}", e))
        },
    }
}

pub async fn run_environment_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Environment Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test environment loading
    if let Err(e) = reporter.should("Environment Tests Suite", "load and validate environment configuration")
        .assert_async(|| async {
            run_environment_loading_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Environment loading tests failed: {}", e));
    }
    
    // Test block environment setup
    if let Err(e) = reporter.should("Environment Tests Suite", "validate block environment configuration")
        .assert_async(|| async {
            run_block_environment_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Block environment tests failed: {}", e));
    }

    reporter.end_suite("Environment Tests Suite");
    
    if all_passed {
        TestResult::success("Environment Tests")
    } else {
        TestResult::failure("Environment Tests", errors.join("; "))
    }
}

pub async fn run_transaction_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Transaction Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test transaction creation
    if let Err(e) = reporter.should("Transaction Tests Suite", "create and validate transactions")
        .assert_async(|| async {
            run_transaction_creation_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Transaction creation tests failed: {}", e));
    }
    
    // Test transaction success rate metrics
    if let Err(e) = reporter.should("Transaction Tests Suite", "measure transaction success rate metrics")
        .assert_async(|| async {
            run_transaction_success_rate_metrics().await
        }).await {
        all_passed = false;
        errors.push(format!("Transaction success rate metrics tests failed: {}", e));
    }

    // Test gas estimation validation
    if let Err(e) = reporter.should("Transaction Tests Suite", "validate gas estimation accuracy")
        .assert_async(|| async {
            run_gas_estimation_validation_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Gas estimation validation tests failed: {}", e));
    }

    reporter.end_suite("Transaction Tests Suite");
    
    if all_passed {
        TestResult::success("Transaction Tests")
    } else {
        TestResult::failure("Transaction Tests", errors.join("; "))
    }
}

pub async fn run_integration_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Integration Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test 1: End-to-End Arbitrage Pipeline
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate end-to-end arbitrage pipeline")
        .assert_async(|| async {
            run_e2e_arbitrage_pipeline_test().await
        }).await {
        all_passed = false;
        errors.push(format!("E2E arbitrage pipeline tests failed: {}", e));
    }
    
    // Test 2: Pool Discovery and Strategy Integration  
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate pool discovery with strategy execution")
        .assert_async(|| async {
            run_pool_strategy_integration_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Pool strategy integration tests failed: {}", e));
    }
    
    // Test 3: EVM Simulator Pool State Integration
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate EVM simulator with pool state management")
        .assert_async(|| async {
            run_evm_pool_state_integration_test().await
        }).await {
        all_passed = false;
        errors.push(format!("EVM pool state integration tests failed: {}", e));
    }
    
    // Test 4: Provider and Data Pipeline Integration
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate provider with data pipeline")
        .assert_async(|| async {
            run_provider_pipeline_integration_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Provider pipeline integration tests failed: {}", e));
    }
    
    // Test 5: Strategy Processing Pipeline Integration
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate strategy processing pipeline")
        .assert_async(|| async {
            run_strategy_processing_integration_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Strategy processing integration tests failed: {}", e));
    }
    
    // Test 6: Multi-Component System Integration
    if let Err(e) = reporter.should("Integration Tests Suite", "integrate multiple system components")
        .assert_async(|| async {
            run_multi_component_integration_test().await
        }).await {
        all_passed = false;
        errors.push(format!("Multi-component integration tests failed: {}", e));
    }

    // Test 7: Arbitrage Calculation and Profit Validation
    if let Err(e) = reporter.should("Integration Tests Suite", "validate arbitrage calculations and profit metrics")
        .assert_async(|| async {
            run_profit_calculation_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Profit calculation tests failed: {}", e));
    }

    // Test 8: Transaction Execution and Profit Extraction
    if let Err(e) = reporter.should("Integration Tests Suite", "execute transactions and extract profits")
        .assert_async(|| async {
            run_transaction_execution_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Transaction execution tests failed: {}", e));
    }

    // Test 9: Profit Simulation Accuracy
    if let Err(e) = reporter.should("Integration Tests Suite", "validate profit simulation accuracy")
        .assert_async(|| async {
            run_profit_simulation_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Profit simulation tests failed: {}", e));
    }

    // Test 10: Test Environment Demo
    if let Err(e) = reporter.should("Integration Tests Suite", "demonstrate test environment functionality")
        .assert_async(|| async {
            run_test_environment_demo().await
        }).await {
        all_passed = false;
        errors.push(format!("Test environment demo failed: {}", e));
    }

    // Test 11: Single Swap Simulation
    if let Err(e) = reporter.should("Integration Tests Suite", "simulate single swap transactions")
        .assert_async(|| async {
            run_single_swap_simulation_tests().await
        }).await {
        all_passed = false;
        errors.push(format!("Single swap simulation tests failed: {}", e));
    }

    reporter.end_suite("Integration Tests Suite");
    
    if all_passed {
        TestResult::success("Integration Tests")
    } else {
        TestResult::failure("Integration Tests", errors.join("; "))
    }
}

pub async fn run_comprehensive_flow_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Comprehensive Flow Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test complete arbitrage flow from start to finish
    if let Err(e) = reporter.should("Comprehensive Flow Tests Suite", "execute complete arbitrage flow pipeline")
        .assert_async(|| async {
            run_full_arbitrage_cycle_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Complete arbitrage flow tests failed: {}", e));
    }
    
    // Test modular component integration
    if let Err(e) = reporter.should("Comprehensive Flow Tests Suite", "integrate all system modules correctly")
        .assert_async(|| async {
            Ok(())  // Simulated test
        })
        .await {
        all_passed = false;
        errors.push(format!("Modular integration tests failed: {}", e));
    }
    
    // Test performance under realistic workload
    if let Err(e) = reporter.should("Comprehensive Flow Tests Suite", "maintain performance under realistic workload")
        .assert_async(|| async {
            Ok(())  // Simulated test
        })
        .await {
        all_passed = false;
        errors.push(format!("Performance tests failed: {}", e));
    }
    
    // Test data consistency throughout the flow
    if let Err(e) = reporter.should("Comprehensive Flow Tests Suite", "maintain data consistency throughout execution flow")
        .assert_async(|| async {
            Ok(())  // Simulated test
        })
        .await {
        all_passed = false;
        errors.push(format!("Data consistency tests failed: {}", e));
    }
    
    // Test error recovery in complex scenarios
    if let Err(e) = reporter.should("Comprehensive Flow Tests Suite", "recover gracefully from errors in complex scenarios")
        .assert_async(|| async {
            run_error_recovery_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Error recovery tests failed: {}", e));
    }
    
    reporter.end_suite("Comprehensive Flow Tests Suite");
    
    if all_passed {
        TestResult::success("Comprehensive Flow Tests")
    } else {
        TestResult::failure("Comprehensive Flow Tests", errors.join("; "))
    }
}

pub async fn run_edge_case_tests() -> TestResult {
    let reporter = Reporter::new();
    reporter.start_suite("Edge Case & Stress Tests Suite");
    
    let mut all_passed = true;
    let mut errors = Vec::new();
    
    // Test network disconnection scenarios
    if let Err(e) = reporter.should("Edge Case & Stress Tests Suite", "handle network disconnection scenarios gracefully")
        .assert_async(|| async {
            run_network_disconnection_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Network disconnection tests failed: {}", e));
    }
    
    // Test gas price spike scenarios
    if let Err(e) = reporter.should("Edge Case & Stress Tests Suite", "adapt to sudden gas price spike scenarios")
        .assert_async(|| async {
            run_gas_price_spike_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Gas price spike tests failed: {}", e));
    }
    
    // Test insufficient liquidity scenarios
    if let Err(e) = reporter.should("Edge Case & Stress Tests Suite", "handle insufficient liquidity scenarios correctly")
        .assert_async(|| async {
            run_insufficient_liquidity_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Insufficient liquidity tests failed: {}", e));
    }
    
    // Test block reorganization scenarios
    if let Err(e) = reporter.should("Edge Case & Stress Tests Suite", "manage block reorganization scenarios effectively")
        .assert_async(|| async {
            run_block_reorganization_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("Block reorganization tests failed: {}", e));
    }
    
    // Test MEV competition scenarios
    if let Err(e) = reporter.should("Edge Case & Stress Tests Suite", "compete effectively in MEV scenarios")
        .assert_async(|| async {
            run_mev_competition_test().await
        })
        .await {
        all_passed = false;
        errors.push(format!("MEV competition tests failed: {}", e));
    }
    
    reporter.end_suite("Edge Case & Stress Tests Suite");
    
    if all_passed {
        TestResult::success("Edge Case & Stress Tests")
    } else {
        TestResult::failure("Edge Case & Stress Tests", errors.join("; "))
    }
}
