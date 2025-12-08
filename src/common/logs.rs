use super::pairs::Event;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Filter, Log};
use alloy_primitives::aliases::U24;
use futures::StreamExt;
use log::{debug, error, info, warn};
use revm::primitives::keccak256;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::{broadcast::Sender, mpsc, RwLock};

/// Core log processing service for arbitrage opportunity detection
pub struct LogProcessor {
    pairs: Arc<RwLock<HashMap<Address, Event>>>,
    token_pair_index: Arc<RwLock<HashMap<TokenPair, Vec<Address>>>>,
    event_sender: Sender<LogEvent>,
    // Add queue for better performance and backpressure handling
    event_queue_tx: mpsc::Sender<LogEvent>,
    // Add provider for dynamic pool discovery
    provider: Arc<RootProvider<PubSubFrontend>>,
    // Cache path for persisting newly discovered pools
    cache_path: String,
}

/// Represents a token pair for efficient indexing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TokenPair {
    token0: Address,
    token1: Address,
}

impl TokenPair {
    fn new(token_a: Address, token_b: Address) -> Self {
        // Always order tokens consistently for matching
        if token_a <= token_b {
            Self {
                token0: token_a,
                token1: token_b,
            }
        } else {
            Self {
                token0: token_b,
                token1: token_a,
            }
        }
    }
}

impl LogProcessor {
    /// Create a new log processor with the given pairs and event sender
    pub fn new(
        pairs: HashMap<Address, Event>,
        event_sender: Sender<LogEvent>,
        provider: Arc<RootProvider<PubSubFrontend>>,
        cache_path: String,
    ) -> (Self, mpsc::Receiver<LogEvent>) {
        let token_pair_index = Self::build_token_pair_index(&pairs);

        // Create bounded MPSC queue for reliable delivery with backpressure
        let (event_queue_tx, event_queue_rx) = mpsc::channel::<LogEvent>(2000); // Larger buffer for high frequency

        info!(
            "LogProcessor initialized with {} pairs and {} unique token pairs",
            pairs.len(),
            token_pair_index.len()
        );
        info!("Event queue initialized with capacity: 2000");
        info!("Dynamic pool discovery enabled");

        let processor = Self {
            pairs: Arc::new(RwLock::new(pairs)),
            token_pair_index: Arc::new(RwLock::new(token_pair_index)),
            event_sender,
            event_queue_tx,
            provider,
            cache_path,
        };

        (processor, event_queue_rx)
    }

     /// Build an efficient index of token pairs to pool addresses
     fn build_token_pair_index(pairs: &HashMap<Address, Event>) -> HashMap<TokenPair, Vec<Address>> {
         let mut index = HashMap::new();
         
         // Define our target tokens for detailed logging
         let dai_addr = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap_or_default();
         let weth_addr = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap_or_default();

         for (pool_address, event) in pairs {
             let token_pair = match event {
                 Event::PairCreated(pair) => {
                     let tp = TokenPair::new(pair.token0, pair.token1);
                     // Log DAI/WETH pairs
                     if (pair.token0 == dai_addr && pair.token1 == weth_addr) ||
                        (pair.token0 == weth_addr && pair.token1 == dai_addr) {
                         info!("📍 Indexing V2 DAI/WETH pool: {:?} (token0={:?}, token1={:?}, fee={})", 
                             pool_address, pair.token0, pair.token1, pair.fee);
                     }
                     tp
                 }
                 Event::PoolCreated(pool) => {
                     let tp = TokenPair::new(pool.token0, pool.token1);
                     // Log DAI/WETH pairs
                     if (pool.token0 == dai_addr && pool.token1 == weth_addr) ||
                        (pool.token0 == weth_addr && pool.token1 == dai_addr) {
                         info!("📍 Indexing V3 DAI/WETH pool: {:?} (token0={:?}, token1={:?}, fee={})", 
                             pool_address, pool.token0, pool.token1, pool.fee);
                     }
                     tp
                 }
             };

             index
                 .entry(token_pair)
                 .or_insert_with(Vec::new)
                 .push(*pool_address);
         }

         debug!("Built token pair index with {} unique pairs", index.len());
         index
     }

