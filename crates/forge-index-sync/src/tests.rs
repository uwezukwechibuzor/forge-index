//! Tests for the sync engine.

use crate::backfill::planner::{self, BlockRange};
use crate::backfill::progress::BackfillProgress;
use crate::error::SyncError;
use crate::factory::FactoryAddressTracker;
use forge_index_config::{AddressConfig, ContractConfig, FactoryConfig};
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::types::{Address, Hash32, Log};
use indexmap::IndexMap;

fn test_contract() -> ContractConfig {
    ContractConfig {
        name: "TestToken".to_string(),
        abi_json: "[]".to_string(),
        chain_names: vec!["mainnet".to_string()],
        address: AddressConfig::Single(Address::from("0x0000000000000000000000000000000000000001")),
        start_block: 0,
        end_block: None,
        filter: None,
        include_transaction: false,
        include_trace: false,
    }
}

fn make_log() -> Log {
    Log {
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
    }
}

// ── Planner tests ──────────────────────────────────────────────────────

#[test]
fn planner_produces_correct_chunks_for_0_to_10000() {
    let config = test_contract();
    let plan = planner::plan(&config, 1, 10000, None, 2000);

    assert_eq!(plan.contract_name, "TestToken");
    assert_eq!(plan.total_blocks, 10001);
    assert_eq!(plan.ranges.len(), 6);
    assert_eq!(plan.ranges[0], BlockRange { from: 0, to: 1999 });
    assert_eq!(
        plan.ranges[1],
        BlockRange {
            from: 2000,
            to: 3999
        }
    );
    assert_eq!(
        plan.ranges[2],
        BlockRange {
            from: 4000,
            to: 5999
        }
    );
    assert_eq!(
        plan.ranges[3],
        BlockRange {
            from: 6000,
            to: 7999
        }
    );
    assert_eq!(
        plan.ranges[4],
        BlockRange {
            from: 8000,
            to: 9999
        }
    );
    assert_eq!(
        plan.ranges[5],
        BlockRange {
            from: 10000,
            to: 10000
        }
    );
}

#[test]
fn planner_resumes_from_checkpoint() {
    let config = test_contract();
    let plan = planner::plan(&config, 1, 10000, Some(5000), 2000);

    assert_eq!(plan.total_blocks, 5001);
    assert_eq!(plan.ranges[0].from, 5000);
    assert_eq!(plan.ranges[0].to, 6999);
}

#[test]
fn planner_with_end_less_than_start_returns_empty() {
    let mut config = test_contract();
    config.start_block = 5000;
    let plan = planner::plan(&config, 1, 3000, None, 2000);

    assert!(plan.ranges.is_empty());
    assert_eq!(plan.total_blocks, 0);
}

// ── Factory tests ──────────────────────────────────────────────────────

#[test]
fn factory_tracker_extracts_address_from_pool_parameter() {
    let tracker = FactoryAddressTracker::new();
    let factory = FactoryConfig {
        factory_address: vec![Address::from("0x0000000000000000000000000000000000000001")],
        event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
        address_parameter: "pool".to_string(),
        start_block: 0,
    };

    let pool_addr = Address::from("0x0000000000000000000000000000000000000042");
    let mut params = IndexMap::new();
    params.insert("pool".to_string(), DecodedParam::Address(pool_addr));

    let event = DecodedEvent {
        name: "PoolCreated".to_string(),
        contract_name: "UniswapV3Factory".to_string(),
        params,
        raw_log: make_log(),
    };

    tracker
        .process_factory_event(&event, &factory, 1, "UniswapV3Factory")
        .unwrap();

    let addrs = tracker.get_addresses(1, "UniswapV3Factory");
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0], pool_addr);
}

