// Multi-contract simulation framework for MEV strategies
// Supports dynamic contract deployment and execution for various arbitrage types

use crate::common::revm::{EvmSimulator, Tx};
use crate::arbitrage::simulation::{get_address, AddressType};
use alloy::eips::BlockId;
use alloy::providers::{Provider, RootProvider};
use alloy::pubsub::PubSubFrontend;
use alloy_primitives::{Address, U256};
use alloy_primitives::aliases::U24;
use alloy_sol_types::SolCall;
use anyhow::Result;
use log::{debug, info, warn, error};
use revm::primitives::Bytecode;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

/// Types of contracts we can simulate
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContractType {
    /// V3→V2 arbitrage (original arboo.sol)
    ArbitrageV3ToV2,
    /// V2→V3 arbitrage (V2FlashToV3Swap.sol)
    ArbitrageV2ToV3,
    /// Future: Sandwich attacks
    Sandwich,
    /// Future: Liquidation bots
    Liquidation,
    /// Future: Custom strategies
    Custom(String),
}

/// Contract metadata for simulation
#[derive(Debug, Clone)]
pub struct ContractMetadata {
    pub contract_type: ContractType,
    pub bytecode: Bytecode,
    pub constructor_params: Vec<u8>,
    pub deployment_gas_limit: u64,
    pub execution_gas_limit: u64,
}

/// Simulation context for a specific strategy execution
#[derive(Debug, Clone)]
pub struct SimulationContext {
    pub contract_address: Address,
    pub contract_type: ContractType,
    pub block_number: u64,
    pub gas_price: U256,
    pub base_fee: U256,
    pub caller: Address,
}

/// Result of a simulation run
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub profit: U256,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub state_changes: HashMap<Address, U256>, // For balance tracking
    pub execution_time: std::time::Duration,
    pub token_balances: HashMap<Address, U256>, // Track token balances after execution
}

impl Default for SimulationResult {
    fn default() -> Self {
        Self {
            success: false,
            gas_used: 0,
            profit: U256::ZERO,
            error: Some("No simulation performed".to_string()),
            logs: vec![],
            state_changes: HashMap::new(),
            execution_time: std::time::Duration::ZERO,
            token_balances: HashMap::new(),
        }
    }
}

/// Cached block data to avoid repeated network calls
#[derive(Debug, Clone)]
struct BlockCache {
    block_number: u64,
    timestamp: u64,
    gas_price: U256,
    gas_limit: u64,
    cached_at: std::time::Instant,
}

/// Global cache for block data - refreshed every 11 seconds (one block)
static CACHED_BLOCK_DATA: LazyLock<Mutex<Option<BlockCache>>> = LazyLock::new(|| Mutex::new(None));

/// Multi-contract simulation engine
pub struct MultiContractSimulator {
    /// Available contract types and their metadata
    contract_registry: HashMap<ContractType, ContractMetadata>,
    /// Pre-deployed contract addresses for reuse
    deployed_contracts: HashMap<ContractType, Address>,
    /// Simulation settings
    default_gas_limit: u64,
    default_gas_price: U256,
}

impl MultiContractSimulator {
    /// Create a new multi-contract simulator
    pub fn new() -> Self {
        Self {
            contract_registry: HashMap::new(),
            deployed_contracts: HashMap::new(),
            default_gas_limit: 2_000_000,
            default_gas_price: U256::from(20_000_000_000u64), // 20 gwei
        }
    }

    /// Register a contract type with its bytecode and metadata
    pub fn register_contract(
        &mut self,
        contract_type: ContractType,
        bytecode: Bytecode,
        constructor_params: Vec<u8>,
        deployment_gas_limit: Option<u64>,
        execution_gas_limit: Option<u64>,
    ) {
        let metadata = ContractMetadata {
            contract_type: contract_type.clone(),
            bytecode,
            constructor_params,
            deployment_gas_limit: deployment_gas_limit.unwrap_or(self.default_gas_limit),
            execution_gas_limit: execution_gas_limit.unwrap_or(self.default_gas_limit),
        };

        self.contract_registry.insert(contract_type, metadata.clone());
        info!("📝 Registered contract type: {:?}", metadata.contract_type);
    }

