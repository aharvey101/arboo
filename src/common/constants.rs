use alloy_primitives::U256;

/// Common ETH amount constants for testing and simulation
/// All amounts are in wei (with 18 decimal places)

pub fn one_ether() -> U256 {
    U256::from(10).pow(U256::from(18)) // 1e18
}

pub fn one_hundred_ether() -> U256 {
    U256::from(100) * U256::from(10).pow(U256::from(18)) // 100e18
}

pub fn fify_thousand_eth() -> U256 {
    U256::from(50000) * U256::from(10).pow(U256::from(18)) // 50000e18
}

pub fn five_hundred_eth() -> U256 {
    U256::from(500) * U256::from(10).pow(U256::from(18)) // 500e18
}

pub fn one_thousand_eth() -> U256 {
    U256::from(1000) * U256::from(10).pow(U256::from(18)) // 1000e18
}

pub fn five_hundred_thousand_eth() -> U256 {
    U256::from(50000) * U256::from(10).pow(U256::from(18)) // 50000e18
}
