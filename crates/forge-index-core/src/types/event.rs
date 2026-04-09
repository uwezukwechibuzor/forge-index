//! Generic event wrapper combining decoded args with contextual chain data.

use super::{Block, Log, Trace, Transaction};
use serde::{Deserialize, Serialize};

/// A decoded EVM event, generic over the user-defined args struct `T`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event<T> {
    /// The decoded event arguments.
    pub args: T,
    /// The raw log entry.
    pub log: Log,
    /// The block containing this event.
    pub block: Block,
    /// The transaction that emitted this event.
    pub transaction: Transaction,
    /// The execution trace, if available.
    pub trace: Option<Trace>,
    /// The event name (e.g., `"Transfer"`).
    pub name: &'static str,
    /// The contract name that emitted this event.
    pub contract_name: &'static str,
}
