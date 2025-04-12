use alloy_primitives::Address;
use log::info;
use revm::interpreter::{
    CallInputs, CallOutcome, CreateInputs, CreateOutcome, EOFCreateInputs, InstructionResult,
    Interpreter, SuccessOrHalt,
};
use revm::primitives::{Bytes, Log, B256, U256};
use revm::Database;
use revm::EvmContext;
use std::collections::HashMap;
use std::ops::Add;

use crate::arbitrage::simulation::{get_address, AddressType};

/// A comprehensive REVM Inspector that tracks:
/// - Gas usage
/// - Storage access
/// - Contract calls/creates
/// - Log events
/// - Errors
#[derive(Debug)]
pub struct RevmInspector {
    /// Track the call stack
    pub calls: Vec<CallInfo>,
    /// Track all storage slot accesses
    pub storage_accesses: HashMap<B256, Vec<StorageAccess>>,
    /// Track gas usage by opcode
    pub gas_by_opcode: HashMap<u8, u64>,
    /// Track emitted logs
    pub logs: Vec<LogInfo>,
    /// Track any errors that occurred
    pub errors: Vec<ErrorInfo>,
    /// Track balance changes
    pub balance_changes: HashMap<B256, i128>,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    //pub depth: usize,
    pub caller: Address,
    pub address: Option<Address>,
    //pub kind: CallKind,
    pub value: U256,
    pub input: Option<Bytes>,
    pub gas_limit: u64,
    pub gas_used: Option<u64>,
    pub output: Option<Bytes>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CallKind {
    Call,
    StaticCall,
    CallCode,
    DelegateCall,
    Create,
    Create2,
}

#[derive(Debug, Clone)]
pub struct StorageAccess {
    pub address: B256,
    pub slot: B256,
    pub value: B256,
    pub is_write: bool,
}

#[derive(Debug, Clone)]
pub struct LogInfo {
    pub address: Address,
    //pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub phase: String,
    pub message: String,
}

impl RevmInspector {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            storage_accesses: HashMap::new(),
            gas_by_opcode: HashMap::new(),
            logs: Vec::new(),
            errors: Vec::new(),
            balance_changes: HashMap::new(),
        }
    }

    /// Generate a human-readable report of what was captured
    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        // Summarize calls
        report.push_str("\n=== Calls Summary ===\n");
        for (i, call) in self.calls.iter().enumerate() {
            report.push_str(&format!(
                "Call #{}: {} -> {} (), value: {}, gas: {}/{}, success: {}\n",
                i,
                hex::encode(call.caller.to_string()),
                hex::encode(call.address.unwrap()),
                //format!("{:?}", call.kind),
                call.value,
                call.gas_used.unwrap_or(0),
                call.gas_limit,
                call.error.is_none()
            ));
        }

        // Summarize storage accesses
        report.push_str("\n=== Storage Accesses ===\n");
        for (address, accesses) in &self.storage_accesses {
            report.push_str(&format!(
                "Contract: 0x{}...\n",
                hex::encode(address.to_string())
            ));
            for access in accesses {
                report.push_str(&format!(
                    "  {} slot: 0x{}... value: 0x{}...\n",
                    if access.is_write { "WRITE" } else { "READ " },
                    hex::encode(access.slot),
                    hex::encode(access.value)
                ));
            }
        }

        // Summarize gas usage
        report.push_str("\n=== Gas Usage By Opcode ===\n");
        let mut opcodes: Vec<(u8, u64)> =
            self.gas_by_opcode.iter().map(|(&k, &v)| (k, v)).collect();
        opcodes.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by gas used, descending

        for (opcode, gas) in opcodes.iter().take(10) {
            // Show top 10
            report.push_str(&format!("0x{:02x}: {} gas\n", opcode, gas));
        }

        // Summarize logs
        report.push_str("\n=== Logs ===\n");
        for (i, log) in self.logs.iter().enumerate() {
            report.push_str(&format!(
                "Log #{}: Contract: 0x{}..., , Data size: {} bytes\n",
                i,
                hex::encode(log.address),
                //log.topics.len(),
                log.data.len()
            ));
        }

        // Summarize errors
        if !self.errors.is_empty() {
            report.push_str("\n=== Errors ===\n");
            for error in &self.errors {
                report.push_str(&format!("[{}] {}\n", error.phase, error.message));
            }
        }

        // Summarize balance changes
        report.push_str("\n=== Balance Changes ===\n");
        for (address, change) in &self.balance_changes {
            let change_str = if *change >= 0 {
                format!("+{}", change)
            } else {
                format!("{}", change)
            };
            report.push_str(&format!(
                "0x{}...: {} wei\n",
                hex::encode(address),
                change_str
            ));
        }
        info!("REPORT: {:?}", report);
        report
    }
}

