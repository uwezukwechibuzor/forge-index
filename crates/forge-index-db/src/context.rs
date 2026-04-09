//! DbContext — the API exposed to user indexing handlers.

use crate::buffer::WriteBuffer;
use crate::error::DbError;
use crate::query::QueryBuilder;
use crate::row::{ColumnValue, Row};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;

/// The database context passed to user indexing handlers.
///
/// Provides a high-level API for inserting, updating, deleting,
/// and querying rows in the indexed tables.
pub struct DbContext {
    buffer: Arc<WriteBuffer>,
    pool: PgPool,
    pg_schema: String,
}

impl DbContext {
    /// Creates a new database context.
    pub fn new(buffer: Arc<WriteBuffer>, pool: PgPool, pg_schema: String) -> Self {
        Self {
            buffer,
            pool,
            pg_schema,
        }
    }

    /// Starts building an INSERT operation for the given table.
    pub fn insert(&self, table: &str) -> InsertBuilder<'_> {
        InsertBuilder {
            ctx: self,
            table: table.to_string(),
            row: Row::new(),
        }
    }

    /// Starts building an UPDATE operation for the given table.
    pub fn update(&self, table: &str) -> UpdateBuilder<'_> {
        UpdateBuilder {
            ctx: self,
            table: table.to_string(),
            row: Row::new(),
            pk_value: None,
        }
    }

    /// Starts building a DELETE operation for the given table.
    pub fn delete(&self, table: &str) -> DeleteBuilder<'_> {
        DeleteBuilder {
            ctx: self,
            table: table.to_string(),
            pk_col: None,
            pk_value: None,
        }
    }

    /// Starts building a query to find one row.
    pub fn find_one<T: DeserializeOwned>(&self, table: &str) -> QueryBuilder<T> {
        QueryBuilder::new(self.pool.clone(), self.pg_schema.clone(), table.to_string())
    }

    /// Starts building a query to find many rows.
    pub fn find_many<T: DeserializeOwned>(&self, table: &str) -> QueryBuilder<T> {
        QueryBuilder::new(self.pool.clone(), self.pg_schema.clone(), table.to_string())
    }
}

/// Builder for INSERT operations.
pub struct InsertBuilder<'a> {
    ctx: &'a DbContext,
    table: String,
    row: Row,
}

impl<'a> InsertBuilder<'a> {
    /// Sets the row values from a serializable struct.
    pub fn values(mut self, val: impl Serialize) -> Result<Self, DbError> {
        let json = serde_json::to_value(val)?;
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                let col_val = json_to_column_value(v);
                self.row.insert(&k, col_val);
            }
        }
        Ok(self)
    }

    /// Sets the row values from a pre-built Row.
    pub fn row(mut self, row: Row) -> Self {
        self.row = row;
        self
    }

    /// Executes the INSERT by adding the row to the write buffer.
    pub fn execute(self) -> Result<(), DbError> {
        self.ctx.buffer.insert(&self.table, self.row)
    }
}

/// Builder for UPDATE operations.
pub struct UpdateBuilder<'a> {
    ctx: &'a DbContext,
    table: String,
    row: Row,
    pk_value: Option<ColumnValue>,
}

impl<'a> UpdateBuilder<'a> {
    /// Sets a column value.
    pub fn set(mut self, col: &str, val: impl Into<ColumnValue>) -> Self {
        self.row.insert(col, val);
        self
    }

    /// Sets the primary key value to identify the row to update.
    pub fn where_pk(mut self, col: &str, val: impl Into<ColumnValue>) -> Self {
        let cv = val.into();
        self.row.insert(col, cv.clone());
        self.pk_value = Some(cv);
        self
    }

    /// Executes the UPDATE by adding it to the write buffer.
    pub fn execute(self) -> Result<(), DbError> {
        let pk = self.pk_value.unwrap_or(ColumnValue::Null);
        self.ctx.buffer.update(&self.table, self.row, pk)
    }
}

/// Builder for DELETE operations.
pub struct DeleteBuilder<'a> {
    ctx: &'a DbContext,
    table: String,
    pk_col: Option<String>,
    pk_value: Option<ColumnValue>,
}

impl<'a> DeleteBuilder<'a> {
    /// Sets the condition for the DELETE (primary key column and value).
    pub fn where_(mut self, col: &str, val: impl Into<ColumnValue>) -> Self {
        self.pk_col = Some(col.to_string());
        self.pk_value = Some(val.into());
        self
    }

    /// Executes the DELETE by adding it to the write buffer.
    pub fn execute(self) -> Result<(), DbError> {
        let col = self.pk_col.unwrap_or_default();
        let val = self.pk_value.unwrap_or(ColumnValue::Null);
        self.ctx.buffer.delete(&self.table, &col, val)
    }
}

/// Converts a serde_json::Value to a ColumnValue.
fn json_to_column_value(v: serde_json::Value) -> ColumnValue {
    match v {
        serde_json::Value::Null => ColumnValue::Null,
        serde_json::Value::Bool(b) => ColumnValue::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ColumnValue::BigInt(i)
            } else if let Some(f) = n.as_f64() {
                ColumnValue::Float(f)
            } else {
                ColumnValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => ColumnValue::Text(s),
        other => ColumnValue::Json(other),
    }
}
