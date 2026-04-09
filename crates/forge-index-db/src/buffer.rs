//! In-memory write buffer with bulk flush to Postgres.

use crate::error::DbError;
use crate::row::{ColumnValue, Row};
use dashmap::DashMap;
use forge_index_config::Schema;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Statistics about a flush operation.
#[derive(Debug, Default)]
pub struct FlushStats {
    /// Number of rows flushed per table.
    pub rows_per_table: HashMap<String, usize>,
}

/// Metrics tracked by the write buffer.
pub struct BufferMetrics {
    /// Total rows flushed across all tables.
    pub total_flushed: AtomicU64,
}

/// In-memory write buffer that batches row operations and flushes to Postgres.
///
/// Supports INSERT, UPDATE, and DELETE operations. Flushes use batched SQL
/// for efficiency.
pub struct WriteBuffer {
    buffers: DashMap<String, Vec<Row>>,
    max_size: usize,
    flush_interval: Duration,
    pool: PgPool,
    pg_schema: String,
    /// Column names for each table, in order.
    table_columns: HashMap<String, Vec<String>>,
    /// Primary key column for each table.
    pk_columns: HashMap<String, String>,
    /// Public metrics.
    pub metrics: Arc<BufferMetrics>,
}

impl WriteBuffer {
    /// Creates a new write buffer for the given schema.
    pub fn new(pool: PgPool, pg_schema: String, schema: &Schema) -> Self {
        let mut table_columns = HashMap::new();
        let mut pk_columns = HashMap::new();

        for table in &schema.tables {
            let cols: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
            if let Some(pk) = table.columns.iter().find(|c| c.primary_key) {
                pk_columns.insert(table.name.clone(), pk.name.clone());
            }
            table_columns.insert(table.name.clone(), cols);
        }

        Self {
            buffers: DashMap::new(),
            max_size: 10_000,
            flush_interval: Duration::from_millis(500),
            pool,
            pg_schema,
            table_columns,
            pk_columns,
            metrics: Arc::new(BufferMetrics {
                total_flushed: AtomicU64::new(0),
            }),
        }
    }

    /// Inserts a row into the write buffer for the given table.
    pub fn insert(&self, table: &str, row: Row) -> Result<(), DbError> {
        let mut entry = self.buffers.entry(table.to_string()).or_default();
        if entry.len() >= self.max_size {
            return Err(DbError::BufferFull {
                table: table.to_string(),
            });
        }
        entry.push(row);
        Ok(())
    }

    /// Marks a row for UPDATE in the write buffer.
    pub fn update(&self, table: &str, mut row: Row, _pk_value: ColumnValue) -> Result<(), DbError> {
        row.operation = Some("UPDATE".to_string());
        let mut entry = self.buffers.entry(table.to_string()).or_default();
        entry.push(row);
        Ok(())
    }

    /// Marks a row for DELETE in the write buffer.
    pub fn delete(&self, table: &str, pk_col: &str, pk_value: ColumnValue) -> Result<(), DbError> {
        let mut row = Row::new();
        row.insert(pk_col, pk_value);
        row.operation = Some("DELETE".to_string());
        let mut entry = self.buffers.entry(table.to_string()).or_default();
        entry.push(row);
        Ok(())
    }

    /// Flushes all tables and returns statistics.
    pub async fn flush_all(&self) -> Result<FlushStats, DbError> {
        let mut stats = FlushStats::default();
        let table_names: Vec<String> = self.buffers.iter().map(|e| e.key().clone()).collect();

        for table in &table_names {
            let count = self.flush_table(table).await?;
            if count > 0 {
                stats.rows_per_table.insert(table.clone(), count);
            }
        }

        Ok(stats)
    }

