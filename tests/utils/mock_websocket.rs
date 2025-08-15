// Mock WebSocket Provider for Controlled Testing Scenarios
// Provides utilities for simulating WebSocket connections and events

use anyhow::{Result, Context};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use log::{info, debug, warn};
use alloy::primitives::{Address, U256, B256};
use rand::{Rng, thread_rng};

// Helper function to generate random B256
fn random_b256() -> B256 {
    let mut rng = thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    B256::from(bytes)
}

// Helper function to generate random Address
fn random_address() -> Address {
    let mut rng = thread_rng();
    let mut bytes = [0u8; 20];
    rng.fill(&mut bytes);
    Address::from(bytes)
}

/// Mock WebSocket server that simulates blockchain WebSocket providers
pub struct MockWebSocketProvider {
    port: u16,
    sender: broadcast::Sender<MockEvent>,
    receiver: broadcast::Receiver<MockEvent>,
    scenarios: Arc<Mutex<HashMap<String, MockScenario>>>,
    is_running: Arc<Mutex<bool>>,
}

/// Events that can be simulated by the mock provider
#[derive(Debug, Clone)]
pub enum MockEvent {
    NewBlock {
        number: u64,
        hash: B256,
        timestamp: u64,
        gas_limit: u64,
        base_fee: Option<u64>,
    },
    NewTransaction {
        hash: B256,
        from: Address,
        to: Option<Address>,
        value: U256,
        gas_price: u64,
        gas_limit: u64,
    },
    LogEvent {
        address: Address,
        topics: Vec<B256>,
        data: Vec<u8>,
        block_number: u64,
        transaction_hash: B256,
        log_index: u64,
    },
    ConnectionError {
        error_type: ConnectionErrorType,
        message: String,
    },
    SubscriptionResponse {
        subscription_id: String,
        result: Value,
    },
}

#[derive(Debug, Clone)]
pub enum ConnectionErrorType {
    NetworkTimeout,
    DnsFailure,
    ConnectionRefused,
    UnexpectedDisconnect,
    AuthenticationFailed,
}

/// Predefined scenarios that can be replayed
#[derive(Debug, Clone)]
pub struct MockScenario {
    pub name: String,
    pub events: Vec<(u64, MockEvent)>, // (delay_ms, event)
    pub duration_ms: u64,
    pub repeat: bool,
}

impl MockWebSocketProvider {
    /// Create a new mock WebSocket provider
    pub async fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        
        let (sender, receiver) = broadcast::channel(1000);
        let scenarios = Arc::new(Mutex::new(HashMap::new()));
        let is_running = Arc::new(Mutex::new(false));

        // Start the WebSocket server
        let sender_clone = sender.clone();
        tokio::spawn(Self::run_server(listener, sender_clone, scenarios.clone(), is_running.clone()));

        let mock_provider = Self {
            port,
            sender,
            receiver,
            scenarios,
            is_running,
        };

        info!("🎭 Mock WebSocket provider started on port {}", port);
        
