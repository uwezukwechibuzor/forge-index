//! SQL validator — enforces read-only queries with safety checks.

use super::SqlError;

const MAX_QUERY_LENGTH: usize = 10_000;
const MAX_LIMIT: u64 = 1000;

/// Banned keywords that indicate write operations or dangerous access.
const BANNED_KEYWORDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "TRUNCATE", "GRANT", "REVOKE", "COPY",
    "EXECUTE",
];

/// Banned schema references.
const BANNED_SCHEMAS: &[&str] = &["pg_catalog", "information_schema"];

/// A validated and sanitised SQL query.
#[derive(Debug, Clone)]
pub struct ValidatedSql {
    /// The original query string.
    pub original: String,
    /// The sanitised query with LIMIT enforced and schema prefix added.
    pub sanitised: String,
    /// Table names extracted from FROM and JOIN clauses.
    pub table_names: Vec<String>,
}

/// Validates and sanitises a SQL query for safe read-only execution.
pub fn validate_sql(sql: &str, pg_schema: &str) -> Result<ValidatedSql, SqlError> {
    let sql = sql.trim();

    // 1. Check length
    if sql.len() > MAX_QUERY_LENGTH {
        return Err(SqlError::TooLong {
            len: sql.len(),
            max: MAX_QUERY_LENGTH,
        });
    }

    // 2. Reject empty queries
    if sql.is_empty() {
        return Err(SqlError::InvalidStatement("Empty query".to_string()));
    }

    // 3. Check first keyword is SELECT
    let first_word = sql.split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "SELECT" {
        return Err(SqlError::InvalidStatement(
            "Only SELECT statements are allowed".to_string(),
        ));
    }

    // 4. Check for semicolons in the middle (statement chaining)
    let trimmed = sql.trim_end_matches(';').trim();
    if trimmed.contains(';') {
        return Err(SqlError::InvalidStatement(
            "Multiple statements are not allowed".to_string(),
        ));
    }

    // 5. Check for dollar-sign quoting
    if sql.contains("$$") || sql.contains("$tag$") {
        return Err(SqlError::InvalidStatement(
            "Dollar-sign quoting is not allowed".to_string(),
        ));
    }

    // 6. Scan for banned keywords (case-insensitive, word-boundary check)
    let upper = sql.to_uppercase();
    for keyword in BANNED_KEYWORDS {
        if contains_keyword(&upper, keyword) {
            return Err(SqlError::ForbiddenKeyword(keyword.to_string()));
        }
    }

    // 7. Check for banned schema references
    let lower = sql.to_lowercase();
    for schema in BANNED_SCHEMAS {
        if lower.contains(schema) {
            return Err(SqlError::ForbiddenKeyword(format!(
                "Access to {} is not allowed",
                schema
            )));
        }
    }

    // 8. Extract table names from FROM and JOIN clauses
    let table_names = extract_table_names(trimmed);

    // 9. Add schema prefix to unqualified table names
    let mut sanitised = trimmed.to_string();
    for table in &table_names {
        sanitised = add_schema_prefix(&sanitised, table, pg_schema);
    }

    // 10. Enforce LIMIT
    sanitised = enforce_limit(&sanitised);

    Ok(ValidatedSql {
        original: sql.to_string(),
        sanitised,
        table_names,
    })
}

/// Checks if a keyword appears as a standalone word in the upper-cased SQL.
fn contains_keyword(upper_sql: &str, keyword: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = upper_sql[start..].find(keyword) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0
            || !upper_sql.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
                && upper_sql.as_bytes()[abs_pos - 1] != b'_';
        let after_pos = abs_pos + keyword.len();
        let after_ok = after_pos >= upper_sql.len()
            || !upper_sql.as_bytes()[after_pos].is_ascii_alphanumeric()
                && upper_sql.as_bytes()[after_pos] != b'_';
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + keyword.len();
    }
    false
}

/// Extracts table names from FROM and JOIN clauses.
fn extract_table_names(sql: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let tokens: Vec<&str> = sql.split_whitespace().collect();

    for i in 0..tokens.len() {
        let upper = tokens[i].to_uppercase();
        if (upper == "FROM" || upper.ends_with("JOIN")) && i + 1 < tokens.len() {
            let raw = tokens[i + 1]
                .trim_matches(|c: char| c == '(' || c == ')' || c == ',')
                .trim_matches('"');
            if !raw.is_empty()
                && raw != "("
                && !raw.to_uppercase().starts_with("SELECT")
                && !raw.starts_with('(')
            {
                // Only the table name part (after any schema dot)
                let name = if let Some((_schema, table)) = raw.rsplit_once('.') {
                    table.to_string()
                } else {
                    raw.to_string()
                };
                if !tables.contains(&name) {
                    tables.push(name);
                }
            }
        }
    }

    tables
}

/// Adds schema prefix to unqualified table references.
fn add_schema_prefix(sql: &str, table: &str, pg_schema: &str) -> String {
    let qualified = format!("{}.\"{}\"", pg_schema, table);

    // Replace patterns: FROM table, JOIN table (case-insensitive, not already qualified)
    let mut result = sql.to_string();
    let keywords = ["FROM", "JOIN", "from", "join", "From", "Join"];

    for kw in &keywords {
        // Match "FROM table" but not "FROM schema.table"
        let pattern = format!("{} {}", kw, table);
        let already_qualified = format!("{} {}.", kw, pg_schema);
        let already_quoted = format!("{} \"{}", kw, table);

        if result.contains(&pattern) && !result.contains(&already_qualified) {
            result = result.replace(&pattern, &format!("{} {}", kw, qualified));
        }
        if result.contains(&already_quoted) && !result.contains(&already_qualified) {
            result = result.replace(
                &format!("{} \"{}\"", kw, table),
                &format!("{} {}", kw, qualified),
            );
        }
    }

    result
}

