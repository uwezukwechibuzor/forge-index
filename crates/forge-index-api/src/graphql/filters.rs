//! WHERE clause generation from GraphQL filter arguments.

/// Converts a GraphQL filter input JSON into a SQL WHERE clause.
///
/// The filter JSON looks like:
/// ```json
/// { "address": { "eq": "0x..." }, "balance": { "gt": "100" } }
/// ```
///
/// Returns a SQL fragment like:
/// `"address" = '0x...' AND CAST("balance" AS NUMERIC) > 100`
pub fn filters_to_sql(
    filter: &serde_json::Value,
    _pg_schema: &str,
    _table: &str,
) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let params = Vec::new();

    if let serde_json::Value::Object(fields) = filter {
        for (field_name, filter_obj) in fields {
            if let serde_json::Value::Object(ops) = filter_obj {
                for (op, value) in ops {
                    if let Some(condition) = build_condition(field_name, op, value) {
                        conditions.push(condition);
                    }
                }
            }
        }
    }

    let clause = if conditions.is_empty() {
        String::new()
    } else {
        conditions.join(" AND ")
    };

    (clause, params)
}

fn build_condition(field: &str, op: &str, value: &serde_json::Value) -> Option<String> {
    match op {
        "eq" | "gt" | "gte" | "lt" | "lte" => {
            let sql_val = value_to_sql_literal(value)?;
            let sql_op = match op {
                "eq" => "=",
                "gt" => ">",
                "gte" => ">=",
                "lt" => "<",
                "lte" => "<=",
                _ => unreachable!(),
            };
            Some(format!("\"{}\" {} {}", field, sql_op, sql_val))
        }
        "contains" => {
            let raw = value.as_str().unwrap_or("");
            Some(format!(
                "\"{}\" ILIKE '%{}%'",
                field,
                raw.replace('\'', "''")
            ))
        }
        "in" => {
            if let serde_json::Value::Array(arr) = value {
                let vals: Vec<String> = arr.iter().filter_map(value_to_sql_literal).collect();
                if vals.is_empty() {
                    return None;
                }
                Some(format!("\"{}\" IN ({})", field, vals.join(", ")))
            } else {
                None
            }
        }
        "notIn" => {
            if let serde_json::Value::Array(arr) = value {
                let vals: Vec<String> = arr.iter().filter_map(value_to_sql_literal).collect();
                if vals.is_empty() {
                    return None;
                }
                Some(format!("\"{}\" NOT IN ({})", field, vals.join(", ")))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn value_to_sql_literal(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(format!("'{}'", s.replace('\'', "''"))),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => Some("NULL".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_filter() {
        let filter = serde_json::json!({ "name": { "eq": "Alice" } });
        let (sql, _) = filters_to_sql(&filter, "public", "users");
        assert_eq!(sql, "\"name\" = 'Alice'");
    }

    #[test]
    fn gt_filter() {
        let filter = serde_json::json!({ "age": { "gt": 18 } });
        let (sql, _) = filters_to_sql(&filter, "public", "users");
        assert_eq!(sql, "\"age\" > 18");
    }

    #[test]
    fn in_filter() {
        let filter = serde_json::json!({ "status": { "in": ["active", "pending"] } });
        let (sql, _) = filters_to_sql(&filter, "public", "users");
        assert_eq!(sql, "\"status\" IN ('active', 'pending')");
    }

    #[test]
    fn multiple_filters() {
        let filter = serde_json::json!({
            "name": { "eq": "Alice" },
            "age": { "gte": 18 }
        });
        let (sql, _) = filters_to_sql(&filter, "public", "users");
        assert!(sql.contains("\"name\" = 'Alice'"));
        assert!(sql.contains("\"age\" >= 18"));
        assert!(sql.contains(" AND "));
    }
}
