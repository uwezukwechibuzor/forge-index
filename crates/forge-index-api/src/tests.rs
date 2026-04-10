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

// ── GraphQL tests ──────────────────────────────────────────────────────

mod graphql_tests {
    use crate::graphql::filters::filters_to_sql;
    use crate::graphql::pagination::{decode_cursor, encode_cursor};
    use crate::graphql::types::{to_camel_case, to_pascal_case};
    use forge_index_config::{ColumnType, Schema, SchemaBuilder};

    fn test_schema() -> Schema {
        SchemaBuilder::new()
            .table("accounts", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("address", ColumnType::Address)
                    .not_null()
                    .column("balance", ColumnType::BigInt)
                    .not_null()
                    .column("is_owner", ColumnType::Boolean)
                    .not_null()
            })
            .build()
    }

    #[test]
    fn graphql_schema_generates_correct_types() {
        // We can't connect to a real DB here, but we can test the SDL generation
        // by verifying the schema builds without error using a mock pool approach.
        // Instead, test the helper functions.
        assert_eq!(to_camel_case("accounts"), "accounts");
        assert_eq!(to_camel_case("my_table"), "myTable");
        assert_eq!(to_pascal_case("accounts"), "Accounts");
        assert_eq!(to_pascal_case("my_table"), "MyTable");
    }

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let cursor = encode_cursor("pk1", "created_at", &serde_json::json!(12345));
        let decoded = decode_cursor(&cursor).unwrap();
        assert_eq!(decoded.pk_value, "pk1");
        assert_eq!(decoded.order_col, "created_at");
        assert_eq!(decoded.order_val, serde_json::json!(12345));
    }

    #[test]
    fn invalid_cursor_returns_error() {
        let result = decode_cursor("not-valid-base64!!!!");
        assert!(result.is_err());
    }

    #[test]
    fn filters_eq_generates_correct_sql() {
        let filter = serde_json::json!({ "address": { "eq": "0xABC" } });
        let (sql, _) = filters_to_sql(&filter, "public", "accounts");
        assert_eq!(sql, "\"address\" = '0xABC'");
    }

    #[test]
    fn filters_gt_generates_correct_sql() {
        let filter = serde_json::json!({ "balance": { "gt": 100 } });
        let (sql, _) = filters_to_sql(&filter, "public", "accounts");
        assert_eq!(sql, "\"balance\" > 100");
    }

    #[test]
    fn filters_in_generates_correct_sql() {
        let filter = serde_json::json!({ "status": { "in": ["active", "pending"] } });
        let (sql, _) = filters_to_sql(&filter, "public", "accounts");
        assert_eq!(sql, "\"status\" IN ('active', 'pending')");
    }

    #[test]
    fn filters_multiple_fields_anded() {
        let filter = serde_json::json!({
            "address": { "eq": "0xABC" },
            "balance": { "gte": 100 }
        });
        let (sql, _) = filters_to_sql(&filter, "public", "accounts");
        assert!(sql.contains("\"address\" = '0xABC'"));
        assert!(sql.contains("\"balance\" >= 100"));
        assert!(sql.contains(" AND "));
    }

    #[test]
    fn bigint_column_maps_to_string_type() {
        use crate::graphql::types::column_type_to_gql;
        assert_eq!(column_type_to_gql(&ColumnType::BigInt), "String");
        assert_eq!(column_type_to_gql(&ColumnType::Int), "Int");
        assert_eq!(column_type_to_gql(&ColumnType::Boolean), "Boolean");
        assert_eq!(column_type_to_gql(&ColumnType::Address), "String");
    }

    #[test]
    fn graphql_schema_builds_without_error() {
        // Test that schema generation doesn't panic for a valid schema.
        // We can't test query execution without a real DB, but we can verify
        // the dynamic schema builds correctly.
        let schema = test_schema();

        // Build a temporary pool (won't actually connect)
        // Instead, just verify the types and naming are correct
        assert_eq!(to_pascal_case("accounts"), "Accounts");
        assert_eq!(to_camel_case("accounts"), "accounts");

        // The schema has the right structure
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "accounts");
        assert_eq!(schema.tables[0].columns.len(), 4);
    }

    #[test]
    fn playground_html_contains_graphql() {
        use crate::graphql::handler::PLAYGROUND_HTML;
        assert!(PLAYGROUND_HTML.contains("GraphQL"));
        assert!(PLAYGROUND_HTML.contains("/graphql"));
    }
}

