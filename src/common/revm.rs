use crate::arbitrage::simulation::{arboo_bytecode, get_address, AddressType};

use super::revmInspector::{self, RevmInspector};
use alloy::contract::{ContractInstance, Interface};
use alloy::eips::BlockId;
use alloy::network::{AnyNetwork, Ethereum};
use alloy::primitives::{Address, U64};
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::signers::local::PrivateKeySigner;
use alloy_sol_types::SolCall;
use anyhow::{anyhow, Error, Result};
use log::info;
use revm::db::{AlloyDB, CacheDB};
use revm::inspector_handle_register;
use revm::primitives::{Bytes, Log};
use revm::{
    primitives::{AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256, U256},
    Database, Evm,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

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
    pub evm: TokioMutex<
        Evm<
            'a,
            RevmInspector,
            CacheDB<AlloyDB<PubSubFrontend, Ethereum, Arc<RootProvider<PubSubFrontend, Ethereum>>>>,
        >,
    >,
    pub block_number: U64,
}
impl<'a> EvmSimulator<'a> {
    pub fn new(
        provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
        owner: Option<Address>,
        block_number: U64,
    ) -> Self {
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

        self.call(new_tx).unwrap();
    }

    pub fn new_with_db(
        owner: Option<Address>,
        block_number: U64,
        provider: Arc<RootProvider<PubSubFrontend, Ethereum>>,
    ) -> Self {
        let owner = match owner {
            Some(owner) => owner,
            None => PrivateKeySigner::random().address(),
        };
        let contract_wallet = PrivateKeySigner::random();
        let inspector = revmInspector::RevmInspector::new();

        let alloy_db = CacheDB::new(AlloyDB::new(provider, BlockId::from(block_number)).unwrap());

        let evm = Evm::builder()
            .with_db(alloy_db)
            .with_external_context(inspector)
            .append_handler_register(inspector_handle_register)
            .modify_env(|env| {
                env.block.number = U256::from(block_number);
                env.block.coinbase =
                    Address::from_str("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5").unwrap();
            })
            .build();

        let evm = TokioMutex::new(evm);

        Self {
            owner,
            evm,
            block_number,
            contract_address: contract_wallet.address(),
        }
    }