    /// Deploy a contract if not already deployed
    pub async fn ensure_contract_deployed(
        &mut self,
        simulator: &mut EvmSimulator<'_>,
        contract_type: &ContractType,
        force_redeploy: bool,
    ) -> Result<Address> {
        // Check if already deployed and not forcing redeploy
        if !force_redeploy {
            if let Some(&existing_address) = self.deployed_contracts.get(contract_type) {
                debug!("♻️ Reusing deployed contract at: {}", existing_address);
                return Ok(existing_address);
            }
        }

        // Get contract metadata
        let metadata = self.contract_registry
            .get(contract_type)
            .ok_or_else(|| anyhow::anyhow!("Contract type {:?} not registered", contract_type))?;

        // Deploy the contract
        let contract_address = simulator.contract_address;
        
        debug!("🚀 Deploying contract type: {:?} at {}", contract_type, contract_address);
        simulator.deploy_code_at(contract_address, metadata.bytecode.clone()).await;

        // Store deployed address
        self.deployed_contracts.insert(contract_type.clone(), contract_address);
        
        info!("✅ Deployed contract {:?} at: {}", contract_type, contract_address);
        Ok(contract_address)
    }

    /// Setup EVM environment for a specific contract type
    pub async fn setup_evm_for_contract(
        &mut self,
        simulator: &mut EvmSimulator<'_>,
        contract_type: &ContractType,
        initial_eth_balance: U256,
        approve_routers: bool,
    ) -> Result<SimulationContext> {
        // Ensure contract is deployed
        let contract_address = self.ensure_contract_deployed(simulator, contract_type, false).await?;

        // Fund wallet with ETH
        let wallet = simulator.owner;
        simulator.set_eth_balance(wallet, initial_eth_balance).await;
        debug!("💰 Funded wallet {} with {} ETH", wallet, initial_eth_balance);

        // Convert some ETH to WETH for trading
        if initial_eth_balance > U256::ZERO {
            self.convert_eth_to_weth(simulator, initial_eth_balance / U256::from(10)).await?;
        }

        // Approve routers if requested
        if approve_routers {
            self.approve_common_routers(simulator).await?;
        }

        // Create simulation context
        let context = SimulationContext {
            contract_address,
            contract_type: contract_type.clone(),
            block_number: 0, // Will be updated by caller
            gas_price: self.default_gas_price,
            base_fee: self.default_gas_price * U256::from(75) / U256::from(100), // 75% of gas price
            caller: wallet,
        };

        debug!("🎯 EVM setup complete for contract type: {:?}", contract_type);
        Ok(context)
    }

    /// Convert ETH to WETH for trading
    async fn convert_eth_to_weth(
        &self,
        simulator: &mut EvmSimulator<'_>,
        eth_amount: U256,
    ) -> Result<()> {
        alloy::sol! {
            function deposit() external payable;
        }

        let function_call = depositCall {};
        let wallet = simulator.owner;

        let eth_to_weth_tx = Tx {
            caller: wallet,
            transact_to: get_address(AddressType::Weth),
            data: function_call.abi_encode().into(),
            value: eth_amount,
            gas_limit: 100_000,
            gas_price: self.default_gas_price,
        };

        simulator.call(eth_to_weth_tx)?;
        debug!("🔄 Converted {} ETH to WETH", eth_amount);
        Ok(())
    }

    /// Approve common routers to spend tokens
    async fn approve_common_routers(&self, simulator: &mut EvmSimulator<'_>) -> Result<()> {
        alloy::sol! {
            function approve(address spender, uint256 amount) external returns (bool);
        }

        let wallet = simulator.owner;
        let routers = vec![
            get_address(AddressType::V3Router),
            get_address(AddressType::V2Router),
        ];

        for router in routers {
            let approve_data = approveCall {
                spender: router,
                amount: U256::MAX,
            }.abi_encode();

            let approve_tx = Tx {
                caller: wallet,
                transact_to: get_address(AddressType::Weth),
                data: approve_data.into(),
                value: U256::ZERO,
                gas_limit: 100_000,
                gas_price: self.default_gas_price,
            };

            simulator.call(approve_tx)?;
        }

        debug!("✅ Approved common routers for token spending");
        Ok(())
    }

