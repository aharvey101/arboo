use crate::arbitrage::simulation::{self, simulation};
use crate::common::pools::load_all_pools;
use alloy::rpc::types::eth::Filter;
use alloy::{
    providers::{Provider, RootProvider},
    pubsub::PubSubFrontend,
};
use anyhow::Result;
use dotenv::var;
use futures::StreamExt;
use revm_primitives::Address;
use std::sync::Arc;

pub async fn strategy(provider: Arc<RootProvider<PubSubFrontend>>) -> Result<()> {
    //
    // 1 load all pools

    // let (pools, last_id) = load_all_pools(var("WS_URL").unwrap(), 10_000_000, 50000).await?;
    // let pools = get_pools()?;
    let contract_address: Address = "0x3ffeea07a27fab7ad1df5297fa75e77a43cb5790".parse()?;

    // Define the event filter
    let swap_event = "Swap(address,address,int256,int256,uint160,uint128,int24)";
    let event_filter = Filter::new().event(swap_event);

    // Subscribe to the events
    let mut stream = provider.subscribe_logs(&event_filter).await?.into_stream();
    println!("Bout to stream: {:?}", stream);
    while let Some(log) = stream.next().await {
        // Print the log data (you can parse it further based on the event structure)
        let address = log.inner.address;
        // if !pools.contains(&address.clone().to_string().as_str()) {
        // continue;
        // }
        println!("Log address: {:#?}", log.inner.address);
        if address
            != "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"
                .parse::<Address>()
                .unwrap()
        {
            continue;
        }
        println!("Yay a pool we care about");
        // now do simulations!

        let adjacent_pool = "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"
            .parse::<Address>()
            .unwrap();
        let join_handle = tokio::spawn(simulation(address, adjacent_pool))
            .await?
            .expect("Error with spawn");
    }

    // subscribe to price changes on events?

    // 2. Subscribe to all pools
    // 3. When an event happens:
    //      - spawn a thread handling the event
    //      - spawned thread should run the simulation function
    //      - simulation function should calculate the potential for profit
    //      - profit is something like:
    //          - arb revenue - gas cost

    Ok(())
}
// fn get_pools() -> Result<Vec<&str>> {
//     todo!();
//     // just define some pools, we gotta work out a better way to get pools that exist on v2 and v3
//     //
//     let pools: Vec<&str> = vec![
//         "0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8", //USDC/ETH v3
//         "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc", //USDC/ETH v2 0.3% fee
//         "0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36", //ETH/USDT v3? 0.3% fee
//         "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852", //ETH/USDT v2 0.3% fee
//     ];

//     Ok(pools.to_owned())
// }
