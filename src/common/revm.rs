use alloy::contract::{ContractInstance, Interface};
use anyhow::{anyhow, Result};
use alloy_sol_types::SolCall;
use alloy::providers::{RootProvider};
use alloy::pubsub::{PubSubFrontend};
use alloy::primitives::{U64, Address};
use revm::db::{AlloyDB, CacheDB, DbAccount, EmptyDB, EmptyDBTyped};
use alloy::signers::local::PrivateKeySigner;
use revm::handler::register::EvmHandler;
use revm::primitives::SpecId::LATEST;
use revm::primitives::{handler_cfg, Bytes, HandlerCfg, Log, SpecId };
use revm::{
    primitives::{
        keccak256, AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256, U256, specification::{Spec, LatestSpec},
    },
    Evm,
    Database,
    EvmContext,
    InMemoryDB,
    Context,
};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use alloy::network::{AnyNetwork, Ethereum};
use std::str::FromStr;
use alloy::eips::BlockId;

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

// type My_Evm_Context = EvmContext<CacheDB<AlloyDB<Client, AnyNetwork, RootProvider<PubSubFrontend>>>>;

#[derive(Debug )]
pub struct EvmSimulator<'a> {
    pub owner: Address,
    pub evm: Arc<Mutex<Evm<'a, EvmContext<CacheDB<InMemoryDB>>,CacheDB<AlloyDB<PubSubFrontend, AnyNetwork, Arc<RootProvider<PubSubFrontend, AnyNetwork>>>>>>>,  
    pub block_number: U64,
}
impl<'a> EvmSimulator<'a> {
    pub fn new(provider: Arc<RootProvider<PubSubFrontend, AnyNetwork>>, owner: Option<Address>, block_number: U64) -> Self {
        EvmSimulator::new_with_db(owner, block_number, provider)
    }

    pub fn new_with_db(
        owner: Option<Address>,
        block_number: U64,
        provider: Arc<RootProvider<PubSubFrontend, AnyNetwork>>
    ) -> Self {
        let owner = match owner {
            Some(owner) => owner,
            None => PrivateKeySigner::random().address(),
        };

        let alloy_db =  AlloyDB::new(provider, BlockId::from(block_number)).unwrap();

        // let evm_external = EvmContext::new(alloy_db);
        // isshe here is that this should be an Queryable DB but it's not? Maybe InMemoryDB isn't what I am looking for
        let empty_db = CacheDB::new(InMemoryDB::default());    
        let evm_external = EvmContext::new(empty_db);

        // let evm_internal= EvmContext::new(empty_db);
        let evm_internal = EvmContext::new(CacheDB::new(alloy_db));

        let context = Context::new(evm_internal, evm_external);

        let handler_cfg = HandlerCfg {
           spec_id: SpecId::LATEST,
        };
        let handler = EvmHandler::new(handler_cfg);

        let evm= Evm::new(context, handler);   

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

        let result: revm::primitives::ExecutionResult;

        if commit {
            result = match evm.transact_commit(){
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            };
        } else {
            let ref_tx = evm
                .transact() 
                .map_err(|e| anyhow!("EVM staticcall failed: {:?}", e))?;
            result = ref_tx.result;
        }

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

    pub fn load_account(&mut self, address: Address) ->  (){
        if let Ok(mut evm) = self.evm.clone().lock() {
         evm.context.evm.db.load_account(address).unwrap();
        }
        else {
        ()
        }
    }

    // pub fn get_code_at(&mut self, address: Address) -> AccountInfo{
    //     if let Ok(mut evm) = self.evm.clone().lock() {
    // let accountInfo = evm.context.evm.db.load_account(address).unwrap().info.clone();
    //     } else {
    //         AccountInfo::default()    
    //     }
    // }

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
    // NOTE: probably want to change this and not have to get the abi from that folder
    pub fn get_weth_balance(&mut self, address: Address, token: Address, provider:Arc<RootProvider<PubSubFrontend, AnyNetwork>>, latest_gas_limit: &u64, latest_gas_price: &U256)  {

        alloy::sol!{
            function balanceOf(address account) external view returns (uint256);
        }

        let abi = serde_json::from_str(include_str!("../arbitrage/weth.json")).unwrap();

        let contract = ContractInstance::<Address, Arc<RootProvider<PubSubFrontend, AnyNetwork>>, Interface>::new(
            self.owner,
            provider,
            Interface::new(abi),
        );

        // create a transaction, call the balanceOf function of the token contract
        let data = balanceOfCall {
            account: address,
        };

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

        let res = contract.decode_output("balanceOf", &result.output, false).unwrap();

        let balance = res[0].clone();

        println!("balance: {:?}", balance);
        // decode result 

    }

    pub fn get_accounts(&mut self) {
           if let Ok(evm)= self.evm.clone().lock() {
               let accounts = &evm.context.evm.db.accounts;
               println!("Accounts: {:?}", accounts);
           }; 
           ()
    }

    pub fn get_db(&mut self ) {
        if let Ok(evm) = self.evm.clone().lock() {
            let db = &evm.context.evm.db;
            println!("//////////////////////////////////////////////////////");
            println!("Logs: {:?}", db);
        }
        ()
    }



}
