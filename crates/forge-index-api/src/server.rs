//! ApiServer — axum router setup and startup.

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::sync::watch;

use crate::error::ApiError;
use crate::handlers;
use crate::handlers::ready::ReadyState;
use crate::handlers::schema_info::SchemaInfoState;
use crate::handlers::sql::{SqlRateLimiter, SqlState};
use crate::middleware;

/// The main HTTP API server.
pub struct ApiServer {
    /// The port to listen on.
    port: u16,
    /// Watch receiver for indexer readiness state.
    ready_rx: watch::Receiver<bool>,
    /// Prometheus metrics handle for rendering.
    metrics_handle: PrometheusHandle,
    /// Database pool (optional — needed for /sql, /schema, /graphql).
    db_pool: Option<sqlx::PgPool>,
    /// Postgres schema name.
    pg_schema: String,
    /// User-defined data schema (optional — needed for /graphql).
    data_schema: Option<forge_index_config::Schema>,
}

impl ApiServer {
    /// Creates a new API server.
    pub fn new(
        port: u16,
        ready_rx: watch::Receiver<bool>,
        metrics_handle: PrometheusHandle,
    ) -> Self {
        Self {
            port,
            ready_rx,
            metrics_handle,
            db_pool: None,
            pg_schema: "public".to_string(),
            data_schema: None,
        }
    }

    /// Sets the database pool for SQL and GraphQL endpoints.
    pub fn with_db(mut self, pool: sqlx::PgPool, pg_schema: String) -> Self {
        self.db_pool = Some(pool);
        self.pg_schema = pg_schema;
        self
    }

    /// Sets the data schema for GraphQL auto-generation.
    pub fn with_schema(mut self, schema: forge_index_config::Schema) -> Self {
        self.data_schema = Some(schema);
        self
    }

    /// Builds the axum Router with all routes and middleware.
    pub fn router(&self) -> Router {
        let ready_state = ReadyState {
            ready_rx: Arc::new(self.ready_rx.clone()),
        };

        let metrics_handle = self.metrics_handle.clone();

        let mut router = Router::new()
            .route("/health", get(handlers::health::health))
            .route(
                "/ready",
                get(handlers::ready::ready).with_state(ready_state),
            )
            .route(
                "/metrics",
                get(handlers::metrics::metrics_handler).with_state(metrics_handle),
            );

        // Add /sql and /schema routes if a database pool is available
        if let Some(ref pool) = self.db_pool {
            let prod_mode = std::env::var("FORGE_ENV")
                .map(|v| v == "prod")
                .unwrap_or(false);
            let api_key = std::env::var("FORGE_API_KEY").ok();

            if prod_mode && api_key.is_none() {
                tracing::warn!(
                    "FORGE_ENV=prod but FORGE_API_KEY not set — /sql endpoint will allow unauthenticated access"
                );
            }

            let sql_state = SqlState {
                pool: pool.clone(),
                pg_schema: self.pg_schema.clone(),
                rate_limiter: Arc::new(SqlRateLimiter::new(10)),
                api_key,
                prod_mode,
            };

            let schema_state = SchemaInfoState {
                pool: pool.clone(),
                pg_schema: self.pg_schema.clone(),
            };

            router = router
                .route(
                    "/sql",
                    post(handlers::sql::sql_handler).with_state(sql_state),
                )
                .route(
                    "/schema",
                    get(handlers::schema_info::schema_info_handler).with_state(schema_state),
                );

            // Wire GraphQL if a data schema is provided
            if let Some(ref data_schema) = self.data_schema {
                match crate::graphql::schema_gen::GraphqlSchema::build(
                    data_schema,
                    pool.clone(),
                    self.pg_schema.clone(),
                ) {
                    Ok(gql_schema) => {
                        let gql_state = crate::graphql::handler::GraphqlState {
                            schema: std::sync::Arc::new(gql_schema),
                        };
                        router = router.route(
                            "/graphql",
                            get(crate::graphql::handler::graphql_playground)
                                .post(crate::graphql::handler::graphql_handler)
                                .with_state(gql_state),
                        );
                        tracing::info!("GraphQL endpoint enabled at /graphql");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build GraphQL schema — /graphql will not be available");
                    }
                }
            }
        }

        router
            .layer(axum::middleware::from_fn(
                middleware::tracing::request_tracing,
            ))
            .layer(middleware::cors::cors_layer())
    }

    /// Starts the HTTP server and listens for connections.
    ///
    /// Shuts down gracefully on SIGINT or SIGTERM.
    pub async fn run(self) -> Result<(), ApiError> {
        let router = self.router();
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to bind to {}: {}", addr, e)))?;

        tracing::info!("HTTP server listening on {}", addr);

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| ApiError::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Waits for a shutdown signal (SIGINT or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}

/// Installs the Prometheus metrics recorder and returns the handle.
///
/// Call this once at application startup before any metrics are recorded.
pub fn install_metrics_recorder() -> PrometheusHandle {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder).expect("failed to install metrics recorder");
    handle
}
