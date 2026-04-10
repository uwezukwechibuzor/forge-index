//! SQL executor — runs validated queries against PgPool with timeout.

use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Row, TypeInfo};

use super::result::SqlResult;
use super::{SqlError, ValidatedSql};

const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Executes a validated SQL query with a statement timeout.
pub async fn execute_sql(
    pool: &PgPool,
    sql: &ValidatedSql,
    timeout_ms: Option<u64>,
) -> Result<SqlResult, SqlError> {
    let timeout = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    let start = std::time::Instant::now();

    // Use a transaction so SET LOCAL is scoped
    let mut tx = pool.begin().await.map_err(SqlError::Database)?;

    // Set statement timeout
    let timeout_sql = format!("SET LOCAL statement_timeout = '{}ms'", timeout);
    sqlx::query(&timeout_sql)
        .execute(&mut *tx)
        .await
        .map_err(SqlError::Database)?;

    // Execute the query
    let rows: Vec<PgRow> = match sqlx::query(&sql.sanitised).fetch_all(&mut *tx).await {
        Ok(rows) => rows,
        Err(e) => {
            let err_str = e.to_string();
            // Check for timeout-related errors
            if err_str.contains("statement timeout")
                || err_str.contains("canceling statement due to statement timeout")
            {
                return Err(SqlError::Timeout);
            }
            return Err(SqlError::Database(e));
        }
    };

    // Rollback (read-only, nothing to commit)
    let _ = tx.rollback().await;

    let elapsed = start.elapsed().as_millis() as u64;

    if rows.is_empty() {
        return Ok(SqlResult {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time_ms: elapsed,
        });
    }

    // Extract column names from the first row
    let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();

    // Convert rows to JSON
    let json_rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| row_to_json(row, &columns))
        .collect();

    let row_count = json_rows.len();

    Ok(SqlResult {
        columns,
        rows: json_rows,
        row_count,
        execution_time_ms: elapsed,
    })
}

/// Converts a PgRow to a JSON object, handling type-specific serialisation.
fn row_to_json(row: &PgRow, columns: &[String]) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    for (i, col_name) in columns.iter().enumerate() {
        let col = row.columns().get(i).unwrap();
        let type_name = col.type_info().name();

        let value = match type_name {
            "BOOL" => row
                .try_get::<Option<bool>, _>(i)
                .ok()
                .flatten()
                .map(serde_json::Value::Bool)
                .unwrap_or(serde_json::Value::Null),

            "INT2" | "SMALLINT" => row
                .try_get::<Option<i16>, _>(i)
                .ok()
                .flatten()
                .map(|v| serde_json::Value::Number(v.into()))
                .unwrap_or(serde_json::Value::Null),

            "INT4" | "INT" | "INTEGER" => row
                .try_get::<Option<i32>, _>(i)
                .ok()
                .flatten()
                .map(|v| serde_json::Value::Number(v.into()))
                .unwrap_or(serde_json::Value::Null),

            "INT8" | "BIGINT" => row
                .try_get::<Option<i64>, _>(i)
                .ok()
                .flatten()
                .map(|v| serde_json::Value::Number(v.into()))
                .unwrap_or(serde_json::Value::Null),

            "FLOAT4" | "REAL" => row
                .try_get::<Option<f32>, _>(i)
                .ok()
                .flatten()
                .and_then(|v| serde_json::Number::from_f64(v as f64))
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),

            "FLOAT8" | "DOUBLE PRECISION" => row
                .try_get::<Option<f64>, _>(i)
                .ok()
                .flatten()
                .and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),

            "NUMERIC" => {
                // Serialise as string to safely handle large numbers
                row.try_get::<Option<sqlx::types::BigDecimal>, _>(i)
                    .ok()
                    .flatten()
                    .map(|v| serde_json::Value::String(v.to_string()))
                    .unwrap_or(serde_json::Value::Null)
            }

            "JSONB" | "JSON" => row
                .try_get::<Option<serde_json::Value>, _>(i)
                .ok()
                .flatten()
                .unwrap_or(serde_json::Value::Null),

            "BYTEA" => row
                .try_get::<Option<Vec<u8>>, _>(i)
                .ok()
                .flatten()
                .map(|v| serde_json::Value::String(format!("0x{}", hex::encode(v))))
                .unwrap_or(serde_json::Value::Null),

            // TEXT, VARCHAR, and everything else → String via Display
            _ => row
                .try_get::<Option<String>, _>(i)
                .ok()
                .flatten()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        };

        map.insert(col_name.clone(), value);
    }

    serde_json::Value::Object(map)
}
