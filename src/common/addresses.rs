use alloy_primitives::{address, Address};

/// Test and mock addresses for simulation purposes

pub fn me() -> Address {
    address!("0000000000000000000000000000000000000001")
}

#[derive(Debug)]
pub enum MockAddress {
    UniV2,
    UniV3,
}

pub fn mock_addresses(address_type: MockAddress) -> Address {
    match address_type {
        MockAddress::UniV2 => address!("d3d2E2692501A5c9Ca623199D38826e513033a17"),
        MockAddress::UniV3 => address!("1d42064Fc4Beb5F8aAF85F4617AE8b3b5B8Bd801"),
    }
}
