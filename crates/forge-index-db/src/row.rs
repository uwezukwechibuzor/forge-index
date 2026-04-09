//! Row and ColumnValue types for the write buffer.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A single column value in a row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnValue {
    /// Variable-length text.
    Text(String),
    /// Boolean value.
    Boolean(bool),
    /// 32-bit integer.
    Int(i32),
    /// 64-bit integer.
    BigInt(i64),
    /// Double-precision floating point.
    Float(f64),
    /// Raw byte data.
    Bytes(Vec<u8>),
    /// JSON value.
    Json(serde_json::Value),
    /// SQL NULL.
    Null,
    /// Arbitrary-precision numeric stored as a string (e.g. U256).
    BigNumeric(String),
}

impl From<String> for ColumnValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&str> for ColumnValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<bool> for ColumnValue {
    fn from(v: bool) -> Self {
        Self::Boolean(v)
    }
}

impl From<i32> for ColumnValue {
    fn from(v: i32) -> Self {
        Self::Int(v)
    }
}

impl From<i64> for ColumnValue {
    fn from(v: i64) -> Self {
        Self::BigInt(v)
    }
}

impl From<f64> for ColumnValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<Vec<u8>> for ColumnValue {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl From<serde_json::Value> for ColumnValue {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

/// Represents a single database row as an ordered map of column names to values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// The ordered column-value pairs.
    pub columns: IndexMap<String, ColumnValue>,
    /// Internal operation marker for the write buffer.
    /// `None` = INSERT, `Some("UPDATE")`, `Some("DELETE")`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

impl Row {
    /// Creates a new empty row (INSERT operation).
    pub fn new() -> Self {
        Self {
            columns: IndexMap::new(),
            operation: None,
        }
    }

    /// Inserts a column value into the row.
    pub fn insert(&mut self, col: &str, val: impl Into<ColumnValue>) {
        self.columns.insert(col.to_string(), val.into());
    }

    /// Returns a reference to the value for the given column.
    pub fn get(&self, col: &str) -> Option<&ColumnValue> {
        self.columns.get(col)
    }

    /// Returns the SQL text representation of a column value for use in queries.
    pub fn to_sql_literal(val: &ColumnValue) -> String {
        match val {
            ColumnValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
            ColumnValue::Boolean(b) => b.to_string(),
            ColumnValue::Int(i) => i.to_string(),
            ColumnValue::BigInt(i) => i.to_string(),
            ColumnValue::Float(f) => f.to_string(),
            ColumnValue::Bytes(b) => format!("'\\x{}'", hex::encode(b)),
            ColumnValue::Json(v) => format!("'{}'", v.to_string().replace('\'', "''")),
            ColumnValue::Null => "NULL".to_string(),
            ColumnValue::BigNumeric(s) => s.clone(),
        }
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}