    /// Execute a transaction with the specified contract
    pub async fn execute_contract_transaction(
        &self,
        simulator: &mut EvmSimulator<'_>,
        context: &SimulationContext,
        transaction_data: Vec<u8>,
        value: U256,
        gas_limit: Option<u64>,
    ) -> Result<SimulationResult> {
        let metadata = self.contract_registry
            .get(&context.contract_type)
            .ok_or_else(|| anyhow::anyhow!("Contract type {:?} not registered", context.contract_type))?;

        let gas_limit = gas_limit.unwrap_or(metadata.execution_gas_limit);
        let execution_start = std::time::Instant::now();

        let tx = Tx {
            caller: context.caller,
            transact_to: context.contract_address,
            data: transaction_data.into(),
            value,
            gas_limit,
            gas_price: context.gas_price,
        };

        // Track wallet balance before execution
        let balance_before = simulator.get_eth_balance(context.caller).await;

        // Execute transaction
        match simulator.call(tx) {
            Ok(result) => {
                let balance_after = simulator.get_eth_balance(context.caller).await;
                let profit = if balance_after > balance_before {
                    balance_after - balance_before
                } else {
                    U256::ZERO
                };

                let execution_time = execution_start.elapsed();

                Ok(SimulationResult {
                    success: true,
                    gas_used: result.gas_used,
                    profit,
                    error: None,
                    logs: vec![], // TODO: Extract logs from result
                    state_changes: HashMap::new(), // TODO: Track state changes
                    execution_time,
                    token_balances: HashMap::new(), // TODO: Track token balances
                })
            }
            Err(e) => {
                let error_msg = self.parse_execution_error(&e);
                warn!("❌ Contract execution failed: {}", error_msg);
                
                Ok(SimulationResult {
                    success: false,
                    gas_used: gas_limit,
                    profit: U256::ZERO,
                    error: Some(error_msg),
                    logs: vec![],
                    state_changes: HashMap::new(),
                    execution_time: execution_start.elapsed(),
                    token_balances: HashMap::new(),
                })
            }
        }
    }

