//! Factory pattern integration tests.
//!
//! Run with: `cargo test -p forge-index --test factory_test`

mod common;

use forge_index_config::FactoryConfig;
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::types::{Address, Hash32, Log};
use forge_index_sync::FactoryAddressTracker;
use indexmap::IndexMap;

fn make_pool_created_event(pool_address: Address) -> DecodedEvent {
    let mut params = IndexMap::new();
    params.insert(
        "token0".to_string(),
        DecodedParam::Address(Address([0x01; 20])),
    );
    params.insert(
        "token1".to_string(),
        DecodedParam::Address(Address([0x02; 20])),
    );
    params.insert("pool".to_string(), DecodedParam::Address(pool_address));

    DecodedEvent {
        name: "PoolCreated".to_string(),
        contract_name: "Factory".to_string(),
        params,
        raw_log: Log {
            id: "test-0".to_string(),
            chain_id: 1,
            address: Address([0x10; 20]),
            topics: vec![Hash32([0u8; 32])],
            data: vec![],
            block_number: 100,
            block_hash: Hash32([0u8; 32]),
            transaction_hash: Hash32([0u8; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        },
    }
}

fn factory_config() -> FactoryConfig {
    FactoryConfig {
        factory_address: vec![Address([0x10; 20])],
        event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
        address_parameter: "pool".to_string(),
        start_block: 0,
    }
}

#[test]
fn test_factory_discovers_child_addresses() {
    let tracker = FactoryAddressTracker::new();
    let config = factory_config();

    let pools = [
        Address([0xA1; 20]),
        Address([0xA2; 20]),
        Address([0xA3; 20]),
    ];

    for pool in &pools {
        let event = make_pool_created_event(*pool);
        tracker
            .process_factory_event(&event, &config, 1, "Factory")
            .unwrap();
    }

    assert_eq!(tracker.address_count(1, "Factory"), 3);
    let addresses = tracker.get_addresses(1, "Factory");
    assert_eq!(addresses.len(), 3);
    assert!(addresses.contains(&pools[0]));
    assert!(addresses.contains(&pools[1]));
    assert!(addresses.contains(&pools[2]));
}

#[test]
fn test_factory_add_addresses_direct() {
    let tracker = FactoryAddressTracker::new();

    let addrs = vec![Address([0xB1; 20]), Address([0xB2; 20])];

    tracker.add_addresses(1, "Factory", addrs.clone());
    assert_eq!(tracker.address_count(1, "Factory"), 2);
    assert_eq!(tracker.get_addresses(1, "Factory"), addrs);
}

#[test]
fn test_factory_separate_chains() {
    let tracker = FactoryAddressTracker::new();
    let config = factory_config();

    // Chain 1: 2 pools
    for i in 0..2u8 {
        let event = make_pool_created_event(Address([0xC0 + i; 20]));
        tracker
            .process_factory_event(&event, &config, 1, "Factory")
            .unwrap();
    }

    // Chain 10: 3 pools
    for i in 0..3u8 {
        let event = make_pool_created_event(Address([0xD0 + i; 20]));
        tracker
            .process_factory_event(&event, &config, 10, "Factory")
            .unwrap();
    }

    assert_eq!(tracker.address_count(1, "Factory"), 2);
    assert_eq!(tracker.address_count(10, "Factory"), 3);
}

#[test]
fn test_factory_wrong_parameter_returns_error() {
    let tracker = FactoryAddressTracker::new();
    let config = FactoryConfig {
        factory_address: vec![Address([0x10; 20])],
        event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
        address_parameter: "nonexistent_param".to_string(),
        start_block: 0,
    };

    let event = make_pool_created_event(Address([0xAA; 20]));
    let result = tracker.process_factory_event(&event, &config, 1, "Factory");
    assert!(result.is_err(), "Should error when parameter not found");
}

#[test]
fn test_factory_loads_addresses_from_cache_simulation() {
    let tracker = FactoryAddressTracker::new();
    let config = factory_config();

    // Simulate first run: discover pools
    for i in 0..3u8 {
        let event = make_pool_created_event(Address([0xE0 + i; 20]));
        tracker
            .process_factory_event(&event, &config, 1, "Factory")
            .unwrap();
    }
    let discovered = tracker.get_addresses(1, "Factory");
    assert_eq!(discovered.len(), 3);

    // Simulate second run: load from "cache" (direct add)
    let tracker2 = FactoryAddressTracker::new();
    tracker2.add_addresses(1, "Factory", discovered.clone());

    assert_eq!(tracker2.address_count(1, "Factory"), 3);
    assert_eq!(tracker2.get_addresses(1, "Factory"), discovered);
}
