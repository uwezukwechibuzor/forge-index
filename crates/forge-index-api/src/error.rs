//! API error types with automatic JSON response conversion.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::response::{ApiErrorBody, ApiResponse};

/// Errors returned from API handlers.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// An internal server error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// The requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// The request was malformed.
    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
        };

        let body = ApiResponse::<()> {
            data: None,
            meta: None,
            error: Some(ApiErrorBody {
                code: code.to_string(),
                message,
            }),
        };

        (status, Json(body)).into_response()
    }
}