    /// Execute arbitrage simulation with full market data integration
    pub async fn execute_arbitrage_simulation(
        &self,
        simulator: &mut EvmSimulator<'_>,
        provider: &RootProvider<PubSubFrontend>,
        context: &SimulationContext,
        target_pool: Address,
        token_a: Address,
        token_b: Address,
        amount: U256,
        fee: U24,
        enable_logging: bool,
    ) -> Result<SimulationResult> {
        let simulation_start = std::time::Instant::now();

        if enable_logging {
            info!(
                "🚀 Starting arbitrage simulation - Pool: {}, Token A: {}, Token B: {}, Amount: {} wei, Fee: {}",
                target_pool, token_a, token_b, amount, fee
            );
        }

        // Get latest block data (cached for performance)
        let (gas_limit, gas_price) = self.get_cached_block_data(provider).await?;

        // Check initial token balances
        let initial_token_balances = self.get_multiple_token_balances(
            simulator,
            &[token_a, token_b, get_address(AddressType::Weth)],
            context.caller
        ).await?;

        // Prepare arbitrage transaction based on contract type
        let transaction_data = match context.contract_type {
            ContractType::ArbitrageV3ToV2 => {
                alloy::sol! {
                    function flashSwap_V3_to_V2(
                        address pool0,
                        uint24 fee1,
                        address tokenIn,
                        address tokenOut,
                        uint256 amountIn,
                    ) external;
                }

                let function_call = flashSwap_V3_to_V2Call {
                    pool0: target_pool,
                    fee1: fee,
                    tokenIn: token_a,
                    tokenOut: token_b,
                    amountIn: amount,
                };
                function_call.abi_encode()
            }
            ContractType::ArbitrageV2ToV3 => {
                // Add V2 to V3 arbitrage call here
                alloy::sol! {
                    function flashSwap_V2_to_V3(
                        address pool0,
                        uint24 fee1,
                        address tokenIn,
                        address tokenOut,
                        uint256 amountIn,
                    ) external;
                }

                let function_call = flashSwap_V2_to_V3Call {
                    pool0: target_pool,
                    fee1: fee,
                    tokenIn: token_a,
                    tokenOut: token_b,
                    amountIn: amount,
                };
                function_call.abi_encode()
            }
            _ => return Err(anyhow::anyhow!("Unsupported contract type for arbitrage: {:?}", context.contract_type)),
        };

        // Execute the arbitrage transaction
        let tx = Tx {
            caller: context.caller,
            transact_to: context.contract_address,
            data: transaction_data.into(),
            value: U256::ZERO,
            gas_limit,
            gas_price,
        };

        match simulator.call(tx) {
            Ok(result) => {
                // Check final token balances
                let final_token_balances = self.get_multiple_token_balances(
                    simulator,
                    &[token_a, token_b, get_address(AddressType::Weth)],
                    context.caller
                ).await?;

                // Calculate profit in target token
                let target_token = if token_b == get_address(AddressType::Weth) { token_a } else { token_b };
                let profit = final_token_balances.get(&target_token).unwrap_or(&U256::ZERO)
                    .saturating_sub(*initial_token_balances.get(&target_token).unwrap_or(&U256::ZERO));

                let execution_time = simulation_start.elapsed();

                if enable_logging {
                    info!(
                        "✅ Arbitrage simulation complete - Profit: {} wei, Duration: {:?}",
                        profit, execution_time
                    );
                }

                Ok(SimulationResult {
                    success: true,
                    gas_used: result.gas_used,
                    profit,
                    error: None,
                    logs: vec![], // TODO: Extract detailed logs
                    state_changes: HashMap::new(),
                    execution_time,
                    token_balances: final_token_balances,
                })
            }
            Err(e) => {
                let error_msg = self.parse_execution_error(&e);
                error!("❌ Arbitrage simulation failed: {}", error_msg);
                
                Ok(SimulationResult {
                    success: false,
                    gas_used: gas_limit,
                    profit: U256::ZERO,
                    error: Some(error_msg),
                    logs: vec![],
                    state_changes: HashMap::new(),
                    execution_time: simulation_start.elapsed(),
                    token_balances: initial_token_balances,
                })
            }
        }
    }

    /// Get cached block data or fetch fresh data if cache is stale
    async fn get_cached_block_data(
        &self,
        provider: &RootProvider<PubSubFrontend>,
    ) -> Result<(u64, U256)> {
        // Check cache first
        let cache_data = {
            let cache = CACHED_BLOCK_DATA.lock().unwrap();
            cache.clone()
        };

        // Use cached data if less than 11 seconds old (one block)
        if let Some(block_cache) = cache_data {
            if block_cache.cached_at.elapsed().as_secs() < 11 {
                return Ok((block_cache.gas_limit, block_cache.gas_price));
            }
        }

        // Refresh cache - fetch new block data
        let latest_block_number = provider.get_block_number().await?;
        debug!("Fetched block number: {}", latest_block_number);

        let block_id = BlockId::from_str(&latest_block_number.to_string())
            .map_err(|e| anyhow::anyhow!("Invalid block number format: {}", e))?;
        
        let latest_block = provider
            .get_block(block_id, alloy::rpc::types::BlockTransactionsKind::Full)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to get block"))?;

        let gas_limit = latest_block.header.gas_limit;
        let gas_price = U256::from(
            latest_block.header.base_fee_per_gas
                .ok_or_else(|| anyhow::anyhow!("Block missing base fee"))?
        );
        let timestamp = latest_block.header.timestamp;

        // Update cache
        {
            let mut cache = CACHED_BLOCK_DATA.lock().unwrap();
            *cache = Some(BlockCache {
                block_number: latest_block_number,
                timestamp,
                gas_price,
                gas_limit,
                cached_at: std::time::Instant::now(),
            });
        }

        debug!("Refreshed block cache - Gas limit: {}, Gas price: {}", gas_limit, gas_price);
        Ok((gas_limit, gas_price))
    }

