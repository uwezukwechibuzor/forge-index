//! Prometheus metric definitions and update helpers.
//!
//! All forge_ metrics are defined here. Other crates call these helpers
//! to update metrics without directly depending on metrics-exporter-prometheus.

use std::time::Duration;

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

/// Records an RPC request with status and duration.
pub fn record_rpc_request(chain_id: u64, method: &str, success: bool, duration: Duration) {
    let status = if success { "success" } else { "error" };
    metrics::counter!(
        "forge_rpc_requests_total",
        "chain_id" => chain_id.to_string(),
        "method" => method.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
    metrics::histogram!(
        "forge_rpc_request_duration_seconds",
        "chain_id" => chain_id.to_string(),
        "method" => method.to_string()
    )
    .record(duration.as_secs_f64());
}

/// Sets the indexer lag in blocks.
pub fn set_indexer_lag(chain_id: u64, lag_blocks: u64) {
    metrics::gauge!(
        "forge_indexer_lag_blocks",
        "chain_id" => chain_id.to_string()
    )
    .set(lag_blocks as f64);
}

/// Sets the write buffer size for a table.
pub fn set_buffer_size(table: &str, size: usize) {
    metrics::gauge!(
        "forge_write_buffer_size",
        "table" => table.to_string()
    )
    .set(size as f64);
}

/// Records a DB flush duration.
pub fn record_db_flush(duration: Duration) {
    metrics::histogram!("forge_db_flush_duration_seconds").record(duration.as_secs_f64());
}

/// Sets the backfill progress as a percentage (0.0 to 100.0).
pub fn set_backfill_progress(chain_id: u64, progress: f32) {
    metrics::gauge!(
        "forge_backfill_progress",
        "chain_id" => chain_id.to_string()
    )
    .set(progress as f64);
}

/// Records an HTTP request duration.
pub fn record_http_request(method: &str, path: &str, status: u16, duration: Duration) {
    metrics::histogram!(
        "forge_http_request_duration_seconds",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string()
    )
    .record(duration.as_secs_f64());
}

/// Installs the Prometheus metrics recorder and returns the handle.
///
/// Call once at application startup before any metrics are recorded.
pub fn install_metrics_recorder() -> metrics_exporter_prometheus::PrometheusHandle {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder).expect("failed to install metrics recorder");
    handle
}

/// Builds a recorder for testing (does not set as global).
///
/// Returns both the recorder and its handle.
pub fn build_test_recorder() -> (
    metrics_exporter_prometheus::PrometheusRecorder,
    metrics_exporter_prometheus::PrometheusHandle,
) {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    (recorder, handle)
}
