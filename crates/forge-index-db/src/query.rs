//! Query builder for find_one / find_many operations.

use crate::error::DbError;
use crate::row::ColumnValue;
use serde::de::DeserializeOwned;
use sqlx::PgPool;

/// Sort direction for ORDER BY clauses.
#[derive(Debug, Clone, Copy)]
pub enum Dir {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// A fluent query builder for SELECT operations.
pub struct QueryBuilder<T> {
    pool: PgPool,
    pg_schema: String,
    table: String,
    conditions: Vec<String>,
    order_by: Vec<(String, Dir)>,
    limit: Option<i64>,
    offset: Option<i64>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned> QueryBuilder<T> {
    /// Creates a new query builder.
    pub fn new(pool: PgPool, pg_schema: String, table: String) -> Self {
        Self {
            pool,
            pg_schema,
            table,
            conditions: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Adds a WHERE condition.
    pub fn where_(mut self, col: &str, op: &str, val: ColumnValue) -> Self {
        let literal = crate::row::Row::to_sql_literal(&val);
        self.conditions
            .push(format!("\"{}\" {} {}", col, op, literal));
        self
    }

    /// Adds an ORDER BY clause.
    pub fn order_by(mut self, col: &str, dir: Dir) -> Self {
        self.order_by.push((col.to_string(), dir));
        self
    }

    /// Sets the LIMIT.
    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Sets the OFFSET.
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }

    fn build_sql(&self) -> String {
        let mut sql = format!(
            "SELECT row_to_json(t.*) as doc FROM \"{}\".\"{}\" t",
            self.pg_schema, self.table
        );

        if !self.conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.conditions.join(" AND "));
        }

        if !self.order_by.is_empty() {
            sql.push_str(" ORDER BY ");
            let parts: Vec<String> = self
                .order_by
                .iter()
                .map(|(col, dir)| {
                    let d = match dir {
                        Dir::Asc => "ASC",
                        Dir::Desc => "DESC",
                    };
                    format!("\"{}\" {}", col, d)
                })
                .collect();
            sql.push_str(&parts.join(", "));
        }

        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        sql
    }

    /// Executes the query and returns the first result.
    pub async fn first(mut self) -> Result<Option<T>, DbError> {
        self.limit = Some(1);
        let sql = self.build_sql();
        let row: Option<(serde_json::Value,)> = sqlx::query_as(&sql)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DbError::Query {
                table: self.table.clone(),
                source: e,
            })?;

        match row {
            Some((json,)) => {
                let val: T = serde_json::from_value(json)?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Executes the query and returns all results.
    pub async fn all(self) -> Result<Vec<T>, DbError> {
        let sql = self.build_sql();
        let rows: Vec<(serde_json::Value,)> = sqlx::query_as(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::Query {
                table: self.table.clone(),
                source: e,
            })?;

        let mut results = Vec::with_capacity(rows.len());
        for (json,) in rows {
            results.push(serde_json::from_value(json)?);
        }
        Ok(results)
    }
}
