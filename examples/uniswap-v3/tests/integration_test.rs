//! Integration tests for the Uniswap V3 indexer.

use forge_index::prelude::*;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::types::{Address, Hash32, Log};
use indexmap::IndexMap;

const FACTORY_ABI: &str = include_str!("../abis/UniswapV3Factory.json");
const POOL_ABI: &str = include_str!("../abis/UniswapV3Pool.json");

// ── ABI parsing tests ──────────────────────────────────────────────────

#[test]
fn factory_abi_parses_correctly() {
    let parsed = parse_abi(FACTORY_ABI).unwrap();
    assert_eq!(parsed.events.len(), 1);
    assert_eq!(parsed.events[0].name, "PoolCreated");
    assert_eq!(parsed.events[0].inputs.len(), 5);
}

#[test]
fn pool_abi_parses_correctly() {
    let parsed = parse_abi(POOL_ABI).unwrap();
    assert_eq!(parsed.events.len(), 4);
    let names: Vec<&str> = parsed.events.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Initialize"));
    assert!(names.contains(&"Swap"));
    assert!(names.contains(&"Mint"));
    assert!(names.contains(&"Burn"));
}

#[test]
fn pool_created_selector_is_correct() {
    let parsed = parse_abi(FACTORY_ABI).unwrap();
    // PoolCreated(address,address,uint24,int24,address)
    let selector = parsed.events[0].selector.to_string();
    assert_eq!(
        selector,
        "0x783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118"
    );
}

// ── Schema tests ────────────────────────────────────────────────────────

#[test]
fn schema_has_four_tables() {
    let schema = uniswap_v3_indexer::schema::build();
    assert_eq!(schema.tables.len(), 4);
    let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"pools"));
    assert!(names.contains(&"swaps"));
    assert!(names.contains(&"mints"));
    assert!(names.contains(&"pool_stats"));
}

#[test]
fn schema_sql_creates_all_tables_and_reorg_tables() {
    let schema = uniswap_v3_indexer::schema::build();
    let sql = schema.to_create_sql("public");
    assert!(sql.iter().any(|s| s.contains("\"pools\"")));
    assert!(sql.iter().any(|s| s.contains("\"_reorg_pools\"")));
    assert!(sql.iter().any(|s| s.contains("\"swaps\"")));
    assert!(sql.iter().any(|s| s.contains("\"_reorg_swaps\"")));
    assert!(sql.iter().any(|s| s.contains("\"mints\"")));
    assert!(sql.iter().any(|s| s.contains("\"pool_stats\"")));
}

// ── Builder tests ───────────────────────────────────────────────────────

#[test]
fn forge_index_builder_accepts_uniswap_config() {
    let factory_address = Address::from("0x1F98431c8aD98523631AE4a59f267346ea31F984");

    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = "http://localhost:8545".to_string();
        })
        .contract("UniswapV3Factory", |c| {
            c.abi_json = FACTORY_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(factory_address);
            c.start_block = 12_369_621;
        })
        .contract("UniswapV3Pool", |c| {
            c.abi_json = POOL_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Factory(FactoryConfig {
                factory_address: vec![factory_address],
                event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
                address_parameter: "pool".to_string(),
                start_block: 12_369_621,
            });
            c.start_block = 12_369_621;
        })
        .schema(uniswap_v3_indexer::schema::build())
        .database(DatabaseConfig::postgres("postgres://localhost/test"))
        .build()
        .unwrap();

    async fn noop(_e: DecodedEvent, _c: serde_json::Value) -> Result<(), anyhow::Error> {
        Ok(())
    }

    let result = ForgeIndex::new()
        .config(config)
        .schema(uniswap_v3_indexer::schema::build())
        .on("UniswapV3Factory:PoolCreated", noop)
        .on("UniswapV3Pool:Initialize", noop)
        .on("UniswapV3Pool:Swap", noop)
        .on("UniswapV3Pool:Mint", noop)
        .build();

    assert!(result.is_ok(), "builder should accept Uniswap V3 config");
}

// ── Handler tests ───────────────────────────────────────────────────────

