//! Account watching configuration.

use forge_index_core::Address;
use serde::{Deserialize, Serialize};

/// Configuration for an account to watch for transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    /// Human-readable name for this account watcher.
    pub name: String,
    /// The chain names to watch this account on.
    pub chain_names: Vec<String>,
    /// The account address to watch.
    pub address: Address,
    /// The block number to start watching from.
    pub start_block: u64,
    /// Whether to include full transaction data.
    pub include_transaction: bool,
}
