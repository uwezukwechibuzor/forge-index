//! Integration tests for the NFT indexer.

use forge_index::prelude::*;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::types::{Address, Hash32, Log};
use indexmap::IndexMap;

const ERC721_ABI: &str = include_str!("../abis/ERC721.json");
const ZERO: &str = "0x0000000000000000000000000000000000000000";

fn make_log(block: u64, log_idx: u32) -> Log {
    Log {
        id: format!("test-{}", log_idx),
        chain_id: 1,
        address: Address::from("0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D"),
        topics: vec![],
        data: vec![],
        block_number: block,
        block_hash: Hash32([0; 32]),
        transaction_hash: Hash32([block as u8; 32]),
        log_index: log_idx,
        transaction_index: 0,
        removed: false,
    }
}

fn make_transfer_event(
    from: &str,
    to: &str,
    token_id: u128,
    block: u64,
    log_idx: u32,
) -> DecodedEvent {
    let mut params = IndexMap::new();
    params.insert(
        "from".to_string(),
        DecodedParam::Address(Address::from(from)),
    );
    params.insert("to".to_string(), DecodedParam::Address(Address::from(to)));
    params.insert("tokenId".to_string(), DecodedParam::Uint(token_id));

    DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC721".to_string(),
        params,
        raw_log: make_log(block, log_idx),
    }
}

// ── ABI tests ───────────────────────────────────────────────────────────

#[test]
fn erc721_abi_parses_correctly() {
    let parsed = parse_abi(ERC721_ABI).unwrap();
    assert_eq!(parsed.events.len(), 3);
    let names: Vec<&str> = parsed.events.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Transfer"));
    assert!(names.contains(&"Approval"));
    assert!(names.contains(&"ApprovalForAll"));
    assert_eq!(parsed.functions.len(), 6);
}

#[test]
fn transfer_selector_is_correct() {
    let parsed = parse_abi(ERC721_ABI).unwrap();
    let transfer = parsed.events.iter().find(|e| e.name == "Transfer").unwrap();
    // Transfer(address,address,uint256)
    assert_eq!(
        transfer.selector.to_string(),
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    );
}

// ── Schema tests ────────────────────────────────────────────────────────

#[test]
fn schema_has_five_tables() {
    let schema = nft_indexer::schema::build();
    assert_eq!(schema.tables.len(), 5);
    let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"tokens"));
    assert!(names.contains(&"transfers"));
    assert!(names.contains(&"holders"));
    assert!(names.contains(&"collection_stats"));
    assert!(names.contains(&"collection_info"));
}

#[test]
fn schema_generates_valid_sql() {
    let schema = nft_indexer::schema::build();
    let sql = schema.to_create_sql("public");
    assert!(sql.iter().any(|s| s.contains("\"tokens\"")));
    assert!(sql.iter().any(|s| s.contains("\"_reorg_tokens\"")));
    assert!(sql.iter().any(|s| s.contains("\"holders\"")));
    assert!(sql.iter().any(|s| s.contains("\"collection_stats\"")));
    assert!(sql.iter().any(|s| s.contains("\"collection_info\"")));
}

// ── Handler tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn handle_mint_transfer() {
    let addr1 = "0x0000000000000000000000000000000000000001";
    let event = make_transfer_event(ZERO, addr1, 1, 12_287_507, 0);
    let result = nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn handle_burn_transfer() {
    let addr1 = "0x0000000000000000000000000000000000000001";
    let event = make_transfer_event(addr1, ZERO, 1, 12_300_000, 0);
    let result = nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn handle_normal_transfer() {
    let addr1 = "0x0000000000000000000000000000000000000001";
    let addr2 = "0x0000000000000000000000000000000000000002";
    let event = make_transfer_event(addr1, addr2, 42, 12_400_000, 0);
    let result = nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn handle_transfer_rejects_missing_params() {
    let event = DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC721".to_string(),
        params: IndexMap::new(),
        raw_log: make_log(100, 0),
    };
    let result = nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn handle_block_interval() {
    // Block interval events come as DecodedEvents with the block info
    let event = DecodedEvent {
        name: "block".to_string(),
        contract_name: "StatsUpdate".to_string(),
        params: IndexMap::new(),
        raw_log: make_log(12_287_600, 0),
    };
    let result = nft_indexer::handlers::handle_block(event, serde_json::Value::Null).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn handle_setup() {
    let result = nft_indexer::handlers::handle_setup(serde_json::Value::Null).await;
    assert!(result.is_ok());
}

// ── Builder tests ───────────────────────────────────────────────────────

#[test]
fn forge_index_builder_accepts_nft_config() {
    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = "http://localhost:8545".to_string();
        })
        .contract("ERC721", |c| {
            c.abi_json = ERC721_ABI.to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(Address::from(
                "0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D",
            ));
            c.start_block = 12_287_507;
        })
        .block_interval("StatsUpdate", |bi| {
            bi.chain_name = "mainnet".to_string();
            bi.interval = 100;
            bi.start_block = 12_287_507;
        })
        .schema(nft_indexer::schema::build())
        .database(DatabaseConfig::postgres("postgres://localhost/test"))
        .build()
        .unwrap();

    async fn noop(_e: DecodedEvent, _c: serde_json::Value) -> Result<(), anyhow::Error> {
        Ok(())
    }

    let result = ForgeIndex::new()
        .config(config)
        .schema(nft_indexer::schema::build())
        .on("ERC721:Transfer", noop)
        .build();

    assert!(result.is_ok(), "builder should accept NFT config");
}

#[test]
fn schema_build_id_is_deterministic() {
    let s1 = nft_indexer::schema::build();
    let s2 = nft_indexer::schema::build();
    assert_eq!(s1.build_id(), s2.build_id());
}

// ── Mint/burn balance logic tests ───────────────────────────────────────

#[tokio::test]
async fn five_mints_three_transfers_two_burns_sequence() {
    let addr1 = "0x0000000000000000000000000000000000000001";
    let addr2 = "0x0000000000000000000000000000000000000002";
    let addr3 = "0x0000000000000000000000000000000000000003";

    // 5 mints to addr1
    for token_id in 1..=5u128 {
        let event = make_transfer_event(ZERO, addr1, token_id, 100 + token_id as u64, 0);
        nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null)
            .await
            .unwrap();
    }

    // 3 transfers: token 1,2,3 from addr1 to addr2
    for token_id in 1..=3u128 {
        let event = make_transfer_event(addr1, addr2, token_id, 200 + token_id as u64, 0);
        nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null)
            .await
            .unwrap();
    }

    // 2 burns: token 4,5 from addr1
    for token_id in 4..=5u128 {
        let event = make_transfer_event(addr1, ZERO, token_id, 300 + token_id as u64, 0);
        nft_indexer::handlers::handle_transfer(event, serde_json::Value::Null)
            .await
            .unwrap();
    }

    // All handlers ran without error — balance tracking would be verified
    // against the DB in a full integration test with testcontainers.
    // Expected final state:
    //   total_supply = 3 (5 minted - 2 burned)
    //   addr1: 0 tokens (3 transferred + 2 burned)
    //   addr2: 3 tokens
}