     /// Process a single log event and attempt to create arbitrage opportunities
     pub async fn process_log(&self, log: &Log) -> Option<LogEvent> {
         let pool_address = log.address();
         
         // Define our target tokens for detailed logging
         let dai_addr = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap_or_default();
         let weth_addr = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap_or_default();
         
         // Check if pool exists in our cache
         let pairs_guard = self.pairs.read().await;
         let pool_exists = pairs_guard.contains_key(&pool_address);
         
         if let Some(event) = pairs_guard.get(&pool_address) {
             // Log if this is a DAI/WETH pool
             match event {
                 Event::PairCreated(pair) => {
                     if (pair.token0 == dai_addr && pair.token1 == weth_addr) ||
                        (pair.token0 == weth_addr && pair.token1 == dai_addr) {
                         info!("📥 Received log from V2 DAI/WETH pool: {:?}", pool_address);
                     }
                 }
                 Event::PoolCreated(pool) => {
                     if (pool.token0 == dai_addr && pool.token1 == weth_addr) ||
                        (pool.token0 == weth_addr && pool.token1 == dai_addr) {
                         info!("📥 Received log from V3 DAI/WETH pool: {:?}", pool_address);
                     }
                 }
             }
         }
         
          if !pool_exists {
              drop(pairs_guard); // Release read lock before attempting discovery
              warn!("⚠️ Log from unknown pool: {:?}, attempting dynamic discovery", pool_address);
              
              // Try to discover the pool dynamically
              if let Some(event) = self.discover_pool_from_logs(pool_address).await {
                  // Add discovered pool to cache
                  let mut pairs_mut = self.pairs.write().await;
                  pairs_mut.insert(pool_address, event.clone());
                  drop(pairs_mut);
                  
                  // Update token pair index
                  let token_pair = match &event {
                      Event::PairCreated(pair) => TokenPair::new(pair.token0, pair.token1),
                      Event::PoolCreated(pool) => TokenPair::new(pool.token0, pool.token1),
                  };
                  let mut index = self.token_pair_index.write().await;
                  index
                      .entry(token_pair)
                      .or_insert_with(Vec::new)
                      .push(pool_address);
                  
                  // Persist to CSV cache file
                  match crate::common::pools::append_pool_to_cache(&event, &self.cache_path).await {
                      Ok(pool_id) => {
                          info!("✨ Dynamically discovered, cached, and persisted pool: {:?} (ID: {})", 
                               pool_address, pool_id);
                      }
                      Err(e) => {
                          warn!("⚠️ Failed to persist discovered pool to cache: {}", e);
                          info!("✨ Dynamically discovered and cached pool (memory only): {:?}", pool_address);
                      }
                  }
                  
                  // Continue processing with the newly discovered pool
              } else {
                 warn!("❌ Failed to discover pool: {:?}", pool_address);
                 return None;
             }
         } else {
             drop(pairs_guard);
         }
         
         // Look up the pool that generated this log
         let pairs_guard = self.pairs.read().await;
         let source_event = pairs_guard.get(&pool_address)?.clone();
         drop(pairs_guard);
         debug!("Processing swap log from known pool: {:?}", pool_address);

         // Now that we know this pool exists, find potential arbitrage with the other version
         match source_event {
             Event::PairCreated(v2_pool) => self.find_arbitrage_for_v2_pool(&v2_pool, pool_address).await,
             Event::PoolCreated(v3_pool) => self.find_arbitrage_for_v3_pool(&v3_pool, pool_address).await,
         }
     }

