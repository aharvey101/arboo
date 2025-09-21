// Factory for creating pre-configured simulation environments
// Bridges the gap between specific contract bytecode and the generic simulation framework

use crate::common::bytecode::{arboo_bytecode, v2_flash_to_v3_swap_bytecode};
use crate::common::simulation::{MultiContractSimulator, ContractType};
use anyhow::Result;
use log::info;

/// Factory for creating simulation environments with pre-registered contracts
pub struct SimulationFactory;

impl SimulationFactory {
    /// Create a simulator with all arbitrage contracts pre-registered
    pub fn create_arbitrage_simulator() -> Result<MultiContractSimulator> {
        let mut simulator = MultiContractSimulator::new();

        // Register V3→V2 arbitrage contract (original arboo.sol)
        simulator.register_contract(
            ContractType::ArbitrageV3ToV2,
            arboo_bytecode(),
            vec![], // No constructor params
            Some(1_500_000), // Deployment gas limit
            Some(800_000),   // Execution gas limit
        );

        // Register V2→V3 arbitrage contract (V2FlashToV3Swap.sol)
        simulator.register_contract(
            ContractType::ArbitrageV2ToV3,
            v2_flash_to_v3_swap_bytecode(),
            vec![], // No constructor params
            Some(1_500_000), // Deployment gas limit
            Some(800_000),   // Execution gas limit
        );

        info!("🏭 Created arbitrage simulator with {} registered contracts", 
              simulator.get_registered_contracts().len());

        Ok(simulator)
    }

    /// Create a simulator for a specific arbitrage type only
    pub fn create_single_contract_simulator(contract_type: ContractType) -> Result<MultiContractSimulator> {
        let mut simulator = MultiContractSimulator::new();

        match contract_type {
            ContractType::ArbitrageV3ToV2 => {
                simulator.register_contract(
                    ContractType::ArbitrageV3ToV2,
                    arboo_bytecode(),
                    vec![],
                    Some(1_500_000),
                    Some(800_000),
                );
            }
            ContractType::ArbitrageV2ToV3 => {
                simulator.register_contract(
                    ContractType::ArbitrageV2ToV3,
                    v2_flash_to_v3_swap_bytecode(),
                    vec![],
                    Some(1_500_000),
                    Some(800_000),
                );
            }
            _ => {
                return Err(anyhow::anyhow!("Contract type {:?} not supported by factory", contract_type));
            }
        }

        info!("🔧 Created single-contract simulator for: {:?}", contract_type);
        Ok(simulator)
    }

    /// Map from the old arbitrage contract type enum to the new simulation contract type
    pub fn map_arbitrage_type_to_contract_type(
        arbitrage_type: crate::strategies::arbitrage::ArbitrageContractType
    ) -> ContractType {
        match arbitrage_type {
            crate::strategies::arbitrage::ArbitrageContractType::V3ToV2 => ContractType::ArbitrageV3ToV2,
            crate::strategies::arbitrage::ArbitrageContractType::V2ToV3 => ContractType::ArbitrageV2ToV3,
        }
    }

    /// Create simulator specifically configured for the arbitrage strategy
    pub fn create_for_arbitrage_strategy() -> Result<MultiContractSimulator> {
        Self::create_arbitrage_simulator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_arbitrage_simulator() {
        let simulator = SimulationFactory::create_arbitrage_simulator().unwrap();
        let registered = simulator.get_registered_contracts();
        
        assert_eq!(registered.len(), 2);
        assert!(registered.contains(&ContractType::ArbitrageV3ToV2));
        assert!(registered.contains(&ContractType::ArbitrageV2ToV3));
    }

    #[test]
    fn test_create_single_contract_simulator() {
        let simulator = SimulationFactory::create_single_contract_simulator(
            ContractType::ArbitrageV3ToV2
        ).unwrap();
        let registered = simulator.get_registered_contracts();
        
        assert_eq!(registered.len(), 1);
        assert!(registered.contains(&ContractType::ArbitrageV3ToV2));
    }

    #[test]
    fn test_arbitrage_type_mapping() {
        use crate::strategies::arbitrage::ArbitrageContractType;
        
        let v3_to_v2 = SimulationFactory::map_arbitrage_type_to_contract_type(
            ArbitrageContractType::V3ToV2
        );
        assert_eq!(v3_to_v2, ContractType::ArbitrageV3ToV2);

        let v2_to_v3 = SimulationFactory::map_arbitrage_type_to_contract_type(
            ArbitrageContractType::V2ToV3
        );
        assert_eq!(v2_to_v3, ContractType::ArbitrageV2ToV3);
    }
}
