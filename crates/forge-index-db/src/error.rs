//! Database error types.

/// Errors originating from the database layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The schema is locked by another process.
    #[error("Schema locked: {schema}")]
    SchemaLocked {
        /// The locked schema name.
        schema: String,
    },

    /// Failed to create the connection pool.
    #[error("Pool creation failed: {0}")]
    PoolCreation(String),

    /// A migration statement failed.
    #[error("Migration failed: {0}")]
    Migration(String),

    /// Flushing the write buffer failed.
    #[error("Flush failed for table '{table}': {source}")]
    Flush {
        /// The table being flushed.
        table: String,
        /// The underlying database error.
        source: sqlx::Error,
    },

    /// A query failed.
    #[error("Query failed on table '{table}': {source}")]
    Query {
        /// The table being queried.
        table: String,
        /// The underlying database error.
        source: sqlx::Error,
    },

    /// Serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The write buffer for a table is full.
    #[error("Buffer full for table '{table}'")]
    BufferFull {
        /// The table whose buffer is full.
        table: String,
    },

    /// A raw sqlx error.
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}
