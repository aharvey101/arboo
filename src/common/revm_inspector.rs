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

/// Comprehensive mapping of function selectors to human-readable function signatures
fn get_function_signature(selector: &str) -> &'static str {
    match selector {
        // ERC20 Functions
        "70a08231" => "balanceOf(address)",
        "a9059cbb" => "transfer(address,uint256)",
        "095ea7b3" => "approve(address,uint256)",
        "dd62ed3e" => "allowance(address,address)",
        "23b872dd" => "transferFrom(address,address,uint256)",
        "06fdde03" => "name()",
        "95d89b41" => "symbol()",
        "313ce567" => "decimals()",
        "18160ddd" => "totalSupply()",
        
        // WETH Functions
        "d0e30db0" => "deposit()",
        "2e1a7d4d" => "withdraw(uint256)",
        
        // Uniswap V2 Functions
        "38ed1739" => "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
        "8803dbee" => "swapTokensForExactTokens(uint256,uint256,address[],address,uint256)",
        "7ff36ab5" => "swapExactETHForTokens(uint256,address[],address,uint256)",
        "4a25d94a" => "swapTokensForExactETH(uint256,uint256,address[],address,uint256)",
        "18cbafe5" => "swapExactTokensForETH(uint256,uint256,address[],address,uint256)",
        "fb3bdb41" => "swapETHForExactTokens(uint256,address[],address,uint256)",
        "02751cec" => "removeLiquidity(address,address,uint256,uint256,uint256,address,uint256)",
        "af2979eb" => "removeLiquidityETH(address,uint256,uint256,uint256,address,uint256)",
        "e8e33700" => "addLiquidity(address,address,uint256,uint256,uint256,uint256,address,uint256)",
        "f305d719" => "addLiquidityETH(address,uint256,uint256,uint256,address,uint256)",
        "0902f1ac" => "getReserves()",
        "89afcb44" => "getAmountOut(uint256,uint256,uint256)",
        "85f8c259" => "getAmountIn(uint256,uint256,uint256)",
        "d06ca61f" => "getAmountsOut(uint256,address[])",
        "1f00ca74" => "getAmountsIn(uint256,address[])",
        
        // Uniswap V3 Functions
        "04e45aaf" => "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
        "c04b8d59" => "exactInput((bytes,address,uint256,uint256,uint256))",
        "db3e2198" => "exactOutputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
        "f28c0498" => "exactOutput((bytes,address,uint256,uint256,uint256))",
        "414bf389" => "exactInputSingle_v2((address,address,uint24,address,uint256,uint256,uint256,uint160))",
        "b858183f" => "exactInput_v2((bytes,address,uint256,uint256,uint256))",
        "09b81346" => "exactOutputSingle_v2((address,address,uint24,address,uint256,uint256,uint256,uint160))",
        "f7729d43" => "exactOutput_v2((bytes,address,uint256,uint256,uint256))",
        "fa461e33" => "uniswapV3FlashCallback(uint256,uint256,bytes)",
        "f3995c67" => "uniswapV3MintCallback(uint256,uint256,bytes)",
        "23a69e75" => "uniswapV3SwapCallback(int256,int256,bytes)",
        
        // Uniswap V3 Pool Functions
        "3c8a7d8d" => "swap(address,bool,int256,uint160,bytes)",
        "a34123a7" => "mint(address,int24,int24,uint128,bytes)",
        "0c49ccbe" => "burn(int24,int24,uint128)",
        "fc6f7865" => "collect(address,int24,int24,uint128,uint128)",
        "490e6cbc" => "snapshotCumulativesInside(int24,int24)",
        "514ea4bf" => "increaseObservationCardinalityNext(uint16)",
        
        // Flash Loan Functions
        "128acb08" => "flash(address,uint256,uint256,bytes)",
        "618dc65e" => "flashLoan(address,address[],uint256[],uint256[],address,bytes,uint16)",
        "ab9c4b5d" => "flashLoanSimple(address,address,uint256,bytes,uint16)",
        
        // Arbitrage Functions
        "7bd04165" => "arbitrageFunction()",
        "5d47ff29" => "executeArbitrage(address,address,uint256,uint256,bool,bytes)",
        "e83bfc7d" => "arbitrage(address,address,uint256,bool)",
        
        // Multicall Functions
        "ac9650d8" => "multicall(bytes[])",
        "1f0464d1" => "multicall(bytes[])",
        "5ae401dc" => "multicall(uint256,bytes[])",
        
        // Common DEX Functions
        "022c0d9f" => "swap(uint256,uint256,address,bytes)",
        "ced7b2d3" => "swapExactAmountIn(address,uint256,address,uint256,uint256)",
        "8201aa3f" => "swapExactAmountOut(address,uint256,address,uint256,uint256)",
        
        // Governance Functions
        "fe0d94c1" => "execute(uint256)",
        "40e58ee5" => "propose(address[],uint256[],string[],bytes[],string)",
        "56781388" => "castVote(uint256,uint8)",
        "15373e3d" => "castVoteWithReason(uint256,uint8,string)",
        
        // Common Contract Functions
        "8da5cb5b" => "owner()",
        "f2fde38b" => "transferOwnership(address)",
        "715018a6" => "renounceOwnership()",
        "8456cb59" => "pause()",
        "3f4ba83a" => "unpause()",
        "5c975abb" => "paused()",
        
        // Proxy Functions
        "3659cfe6" => "upgradeTo(address)",
        "4f1ef286" => "upgradeToAndCall(address,bytes)",
        "52d1902d" => "proxiableUUID()",
        
        // Factory Functions
        "c9c65396" => "createPair(address,address)",
        "e6a43905" => "getPair(address,address)",
        "1e3dd18b" => "allPairs(uint256)",
        "574f2ba3" => "allPairsLength()",
        "5909c0d5" => "createPool(address,address,uint24)",
        "1698ee82" => "getPool(address,address,uint24)",
        
        // Staking Functions
        "a694fc3a" => "stake(uint256)",
        "3d18b912" => "getReward()",
        "8b876347" => "earned(address)",
        
        // NFT Functions
        "081812fc" => "getApproved(uint256)",
        "42842e0e" => "safeTransferFrom(address,address,uint256)",
        "b88d4fde" => "safeTransferFrom(address,address,uint256,bytes)",
        "a22cb465" => "setApprovalForAll(address,bool)",
        "e985e9c5" => "isApprovedForAll(address,address)",
        "6352211e" => "ownerOf(uint256)",
        "c87b56dd" => "tokenURI(uint256)",
        
        _ => "Unknown Function"
    }
}

