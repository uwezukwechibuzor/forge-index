//! Backfill worker — fetches and decodes events for one block range.

use std::sync::Arc;

use forge_index_config::{AddressConfig, ContractConfig};
use forge_index_core::abi::decoder::{DecodedEvent, LogDecoder};
use forge_index_core::abi::types::AbiError;
use forge_index_core::types::{Address, Hash32};
use forge_index_rpc::{CachedRpcClient, LogFilter};

use crate::backfill::planner::BlockRange;
use crate::error::SyncError;

/// Fetches and decodes events for one contract within a single block range.
pub struct BackfillWorker {
    /// The RPC client (with cache).
    client: Arc<CachedRpcClient>,
    /// The ABI decoder for this contract.
    decoder: Arc<LogDecoder>,
    /// The contract configuration.
    contract: ContractConfig,
    /// The chain ID.
    chain_id: u64,
    /// Event selectors to watch for (topic[0] values).
    selectors: Vec<Hash32>,
    /// Resolved contract addresses (for non-factory contracts).
    addresses: Vec<Address>,
}

impl BackfillWorker {
    /// Creates a new backfill worker.
    pub fn new(
        client: Arc<CachedRpcClient>,
        decoder: Arc<LogDecoder>,
        contract: ContractConfig,
        chain_id: u64,
        selectors: Vec<Hash32>,
    ) -> Self {
        let addresses = match &contract.address {
            AddressConfig::Single(addr) => vec![*addr],
            AddressConfig::Multiple(addrs) => addrs.clone(),
            AddressConfig::Factory(_) => vec![], // Will be set via set_addresses
        };

        Self {
            client,
            decoder,
            contract,
            chain_id,
            selectors,
            addresses,
        }
    }

    /// Sets the resolved addresses (for factory-discovered contracts).
    pub fn set_addresses(&mut self, addresses: Vec<Address>) {
        self.addresses = addresses;
    }

    /// Fetches and decodes all matching events within the given block range.
    ///
    /// Returns events sorted by (block_number, log_index).
    pub async fn fetch_range(&self, range: &BlockRange) -> Result<Vec<DecodedEvent>, SyncError> {
        let filter = LogFilter {
            address: self.addresses.clone(),
            topics: if self.selectors.is_empty() {
                vec![]
            } else {
                vec![Some(self.selectors.clone())]
            },
            from_block: range.from,
            to_block: range.to,
        };

        let logs = self
            .client
            .get_logs(filter)
            .await
            .map_err(|e| SyncError::Rpc {
                chain_id: self.chain_id,
                source: e,
            })?;

        let mut events = Vec::with_capacity(logs.len());

        for log in &logs {
            match self.decoder.decode(log, &self.contract.name) {
                Ok(event) => events.push(event),
                Err(AbiError::InvalidSelector) => {
                    // Skip logs that don't match any known event (e.g. from other contracts)
                    continue;
                }
                Err(e) => {
                    return Err(SyncError::Decode {
                        contract: self.contract.name.clone(),
                        source: e,
                    });
                }
            }
        }

        // Sort by (block_number, log_index)
        events.sort_by(|a, b| {
            let block_cmp = a.raw_log.block_number.cmp(&b.raw_log.block_number);
            block_cmp.then(a.raw_log.log_index.cmp(&b.raw_log.log_index))
        });

        Ok(events)
    }

    /// Returns the contract name.
    pub fn contract_name(&self) -> &str {
        &self.contract.name
    }
}
