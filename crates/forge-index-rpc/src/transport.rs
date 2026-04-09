//! Provider construction from transport configuration.

use crate::client::RpcClient;
use crate::dedup::RequestDedup;
use crate::error::RpcError;
use crate::rate_limiter::RateLimiter;
use forge_index_config::ChainConfig;
use std::sync::Arc;

/// Builds an HTTP-only alloy provider from a URL.
pub fn build_http_provider(url: &str) -> Result<alloy::providers::ReqwestProvider, RpcError> {
    let parsed: url::Url = url
        .parse()
        .map_err(|e: url::ParseError| RpcError::Transport(e.to_string()))?;
    Ok(alloy::providers::ReqwestProvider::new_http(parsed))
}

/// Builds an `RpcClient` from a chain configuration.
///
/// Uses HTTP transport. WebSocket subscriptions are available only if
/// `rpc_ws` is configured.
pub fn build_from_config(config: &ChainConfig) -> Result<RpcClient, RpcError> {
    let provider = build_http_provider(&config.rpc_http)?;
    let max_rps = config.max_rpc_requests_per_second.unwrap_or(25);

    Ok(RpcClient {
        provider: Arc::new(provider),
        chain_id: config.chain_id,
        rate_limiter: Arc::new(RateLimiter::new(max_rps)),
        dedup: Arc::new(RequestDedup::new()),
        has_ws: config.rpc_ws.is_some(),
    })
}