fn make_log(block: u64, log_idx: u32) -> Log {
    Log {
        id: format!("test-{}", log_idx),
        chain_id: 1,
        address: Address::from("0x0000000000000000000000000000000000000ABC"),
        topics: vec![],
        data: vec![],
        block_number: block,
        block_hash: Hash32([0; 32]),
        transaction_hash: Hash32([0x33; 32]),
        log_index: log_idx,
        transaction_index: 0,
        removed: false,
    }
}

#[tokio::test]
async fn pool_created_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "token0".to_string(),
        DecodedParam::Address(Address::from("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
    );
    params.insert(
        "token1".to_string(),
        DecodedParam::Address(Address::from("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
    );
    params.insert("fee".to_string(), DecodedParam::Uint(3000));
    params.insert("tickSpacing".to_string(), DecodedParam::Int(60));
    params.insert(
        "pool".to_string(),
        DecodedParam::Address(Address::from("0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8")),
    );

    let event = DecodedEvent {
        name: "PoolCreated".to_string(),
        contract_name: "UniswapV3Factory".to_string(),
        params,
        raw_log: make_log(12_369_621, 0),
    };

    let result =
        uniswap_v3_indexer::handlers::handle_pool_created(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn swap_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "sender".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000001")),
    );
    params.insert(
        "recipient".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000002")),
    );
    params.insert("amount0".to_string(), DecodedParam::Int(-1000));
    params.insert("amount1".to_string(), DecodedParam::Int(500));
    params.insert(
        "sqrtPriceX96".to_string(),
        DecodedParam::Uint256("1234567890123456789".to_string()),
    );
    params.insert("liquidity".to_string(), DecodedParam::Uint(999999));
    params.insert("tick".to_string(), DecodedParam::Int(-200));

    let event = DecodedEvent {
        name: "Swap".to_string(),
        contract_name: "UniswapV3Pool".to_string(),
        params,
        raw_log: make_log(12_400_000, 3),
    };

    let result = uniswap_v3_indexer::handlers::handle_swap(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn mint_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "sender".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000001")),
    );
    params.insert(
        "owner".to_string(),
        DecodedParam::Address(Address::from("0x0000000000000000000000000000000000000002")),
    );
    params.insert("tickLower".to_string(), DecodedParam::Int(-887220));
    params.insert("tickUpper".to_string(), DecodedParam::Int(887220));
    params.insert("amount".to_string(), DecodedParam::Uint(50000));
    params.insert(
        "amount0".to_string(),
        DecodedParam::Uint256("100000".to_string()),
    );
    params.insert(
        "amount1".to_string(),
        DecodedParam::Uint256("200000".to_string()),
    );

    let event = DecodedEvent {
        name: "Mint".to_string(),
        contract_name: "UniswapV3Pool".to_string(),
        params,
        raw_log: make_log(12_400_000, 5),
    };

    let result = uniswap_v3_indexer::handlers::handle_mint(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn initialize_handler_processes_event() {
    let mut params = IndexMap::new();
    params.insert(
        "sqrtPriceX96".to_string(),
        DecodedParam::Uint256("79228162514264337593543950336".to_string()),
    );
    params.insert("tick".to_string(), DecodedParam::Int(0));

    let event = DecodedEvent {
        name: "Initialize".to_string(),
        contract_name: "UniswapV3Pool".to_string(),
        params,
        raw_log: make_log(12_369_622, 1),
    };

    let result =
        uniswap_v3_indexer::handlers::handle_initialize(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[test]
fn schema_build_id_is_deterministic() {
    let s1 = uniswap_v3_indexer::schema::build();
    let s2 = uniswap_v3_indexer::schema::build();
    assert_eq!(s1.build_id(), s2.build_id());
}

#[tokio::test]
async fn handler_rejects_missing_params() {
    let event = DecodedEvent {
        name: "Swap".to_string(),
        contract_name: "UniswapV3Pool".to_string(),
        params: IndexMap::new(),
        raw_log: make_log(100, 0),
    };

    let result = uniswap_v3_indexer::handlers::handle_swap(event, serde_json::Value::Null).await;
    assert!(result.is_err(), "should fail with missing params");
}