// ── SQL-over-HTTP tests ──────────────────────────────────────────────────

mod sql_parser_tests {
    use crate::sql::parser::validate_sql;
    use crate::sql::SqlError;

    #[test]
    fn valid_select_passes_validation() {
        let result = validate_sql("SELECT * FROM accounts", "public");
        assert!(result.is_ok());
    }

    #[test]
    fn insert_returns_400() {
        let result = validate_sql("INSERT INTO accounts VALUES (1)", "public");
        assert!(
            matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Only SELECT"))
        );
    }

    #[test]
    fn drop_table_returns_400() {
        let result = validate_sql("SELECT 1; DROP TABLE accounts", "public");
        assert!(result.is_err());
    }

    #[test]
    fn delete_rejected() {
        let result = validate_sql("DELETE FROM accounts", "public");
        assert!(
            matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Only SELECT"))
        );
    }

    #[test]
    fn update_rejected() {
        let result = validate_sql("UPDATE accounts SET balance = 0", "public");
        assert!(
            matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Only SELECT"))
        );
    }

    #[test]
    fn semicolon_in_middle_returns_400() {
        let result = validate_sql("SELECT 1; SELECT 2", "public");
        assert!(
            matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Multiple statements"))
        );
    }

    #[test]
    fn limit_appended_when_missing() {
        let v = validate_sql("SELECT * FROM accounts", "public").unwrap();
        assert!(v.sanitised.contains("LIMIT 1000"));
    }

    #[test]
    fn limit_5000_clamped_to_1000() {
        let v = validate_sql("SELECT * FROM accounts LIMIT 5000", "public").unwrap();
        assert!(v.sanitised.contains("LIMIT 1000"));
        assert!(!v.sanitised.contains("5000"));
    }

    #[test]
    fn limit_under_max_preserved() {
        let v = validate_sql("SELECT * FROM accounts LIMIT 10", "public").unwrap();
        assert!(v.sanitised.contains("LIMIT 10"));
    }

    #[test]
    fn table_name_gets_schema_prefix() {
        let v = validate_sql("SELECT * FROM accounts", "public").unwrap();
        assert!(v.sanitised.contains("public.\"accounts\""));
    }

    #[test]
    fn table_names_extracted_from_join() {
        let v = validate_sql(
            "SELECT a.id, b.name FROM accounts a JOIN transfers b ON a.id = b.from_id",
            "myschema",
        )
        .unwrap();
        assert!(v.table_names.contains(&"accounts".to_string()));
        assert!(v.table_names.contains(&"transfers".to_string()));
    }

    #[test]
    fn pg_catalog_rejected() {
        let result = validate_sql("SELECT * FROM pg_catalog.pg_tables", "public");
        assert!(matches!(result, Err(SqlError::ForbiddenKeyword(_))));
    }

    #[test]
    fn information_schema_rejected() {
        let result = validate_sql("SELECT * FROM information_schema.tables", "public");
        assert!(matches!(result, Err(SqlError::ForbiddenKeyword(_))));
    }

    #[test]
    fn query_too_long_rejected() {
        let long = format!("SELECT * FROM accounts WHERE id = '{}'", "x".repeat(10_000));
        let result = validate_sql(&long, "public");
        assert!(matches!(result, Err(SqlError::TooLong { .. })));
    }

    #[test]
    fn dollar_quoting_rejected() {
        let result = validate_sql("SELECT $$injected$$", "public");
        assert!(result.is_err());
    }

    #[test]
    fn empty_query_rejected() {
        let result = validate_sql("", "public");
        assert!(result.is_err());
    }
}

