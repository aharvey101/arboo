use alloy_primitives::U256;

/// Data parsing utilities for simulation results

#[derive(Debug)]
pub enum ParserType {
    UTF8,
    U256,
}

#[derive(Debug)]
pub struct ParserInput<'a> {
    pub parser_type: ParserType,
    pub data: &'a [u8],
}

pub fn parse_data(inputs: Vec<ParserInput>) -> Vec<String> {
    inputs
        .iter()
        .map(|input| match input.parser_type {
            ParserType::UTF8 => String::from_utf8(input.data.to_vec())
                .unwrap_or_else(|_| "Invalid UTF-8".to_string()),
            ParserType::U256 => U256::from_be_slice(input.data).to_string(),
        })
        .collect()
}