    /// Check balance of a specific token for an account
    pub async fn get_token_balance(
        &self,
        simulator: &mut EvmSimulator<'_>,
        token: Address,
        account: Address,
    ) -> Result<U256> {
        alloy::sol! {
            function balanceOf(address account) external view returns (uint256);
        }

        let balance_call = balanceOfCall { account };
        let call_data = balance_call.abi_encode();

        let tx = Tx {
            caller: simulator.owner,
            transact_to: token,
            data: call_data.into(),
            value: U256::ZERO,
            gas_limit: 100_000,
            gas_price: self.default_gas_price,
        };

        match simulator.staticcall(tx) {
            Ok(result) => {
                if result.output.len() >= 32 {
                    Ok(U256::from_be_slice(&result.output[..32]))
                } else {
                    Ok(U256::ZERO)
                }
            }
            Err(_) => Ok(U256::ZERO),
        }
    }

    /// Get balances for multiple tokens efficiently
    pub async fn get_multiple_token_balances(
        &self,
        simulator: &mut EvmSimulator<'_>,
        tokens: &[Address],
        account: Address,
    ) -> Result<HashMap<Address, U256>> {
        let mut balances = HashMap::new();
        
        for &token in tokens {
            let balance = self.get_token_balance(simulator, token, account).await?;
            balances.insert(token, balance);
        }

        Ok(balances)
    }

    /// Check WETH balance optimized for frequent calls
    pub async fn check_weth_balance_optimized(
        &self,
        simulator: &mut EvmSimulator<'_>,
        wallet_address: Address,
        gas_limit: u64,
        gas_price: U256,
    ) -> Result<U256> {
        alloy::sol! {
            function balanceOf(address account) external view returns (uint256);
        }

        let tx = Tx {
            caller: wallet_address,
            transact_to: get_address(AddressType::Weth),
            data: balanceOfCall { account: wallet_address }.abi_encode().into(),
            value: U256::ZERO,
            gas_limit,
            gas_price,
        };

        let result = simulator.call(tx)
            .map_err(|e| anyhow::anyhow!("Failed to check WETH balance: {}", e))?;

        Ok(U256::from_be_slice(&result.output))
    }

    /// Parse execution errors to extract meaningful error messages
    fn parse_execution_error(&self, error: &anyhow::Error) -> String {
        let error_str = format!("{:?}", error);
        
        if error_str.contains("EVM REVERT:") {
            if let Some(start) = error_str.find("0x") {
                if let Some(end) = error_str[start..].find(" / Gas used:") {
                    let hex_data = &error_str[start..start + end];
                    if let Ok(decoded) = crate::common::decode_result::decode_revert_hex(hex_data) {
                        return format!("EVM Revert: {}", decoded);
                    } else {
                        return format!("EVM Revert (raw): {}", hex_data);
                    }
                }
            }
        }
        
        error.to_string()
    }

    /// Get list of registered contract types
    pub fn get_registered_contracts(&self) -> Vec<ContractType> {
        self.contract_registry.keys().cloned().collect()
    }

    /// Get deployed contract address for a type
    pub fn get_deployed_address(&self, contract_type: &ContractType) -> Option<Address> {
        self.deployed_contracts.get(contract_type).copied()
    }

    /// Clear deployed contracts (useful for testing)
    pub fn clear_deployed_contracts(&mut self) {
        self.deployed_contracts.clear();
        debug!("🧹 Cleared all deployed contracts");
    }

    /// Update gas settings
    pub fn update_gas_settings(&mut self, gas_limit: u64, gas_price: U256) {
        self.default_gas_limit = gas_limit;
        self.default_gas_price = gas_price;
        debug!("⛽ Updated gas settings: limit={}, price={}", gas_limit, gas_price);
    }
}

impl Default for MultiContractSimulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a simulator with common arbitrage contracts pre-registered
pub fn create_arbitrage_simulator() -> MultiContractSimulator {
    let simulator = MultiContractSimulator::new();
    
    // Note: Bytecode registration will be done by the calling code
    // since we need to import the actual bytecode functions
    
    simulator
}

/// Utility functions for common ETH amounts
pub mod amounts {
    use alloy_primitives::U256;

    pub fn one_ether() -> U256 {
        U256::from(10).pow(U256::from(18)) // 1e18
    }

    pub fn one_hundred_ether() -> U256 {
        U256::from(100) * U256::from(10).pow(U256::from(18)) // 100e18
    }

