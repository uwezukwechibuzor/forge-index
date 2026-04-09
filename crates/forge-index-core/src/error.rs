//! Top-level error type for the forge-index framework.

/// The unified error type used across all forge-index crates.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ForgeError {
    /// An RPC request failed for a specific chain.
    #[error("RPC error on chain {chain_id}: {message}")]
    Rpc {
        /// The chain ID where the RPC error occurred.
        chain_id: u64,
        /// A human-readable error message.
        message: String,
    },

    /// A database operation failed.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// ABI decoding failed.
    #[error("ABI decode error: {message}")]
    AbiDecode {
        /// A human-readable error message.
        message: String,
    },

    /// A configuration error.
    #[error("Config error: {0}")]
    Config(String),

    /// The schema is locked by another process.
    #[error("Schema locked: {schema}")]
    SchemaLocked {
        /// The name of the locked schema.
        schema: String,
    },

    /// A chain reorganization was detected.
    #[error("Reorg detected on chain {chain_id} at block {block_number}")]
    Reorg {
        /// The chain ID where the reorg occurred.
        chain_id: u64,
        /// The block number at which the reorg was detected.
        block_number: u64,
    },

    /// An event handler failed.
    #[error("Handler '{handler}' failed: {source}")]
    Handler {
        /// The name of the handler that failed.
        handler: String,
        /// The underlying error.
        source: anyhow::Error,
    },

    /// An I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_error_displays_human_readable_messages() {
        let rpc = ForgeError::Rpc {
            chain_id: 1,
            message: "timeout".into(),
        };
        assert_eq!(rpc.to_string(), "RPC error on chain 1: timeout");

        let abi = ForgeError::AbiDecode {
            message: "invalid selector".into(),
        };
        assert_eq!(abi.to_string(), "ABI decode error: invalid selector");

        let config = ForgeError::Config("missing field".into());
        assert_eq!(config.to_string(), "Config error: missing field");

        let locked = ForgeError::SchemaLocked {
            schema: "public".into(),
        };
        assert_eq!(locked.to_string(), "Schema locked: public");

        let reorg = ForgeError::Reorg {
            chain_id: 1,
            block_number: 100,
        };
        assert_eq!(reorg.to_string(), "Reorg detected on chain 1 at block 100");

        let handler = ForgeError::Handler {
            handler: "onTransfer".into(),
            source: anyhow::anyhow!("panic"),
        };
        assert_eq!(handler.to_string(), "Handler 'onTransfer' failed: panic");

        let io = ForgeError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert_eq!(io.to_string(), "I/O error: gone");
    }
}
