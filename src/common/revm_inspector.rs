use alloy_primitives::Address;

use revm::interpreter::{
    CallInputs, CallOutcome, CreateInputs, CreateOutcome, EOFCreateInputs,
    InstructionResult, Interpreter,
};
use revm::primitives::{Bytes, Log, B256, U256};
use revm::Database;
use revm::EvmContext;
use std::collections::HashMap;
use std::env;

use crate::common::decode_result::{decode_evm_revert, EVMErrorType};

/// A comprehensive REVM Inspector that tracks:
#[derive(Debug, Default)]
pub struct RevmInspector {
    /// Track the call stack
    pub calls: Vec<CallInfo>,
    /// Track current call depth
    pub call_depth: usize,
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
    pub depth: usize,
    pub caller: Address,
    pub address: Option<Address>,
    pub kind: CallKind,
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
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub phase: String,
    pub message: String,
}

impl RevmInspector {
    /// Check if detailed inspection logging is enabled via ENABLE_DETAILED_INSPECTOR env var
    fn is_detailed_logging_enabled() -> bool {
        env::var("ENABLE_DETAILED_INSPECTOR")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false)
    }

    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            call_depth: 0,
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

        // Check if detailed reporting is enabled
        if Self::is_detailed_logging_enabled() {
            // Full detailed report
            report.push_str("\n=== Detailed Calls Analysis ===\n");
        for (i, call) in self.calls.iter().enumerate() {
            let address_str = call.address
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "None".to_string());
            
            let success = call.error.is_none();
            let status_icon = if success { "✅" } else { "❌" };
            
            report.push_str(&format!(
                "{} Call #{}: {} -> {} ({})\n",
                status_icon,
                i,
                call.caller,
                address_str,
                format!("{:?}", call.kind)
            ));
            
            report.push_str(&format!(
                "   Value: {} wei\n",
                call.value
            ));
            
            report.push_str(&format!(
                "   Gas: {}/{} ({}% used)\n",
                call.gas_used.unwrap_or(0),
                call.gas_limit,
                if call.gas_limit > 0 {
                    (call.gas_used.unwrap_or(0) * 100) / call.gas_limit
                } else {
                    0
                }
            ));
            
            // Show input data if available
            if let Some(input) = &call.input {
                if !input.is_empty() {
                    let input_preview = if input.len() > 32 {
                        format!("{}... ({} bytes total)", hex::encode(&input[..32]), input.len())
                    } else {
                        format!("{} ({} bytes)", hex::encode(input), input.len())
                    };
                    report.push_str(&format!("   Input: {}\n", input_preview));
                    
                    // Try to decode function selector
                    if input.len() >= 4 {
                        let selector = hex::encode(&input[..4]);
                        report.push_str(&format!("   Function Selector: 0x{}\n", selector));
                    }
                }
            }
            
            // Show output data if available
            if let Some(output) = &call.output {
                if !output.is_empty() {
                    let output_preview = if output.len() > 32 {
                        format!("{}... ({} bytes total)", hex::encode(&output[..32]), output.len())
                    } else {
                        format!("{} ({} bytes)", hex::encode(output), output.len())
                    };
                    report.push_str(&format!("   Output: {}\n", output_preview));
                }
            }
            
            // Show error details if present
            if let Some(error) = &call.error {
                report.push_str(&format!("   ❌ Error: {}\n", error));
            }
            
            report.push_str("\n");
        }

        // Summarize storage accesses
        report.push_str("\n=== Storage Accesses ===\n");
        for (address, accesses) in &self.storage_accesses {
            report.push_str(&format!(
                "Contract: {}...\n",
                &address.to_string()[..10]
            ));
            for access in accesses {
                report.push_str(&format!(
                    "  {} slot: {}... value: {}...\n",
                    if access.is_write { "WRITE" } else { "READ " },
                    &access.slot.to_string()[..10],
                    &access.value.to_string()[..10]
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
                "Log #{}: Contract: {}, Topics: {}, Data size: {} bytes\n",
                i,
                log.address,
                log.topics.len(),
                log.data.len()
            ));
        }

        // Detailed errors analysis
        if !self.errors.is_empty() {
            report.push_str("=== 🔍 Detailed Error Analysis ===\n");
            for (i, error) in self.errors.iter().enumerate() {
                report.push_str(&format!(
                    "Error #{}: [{}] {}\n",
                    i + 1,
                    error.phase,
                    error.message
                ));
                
                // Add context based on error type
                if error.message.contains("Stop") {
                    report.push_str("   🔍 Analysis: 'Stop' error usually indicates:\n");
                    report.push_str("      - Insufficient gas for operation\n");
                    report.push_str("      - Invalid opcode or state\n");
                    report.push_str("      - Contract execution halted\n");
                } else if error.message.contains("Revert") {
                    report.push_str("   🔍 Analysis: Contract explicitly reverted:\n");
                    report.push_str("      - Check function requirements/conditions\n");
                    report.push_str("      - Verify input parameters\n");
                    report.push_str("      - Look for require() statements\n");
                } else if error.message.contains("OutOfGas") {
                    report.push_str("   🔍 Analysis: Transaction ran out of gas:\n");
                    report.push_str("      - Increase gas limit\n");
                    report.push_str("      - Check for infinite loops\n");
                    report.push_str("      - Optimize contract logic\n");
                }
                report.push_str("\n");
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
                "{}...: {} wei\n",
                &address.to_string()[..10],
                change_str
            ));
        }
        log::debug!("REPORT: {}", report);
        
        } else {
            // Simple summary when detailed logging is disabled
            let successful_calls = self.calls.iter().filter(|call| call.error.is_none()).count();
            let failed_calls = self.calls.iter().filter(|call| call.error.is_some()).count();
            let total_gas = self.calls.iter().map(|call| call.gas_used.unwrap_or(0)).sum::<u64>();
            
            report.push_str(&format!("📊 Call Summary: {} successful, {} failed, {} total gas used\n", 
                successful_calls, failed_calls, total_gas));
                
            if failed_calls > 0 {
                report.push_str("❌ Some calls failed - enable ENABLE_DETAILED_INSPECTOR=true for full analysis\n");
            }
        }
        
        report
    }

    /// Analyze failed calls specifically for debugging
    pub fn analyze_failures(&self) -> String {
        let mut analysis = String::new();
        
        let failed_calls: Vec<&CallInfo> = self.calls.iter()
            .filter(|call| call.error.is_some())
            .collect();
            
        if failed_calls.is_empty() {
            analysis.push_str("✅ No failed calls detected\n");
            return analysis;
        }
        
        analysis.push_str(&format!("🔍 FAILURE ANALYSIS: {} failed calls detected\n\n", failed_calls.len()));
        
        for (i, failed_call) in failed_calls.iter().enumerate() {
            analysis.push_str(&format!("❌ Failed Call #{}\n", i + 1));
            analysis.push_str(&format!("   Target: {}\n", 
                failed_call.address
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "Contract Creation".to_string())
            ));
            analysis.push_str(&format!("   Caller: {}\n", failed_call.caller));
            analysis.push_str(&format!("   Value: {} wei\n", failed_call.value));
            
            if let Some(error) = &failed_call.error {
                analysis.push_str(&format!("   Error: {}\n", error));
                
                // Provide specific debugging suggestions
                if error.contains("Stop") {
                    analysis.push_str("   💡 Debugging Steps:\n");
                    analysis.push_str("      1. Check if contract exists at target address\n");
                    analysis.push_str("      2. Verify gas limit is sufficient\n");
                    analysis.push_str("      3. Check for invalid opcodes\n");
                    analysis.push_str("      4. Ensure proper contract state\n");
                }
            }
            
            if let Some(input) = &failed_call.input {
                if input.len() >= 4 {
                    let selector = hex::encode(&input[..4]);
                    analysis.push_str(&format!("   Function Called: 0x{}\n", selector));
                    
                    // Try to identify common function selectors
                    match selector.as_str() {
                        "a9059cbb" => analysis.push_str("   📝 Function: transfer(address,uint256)\n"),
                        "23b872dd" => analysis.push_str("   📝 Function: transferFrom(address,address,uint256)\n"),
                        "095ea7b3" => analysis.push_str("   📝 Function: approve(address,uint256)\n"),
                        "dd62ed3e" => analysis.push_str("   📝 Function: allowance(address,address)\n"),
                        "70a08231" => analysis.push_str("   📝 Function: balanceOf(address)\n"),
                        "d0e30db0" => analysis.push_str("   📝 Function: deposit() - WETH wrap\n"),
                        "2e1a7d4d" => analysis.push_str("   📝 Function: withdraw(uint256) - WETH unwrap\n"),
                        _ => analysis.push_str(&format!("   📝 Function: Unknown selector 0x{}\n", selector)),
                    }
                }
            }
            
            analysis.push_str("\n");
        }
        
        analysis
    }
}