/// Enforces a LIMIT clause: appends LIMIT 1000 if missing, clamps if > 1000.
fn enforce_limit(sql: &str) -> String {
    let upper = sql.to_uppercase();

    if let Some(limit_pos) = find_keyword_pos(&upper, "LIMIT") {
        // Extract the number after LIMIT
        let after_limit = &sql[limit_pos + 5..].trim_start();
        if let Some(num_str) = after_limit.split_whitespace().next() {
            if let Ok(n) = num_str
                .trim_matches(|c: char| !c.is_ascii_digit())
                .parse::<u64>()
            {
                if n > MAX_LIMIT {
                    // Replace the number with MAX_LIMIT
                    let num_start = sql.len() - sql[limit_pos + 5..].len()
                        + sql[limit_pos + 5..].len().saturating_sub(after_limit.len());
                    let num_end = num_start + num_str.len();
                    return format!("{}{}{}", &sql[..num_start], MAX_LIMIT, &sql[num_end..]);
                }
            }
        }
        sql.to_string()
    } else {
        format!("{} LIMIT {}", sql, MAX_LIMIT)
    }
}

/// Finds position of a keyword at word boundary in upper-cased SQL.
fn find_keyword_pos(upper_sql: &str, keyword: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(pos) = upper_sql[start..].find(keyword) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0
            || !upper_sql.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
                && upper_sql.as_bytes()[abs_pos - 1] != b'_';
        let after_pos = abs_pos + keyword.len();
        let after_ok = after_pos >= upper_sql.len()
            || !upper_sql.as_bytes()[after_pos].is_ascii_alphanumeric()
                && upper_sql.as_bytes()[after_pos] != b'_';
        if before_ok && after_ok {
            return Some(abs_pos);
        }
        start = abs_pos + keyword.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_select_passes() {
        let result = validate_sql("SELECT * FROM accounts", "public");
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v.sanitised.contains("public.\"accounts\""));
        assert!(v.sanitised.contains("LIMIT 1000"));
    }

    #[test]
    fn insert_rejected() {
        let result = validate_sql("INSERT INTO accounts VALUES (1)", "public");
        assert!(matches!(result, Err(SqlError::InvalidStatement(_))));
    }

    #[test]
    fn drop_rejected() {
        let result = validate_sql("SELECT 1; DROP TABLE accounts", "public");
        assert!(matches!(result, Err(SqlError::InvalidStatement(_))));
    }

    #[test]
    fn limit_appended_when_missing() {
        let v = validate_sql("SELECT * FROM accounts", "public").unwrap();
        assert!(v.sanitised.ends_with("LIMIT 1000"));
    }

    #[test]
    fn limit_clamped_when_too_high() {
        let v = validate_sql("SELECT * FROM accounts LIMIT 5000", "public").unwrap();
        assert!(v.sanitised.contains("LIMIT 1000"));
        assert!(!v.sanitised.contains("5000"));
    }

    #[test]
    fn limit_preserved_when_under_max() {
        let v = validate_sql("SELECT * FROM accounts LIMIT 10", "public").unwrap();
        assert!(v.sanitised.contains("LIMIT 10"));
    }

    #[test]
    fn schema_prefix_added() {
        let v = validate_sql("SELECT * FROM accounts", "public").unwrap();
        assert!(v.sanitised.contains("public.\"accounts\""));
    }

    #[test]
    fn table_names_extracted() {
        let v = validate_sql(
            "SELECT a.*, b.name FROM accounts a JOIN transfers b ON a.id = b.from_id",
            "public",
        )
        .unwrap();
        assert!(v.table_names.contains(&"accounts".to_string()));
        assert!(v.table_names.contains(&"transfers".to_string()));
    }

    #[test]
    fn semicolon_in_middle_rejected() {
        let result = validate_sql("SELECT 1; SELECT 2", "public");
        assert!(matches!(result, Err(SqlError::InvalidStatement(_))));
    }

    #[test]
    fn dollar_quoting_rejected() {
        let result = validate_sql("SELECT $$ DROP TABLE foo $$", "public");
        assert!(matches!(result, Err(SqlError::InvalidStatement(_))));
    }

    #[test]
    fn pg_catalog_rejected() {
        let result = validate_sql("SELECT * FROM pg_catalog.pg_tables", "public");
        assert!(matches!(result, Err(SqlError::ForbiddenKeyword(_))));
    }

    #[test]
    fn query_too_long_rejected() {
        let long = format!("SELECT * FROM accounts WHERE id = '{}'", "x".repeat(10_000));
        let result = validate_sql(&long, "public");
        assert!(matches!(result, Err(SqlError::TooLong { .. })));
    }

    #[test]
    fn empty_query_rejected() {
        let result = validate_sql("", "public");
        assert!(matches!(result, Err(SqlError::InvalidStatement(_))));
    }
}
