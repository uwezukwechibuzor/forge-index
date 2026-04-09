//! Event ordering strategy.

use serde::{Deserialize, Serialize};

/// Controls how events across chains are ordered during indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ordering {
    /// Globally ordered by timestamp, providing consistent cross-chain ordering.
    Omnichain,
    /// Per-chain parallel processing — faster but no cross-chain consistency.
    Multichain,
}
