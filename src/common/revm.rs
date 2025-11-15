use crate::common::bytecode::arboo_bytecode;
use crate::common::revm_inspector;
use crate::common::simulation::{get_address, AddressType};
use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::primitives::{Address, U64};
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::signers::local::PrivateKeySigner;
use alloy_sol_types::SolCall;
use anyhow::{anyhow, Error, Result};
use revm::inspector_handle_register;

use revm::db::{AlloyDB, CacheDB};
use revm::primitives::{Bytes, Log};
use revm::{
    primitives::{AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256, U256},
    Database, Evm,
};
use std::str::FromStr;

#[derive(Debug, Clone, Default)]
pub struct VictimTx {
    pub tx_hash: B256,
    pub from: Address,
    pub to: Address,
    pub data: Bytes,
    pub value: U256,
    pub gas_price: U256,
    pub gas_limit: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Tx {
    pub caller: Address,
    pub transact_to: Address,
    pub data: Bytes,
    pub value: U256,
    pub gas_price: U256,
    pub gas_limit: u64,
}

impl Tx {
    pub fn from(tx: VictimTx) -> Self {
        let gas_limit = tx.gas_limit.unwrap_or(5000000);
        Self {
            caller: tx.from,
            transact_to: tx.to,
            data: tx.data,
            value: tx.value,
            gas_price: tx.gas_price,
            gas_limit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxResult {
    pub output: Bytes,
    pub logs: Option<Vec<Log>>,
    pub gas_used: u64,
    pub gas_refunded: u64,
}

// type My_Evm_Context = EvmContext<CacheDB<AlloyDB<Client, AnyNetwork, RootProvider<PubSubFrontend>>>>;

#[derive(Debug)]
pub struct EvmSimulator<'a> {
    pub owner: Address,
    pub contract_address: Address,
    pub evm: Evm<
        'a,
        revm_inspector::RevmInspector,
        CacheDB<AlloyDB<PubSubFrontend, Ethereum, RootProvider<PubSubFrontend, Ethereum>>>,
    >,
    pub block_number: U64,
}
impl EvmSimulator<'_> {
    pub fn new(
        provider: RootProvider<PubSubFrontend, Ethereum>,
        owner: Option<Address>,
        block_number: U64,
    ) -> Result<Self, anyhow::Error> {
        EvmSimulator::new_with_db(owner, block_number, provider)
    }

    pub async fn setup(&mut self) {
        self.deploy_code_at(self.contract_address, arboo_bytecode())
            .await;

        let initial_eth_balance = U256::from(100_000_000) * U256::from(10).pow(U256::from(18));

        self.set_eth_balance(self.owner, initial_eth_balance).await;

        alloy::sol! {
            function swapEthForWeth(
                address to,
                uint256 deadline
            ) external payable;
        };
        let function_call = swapEthForWethCall {
            to: self.owner,
            deadline: U256::from(9999999999_u64),
        };

        let function_call_data = function_call.abi_encode();

        let new_tx = Tx {
            caller: self.owner,
            transact_to: get_address(AddressType::Weth),
            data: function_call_data.into(),
            value: U256::from(10_000_000) * U256::from(10).pow(U256::from(18)),
            gas_limit: 50_000_000u64,
            gas_price: U256::from(10000000000u128),
        };

        self.call(new_tx)
            .map_err(|e| log::error!("Failed to initialize WETH swap: {}", e))
            .ok(); // Non-critical setup failure, continue execution
    }

    pub fn new_with_db(
        owner: Option<Address>,
        block_number: U64,
        provider: RootProvider<PubSubFrontend, Ethereum>,
    ) -> Result<Self, anyhow::Error> {
        let owner = match owner {
            Some(owner) => owner,
            None => PrivateKeySigner::random().address(),
        };
        let contract_wallet = PrivateKeySigner::random();
        let inspector = revm_inspector::RevmInspector::new();

        // Create AlloyDB without creating a new runtime - use the current runtime context
        let alloy_db = AlloyDB::new(provider, BlockId::from(block_number)).ok_or_else(|| {
            anyhow::anyhow!("Failed to create AlloyDB - current runtime may be incompatible")
        })?;

        let cache_db = CacheDB::new(alloy_db);

        let evm = Evm::builder()
            .with_db(cache_db)
            .with_external_context(inspector)
            .append_handler_register(inspector_handle_register)
            .modify_env(|env| {
                env.block.number = U256::from(block_number);
                env.block.coinbase =
                    Address::from_str("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5")
                        .unwrap_or_else(|_| Address::ZERO); // Use zero address as fallback
            })
            .build();

        //let evm = TokioMutex::new(evm);

        Ok(Self {
            owner,
            evm,
            block_number,
            contract_address: contract_wallet.address(),
        })
    }

    pub async fn get_block_number(&mut self) -> U256 {
        self.evm.block().number
    }

    pub async fn get_coinbase(&mut self) -> Address {
        self.evm.block().coinbase
    }

    pub async fn get_base_fee(&mut self) -> U256 {
        self.evm.block().basefee
    }

    pub async fn set_base_fee(&mut self, base_fee: U256) {
        self.evm.context.evm.env.block.basefee = base_fee;
    }

    pub fn staticcall(&mut self, tx: Tx) -> Result<TxResult> {
        let result = self._call(tx, false);

        // Generate and log the inspector report after the call
        self.generate_inspector_report();

        result
    }

    pub fn call(&mut self, tx: Tx) -> Result<TxResult> {
        let result = self._call(tx, true);

        // Generate and log the inspector report after the call
        self.generate_inspector_report();

        result
    }

    pub fn _call(&mut self, tx: Tx, commit: bool) -> Result<TxResult> {
        self.evm.context.evm.env.tx.caller = tx.caller;
        self.evm.context.evm.env.tx.transact_to = TransactTo::Call(tx.transact_to);
        self.evm.context.evm.env.tx.data = tx.data; // Remove unnecessary clone
        self.evm.context.evm.env.tx.value = tx.value;
        self.evm.context.evm.env.tx.gas_price = tx.gas_price;
        self.evm.context.evm.env.tx.gas_limit = tx.gas_limit;

        let result = match commit {
            true => match self.evm.transact_commit() {
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            },
            false => {
                let ref_tx = self
                    .evm
                    .transact()
                    .map_err(|e| anyhow!("EVM staticcall failed: {:?}", e))?;
                ref_tx.result
            }
        };
        //info!("Result: {:?}", result);
        let output = match result {
            ExecutionResult::Success {
                gas_used,
                gas_refunded,
                output,
                logs,
                ..
            } => match output {
                Output::Call(o) => TxResult {
                    output: o,
                    logs: Some(logs),
                    gas_used,
                    gas_refunded,
                },
                Output::Create(o, _) => TxResult {
                    output: o,
                    logs: Some(logs),
                    gas_used,
                    gas_refunded,
                },
            },
            ExecutionResult::Revert { gas_used, output } => {
                // Log failure analysis automatically on revert
                let failure_analysis = self.evm.context.external.analyze_failures();
                if !failure_analysis.is_empty() {
                    log::error!(
                        "Transaction reverted - Failure Analysis:\n{}",
                        failure_analysis
                    );
                }

                return Err(anyhow!(
                    "EVM REVERT: {:?} / Gas used: {:?}",
                    output,
                    gas_used
                ));
            }
            ExecutionResult::Halt { reason, .. } => {
                // Log failure analysis automatically on halt
                let failure_analysis = self.evm.context.external.analyze_failures();
                if !failure_analysis.is_empty() {
                    log::error!(
                        "Transaction halted - Failure Analysis:\n{}",
                        failure_analysis
                    );
                }

                return Err(anyhow!("EVM HALT: {:?}", reason));
            }
        };

        Ok(output)
    }

    pub async fn insert_account_info(&mut self, target: Address, account_info: AccountInfo) {
        self.evm
            .context
            .evm
            .db
            .insert_account_info(target, account_info);
    }

    pub async fn insert_contract(&mut self, data: Bytecode) {
        let code_hash = data.hash_slow();
        log::debug!("code hash in insert_contract: {:?}", code_hash);
        let mut account_info = AccountInfo::new(U256::from(0), 0, code_hash, data);
        self.evm.context.evm.db.insert_contract(&mut account_info);
    }

    pub async fn deploy(&mut self, bytecode: Bytecode) {
        let code_hash = bytecode.hash_slow();
        let contract_info = AccountInfo::new(U256::MAX, 0, code_hash, bytecode);
        self.insert_account_info(self.owner, contract_info).await;
    }

    pub async fn deploy_code_at(&mut self, target: Address, bytecode: Bytecode) {
        let code_hash = bytecode.hash_slow();
        let contract_info = AccountInfo::new(U256::MAX, 0, code_hash, bytecode);
        self.insert_account_info(target, contract_info).await;
    }
    pub async fn get_account(&mut self, address: Address) -> Result<AccountInfo, Error> {
        let account = self
            .evm
            .context
            .evm
            .db
            .basic(address)
            .map_err(|e| anyhow::anyhow!("Database error accessing account {}: {}", address, e))?
            .ok_or_else(|| anyhow::anyhow!("Account {} not found", address))?;
        Ok(account)
    }

    pub async fn get_contract(&mut self, _code_hash: B256) -> Result<(), Error> {
        let new_code_hash =
            B256::from_str("0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")?;
        let contracts = self.evm.context.evm.db.code_by_hash(new_code_hash);
        log::debug!("contracts: {:?}", contracts);
        Ok(())
    }

    pub async fn set_eth_balance(&mut self, target: Address, amount: U256) {
        let user_balance = amount;
        let user_info = AccountInfo::new(user_balance, 0, B256::ZERO, Bytecode::default());
        self.insert_account_info(target, user_info).await;
    }

    pub async fn get_eth_balance(&mut self, address: Address) -> U256 {
        self.evm
            .context
            .evm
            .db
            .load_account(address)
            .map(|account| account.info.balance)
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to load account {} for balance check: {}",
                    address,
                    e
                );
                U256::ZERO
            })
    }

    pub async fn load_account(&mut self, address: Address) -> () {
        if let Err(e) = self.evm.context.evm.db.load_account(address) {
            log::warn!("Failed to load account {}: {}", address, e);
        }
    }

    pub async fn get_code_at(&mut self, address: Address) -> Result<AccountInfo, Error> {
        Ok(self
            .evm
            .context
            .evm
            .db
            .load_account(address)
            .map_err(|e| anyhow::anyhow!("Failed to load account {}: {}", address, e))?
            .info
            .clone())
    }

    pub async fn get_erc20_balance(
        &mut self,
        address: Address,
        _token: Address,
        index: U256,
    ) -> U256 {
        self.evm
            .context
            .evm
            .db
            .storage(address, index)
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to get storage for {} at index {}: {}",
                    address,
                    index,
                    e
                );
                U256::ZERO
            })
    }

    pub async fn get_storage(&mut self, address: Address) -> Result<AccountInfo, Error> {
        self.evm
            .context
            .evm
            .db
            .load_account(address)
            .map(|account| account.info.clone())
            .map_err(|e| anyhow::anyhow!("Failed to load account {} for storage: {}", address, e))
    }
    pub async fn insert_account_storage(&mut self, target: Address, index: U256, value: U256) {
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(target, index, value)
            .map_err(|e| log::warn!("Failed to insert account storage for {}: {}", target, e))
            .ok();
    }

    pub async fn get_accounts(&mut self) {
        let accounts = &self.evm.context.evm.db.accounts;
        log::debug!("Accounts: {:?}", accounts);
    }

    pub async fn get_db(&mut self) {
        let db = &self.evm.context.evm.db;
        log::debug!("//////////////////////////////////////////////////////");
        log::debug!("Logs: {:?}", db);
    }

    pub async fn load_pool_state(&mut self, pool_address: Address) -> Result<(), Error> {
        // Get all storage slots from the provider
        // You might want to batch this or load specific slots based on the pool type (V2 or V3)
        let storage_slots = vec![
            U256::from(0), // reserves for V2
            U256::from(1), // fees
            U256::from(2), // token balances
                           // Add more slots based on the pool type
        ];

        for slot in storage_slots {
            let value = self.evm.context.evm.db.storage(pool_address, slot)?;
            self.evm
                .context
                .evm
                .db
                .insert_account_storage(pool_address, slot, value)?;
        }

        Ok(())
    }

    // Helper method to load V2 pool specific storage
    pub async fn load_v2_pool_state(&mut self, pool_address: Address) -> Result<(), Error> {
        // V2 pools store reserves in slot 0
        let reserves_slot = U256::from(0);
        let reserves = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, reserves_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, reserves_slot, reserves)?;

        // Load other V2-specific storage slots
        // token0 balance
        let token0_balance_slot = U256::from(1);
        let token0_balance = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, token0_balance_slot)?;
        self.evm.context.evm.db.insert_account_storage(
            pool_address,
            token0_balance_slot,
            token0_balance,
        )?;

        // token1 balance
        let token1_balance_slot = U256::from(2);
        let token1_balance = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, token1_balance_slot)?;
        self.evm.context.evm.db.insert_account_storage(
            pool_address,
            token1_balance_slot,
            token1_balance,
        )?;

        Ok(())
    }

    // Helper method to load V3 pool specific storage
    pub async fn load_v3_pool_state(&mut self, pool_address: Address) -> Result<(), Error> {
        // Basic pool state
        let liquidity_slot = U256::from(0);
        let liquidity = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, liquidity_slot)?;

        // info!("liquidity {:?}", liquidity);

        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, liquidity_slot, liquidity)?;

        let sqrt_price_slot = U256::from(1);
        let sqrt_price = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, sqrt_price_slot)?;
        self.evm.context.evm.db.insert_account_storage(
            pool_address,
            sqrt_price_slot,
            sqrt_price,
        )?;

        let tick_slot = U256::from(2);
        let tick = self.evm.context.evm.db.storage(pool_address, tick_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, tick_slot, tick)?;

        // Fee and protocol fee settings
        let fee_slot = U256::from(3);
        let fee = self.evm.context.evm.db.storage(pool_address, fee_slot)?;

        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, fee_slot, fee)?;

        let token0_slot = U256::from(4);
        let token0 = self.evm.context.evm.db.storage(pool_address, token0_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, token0_slot, token0)?;

        let token1_slot = U256::from(5);
        let token1 = self.evm.context.evm.db.storage(pool_address, token1_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(pool_address, token1_slot, token1)?;

        // Fee growth trackers
        //        let fee_growth_global0_slot = U256::from(6);
        //        let fee_growth_global0 = evm
        //            .context
        //            .evm
        //            .db
        //            .storage(pool_address, fee_growth_global0_slot)?;
        //        evm.context.evm.db.insert_account_storage(
        //            pool_address,
        //            fee_growth_global0_slot,
        //            fee_growth_global0,
        //        )?;
        //
        //        let fee_growth_global1_slot = U256::from(7);
        //        let fee_growth_global1 = evm
        //            .context
        //            .evm
        //            .db
        //            .storage(pool_address, fee_growth_global1_slot)?;
        //        evm.context.evm.db.insert_account_storage(
        //            pool_address,
        //            fee_growth_global1_slot,
        //            fee_growth_global1,
        //        )?;

        // Protocol fees
        let protocol_fees0_slot = U256::from(8);
        let protocol_fees0 = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, protocol_fees0_slot)?;
        self.evm.context.evm.db.insert_account_storage(
            pool_address,
            protocol_fees0_slot,
            protocol_fees0,
        )?;

        let protocol_fees1_slot = U256::from(9);
        let protocol_fees1 = self
            .evm
            .context
            .evm
            .db
            .storage(pool_address, protocol_fees1_slot)?;
        self.evm.context.evm.db.insert_account_storage(
            pool_address,
            protocol_fees1_slot,
            protocol_fees1,
        )?;

        // Token balances (tracked in ERC20 contracts)
        let token0_addr = Address::from_slice(&token0.to_be_bytes::<32>()[12..]);
        let balance0_slot = get_balance_slot(pool_address);

        let balance0 = self
            .evm
            .context
            .evm
            .db
            .storage(token0_addr, balance0_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(token0_addr, balance0_slot, balance0)?;

        let token1_addr = Address::from_slice(&token1.to_be_bytes::<32>()[12..]);
        let balance1_slot = get_balance_slot(pool_address);

        let balance1 = self
            .evm
            .context
            .evm
            .db
            .storage(token1_addr, balance1_slot)?;
        self.evm
            .context
            .evm
            .db
            .insert_account_storage(token1_addr, balance1_slot, balance1)?;

        Ok(())
    }

    /// Generate and log the inspector report
    pub fn generate_inspector_report(&mut self) {
        let report = self.evm.context.external.generate_report();

        //log::debug!("Inspector Report:\n{}", report);
    }

    /// Get detailed analysis of any failed calls
    pub fn analyze_failures(&mut self) -> String {
        self.evm.context.external.analyze_failures()
    }

    /// Clear the inspector data (useful for multiple test runs)
    pub fn clear_inspector_data(&mut self) {
        self.evm.context.external = revm_inspector::RevmInspector::new();
    }
}
// Helper function to calculate balance slot for an address
fn get_balance_slot(address: Address) -> U256 {
    // This is a simplified version - you might need to adjust based on actual storage layout
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(address.as_slice());
    U256::from_be_bytes(bytes)
}
