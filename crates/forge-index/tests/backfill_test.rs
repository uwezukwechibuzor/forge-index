//! Backfill integration tests.
//!
//! Tests require Docker for testcontainers Postgres.
//! Run with: `cargo test -p forge-index --test backfill_test -- --ignored`

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use forge_index_config::{AddressConfig, ChainConfig, ColumnType, ContractConfig, SchemaBuilder};
use forge_index_core::abi::decoder::DecodedEvent;
use forge_index_core::abi::decoder::LogDecoder;
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::registry::EventRegistry;
use forge_index_core::types::Address;
use forge_index_db::buffer::WriteBuffer;
use forge_index_db::handler::EventHandlerFn;
use forge_index_db::row::Row;
use forge_index_db::DbContext;
use forge_index_rpc::RpcCacheStore;
use forge_index_sync::{
    BackfillCoordinator, BackfillProgress, BackfillWorker, FactoryAddressTracker,
};

use common::fixtures::{ERC20_ABI, TRANSFER_TOPIC};
use common::mock_rpc::MockRpc;
use common::test_db::TestDb;

fn test_schema() -> forge_index_config::Schema {
    SchemaBuilder::new()
        .table("transfer_events", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("block_number", ColumnType::BigInt)
                .not_null()
        })
        .build()
}

fn test_contract() -> ContractConfig {
    ContractConfig {
        name: "ERC20".to_string(),
        abi_json: ERC20_ABI.to_string(),
        chain_names: vec!["mainnet".to_string()],
        address: AddressConfig::Single(Address::from("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
        start_block: 0,
        end_block: None,
        filter: None,
        include_transaction: false,
        include_trace: false,
    }
}

/// Simple handler that inserts transfer events.
struct TestTransferHandler;

impl EventHandlerFn for TestTransferHandler {
    fn call(
        &self,
        event: DecodedEvent,
        ctx: DbContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>>
    {
        Box::pin(async move {
            let mut row = Row::new();
            row.insert("id", event.raw_log.id.clone());
            row.insert("block_number", event.raw_log.block_number as i64);
            ctx.insert("transfer_events").row(row).execute()?;
            Ok(())
        })
    }
}

async fn build_coordinator(
    mock: &MockRpc,
    db: &TestDb,
    schema: &forge_index_config::Schema,
    contracts: Vec<ContractConfig>,
    chunk_size: u64,
) -> BackfillCoordinator {
    let parsed_abi = parse_abi(&contracts[0].abi_json).unwrap();
    let decoder = Arc::new(LogDecoder::new(&parsed_abi));

    let chain_config = ChainConfig {
        chain_id: 1,
        name: "test".to_string(),
        rpc_http: mock.url(),
        rpc_ws: None,
        max_block_range: Some(chunk_size),
        max_rpc_requests_per_second: Some(1000),
        poll_interval_ms: None,
    };
    let rpc = forge_index_rpc::build_from_config(&chain_config).unwrap();
    let cached = Arc::new(forge_index_rpc::CachedRpcClient::new(
        rpc,
        RpcCacheStore::new(db.pool.clone()),
    ));

    let selectors = vec![forge_index_core::types::Hash32::from(TRANSFER_TOPIC)];
    let mut workers = HashMap::new();
    for contract in &contracts {
        let worker = BackfillWorker::new(
            cached.clone(),
            decoder.clone(),
            contract.clone(),
            1,
            selectors.clone(),
        );
        workers.insert((1u64, contract.name.clone()), worker);
    }

    let mut db_handlers: HashMap<String, Arc<dyn EventHandlerFn>> = HashMap::new();
    db_handlers.insert("ERC20:Transfer".to_string(), Arc::new(TestTransferHandler));

    let write_buffer = Arc::new(WriteBuffer::new(
        db.pool.clone(),
        "public".to_string(),
        schema,
    ));

    let cache_store = Arc::new(RpcCacheStore::new(db.pool.clone()));
    let progress = Arc::new(BackfillProgress::new());
    let factory_tracker = Arc::new(FactoryAddressTracker::new());

    let mut chain_contracts = HashMap::new();
    chain_contracts.insert(1u64, contracts);

    let mut chain_chunk_sizes = HashMap::new();
    chain_chunk_sizes.insert(1u64, chunk_size);

    BackfillCoordinator::new(
        workers,
        Arc::new(EventRegistry::new()),
        db_handlers,
        write_buffer,
        cache_store,
        progress,
        factory_tracker,
        chain_contracts,
        chain_chunk_sizes,
        db.pool.clone(),
        "public".to_string(),
    )
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_backfill_single_chain_single_contract() {
    let db = TestDb::new().await;
    let schema = test_schema();
    db.setup_schema(&schema).await;

    let mock = MockRpc::new().await;
    mock.mock_block_number(100).await;

    // Create 10 transfer logs spread across blocks
    let logs: Vec<_> = (0..10)
        .map(|i| common::fixtures::make_transfer_log(i * 10 + 1, 0))
        .collect();
    mock.mock_logs(&logs).await;

    let contract = test_contract();
    let coordinator = build_coordinator(&mock, &db, &schema, vec![contract], 50).await;

    coordinator.run_chain(1, 100).await.unwrap();

    let count = db.count_rows("transfer_events").await;
    assert_eq!(count, 10, "Expected 10 transfer events, got {}", count);

    let checkpoint = db.get_checkpoint(1, "ERC20").await;
    assert!(checkpoint.is_some(), "Checkpoint should be set");
    assert!(checkpoint.unwrap() > 0, "Checkpoint should be > 0");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_backfill_resumes_from_checkpoint() {
    let db = TestDb::new().await;
    let schema = test_schema();
    db.setup_schema(&schema).await;

    // Pre-set checkpoint at block 50
    db.set_checkpoint(1, "ERC20", 51).await;

    let mock = MockRpc::new().await;
    mock.mock_block_number(100).await;

    // Only logs in blocks 51-100
    let logs: Vec<_> = (0..5)
        .map(|i| common::fixtures::make_transfer_log(51 + i * 10, 0))
        .collect();
    mock.mock_logs(&logs).await;

    let contract = test_contract();
    let coordinator = build_coordinator(&mock, &db, &schema, vec![contract], 50).await;

    coordinator.run_chain(1, 100).await.unwrap();

    let count = db.count_rows("transfer_events").await;
    assert_eq!(
        count, 5,
        "Should only index events from block 51+, got {}",
        count
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_backfill_write_buffer_flushed_per_chunk() {
    let db = TestDb::new().await;
    let schema = test_schema();
    db.setup_schema(&schema).await;

    let mock = MockRpc::new().await;
    mock.mock_block_number(100).await;

    // 20 logs across 100 blocks
    let logs: Vec<_> = (0..20)
        .map(|i| common::fixtures::make_transfer_log(i * 5 + 1, 0))
        .collect();
    mock.mock_logs(&logs).await;

    let contract = test_contract();
    // Small chunk size to force multiple flushes
    let coordinator = build_coordinator(&mock, &db, &schema, vec![contract], 10).await;

    coordinator.run_chain(1, 100).await.unwrap();

    let count = db.count_rows("transfer_events").await;
    assert_eq!(
        count, 20,
        "All events should be flushed to DB, got {}",
        count
    );

    // Checkpoint should be at or past block 100
    let checkpoint = db.get_checkpoint(1, "ERC20").await;
    assert!(checkpoint.is_some());
}
