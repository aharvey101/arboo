pub mod utils;
pub mod fixtures;

pub mod atomic {
    pub mod atomic_tests;
}

pub mod unit {
    pub mod env_loading_tests;
    pub mod log_event_processing_tests;
    pub mod test_fork_check;
}

pub mod pool {
    pub mod pool_data_tests;
}

pub mod evm {
//    pub mod evm_simulator_tests;
    pub mod alloydb_compatibility_test;
    pub mod direct_reth_alloydb_test;
}


pub mod performance {
}

pub mod memory {
}

pub mod integration {
    pub mod anvil_setup_test;
    pub mod simple_e2e_test;
    pub mod simple_arb_test;
    //pub mod concurrent_opportunities_tests;
    pub mod full_arbitrage_cycle_tests;
    pub mod full_arb_e2e;
}

pub mod edge_cases {
}

pub mod misc {
    pub mod logger_tests;
    pub mod rpc_call_measurement_tests;
}

