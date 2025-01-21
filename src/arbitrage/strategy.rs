use crate::arbitrage::simulation::simulation;
use crate::common::pools::{load_all_pools, Pool};
use alloy::{
    providers::{Provider, RootProvider},
    pubsub::PubSubFrontend,
};
use anyhow::Result;
use revm::primitives::Address;
use std::{collections::HashMap, str::FromStr, sync::Arc};

pub async fn strategy(provider: Arc<RootProvider<PubSubFrontend>>) -> Result<()> {
    // Load all pools
    let (pools, _) = load_all_pools(std::env::var("WS_URL").unwrap(), 10_000_000, 50000).await?;

    // Create a map of pool addresses to their paired pools
    let pool_map: HashMap<Address, Address> = get_pool_pairs()
        .iter()
        .flat_map(|(pool_a, pool_b)| {
            let addr_a = Address::from_str(pool_a).unwrap();
            let addr_b = Address::from_str(pool_b).unwrap();
            vec![(addr_a, addr_b), (addr_b, addr_a)]
        })
        .collect();

    // Iterate through the pools and perform simulations
    for (pool_a, pool_b) in pool_map {
        // Perform simulation for each pool pair
        let result = simulation(pool_a, pool_b).await?;
        println!(
            "Simulation result for pools {:?} and {:?}: {:?}",
            pool_a, pool_b, result
        );
    }

    Ok(())
}

fn get_pool_pairs() -> Vec<(String, String)> {
    vec![
        // USDC/ETH V2 0.3% <-> ETH/USDT V3 0.3%
        (
            "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc".to_string(),
            "0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36".to_string(),
        ),
        // USDC/ETH V3 0.3% <-> ETH/USDT V2 0.3%
        (
            "0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8".to_string(),
            "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".to_string(),
        ),
    ]
}
