use std::fmt;

/// Represents a decoded EVM revert error
#[derive(Debug)]
pub struct DecodedEVMRevert {
    /// Error signature (selector)
    pub selector: [u8; 4],
    /// Error type
    pub error_type: EVMErrorType,
    /// Raw bytes of the revert data
    pub raw_data: Vec<u8>,
}

/// Types of EVM errors we can decode
#[derive(Debug)]
pub enum EVMErrorType {
    /// String error (most common): Error(string)
    StringError(String),
    /// Panic error with a uint256 error code
    PanicError(u64),
    /// Known custom error with decoded parameters
    KnownCustomError { name: String, description: String, params: Vec<u8> },
    /// Unknown custom error with raw parameters
    CustomError(Vec<u8>),
    /// Unknown or malformed error
    Unknown,
}

impl fmt::Display for DecodedEVMRevert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EVM Revert: ")?;
        match &self.error_type {
            EVMErrorType::StringError(msg) => {
                write!(f, "{} [0x{}]", msg, hex::encode(self.selector))
            }
            EVMErrorType::PanicError(code) => {
                write!(f, "Panic({}): {}", code, panic_code_to_message(*code))
            }
            EVMErrorType::KnownCustomError { name, description, params: _ } => {
                write!(f, "{}: {} [0x{}]", name, description, hex::encode(self.selector))
            }
            EVMErrorType::CustomError(data) => write!(
                f,
                "Custom Error [0x{}] with data: 0x{}",
                hex::encode(self.selector),
                hex::encode(data)
            ),
            EVMErrorType::Unknown => {
                write!(f, "Unknown Error Format [0x{}]", hex::encode(self.selector))
            }
        }
    }
}

/// Convert Solidity panic codes to human-readable messages
fn panic_code_to_message(code: u64) -> &'static str {
    match code {
        0x01 => "Assertion failed",
        0x11 => "Arithmetic operation underflowed or overflowed",
        0x12 => "Division or modulo by zero",
        0x21 => "Entered an invalid opcode",
        0x22 => "Out of gas",
        0x31 => "Array out of bounds access",
        0x32 => "Access to storage outside of allocated slot",
        0x41 => "Failed to allocate memory",
        0x51 => "Called a zero-initialized variable of internal function type",
        _ => "Unknown panic code",
    }
}

/// Translate common abbreviated DeFi error messages to more descriptive ones
fn translate_error_message(message: &str) -> String {
    match message {
        // Common Uniswap/DEX errors
        "SPL" => "Slippage Protection: Transaction would exceed maximum slippage tolerance".to_string(),
        "LOK" => "Locked: Pool or contract is currently locked (possibly during rebalancing or emergency pause)".to_string(),
        "IIA" => "Insufficient Input Amount: Not enough tokens provided for the swap".to_string(),
        "IOA" => "Insufficient Output Amount: Swap would not produce enough output tokens".to_string(),
        "IL" => "Insufficient Liquidity: Pool does not have enough liquidity for this trade size".to_string(),
        "IP" => "Invalid Path: The token swap path is invalid or contains unsupported pairs".to_string(),
        "K" => "K Invariant: Uniswap V2 constant product formula violation (x*y=k)".to_string(),
        "EXPIRED" => "Transaction Expired: Deadline for transaction execution has passed".to_string(),
        "FORBIDDEN" => "Forbidden: Operation not allowed by the contract or caller lacks permission".to_string(),
        "IDENTICAL_ADDRESSES" => "Identical Addresses: Cannot create pair with the same token address".to_string(),
        "PAIR_EXISTS" => "Pair Exists: Trading pair already exists for these tokens".to_string(),
        "ZERO_ADDRESS" => "Zero Address: Invalid zero address provided where token address expected".to_string(),
        
        // Flash loan specific errors
        "FLASH_LOAN_FAILED" => "Flash Loan Failed: Callback validation or repayment failed".to_string(),
        "INSUFFICIENT_FLASH_LOAN_BALANCE" => "Insufficient Flash Loan Balance: Not enough balance to repay flash loan".to_string(),
        
        // ERC20 errors
        "TRANSFER_FAILED" => "Transfer Failed: ERC20 token transfer was unsuccessful".to_string(),
        "APPROVE_FAILED" => "Approval Failed: ERC20 token approval was unsuccessful".to_string(),
        "INSUFFICIENT_BALANCE" => "Insufficient Balance: Account does not have enough token balance".to_string(),
        "INSUFFICIENT_ALLOWANCE" => "Insufficient Allowance: Spender allowance is too low for this transfer".to_string(),
        
        // Arithmetic errors
        "OVERFLOW" => "Arithmetic Overflow: Calculation result exceeds maximum value".to_string(),
        "UNDERFLOW" => "Arithmetic Underflow: Calculation result goes below minimum value".to_string(),
        "DIV_BY_ZERO" => "Division by Zero: Attempted to divide by zero".to_string(),
        
        // Access control errors
        "NOT_OWNER" => "Not Owner: Only the contract owner can perform this action".to_string(),
        "UNAUTHORIZED" => "Unauthorized: Caller is not authorized to perform this action".to_string(),
        "PAUSED" => "Contract Paused: Contract operations are currently paused".to_string(),
        
        // If no translation found, return original with context hint
        _ => {
            if message.len() <= 5 && message.chars().all(|c| c.is_ascii_uppercase()) {
                format!("{} (abbreviated error - likely DeFi/DEX related)", message)
            } else {
                message.to_string()
            }
        }
    }
}

