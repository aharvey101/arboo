pub mod utils;
pub mod fixtures;

pub mod atomic {
    pub mod atomic_tests;
}

pub mod unit {
    pub mod arbitrage_calculation_tests;
    pub mod env_loading_tests;
    pub mod error_recovery_tests;
    pub mod log_event_processing_tests;
    pub mod profit_simulation_tests;
    pub mod single_swap_simulation_tests;
    pub mod test_fork_check;
    pub mod transaction_creation_tests;
    pub mod transaction_execution_tests;
}

pub mod pool {
    pub mod pool_data_tests;
    pub mod pool_pairing_tests;
}

pub mod evm {
//    pub mod evm_simulator_tests;
    pub mod alloydb_compatibility_test;
    pub mod direct_reth_alloydb_test;
}


pub mod performance {
    pub mod high_frequency_tests;
    pub mod mev_competition_tests;
    pub mod opportunity_detection_benchmarks;
    pub mod simulation_execution_benchmarks;
    pub mod transaction_success_rate_metrics;
}

pub mod memory {
    pub mod memory_usage_profiling;
}

pub mod integration {
    pub mod concurrent_opportunities_tests;
    pub mod full_arbitrage_cycle_tests;
    pub mod network_disconnection_tests;
}

pub mod edge_cases {
    pub mod block_reorganization_tests;
    pub mod gas_price_spike_tests;
    pub mod insufficient_liquidity_tests;
}

pub mod misc {
    pub mod logger_tests;
    pub mod rpc_call_measurement_tests;
}

