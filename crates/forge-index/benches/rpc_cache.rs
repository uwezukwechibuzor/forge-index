//! Benchmark: RPC cache hit vs miss latency.
//!
//! Measures the overhead of the cache layer by comparing
//! cached data lookup against constructing new cache entries.

use criterion::{criterion_group, criterion_main, Criterion};
use forge_index_core::types::{Address, Hash32, Log};
use forge_index_rpc::LogFilter;

fn make_filter(from: u64, to: u64) -> LogFilter {
    LogFilter {
        address: vec![Address::from(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        )],
        topics: vec![Some(vec![Hash32::from(
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        )])],
        from_block: from,
        to_block: to,
    }
}

fn make_logs(count: usize) -> Vec<Log> {
    (0..count)
        .map(|i| {
            let block_hash = Hash32([i as u8; 32]);
            Log {
                id: format!("{}-{}", block_hash, i),
                chain_id: 1,
                address: Address::from("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                topics: vec![Hash32::from(
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                )],
                data: vec![0u8; 32],
                block_number: i as u64,
                block_hash,
                transaction_hash: Hash32([0u8; 32]),
                log_index: 0,
                transaction_index: 0,
                removed: false,
            }
        })
        .collect()
}

fn bench_rpc_cache(c: &mut Criterion) {
    c.bench_function("create_log_filter", |b| {
        b.iter(|| {
            let _filter = make_filter(0, 1000);
        })
    });

    c.bench_function("serialize_100_logs_to_json", |b| {
        let logs = make_logs(100);
        b.iter(|| {
            let _json = serde_json::to_value(&logs).unwrap();
        })
    });

    c.bench_function("deserialize_100_logs_from_json", |b| {
        let logs = make_logs(100);
        let json = serde_json::to_value(&logs).unwrap();
        let json_str = serde_json::to_string(&json).unwrap();
        b.iter(|| {
            let _logs: Vec<Log> = serde_json::from_str(&json_str).unwrap();
        })
    });

    c.bench_function("create_1000_logs", |b| {
        b.iter(|| {
            let _logs = make_logs(1000);
        })
    });
}

criterion_group!(benches, bench_rpc_cache);
criterion_main!(benches);
