//! RPC client abstraction for the forge-index EVM indexing framework.
//!
//! Provides an `RpcClient` with retry logic, rate limiting, request deduplication,
//! and both HTTP and WebSocket support over alloy-rs providers.

mod client;
mod dedup;
mod error;
mod rate_limiter;
mod retry;
mod transport;
mod types;

pub use client::RpcClient;
pub use error::RpcError;
pub use transport::{build_from_config, build_http_provider};
pub use types::{LogFilter, TransactionReceipt};
