//! SQL-over-HTTP end-to-end tests.
//!
//! Run with: `cargo test -p forge-index --test sql_test -- --ignored`

mod common;

use common::test_db::TestDb;
use forge_index_api::sql::parser::validate_sql;
use forge_index_api::sql::SqlError;

#[test]
fn test_sql_select_valid() {
    let result = validate_sql("SELECT * FROM accounts WHERE balance > 100", "public");
    assert!(result.is_ok());
    let v = result.unwrap();
    assert!(v.sanitised.contains("public.\"accounts\""));
    assert!(v.sanitised.contains("LIMIT 1000"));
}

#[test]
fn test_sql_insert_rejected() {
    let result = validate_sql("INSERT INTO accounts VALUES ('a', 100)", "public");
    assert!(matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Only SELECT")));
}

#[test]
fn test_sql_drop_rejected() {
    let result = validate_sql("DROP TABLE accounts", "public");
    assert!(matches!(result, Err(SqlError::InvalidStatement(msg)) if msg.contains("Only SELECT")));
}

#[test]
fn test_sql_limit_enforced() {
    // No limit → appended
    let v = validate_sql("SELECT * FROM t", "public").unwrap();
    assert!(v.sanitised.contains("LIMIT 1000"));

    // Over limit → clamped
    let v = validate_sql("SELECT * FROM t LIMIT 5000", "public").unwrap();
    assert!(v.sanitised.contains("LIMIT 1000"));
    assert!(!v.sanitised.contains("5000"));

    // Under limit → preserved
    let v = validate_sql("SELECT * FROM t LIMIT 50", "public").unwrap();
    assert!(v.sanitised.contains("LIMIT 50"));
}

#[test]
fn test_sql_schema_prefix_added() {
    let v = validate_sql("SELECT * FROM accounts", "myschema").unwrap();
    assert!(v.sanitised.contains("myschema.\"accounts\""));
}

#[test]
fn test_sql_semicolon_injection_blocked() {
    let result = validate_sql("SELECT 1; DROP TABLE accounts", "public");
    assert!(result.is_err());
}

#[test]
fn test_sql_pg_catalog_blocked() {
    let result = validate_sql("SELECT * FROM pg_catalog.pg_tables", "public");
    assert!(matches!(result, Err(SqlError::ForbiddenKeyword(_))));
}

#[test]
fn test_sql_information_schema_blocked() {
    let result = validate_sql("SELECT * FROM information_schema.columns", "public");
    assert!(matches!(result, Err(SqlError::ForbiddenKeyword(_))));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_sql_timeout_returns_error() {
    let db = TestDb::new().await;

    let validated = validate_sql("SELECT pg_sleep(6)", "public").unwrap();
    let result =
        forge_index_api::sql::execute_sql(&db.pool, &validated, Some(100)).await;

    assert!(
        matches!(result, Err(SqlError::Timeout)),
        "Expected timeout error, got: {:?}",
        result
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_sql_execute_returns_rows() {
    let db = TestDb::new().await;

    // Create a test table
    sqlx::query("CREATE TABLE IF NOT EXISTS test_accounts (address TEXT PRIMARY KEY, balance NUMERIC NOT NULL)")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO test_accounts VALUES ('0xAAA', 1000), ('0xBBB', 2000)")
        .execute(&db.pool)
        .await
        .unwrap();

    let validated = validate_sql("SELECT * FROM test_accounts ORDER BY address", "public").unwrap();
    let result = forge_index_api::sql::execute_sql(&db.pool, &validated, None)
        .await
        .unwrap();

    assert_eq!(result.row_count, 2);
    assert!(result.columns.contains(&"address".to_string()));
    assert!(result.columns.contains(&"balance".to_string()));
}

#[test]
fn test_sql_rate_limit() {
    use forge_index_api::handlers::sql::SqlRateLimiter;
    use std::net::{IpAddr, Ipv4Addr};

    let limiter = SqlRateLimiter::new(10);
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    // First 10 should succeed
    for _ in 0..10 {
        assert!(limiter.try_acquire(ip));
    }
    // 11th should fail
    assert!(!limiter.try_acquire(ip));
}