    pub fn set_arc_mutex(&mut self) -> Arc<TokioMutex<&mut EvmSimulator<'a>>> {
        Arc::new(TokioMutex::new(self))
    }

    pub async fn get_block_number(&mut self) -> U256 {
        let evm = self.evm.lock().await;
        evm.block().number
    }

    pub async fn get_coinbase(&mut self) -> Address {
        let evm = self.evm.lock().await;
        evm.block().coinbase
    }

    pub async fn get_base_fee(&mut self) -> U256 {
        let evm = self.evm.lock().await;
        evm.block().basefee
    }

    pub async fn set_base_fee(&mut self, base_fee: U256) {
        let mut evm = self.evm.lock().await;
        evm.context.evm.env.block.basefee = base_fee;
    }

    pub fn staticcall(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, false)
    }

    pub fn call(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, true)
    }

    pub fn _call(&mut self, tx: Tx, commit: bool) -> Result<TxResult> {
        if let Ok(mut evm) = self.evm.try_lock() {
            evm.context.evm.env.tx.caller = tx.caller;
            evm.context.evm.env.tx.transact_to = TransactTo::Call(tx.transact_to);
            evm.context.evm.env.tx.data = tx.data.clone();
            evm.context.evm.env.tx.value = tx.value;
            evm.context.evm.env.tx.gas_price = tx.gas_price;
            evm.context.evm.env.tx.gas_limit = tx.gas_limit;

            let result = match commit {
                true => match evm.transact_commit() {
                    Ok(result) => result,
                    Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
                },
                false => {
                    let ref_tx = evm
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
                    return Err(anyhow!(
                        "EVM REVERT: {:?} / Gas used: {:?}",
                        output,
                        gas_used
                    ))
                }
                ExecutionResult::Halt { reason, .. } => {
                    return Err(anyhow!("EVM HALT: {:?}", reason))
                }
            };

            Ok(output)
        } else {
            Err(anyhow!("EVM lock failed"))
        }
    }

    pub async fn insert_account_info(&mut self, target: Address, account_info: AccountInfo) {
        let mut evm = self.evm.lock().await;
        evm.context.evm.db.insert_account_info(target, account_info);
    }

    pub async fn insert_contract(&mut self, data: Bytecode) {
        let mut evm = self.evm.lock().await;
        let code_hash = data.hash_slow();
        info!("code hash in insert_contract: {:?}", code_hash);
        let mut account_info = AccountInfo::new(U256::from(0), 0, code_hash, data);
        evm.context.evm.db.insert_contract(&mut account_info);
    }

    pub async fn deploy(&mut self, bytecode: Bytecode) {
        let code_hash = bytecode.clone().hash_slow();
        let contract_info = AccountInfo::new(U256::MAX, 0, code_hash, bytecode.clone());
        self.insert_account_info(self.owner, contract_info).await;
    }

    pub async fn deploy_code_at(&mut self, target: Address, bytecode: Bytecode) {
        let code_hash = bytecode.clone().hash_slow();
        let contract_info = AccountInfo::new(U256::MAX, 0, code_hash, bytecode.clone());
        self.insert_account_info(target, contract_info).await;
    }
    pub async fn get_account(&mut self, address: Address) -> Result<AccountInfo, Error> {
        let mut evm = self.evm.lock().await;
        let account = evm.context.evm.db.basic(address).unwrap().unwrap();
        Ok(account)
    }

    pub async fn get_contract(&mut self, code_hash: B256) -> Result<(), Error> {
        let mut evm = self.evm.lock().await;
        let new_code_hash =
            B256::from_str("0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")?;
        let contracts = evm.context.evm.db.code_by_hash(new_code_hash);
        info!("contracts: {:?}", contracts);
        Ok(())
    }

    pub async fn set_eth_balance(&mut self, target: Address, amount: U256) {
        let user_balance = amount;
        let user_info = AccountInfo::new(user_balance, 0, B256::ZERO, Bytecode::default());
        self.insert_account_info(target, user_info).await;
    }

    pub async fn get_eth_balance(&mut self, address: Address) -> U256 {
        let mut evm = self.evm.lock().await;
        evm.context
            .evm
            .db
            .load_account(address)
            .unwrap()
            .info
            .balance
    }

    pub async fn load_account(&mut self, address: Address) -> () {
        let mut evm = self.evm.lock().await;
        evm.context.evm.db.load_account(address).unwrap();
    }

    pub async fn get_code_at(&mut self, address: Address) -> Result<AccountInfo, Error> {
        let mut evm = self.evm.lock().await;
        Ok(evm
            .context
            .evm
            .db
            .load_account(address)
            .unwrap()
            .info
            .clone())
    }

    pub async fn get_erc20_balance(
        &mut self,
        address: Address,
        token: Address,
        index: U256,
    ) -> U256 {
        let mut evm = self.evm.lock().await;
        evm.context.evm.db.storage(address, index).unwrap()
    }

    pub async fn get_storage(&mut self, address: Address) -> AccountInfo {
        let mut evm = self.evm.lock().await;
        evm.context
            .evm
            .db
            .load_account(address)
            .unwrap()
            .info
            .clone()
    }

    pub async fn insert_account_storage(&mut self, target: Address, index: U256, value: U256) {
        let mut evm = self.evm.lock().await;
        evm.context
            .evm
            .db
            .insert_account_storage(target, index, value)
            .unwrap();
    }
    // NOTE: probably want to change this and not have to get the abi from that folder
    pub fn get_weth_balance(
        &mut self,
        address: Address,
        token: Address,
        provider: Arc<RootProvider<PubSubFrontend, AnyNetwork>>,
        latest_gas_limit: &u64,
        latest_gas_price: &U256,
    ) {
        alloy::sol! {
            function balanceOf(address account) external view returns (uint256);
        }

        let abi = serde_json::from_str(include_str!("../arbitrage/weth.json")).unwrap();

        let contract = ContractInstance::<
            Address,
            Arc<RootProvider<PubSubFrontend, AnyNetwork>>,
            Interface,
        >::new(self.owner, provider, Interface::new(abi));

        // create a transaction, call the balanceOf function of the token contract
        let data = balanceOfCall { account: address };

        let data = data.abi_encode();

        let tx = Tx {
            caller: self.owner,
            transact_to: token,
            data: data.into(),
            value: U256::ZERO,
            gas_price: *latest_gas_price,
            gas_limit: *latest_gas_limit,
        };

        let result = self.call(tx).unwrap();

        print!("result from balance of call: {:?}", result);

        let res = contract
            .decode_output("balanceOf", &result.output, false)
            .unwrap();

        let balance = res[0].clone();

        info!("balance: {:?}", balance);
        // decode result
    }

    pub async fn get_accounts(&mut self) {
        let evm = self.evm.lock().await;
        let accounts = &evm.context.evm.db.accounts;
        info!("Accounts: {:?}", accounts);
    }

    pub async fn get_db(&mut self) {
        let evm = self.evm.lock().await;
        let db = &evm.context.evm.db;
        info!("//////////////////////////////////////////////////////");
        info!("Logs: {:?}", db);
    }

    pub async fn load_pool_state(&self, pool_address: Address) -> Result<(), Error> {
        let mut evm = self.evm.lock().await;

        // Get all storage slots from the provider
        // You might want to batch this or load specific slots based on the pool type (V2 or V3)
        let storage_slots = vec![
            U256::from(0), // reserves for V2
            U256::from(1), // fees
            U256::from(2), // token balances
                           // Add more slots based on the pool type
        ];

        for slot in storage_slots {
            let value = evm.context.evm.db.storage(pool_address, slot)?;
            evm.context
                .evm
                .db
                .insert_account_storage(pool_address, slot, value)?;
        }

        Ok(())
    }

    // Helper method to load V2 pool specific storage
    pub async fn load_v2_pool_state(&self, pool_address: Address) -> Result<(), Error> {
        let mut evm = self.evm.lock().await;

        // V2 pools store reserves in slot 0
        let reserves_slot = U256::from(0);
        let reserves = evm.context.evm.db.storage(pool_address, reserves_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, reserves_slot, reserves)?;

        // Load other V2-specific storage slots
        // token0 balance
        let token0_balance_slot = U256::from(1);
        let token0_balance = evm
            .context
            .evm
            .db
            .storage(pool_address, token0_balance_slot)?;
        evm.context.evm.db.insert_account_storage(
            pool_address,
            token0_balance_slot,
            token0_balance,
        )?;

        // token1 balance
        let token1_balance_slot = U256::from(2);
        let token1_balance = evm
            .context
            .evm
            .db
            .storage(pool_address, token1_balance_slot)?;
        evm.context.evm.db.insert_account_storage(
            pool_address,
            token1_balance_slot,
            token1_balance,
        )?;

        Ok(())
    }

    // Helper method to load V3 pool specific storage
    pub async fn load_v3_pool_state(&self, pool_address: Address) -> Result<(), Error> {
        let mut evm = self.evm.lock().await;

        // Basic pool state
        let liquidity_slot = U256::from(0);
        let liquidity = evm.context.evm.db.storage(pool_address, liquidity_slot)?;

        // info!("liquidity {:?}", liquidity);

        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, liquidity_slot, liquidity)?;

        let sqrt_price_slot = U256::from(1);
        let sqrt_price = evm.context.evm.db.storage(pool_address, sqrt_price_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, sqrt_price_slot, sqrt_price)?;

        let tick_slot = U256::from(2);
        let tick = evm.context.evm.db.storage(pool_address, tick_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, tick_slot, tick)?;

        // Fee and protocol fee settings
        let fee_slot = U256::from(3);
        let fee = evm.context.evm.db.storage(pool_address, fee_slot)?;

        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, fee_slot, fee)?;

        let token0_slot = U256::from(4);
        let token0 = evm.context.evm.db.storage(pool_address, token0_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(pool_address, token0_slot, token0)?;

        let token1_slot = U256::from(5);
        let token1 = evm.context.evm.db.storage(pool_address, token1_slot)?;
        evm.context
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
        let protocol_fees0 = evm
            .context
            .evm
            .db
            .storage(pool_address, protocol_fees0_slot)?;
        evm.context.evm.db.insert_account_storage(
            pool_address,
            protocol_fees0_slot,
            protocol_fees0,
        )?;

        let protocol_fees1_slot = U256::from(9);
        let protocol_fees1 = evm
            .context
            .evm
            .db
            .storage(pool_address, protocol_fees1_slot)?;
        evm.context.evm.db.insert_account_storage(
            pool_address,
            protocol_fees1_slot,
            protocol_fees1,
        )?;

        // Token balances (tracked in ERC20 contracts)
        let token0_addr = Address::from_slice(&token0.to_be_bytes::<32>()[12..]);
        let balance0_slot = get_balance_slot(pool_address);

        let balance0 = evm.context.evm.db.storage(token0_addr, balance0_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(token0_addr, balance0_slot, balance0)?;

        let token1_addr = Address::from_slice(&token1.to_be_bytes::<32>()[12..]);
        let balance1_slot = get_balance_slot(pool_address);

        let balance1 = evm.context.evm.db.storage(token1_addr, balance1_slot)?;
        evm.context
            .evm
            .db
            .insert_account_storage(token1_addr, balance1_slot, balance1)?;

        Ok(())
    }
}
// Helper function to calculate balance slot for an address
fn get_balance_slot(address: Address) -> U256 {
    // This is a simplified version - you might need to adjust based on actual storage layout
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(address.as_slice());
    U256::from_be_bytes(bytes)
}
