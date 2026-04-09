//! Database manager — setup, locking, and migrations.

use crate::error::DbError;
use forge_index_config::{DatabaseConfig, Schema};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// Status of the build ID comparison.
#[derive(Debug, PartialEq, Eq)]
pub enum BuildIdStatus {
    /// Build ID matches the previously stored one.
    Same,
    /// Build ID has changed since last run.
    Changed {
        /// The previously stored build ID.
        old: String,
        /// The new build ID.
        new: String,
    },
    /// No build ID has been stored yet (fresh database).
    NotFound,
}

/// Manages database setup, schema migrations, and advisory locking.
pub struct DatabaseManager {
    pool: PgPool,
    lock_key: i64,
    #[allow(dead_code)]
    pg_schema: String,
}

impl DatabaseManager {
    /// Creates a new database manager from the given configuration.
    pub async fn new(config: &DatabaseConfig) -> Result<Self, DbError> {
        match config {
            DatabaseConfig::Postgres {
                connection_string,
                schema,
                pool_max_connections,
            } => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(*pool_max_connections)
                    .connect(connection_string)
                    .await
                    .map_err(|e| DbError::PoolCreation(e.to_string()))?;

                let lock_key = schema_lock_key(schema);

                Ok(Self {
                    pool,
                    lock_key,
                    pg_schema: schema.clone(),
                })
            }
            DatabaseConfig::PGlite { .. } => Err(DbError::PoolCreation(
                "PGlite is not yet supported".to_string(),
            )),
        }
    }

    /// Creates a database manager from an existing pool (for testing).
    pub fn from_pool(pool: PgPool, pg_schema: &str) -> Self {
        Self {
            pool: pool.clone(),
            lock_key: schema_lock_key(pg_schema),
            pg_schema: pg_schema.to_string(),
        }
    }

    /// Runs all schema setup: create schema, acquire lock, run migrations,
    /// create meta table, and store the build ID.
    pub async fn setup(&self, schema: &Schema, pg_schema: &str) -> Result<(), DbError> {
        // 1. Create schema
        let create_schema = format!("CREATE SCHEMA IF NOT EXISTS \"{}\"", pg_schema);
        sqlx::query(&create_schema)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Migration(e.to_string()))?;

        // 2. Acquire advisory lock
        let lock_key = schema_lock_key(pg_schema);
        let locked: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(lock_key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::Migration(e.to_string()))?;

        if !locked.0 {
            return Err(DbError::SchemaLocked {
                schema: pg_schema.to_string(),
            });
        }

        // 3. Run all CREATE TABLE / CREATE INDEX statements
        let stmts = schema.to_create_sql(pg_schema);
        for stmt in &stmts {
            sqlx::query(stmt)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::Migration(format!("{}: {}", stmt, e)))?;
        }

        // 4. Create _forge_meta table
        let meta_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS "{}"."{}" (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"#,
            pg_schema, "_forge_meta"
        );
        sqlx::query(&meta_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Migration(e.to_string()))?;

        // 5. Upsert build_id
        let build_id = schema.build_id();
        let upsert_sql = format!(
            r#"INSERT INTO "{}"."_forge_meta" (key, value) VALUES ('build_id', $1)
               ON CONFLICT (key) DO UPDATE SET value = $1"#,
            pg_schema
        );
        sqlx::query(&upsert_sql)
            .bind(&build_id)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Migration(e.to_string()))?;

        Ok(())
    }

    /// Releases the advisory lock. Call on graceful shutdown.
    pub async fn release_lock(&self) -> Result<(), DbError> {
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(self.lock_key)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Returns the connection pool.
    pub fn get_pool(&self) -> PgPool {
        self.pool.clone()
    }

    /// Checks whether the schema's build ID matches what's stored in the database.
    pub async fn check_build_id(
        &self,
        schema: &Schema,
        pg_schema: &str,
    ) -> Result<BuildIdStatus, DbError> {
        let new_id = schema.build_id();

        let query = format!(
            r#"SELECT value FROM "{}"."_forge_meta" WHERE key = 'build_id'"#,
            pg_schema
        );

        let result: Result<(String,), _> = sqlx::query_as(&query).fetch_one(&self.pool).await;

        match result {
            Ok((old_id,)) => {
                if old_id == new_id {
                    Ok(BuildIdStatus::Same)
                } else {
                    Ok(BuildIdStatus::Changed {
                        old: old_id,
                        new: new_id,
                    })
                }
            }
            Err(sqlx::Error::RowNotFound) => Ok(BuildIdStatus::NotFound),
            Err(sqlx::Error::Database(e)) if e.message().contains("does not exist") => {
                Ok(BuildIdStatus::NotFound)
            }
            Err(e) => Err(DbError::Query {
                table: "_forge_meta".to_string(),
                source: e,
            }),
        }
    }
}

/// Computes a stable i64 advisory lock key from a schema name.
fn schema_lock_key(schema: &str) -> i64 {
    let mut hasher = Sha256::new();
    hasher.update(b"forge-index-lock:");
    hasher.update(schema.as_bytes());
    let hash = hasher.finalize();
    i64::from_be_bytes(hash[..8].try_into().unwrap())
}