        Ok(mock_provider)
    }

    /// Get the WebSocket URL for this mock provider
    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }

    /// Start broadcasting events from a predefined scenario
    pub async fn start_scenario(&self, scenario_name: &str) -> Result<()> {
        let scenario = {
            let scenarios = self.scenarios.lock().unwrap();
            scenarios.get(scenario_name)
                .cloned()
                .context("Scenario not found")?
        };

        info!("🎬 Starting scenario: {}", scenario_name);

        let sender = self.sender.clone();
        tokio::spawn(async move {
            loop {
                for (delay_ms, event) in &scenario.events {
                    tokio::time::sleep(tokio::time::Duration::from_millis(*delay_ms)).await;
                    if let Err(e) = sender.send(event.clone()) {
                        warn!("Failed to send mock event: {}", e);
                    }
                }

                if !scenario.repeat {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Send a single event immediately
    pub fn send_event(&self, event: MockEvent) -> Result<()> {
        self.sender.send(event)
            .context("Failed to send mock event")?;
        Ok(())
    }

    /// Add a predefined scenario
    pub fn add_scenario(&self, scenario: MockScenario) {
        let mut scenarios = self.scenarios.lock().unwrap();
        scenarios.insert(scenario.name.clone(), scenario);
    }

    /// Subscribe to events from this mock provider
    pub fn subscribe(&self) -> broadcast::Receiver<MockEvent> {
        self.sender.subscribe()
    }

    /// Simulate a series of new blocks
    pub async fn simulate_blocks(&self, start_block: u64, count: u64, interval_ms: u64) -> Result<()> {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            for i in 0..count {
                let block_number = start_block + i;
                let event = MockEvent::NewBlock {
                    number: block_number,
                    hash: random_b256(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    gas_limit: 30_000_000,
                    base_fee: Some(20_000_000_000), // 20 gwei
                };

                if let Err(e) = sender.send(event) {
                    warn!("Failed to send block event: {}", e);
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        });

        Ok(())
    }

    /// Simulate connection errors
    pub fn simulate_connection_error(&self, error_type: ConnectionErrorType, message: &str) -> Result<()> {
        let event = MockEvent::ConnectionError {
            error_type,
            message: message.to_string(),
        };
        self.send_event(event)
    }

    /// Stop the mock WebSocket provider
    pub async fn stop(self) -> Result<()> {
        info!("Stopping mock WebSocket provider on port {}", self.port);
        {
            let mut running = self.is_running.lock().unwrap();
            *running = false;
        }
        debug!("Mock WebSocket provider stopped");
        Ok(())
    }

    /// Internal server runner
    async fn run_server(
        listener: TcpListener,
        event_sender: broadcast::Sender<MockEvent>,
        scenarios: Arc<Mutex<HashMap<String, MockScenario>>>,
        is_running: Arc<Mutex<bool>>,
    ) {
        {
            let mut running = is_running.lock().unwrap();
            *running = true;
        }

        let mut event_receiver = event_sender.subscribe();

        while let Ok((stream, addr)) = listener.accept().await {
            debug!("📞 New WebSocket connection from {}", addr);
            
            let event_sender_clone = event_sender.clone();
            let mut event_receiver_clone = event_sender.subscribe();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, event_receiver_clone).await {
                    warn!("WebSocket connection error: {}", e);
                }
            });
        }
    }

    /// Handle individual WebSocket connections
    async fn handle_connection(
        stream: TcpStream,
        mut event_receiver: broadcast::Receiver<MockEvent>,
    ) -> Result<()> {
        let ws_stream = accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Handle incoming messages and outgoing events concurrently
        let send_task = tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                let message = Self::event_to_jsonrpc(&event);
                if let Ok(message_text) = serde_json::to_string(&message) {
                    if let Err(e) = ws_sender.send(Message::Text(message_text.into())).await {
                        warn!("Failed to send event: {}", e);
                        break;
                    }
                }
            }
        });

        let receive_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("� Received: {}", text);
                        // We could handle JSON-RPC requests here if needed
                    }
                    Ok(Message::Close(_)) => {
                        debug!("🔌 WebSocket connection closed");
                        break;
                    }
                    Err(e) => {
                        warn!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {},
            _ = receive_task => {},
        }

        Ok(())
    }

    /// Handle JSON-RPC requests from clients
    fn handle_jsonrpc_request(request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(json!(null));
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        
        match method {
            "eth_subscribe" => {
                // Return a mock subscription ID
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": format!("0x{:x}", rand::random::<u64>())
                })
            }
            "eth_unsubscribe" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": true
                })
            }
            "eth_blockNumber" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": format!("0x{:x}", chrono::Utc::now().timestamp() % 1000000)
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                })
            }
        }
    }

    /// Convert mock events to JSON-RPC notifications
    fn event_to_jsonrpc(event: &MockEvent) -> Value {
        match event {
            MockEvent::NewBlock { number, hash, timestamp, gas_limit, base_fee } => {
                json!({
                    "jsonrpc": "2.0",
                    "method": "eth_subscription",
                    "params": {
                        "subscription": "0x123",
                        "result": {
                            "number": format!("0x{:x}", number),
                            "hash": format!("0x{:x}", hash),
                            "timestamp": format!("0x{:x}", timestamp),
                            "gasLimit": format!("0x{:x}", gas_limit),
                            "baseFeePerGas": base_fee.map(|fee| format!("0x{:x}", fee))
                        }
                    }
                })
            }
            MockEvent::LogEvent { address, topics, data, block_number, transaction_hash, log_index } => {
                json!({
                    "jsonrpc": "2.0",
                    "method": "eth_subscription",
                    "params": {
                        "subscription": "0x456",
                        "result": {
                            "address": format!("0x{:x}", address),
                            "topics": topics.iter().map(|t| format!("0x{:x}", t)).collect::<Vec<_>>(),
                            "data": format!("0x{}", hex::encode(data)),
                            "blockNumber": format!("0x{:x}", block_number),
                            "transactionHash": format!("0x{:x}", transaction_hash),
                            "logIndex": format!("0x{:x}", log_index)
                        }
                    }
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "method": "mock_event",
                    "params": format!("{:?}", event)
                })
            }
        }
    }
}