    pub fn five_hundred_ether() -> U256 {
        U256::from(500) * U256::from(10).pow(U256::from(18)) // 500e18
    }

    pub fn one_thousand_ether() -> U256 {
        U256::from(1000) * U256::from(10).pow(U256::from(18)) // 1000e18
    }

    pub fn fifty_thousand_ether() -> U256 {
        U256::from(50000) * U256::from(10).pow(U256::from(18)) // 50000e18
    }

    pub fn five_hundred_thousand_ether() -> U256 {
        U256::from(500000) * U256::from(10).pow(U256::from(18)) // 500000e18
    }

    pub fn wei_to_ether(wei: U256) -> f64 {
        // Convert U256 to string first, then parse as f64
        let wei_str = wei.to_string();
        let wei_f64: f64 = wei_str.parse().unwrap_or(0.0);
        wei_f64 / 1e18
    }

    pub fn ether_to_wei(ether: f64) -> U256 {
        U256::from((ether * 1e18) as u128)
    }
}

/// Data parsing utilities for simulation results
pub mod parsing {
    use alloy_primitives::U256;

    #[derive(Debug)]
    pub enum ParserType {
        UTF8,
        U256,
        Address,
        Bool,
    }

    #[derive(Debug)]
    pub struct ParserInput<'a> {
        parser_type: ParserType,
        data: &'a [u8],
    }

    impl<'a> ParserInput<'a> {
        pub fn new(parser_type: ParserType, data: &'a [u8]) -> Self {
            Self { parser_type, data }
        }
    }

    pub fn parse_data(inputs: Vec<ParserInput>) -> Vec<String> {
        inputs
            .iter()
            .map(|input| match input.parser_type {
                ParserType::UTF8 => String::from_utf8(input.data.to_vec())
                    .unwrap_or_else(|_| "Invalid UTF-8".to_string()),
                ParserType::U256 => U256::from_be_slice(input.data).to_string(),
                ParserType::Address => {
                    if input.data.len() >= 20 {
                        format!("0x{}", hex::encode(&input.data[input.data.len()-20..]))
                    } else {
                        "Invalid Address".to_string()
                    }
                },
                ParserType::Bool => {
                    if input.data.len() >= 32 {
                        let val = U256::from_be_slice(&input.data[..32]);
                        (val != U256::ZERO).to_string()
                    } else {
                        "false".to_string()
                    }
                },
            })
            .collect()
    }
}

/// Simulation metrics and performance tracking
#[derive(Debug, Clone)]
pub struct SimulationMetrics {
    pub total_simulations: usize,
    pub successful_simulations: usize,
    pub failed_simulations: usize,
    pub total_profit: U256,
    pub total_gas_used: u64,
    pub average_execution_time: std::time::Duration,
    pub best_profit: U256,
    pub worst_loss: U256,
}

impl Default for SimulationMetrics {
    fn default() -> Self {
        Self {
            total_simulations: 0,
            successful_simulations: 0,
            failed_simulations: 0,
            total_profit: U256::ZERO,
            total_gas_used: 0,
            average_execution_time: std::time::Duration::ZERO,
            best_profit: U256::ZERO,
            worst_loss: U256::ZERO,
        }
    }
}

impl SimulationMetrics {
    pub fn record_result(&mut self, result: &SimulationResult) {
        self.total_simulations += 1;
        self.total_gas_used += result.gas_used;
        
        if result.success {
            self.successful_simulations += 1;
            self.total_profit += result.profit;
            
            if result.profit > self.best_profit {
                self.best_profit = result.profit;
            }
        } else {
            self.failed_simulations += 1;
            // Track losses if profit is negative (though U256 can't be negative, we track zero profits as potential losses)
        }

        // Update average execution time
        let total_time = self.average_execution_time * (self.total_simulations - 1) as u32 + result.execution_time;
        self.average_execution_time = total_time / self.total_simulations as u32;
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_simulations == 0 {
            0.0
        } else {
            self.successful_simulations as f64 / self.total_simulations as f64
        }
    }

    pub fn average_profit(&self) -> U256 {
        if self.successful_simulations == 0 {
            U256::ZERO
        } else {
            self.total_profit / U256::from(self.successful_simulations)
        }
    }
}