impl<DB: Database> revm::Inspector<DB> for RevmInspector {
    //

    fn log(&mut self, _interp: &mut Interpreter, _context: &mut EvmContext<DB>, log: &Log) {
        // Capture the log
        self.logs.push(LogInfo {
            address: log.address,
            topics: log.data.topics().to_vec(),
            data: Bytes::copy_from_slice(&log.data.data),
        });
    }

    fn call(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        // Increment call depth
        self.call_depth += 1;
        
        // Determine call kind based on inputs scheme
        let kind = match inputs.scheme {
            revm::interpreter::CallScheme::Call => CallKind::Call,
            revm::interpreter::CallScheme::CallCode => CallKind::CallCode,
            revm::interpreter::CallScheme::DelegateCall => CallKind::DelegateCall,
            revm::interpreter::CallScheme::StaticCall => CallKind::StaticCall,
            revm::interpreter::CallScheme::ExtCall => CallKind::Call,
            revm::interpreter::CallScheme::ExtStaticCall => CallKind::StaticCall,
            revm::interpreter::CallScheme::ExtDelegateCall => CallKind::DelegateCall,
        };
        
        // Log detailed call information only if enabled
        if Self::is_detailed_logging_enabled() {
            let indent = "  ".repeat(self.call_depth);
            log::debug!("{}📞 CALL #{}: {} -> {}", 
                indent, 
                self.calls.len() + 1,
                inputs.caller,
                inputs.target_address
            );
            log::debug!("{}   Kind: {:?}, Value: {} wei, Gas: {}", 
                indent,
                kind,
                inputs.value.get(),
                inputs.gas_limit
            );
            
            // Decode and log function selector if present
            if inputs.input.len() >= 4 {
                let selector = &inputs.input[0..4];
                let selector_hex = hex::encode(selector);
                let function_name = match selector_hex.as_str() {
                    "d0e30db0" => "deposit() - WETH wrap",
                    "095ea7b3" => "approve(address,uint256) - ERC20 approval",
                    "70a08231" => "balanceOf(address) - ERC20 balance check",
                    "a9059cbb" => "transfer(address,uint256) - ERC20 transfer",
                    "7bd04165" => "arbitrageFunction() - Main arbitrage execution",
                    "38ed1739" => "swapExactTokensForTokens() - Uniswap V2 swap",
                    "04e45aaf" => "exactInputSingle() - Uniswap V3 swap",
                    "128acb08" => "flash() - Flash loan",
                    "fa461e33" => "uniswapV3FlashCallback() - Flash callback",
                    "0902f1ac" => "getReserves() - Uniswap V2 pair reserves",
                    _ => "Unknown function"
                };
                log::debug!("{}   Function: 0x{} - {}", indent, selector_hex, function_name);
                
                // Log input data preview
                if inputs.input.len() > 4 {
                    let data_preview = if inputs.input.len() > 36 {
                        format!("{}... ({} bytes total)", hex::encode(&inputs.input[4..36]), inputs.input.len() - 4)
                    } else {
                        format!("{} ({} bytes)", hex::encode(&inputs.input[4..]), inputs.input.len() - 4)
                    };
                    log::debug!("{}   Input Data: {}", indent, data_preview);
                }
            } else if !inputs.input.is_empty() {
                log::debug!("{}   Raw Input: {} ({} bytes)", indent, hex::encode(&inputs.input), inputs.input.len());
            }
        }
        
        // Record the call
        let call_info = CallInfo {
            depth: self.call_depth,
            address: Some(inputs.target_address),
            caller: inputs.caller,
            kind,
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
        // Decrement call depth
        if self.call_depth > 0 {
            self.call_depth -= 1;
        }
        
        // Extract gas spent from outcome (needed for both logging and call tracking)
        let gas_spent = outcome.gas().spent();
        
        // Log call completion only if detailed logging is enabled
        if Self::is_detailed_logging_enabled() {
            let indent = "  ".repeat(self.call_depth + 1);
            let result = outcome.instruction_result();
            
            log::debug!("{}✅ CALL RESULT: Gas used: {}, Result: {:?}", 
                indent, gas_spent, result);
        }
            
        if let Some(last_call) = self.calls.last_mut() {
            // Extract information from the outcome
            last_call.gas_used = Some(gas_spent);
            last_call.output = Some(outcome.output().clone());
            
            // Log detailed output only if enabled
            if Self::is_detailed_logging_enabled() {
                let result = outcome.instruction_result();
                let indent = "  ".repeat(self.call_depth + 1);

                // Log output data if present
                let output = outcome.output();
                if !output.is_empty() {
                    let output_preview = if output.len() > 32 {
                        format!("{}... ({} bytes total)", hex::encode(&output[..32]), output.len())
                    } else {
                        format!("{} ({} bytes)", hex::encode(output), output.len())
                    };
                    log::debug!("{}   Output: {}", indent, output_preview);
                }

                // Set error information based on the instruction result
                match result {
                    InstructionResult::Return => {
                        log::debug!("{}   ✅ Success", indent);
                    }
                    InstructionResult::Revert => {
                        log::debug!("{}   ❌ Reverted", indent);
                        
                        // Use enhanced error decoding
                        if !output.is_empty() {
                            let decoded = decode_evm_revert(output.clone().into());
                            match &decoded.error_type {
                                EVMErrorType::StringError(msg) => {
                                    log::debug!("{}   Revert reason: {}", indent, msg);
                                }
                                EVMErrorType::KnownCustomError { name, description, .. } => {
                                    log::debug!("{}   Custom Error: {} - {}", indent, name, description);
                                }
                                EVMErrorType::PanicError(code) => {
                                    log::debug!("{}   Panic error: Code {}", indent, code);
                                }
                                EVMErrorType::CustomError(data) => {
                                    log::debug!("{}   Unknown custom error: 0x{}", indent, hex::encode(data));
                                }
                                EVMErrorType::Unknown => {
                                    log::debug!("{}   Unknown error format", indent);
                                }
                            };
                        }
                    }
                    error => {
                        log::debug!("{}   ❌ Error: {:?}", indent, error);
                    }
                }
            }
            
            // Always set error information regardless of logging level
            let result = outcome.instruction_result();
            match result {
                InstructionResult::Return => {
                    // Success, no error
                }
                InstructionResult::Revert => {
                    let output = outcome.output();
                    
                    // Use enhanced error decoding for call info and error collection
                    if !output.is_empty() {
                        let decoded = decode_evm_revert(output.clone().into());
                        let error_message = match &decoded.error_type {
                            EVMErrorType::StringError(msg) => {
                                msg.clone()
                            }
                            EVMErrorType::KnownCustomError { name, description, .. } => {
                                format!("{}: {}", name, description)
                            }
                            EVMErrorType::PanicError(code) => {
                                format!("Panic error: Code {}", code)
                            }
                            EVMErrorType::CustomError(data) => {
                                format!("Unknown custom error: 0x{}", hex::encode(data))
                            }
                            EVMErrorType::Unknown => {
                                "Unknown error format".to_string()
                            }
                        };
                        
                        last_call.error = Some(error_message.clone());
                        self.errors.push(ErrorInfo {
                            phase: "call".to_string(),
                            message: error_message,
                        });
                    } else {
                        last_call.error = Some("Reverted (no data)".to_string());
                        self.errors.push(ErrorInfo {
                            phase: "call".to_string(),
                            message: "Reverted without error data".to_string(),
                        });
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

        outcome
    }

    fn create(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        // Increment call depth
        self.call_depth += 1;
        
        // Record the create
        let call_info = CallInfo {
            depth: self.call_depth,
            address: None,
            caller: inputs.caller,
            kind: CallKind::Create,
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
        // Decrement call depth
        if self.call_depth > 0 {
            self.call_depth -= 1;
        }
        
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

        //log::debug!("outcome of end: {:?}", outcome);
        outcome
    }

    fn eofcreate(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut EOFCreateInputs,
    ) -> Option<CreateOutcome> {
        // Increment call depth
        self.call_depth += 1;
        
        // Record the EOF create (similar to create)
        let call_info = CallInfo {
            depth: self.call_depth,
            caller: inputs.caller,
            address: None,
            kind: CallKind::Create2,
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
        // Decrement call depth
        if self.call_depth > 0 {
            self.call_depth -= 1;
        }
        
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

    fn step(&mut self, interp: &mut Interpreter, _context: &mut EvmContext<DB>) {
        // Track gas usage by opcode
        let opcode = interp.current_opcode();
        let _gas_before = interp.gas.remaining();
        
        // We can't easily track gas per opcode without step_end, so we'll track opcodes encountered
        *self.gas_by_opcode.entry(opcode).or_insert(0) += 1;
    }

    fn step_end(&mut self, interp: &mut Interpreter, _context: &mut EvmContext<DB>) {
        // Track when steps end (could be used for more sophisticated gas tracking)
        let _opcode = interp.current_opcode();
        // For now, just track that the step ended
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        // Track balance changes from selfdestruct
        // Convert Address to B256 for use as key by extending to 32 bytes
        let mut contract_bytes = [0u8; 32];
        contract_bytes[12..32].copy_from_slice(contract.as_slice());
        let contract_key = B256::from(contract_bytes);
        
        let mut target_bytes = [0u8; 32];
        target_bytes[12..32].copy_from_slice(target.as_slice());
        let target_key = B256::from(target_bytes);
        
        // Contract loses its balance (convert U256 to i128 safely)
        let balance_change = if value > U256::from(i128::MAX) {
            i128::MAX // Cap at max value to prevent overflow
        } else {
            value.as_limbs()[0] as i128
        };
        
        *self.balance_changes.entry(contract_key).or_insert(0) -= balance_change;
        *self.balance_changes.entry(target_key).or_insert(0) += balance_change;
    }
}
