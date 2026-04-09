//! GET /metrics endpoint — Prometheus exposition format.
//!
//! Also provides helper functions for other crates to update metrics
//! without importing metrics-exporter-prometheus directly.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use metrics_exporter_prometheus::PrometheusHandle;

/// Returns Prometheus metrics in text/plain exposition format.
pub async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    let body = handle.render();
    (
        StatusCode::OK,
        [("content-type", "text/plain; charset=utf-8")],
        body,
    )
}

// ── Metric update helpers ────────────────────────────────────────────

/// Records a processed block.
pub fn record_block_processed(chain_id: u64) {
    metrics::counter!("forge_blocks_processed_total", "chain_id" => chain_id.to_string())
        .increment(1);
}

/// Records an indexed event.
pub fn record_event_indexed(chain_id: u64, contract: &str, event: &str) {
    metrics::counter!(
        "forge_events_indexed_total",
        "chain_id" => chain_id.to_string(),
        "contract" => contract.to_string(),
        "event" => event.to_string()
    )
    .increment(1);
}

/// Records an RPC request.
pub fn record_rpc_request(chain_id: u64, method: &str, success: bool) {
    let status = if success { "success" } else { "error" };
    metrics::counter!(
        "forge_rpc_requests_total",
        "chain_id" => chain_id.to_string(),
        "method" => method.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

/// Records RPC request duration in seconds.
pub fn record_rpc_duration(chain_id: u64, method: &str, duration_secs: f64) {
    metrics::histogram!(
        "forge_rpc_request_duration_seconds",
        "chain_id" => chain_id.to_string(),
        "method" => method.to_string()
    )
    .record(duration_secs);
}

/// Updates the indexer lag in blocks.
pub fn update_lag(chain_id: u64, lag: u64) {
    metrics::gauge!(
        "forge_indexer_lag_blocks",
        "chain_id" => chain_id.to_string()
    )
    .set(lag as f64);
}

/// Updates the write buffer size for a table.
pub fn update_buffer_size(table: &str, size: u64) {
    metrics::gauge!(
        "forge_write_buffer_size",
        "table" => table.to_string()
    )
    .set(size as f64);
}

/// Records DB flush duration in seconds.
pub fn record_flush_duration(duration_secs: f64) {
    metrics::histogram!("forge_db_flush_duration_seconds").record(duration_secs);
}

/// Updates backfill progress as a fraction (0.0 to 1.0).
pub fn update_backfill_progress(chain_id: u64, progress: f64) {
    metrics::gauge!(
        "forge_backfill_progress",
        "chain_id" => chain_id.to_string()
    )
    .set(progress);
}

/// Records HTTP request duration.
pub fn record_http_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    metrics::histogram!(
        "forge_http_request_duration_seconds",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string()
    )
    .record(duration_secs);
}
