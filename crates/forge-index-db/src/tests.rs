//! Integration tests for the database layer (require Docker for testcontainers).

use crate::buffer::WriteBuffer;
use crate::context::DbContext;
use crate::manager::{BuildIdStatus, DatabaseManager};
use crate::reorg::{Operation, ReorgStore};
use crate::row::{ColumnValue, Row};
use forge_index_config::{ColumnType, Schema, SchemaBuilder};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

fn test_schema() -> Schema {
    SchemaBuilder::new()
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("from_addr", ColumnType::Address)
                .not_null()
                .column("to_addr", ColumnType::Address)
                .not_null()
                .column("amount", ColumnType::BigInt)
                .not_null()
                .column("block_number", ColumnType::BigInt)
                .not_null()
        })
        .build()
}

async fn setup_pg() -> (PgPool, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_host_auth()
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let url = format!("postgres://postgres@127.0.0.1:{}/postgres", port);

    let pool = loop {
        match PgPool::connect(&url).await {
            Ok(pool) => break pool,
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
        }
    };

    (pool, container)
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn database_manager_setup_creates_all_tables_and_forge_meta() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "test_schema");
    let schema = test_schema();

    mgr.setup(&schema, "test_schema").await.unwrap();

    // Verify tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'test_schema' ORDER BY table_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();
    assert!(names.contains(&"transfers"), "missing transfers table");
    assert!(
        names.contains(&"_reorg_transfers"),
        "missing _reorg_transfers shadow table"
    );
    assert!(names.contains(&"_forge_meta"), "missing _forge_meta table");

    // Verify build_id is stored
    let (val,): (String,) =
        sqlx::query_as("SELECT value FROM test_schema._forge_meta WHERE key = 'build_id'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(val, schema.build_id());

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn setup_on_already_locked_schema_returns_schema_locked() {
    let (pool, _container) = setup_pg().await;
    let schema = test_schema();

    let mgr1 = DatabaseManager::from_pool(pool.clone(), "locked_schema");
    mgr1.setup(&schema, "locked_schema").await.unwrap();

    // Second connection should fail to acquire the lock
    let pool2 = sqlx::PgPool::connect(&format!(
        "postgres://postgres@127.0.0.1:{}/postgres",
        _container.get_host_port_ipv4(5432).await.unwrap()
    ))
    .await
    .unwrap();
    let mgr2 = DatabaseManager::from_pool(pool2, "locked_schema");
    let result = mgr2.setup(&schema, "locked_schema").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        crate::error::DbError::SchemaLocked { schema } => {
            assert_eq!(schema, "locked_schema");
        }
        other => panic!("Expected SchemaLocked, got: {:?}", other),
    }

    mgr1.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn check_build_id_returns_correct_statuses() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "build_id_test");
    let schema1 = test_schema();

    // Not found on fresh DB
    let status = mgr.check_build_id(&schema1, "build_id_test").await.unwrap();
    assert_eq!(status, BuildIdStatus::NotFound);

    // Same after setup
    mgr.setup(&schema1, "build_id_test").await.unwrap();
    let status = mgr.check_build_id(&schema1, "build_id_test").await.unwrap();
    assert_eq!(status, BuildIdStatus::Same);

    // Changed after schema change
    let schema2 = SchemaBuilder::new()
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("extra", ColumnType::Text)
                .nullable()
        })
        .build();
    let status = mgr.check_build_id(&schema2, "build_id_test").await.unwrap();
    match status {
        BuildIdStatus::Changed { old, new } => {
            assert_eq!(old, schema1.build_id());
            assert_eq!(new, schema2.build_id());
        }
        other => panic!("Expected Changed, got: {:?}", other),
    }

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn write_buffer_insert_then_flush_writes_rows_to_db() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "buf_test");
    let schema = test_schema();
    mgr.setup(&schema, "buf_test").await.unwrap();

    let buffer = WriteBuffer::new(pool.clone(), "buf_test".to_string(), &schema);

    let mut row = Row::new();
    row.insert("id", "tx1");
    row.insert("from_addr", "0xAAA");
    row.insert("to_addr", "0xBBB");
    row.insert("amount", ColumnValue::BigNumeric("1000".to_string()));
    row.insert("block_number", ColumnValue::BigInt(42));

    buffer.insert("transfers", row).unwrap();
    buffer.flush_table("transfers").await.unwrap();

    // Verify in DB
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM buf_test.transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let (id,): (String,) = sqlx::query_as("SELECT id FROM buf_test.transfers WHERE id = 'tx1'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(id, "tx1");

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn write_buffer_flush_row_count_correct() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "flush_count");
    let schema = test_schema();
    mgr.setup(&schema, "flush_count").await.unwrap();

    let buffer = WriteBuffer::new(pool.clone(), "flush_count".to_string(), &schema);

    for i in 0..5 {
        let mut row = Row::new();
        row.insert("id", format!("tx{}", i));
        row.insert("from_addr", "0xAAA");
        row.insert("to_addr", "0xBBB");
        row.insert("amount", ColumnValue::BigNumeric("100".to_string()));
        row.insert("block_number", ColumnValue::BigInt(i as i64));
        buffer.insert("transfers", row).unwrap();
    }

    let count = buffer.flush_table("transfers").await.unwrap();
    assert_eq!(count, 5);

    let (db_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM flush_count.transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_count, 5);

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn write_buffer_background_flush_fires() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "bg_flush");
    let schema = test_schema();
    mgr.setup(&schema, "bg_flush").await.unwrap();

    let buffer = Arc::new(WriteBuffer::new(
        pool.clone(),
        "bg_flush".to_string(),
        &schema,
    ));

    let handle = buffer.clone().start_background_flush();

    let mut row = Row::new();
    row.insert("id", "bg1");
    row.insert("from_addr", "0xAAA");
    row.insert("to_addr", "0xBBB");
    row.insert("amount", ColumnValue::BigNumeric("500".to_string()));
    row.insert("block_number", ColumnValue::BigInt(99));
    buffer.insert("transfers", row).unwrap();

    // Wait for background flush to fire (interval is 500ms)
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bg_flush.transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "background flush should have written the row");

    handle.abort();
    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn write_buffer_read_one_returns_buffered_row() {
    let (pool, _container) = setup_pg().await;
    let schema = test_schema();
    let buffer = WriteBuffer::new(pool, "test".to_string(), &schema);

    let mut row = Row::new();
    row.insert("id", "r1");
    row.insert("from_addr", "0xAAA");
    row.insert("to_addr", "0xBBB");
    row.insert("amount", ColumnValue::BigNumeric("999".to_string()));
    row.insert("block_number", ColumnValue::BigInt(1));
    buffer.insert("transfers", row).unwrap();

    let found = buffer.read_one("transfers", "id", &ColumnValue::Text("r1".to_string()));
    assert!(found.is_some());
    assert_eq!(
        found.unwrap().get("from_addr"),
        Some(&ColumnValue::Text("0xAAA".to_string()))
    );

    // Miss
    let not_found = buffer.read_one("transfers", "id", &ColumnValue::Text("nope".to_string()));
    assert!(not_found.is_none());
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn reorg_rollback_deletes_rows_at_block() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "reorg_test");
    let schema = test_schema();
    mgr.setup(&schema, "reorg_test").await.unwrap();

    let buffer = WriteBuffer::new(pool.clone(), "reorg_test".to_string(), &schema);
    let reorg_store = ReorgStore::new(pool.clone(), "reorg_test".to_string());

    // Insert a row at block 100
    let mut row = Row::new();
    row.insert("id", "r1");
    row.insert("from_addr", "0xAAA");
    row.insert("to_addr", "0xBBB");
    row.insert("amount", ColumnValue::BigNumeric("100".to_string()));
    row.insert("block_number", ColumnValue::BigInt(100));
    buffer.insert("transfers", row.clone()).unwrap();
    buffer.flush_table("transfers").await.unwrap();

    // Record the insert in the shadow table
    let columns: Vec<String> = vec!["id", "from_addr", "to_addr", "amount", "block_number"]
        .into_iter()
        .map(String::from)
        .collect();
    reorg_store
        .record_flush("transfers", &[row], Operation::Insert, 100, &columns)
        .await
        .unwrap();

    // Verify row exists
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reorg_test.transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Rollback to block 99 (undo block 100)
    let affected = reorg_store
        .rollback_from_block("transfers", 100, "id", &columns)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    // Row should be gone
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reorg_test.transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn reorg_clear_before_block_removes_old_shadow_rows() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "reorg_clear");
    let schema = test_schema();
    mgr.setup(&schema, "reorg_clear").await.unwrap();

    let reorg_store = ReorgStore::new(pool.clone(), "reorg_clear".to_string());
    let columns: Vec<String> = vec!["id", "from_addr", "to_addr", "amount", "block_number"]
        .into_iter()
        .map(String::from)
        .collect();

    // Record flushes at blocks 10, 20, 30
    for block in [10u64, 20, 30] {
        let mut row = Row::new();
        row.insert("id", format!("r{}", block));
        row.insert("from_addr", "0xAAA");
        row.insert("to_addr", "0xBBB");
        row.insert("amount", ColumnValue::BigNumeric("100".to_string()));
        row.insert("block_number", ColumnValue::BigInt(block as i64));
        reorg_store
            .record_flush("transfers", &[row], Operation::Insert, block, &columns)
            .await
            .unwrap();
    }

    // Clear rows before block 25
    reorg_store
        .clear_before_block("transfers", 25)
        .await
        .unwrap();

    // Only block 30 should remain
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reorg_clear._reorg_transfers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "only block 30 shadow row should remain");

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn db_context_insert_then_find_one_returns_same_row() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "ctx_test");
    let schema = test_schema();
    mgr.setup(&schema, "ctx_test").await.unwrap();

    let buffer = Arc::new(WriteBuffer::new(
        pool.clone(),
        "ctx_test".to_string(),
        &schema,
    ));
    let ctx = DbContext::new(buffer.clone(), pool.clone(), "ctx_test".to_string());

    // Insert using the fluent API
    let mut row = Row::new();
    row.insert("id", "find1");
    row.insert("from_addr", "0xAAA");
    row.insert("to_addr", "0xBBB");
    row.insert("amount", ColumnValue::BigNumeric("42".to_string()));
    row.insert("block_number", ColumnValue::BigInt(1));
    buffer.insert("transfers", row).unwrap();
    buffer.flush_all().await.unwrap();

    // Query it
    #[derive(Debug, serde::Deserialize)]
    struct Transfer {
        id: String,
        from_addr: String,
        to_addr: String,
    }

    let result: Option<Transfer> = ctx
        .find_one("transfers")
        .where_("id", "=", ColumnValue::Text("find1".to_string()))
        .first()
        .await
        .unwrap();

    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.id, "find1");
    assert_eq!(t.from_addr, "0xAAA");

    mgr.release_lock().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn db_context_find_many_with_limit_returns_correct_count() {
    let (pool, _container) = setup_pg().await;
    let mgr = DatabaseManager::from_pool(pool.clone(), "ctx_many");
    let schema = test_schema();
    mgr.setup(&schema, "ctx_many").await.unwrap();

    let buffer = Arc::new(WriteBuffer::new(
        pool.clone(),
        "ctx_many".to_string(),
        &schema,
    ));
    let ctx = DbContext::new(buffer.clone(), pool.clone(), "ctx_many".to_string());

    for i in 0..10 {
        let mut row = Row::new();
        row.insert("id", format!("m{}", i));
        row.insert("from_addr", "0xAAA");
        row.insert("to_addr", "0xBBB");
        row.insert("amount", ColumnValue::BigNumeric(i.to_string()));
        row.insert("block_number", ColumnValue::BigInt(i));
        buffer.insert("transfers", row).unwrap();
    }
    buffer.flush_all().await.unwrap();

    #[derive(Debug, serde::Deserialize)]
    struct Transfer {
        id: String,
    }

    let results: Vec<Transfer> = ctx.find_many("transfers").limit(5).all().await.unwrap();

    assert_eq!(results.len(), 5);

    mgr.release_lock().await.unwrap();
}
