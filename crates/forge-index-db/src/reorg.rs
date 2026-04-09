//! Reorg shadow table system for chain reorganization rollback.

use crate::error::DbError;
use crate::row::Row;
use sqlx::PgPool;

/// The type of operation recorded in the shadow table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// An INSERT was performed on the main table.
    Insert,
    /// An UPDATE was performed on the main table.
    Update,
    /// A DELETE was performed on the main table.
    Delete,
}

impl Operation {
    /// Returns the operation name as stored in the shadow table.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
        }
    }

    /// Returns the inverse operation (what to do to undo this operation).
    pub fn inverse(&self) -> &'static str {
        match self {
            Self::Insert => "DELETE",
            Self::Update => "UPDATE",
            Self::Delete => "INSERT",
        }
    }
}

/// Manages shadow tables for chain reorganization support.
///
/// Each main table has a `_reorg_{table}` shadow table that records the
/// inverse of every write operation. During a reorg, shadow rows are
/// replayed in reverse to undo the effects of orphaned blocks.
pub struct ReorgStore {
    pool: PgPool,
    pg_schema: String,
}

impl ReorgStore {
    /// Creates a new reorg store.
    pub fn new(pool: PgPool, pg_schema: String) -> Self {
        Self { pool, pg_schema }
    }

    /// Records a flush operation in the shadow table.
    ///
    /// For each row flushed to the main table, writes the inverse operation
    /// to the shadow table so it can be undone during a reorg.
    pub async fn record_flush(
        &self,
        table: &str,
        rows: &[Row],
        operation: Operation,
        block_number: u64,
        columns: &[String],
    ) -> Result<(), DbError> {
        let shadow_table = format!("_reorg_{}", table);
        let inverse_op = operation.inverse();

        for row in rows {
            let mut col_names: Vec<String> = columns.iter().map(|c| format!("\"{}\"", c)).collect();
            col_names.push("\"_operation\"".to_string());
            col_names.push("\"_block_number\"".to_string());

            let mut values: Vec<String> = columns
                .iter()
                .map(|c| {
                    row.get(c)
                        .map(Row::to_sql_literal)
                        .unwrap_or_else(|| "NULL".to_string())
                })
                .collect();
            values.push(format!("'{}'", inverse_op));
            values.push(block_number.to_string());

            let sql = format!(
                "INSERT INTO \"{}\".\"{}\" ({}) VALUES ({})",
                self.pg_schema,
                shadow_table,
                col_names.join(", "),
                values.join(", ")
            );

            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::Flush {
                    table: shadow_table.clone(),
                    source: e,
                })?;
        }

        Ok(())
    }

    /// Rolls back all operations at or after the given block number.
    ///
    /// Reads shadow rows in reverse order and replays them against the
    /// main table to undo the effects of orphaned blocks.
    pub async fn rollback_from_block(
        &self,
        table: &str,
        block_number: u64,
        pk_col: &str,
        columns: &[String],
    ) -> Result<usize, DbError> {
        let shadow_table = format!("_reorg_{}", table);

        // Read shadow rows for this block and beyond, in reverse order
        let col_list: String = columns
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "SELECT {}, \"_operation\", \"_block_number\" FROM \"{}\".\"{}\" \
             WHERE \"_block_number\" >= $1 ORDER BY \"_block_number\" DESC",
            col_list, self.pg_schema, shadow_table
        );

        let rows = sqlx::query(&query)
            .bind(block_number as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::Query {
                table: shadow_table.clone(),
                source: e,
            })?;

        let mut affected = 0;

        for row in &rows {
            let op: &str = sqlx::Row::get(row, "_operation");

            match op {
                "DELETE" => {
                    // Undo an INSERT: delete the row from main table
                    let pk_val: String = sqlx::Row::get(row, pk_col);
                    let sql = format!(
                        "DELETE FROM \"{}\".\"{}\" WHERE \"{}\" = '{}'",
                        self.pg_schema, table, pk_col, pk_val
                    );
                    sqlx::query(&sql)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| DbError::Flush {
                            table: table.to_string(),
                            source: e,
                        })?;
                }
                "INSERT" => {
                    // Undo a DELETE: re-insert the row
                    let col_list_quoted: String = columns
                        .iter()
                        .map(|c| format!("\"{}\"", c))
                        .collect::<Vec<_>>()
                        .join(", ");

                    let vals: Vec<String> = columns
                        .iter()
                        .map(|c| {
                            let val: Option<String> = sqlx::Row::try_get(row, c.as_str()).ok();
                            val.map(|v| format!("'{}'", v.replace('\'', "''")))
                                .unwrap_or_else(|| "NULL".to_string())
                        })
                        .collect();

                    let sql = format!(
                        "INSERT INTO \"{}\".\"{}\" ({}) VALUES ({}) ON CONFLICT DO NOTHING",
                        self.pg_schema,
                        table,
                        col_list_quoted,
                        vals.join(", ")
                    );
                    sqlx::query(&sql)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| DbError::Flush {
                            table: table.to_string(),
                            source: e,
                        })?;
                }
                "UPDATE" => {
                    // Undo an UPDATE: restore old values
                    let pk_val: String = sqlx::Row::get(row, pk_col);
                    let sets: Vec<String> = columns
                        .iter()
                        .filter(|c| c.as_str() != pk_col)
                        .map(|c| {
                            let val: Option<String> = sqlx::Row::try_get(row, c.as_str()).ok();
                            let val_str = val
                                .map(|v| format!("'{}'", v.replace('\'', "''")))
                                .unwrap_or_else(|| "NULL".to_string());
                            format!("\"{}\" = {}", c, val_str)
                        })
                        .collect();

                    if !sets.is_empty() {
                        let sql = format!(
                            "UPDATE \"{}\".\"{}\" SET {} WHERE \"{}\" = '{}'",
                            self.pg_schema,
                            table,
                            sets.join(", "),
                            pk_col,
                            pk_val
                        );
                        sqlx::query(&sql).execute(&self.pool).await.map_err(|e| {
                            DbError::Flush {
                                table: table.to_string(),
                                source: e,
                            }
                        })?;
                    }
                }
                _ => {}
            }

            affected += 1;
        }

        // Clean up applied shadow rows
        let cleanup = format!(
            "DELETE FROM \"{}\".\"{}\" WHERE \"_block_number\" >= $1",
            self.pg_schema, shadow_table
        );
        sqlx::query(&cleanup)
            .bind(block_number as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Flush {
                table: shadow_table,
                source: e,
            })?;

        Ok(affected)
    }

    /// Prunes shadow rows older than the given block number.
    pub async fn clear_before_block(&self, table: &str, block_number: u64) -> Result<(), DbError> {
        let shadow_table = format!("_reorg_{}", table);
        let sql = format!(
            "DELETE FROM \"{}\".\"{}\" WHERE \"_block_number\" < $1",
            self.pg_schema, shadow_table
        );
        sqlx::query(&sql)
            .bind(block_number as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Flush {
                table: shadow_table,
                source: e,
            })?;
        Ok(())
    }
}