/// Decode known custom error selectors
fn decode_known_custom_error(selector: [u8; 4], data: Vec<u8>) -> Option<EVMErrorType> {
    match selector {
        // Common Uniswap V2/V3 errors - this seems to be the most frequent one
        [0x8b, 0x02, 0x88, 0x3f] => {
            let mut description = "Uniswap swap calculation failed".to_string();
            if data.len() >= 68 { // 4 bytes selector + 2 * 32 bytes parameters
                let param1_bytes = &data[4..36];
                let param2_bytes = &data[36..68];
                
                let param1_formatted = format_u256_bytes(param1_bytes);
                let param2_formatted = format_u256_bytes(param2_bytes);
                
                description.push_str(&format!(" | Input: {} | Output: {}", param1_formatted, param2_formatted));
            }
            Some(EVMErrorType::KnownCustomError {
                name: "SwapCalculationError".to_string(),
                description,
                params: data[4..].to_vec(),
            })
        },
        [0x08, 0x1f, 0xf1, 0x2e] => Some(EVMErrorType::KnownCustomError {
            name: "InsufficientOutputAmount".to_string(),
            description: "The swap would not produce enough output tokens".to_string(),
            params: data[4..].to_vec(),
        }),
        [0x08, 0x62, 0x2a, 0x8e] => Some(EVMErrorType::KnownCustomError {
            name: "InsufficientInputAmount".to_string(),
            description: "Not enough input tokens provided for the swap".to_string(),
            params: data[4..].to_vec(),
        }),
        [0x08, 0x5f, 0xb4, 0x2e] => Some(EVMErrorType::KnownCustomError {
            name: "InsufficientLiquidity".to_string(),
            description: "Pool does not have enough liquidity for this trade".to_string(),
            params: data[4..].to_vec(),
        }),
        [0x08, 0x7e, 0x2a, 0x5f] => Some(EVMErrorType::KnownCustomError {
            name: "InvalidPath".to_string(),
            description: "The token swap path is invalid".to_string(),
            params: data[4..].to_vec(),
        }),
        [0x4e, 0x48, 0x7b, 0x71] => Some(EVMErrorType::KnownCustomError {
            name: "Panic".to_string(),
            description: "Contract execution panicked".to_string(),
            params: data[4..].to_vec(),
        }),
        // Flash loan errors
        [0x46, 0xb2, 0xa0, 0x21] => Some(EVMErrorType::KnownCustomError {
            name: "FlashLoanFailed".to_string(),
            description: "Flash loan callback validation failed".to_string(),
            params: data[4..].to_vec(),
        }),
        // Generic ERC20 errors
        [0x94, 0x28, 0x0d, 0x62] => Some(EVMErrorType::KnownCustomError {
            name: "TransferFailed".to_string(),
            description: "ERC20 token transfer failed".to_string(),
            params: data[4..].to_vec(),
        }),
        [0xa9, 0x05, 0x9c, 0xbb] => Some(EVMErrorType::KnownCustomError {
            name: "ApprovalFailed".to_string(),
            description: "ERC20 token approval failed".to_string(),
            params: data[4..].to_vec(),
        }),
        // Add more known selectors as we encounter them
        _ => None,
    }
}

/// Decode an EVM revert error from bytes
pub fn decode_evm_revert(data: Vec<u8>) -> DecodedEVMRevert {
    // Check if we have enough data for a selector (4 bytes)
    if data.len() < 4 {
        return DecodedEVMRevert {
            selector: [0; 4],
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        };
    }

    // Extract the selector (first 4 bytes)
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&data[0..4]);

    // Handle the common string error case (Error(string))
    if selector == [0x08, 0xc3, 0x79, 0xa0] {
        return decode_string_error(data);
    }

    // Handle Panic errors
    if selector == [0x4e, 0x48, 0x7b, 0x71] {
        return decode_panic_error(data);
    }

    // Try to decode known custom errors first
    if let Some(known_error) = decode_known_custom_error(selector, data.clone()) {
        return DecodedEVMRevert {
            selector,
            error_type: known_error,
            raw_data: data,
        };
    }

    // Log unknown selectors to help improve our decoder
    log::debug!("Unknown custom error selector: 0x{}, data length: {}", hex::encode(selector), data.len());

    // Handle other custom errors
    DecodedEVMRevert {
        selector,
        error_type: EVMErrorType::CustomError(data[4..].to_vec()),
        raw_data: data,
    }
}

