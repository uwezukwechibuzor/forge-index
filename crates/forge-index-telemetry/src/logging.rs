//! Structured logging setup for dev and prod modes.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Logging output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogMode {
    /// Human-readable pretty output with file/line info.
    Dev,
    /// Machine-readable JSON output for production log aggregation.
    Prod,
}

/// Initializes the global tracing subscriber.
///
/// The `level` parameter sets the default log level (e.g. "info", "debug").
/// It can be overridden by the `FORGE_LOG_LEVEL` environment variable.
///
/// Noisy dependencies (h2, hyper, tower_http) are suppressed below WARN.
pub fn init_logging(mode: LogMode, level: &str) {
    let env_level = std::env::var("FORGE_LOG_LEVEL").unwrap_or_else(|_| level.to_string());

    let filter = EnvFilter::try_new(format!("{},h2=warn,hyper=warn,tower_http=warn", env_level))
        .unwrap_or_else(|_| EnvFilter::new("info,h2=warn,hyper=warn,tower_http=warn"));

    match mode {
        LogMode::Dev => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true)
                        .pretty(),
                )
                .init();
        }
        LogMode::Prod => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_current_span(true)
                        .with_span_list(true),
                )
                .init();
        }
    }
}

/// Initializes logging without setting a global subscriber.
///
/// Returns a guard — useful for tests that need isolated logging.
pub fn init_logging_for_test() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug,h2=warn,hyper=warn")
        .with_test_writer()
        .try_init();
}
