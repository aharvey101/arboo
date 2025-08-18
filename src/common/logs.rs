use super::pairs::Event;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::{Filter, Log};
use alloy_primitives::aliases::U24;
use futures::StreamExt;
use log::{info, warn, debug};
use revm::primitives::keccak256;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::broadcast::Sender;

/// Core log processing service for arbitrage opportunity detection
pub struct LogProcessor {
    pairs: HashMap<Address, Event>,
    token_pair_index: HashMap<TokenPair, Vec<Address>>,
    event_sender: Sender<LogEvent>,
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
            Self { token0: token_a, token1: token_b }
        } else {
            Self { token0: token_b, token1: token_a }
        }
    }
}

impl LogProcessor {
    /// Create a new log processor with the given pairs and event sender
    pub fn new(pairs: HashMap<Address, Event>, event_sender: Sender<LogEvent>) -> Self {
        let token_pair_index = Self::build_token_pair_index(&pairs);
        
        info!("LogProcessor initialized with {} pairs and {} unique token pairs", 
              pairs.len(), token_pair_index.len());
        
        Self {
            pairs,
            token_pair_index,
            event_sender,
        }
    }

    /// Build an efficient index of token pairs to pool addresses
    fn build_token_pair_index(pairs: &HashMap<Address, Event>) -> HashMap<TokenPair, Vec<Address>> {
        let mut index = HashMap::new();
        
        for (pool_address, event) in pairs {
            let token_pair = match event {
                Event::PairCreated(pair) => TokenPair::new(pair.token0, pair.token1),
                Event::PoolCreated(pool) => TokenPair::new(pool.token0, pool.token1),
            };
            
            index.entry(token_pair).or_insert_with(Vec::new).push(*pool_address);
        }
        
        debug!("Built token pair index with {} unique pairs", index.len());
        index
    }

    /// Process a single log event and attempt to create arbitrage opportunities
    pub fn process_log(&self, log: &Log) -> Option<LogEvent> {
        let pool_address = log.address();
        
        // Look up the pool that generated this log
        let source_event = self.pairs.get(&pool_address)?;
        debug!("Processing log from pool: {:?}", pool_address);

        match source_event {
            Event::PairCreated(v2_pool) => {
                self.find_arbitrage_for_v2_pool(v2_pool, pool_address)
            }
            Event::PoolCreated(v3_pool) => {
                self.find_arbitrage_for_v3_pool(v3_pool, pool_address)
            }
        }
    }

    /// Find V3 counterpart for a V2 pool to create arbitrage opportunity
    fn find_arbitrage_for_v2_pool(
        &self, 
        v2_pool: &super::pairs::V2PoolCreated, 
        pool_address: Address
    ) -> Option<LogEvent> {
        let token_pair = TokenPair::new(v2_pool.token0, v2_pool.token1);
        
        // Find all pools with the same token pair
        let matching_pools = self.token_pair_index.get(&token_pair)?;
        
        // Look for a V3 pool among the matching pools
        for &candidate_address in matching_pools {
            if candidate_address == pool_address {
                continue; // Skip self
            }
            
            if let Some(Event::PoolCreated(v3_pool)) = self.pairs.get(&candidate_address) {
                // Validate token pair consistency
                if Self::is_valid_token_pair(v3_pool.token0, v3_pool.token1) {
                    debug!("Found V2->V3 arbitrage: {:?} -> {:?}", pool_address, candidate_address);
                    
                    return Some(LogEvent {
                        pool_variant: 2, // V2 pool generated the log
                        corresponding_pool_address: v3_pool.pair_address,
                        log_pool_address: pool_address,
                        token0: v2_pool.token0,
                        token1: v2_pool.token1,
                        fee: U24::from(v2_pool.fee),
                    });
                }
            }
        }
        
        debug!("No V3 counterpart found for V2 pool: {:?}", pool_address);
        None
    }