/// Decode a string error (Error(string))
fn decode_string_error(data: Vec<u8>) -> DecodedEVMRevert {
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&data[0..4]);

    // Need at least 4 bytes for selector + 32 bytes for offset + 32 bytes for length
    if data.len() < 68 {
        return DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        };
    }

    // Parse the offset (should be 0x20 = 32)
    let offset_bytes = &data[4..36];
    let offset = u256_to_u64(offset_bytes);

    // offset must be 32
    if offset != 32 {
        return DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        };
    }

    // Parse the length of the string
    let length_bytes = &data[36..68];
    let length = u256_to_u64(length_bytes) as usize;

    // Ensure we have enough data for the string
    let expected_size = 4 + 32 + 32 + length;
    if data.len() < expected_size {
        return DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        };
    }

    // Extract and convert the string
    let string_bytes = &data[68..68 + length];
    match String::from_utf8(string_bytes.to_vec()) {
        Ok(message) => {
            let translated_message = translate_error_message(&message);
            DecodedEVMRevert {
                selector,
                error_type: EVMErrorType::StringError(translated_message),
                raw_data: data,
            }
        },
        Err(_) => DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        },
    }
}

/// Decode a panic error (Panic(uint256))
fn decode_panic_error(data: Vec<u8>) -> DecodedEVMRevert {
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&data[0..4]);

    // Need at least 4 bytes for selector + 32 bytes for the panic code
    if data.len() < 36 {
        return DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::Unknown,
            raw_data: data,
        };
    }

    // Parse the panic code
    let code_bytes = &data[4..36];
    let code = u256_to_u64(code_bytes);

    DecodedEVMRevert {
        selector,
        error_type: EVMErrorType::PanicError(code),
        raw_data: data,
    }
}

/// Convert a big-endian u256 bytes to u64
fn u256_to_u64(bytes: &[u8]) -> u64 {
    // For simplicity, we just read the last 8 bytes (64 bits)
    // This assumes the number fits in u64, which is usually the case for lengths and offsets
    let mut result = 0u64;
    let start = bytes.len().saturating_sub(8);

    let tmp = bytes[start..].iter().enumerate();
    for (_, byte) in tmp {
        result = (result << 8) | (*byte as u64);
    }

    result
}

/// Convert u256 bytes to a more readable format
fn format_u256_bytes(bytes: &[u8]) -> String {
    if bytes.len() >= 32 {
        // Try to interpret as both hex and decimal for better readability
        let hex_str = hex::encode(bytes);
        // Check if it's a small number that fits in u64
        let mut is_small_number = true;
        for i in 0..24 { // First 24 bytes should be zero for small numbers
            if bytes[i] != 0 {
                is_small_number = false;
                break;
            }
        }
        
        if is_small_number {
            let decimal_val = u256_to_u64(bytes);
            if decimal_val < 1_000_000_000_000_000_000 { // Less than 10^18 (reasonable for token amounts)
                format!("0x{} ({})", hex_str, decimal_val)
            } else {
                // Might be wei amount, show in ether too
                let ether = decimal_val as f64 / 1e18;
                format!("0x{} ({} wei = {:.6} ETH)", hex_str, decimal_val, ether)
            }
        } else {
            format!("0x{}", hex_str)
        }
    } else {
        format!("0x{}", hex::encode(bytes))
    }
}

/// Utility function to convert hex string to bytes
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, hex::FromHexError> {
    let hex = hex.trim_start_matches("0x");
    hex::decode(hex)
}

/// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_string_error() {
        // Example error: "UniswapV2: TRANSFER_FAILED"
        let hex_data = "0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000001a556e697377617056323a205452414e534645525f4641494c4544000000000000";
        let bytes = hex_to_bytes(hex_data).expect("Test hex should be valid");
        let decoded = decode_evm_revert(bytes);

        match decoded.error_type {
            EVMErrorType::StringError(msg) => {
                assert_eq!(msg, "UniswapV2: TRANSFER_FAILED");
            }
            _ => panic!("Expected StringError"),
        }
    }
}

/// Main function to decode a revert error from hex string
pub fn decode_revert_hex(hex_error: &str) -> Result<DecodedEVMRevert, hex::FromHexError> {
    let bytes = hex_to_bytes(hex_error)?;
    Ok(decode_evm_revert(bytes))
}

#[test]
fn test_real_examples() {
    // Test example 1: "SPL" -> translated to slippage protection message
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000353504c0000000000000000000000000000000000000000000000000000000000").expect("Test hex should be valid");
    println!("Example 1: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => assert_eq!(msg, "Slippage Protection: Transaction would exceed maximum slippage tolerance"),
        _ => panic!("Expected StringError for Example 1"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);

    // Test example 2: "UniswapV2: TRANSFER_FAILED"
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000001a556e697377617056323a205452414e534645525f4641494c4544000000000000").expect("Test hex should be valid");
    println!("Example 2: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => assert_eq!(msg, "UniswapV2: TRANSFER_FAILED"),
        _ => panic!("Expected StringError for Example 2"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);

    // Test example 3: "UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT"
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002b556e697377617056324c6962726172793a20494e53554646494349454e545f494e5055545f414d4f554e5400000000000000000000000000000000000000000000").expect("Test hex should be valid");
    println!("Example 3: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => {
            assert_eq!(msg, "UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT")
        }
        _ => panic!("Expected StringError for Example 3"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);
}