    /// Find V3 counterpart for a V2 pool to create arbitrage opportunity
    async fn find_arbitrage_for_v2_pool(
        &self,
        v2_pool: &super::pairs::V2PoolCreated,
        pool_address: Address,
    ) -> Option<LogEvent> {
        let token_pair = TokenPair::new(v2_pool.token0, v2_pool.token1);

        // Find all pools with the same token pair
        let index_guard = self.token_pair_index.read().await;
        let matching_pools = index_guard.get(&token_pair)?;
        
        // Collect all V3 pools with the same token pair and prioritize by fee tier
        let mut v3_candidates = Vec::new();
        
        for &candidate_address in matching_pools {
            if candidate_address == pool_address {
                continue; // Skip self
            }

            let pairs_guard = self.pairs.read().await;
            if let Some(Event::PoolCreated(v3_pool)) = pairs_guard.get(&candidate_address) {
                let v3_pool = v3_pool.clone();
                drop(pairs_guard);
                
                // Validate token pair consistency
                if Self::is_valid_token_pair(v3_pool.token0, v3_pool.token1) {
                    v3_candidates.push((candidate_address, v3_pool));
                }
            }
        }
        
        // Prioritize V3 pools by fee tier (prefer higher fees = higher liquidity for major pairs)
        // Priority order: 0.3% (3000), 0.05% (500), 1% (10000), 0.01% (100)
        v3_candidates.sort_by(|a, b| {
            let fee_priority = |fee: u32| match fee {
                3000 => 0,  // 0.3% - highest priority (most liquid for major pairs)
                500 => 1,   // 0.05% - second priority
                10000 => 2, // 1.0% - third priority  
                100 => 3,   // 0.01% - fourth priority
                _ => 4,     // Other fees - lowest priority
            };
            fee_priority(a.1.fee).cmp(&fee_priority(b.1.fee))
        });
        
        // Select the highest priority V3 pool
        if let Some((candidate_address, v3_pool)) = v3_candidates.into_iter().next() {
            debug!(
                "Found V2->V3 arbitrage: {:?} -> {:?} (fee: {})",
                pool_address, candidate_address, v3_pool.fee
            );

            return Some(LogEvent {
                pool_variant: 2, // V2 pool generated the log
                corresponding_pool_address: v3_pool.pair_address,
                log_pool_address: pool_address,
                token0: v2_pool.token0,
                token1: v2_pool.token1,
                fee: U24::from(v2_pool.fee),
            });
        }

        debug!("No V3 counterpart found for V2 pool: {:?}", pool_address);
        None
    }

    /// Find V2 counterpart for a V3 pool to create arbitrage opportunity
    async fn find_arbitrage_for_v3_pool(
        &self,
        v3_pool: &super::pairs::V3PoolCreated,
        pool_address: Address,
    ) -> Option<LogEvent> {
        let token_pair = TokenPair::new(v3_pool.token0, v3_pool.token1);
        
        debug!("Looking for arbitrage for V3 pool {} with tokens: {:?} - {:?}", 
               pool_address, v3_pool.token0, v3_pool.token1);

        // Find all pools with the same token pair
        let index_guard = self.token_pair_index.read().await;
        let matching_pools = index_guard.get(&token_pair)?;
        debug!("Found {} pools with matching token pair", matching_pools.len());
        
        // Look for a V2 pool among the matching pools
        for &candidate_address in matching_pools {
            if candidate_address == pool_address {
                debug!("candidate address = pool address, skipping");
                continue; // Skip self
            }

            let pairs_guard = self.pairs.read().await;
            if let Some(Event::PairCreated(v2_pool)) = pairs_guard.get(&candidate_address) {
                let v2_pool = v2_pool.clone();
                drop(pairs_guard);
                
                debug!(
                    "Found V3->V2 arbitrage: {:?} -> {:?}",
                    pool_address, candidate_address
                );

                return Some(LogEvent {
                    pool_variant: 3, // V3 pool generated the log
                    corresponding_pool_address: v2_pool.pair_address,
                    log_pool_address: pool_address,
                    token0: v3_pool.token0,
                    token1: v3_pool.token1,
                    fee: U24::from(v3_pool.fee),
                });
            }
        }

        debug!("No V2 counterpart found for V3 pool: {:?}", pool_address);
        None
    }