/// Batch simulation runner for testing multiple scenarios
pub struct BatchSimulationRunner {
    pub simulator: MultiContractSimulator,
    pub metrics: SimulationMetrics,
    pub results: Vec<SimulationResult>,
}

impl BatchSimulationRunner {
    pub fn new() -> Self {
        Self {
            simulator: MultiContractSimulator::new(),
            metrics: SimulationMetrics::default(),
            results: Vec::new(),
        }
    }

    pub async fn run_batch_simulations<F>(
        &mut self,
        scenarios: Vec<F>,
    ) -> Result<&SimulationMetrics>
    where
        F: std::future::Future<Output = Result<SimulationResult>>,
    {
        for scenario in scenarios {
            match scenario.await {
                Ok(result) => {
                    self.metrics.record_result(&result);
                    self.results.push(result);
                }
                Err(e) => {
                    warn!("Batch simulation scenario failed: {}", e);
                    let failed_result = SimulationResult {
                        success: false,
                        error: Some(e.to_string()),
                        ..Default::default()
                    };
                    self.metrics.record_result(&failed_result);
                    self.results.push(failed_result);
                }
            }
        }

        Ok(&self.metrics)
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.metrics = SimulationMetrics::default();
    }

    pub fn get_successful_results(&self) -> Vec<&SimulationResult> {
        self.results.iter().filter(|r| r.success).collect()
    }

