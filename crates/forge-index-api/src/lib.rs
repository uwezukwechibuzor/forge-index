//! HTTP API server for the forge-index EVM indexing framework.
//!
//! Provides health, readiness, Prometheus metrics endpoints,
//! auto-generated GraphQL API, request tracing, and CORS middleware.

pub mod error;
pub mod graphql;
pub mod handlers;
pub mod middleware;
pub mod response;
pub mod server;

pub use error::ApiError;
pub use graphql::{GraphqlSchema, GraphqlState};
pub use handlers::metrics::{
    record_block_processed, record_event_indexed, record_flush_duration, record_http_request,
    record_rpc_duration, record_rpc_request, update_backfill_progress, update_buffer_size,
    update_lag,
};
pub use response::{ApiErrorBody, ApiResponse};
pub use server::{install_metrics_recorder, ApiServer};

#[cfg(test)]
mod tests;