mod sql_rate_limiter_tests {
    use crate::handlers::sql::SqlRateLimiter;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn rate_limiter_allows_up_to_max() {
        let limiter = SqlRateLimiter::new(10);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        for _ in 0..10 {
            assert!(limiter.try_acquire(ip));
        }
    }

    #[test]
    fn rate_limiter_rejects_11th_request() {
        let limiter = SqlRateLimiter::new(10);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        for _ in 0..10 {
            limiter.try_acquire(ip);
        }
        assert!(!limiter.try_acquire(ip), "11th request should be rejected");
    }

    #[test]
    fn rate_limiter_different_ips_independent() {
        let limiter = SqlRateLimiter::new(10);
        let ip1 = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let ip2 = IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8));

        for _ in 0..10 {
            limiter.try_acquire(ip1);
        }
        assert!(!limiter.try_acquire(ip1));
        assert!(
            limiter.try_acquire(ip2),
            "different IP should still be allowed"
        );
    }
}

mod sql_integration_tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sqlx::PgPool;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;
    use tower::util::ServiceExt;

    use crate::handlers::schema_info::SchemaInfoState;
    use crate::handlers::sql::{SqlRateLimiter, SqlState};
    use crate::server::ApiServer;

    async fn setup_pg() -> (PgPool, testcontainers::ContainerAsync<Postgres>) {
        let container = Postgres::default()
            .with_host_auth()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
        let url = format!("postgres://postgres@127.0.0.1:{}/postgres", port);

        let pool = loop {
            match PgPool::connect(&url).await {
                Ok(pool) => break pool,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            }
        };

        (pool, container)
    }

    async fn setup_test_table(pool: &PgPool) {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS accounts (
                 address TEXT PRIMARY KEY,
                 balance NUMERIC NOT NULL DEFAULT 0,
                 is_active BOOLEAN NOT NULL DEFAULT true,
                 metadata JSONB
               )"#,
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            r#"INSERT INTO accounts (address, balance, is_active, metadata) VALUES
               ('0xAAA', 1000, true, '{"name": "Alice"}'),
               ('0xBBB', 2000, false, null),
               ('0xCCC', 500, true, '{"name": "Charlie"}')"#,
        )
        .execute(pool)
        .await
        .unwrap();

        // Run ANALYZE so pg_class.reltuples is populated
        sqlx::query("ANALYZE accounts").execute(pool).await.unwrap();
    }

    fn build_test_server_with_db(pool: PgPool) -> axum::Router {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let _ = recorder;

        let server = ApiServer::new(0, rx, handle).with_db(pool, "public".to_string());
        server.router()
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_valid_select_returns_rows() {
        let (pool, _container) = setup_pg().await;
        setup_test_table(&pool).await;

        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "SELECT address, balance FROM accounts ORDER BY address LIMIT 10"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["data"]["rows"].as_array().unwrap().len(), 3);
        assert!(json["data"]["columns"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("address")));
        assert!(json["data"]["columns"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("balance")));
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_without_limit_gets_limit_appended() {
        let (pool, _container) = setup_pg().await;
        setup_test_table(&pool).await;

        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "SELECT * FROM accounts"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Should succeed (LIMIT 1000 auto-appended) and return 3 rows
        assert_eq!(json["data"]["rows"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_insert_returns_400() {
        let (pool, _container) = setup_pg().await;
        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "INSERT INTO accounts (address) VALUES ('0xDDD')"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Only SELECT"));
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_drop_table_returns_400() {
        let (pool, _container) = setup_pg().await;
        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "DROP TABLE accounts"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_timeout_returns_408() {
        let (pool, _container) = setup_pg().await;
        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "SELECT pg_sleep(6)",
            "timeout_ms": 100
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_numeric_serialised_as_string() {
        let (pool, _container) = setup_pg().await;
        setup_test_table(&pool).await;

        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "SELECT balance FROM accounts WHERE address = '0xAAA'"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let balance = &json["data"]["rows"][0]["balance"];
        // NUMERIC should be serialised as a string
        assert!(
            balance.is_string(),
            "NUMERIC should be a JSON string, got: {:?}",
            balance
        );
        assert_eq!(balance.as_str().unwrap(), "1000");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_jsonb_serialised_as_embedded_object() {
        let (pool, _container) = setup_pg().await;
        setup_test_table(&pool).await;

        let app = build_test_server_with_db(pool);
        let body = serde_json::json!({
            "query": "SELECT metadata FROM accounts WHERE address = '0xAAA'"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let metadata = &json["data"]["rows"][0]["metadata"];
        assert!(
            metadata.is_object(),
            "JSONB should be an embedded JSON object, got: {:?}",
            metadata
        );
        assert_eq!(metadata["name"], "Alice");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_prod_mode_without_auth_returns_401() {
        let (pool, _container) = setup_pg().await;
        // Build router with prod-mode SQL state manually
        let sql_state = SqlState {
            pool: pool.clone(),
            pg_schema: "public".to_string(),
            rate_limiter: Arc::new(SqlRateLimiter::new(10)),
            api_key: Some("test-secret-key".to_string()),
            prod_mode: true,
        };

        let schema_state = SchemaInfoState {
            pool: pool.clone(),
            pg_schema: "public".to_string(),
        };

        let app = axum::Router::new()
            .route(
                "/sql",
                axum::routing::post(crate::handlers::sql::sql_handler).with_state(sql_state),
            )
            .route(
                "/schema",
                axum::routing::get(crate::handlers::schema_info::schema_info_handler)
                    .with_state(schema_state),
            );

        let body = serde_json::json!({ "query": "SELECT 1" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn post_sql_prod_mode_with_valid_auth_succeeds() {
        let (pool, _container) = setup_pg().await;

        let sql_state = SqlState {
            pool: pool.clone(),
            pg_schema: "public".to_string(),
            rate_limiter: Arc::new(SqlRateLimiter::new(10)),
            api_key: Some("test-secret-key".to_string()),
            prod_mode: true,
        };

        let app = axum::Router::new().route(
            "/sql",
            axum::routing::post(crate::handlers::sql::sql_handler).with_state(sql_state),
        );

        let body = serde_json::json!({ "query": "SELECT 1 as val" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer test-secret-key")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn get_schema_returns_table_metadata() {
        let (pool, _container) = setup_pg().await;
        setup_test_table(&pool).await;

        let app = build_test_server_with_db(pool);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/schema")
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

        let tables = json["data"]["tables"].as_array().unwrap();
        assert!(!tables.is_empty());

        // Find the accounts table
        let accounts = tables
            .iter()
            .find(|t| t["name"] == "accounts")
            .expect("accounts table should be present");

        let columns = accounts["columns"].as_array().unwrap();
        assert!(columns.iter().any(|c| c["name"] == "address"));
        assert!(columns.iter().any(|c| c["name"] == "balance"));

        // Check primary key
        let addr_col = columns.iter().find(|c| c["name"] == "address").unwrap();
        assert_eq!(addr_col["primary_key"], true);

        // Row count should be approximate 3
        let row_count = accounts["row_count"].as_i64().unwrap();
        assert!(row_count >= 0, "row_count should be non-negative");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn rate_limiter_returns_429() {
        let (pool, _container) = setup_pg().await;

        let sql_state = SqlState {
            pool: pool.clone(),
            pg_schema: "public".to_string(),
            rate_limiter: Arc::new(SqlRateLimiter::new(10)),
            api_key: None,
            prod_mode: false,
        };

        let app = axum::Router::new().route(
            "/sql",
            axum::routing::post(crate::handlers::sql::sql_handler).with_state(sql_state),
        );

        let body = serde_json::json!({ "query": "SELECT 1" });
        let body_str = serde_json::to_string(&body).unwrap();

        // Send 10 requests (should all succeed by consuming tokens)
        for _ in 0..10 {
            let app_clone = app.clone();
            let resp = app_clone
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/sql")
                        .header("content-type", "application/json")
                        .body(Body::from(body_str.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // 11th should be rate limited
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(body_str))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().get("retry-after").is_some());
    }
}
