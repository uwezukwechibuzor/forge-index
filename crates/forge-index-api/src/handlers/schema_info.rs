//! GET /schema — returns metadata about indexed tables.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::response::ApiResponse;

/// Shared state for the schema endpoint.
#[derive(Clone)]
pub struct SchemaInfoState {
    pub pool: sqlx::PgPool,
    pub pg_schema: String,
}

/// GET /schema handler.
pub async fn schema_info_handler(State(state): State<SchemaInfoState>) -> Response {
    match fetch_schema_info(&state.pool, &state.pg_schema).await {
        Ok(tables) => ApiResponse::ok(serde_json::json!({ "tables": tables })).into_response(),
        Err(e) => ApiResponse::<()>::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DATABASE_ERROR",
            &e.to_string(),
        )
        .into_response(),
    }
}

async fn fetch_schema_info(
    pool: &sqlx::PgPool,
    pg_schema: &str,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    // Fetch all user tables in the schema (exclude internal tables)
    let table_rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT tablename::text
           FROM pg_tables
           WHERE schemaname = $1
             AND tablename NOT LIKE '\_%'
           ORDER BY tablename"#,
    )
    .bind(pg_schema)
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();

    for (table_name,) in &table_rows {
        // Fetch columns
        let columns: Vec<ColumnInfo> = sqlx::query_as(
            r#"SELECT
                 c.column_name::text,
                 c.data_type::text,
                 c.is_nullable::text,
                 CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END as is_pk
               FROM information_schema.columns c
               LEFT JOIN (
                 SELECT kcu.column_name
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage kcu
                   ON tc.constraint_name = kcu.constraint_name
                   AND tc.table_schema = kcu.table_schema
                 WHERE tc.constraint_type = 'PRIMARY KEY'
                   AND tc.table_schema = $1
                   AND tc.table_name = $2
               ) pk ON pk.column_name = c.column_name
               WHERE c.table_schema = $1
                 AND c.table_name = $2
               ORDER BY c.ordinal_position"#,
        )
        .bind(pg_schema)
        .bind(table_name)
        .fetch_all(pool)
        .await?;

        // Approximate row count (fast, from pg_class stats)
        let row_count: (f64,) = sqlx::query_as(
            r#"SELECT COALESCE(reltuples, 0)::float8
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               WHERE n.nspname = $1 AND c.relname = $2"#,
        )
        .bind(pg_schema)
        .bind(table_name)
        .fetch_optional(pool)
        .await?
        .unwrap_or((0.0,));

        let col_json: Vec<serde_json::Value> = columns
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.column_name,
                    "type": c.data_type,
                    "primary_key": c.is_pk,
                    "nullable": c.is_nullable == "YES",
                })
            })
            .collect();

        tables.push(serde_json::json!({
            "name": table_name,
            "columns": col_json,
            "row_count": row_count.0 as i64,
        }));
    }

    Ok(tables)
}

#[derive(sqlx::FromRow)]
struct ColumnInfo {
    column_name: String,
    data_type: String,
    is_nullable: String,
    is_pk: bool,
}
