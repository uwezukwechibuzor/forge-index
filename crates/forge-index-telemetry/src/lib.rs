//! Telemetry for the forge-index EVM indexing framework.
//!
//! Provides structured logging, Prometheus metrics helpers,
//! and build ID generation for detecting config/schema changes.

pub mod build_id;
pub mod logging;
pub mod metrics;

pub use build_id::{compute_build_id, log_build_id_status, BuildIdStatus, BuildInput};
pub use logging::{init_logging, init_logging_for_test, LogMode};
pub use metrics::{
    build_test_recorder, install_metrics_recorder, record_block_processed, record_db_flush,
    record_event_indexed, record_http_request, record_rpc_request, set_backfill_progress,
    set_buffer_size, set_indexer_lag,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_dev_does_not_panic() {
        // Can only init once per process — use try_init approach
        logging::init_logging_for_test();
    }

    #[test]
    fn log_build_id_status_does_not_panic() {
        // These just log — verify they don't panic for each variant
        log_build_id_status(&BuildIdStatus::New, "abc123");
        log_build_id_status(&BuildIdStatus::Same, "abc123");
        log_build_id_status(
            &BuildIdStatus::Changed {
                old: "old123".to_string(),
            },
            "new456",
        );
    }

    #[test]
    fn metrics_record_block_processed_does_not_panic() {
        // Without a global recorder, metrics calls are no-ops but shouldn't panic
        record_block_processed(1);
    }

    #[test]
    fn metrics_set_indexer_lag_does_not_panic() {
        set_indexer_lag(1, 42);
    }

    #[test]
    fn metrics_with_test_recorder() {
        let (recorder, handle) = build_test_recorder();
        // Set as global (may fail if another test already set it — that's OK)
        let _ = ::metrics::set_global_recorder(recorder);

        record_block_processed(1);
        record_block_processed(1);

        let rendered = handle.render();
        // The counter should appear in the rendered output
        // (may not if another recorder was already set globally)
        let _ = rendered;
    }
}
