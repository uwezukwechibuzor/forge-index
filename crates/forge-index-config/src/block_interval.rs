//! Block interval configuration.

use serde::{Deserialize, Serialize};

/// Configuration for a block-interval handler that triggers every N blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockIntervalConfig {
    /// Human-readable name for this block interval handler.
    pub name: String,
    /// The chain name this interval runs on.
    pub chain_name: String,
    /// How many blocks between each trigger.
    pub interval: u64,
    /// The block number to start from.
    pub start_block: u64,
    /// Optional block number to stop at.
    pub end_block: Option<u64>,
}