impl<DB: Database> revm::Inspector<DB> for RevmInspector {
    fn step(&mut self, interp: &mut Interpreter, _context: &mut EvmContext<DB>) {
        // Record the current opcode
    }

    //

    fn log(&mut self, _interp: &mut Interpreter, _context: &mut EvmContext<DB>, log: &Log) {
        // Capture the log
        self.logs.push(LogInfo {
            address: log.address,
            //topics: log.topics.clone(),
            data: Bytes::copy_from_slice(&log.data.data),
        });
    }

    fn call(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        // Record the call
        let call_info = CallInfo {
            address: Some(inputs.target_address),
            caller: inputs.caller,
            value: inputs.value.get(),
            input: Some(inputs.input.clone()),
            gas_limit: inputs.gas_limit,
            gas_used: None, // Will be populated in call_end
            output: None,   // Will be populated in call_end
            error: None,    // Will be populated in call_end
        };

        self.calls.push(call_info.clone());

        None // Allow call to proceed normally
    }

    fn call_end(
        &mut self,
        _context: &mut EvmContext<DB>,
        _inputs: &CallInputs,
        outcome: CallOutcome,
    ) -> CallOutcome {
        if let Some(last_call) = self.calls.last_mut() {
            // Extract information from the outcome
            last_call.gas_used = Some(outcome.gas().spent());
            last_call.output = Some(outcome.output().clone());

            // Set error information based on the instruction result
            match outcome.instruction_result() {
                InstructionResult::Return => {
                    // Success case, no error
                }
                InstructionResult::Revert => {
                    last_call.error = Some("Reverted".to_string());

                    // Try to decode the revert reason
                    let output = outcome.output();
                    if output.len() >= 4 + 32 + 32 {
                        // Solidity revert with string error message format (keccak256("Error(string)"))
                        if output[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
                            // Skip the function selector and offset
                            let str_len = u32::from_be_bytes([
                                output[36], output[37], output[38], output[39],
                            ]) as usize;
                            if output.len() >= 4 + 32 + 32 + str_len {
                                let error_msg = String::from_utf8_lossy(
                                    &output[4 + 32 + 32..4 + 32 + 32 + str_len],
                                );
                                self.errors.push(ErrorInfo {
                                    phase: "call".to_string(),
                                    message: error_msg.to_string(),
                                });
                            }
                        }
                    }
                }
                error => {
                    last_call.error = Some(format!("Error: {:?}", error));

                    // Add to errors collection
                    self.errors.push(ErrorInfo {
                        phase: "call".to_string(),
                        message: format!("EVM error: {:?}", error),
                    });
                }
            }
        }

        //  so we gotta figure out, where it fails,
        //  probably in the second swap?
        //  so lets log, if callInputs address is uniswapV2Router?
        //
        // if _inputs.bytecode_address == get_address(AddressType::V2Router) {
        //     info!("outcome: {:?}", self.errors);
        // }
        //
        // The idea here is we want to understand what is happening when we call these addresses
        if _inputs.bytecode_address == get_address(AddressType::V3Router) {
            info!("V3 Router call end outcome")
        }
        if _inputs.bytecode_address == get_address(AddressType::V2Router) {
            info!("V3 Router call end outcome")
        }
        // if _inputs.bytecode_address == get_address(AddressType::V3Router) {
        //     info!("V3 Router call end outcome")
        // }

        //info!("outcome: {:?}", outcome);
        outcome
    }

