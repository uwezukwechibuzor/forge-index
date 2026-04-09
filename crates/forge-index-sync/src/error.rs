//! Sync engine error types.

use forge_index_core::abi::AbiError;
use forge_index_db::DbError;
use forge_index_rpc::RpcError;

/// Errors from the sync engine.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// An RPC call failed.
    #[error("RPC error on chain {chain_id}: {source}")]
    Rpc {
        /// The chain where the error occurred.
        chain_id: u64,
        /// The underlying RPC error.
        source: RpcError,
    },

    /// ABI decoding failed.
    #[error("Decode error for contract '{contract}': {source}")]
    Decode {
        /// The contract being decoded.
        contract: String,
        /// The underlying ABI error.
        source: AbiError,
    },

    /// A database operation failed.
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    /// Failed to decode factory event address parameter.
    #[error(
        "Factory decode error for contract '{contract}': param '{param}' not found or invalid"
    )]
    FactoryDecode {
        /// The factory contract.
        contract: String,
        /// The parameter name that was expected.
        param: String,
    },

    /// An event handler panicked.
    #[error("Handler '{handler}' panicked: {message}")]
    HandlerPanic {
        /// The handler key (e.g. "ERC20:Transfer").
        handler: String,
        /// The panic message.
        message: String,
    },

    /// Referenced chain not found in configuration.
    #[error("Chain not found: {0}")]
    ChainNotFound(u64),
}

impl From<RpcError> for SyncError {
    fn from(e: RpcError) -> Self {
        Self::Rpc {
            chain_id: 0,
            source: e,
        }
    }
}
