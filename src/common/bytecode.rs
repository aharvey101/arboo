use revm::primitives::Bytecode;

/// Provides bytecode for pre-compiled contracts used in simulations
/// This module contains all the contract bytecode loading functionality

pub fn arboo_bytecode() -> Bytecode {
    // Read bytecode from hex file
    let hex_content = std::fs::read_to_string("src/bytecode/updated_arbitrage.hex")
        .expect("Failed to read arbitrage contract bytecode from hex file");

    // Remove 0x prefix if present and any whitespace
    let hex_str = hex_content.trim().trim_start_matches("0x");

    let bytes = hex::decode(hex_str).expect("Invalid hex string in bytecode file");
    Bytecode::new_raw(bytes.into())
}

pub fn v2_flash_to_v3_swap_bytecode() -> Bytecode {
    // Read bytecode from hex file
    let hex_content = std::fs::read_to_string("src/bytecode/v2_flash_to_v3_swap.hex")
        .expect("Failed to read V2FlashToV3Swap contract bytecode from hex file");

    // Remove 0x prefix if present and any whitespace
    let hex_str = hex_content.trim().trim_start_matches("0x");

    let bytes = hex::decode(hex_str).expect("Invalid hex string in bytecode file");
    Bytecode::new_raw(bytes.into())
}
