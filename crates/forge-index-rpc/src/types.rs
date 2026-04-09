//! Request/response types and cache key generation.

use forge_index_core::{Address, Hash32, Log};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Filter for querying logs from the RPC provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFilter {
    /// Contract addresses to filter by.
    pub address: Vec<Address>,
    /// Topic filters — each element is an OR-set for that topic position.
    /// `None` means "any value" for that position.
    pub topics: Vec<Option<Vec<Hash32>>>,
    /// Start block (inclusive).
    pub from_block: u64,
    /// End block (inclusive).
    pub to_block: u64,
}

impl LogFilter {
    /// Converts this filter to an alloy `Filter` for use with the provider.
    pub fn to_alloy_filter(&self) -> alloy::rpc::types::Filter {
        use alloy::primitives::{Address as AlloyAddress, B256};
        use alloy::rpc::types::Filter;

        let mut filter = Filter::new()
            .from_block(self.from_block)
            .to_block(self.to_block);

        if !self.address.is_empty() {
            let addrs: Vec<AlloyAddress> = self.address.iter().map(|a| (*a).into()).collect();
            filter = filter.address(addrs);
        }

        for (i, topic_option) in self.topics.iter().enumerate().take(4) {
            if let Some(values) = topic_option {
                let b256s: Vec<B256> = values.iter().map(|h| B256::from(h.0)).collect();
                filter.topics[i] = b256s.into();
            }
        }

        filter
    }
}

/// A simplified transaction receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// The transaction hash.
    pub transaction_hash: Hash32,
    /// The block number containing the transaction.
    pub block_number: u64,
    /// The block hash.
    pub block_hash: Hash32,
    /// The sender address.
    pub from: Address,
    /// The recipient address.
    pub to: Option<Address>,
    /// Gas consumed by the transaction.
    pub gas_used: u64,
    /// Whether the transaction succeeded.
    pub status: bool,
    /// Logs emitted by the transaction.
    pub logs: Vec<Log>,
    /// Contract address created, if any.
    pub contract_address: Option<Address>,
}

/// Generates a deterministic cache key from method name and serialized params.
pub(crate) fn cache_key(method: &str, params: &impl Serialize) -> String {
    let params_json = serde_json::to_string(params).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(method.as_bytes());
    hasher.update(params_json.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_filter_encodes_topics_as_hex() {
        let hash =
            Hash32::from("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        let filter = LogFilter {
            address: vec![],
            topics: vec![Some(vec![hash])],
            from_block: 0,
            to_block: 100,
        };
        let alloy_filter = filter.to_alloy_filter();
        let json = serde_json::to_value(&alloy_filter).unwrap();
        let json_str = serde_json::to_string(&json).unwrap();

        // The topics should contain the hex-encoded hash somewhere in the JSON
        let expected_hex = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
        assert!(
            json_str.contains(expected_hex),
            "Expected topics to contain {}, got: {}",
            expected_hex,
            json_str
        );

        // Verify topics array exists and has the right structure
        let topics = json.get("topics").unwrap();
        assert!(topics.is_array(), "topics should be an array");
        let topics_arr = topics.as_array().unwrap();
        assert!(!topics_arr.is_empty(), "topics should not be empty");

        // topic0 may be a string (single value) or array (multiple values)
        let topic0 = &topics_arr[0];
        let encoded = if topic0.is_string() {
            topic0.as_str().unwrap().to_string()
        } else if topic0.is_array() {
            topic0.as_array().unwrap()[0].as_str().unwrap().to_string()
        } else {
            panic!("Unexpected topic format: {:?}", topic0);
        };

        assert!(encoded.starts_with("0x"), "topic should be 0x-prefixed hex");
        assert_eq!(encoded.len(), 66, "topic should be 66 chars (0x + 64)");
    }
}
