//! Integration tests for the ERC20 indexer.

use forge_index::prelude::*;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::types::{Address, Hash32, Log};
use indexmap::IndexMap;

const ERC20_ABI: &str = include_str!("../abis/ERC20.json");

/// Tests that the ABI parses correctly and contains expected events.
#[test]
fn abi_parses_correctly() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    assert_eq!(parsed.events.len(), 2);
    assert_eq!(parsed.events[0].name, "Transfer");
    assert_eq!(parsed.events[1].name, "Approval");
    assert_eq!(parsed.functions.len(), 9);
}

/// Tests that the Transfer event selector matches the expected keccak256.
#[test]
fn transfer_selector_is_correct() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    let transfer = &parsed.events[0];
    assert_eq!(
        transfer.selector.to_string(),
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    );
}

/// Tests that the Approval event selector matches the expected keccak256.
#[test]
fn approval_selector_is_correct() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    let approval = &parsed.events[1];
    assert_eq!(
        approval.selector.to_string(),
        "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925"
    );
}

/// Tests that the schema builds correctly with 4 tables.
#[test]
fn schema_has_correct_tables() {
    let schema = erc20_indexer::schema::build();
    assert_eq!(schema.tables.len(), 4);

    let table_names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(table_names.contains(&"accounts"));
    assert!(table_names.contains(&"transfer_events"));
    assert!(table_names.contains(&"approval_events"));
    assert!(table_names.contains(&"token_stats"));
}

/// Tests that the schema generates valid SQL for all tables.
#[test]
fn schema_sql_generation() {
    let schema = erc20_indexer::schema::build();
    let sql = schema.to_create_sql("public");

    // 4 tables × (main + reorg) + indexes
    assert!(sql.len() >= 8, "should have at least 8 SQL statements");

    // Check main tables exist
    assert!(sql.iter().any(|s| s.contains("\"accounts\"")));
    assert!(sql.iter().any(|s| s.contains("\"transfer_events\"")));
    assert!(sql.iter().any(|s| s.contains("\"approval_events\"")));
    assert!(sql.iter().any(|s| s.contains("\"token_stats\"")));

    // Check reorg shadow tables
    assert!(sql.iter().any(|s| s.contains("\"_reorg_accounts\"")));
    assert!(sql.iter().any(|s| s.contains("\"_reorg_transfer_events\"")));
}

/// Tests that the ForgeIndex builder accepts the ERC20 config and schema.
#[test]
fn forge_index_builder_accepts_erc20_config() {
    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = "http://localhost:8545".to_string();
        })
        .contract("ERC20", |c| {
            c.abi_json = ERC20_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(Address::from(
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            ));
            c.start_block = 6_082_465;
        })
        .schema(erc20_indexer::schema::build())
        .database(DatabaseConfig::postgres("postgres://localhost/test"))
        .build()
        .unwrap();

    let schema = erc20_indexer::schema::build();

    async fn transfer_handler(
        _e: DecodedEvent,
        _c: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
    async fn approval_handler(
        _e: DecodedEvent,
        _c: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    let result = ForgeIndex::new()
        .config(config)
        .schema(schema)
        .on("ERC20:Transfer", transfer_handler)
        .on("ERC20:Approval", approval_handler)
        .build();

    assert!(result.is_ok(), "builder should accept valid ERC20 config");
}

/// Tests the Transfer handler with a synthetic decoded event.
#[tokio::test]
async fn transfer_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "from".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000001")),
    );
    params.insert(
        "to".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000002")),
    );
    params.insert(
        "value".to_string(),
        DecodedParam::Uint256("1000".to_string()),
    );

    let event = DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC20".to_string(),
        params,
        raw_log: Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![],
            data: vec![],
            block_number: 100,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0x33; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        },
    };

    let result = erc20_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_ok(), "transfer handler should succeed");
}

/// Tests the Approval handler with a synthetic decoded event.
#[tokio::test]
async fn approval_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "owner".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000001")),
    );
    params.insert(
        "spender".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000003")),
    );
    params.insert("value".to_string(), DecodedParam::Uint(5000));

    let event = DecodedEvent {
        name: "Approval".to_string(),
        contract_name: "ERC20".to_string(),
        params,
        raw_log: Log {
            id: "test-1".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![],
            data: vec![],
            block_number: 100,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0x44; 32]),
            log_index: 1,
            transaction_index: 0,
            removed: false,
        },
    };

    let result = erc20_indexer::handlers::handle_approval(event, serde_json::Value::Null).await;
    assert!(result.is_ok(), "approval handler should succeed");
}

/// Tests that the schema build_id is deterministic.
#[test]
fn schema_build_id_is_deterministic() {
    let s1 = erc20_indexer::schema::build();
    let s2 = erc20_indexer::schema::build();
    assert_eq!(s1.build_id(), s2.build_id());
}

/// Tests that handler rejects events with missing parameters.
#[tokio::test]
async fn transfer_handler_rejects_missing_params() {
    let event = DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC20".to_string(),
        params: IndexMap::new(), // empty — missing from/to/value
        raw_log: Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0; 20]),
            topics: vec![],
            data: vec![],
            block_number: 100,
            block_hash: Hash32([0; 32]),
            transaction_hash: Hash32([0; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        },
    };

    let result = erc20_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_err(), "should fail with missing params");
}
