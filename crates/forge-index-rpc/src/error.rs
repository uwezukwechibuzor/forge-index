//! RPC error types.

use forge_index_core::ForgeError;

/// Errors that can occur during RPC operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RpcError {
    /// A transport-level error (network, HTTP status, etc.).
    #[error("Transport error: {0}")]
    Transport(String),

    /// The rate limiter rejected the request.
    #[error("Rate limited on chain {chain_id}")]
    RateLimit {
        /// The chain ID that was rate-limited.
        chain_id: u64,
    },

    /// The RPC call timed out.
    #[error("Timeout calling {method}")]
    Timeout {
        /// The RPC method that timed out.
        method: String,
    },

    /// Failed to decode the RPC response.
    #[error("Decode error in {method}: {message}")]
    Decode {
        /// The RPC method.
        method: String,
        /// Details about the decode failure.
        message: String,
    },

    /// WebSocket transport is not available.
    #[error("No WebSocket transport for chain {chain_id}")]
    NoWebSocket {
        /// The chain ID.
        chain_id: u64,
    },

    /// All retry attempts were exhausted.
    #[error("Max retries exceeded for {method} after {attempts} attempts")]
    MaxRetriesExceeded {
        /// The RPC method.
        method: String,
        /// Total number of attempts made.
        attempts: u32,
    },
}

impl RpcError {
    /// Returns `true` if this error should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport(_) | Self::RateLimit { .. } | Self::Timeout { .. }
        )
    }
}

impl From<RpcError> for ForgeError {
    fn from(e: RpcError) -> Self {
        let chain_id = match &e {
            RpcError::RateLimit { chain_id } | RpcError::NoWebSocket { chain_id } => *chain_id,
            _ => 0,
        };
        ForgeError::Rpc {
            chain_id,
            message: e.to_string(),
        }
    }
}
