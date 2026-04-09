//! Factory address tracker — dynamic address collection from factory events.

use dashmap::DashMap;
use forge_index_config::FactoryConfig;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::types::Address;

use crate::error::SyncError;

/// Tracks dynamically discovered child addresses from factory events.
pub struct FactoryAddressTracker {
    /// (chain_id, factory_contract_name) -> discovered child addresses
    addresses: DashMap<(u64, String), Vec<Address>>,
}

impl FactoryAddressTracker {
    /// Creates a new factory address tracker.
    pub fn new() -> Self {
        Self {
            addresses: DashMap::new(),
        }
    }

    /// Processes a factory event and extracts the child address.
    ///
    /// Uses the `address_parameter` field from the factory config to determine
    /// which event parameter contains the child contract address.
    pub fn process_factory_event(
        &self,
        event: &DecodedEvent,
        factory: &FactoryConfig,
        chain_id: u64,
        contract_name: &str,
    ) -> Result<(), SyncError> {
        let param =
            event
                .get(&factory.address_parameter)
                .map_err(|_| SyncError::FactoryDecode {
                    contract: contract_name.to_string(),
                    param: factory.address_parameter.clone(),
                })?;

        let address = match param {
            DecodedParam::Address(addr) => *addr,
            _ => {
                return Err(SyncError::FactoryDecode {
                    contract: contract_name.to_string(),
                    param: factory.address_parameter.clone(),
                });
            }
        };

        self.addresses
            .entry((chain_id, contract_name.to_string()))
            .or_default()
            .push(address);

        Ok(())
    }

    /// Returns all discovered addresses for a factory contract.
    pub fn get_addresses(&self, chain_id: u64, factory_contract: &str) -> Vec<Address> {
        self.addresses
            .get(&(chain_id, factory_contract.to_string()))
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Returns the number of discovered addresses for a factory contract.
    pub fn address_count(&self, chain_id: u64, factory_contract: &str) -> usize {
        self.addresses
            .get(&(chain_id, factory_contract.to_string()))
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Adds addresses directly (e.g., loaded from cache).
    pub fn add_addresses(&self, chain_id: u64, contract_name: &str, addrs: Vec<Address>) {
        self.addresses
            .entry((chain_id, contract_name.to_string()))
            .or_default()
            .extend(addrs);
    }
}

impl Default for FactoryAddressTracker {
    fn default() -> Self {
        Self::new()
    }
}