    /// Validate that token pair is valid (different tokens)
    fn is_valid_token_pair(token0: Address, token1: Address) -> bool {
        token0 != token1
    }

    /// Discover pool details from blockchain when pool is not in cache
    /// Attempts to determine if pool is V2 or V3 and extract token pair
    async fn discover_pool_from_logs(&self, pool_address: Address) -> Option<Event> {
        debug!("Attempting to discover pool {} from historical logs or direct calls", pool_address);
        
        // First try to discover from historical factory events
        let current_block = match self.provider.get_block_number().await {
            Ok(block) => block,
            Err(e) => {
                warn!("Failed to get current block for discovery: {}", e);
                return None;
            }
        };
        
        // Search back up to 1000 blocks for PoolCreated/PairCreated events from this address
        let from_block = current_block.saturating_sub(1000);
        
        // Try V3 PoolCreated event first
        if let Ok(event) = self.try_discover_v3_pool(pool_address, from_block, current_block).await {
            return Some(event);
        }
        
        // Try V2 PairCreated event
        if let Ok(event) = self.try_discover_v2_pool(pool_address, from_block, current_block).await {
            return Some(event);
        }
        
        // If not found in recent blocks, try querying the pool directly
        debug!("Pool creation events not found in recent blocks, attempting direct pool query");
        if let Some(event) = self.discover_pool_by_querying(pool_address).await {
            return Some(event);
        }
        
        debug!("Could not discover pool {} in recent blocks or by direct query", pool_address);
        None
    }

    /// Try to discover pool by querying the pool contract directly
    async fn discover_pool_by_querying(&self, pool_address: Address) -> Option<Event> {
        // Try to determine if it's a V2 or V3 pool by querying known methods
        
        // For V2 pairs, try to call token0() and token1()
        if let Some(event) = self.try_query_v2_pool(pool_address).await {
            return Some(event);
        }
        
        // For V3 pools, try to call token0(), token1(), and fee()
        if let Some(event) = self.try_query_v3_pool(pool_address).await {
            return Some(event);
        }
        
        None
    }

    /// Query a V2 pool directly for its token addresses
    async fn try_query_v2_pool(&self, pool_address: Address) -> Option<Event> {
        use alloy::sol_types::SolCall;
        
        alloy::sol! {
            function token0() external view returns (address);
            function token1() external view returns (address);
        }
        
        // Try to call token0()
        let token0_call = token0Call {};
        let token0_result = self.provider
            .call(
                &alloy::rpc::types::TransactionRequest::default()
                    .to(pool_address)
                    .input(token0_call.abi_encode().into()),
            )
            .await
            .ok()?;
        
        // Try to call token1()
        let token1_call = token1Call {};
        let token1_result = self.provider
            .call(
                &alloy::rpc::types::TransactionRequest::default()
                    .to(pool_address)
                    .input(token1_call.abi_encode().into()),
            )
            .await
            .ok()?;
        
        // Decode results
        use alloy_sol_types::SolValue;
        let token0 = Address::abi_decode(&token0_result, false).ok()?;
        let token1 = Address::abi_decode(&token1_result, false).ok()?;
        
        if token0 != token1 && token0 != Address::ZERO && token1 != Address::ZERO {
            info!("✅ Discovered V2 pair by querying: {:?} (token0: {:?}, token1: {:?})",
                  pool_address, token0, token1);
            
            return Some(Event::PairCreated(super::pairs::V2PoolCreated {
                pair_address: pool_address,
                token0,
                token1,
                fee: 3000, // V2 default fee
            }));
        }
        
        None
    }

