//! Tests for the API server.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http::header;
use tower::util::ServiceExt;

use crate::response::ApiResponse;
use crate::server::ApiServer;

fn build_test_server(ready: bool) -> axum::Router {
    let (_tx, rx) = tokio::sync::watch::channel(ready);

    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    // Don't set as global recorder in tests — multiple tests would conflict.
    // The handle still works for rendering.
    let _ = recorder;

    let server = ApiServer::new(0, rx, handle);
    server.router()
}

#[tokio::test]
async fn health_returns_200_with_correct_json() {
    let app = build_test_server(false);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert!(json["timestamp"].is_number());
}

#[tokio::test]
async fn ready_returns_503_when_not_ready() {
    let app = build_test_server(false);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "syncing");
    assert!(json["message"].as_str().unwrap().contains("backfill"));
}

#[tokio::test]
async fn ready_returns_200_when_ready() {
    let app = build_test_server(true);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ready");
}

#[tokio::test]
async fn metrics_returns_200_with_text_plain() {
    let app = build_test_server(false);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/plain"),
        "expected text/plain, got: {}",
        content_type
    );
}

#[tokio::test]
async fn metrics_renders_without_error() {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    let rendered = handle.render();
    // Just verify render() doesn't panic — content may be empty initially
    let _ = rendered;
}

#[test]
fn api_response_ok_serializes_correctly() {
    let (status, json) = ApiResponse::ok("hello");
    assert_eq!(status, StatusCode::OK);

    let value = serde_json::to_value(&json.0).unwrap();
    assert_eq!(value["data"], "hello");
    assert!(value.get("error").is_none());
}

#[test]
fn api_response_error_serializes_correctly() {
    let (status, json) =
        ApiResponse::<()>::error(StatusCode::BAD_REQUEST, "BAD_REQUEST", "invalid input");
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let value = serde_json::to_value(&json.0).unwrap();
    assert!(value.get("data").is_none());
    assert_eq!(value["error"]["code"], "BAD_REQUEST");
    assert_eq!(value["error"]["message"], "invalid input");
}

#[tokio::test]
async fn api_error_internal_produces_500() {
    use crate::error::ApiError;
    use axum::response::IntoResponse;

    let err = ApiError::Internal("something broke".to_string());
    let response = err.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"]["code"], "INTERNAL");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("something broke"));
}

#[tokio::test]
async fn cors_headers_present() {
    let app = build_test_server(false);

    let response: axum::response::Response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header("Origin", "http://example.com")
                .header("Access-Control-Request-Method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        response.status().is_success(),
        "preflight should succeed, got {}",
        response.status()
    );

    let acl = response.headers().get("access-control-allow-origin");
    assert!(
        acl.is_some(),
        "access-control-allow-origin header should be present"
    );
}
