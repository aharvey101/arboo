use crate::arbitrage::simulation::simulation;
use crate::common::logs::LogEvent;
use crate::common::pools::{load_all_pools, Pool};
use alloy::{
    providers::{Provider, RootProvider},
    pubsub::PubSubFrontend,
};
use anyhow::Result;
use revm::primitives::Address;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::broadcast::Sender;

pub async fn strategy(
    provider: Arc<RootProvider<PubSubFrontend>>,
    sender: Sender<LogEvent>,
){

    let mut event_reciever = sender.subscribe();

    loop {
        match event_reciever.recv().await {
            Ok(event) => 
            {

            // doing the simulation is very slow
            // we can maybe do it faster by doing two quote simulations and checking the difference??
            
            // run the simulation twice?
            simulation(event.token0, event.token1).await.expect("Error running simulation");
            simulation(event.token1, event.token0).await.expect("Error doing second simulation");

            }
            Err(err) => print!("Error recieving event {:?}", err),
        }
    }


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
