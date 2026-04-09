//! Standard JSON response envelope types.

use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

/// Standard API response envelope.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    /// The response data, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Optional metadata (pagination, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    /// Error details, if this is an error response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

/// Error details within an API response.
#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    /// Machine-readable error code (e.g. "NOT_FOUND").
    pub code: String,
    /// Human-readable error message.
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a 200 OK response with data.
    pub fn ok(data: T) -> (StatusCode, Json<Self>) {
        (
            StatusCode::OK,
            Json(Self {
                data: Some(data),
                meta: None,
                error: None,
            }),
        )
    }

    /// Creates a 200 OK response with data and metadata.
    pub fn ok_with_meta(data: T, meta: serde_json::Value) -> (StatusCode, Json<Self>) {
        (
            StatusCode::OK,
            Json(Self {
                data: Some(data),
                meta: Some(meta),
                error: None,
            }),
        )
    }
}

impl ApiResponse<()> {
    /// Creates an error response with the given status code.
    pub fn error(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<Self>) {
        (
            status,
            Json(Self {
                data: None,
                meta: None,
                error: Some(ApiErrorBody {
                    code: code.to_string(),
                    message: message.to_string(),
                }),
            }),
        )
    }
}