    fn create(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        // Record the create
        let call_info = CallInfo {
            address: None,
            caller: inputs.caller,
            value: inputs.value,
            input: None,
            gas_limit: inputs.gas_limit,
            gas_used: None, // Will be populated in call_end
            output: None,   // Will be populated in call_end
            error: None,    // Will be populated in call_end
        };

        self.calls.push(call_info);

        None // Allow create to proceed normally
    }
    fn create_end(
        &mut self,
        _context: &mut EvmContext<DB>,
        _inputs: &CreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        // For CreateOutcome, we need to handle it similarly to CallOutcome
        // Based on the InterpreterResult inside CreateOutcome
        if let Some(last_call) = self.calls.last_mut() {
            last_call.gas_used = Some(outcome.result.gas.spent());

            // Determine the outcome type
            match &outcome.result.result {
                InstructionResult::Return => {
                    last_call.output = Some(outcome.result.output.clone());
                }
                InstructionResult::Revert => {
                    last_call.output = Some(outcome.result.output.clone());
                    last_call.error = Some("Reverted".to_string());

                    // Try to decode the revert reason
                    let output = &outcome.result.output;
                    if output.len() >= 4 + 32 + 32 && output[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
                        let str_len =
                            u32::from_be_bytes([output[36], output[37], output[38], output[39]])
                                as usize;
                        if output.len() >= 4 + 32 + 32 + str_len {
                            let error_msg = String::from_utf8_lossy(
                                &output[4 + 32 + 32..4 + 32 + 32 + str_len],
                            );
                            self.errors.push(ErrorInfo {
                                phase: "create".to_string(),
                                message: error_msg.to_string(),
                            });
                        }
                    }
                }
                error => {
                    last_call.error = Some(format!("Error: {:?}", error));

                    self.errors.push(ErrorInfo {
                        phase: "create".to_string(),
                        message: format!("EVM error: {:?}", error),
                    });
                }
            }
        }

        log::debug!("outcome of end: {:?}", outcome);
        outcome
    }

    fn eofcreate(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut EOFCreateInputs,
    ) -> Option<CreateOutcome> {
        // Record the EOF create (similar to create)
        let call_info = CallInfo {
            caller: inputs.caller,
            address: None,
            value: inputs.value,
            input: None,
            gas_limit: inputs.gas_limit,
            gas_used: None,
            output: None,
            error: None,
        };

        self.calls.push(call_info);

        None
    }

    fn eofcreate_end(
        &mut self,
        _context: &mut EvmContext<DB>,
        _inputs: &EOFCreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        // Handle EOF create outcome (same structure as create_end)
        if let Some(last_call) = self.calls.last_mut() {
            last_call.gas_used = Some(outcome.result.gas.spent());

            match &outcome.result.result {
                InstructionResult::Return => {
                    last_call.output = Some(outcome.result.output.clone());
                }
                InstructionResult::Revert => {
                    last_call.output = Some(outcome.result.output.clone());
                    last_call.error = Some("Reverted".to_string());

                    // Try to decode the revert reason
                    let output = &outcome.result.output;
                    if output.len() >= 4 + 32 + 32 && output[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
                        let str_len =
                            u32::from_be_bytes([output[36], output[37], output[38], output[39]])
                                as usize;
                        if output.len() >= 4 + 32 + 32 + str_len {
                            let error_msg = String::from_utf8_lossy(
                                &output[4 + 32 + 32..4 + 32 + 32 + str_len],
                            );
                            self.errors.push(ErrorInfo {
                                phase: "eofcreate".to_string(),
                                message: error_msg.to_string(),
                            });
                        }
                    }
                }
                error => {
                    last_call.error = Some(format!("Error: {:?}", error));

                    self.errors.push(ErrorInfo {
                        phase: "eofcreate".to_string(),
                        message: format!("EVM error: {:?}", error),
                    });
                }
            }
        }

        outcome
    }

    //fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
    //    self.selfdestructs.push(SelfDestructInfo {
    //        contract,
    //        target,
    //        value,
    //    });
    //}
}
