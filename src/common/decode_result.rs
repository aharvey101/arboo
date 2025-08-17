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
    /// Custom error with raw parameters
    CustomError(Vec<u8>),
    /// Unknown or malformed error
    Unknown,
}

impl fmt::Display for DecodedEVMRevert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EVM Revert: ")?;
        match &self.error_type {
            EVMErrorType::StringError(msg) => {
                write!(f, "Error(\"{}\") [0x{}]", msg, hex::encode(self.selector))
            }
            EVMErrorType::PanicError(code) => {
                write!(f, "Panic({}): {}", code, panic_code_to_message(*code))
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
        Ok(message) => DecodedEVMRevert {
            selector,
            error_type: EVMErrorType::StringError(message),
            raw_data: data,
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
        let bytes = hex_to_bytes(hex_data).unwrap();
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
    // Test example 1: "SPL"
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000353504c0000000000000000000000000000000000000000000000000000000000").unwrap();
    println!("Example 1: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => assert_eq!(msg, "SPL"),
        _ => panic!("Expected StringError for Example 1"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);

    // Test example 2: "UniswapV2: TRANSFER_FAILED"
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000001a556e697377617056323a205452414e534645525f4641494c4544000000000000").unwrap();
    println!("Example 2: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => assert_eq!(msg, "UniswapV2: TRANSFER_FAILED"),
        _ => panic!("Expected StringError for Example 2"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);

    // Test example 3: "UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT"
    let result = decode_revert_hex("0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002b556e697377617056324c6962726172793a20494e53554646494349454e545f494e5055545f414d4f554e5400000000000000000000000000000000000000000000").unwrap();
    println!("Example 3: {}", result);
    match &result.error_type {
        EVMErrorType::StringError(msg) => {
            assert_eq!(msg, "UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT")
        }
        _ => panic!("Expected StringError for Example 3"),
    }
    assert_eq!(result.selector, [0x08, 0xc3, 0x79, 0xa0]);
}