    /// Query a V3 pool directly for its token addresses and fee
    async fn try_query_v3_pool(&self, pool_address: Address) -> Option<Event> {
        use alloy::sol_types::SolCall;
        
        alloy::sol! {
            function token0() external view returns (address);
            function token1() external view returns (address);
            function fee() external view returns (uint24);
        }
        
        // Try to call token0()
        let token0_call = token0Call {};
        let token0_result = self.provider
            .call(
                &alloy::rpc::types::TransactionRequest::default()
                    .to(pool_address)
                    .input(token0_call.abi_encode().into()),
            )
            .await
            .ok()?;
        
        // Try to call token1()
        let token1_call = token1Call {};
        let token1_result = self.provider
            .call(
                &alloy::rpc::types::TransactionRequest::default()
                    .to(pool_address)
                    .input(token1_call.abi_encode().into()),
            )
            .await
            .ok()?;
        
        // Try to call fee()
        let fee_call = feeCall {};
        let fee_result = self.provider
            .call(
                &alloy::rpc::types::TransactionRequest::default()
                    .to(pool_address)
                    .input(fee_call.abi_encode().into()),
            )
            .await
            .ok()?;
        
        // Decode results
        use alloy_sol_types::SolValue;
        let token0 = Address::abi_decode(&token0_result, false).ok()?;
        let token1 = Address::abi_decode(&token1_result, false).ok()?;
        let fee = u32::abi_decode(&fee_result, false).ok()?;
        
        if token0 != token1 && token0 != Address::ZERO && token1 != Address::ZERO {
            info!("✅ Discovered V3 pool by querying: {:?} (token0: {:?}, token1: {:?}, fee: {})",
                  pool_address, token0, token1, fee);
            
            return Some(Event::PoolCreated(super::pairs::V3PoolCreated {
                pair_address: pool_address,
                token0,
                token1,
                fee,
                tick_spacing: 0,
            }));
        }
        
        None
    }