/// Extract and format function parameters from calldata
fn decode_function_parameters(selector: &str, input: &[u8]) -> String {
    if input.len() < 4 {
        return String::new();
    }
    
    let params_data = &input[4..];
    if params_data.is_empty() {
        return String::new();
    }
    
    match selector {
        // Single address parameter
        "70a08231" | "8da5cb5b" | "f2fde38b" => {
            if params_data.len() >= 32 {
                let addr_bytes = &params_data[12..32];
                format!(" → address: 0x{}", hex::encode(addr_bytes))
            } else {
                String::new()
            }
        }
        // Single uint256 parameter  
        "2e1a7d4d" | "a694fc3a" => {
            if params_data.len() >= 32 {
                let amount = U256::from_be_slice(&params_data[0..32]);
                format!(" → amount: {}", amount)
            } else {
                String::new()
            }
        }
        // Address + uint256 (transfer, approve)
        "a9059cbb" | "095ea7b3" => {
            if params_data.len() >= 64 {
                let addr_bytes = &params_data[12..32];
                let amount = U256::from_be_slice(&params_data[32..64]);
                format!(" → to: 0x{}, amount: {}", hex::encode(addr_bytes), amount)
            } else {
                String::new()
            }
        }
        // Two addresses (allowance, getPair)
        "dd62ed3e" | "e6a43905" => {
            if params_data.len() >= 64 {
                let addr1_bytes = &params_data[12..32];
                let addr2_bytes = &params_data[44..64];
                format!(" → addr1: 0x{}, addr2: 0x{}", hex::encode(addr1_bytes), hex::encode(addr2_bytes))
            } else {
                String::new()
            }
        }
        // Swap functions - just show first few parameters
        "38ed1739" | "8803dbee" | "18cbafe5" => {
            if params_data.len() >= 64 {
                let amount_in = U256::from_be_slice(&params_data[0..32]);
                let amount_out = U256::from_be_slice(&params_data[32..64]);
                format!(" → amountIn: {}, amountOut: {}", amount_in, amount_out)
            } else {
                String::new()
            }
        }
        _ => {
            // For unknown functions, just show parameter count
            let param_count = params_data.len() / 32;
            if param_count > 0 {
                format!(" → {} parameters", param_count)
            } else {
                String::new()
            }
        }
    }
}

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
                        let function_signature = get_function_signature(&selector);
                        let parameters = decode_function_parameters(&selector, input);
                        
                        report.push_str(&format!("   📋 Function: 0x{} → {}{}\n", 
                            selector, function_signature, parameters));
                            
                        // Add context for important function types
                        match selector.as_str() {
                            "38ed1739" | "8803dbee" | "18cbafe5" | "4a25d94a" => {
                                report.push_str("      🔄 Uniswap V2 Swap\n");
                            }
                            "04e45aaf" | "c04b8d59" | "db3e2198" | "f28c0498" => {
                                report.push_str("      🔄 Uniswap V3 Swap\n");
                            }
                            "128acb08" | "618dc65e" | "ab9c4b5d" => {
                                report.push_str("      ⚡ Flash Loan\n");
                            }
                            "7bd04165" | "5d47ff29" | "e83bfc7d" => {
                                report.push_str("      💰 Arbitrage Function\n");
                            }
                            "d0e30db0" => {
                                report.push_str("      📦 WETH Wrap\n");
                            }
                            "2e1a7d4d" => {
                                report.push_str("      📤 WETH Unwrap\n");
                            }
                            _ => {}
                        }
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
                    let function_signature = get_function_signature(&selector);
                    let parameters = decode_function_parameters(&selector, input);
                    
                    analysis.push_str(&format!("   📋 Function Called: 0x{} → {}{}\n", 
                        selector, function_signature, parameters));
                    
                    // Add specific debugging advice based on function type
                    match selector.as_str() {
                        "a9059cbb" | "23b872dd" => {
                            analysis.push_str("   � ERC20 Transfer Issues:\n");
                            analysis.push_str("      1. Check token balance\n");
                            analysis.push_str("      2. Verify allowance (for transferFrom)\n");
                            analysis.push_str("      3. Ensure recipient address is valid\n");
                        }
                        "095ea7b3" => {
                            analysis.push_str("   � ERC20 Approval Issues:\n");
                            analysis.push_str("      1. Check if spender address is valid\n");
                            analysis.push_str("      2. Some tokens require zero approval first\n");
                        }
                        "38ed1739" | "8803dbee" | "18cbafe5" => {
                            analysis.push_str("   � Uniswap V2 Swap Issues:\n");
                            analysis.push_str("      1. Check slippage tolerance\n");
                            analysis.push_str("      2. Verify pool liquidity\n");
                            analysis.push_str("      3. Check token approvals\n");
                            analysis.push_str("      4. Verify deadline hasn't passed\n");
                        }
                        "04e45aaf" | "c04b8d59" => {
                            analysis.push_str("   � Uniswap V3 Swap Issues:\n");
                            analysis.push_str("      1. Check price limits (sqrtPriceLimitX96)\n");
                            analysis.push_str("      2. Verify pool exists and has liquidity\n");
                            analysis.push_str("      3. Check token approvals\n");
                            analysis.push_str("      4. Verify deadline\n");
                        }
                        "128acb08" => {
                            analysis.push_str("   � Flash Loan Issues:\n");
                            analysis.push_str("      1. Check callback implementation\n");
                            analysis.push_str("      2. Verify fee payment\n");
                            analysis.push_str("      3. Ensure sufficient balance for repayment\n");
                        }
                        "d0e30db0" => {
                            analysis.push_str("   � WETH Wrap Issues:\n");
                            analysis.push_str("      1. Check ETH balance\n");
                            analysis.push_str("      2. Verify msg.value matches deposit amount\n");
                        }
                        "2e1a7d4d" => {
                            analysis.push_str("   � WETH Unwrap Issues:\n");
                            analysis.push_str("      1. Check WETH balance\n");
                            analysis.push_str("      2. Verify unwrap amount is valid\n");
                        }
                        _ => {
                            analysis.push_str("   💡 General Debugging Steps:\n");
                            analysis.push_str("      1. Check contract exists at target address\n");
                            analysis.push_str("      2. Verify gas limit is sufficient\n");
                            analysis.push_str("      3. Check for invalid opcodes\n");
                            analysis.push_str("      4. Ensure proper contract state\n");
                        }
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
                let function_signature = get_function_signature(&selector_hex);
                let parameters = decode_function_parameters(&selector_hex, &inputs.input);
                
                log::debug!("{}   📋 Function: 0x{} → {}{}", 
                    indent, selector_hex, function_signature, parameters);
                
                // Log additional context for important functions
                match selector_hex.as_str() {
                    "38ed1739" | "8803dbee" | "18cbafe5" | "4a25d94a" => {
                        log::debug!("{}      🔄 Uniswap V2 Swap Detected", indent);
                    }
                    "04e45aaf" | "c04b8d59" | "db3e2198" | "f28c0498" => {
                        log::debug!("{}      🔄 Uniswap V3 Swap Detected", indent);
                    }
                    "128acb08" | "618dc65e" | "ab9c4b5d" => {
                        log::debug!("{}      ⚡ Flash Loan Detected", indent);
                    }
                    "7bd04165" | "5d47ff29" | "e83bfc7d" => {
                        log::debug!("{}      💰 Arbitrage Function Detected", indent);
                    }
                    "d0e30db0" => {
                        log::debug!("{}      📦 WETH Wrap Detected", indent);
                    }
                    "2e1a7d4d" => {
                        log::debug!("{}      📤 WETH Unwrap Detected", indent);
                    }
                    _ => {}
                }
                
                // Log raw input data for debugging if needed
                if inputs.input.len() > 4 {
                    let data_preview = if inputs.input.len() > 36 {
                        format!("{}... ({} bytes total)", hex::encode(&inputs.input[4..36]), inputs.input.len() - 4)
                    } else {
                        format!("{} ({} bytes)", hex::encode(&inputs.input[4..]), inputs.input.len() - 4)
                    };
                    log::debug!("{}   📊 Raw Data: {}", indent, data_preview);
                }
            } else if !inputs.input.is_empty() {
                log::debug!("{}   📊 Raw Input: {} ({} bytes)", indent, hex::encode(&inputs.input), inputs.input.len());
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
