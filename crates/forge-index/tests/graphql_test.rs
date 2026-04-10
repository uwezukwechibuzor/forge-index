//! GraphQL API end-to-end tests.
//!
//! Run with: `cargo test -p forge-index --test graphql_test`

mod common;

use forge_index_api::graphql::filters::filters_to_sql;
use forge_index_api::graphql::pagination::{decode_cursor, encode_cursor};
use forge_index_api::graphql::types::{column_type_to_gql, to_camel_case, to_pascal_case};
use forge_index_config::ColumnType;

#[test]
fn test_graphql_type_mapping() {
    assert_eq!(column_type_to_gql(&ColumnType::Text), "String");
    assert_eq!(column_type_to_gql(&ColumnType::Int), "Int");
    assert_eq!(column_type_to_gql(&ColumnType::BigInt), "String");
    assert_eq!(column_type_to_gql(&ColumnType::Boolean), "Boolean");
    assert_eq!(column_type_to_gql(&ColumnType::Address), "String");
}

#[test]
fn test_graphql_naming_conventions() {
    assert_eq!(to_camel_case("accounts"), "accounts");
    assert_eq!(to_camel_case("transfer_events"), "transferEvents");
    assert_eq!(to_camel_case("my_big_table"), "myBigTable");

    assert_eq!(to_pascal_case("accounts"), "Accounts");
    assert_eq!(to_pascal_case("transfer_events"), "TransferEvents");
}

#[test]
fn test_graphql_filter_eq() {
    let filter = serde_json::json!({ "address": { "eq": "0xABC" } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"address\" = '0xABC'");
}

#[test]
fn test_graphql_filter_gt() {
    let filter = serde_json::json!({ "balance": { "gt": 5000 } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"balance\" > 5000");
}

#[test]
fn test_graphql_filter_gte() {
    let filter = serde_json::json!({ "balance": { "gte": 5000 } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"balance\" >= 5000");
}

#[test]
fn test_graphql_filter_lt() {
    let filter = serde_json::json!({ "balance": { "lt": 100 } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"balance\" < 100");
}

#[test]
fn test_graphql_filter_lte() {
    let filter = serde_json::json!({ "balance": { "lte": 100 } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"balance\" <= 100");
}

#[test]
fn test_graphql_filter_in() {
    let filter = serde_json::json!({ "status": { "in": ["active", "pending"] } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"status\" IN ('active', 'pending')");
}

#[test]
fn test_graphql_filter_not_in() {
    let filter = serde_json::json!({ "status": { "notIn": ["deleted"] } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert_eq!(sql, "\"status\" NOT IN ('deleted')");
}

#[test]
fn test_graphql_filter_contains() {
    let filter = serde_json::json!({ "name": { "contains": "alice" } });
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert!(sql.contains("LIKE"));
    assert!(sql.contains("alice"));
}

#[test]
fn test_graphql_multiple_filters_anded() {
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
fn test_graphql_cursor_roundtrip() {
    let cursor = encode_cursor("pk-123", "created_at", &serde_json::json!(1234567));
    let decoded = decode_cursor(&cursor).unwrap();
    assert_eq!(decoded.pk_value, "pk-123");
    assert_eq!(decoded.order_col, "created_at");
    assert_eq!(decoded.order_val, serde_json::json!(1234567));
}

#[test]
fn test_graphql_cursor_string_values() {
    let cursor = encode_cursor("id-1", "name", &serde_json::json!("Alice"));
    let decoded = decode_cursor(&cursor).unwrap();
    assert_eq!(decoded.pk_value, "id-1");
    assert_eq!(decoded.order_val, serde_json::json!("Alice"));
}

#[test]
fn test_graphql_invalid_cursor() {
    assert!(decode_cursor("not-valid-base64!!!!").is_err());
    assert!(decode_cursor("").is_err());
}

#[test]
fn test_graphql_bigint_maps_to_string() {
    // BigInt should map to String in GraphQL to avoid JS number precision issues
    assert_eq!(column_type_to_gql(&ColumnType::BigInt), "String");
}

#[test]
fn test_graphql_empty_filter() {
    let filter = serde_json::json!({});
    let (sql, _) = filters_to_sql(&filter, "public", "accounts");
    assert!(sql.is_empty() || sql == "1=1", "Empty filter should produce empty or trivial SQL, got: {}", sql);
}