/// Predefined mock scenarios
pub struct MockScenarios;

impl MockScenarios {
    /// Normal operation with regular blocks and some transactions
    pub fn normal_operation() -> MockScenario {
        MockScenario {
            name: "normal_operation".to_string(),
            events: vec![
                (0, MockEvent::NewBlock {
                    number: 1000,
                    hash: random_b256(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    gas_limit: 30_000_000,
                    base_fee: Some(20_000_000_000),
                }),
                (12000, MockEvent::NewBlock {
                    number: 1001,
                    hash: random_b256(),
                    timestamp: chrono::Utc::now().timestamp() as u64 + 12,
                    gas_limit: 30_000_000,
                    base_fee: Some(21_000_000_000),
                }),
            ],
            duration_ms: 24000,
            repeat: true,
        }
    }

    /// Network instability with connection errors
    pub fn network_instability() -> MockScenario {
        MockScenario {
            name: "network_instability".to_string(),
            events: vec![
                (0, MockEvent::NewBlock {
                    number: 2000,
                    hash: random_b256(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    gas_limit: 30_000_000,
                    base_fee: Some(25_000_000_000),
                }),
                (5000, MockEvent::ConnectionError {
                    error_type: ConnectionErrorType::NetworkTimeout,
                    message: "Connection timeout".to_string(),
                }),
                (10000, MockEvent::ConnectionError {
                    error_type: ConnectionErrorType::DnsFailure,
                    message: "DNS resolution failed".to_string(),
                }),
                (15000, MockEvent::NewBlock {
                    number: 2001,
                    hash: random_b256(),
                    timestamp: chrono::Utc::now().timestamp() as u64 + 15,
                    gas_limit: 30_000_000,
                    base_fee: Some(30_000_000_000),
                }),
            ],
            duration_ms: 20000,
            repeat: false,
        }
    }

    /// High-frequency trading scenario with many events
    pub fn high_frequency() -> MockScenario {
        let mut events = Vec::new();
        
        // Generate rapid-fire events
        for i in 0..100 {
            events.push((i * 100, MockEvent::NewTransaction {
                hash: random_b256(),
                from: random_address(),
                to: Some(random_address()),
                value: U256::from(1000000000000000000u64), // 1 ETH
                gas_price: 20_000_000_000 + (i * 1_000_000_000),
                gas_limit: 21000,
            }));
        }

        MockScenario {
            name: "high_frequency".to_string(),
            events,
            duration_ms: 10000,
            repeat: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_startup() -> Result<()> {
        let mock = MockWebSocketProvider::new().await?;
        assert!(mock.port > 0);
        assert!(mock.ws_url().starts_with("ws://127.0.0.1:"));
        Ok(())
    }

    #[tokio::test]
    async fn test_scenarios() {
        let normal = MockScenarios::normal_operation();
        assert_eq!(normal.name, "normal_operation");
        assert!(!normal.events.is_empty());

        let instability = MockScenarios::network_instability();
        assert_eq!(instability.name, "network_instability");
        assert!(!instability.events.is_empty());
    }
}
