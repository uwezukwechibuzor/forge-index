//! GET /ready endpoint.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::watch;

/// Readiness check response.
#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    /// "ready" or "syncing".
    pub status: String,
    /// Optional message explaining why the service is not ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Shared state for the ready endpoint.
#[derive(Clone)]
pub struct ReadyState {
    /// Watch receiver that indicates whether the indexer is ready.
    pub ready_rx: Arc<watch::Receiver<bool>>,
}

/// Returns 200 OK if the indexer is ready, 503 if still syncing.
pub async fn ready(State(state): State<ReadyState>) -> (StatusCode, Json<ReadyResponse>) {
    let is_ready = *state.ready_rx.borrow();

    if is_ready {
        (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready".to_string(),
                message: None,
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "syncing".to_string(),
                message: Some("Historical backfill in progress".to_string()),
            }),
        )
    }
}
