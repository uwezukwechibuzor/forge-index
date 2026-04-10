//! POST /sql — SQL-over-HTTP endpoint.

use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::response::ApiResponse;
use crate::sql::{self, SqlError};

/// Shared state for the SQL endpoint.
#[derive(Clone)]
pub struct SqlState {
    pub pool: sqlx::PgPool,
    pub pg_schema: String,
    pub rate_limiter: Arc<SqlRateLimiter>,
    /// If set, requires Bearer token auth (prod mode).
    pub api_key: Option<String>,
    /// Whether we're in prod mode.
    pub prod_mode: bool,
}

/// Request body for POST /sql.
#[derive(Debug, Deserialize)]
pub struct SqlRequest {
    pub query: String,
    pub timeout_ms: Option<u64>,
}

/// POST /sql handler.
pub async fn sql_handler(
    State(state): State<SqlState>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<SqlRequest>,
) -> Response {
    // 1. Auth check in prod mode
    if state.prod_mode {
        if let Some(ref expected_key) = state.api_key {
            let provided = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));

            match provided {
                Some(token) if token == expected_key => {}
                _ => {
                    return ApiResponse::<()>::error(
                        StatusCode::UNAUTHORIZED,
                        "UNAUTHORIZED",
                        "Invalid or missing Authorization header",
                    )
                    .into_response();
                }
            }
        } else {
            tracing::warn!("FORGE_API_KEY not set in prod mode — allowing unauthenticated request");
        }
    }

    // 2. Rate limiting
    let ip = connect_info
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    if !state.rate_limiter.try_acquire(ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", "1")],
            Json(ApiResponse::<()> {
                data: None,
                meta: None,
                error: Some(crate::response::ApiErrorBody {
                    code: "RATE_LIMITED".to_string(),
                    message: "Too many requests. Max 10 requests per second.".to_string(),
                }),
            }),
        )
            .into_response();
    }

    // 3. Validate SQL
    let validated = match sql::validate_sql(&req.query, &state.pg_schema) {
        Ok(v) => v,
        Err(e) => {
            let (status, code) = match &e {
                SqlError::InvalidStatement(_) => (StatusCode::BAD_REQUEST, "INVALID_SQL"),
                SqlError::ForbiddenKeyword(_) => (StatusCode::BAD_REQUEST, "FORBIDDEN_KEYWORD"),
                SqlError::TooLong { .. } => (StatusCode::BAD_REQUEST, "QUERY_TOO_LONG"),
                SqlError::Timeout => (StatusCode::REQUEST_TIMEOUT, "TIMEOUT"),
                SqlError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            };
            return ApiResponse::<()>::error(status, code, &e.to_string()).into_response();
        }
    };

    // 4. Execute
    match sql::execute_sql(&state.pool, &validated, req.timeout_ms).await {
        Ok(result) => {
            let meta = serde_json::json!({
                "execution_time_ms": result.execution_time_ms,
                "row_count": result.row_count,
            });
            ApiResponse::ok_with_meta(
                serde_json::json!({
                    "rows": result.rows,
                    "columns": result.columns,
                }),
                meta,
            )
            .into_response()
        }
        Err(SqlError::Timeout) => ApiResponse::<()>::error(
            StatusCode::REQUEST_TIMEOUT,
            "TIMEOUT",
            "Query execution timed out",
        )
        .into_response(),
        Err(e) => ApiResponse::<()>::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        )
        .into_response(),
    }
}

// ── Rate limiter ──────────────────────────────────────────────────────

use dashmap::DashMap;
use tokio::time::Instant;

/// Per-IP token bucket rate limiter for the SQL endpoint.
pub struct SqlRateLimiter {
    buckets: DashMap<IpAddr, TokenBucket>,
    max_rps: u32,
}

struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_rps: u32) -> Self {
        Self {
            tokens: max_rps as f64,
            max_tokens: max_rps as f64,
            refill_rate: max_rps as f64,
            last_refill: Instant::now(),
        }
    }

    fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = (now - self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl SqlRateLimiter {
    /// Creates a new rate limiter with the given max requests per second per IP.
    pub fn new(max_rps: u32) -> Self {
        Self {
            buckets: DashMap::new(),
            max_rps,
        }
    }

    /// Tries to acquire a token for the given IP. Returns false if rate limited.
    pub fn try_acquire(&self, ip: IpAddr) -> bool {
        let mut entry = self
            .buckets
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(self.max_rps));
        entry.try_acquire()
    }
}
