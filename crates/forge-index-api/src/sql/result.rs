//! SqlResult type with JSON row serialisation.

use serde::Serialize;

/// The result of a SQL query execution.
#[derive(Debug, Serialize)]
pub struct SqlResult {
    /// Column names in order.
    pub columns: Vec<String>,
    /// Rows as JSON objects (one object per row, keyed by column name).
    pub rows: Vec<serde_json::Value>,
    /// Number of rows returned.
    pub row_count: usize,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
}
