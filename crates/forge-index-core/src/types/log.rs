//! EVM log (event emission) type.

use super::{Address, Hash32};
use serde::{Deserialize, Serialize};

/// Represents an EVM log entry emitted by a contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    /// Unique identifier in the format `"{block_hash}-{log_index}"`.
    pub id: String,
    /// The chain ID this log belongs to.
    pub chain_id: u64,
    /// The contract address that emitted this log.
    pub address: Address,
    /// The indexed topics (up to 4).
    pub topics: Vec<Hash32>,
    /// The non-indexed log data.
    pub data: Vec<u8>,
    /// The block number containing this log.
    pub block_number: u64,
    /// The hash of the block containing this log.
    pub block_hash: Hash32,
    /// The hash of the transaction that emitted this log.
    pub transaction_hash: Hash32,
    /// The index of this log within the block.
    pub log_index: u32,
    /// The index of the transaction within the block.
    pub transaction_index: u32,
    /// Whether this log was removed due to a chain reorganization.
    /// Always `false` — forge-index handles reorgs at a higher level.
    pub removed: bool,
}

impl Log {
    /// Creates a `Log` from an alloy RPC log type.
    ///
    /// The `chain_id` must be supplied externally.
    pub fn from_alloy(log: &alloy::rpc::types::Log, chain_id: u64) -> Self {
        let block_hash = log
            .block_hash
            .map(Hash32::from)
            .unwrap_or(Hash32([0u8; 32]));
        let log_index = log.log_index.unwrap_or(0) as u32;

        Self {
            id: format!("{}-{}", block_hash, log_index),
            chain_id,
            address: Address::from(log.address()),
            topics: log.topics().iter().map(|t| Hash32::from(*t)).collect(),
            data: log.data().data.to_vec(),
            block_number: log.block_number.unwrap_or(0),
            block_hash,
            transaction_hash: log
                .transaction_hash
                .map(Hash32::from)
                .unwrap_or(Hash32([0u8; 32])),
            log_index,
            transaction_index: log.transaction_index.unwrap_or(0) as u32,
            removed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address as AlloyAddress, Bytes, B256};

    #[test]
    fn log_id_format_is_block_hash_dash_log_index() {
        let block_hash = Hash32::from(B256::from([0x11; 32]));
        let log = Log {
            id: format!("{}-{}", block_hash, 5),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![],
            data: vec![],
            block_number: 100,
            block_hash,
            transaction_hash: Hash32([0; 32]),
            log_index: 5,
            transaction_index: 0,
            removed: false,
        };

        assert_eq!(log.id, format!("0x{}-5", hex::encode([0x11; 32])));
    }

    #[test]
    fn log_from_alloy_preserves_fields() {
        let contract_addr = AlloyAddress::from([0xCC; 20]);
        let topic = B256::from([0xEE; 32]);
        let block_hash = B256::from([0x33; 32]);
        let tx_hash = B256::from([0x44; 32]);

        let inner_log =
            alloy::primitives::Log::new(contract_addr, vec![topic], Bytes::from(vec![0x01, 0x02]))
                .unwrap();

        let rpc_log = alloy::rpc::types::Log {
            inner: inner_log,
            block_hash: Some(block_hash),
            block_number: Some(200),
            block_timestamp: None,
            transaction_hash: Some(tx_hash),
            transaction_index: Some(1),
            log_index: Some(3),
            removed: false,
        };

        let log = Log::from_alloy(&rpc_log, 42);

        assert_eq!(log.chain_id, 42);
        assert_eq!(log.address, Address::from(contract_addr));
        assert_eq!(log.topics.len(), 1);
        assert_eq!(log.topics[0], Hash32::from(topic));
        assert_eq!(log.data, vec![0x01, 0x02]);
        assert_eq!(log.block_number, 200);
        assert_eq!(log.block_hash, Hash32::from(block_hash));
        assert_eq!(log.transaction_hash, Hash32::from(tx_hash));
        assert_eq!(log.log_index, 3);
        assert_eq!(log.transaction_index, 1);
        assert!(!log.removed);
        assert_eq!(log.id, format!("{}-3", Hash32::from(block_hash)));
    }
}
