//! CORS middleware configuration.

use http::Method;
use tower_http::cors::{Any, CorsLayer};

/// Creates a CORS layer that allows common methods and headers.
///
/// In development mode, allows all origins. In production, this should
/// be restricted to specific origins.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
        .allow_origin(Any)
}