#[test]
fn factory_tracker_loads_addresses_from_add() {
    let tracker = FactoryAddressTracker::new();
    let addr1 = Address::from("0x0000000000000000000000000000000000000001");
    let addr2 = Address::from("0x0000000000000000000000000000000000000002");

    tracker.add_addresses(1, "Factory", vec![addr1, addr2]);

    assert_eq!(tracker.address_count(1, "Factory"), 2);
    let addrs = tracker.get_addresses(1, "Factory");
    assert_eq!(addrs[0], addr1);
    assert_eq!(addrs[1], addr2);
}

// ── Progress tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn progress_eta_calculation() {
    tokio::time::pause();

    let progress = BackfillProgress::new();
    progress.init_chain(1, 10000);

    // Advance time by 10 seconds and process 5000 blocks
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    progress.record(1, 5000, 100);

    let chain = progress.get_chain(1).unwrap();
    let pct = chain.percent_complete();
    assert!((pct - 50.0).abs() < 1.0, "expected ~50%, got {:.1}%", pct);

    let bps = chain.blocks_per_second();
    assert!(
        (bps - 500.0).abs() < 50.0,
        "expected ~500 blocks/s, got {:.0}",
        bps
    );

    let eta = chain.eta_seconds().unwrap();
    // 5000 remaining / 500 bps = 10 seconds
    assert!(
        (eta - 10.0).abs() < 2.0,
        "expected ETA ~10s, got {:.1}s",
        eta
    );
}

// ── Error handling tests ───────────────────────────────────────────────

#[test]
fn handler_panic_error_is_created_correctly() {
    let err = SyncError::HandlerPanic {
        handler: "ERC20:Transfer".to_string(),
        message: "divide by zero".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("ERC20:Transfer"));
    assert!(msg.contains("divide by zero"));
}

#[test]
fn factory_decode_error_on_missing_param() {
    let tracker = FactoryAddressTracker::new();
    let factory = FactoryConfig {
        factory_address: vec![],
        event_signature: "Created(address)".to_string(),
        address_parameter: "nonexistent".to_string(),
        start_block: 0,
    };

    let event = DecodedEvent {
        name: "Created".to_string(),
        contract_name: "Factory".to_string(),
        params: IndexMap::new(),
        raw_log: make_log(),
    };

    let result = tracker.process_factory_event(&event, &factory, 1, "Factory");
    assert!(result.is_err());
    match result.unwrap_err() {
        SyncError::FactoryDecode { contract, param } => {
            assert_eq!(contract, "Factory");
            assert_eq!(param, "nonexistent");
        }
        other => panic!("expected FactoryDecode, got: {:?}", other),
    }
}

// ── Worker sort test ───────────────────────────────────────────────────

#[test]
fn events_sort_by_block_number_and_log_index() {
    let mut events = vec![
        DecodedEvent {
            name: "B".to_string(),
            contract_name: "C".to_string(),
            params: IndexMap::new(),
            raw_log: Log {
                block_number: 200,
                log_index: 0,
                ..make_log()
            },
        },
        DecodedEvent {
            name: "A".to_string(),
            contract_name: "C".to_string(),
            params: IndexMap::new(),
            raw_log: Log {
                block_number: 100,
                log_index: 1,
                ..make_log()
            },
        },
        DecodedEvent {
            name: "A".to_string(),
            contract_name: "C".to_string(),
            params: IndexMap::new(),
            raw_log: Log {
                block_number: 100,
                log_index: 0,
                ..make_log()
            },
        },
    ];

    events.sort_by(|a, b| {
        let block_cmp = a.raw_log.block_number.cmp(&b.raw_log.block_number);
        block_cmp.then(a.raw_log.log_index.cmp(&b.raw_log.log_index))
    });

    assert_eq!(events[0].raw_log.block_number, 100);
    assert_eq!(events[0].raw_log.log_index, 0);
    assert_eq!(events[1].raw_log.block_number, 100);
    assert_eq!(events[1].raw_log.log_index, 1);
    assert_eq!(events[2].raw_log.block_number, 200);
    assert_eq!(events[2].raw_log.log_index, 0);
}