    /// Try to discover V3 pool by looking for PoolCreated events
    async fn try_discover_v3_pool(
        &self,
        pool_address: Address,
        from_block: u64,
        to_block: u64,
    ) -> Result<Event, Box<dyn std::error::Error>> {
        let v3_factory = crate::common::pools::UNISWAP_V3_FACTORY;
        let pool_created_sig = keccak256("PoolCreated(address,address,uint24,int24,address)".as_bytes());
        
        let filter = Filter::new()
            .address(v3_factory)
            .event_signature(vec![pool_created_sig])
            .from_block(from_block)
            .to_block(to_block);
        
        let logs = self.provider.get_logs(&filter).await?;
        
        for log in logs {
            // Check if this log contains our pool address in the data
            let log_data = &log.inner.data.data;
            
            // V3 PoolCreated: (uint24 fee, int24 tickSpacing, address pool)
            use alloy_sol_types::SolValue;
            match <(u32, i32, Address)>::abi_decode(log_data, false) {
                Ok((fee, _tick_spacing, discovered_pool)) => {
                    if discovered_pool == pool_address {
                        // Found it! Extract token addresses from topics
                        if log.topics().len() >= 3 {
                            use alloy::primitives::FixedBytes;
                            
                            if let Ok(token0_bytes) = FixedBytes::<20>::try_from(&log.topics()[1][12..32]) {
                                if let Ok(token1_bytes) = FixedBytes::<20>::try_from(&log.topics()[2][12..32]) {
                                    let token0 = Address::from(token0_bytes);
                                    let token1 = Address::from(token1_bytes);
                                    
                                    info!("✅ Discovered V3 pool: {:?} (token0: {:?}, token1: {:?}, fee: {})",
                                          pool_address, token0, token1, fee);
                                    
                                    return Ok(Event::PoolCreated(super::pairs::V3PoolCreated {
                                        pair_address: pool_address,
                                        token0,
                                        token1,
                                        fee,
                                        tick_spacing: 0,
                                    }));
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        
        Err("V3 pool not found".into())
    }

    /// Try to discover V2 pair by looking for PairCreated events
    async fn try_discover_v2_pool(
        &self,
        pool_address: Address,
        from_block: u64,
        to_block: u64,
    ) -> Result<Event, Box<dyn std::error::Error>> {
        let v2_factory = crate::common::pools::UNISWAP_V2_FACTORY;
        let pair_created_sig = keccak256("PairCreated(address,address,address,uint256)".as_bytes());
        
        let filter = Filter::new()
            .address(v2_factory)
            .event_signature(vec![pair_created_sig])
            .from_block(from_block)
            .to_block(to_block);
        
        let logs = self.provider.get_logs(&filter).await?;
        
        for log in logs {
            // Check if this log contains our pool address
            let log_data = log.inner.data.data.to_vec();
            
            // V2 PairCreated: (address pair, uint idx)
            use alloy_sol_types::SolValue;
            match <(Address, u32)>::abi_decode(&log_data, false) {
                Ok((discovered_pair, _idx)) => {
                    if discovered_pair == pool_address {
                        // Found it! Extract token addresses from topics
                        if log.topics().len() >= 3 {
                            use alloy::primitives::FixedBytes;
                            
                            if let Ok(token0_bytes) = FixedBytes::<20>::try_from(&log.topics()[1][12..32]) {
                                if let Ok(token1_bytes) = FixedBytes::<20>::try_from(&log.topics()[2][12..32]) {
                                    let token0 = Address::from(token0_bytes);
                                    let token1 = Address::from(token1_bytes);
                                    
                                    info!("✅ Discovered V2 pair: {:?} (token0: {:?}, token1: {:?})",
                                          pool_address, token0, token1);
                                    
                                    return Ok(Event::PairCreated(super::pairs::V2PoolCreated {
                                        pair_address: pool_address,
                                        token0,
                                        token1,
                                        fee: 3000, // V2 default fee
                                    }));
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        
        Err("V2 pair not found".into())
    }

    /// Send a log event using the queue for better reliability
    fn send_log_event(&self, log_event: LogEvent) {
        // Try to send via queue first (non-blocking, reliable)
        match self.event_queue_tx.try_send(log_event.clone()) {
            Ok(_) => debug!("Log event queued successfully"),
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("Event queue full, falling back to broadcast (may drop)");
                // Fallback to broadcast if queue is full
                if let Err(e) = self.event_sender.send(log_event) {
                    warn!("Failed to send via broadcast channel: {}", e);
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Event queue closed, attempting broadcast");
                if let Err(e) = self.event_sender.send(log_event) {
                    error!("Both queue and broadcast failed: {}", e);
                }
            }
        }
    }
}

/// Main entry point for log processing - sets up subscription and processes incoming logs
pub async fn get_logs(
    client: Arc<RootProvider<PubSubFrontend>>,
    pairs: HashMap<Address, Event>,
    event_sender: Sender<LogEvent>,
    cancellation_token: tokio_util::sync::CancellationToken,
    cache_path: String,
) {
    info!("Starting log subscription service...");

    let (processor, mut event_queue_rx) = LogProcessor::new(pairs, event_sender.clone(), client.clone(), cache_path);

    // Spawn queue processor task to handle events from queue to broadcast
    let sender_clone = event_sender.clone();
    let cancellation_token_clone = cancellation_token.clone();
    let processor = Arc::new(processor);
    let processor_for_stream = processor.clone();
    
    tokio::spawn(async move {
        let mut processed_count = 0u64;
        let mut dropped_count = 0u64;

        loop {
            tokio::select! {
                Some(log_event) = event_queue_rx.recv() => {
                    match sender_clone.send(log_event) {
                        Ok(_) => {
                            processed_count += 1;
                            if processed_count % 100 == 0 {
                                debug!("Processed {} events from queue", processed_count);
                            }
                        }
                        Err(_) => {
                            dropped_count += 1;
                            if dropped_count % 10 == 0 {
                                warn!("Dropped {} events (no receivers)", dropped_count);
                            }
                        }
                    }
                }
                _ = cancellation_token_clone.cancelled() => {
                    info!("Queue processor shutdown requested");
                    break;
                }
            }
        }

        info!(
            "Queue processor stopped. Processed: {}, Dropped: {}",
            processed_count, dropped_count
        );
    });

    // Get current block number to ensure we capture all logs from this point forward
    let current_block = match client.get_block_number().await {
        Ok(block) => block,
        Err(e) => {
            warn!("Failed to get current block number: {}. Using 'latest' instead.", e);
            u64::MAX // Use MAX as sentinel, will be interpreted as 'latest'
        }
    };
    
    // Subscribe from a slightly earlier block to catch recent swaps that may have happened during startup
    // This helps capture swap events that occurred during arboo compilation and initialization
    let subscription_start_block = if current_block > 10 {
        current_block - 10
    } else {
        0
    };
    
    // Create event signature filters from the subscription start block onward
    let filter = create_swap_event_filter(subscription_start_block);
    info!("Log subscription will capture events from block {} onward (current block: {})", subscription_start_block, current_block);
    
    // Subscribe to logs
    let stream = match subscribe_to_logs(&client, filter).await {
        Ok(stream) => stream,
        Err(e) => {
            panic!("Critical: Cannot subscribe to blockchain logs: {}", e);
        }
    };

    info!("Log subscription established, processing incoming events...");

    // Process incoming log stream
    process_log_stream(stream, processor_for_stream, cancellation_token).await;
}

/// Create a filter for V2 and V3 Swap events starting from a specific block
fn create_swap_event_filter(from_block_num: u64) -> Filter {
    let v2_swap_signature =
        keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_signature =
        keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)".as_bytes());

    info!("V2 Swap signature: 0x{}", hex::encode(v2_swap_signature));
    info!("V3 Swap signature: 0x{}", hex::encode(v3_swap_signature));

    let block_tag = if from_block_num == u64::MAX {
        BlockNumberOrTag::Latest
    } else {
        BlockNumberOrTag::Number(from_block_num)
    };
    
    info!("Creating filter to capture logs from block: {:?}", block_tag);

    Filter::new()
        .event_signature(vec![v2_swap_signature, v3_swap_signature])
        // Subscribe from the current block onward to capture all new events
        .from_block(block_tag)
}

/// Subscribe to blockchain logs with error handling
async fn subscribe_to_logs(
    client: &Arc<RootProvider<PubSubFrontend>>,
    filter: Filter,
) -> Result<impl futures::Stream<Item = Log>, Box<dyn std::error::Error + Send + Sync>> {
    let subscription = client.subscribe_logs(&filter).await.map_err(|e| {
        log::error!("Failed to subscribe to logs: {}", e);
        e
    })?;

    Ok(subscription.into_stream())
}

/// Enhanced log processing loop with batching for high-frequency scenarios
async fn process_log_stream<S>(
    mut stream: S,
    processor: Arc<LogProcessor>,
    cancellation_token: tokio_util::sync::CancellationToken,
) where
    S: futures::Stream<Item = Log> + Unpin,
{
    let mut processed_count = 0u64;
    let mut opportunity_count = 0u64;
    let start_time = std::time::Instant::now();

    loop {
        tokio::select! {
            Some(log) = stream.next() => {
                processed_count += 1;
                info!("📥 Received log #{} from address: {:?}", processed_count, log.address());
                // Process the log and potentially create an arbitrage opportunity
                if let Some(log_event) = processor.process_log(&log).await {
                    opportunity_count += 1;

                    info!("Arbitrage opportunity detected #{}: V{} pool {:?} -> V{} counterpart {:?}",
                          opportunity_count,
                          if log_event.pool_variant == 2 { 2 } else { 3 },
                          log_event.log_pool_address,
                          if log_event.pool_variant == 2 { 3 } else { 2 },
                          log_event.corresponding_pool_address);
                    processor.send_log_event(log_event);
                }
                // Log processing stats every 1000 logs
                if processed_count % 1000 == 0 {
                    let elapsed = start_time.elapsed();
                    let logs_per_sec = processed_count as f64 / elapsed.as_secs_f64();
                    info!("Processing stats - Logs: {}, Opportunities: {}, Rate: {:.2}/sec",
                          processed_count, opportunity_count, logs_per_sec);
                }
            }
            _ = cancellation_token.cancelled() => {
                info!("Log stream processing shutdown requested");
                break;
            }
        }
    }

    info!(
        "Log stream ended after processing {} logs with {} opportunities",
        processed_count, opportunity_count
    );
}

/// Represents an arbitrage opportunity detected from blockchain logs
#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    /// Pool version that generated the log (2 for V2, 3 for V3)
    pub pool_variant: usize,
    /// Address of the counterpart pool for arbitrage
    pub corresponding_pool_address: Address,
    /// Address of the pool that generated the log event
    pub log_pool_address: Address,
    /// First token in the pair
    pub token0: Address,
    /// Second token in the pair
    pub token1: Address,
    /// Fee tier for the pool
    pub fee: U24,
}

impl LogEvent {
    /// Check if this represents a V2 to V3 arbitrage opportunity
    pub fn is_v2_to_v3(&self) -> bool {
        self.pool_variant == 2
    }

    /// Check if this represents a V3 to V2 arbitrage opportunity  
    pub fn is_v3_to_v2(&self) -> bool {
        self.pool_variant == 3
    }

    /// Get a description of the arbitrage direction
    pub fn arbitrage_direction(&self) -> &'static str {
        match self.pool_variant {
            2 => "V2->V3",
            3 => "V3->V2",
            _ => "Unknown",
        }
    }

    /// Validate that the log event has consistent data
    pub fn is_valid(&self) -> bool {
        self.token0 != self.token1
            && self.log_pool_address != Address::ZERO
            && self.corresponding_pool_address != Address::ZERO
            && (self.pool_variant == 2 || self.pool_variant == 3)
    }
}

// Import the MevEvent trait at the top of the file
use crate::strategies::traits::MevEvent;

impl MevEvent for LogEvent {
    fn event_type(&self) -> &str {
        "arbitrage_opportunity"
    }

    fn block_number(&self) -> u64 {
        // In a real implementation, this would come from the log data
        // For tests, we can use a default value
        0
    }

    fn transaction_index(&self) -> Option<u64> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_boxed(&self) -> Box<dyn MevEvent> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tokens() -> (Address, Address) {
        (
            Address::from([0x01; 20]), // WETH
            Address::from([0x02; 20]), // USDC
        )
    }

    #[test]
    fn test_token_pair_ordering() {
        let (token_a, token_b) = create_test_tokens();

        // Test that token pairs are consistently ordered
        let pair1 = TokenPair::new(token_a, token_b);
        let pair2 = TokenPair::new(token_b, token_a);

        assert_eq!(pair1, pair2, "Token pairs should be order-independent");
        assert_eq!(pair1.token0, token_a, "Lower address should be token0");
        assert_eq!(pair1.token1, token_b, "Higher address should be token1");
    }

    #[test]
    fn test_log_event_validation() {
        let (token0, token1) = create_test_tokens();

        let valid_event = LogEvent {
            pool_variant: 2,
            corresponding_pool_address: Address::from([0x10; 20]),
            log_pool_address: Address::from([0x20; 20]),
            token0,
            token1,
            fee: U24::from(3000),
        };

        assert!(
            valid_event.is_valid(),
            "Valid LogEvent should pass validation"
        );
        assert!(
            valid_event.is_v2_to_v3(),
            "Should identify V2->V3 direction"
        );
        assert_eq!(valid_event.arbitrage_direction(), "V2->V3");

        // Test invalid event (same tokens)
        let invalid_event = LogEvent {
            pool_variant: 2,
            corresponding_pool_address: Address::from([0x10; 20]),
            log_pool_address: Address::from([0x20; 20]),
            token0,
            token1: token0, // Same token
            fee: U24::from(3000),
        };

        assert!(
            !invalid_event.is_valid(),
            "Invalid LogEvent should fail validation"
        );
    }
}
