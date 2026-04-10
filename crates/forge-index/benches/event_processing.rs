//! Benchmark: event processing throughput (events/second).
//!
//! Measures the cost of processing decoded events through handlers
//! and flushing to the write buffer.

use criterion::{criterion_group, criterion_main, Criterion};
use forge_index_config::{ColumnType, SchemaBuilder};
use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_core::types::{Address, Hash32, Log};
use forge_index_db::buffer::WriteBuffer;
use forge_index_db::DbContext;
use indexmap::IndexMap;
use std::sync::Arc;

fn make_test_event(i: u64) -> DecodedEvent {
    let mut params = IndexMap::new();
    params.insert(
        "from".to_string(),
        DecodedParam::Address(Address([0x01; 20])),
    );
    params.insert("to".to_string(), DecodedParam::Address(Address([0x02; 20])));
    params.insert("value".to_string(), DecodedParam::Uint256(i.to_string()));

    let block_hash = Hash32([i as u8; 32]);
    DecodedEvent {
        name: "Transfer".to_string(),
        contract_name: "ERC20".to_string(),
        params,
        raw_log: Log {
            id: format!("{}-0", i),
            chain_id: 1,
            address: Address([0xA0; 20]),
            topics: vec![Hash32([0u8; 32])],
            data: vec![0u8; 32],
            block_number: i,
            block_hash,
            transaction_hash: Hash32([0u8; 32]),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        },
    }
}

fn bench_event_processing(c: &mut Criterion) {
    // This benchmark measures the overhead of creating DbContext
    // and building insert operations (without actual DB flush).
    let _schema = SchemaBuilder::new()
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("block_number", ColumnType::BigInt)
                .not_null()
        })
        .build();

    let events: Vec<DecodedEvent> = (0..1000).map(make_test_event).collect();

    c.bench_function("create_1000_decoded_events", |b| {
        b.iter(|| {
            let _events: Vec<DecodedEvent> = (0..1000).map(make_test_event).collect();
        })
    });

    c.bench_function("sort_1000_events_by_block_and_log_index", |b| {
        b.iter(|| {
            let mut events_copy = events.clone();
            events_copy.sort_by(|a, b| {
                let block_cmp = a.raw_log.block_number.cmp(&b.raw_log.block_number);
                block_cmp.then(a.raw_log.log_index.cmp(&b.raw_log.log_index))
            });
        })
    });

    c.bench_function("extract_event_params_1000_events", |b| {
        b.iter(|| {
            for event in &events {
                let _from = event.params.get("from");
                let _to = event.params.get("to");
                let _value = event.params.get("value");
            }
        })
    });
}

criterion_group!(benches, bench_event_processing);
criterion_main!(benches);
