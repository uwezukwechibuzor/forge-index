//! Integration tests for the ERC20 indexer.

use forge_index::prelude::*;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::types::{Address, Hash32, Log};
use indexmap::IndexMap;

const ERC20_ABI: &str = include_str!("../abis/ERC20.json");

// ── Test helpers ────────────────────────────────────────────────────────
// These verify parameter extraction without a real DbContext.

fn extract_address(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Address(addr) => Ok(addr.to_string()),
        other => anyhow::bail!("Expected address for '{}', got {:?}", name, other),
    }
}

fn extract_uint256(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Uint(v) => Ok(v.to_string()),
        DecodedParam::Uint256(s) => Ok(s.clone()),
        other => anyhow::bail!("Expected uint256 for '{}', got {:?}", name, other),
    }
}

fn make_log(block: u64, log_idx: u32, tx_byte: u8) -> Log {
    Log {
        id: format!("test-{}", log_idx),
        chain_id: 1,
        address: Address([0; 20]),
        topics: vec![],
        data: vec![],
        block_number: block,
        block_hash: Hash32([0; 32]),
        transaction_hash: Hash32([tx_byte; 32]),
        log_index: log_idx,
        transaction_index: 0,
        removed: false,
    }
}

// ── ABI tests ───────────────────────────────────────────────────────────

#[test]
fn abi_parses_correctly() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    assert_eq!(parsed.events.len(), 2);
    assert_eq!(parsed.events[0].name, "Transfer");
    assert_eq!(parsed.events[1].name, "Approval");
    assert_eq!(parsed.functions.len(), 9);
}

#[test]
fn transfer_selector_is_correct() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    let transfer = &parsed.events[0];
    assert_eq!(
        transfer.selector.to_string(),
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    );
}

#[test]
fn approval_selector_is_correct() {
    let parsed = parse_abi(ERC20_ABI).unwrap();
    let approval = &parsed.events[1];
    assert_eq!(
        approval.selector.to_string(),
        "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925"
    );
}

// ── Schema tests ────────────────────────────────────────────────────────

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

#[test]
fn schema_sql_generation() {
    let schema = erc20_indexer::schema::build();
    let sql = schema.to_create_sql("public");
    assert!(sql.len() >= 8);
    assert!(sql.iter().any(|s| s.contains("\"accounts\"")));
    assert!(sql.iter().any(|s| s.contains("\"_reorg_accounts\"")));
}

// ── Builder tests ───────────────────────────────────────────────────────

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

    async fn noop(_e: DecodedEvent, _c: serde_json::Value) -> Result<(), anyhow::Error> {
        Ok(())
    }

    let result = ForgeIndex::new()
        .config(config)
        .schema(erc20_indexer::schema::build())
        .on("ERC20:Transfer", noop)
        .on("ERC20:Approval", noop)
        .build();

    assert!(result.is_ok(), "builder should accept valid ERC20 config");
}

// ── Handler parameter extraction tests ──────────────────────────────────

#[test]
fn transfer_event_extracts_params() {
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
        raw_log: make_log(100, 0, 0x33),
    };

    let from = extract_address(&event, "from").unwrap();
    let to = extract_address(&event, "to").unwrap();
    let value = extract_uint256(&event, "value").unwrap();

    assert!(from.starts_with("0x"));
    assert!(to.starts_with("0x"));
    assert_eq!(value, "1000");
}

#[test]
fn approval_event_extracts_params() {
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
        raw_log: make_log(100, 1, 0x44),
    };

    let owner = extract_address(&event, "owner").unwrap();
    let spender = extract_address(&event, "spender").unwrap();
    let value = extract_uint256(&event, "value").unwrap();

    assert!(owner.starts_with("0x"));
    assert!(spender.starts_with("0x"));
    assert_eq!(value, "5000");
}

#[test]
fn missing_params_returns_error() {
    let event = DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC20".to_string(),
        params: IndexMap::new(),
        raw_log: make_log(100, 0, 0),
    };

    assert!(extract_address(&event, "from").is_err());
}

#[test]
fn schema_build_id_is_deterministic() {
    let s1 = erc20_indexer::schema::build();
    let s2 = erc20_indexer::schema::build();
    assert_eq!(s1.build_id(), s2.build_id());
}
