//! Tests for the ForgeIndex builder and runner.

use crate::builder::ForgeIndex;
use forge_index_config::{AddressConfig, ColumnType, ConfigBuilder, DatabaseConfig, SchemaBuilder};
use forge_index_core::abi::decoder::DecodedEvent;
use forge_index_core::types::Address;

const ERC20_ABI: &str = r#"[
    {
        "type": "event",
        "name": "Transfer",
        "inputs": [
            {"name": "from", "type": "address", "indexed": true},
            {"name": "to", "type": "address", "indexed": true},
            {"name": "value", "type": "uint256", "indexed": false}
        ]
    },
    {
        "type": "event",
        "name": "Approval",
        "inputs": [
            {"name": "owner", "type": "address", "indexed": true},
            {"name": "spender", "type": "address", "indexed": true},
            {"name": "value", "type": "uint256", "indexed": false}
        ]
    }
]"#;

fn test_schema() -> forge_index_config::Schema {
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
        })
        .build()
}

fn test_config() -> forge_index_config::Config {
    ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = "https://eth.rpc.example".to_string();
        })
        .contract("ERC20", |c| {
            c.abi_json = ERC20_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address =
                AddressConfig::Single(Address::from("0x0000000000000000000000000000000000000001"));
            c.start_block = 0;
        })
        .schema(test_schema())
        .database(DatabaseConfig::postgres("postgres://localhost/test"))
        .build()
        .unwrap()
}

async fn dummy_handler(_event: DecodedEvent, _ctx: serde_json::Value) -> Result<(), anyhow::Error> {
    Ok(())
}

#[test]
fn forge_index_without_config_returns_error() {
    let result = ForgeIndex::new()
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    match result {
        Err(e) => assert!(
            e.to_string().contains("Config is required"),
            "expected config error, got: {}",
            e
        ),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn forge_index_without_schema_returns_error() {
    let result = ForgeIndex::new()
        .config(test_config())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    match result {
        Err(e) => assert!(
            e.to_string().contains("Schema is required"),
            "expected schema error, got: {}",
            e
        ),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn forge_index_with_unknown_event_key_returns_error() {
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("UnknownContract:UnknownEvent", dummy_handler)
        .build();

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("UnknownContract:UnknownEvent"),
                "expected unknown handler error, got: {}",
                msg
            );
        }
        Ok(_) => panic!("expected error for unknown event key"),
    }
}

#[test]
fn forge_index_with_valid_config_builds_successfully() {
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    assert!(result.is_ok(), "build should succeed");
}

#[test]
fn forge_index_warns_for_unhandled_events() {
    // Approval event exists in ABI but we only register Transfer handler.
    // This should succeed (just a warning, not an error).
    let result = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build();

    assert!(result.is_ok(), "should succeed even with unhandled events");
}

#[test]
fn forge_index_runner_has_correct_build_id() {
    let runner = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    assert!(!runner.build_id().is_empty());
    assert_eq!(runner.build_id().len(), 64); // SHA-256 hex
}

#[test]
fn forge_index_build_id_is_deterministic() {
    let r1 = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    let r2 = ForgeIndex::new()
        .config(test_config())
        .schema(test_schema())
        .on("ERC20:Transfer", dummy_handler)
        .build()
        .unwrap();

    assert_eq!(r1.build_id(), r2.build_id());
}

#[test]
fn forge_index_default_is_empty() {
    let fi = ForgeIndex::default();
    let result = fi.build();
    assert!(result.is_err());
}
