use anyhow::{anyhow, Result};
use alloy::providers::{Provider, ReqwestProvider, RootProvider, ProviderLayer};
use alloy::pubsub::{PubSubConnect, PubSubFrontend};
use foundry_evm::backend::{Backend, BlockchainDb, BlockchainDbMeta, SharedBackend};
use alloy::primitives::{U64, Address};
use revm::db::{EmptyDB, EmptyDBTyped, CacheDB , AlloyDB};
use alloy::signers::local::PrivateKeySigner;
use revm::inspectors::CustomPrintTracer;
use revm::primitives::{Bytes,  Log, };
use revm::{
    primitives::{
        keccak256, AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256, U256, specification::{Spec, LatestSpec},
    },
    Evm,
    Database,
    EvmContext,
    Handler,
    InMemoryDB,
    Context,

    GetInspector,
    inspector_handle_register,
};
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use alloy::network::{AnyNetwork};
use std::str::FromStr;
use alloy::transports::ws::WsConnect;


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
        let gas_limit = match tx.gas_limit {
            Some(gas_limit) => gas_limit,
            None => 5000000,
        };
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


#[derive(Debug )]
pub struct EvmSimulator<'a> {
    pub owner: Address,
    pub evm: Arc<Mutex<Evm<'a, EvmContext<CacheDB<SharedBackend>>, CacheDB<CacheDB<EmptyDBTyped<Infallible>>>>>>,
    pub block_number: U64,
}
impl<'a> EvmSimulator<'a> {
    pub fn new(provider: Arc<RootProvider<PubSubFrontend, AnyNetwork>>, owner: Option<Address>, block_number: U64) -> Self {

        let shared_backend = SharedBackend::spawn_backend_thread(
            provider.clone(),
            BlockchainDb::new(
                BlockchainDbMeta {
         cfg_env: Default::default(),
                    block_env: Default::default(),
                    hosts: BTreeSet::from(["".to_string()]),
                },
                None,
            ),
            Some(block_number.into()),
        );
        let db = CacheDB::new(shared_backend);
        EvmSimulator::new_with_db(owner, block_number, db)
    }

    pub fn new_with_db(
        owner: Option<Address>,
        block_number: U64,
        my_db: CacheDB<SharedBackend>,
    ) -> Self {
        let owner = match owner {
            Some(owner) => owner,
            None => PrivateKeySigner::random().address(),
        };

        let evm_external = EvmContext::new(my_db.clone());
        // let alloy_db = AlloyDB::new(provider.clone(), block_number.into()).unwrap();
        // let cached_db = CacheDB::new(alloy_db);
        let empty_db = CacheDB::new(InMemoryDB::default());    
        let evm_internal= EvmContext::new(empty_db.clone());

        let context = Context::new(evm_internal, evm_external);

        let handler = Handler::mainnet::<LatestSpec>();

        let evm = Evm::new(context, handler);   

        let evm = evm 
        .modify() 
        .modify_env(|env| {
            env.block.number = U256::from(block_number);
            env.block.coinbase = Address::from_str("0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5").unwrap();
        })
        .build();

        let evm = Arc::new(Mutex::new(evm));

        Self {
            owner,
            evm,
            block_number,
        }
    }

    pub fn set_arc_mutex(&mut self) -> Arc<Mutex<&mut EvmSimulator<'a>>> {
        Arc::new(Mutex::new(self))
    }

    pub fn get_block_number(&mut self) -> U256 {
        if let Ok(evm) = self.evm.lock() {
            evm.block().number
        } else {
            U256::ZERO
        }
    }

    pub fn get_coinbase(&mut self) -> Address {
        if let Ok(evm) = self.evm.lock() {
            evm.block().coinbase
        } else {
            Address::default()
        }
    }

    pub fn get_base_fee(&mut self) -> U256 {
        if let Ok(evm) = self.evm.lock() {
            evm.block().basefee
        } else {
            U256::ZERO
        }
    }

    pub fn set_base_fee(&mut self, base_fee: U256) {
        if let Ok(mut evm) = self.evm.clone().try_lock() {
                evm.context.evm.env.block.basefee = base_fee;
            ;
        } else {
            println!("Failed to set base fee");
        }
    }

    pub fn staticcall(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, false)
    }

    pub fn call(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, true)
    }

    pub fn _call(&mut self, tx: Tx, commit: bool) -> Result<TxResult> {

        if let Ok(mut evm) = self.evm.clone().try_lock() {
            evm.context.evm.env.tx.caller = tx.caller;
            evm.context.evm.env.tx.transact_to = TransactTo::Call(tx.transact_to);
            evm.context.evm.env.tx.data = tx.data.clone();
            evm.context.evm.env.tx.value = tx.value;
            evm.context.evm.env.tx.gas_price = tx.gas_price;
            evm.context.evm.env.tx.gas_limit = tx.gas_limit;

        let result;

        if commit {
            result = match evm.transact_commit() {
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            };
        } else {
            let ref_tx = evm
                .transact() 
                .map_err(|e| anyhow!("EVM staticcall failed: {:?}", e))?;
            result = ref_tx.result;
        }

        println!("result: {:?}", result);
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
            ExecutionResult::Halt { reason, .. } => return Err(anyhow!("EVM HALT: {:?}", reason)),
        };
        Ok(output)
        } else {
            Err(anyhow!("EVM lock failed"))
        }
    }

    pub fn insert_account_info( &mut self, target: Address, account_info: AccountInfo){
        if let Ok(mut evm) = self.evm.clone().lock() {
            evm.context.evm.db.insert_account_info(target, account_info);        
        } else {
            println!("Failed to insert account info");
        }
    }

    pub fn deploy(&mut self, target: Address, bytecode: Bytecode) {
        let contract_info = AccountInfo::new(U256::ZERO, 0, B256::ZERO, bytecode);
        self.insert_account_info(target, contract_info);
    }
    pub fn set_eth_balance(&mut self, target: Address, amount: U256){
        let user_balance = amount.into();
        let user_info = AccountInfo::new(user_balance, 0, B256::ZERO, Bytecode::default());
        self.insert_account_info(target.into(), user_info);
    }

    pub fn get_eth_balance(&mut self, address: Address) -> U256 {
        if let Ok(mut evm) = self.evm.clone().lock() {
            evm.context.evm.db.load_account(address).unwrap().info.balance
        } else {
            U256::ZERO
        }
    }

    pub fn get_erc20_balance(&mut self, address: Address, token: Address, index: U256) -> U256 {
        if let Ok(mut evm) = self.evm.clone().lock() {
            evm.context.evm.db.storage(address, index).unwrap()
        } else {
            U256::ZERO
        }
    }

    pub fn get_storage(&mut self, address: Address) -> AccountInfo {
        if let Ok(mut evm) = self.evm.clone().lock() {
            evm.context.evm.db.load_account(address).unwrap().info.clone()
        } else {
            AccountInfo::default()
        }
    }

    pub fn insert_account_storage(&mut self, target: Address, index: U256, value: U256) {
        if let Ok(mut evm) = self.evm.clone().lock() {
            evm.context.evm.db.insert_account_storage(target, index, value).unwrap();
        } else {
            println!("Failed to insert account storage");
        }
    }

}
