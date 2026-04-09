//! GET /health endpoint.

use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Always "ok".
    pub status: String,
    /// Current Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Returns 200 OK immediately — indicates the server process is alive.
pub async fn health() -> (StatusCode, Json<HealthResponse>) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
            timestamp,
        }),
    )
}
