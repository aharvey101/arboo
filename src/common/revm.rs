use anyhow::{anyhow, Result};
use revm::db::EmptyDBTyped;
use revm::primitives::{Address, Bytes, Log};

use revm::{
    db::CacheDB,
    primitives::{
        keccak256, AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256, U256,
    },
    EVM,
};
use std::convert::Infallible;

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

#[derive(Clone)]
pub struct EvmSimulator {
    pub owner: Address,
    pub evm: EVM<CacheDB<EmptyDBTyped<Infallible>>>,
    pub block_number: u64,
}

impl EvmSimulator {
    pub fn new_with_db(
        owner: Address,
        block_number: u64,
        db: CacheDB<EmptyDBTyped<Infallible>>,
    ) -> Self {
        let mut evm = EVM::new();
        evm.database(db);

        evm.env.block.number = U256::from(block_number + 1);
        evm.env.block.coinbase = "0xDAFEA492D9c6733ae3d56b7Ed1ADB60692c98Bc5"
            .parse()
            .unwrap();

        Self {
            owner,
            evm,
            block_number,
        }
    }

    pub fn insert_db(&mut self, db: CacheDB<EmptyDBTyped<Infallible>>) {
        let mut evm = EVM::new();
        evm.database(db);

        self.evm = evm;
    }

    pub fn get_block_number(&mut self) -> U256 {
        self.evm.env.block.number.into()
    }

    pub fn get_coinbase(&mut self) -> Address {
        self.evm.env.block.coinbase.into()
    }

    pub fn get_base_fee(&mut self) -> U256 {
        self.evm.env.block.basefee.into()
    }

    pub fn set_base_fee(&mut self, base_fee: U256) {
        self.evm.env.block.basefee = base_fee.into();
    }

    pub fn staticcall(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, false)
    }

    pub fn call(&mut self, tx: Tx) -> Result<TxResult> {
        self._call(tx, true)
    }

    pub fn _call(&mut self, tx: Tx, commit: bool) -> Result<TxResult> {
        self.evm.env.tx.caller = tx.caller.into();
        self.evm.env.tx.transact_to = TransactTo::Call(tx.transact_to.into());
        self.evm.env.tx.data = tx.data;
        self.evm.env.tx.value = tx.value.into();
        self.evm.env.tx.gas_price = tx.gas_price.into();
        self.evm.env.tx.gas_limit = tx.gas_limit;

        let result;

        if commit {
            result = match self.evm.transact_commit() {
                Ok(result) => result,
                Err(e) => return Err(anyhow!("EVM call failed: {:?}", e)),
            };
        } else {
            let ref_tx = self
                .evm
                .transact_ref()
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
        println!("Output: {:?}", output);
        Ok(output)
    }

    pub fn insert_account_info(&mut self, target: Address, account_info: AccountInfo) {
        self.evm
            .db
            .as_mut()
            .unwrap()
            .insert_account_info(target, account_info);
    }

    pub fn deploy(&mut self, target: Address, bytecode: Bytecode) {
        let contract_info = AccountInfo::new(U256::ZERO, 0, B256::ZERO, bytecode);
        self.insert_account_info(target, contract_info);
    }
    pub fn set_eth_balance(&mut self, target: Address, amount: U256) {
        let user_balance = amount.into();
        let user_info = AccountInfo::new(user_balance, 0, B256::ZERO, Bytecode::default());
        self.insert_account_info(target.into(), user_info);
    }
}
