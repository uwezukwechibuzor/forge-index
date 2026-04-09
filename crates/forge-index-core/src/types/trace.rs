//! EVM trace (internal transaction) type.

use serde::{Deserialize, Serialize};

use super::Address;

/// The type of EVM trace operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceType {
    /// A standard CALL operation.
    Call,
    /// A CALLCODE operation.
    CallCode,
    /// A DELEGATECALL operation.
    DelegateCall,
    /// A STATICCALL operation.
    StaticCall,
    /// A CREATE operation.
    Create,
    /// A CREATE2 operation.
    Create2,
    /// A SELFDESTRUCT operation.
    Selfdestruct,
}

/// Represents an EVM execution trace (internal transaction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    /// Unique identifier in the format `"{transaction_hash}-{trace_position}"`.
    pub id: String,
    /// The type of trace operation.
    pub trace_type: TraceType,
    /// The sender address.
    pub from: Address,
    /// The recipient address (`None` for contract creation).
    pub to: Option<Address>,
    /// The gas provided for this trace.
    pub gas: u64,
    /// The gas consumed by this trace.
    pub gas_used: u64,
    /// The input data to the call.
    pub input: Vec<u8>,
    /// The output data from the call.
    pub output: Option<Vec<u8>>,
    /// Error message if the trace reverted.
    pub error: Option<String>,
    /// Human-readable revert reason, if available.
    pub revert_reason: Option<String>,
    /// The value transferred in wei, if any.
    pub value: Option<u128>,
}
