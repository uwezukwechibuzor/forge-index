//! Cache key generation for RPC responses.

use forge_index_core::Address;
use sha2::{Digest, Sha256};

/// Generates a deterministic cache key for an `eth_call` result.
///
/// The key is the SHA-256 hex digest of `to_address + call_data + block_number`.
pub fn eth_call_key(to: &Address, data: &[u8], block_number: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(to.to_string().as_bytes());
    hasher.update(data);
    hasher.update(block_number.to_be_bytes());
    hex::encode(hasher.finalize())
}

/// Generates a deterministic cache key for a log filter query.
///
/// The key is the SHA-256 hex digest of the serialized filter parameters.
pub fn log_filter_key(
    chain_id: u64,
    from_block: u64,
    to_block: u64,
    addresses: &[String],
    topics: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(chain_id.to_be_bytes());
    hasher.update(from_block.to_be_bytes());
    hasher.update(to_block.to_be_bytes());
    for addr in addresses {
        hasher.update(addr.as_bytes());
    }
    for topic in topics {
        hasher.update(topic.as_bytes());
    }
    hex::encode(hasher.finalize())
}