    /// Find V2 counterpart for a V3 pool to create arbitrage opportunity
    fn find_arbitrage_for_v3_pool(
        &self, 
        v3_pool: &super::pairs::V3PoolCreated, 
        pool_address: Address
    ) -> Option<LogEvent> {
        let token_pair = TokenPair::new(v3_pool.token0, v3_pool.token1);
        
        // Find all pools with the same token pair
        let matching_pools = self.token_pair_index.get(&token_pair)?;
        
        // Look for a V2 pool among the matching pools
        for &candidate_address in matching_pools {
            if candidate_address == pool_address {
                continue; // Skip self
            }
            
            if let Some(Event::PairCreated(v2_pool)) = self.pairs.get(&candidate_address) {
                debug!("Found V3->V2 arbitrage: {:?} -> {:?}", pool_address, candidate_address);
                
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

    /// Send a log event if the channel is available
    fn send_log_event(&self, log_event: LogEvent) {
        match self.event_sender.send(log_event) {
            Ok(_) => debug!("Log event sent successfully"),
            Err(e) => warn!("Failed to send log event: {}", e),
        }
    }
}

/// Main entry point for log processing - sets up subscription and processes incoming logs
pub async fn get_logs(
    client: Arc<RootProvider<PubSubFrontend>>,
    pairs: HashMap<Address, Event>,
    event_sender: Sender<LogEvent>,
) {
    info!("Starting log subscription service...");
    
    let processor = LogProcessor::new(pairs, event_sender);
    
    // Create event signature filters
    let filter = create_swap_event_filter();
    
    // Subscribe to logs
    let stream = match subscribe_to_logs(&client, filter).await {
        Ok(stream) => stream,
        Err(e) => {
            panic!("Critical: Cannot subscribe to blockchain logs: {}", e);
        }
    };
    
    info!("Log subscription established, processing incoming events...");
    
    // Process incoming log stream
    process_log_stream(stream, processor).await;
}

/// Create a filter for V2 and V3 Swap events
fn create_swap_event_filter() -> Filter {
    let v2_swap_signature = keccak256("Swap(address,uint256,uint256,uint256,uint256,address)".as_bytes());
    let v3_swap_signature = keccak256("Swap(address,address,int256,int256,uint160,uint160,int24)".as_bytes());
    
    Filter::new()
        .event_signature(vec![v2_swap_signature, v3_swap_signature])
        .from_block(BlockNumberOrTag::Latest)
}

/// Subscribe to blockchain logs with error handling
async fn subscribe_to_logs(
    client: &Arc<RootProvider<PubSubFrontend>>, 
    filter: Filter
) -> Result<impl futures::Stream<Item = Log>, Box<dyn std::error::Error + Send + Sync>> {
    let subscription = client.subscribe_logs(&filter).await
        .map_err(|e| {
            log::error!("Failed to subscribe to logs: {}", e);
            e
        })?;
    
    Ok(subscription.into_stream())
}

/// Main log processing loop
async fn process_log_stream<S>(mut stream: S, processor: LogProcessor)
where
    S: futures::Stream<Item = Log> + Unpin,
{
    while let Some(log) = stream.next().await {
        // Process the log and potentially create an arbitrage opportunity
        if let Some(log_event) = processor.process_log(&log) {
            info!("Arbitrage opportunity detected: V{} pool {:?} -> V{} counterpart {:?}", 
                  if log_event.pool_variant == 2 { 2 } else { 3 },
                  log_event.log_pool_address,
                  if log_event.pool_variant == 2 { 3 } else { 2 },
                  log_event.corresponding_pool_address);
            
            processor.send_log_event(log_event);
        }
    }
    
    warn!("Log stream ended unexpectedly");
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
        self.token0 != self.token1 && 
        self.log_pool_address != Address::ZERO &&
        self.corresponding_pool_address != Address::ZERO &&
        (self.pool_variant == 2 || self.pool_variant == 3)
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
        
        assert!(valid_event.is_valid(), "Valid LogEvent should pass validation");
        assert!(valid_event.is_v2_to_v3(), "Should identify V2->V3 direction");
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
        
        assert!(!invalid_event.is_valid(), "Invalid LogEvent should fail validation");
    }
}
