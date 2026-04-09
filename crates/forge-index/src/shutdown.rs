//! Graceful shutdown signal handling.

/// Waits for SIGINT (Ctrl+C) or SIGTERM (Docker stop).
///
/// Returns the name of the signal received for logging.
pub async fn shutdown_signal() -> &'static str {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        "SIGINT"
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        "SIGTERM"
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<&'static str>();

    tokio::select! {
        sig = ctrl_c => sig,
        sig = terminate => sig,
    }
}