    /// Flushes a single table's buffer to Postgres.
    pub async fn flush_table(&self, table: &str) -> Result<usize, DbError> {
        let rows = {
            let mut entry = match self.buffers.get_mut(table) {
                Some(e) => e,
                None => return Ok(0),
            };
            std::mem::take(entry.value_mut())
        };

        if rows.is_empty() {
            return Ok(0);
        }

        let count = rows.len();
        let columns = self.table_columns.get(table);
        let pk_col = self.pk_columns.get(table);

        let mut inserts = Vec::new();
        let mut updates = Vec::new();
        let mut deletes = Vec::new();

        for row in &rows {
            match row.operation.as_deref() {
                Some("UPDATE") => updates.push(row),
                Some("DELETE") => deletes.push(row),
                _ => inserts.push(row),
            }
        }

        // INSERT rows via batched INSERT
        if !inserts.is_empty() {
            if let Some(cols) = columns {
                self.flush_inserts(table, cols, &inserts).await?;
            }
        }

        // UPDATE rows via batched UPDATE
        if !updates.is_empty() {
            if let (Some(cols), Some(pk)) = (columns, pk_col) {
                self.flush_updates(table, cols, pk, &updates).await?;
            }
        }

        // DELETE rows
        if !deletes.is_empty() {
            if let Some(pk) = pk_col {
                self.flush_deletes(table, pk, &deletes).await?;
            }
        }

        self.metrics
            .total_flushed
            .fetch_add(count as u64, Ordering::Relaxed);

        Ok(count)
    }

    async fn flush_inserts(
        &self,
        table: &str,
        columns: &[String],
        rows: &[&Row],
    ) -> Result<(), DbError> {
        if rows.is_empty() {
            return Ok(());
        }

        // Build batched INSERT with VALUES
        let col_list: String = columns
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(", ");

        let mut value_rows = Vec::new();
        for row in rows {
            let vals: Vec<String> = columns
                .iter()
                .map(|col| {
                    row.get(col)
                        .map(Row::to_sql_literal)
                        .unwrap_or_else(|| "NULL".to_string())
                })
                .collect();
            value_rows.push(format!("({})", vals.join(", ")));
        }

        let sql = format!(
            "INSERT INTO \"{}\".\"{}\" ({}) VALUES {} ON CONFLICT DO NOTHING",
            self.pg_schema,
            table,
            col_list,
            value_rows.join(", ")
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Flush {
                table: table.to_string(),
                source: e,
            })?;

        Ok(())
    }

    async fn flush_updates(
        &self,
        table: &str,
        columns: &[String],
        pk_col: &str,
        rows: &[&Row],
    ) -> Result<(), DbError> {
        for row in rows {
            let pk_val = row
                .get(pk_col)
                .map(Row::to_sql_literal)
                .unwrap_or_else(|| "NULL".to_string());

            let sets: Vec<String> = columns
                .iter()
                .filter(|c| c.as_str() != pk_col)
                .filter_map(|c| {
                    row.get(c)
                        .map(|v| format!("\"{}\" = {}", c, Row::to_sql_literal(v)))
                })
                .collect();

            if sets.is_empty() {
                continue;
            }

            let sql = format!(
                "UPDATE \"{}\".\"{}\" SET {} WHERE \"{}\" = {}",
                self.pg_schema,
                table,
                sets.join(", "),
                pk_col,
                pk_val
            );

            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::Flush {
                    table: table.to_string(),
                    source: e,
                })?;
        }
        Ok(())
    }

    async fn flush_deletes(&self, table: &str, pk_col: &str, rows: &[&Row]) -> Result<(), DbError> {
        let pk_vals: Vec<String> = rows
            .iter()
            .filter_map(|r| r.get(pk_col).map(Row::to_sql_literal))
            .collect();

        if pk_vals.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "DELETE FROM \"{}\".\"{}\" WHERE \"{}\" IN ({})",
            self.pg_schema,
            table,
            pk_col,
            pk_vals.join(", ")
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Flush {
                table: table.to_string(),
                source: e,
            })?;

        Ok(())
    }

    /// Reads a single row from the buffer by primary key (before flushing to DB).
    pub fn read_one(&self, table: &str, pk_col: &str, pk_value: &ColumnValue) -> Option<Row> {
        let entry = self.buffers.get(table)?;
        entry
            .iter()
            .rev()
            .find(|r| r.operation.is_none() && r.get(pk_col) == Some(pk_value))
            .cloned()
    }

    /// Spawns a background task that flushes all buffers at the configured interval.
    pub fn start_background_flush(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.flush_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Err(e) = self.flush_all().await {
                    tracing::error!(error = %e, "Background flush failed");
                }
            }
        })
    }
}
