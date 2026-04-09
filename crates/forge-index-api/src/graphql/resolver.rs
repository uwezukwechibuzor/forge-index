//! Query resolvers for single-record and paginated list queries.

use serde_json::Value;
use sqlx::PgPool;

use crate::graphql::filters::filters_to_sql;
use crate::graphql::pagination::{decode_cursor, encode_cursor};

/// Resolves a single-record query by primary key.
pub async fn resolve_single(
    pool: &PgPool,
    pg_schema: &str,
    table: &str,
    pk_col: &str,
    pk_value: &str,
    columns: &[String],
) -> Result<Option<Value>, String> {
    let col_list = columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT row_to_json((SELECT r FROM (SELECT {}) AS r)) AS doc \
         FROM \"{}\".\"{}\" WHERE \"{}\" = $1 LIMIT 1",
        col_list, pg_schema, table, pk_col
    );

    let row: Option<(Value,)> = sqlx::query_as(&sql)
        .bind(pk_value)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(row.map(|(v,)| v))
}

/// Resolves a paginated list query.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_list(
    pool: &PgPool,
    pg_schema: &str,
    table: &str,
    pk_col: &str,
    columns: &[String],
    filter: Option<&Value>,
    order_by: Option<&str>,
    order_direction: Option<&str>,
    limit: Option<i64>,
    after: Option<&str>,
    before: Option<&str>,
) -> Result<Value, String> {
    let col_list = columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");

    let order_col = order_by.unwrap_or(pk_col);
    let dir = order_direction.unwrap_or("ASC");
    let effective_limit = limit.unwrap_or(100).min(1000);

    // Build WHERE conditions
    let mut where_parts = Vec::new();

    // Filter
    if let Some(f) = filter {
        let (filter_sql, _) = filters_to_sql(f, pg_schema, table);
        if !filter_sql.is_empty() {
            where_parts.push(filter_sql);
        }
    }

    // Cursor (after)
    if let Some(cursor_str) = after {
        if let Ok(cursor) = decode_cursor(cursor_str) {
            let cursor_val = value_to_sql(&cursor.order_val);
            where_parts.push(format!(
                "(\"{}\", \"{}\") > ({}, '{}')",
                order_col, pk_col, cursor_val, cursor.pk_value
            ));
        }
    }

    // Cursor (before)
    if let Some(cursor_str) = before {
        if let Ok(cursor) = decode_cursor(cursor_str) {
            let cursor_val = value_to_sql(&cursor.order_val);
            where_parts.push(format!(
                "(\"{}\", \"{}\") < ({}, '{}')",
                order_col, pk_col, cursor_val, cursor.pk_value
            ));
        }
    }

    let where_clause = if where_parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_parts.join(" AND "))
    };

    // Total count (with filters but without pagination)
    let count_sql = format!(
        "SELECT COUNT(*) FROM \"{}\".\"{}\"{}",
        pg_schema, table, where_clause
    );
    let (total_count,): (i64,) = sqlx::query_as(&count_sql)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Fetch limit + 1 to detect hasNextPage
    let fetch_limit = effective_limit + 1;
    let data_sql = format!(
        "SELECT row_to_json((SELECT r FROM (SELECT {}) AS r)) AS doc \
         FROM \"{}\".\"{}\"{} ORDER BY \"{}\" {}, \"{}\" {} LIMIT {}",
        col_list, pg_schema, table, where_clause, order_col, dir, pk_col, dir, fetch_limit
    );

    let rows: Vec<(Value,)> = sqlx::query_as(&data_sql)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let has_next = rows.len() as i64 > effective_limit;
    let items: Vec<Value> = rows
        .into_iter()
        .take(effective_limit as usize)
        .map(|(v,)| v)
        .collect();

    // Build cursors
    let start_cursor = items.first().and_then(|item| {
        let pk = item.get(pk_col)?.as_str()?;
        let order_val = item.get(order_col).cloned().unwrap_or(Value::Null);
        Some(encode_cursor(pk, order_col, &order_val))
    });

    let end_cursor = items.last().and_then(|item| {
        let pk = item.get(pk_col)?.as_str()?;
        let order_val = item.get(order_col).cloned().unwrap_or(Value::Null);
        Some(encode_cursor(pk, order_col, &order_val))
    });

    let page_info = serde_json::json!({
        "hasNextPage": has_next,
        "hasPreviousPage": after.is_some(),
        "startCursor": start_cursor,
        "endCursor": end_cursor,
    });

    Ok(serde_json::json!({
        "items": items,
        "pageInfo": page_info,
        "totalCount": total_count,
    }))
}

fn value_to_sql(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => "NULL".to_string(),
    }
}
