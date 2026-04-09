//! ABI type definitions.

use crate::types::Hash32;
use serde::{Deserialize, Serialize};

/// Solidity ABI type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbiType {
    /// Unsigned integer with the given bit width (8, 16, ..., 256).
    Uint(usize),
    /// Signed integer with the given bit width.
    Int(usize),
    /// 20-byte Ethereum address.
    Address,
    /// Boolean.
    Bool,
    /// Fixed-size byte array (bytes1..bytes32).
    FixedBytes(usize),
    /// Dynamic byte array.
    Bytes,
    /// Dynamic UTF-8 string.
    String,
    /// Dynamic-length array of elements of the given type.
    Array(Box<AbiType>),
    /// Fixed-length array of elements.
    FixedArray(Box<AbiType>, usize),
    /// Tuple of heterogeneous types.
    Tuple(Vec<AbiParam>),
}

impl AbiType {
    /// Returns `true` if this type is dynamically sized in ABI encoding.
    pub fn is_dynamic(&self) -> bool {
        match self {
            Self::Bytes | Self::String | Self::Array(_) => true,
            Self::FixedArray(inner, _) => inner.is_dynamic(),
            Self::Tuple(params) => params.iter().any(|p| p.abi_type.is_dynamic()),
            _ => false,
        }
    }

    /// Returns the canonical Solidity type string (e.g. "uint256", "address").
    pub fn to_sol_string(&self) -> String {
        match self {
            Self::Uint(bits) => format!("uint{}", bits),
            Self::Int(bits) => format!("int{}", bits),
            Self::Address => "address".to_string(),
            Self::Bool => "bool".to_string(),
            Self::FixedBytes(n) => format!("bytes{}", n),
            Self::Bytes => "bytes".to_string(),
            Self::String => "string".to_string(),
            Self::Array(inner) => format!("{}[]", inner.to_sol_string()),
            Self::FixedArray(inner, n) => format!("{}[{}]", inner.to_sol_string(), n),
            Self::Tuple(params) => {
                let inner: Vec<String> =
                    params.iter().map(|p| p.abi_type.to_sol_string()).collect();
                format!("({})", inner.join(","))
            }
        }
    }
}

/// A single ABI parameter (input or output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiParam {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub abi_type: AbiType,
    /// Whether this parameter is indexed (events only).
    pub indexed: bool,
}

/// A parsed ABI event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiEvent {
    /// Event name (e.g. "Transfer").
    pub name: String,
    /// Full canonical signature (e.g. "Transfer(address,address,uint256)").
    pub signature: String,
    /// The keccak256 selector (first topic).
    pub selector: Hash32,
    /// Event inputs.
    pub inputs: Vec<AbiParam>,
}

impl AbiEvent {
    /// Computes the keccak256 selector for this event.
    pub fn compute_selector(signature: &str) -> Hash32 {
        let hash = alloy::primitives::keccak256(signature.as_bytes());
        Hash32(hash.0)
    }

    /// Builds the canonical signature string from the event name and inputs.
    pub fn signature_string(name: &str, inputs: &[AbiParam]) -> String {
        let params: Vec<String> = inputs.iter().map(|p| p.abi_type.to_sol_string()).collect();
        format!("{}({})", name, params.join(","))
    }
}

/// A parsed ABI function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiFunction {
    /// Function name.
    pub name: String,
    /// Function inputs.
    pub inputs: Vec<AbiParam>,
    /// Function outputs.
    pub outputs: Vec<AbiParam>,
    /// State mutability (view, pure, nonpayable, payable).
    pub state_mutability: String,
}

/// ABI-related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AbiError {
    /// Failed to parse ABI JSON.
    #[error("ABI parse error: {0}")]
    ParseError(String),
    /// Unknown Solidity type.
    #[error("Unknown ABI type: {0}")]
    UnknownType(String),
    /// Invalid event selector (missing or wrong topic[0]).
    #[error("Invalid event selector")]
    InvalidSelector,
    /// Decoded data does not match expected ABI.
    #[error("ABI decode mismatch: {0}")]
    DecodeMismatch(String),
}
