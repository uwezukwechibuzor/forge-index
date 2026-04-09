//! Request tracing middleware — logs method, path, status, and latency.

use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

/// Tower middleware that logs every HTTP request with timing information.
///
/// Logs at INFO for 2xx/3xx, WARN for 4xx, ERROR for 5xx.
/// Also records `forge_http_request_duration_seconds` histogram.
pub async fn request_tracing(request: Request<Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    let latency = start.elapsed();
    let latency_ms = latency.as_secs_f64() * 1000.0;

    // Record histogram
    crate::handlers::metrics::record_http_request(&method, &path, status, latency.as_secs_f64());

    // Log at appropriate level
    if status >= 500 {
        tracing::error!(
            method = %method,
            path = %path,
            status = status,
            latency_ms = format!("{:.1}", latency_ms),
            "HTTP {} {} → {} in {:.1}ms",
            method, path, status, latency_ms
        );
    } else if status >= 400 {
        tracing::warn!(
            method = %method,
            path = %path,
            status = status,
            latency_ms = format!("{:.1}", latency_ms),
            "HTTP {} {} → {} in {:.1}ms",
            method, path, status, latency_ms
        );
    } else {
        tracing::info!(
            method = %method,
            path = %path,
            status = status,
            latency_ms = format!("{:.1}", latency_ms),
            "HTTP {} {} → {} in {:.1}ms",
            method, path, status, latency_ms
        );
    }

    response
}
