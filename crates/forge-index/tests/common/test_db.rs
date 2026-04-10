//! Reusable testcontainers Postgres setup for integration tests.

use forge_index_config::Schema;
use forge_index_db::manager::DatabaseManager;
use forge_index_rpc::RpcCacheStore;
use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// A test database backed by a testcontainers Postgres instance.
pub struct TestDb {
    /// The running container (must be held alive for the duration of the test).
    pub _container: testcontainers::ContainerAsync<Postgres>,
    /// A connection pool to the test database.
    pub pool: PgPool,
    /// The connection string for this database.
    pub connection_string: String,
}

impl TestDb {
    /// Starts a new Postgres container and runs ponder_sync migrations.
    pub async fn new() -> Self {
        let container = Postgres::default()
            .with_host_auth()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
        let connection_string = format!("postgres://postgres@127.0.0.1:{}/postgres", port);

        let pool = loop {
            match PgPool::connect(&connection_string).await {
                Ok(pool) => break pool,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            }
        };

        // Run ponder_sync migrations (checkpoints, logs cache, etc.)
        let cache_store = RpcCacheStore::new(pool.clone());
        cache_store
            .setup()
            .await
            .expect("Failed to run cache migrations");

        Self {
            _container: container,
            pool,
            connection_string,
        }
    }

    /// Sets up application schema tables using the DatabaseManager.
    pub async fn setup_schema(&self, schema: &Schema) -> DatabaseManager {
        let mgr = DatabaseManager::from_pool(self.pool.clone(), "public");
        mgr.setup(schema, "public")
            .await
            .expect("Failed to set up schema");
        mgr
    }

    /// Counts rows in the given table.
    pub async fn count_rows(&self, table: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) as cnt FROM \"{}\"", table);
        let row: (i64,) = sqlx::query_as(&sql)
            .fetch_one(&self.pool)
            .await
            .expect("Failed to count rows");
        row.0
    }

    /// Executes a query and returns the first row as JSON.
    pub async fn query_row(&self, sql: &str) -> Option<serde_json::Value> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as(&format!("SELECT row_to_json(t) FROM ({}) t LIMIT 1", sql))
                .fetch_optional(&self.pool)
                .await
                .expect("Failed to query row");
        row.map(|(v,)| v)
    }

    /// Truncates all user tables (non-internal).
    pub async fn truncate_all(&self) {
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' AND tablename NOT LIKE 'pg_%'",
        )
        .fetch_all(&self.pool)
        .await
        .expect("Failed to list tables");

        for (table,) in &tables {
            let sql = format!("TRUNCATE \"{}\" CASCADE", table);
            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .expect("Failed to truncate table");
        }
    }

    /// Returns the checkpoint value for a contract.
    pub async fn get_checkpoint(&self, chain_id: u64, contract: &str) -> Option<u64> {
        let cache_store = RpcCacheStore::new(self.pool.clone());
        cache_store
            .get_checkpoint(chain_id, contract)
            .await
            .ok()
            .flatten()
    }

    /// Sets a checkpoint value for a contract.
    pub async fn set_checkpoint(&self, chain_id: u64, contract: &str, block: u64) {
        let cache_store = RpcCacheStore::new(self.pool.clone());
        cache_store
            .put_checkpoint(chain_id, contract, block)
            .await
            .expect("Failed to set checkpoint");
    }
}
