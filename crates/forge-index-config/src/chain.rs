//! Chain and transport configuration types.

use serde::{Deserialize, Serialize};

/// Configuration for a single EVM chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Human-readable chain name (e.g., "mainnet", "arbitrum").
    pub name: String,
    /// The numeric chain ID.
    pub chain_id: u64,
    /// HTTP RPC endpoint URL.
    pub rpc_http: String,
    /// Optional WebSocket RPC endpoint URL.
    pub rpc_ws: Option<String>,
    /// Optional rate limit for RPC requests per second.
    pub max_rpc_requests_per_second: Option<u32>,
    /// Optional poll interval in milliseconds for new blocks.
    pub poll_interval_ms: Option<u64>,
}

/// Transport configuration for connecting to an RPC provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportConfig {
    /// HTTP-only transport.
    Http(String),
    /// WebSocket-only transport.
    WebSocket(String),
    /// Both HTTP and WebSocket transports.
    Both {
        /// HTTP endpoint URL.
        http: String,
        /// WebSocket endpoint URL.
        ws: String,
    },
}