    pub fn get_profitable_results(&self, min_profit: U256) -> Vec<&SimulationResult> {
        self.results.iter()
            .filter(|r| r.success && r.profit >= min_profit)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitrage::simulation::{arboo_bytecode, v2_flash_to_v3_swap_bytecode};

    #[test]
    fn test_contract_registration() {
        let mut simulator = MultiContractSimulator::new();
        
        let bytecode = Bytecode::default();
        simulator.register_contract(
            ContractType::ArbitrageV3ToV2,
            bytecode,
            vec![],
            Some(1_000_000),
            Some(500_000),
        );

        assert_eq!(simulator.get_registered_contracts().len(), 1);
        assert!(simulator.get_registered_contracts().contains(&ContractType::ArbitrageV3ToV2));
    }

    #[test]
    fn test_contract_type_equality() {
        assert_eq!(ContractType::ArbitrageV3ToV2, ContractType::ArbitrageV3ToV2);
        assert_ne!(ContractType::ArbitrageV3ToV2, ContractType::ArbitrageV2ToV3);
        
        let custom1 = ContractType::Custom("test".to_string());
        let custom2 = ContractType::Custom("test".to_string());
        let custom3 = ContractType::Custom("other".to_string());
        
        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
    }

    #[test]
    fn test_simulation_result_default() {
        let result = SimulationResult::default();
        assert!(!result.success);
        assert_eq!(result.gas_used, 0);
        assert_eq!(result.profit, U256::ZERO);
        assert!(result.error.is_some());
        assert_eq!(result.logs.len(), 0);
        assert_eq!(result.state_changes.len(), 0);
        assert_eq!(result.execution_time, std::time::Duration::ZERO);
        assert_eq!(result.token_balances.len(), 0);
    }

    #[test]
    fn test_simulation_metrics() {
        let mut metrics = SimulationMetrics::default();
        
        // Test successful simulation
        let successful_result = SimulationResult {
            success: true,
            gas_used: 100000,
            profit: U256::from(1000),
            execution_time: std::time::Duration::from_millis(50),
            ..Default::default()
        };
        
        metrics.record_result(&successful_result);
        assert_eq!(metrics.total_simulations, 1);
        assert_eq!(metrics.successful_simulations, 1);
        assert_eq!(metrics.failed_simulations, 0);
        assert_eq!(metrics.success_rate(), 1.0);
        assert_eq!(metrics.best_profit, U256::from(1000));

        // Test failed simulation
        let failed_result = SimulationResult {
            success: false,
            gas_used: 50000,
            execution_time: std::time::Duration::from_millis(25),
            ..Default::default()
        };
        
        metrics.record_result(&failed_result);
        assert_eq!(metrics.total_simulations, 2);
        assert_eq!(metrics.successful_simulations, 1);
        assert_eq!(metrics.failed_simulations, 1);
        assert_eq!(metrics.success_rate(), 0.5);
    }

    #[test]
    fn test_amount_utilities() {
        use super::amounts::*;
        
        assert_eq!(one_ether(), U256::from(10).pow(U256::from(18)));
        assert_eq!(one_hundred_ether(), U256::from(100) * one_ether());
        assert_eq!(five_hundred_ether(), U256::from(500) * one_ether());
        
        // Test conversions
        let wei_amount = one_ether();
        let ether_float = wei_to_ether(wei_amount);
        assert!((ether_float - 1.0).abs() < 1e-10);
        
        let converted_back = ether_to_wei(ether_float);
        assert_eq!(converted_back, wei_amount);
    }

    #[test]
    fn test_data_parsing() {
        use super::parsing::*;
        
        // Test U256 parsing
        let u256_bytes = U256::from(12345).to_be_bytes::<32>();
        let u256_input = ParserInput::new(ParserType::U256, &u256_bytes);
        let results = parse_data(vec![u256_input]);
        assert_eq!(results[0], "12345");
        
        // Test UTF8 parsing
        let utf8_bytes = b"Hello, World!";
        let utf8_input = ParserInput::new(ParserType::UTF8, utf8_bytes);
        let results = parse_data(vec![utf8_input]);
        assert_eq!(results[0], "Hello, World!");
        
        // Test Bool parsing
        let bool_true_bytes = U256::from(1).to_be_bytes::<32>();
        let bool_false_bytes = U256::from(0).to_be_bytes::<32>();
        let bool_true_input = ParserInput::new(ParserType::Bool, &bool_true_bytes);
        let bool_false_input = ParserInput::new(ParserType::Bool, &bool_false_bytes);
        let results = parse_data(vec![bool_true_input, bool_false_input]);
        assert_eq!(results[0], "true");
        assert_eq!(results[1], "false");
    }

    #[test]
    fn test_batch_simulation_runner() {
        let mut runner = BatchSimulationRunner::new();
        assert_eq!(runner.metrics.total_simulations, 0);
        assert_eq!(runner.results.len(), 0);
        
        // Test clearing results
        runner.clear_results();
        assert_eq!(runner.metrics.total_simulations, 0);
        assert_eq!(runner.results.len(), 0);
    }

    #[test]
    fn test_multi_contract_simulator_gas_settings() {
        let mut simulator = MultiContractSimulator::new();
        
        // Test default settings
        assert_eq!(simulator.default_gas_limit, 2_000_000);
        assert_eq!(simulator.default_gas_price, U256::from(20_000_000_000u64));
        
        // Test updating settings
        simulator.update_gas_settings(3_000_000, U256::from(30_000_000_000u64));
        assert_eq!(simulator.default_gas_limit, 3_000_000);
        assert_eq!(simulator.default_gas_price, U256::from(30_000_000_000u64));
    }

    #[test] 
    fn test_create_arbitrage_simulator() {
        let simulator = create_arbitrage_simulator();
        assert_eq!(simulator.get_registered_contracts().len(), 0);
        assert_eq!(simulator.default_gas_limit, 2_000_000);
    }

    #[tokio::test]
    async fn test_contract_registration_with_real_bytecode() {
        let mut simulator = MultiContractSimulator::new();
        
        // Test with real arbitrage bytecode
        let arboo_bytecode = arboo_bytecode();
        simulator.register_contract(
            ContractType::ArbitrageV3ToV2,
            arboo_bytecode,
            vec![],
            Some(2_000_000),
            Some(1_500_000),
        );

        let v2_v3_bytecode = v2_flash_to_v3_swap_bytecode();
        simulator.register_contract(
            ContractType::ArbitrageV2ToV3,
            v2_v3_bytecode,
            vec![],
            Some(2_500_000),
            Some(1_800_000),
        );

        assert_eq!(simulator.get_registered_contracts().len(), 2);
        assert!(simulator.get_registered_contracts().contains(&ContractType::ArbitrageV3ToV2));
        assert!(simulator.get_registered_contracts().contains(&ContractType::ArbitrageV2ToV3));
    }
}
