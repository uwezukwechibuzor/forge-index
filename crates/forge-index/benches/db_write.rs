//! Benchmark: database write patterns.
//!
//! Measures the overhead of constructing Row objects and buffering operations,
//! without actual database flushes (which would require Docker).

use criterion::{criterion_group, criterion_main, Criterion};
use forge_index_db::row::{ColumnValue, Row};
use indexmap::IndexMap;

fn make_row(i: usize) -> Row {
    let mut map = IndexMap::new();
    map.insert("id".to_string(), ColumnValue::Text(format!("row-{}", i)));
    map.insert(
        "address".to_string(),
        ColumnValue::Text(format!("0x{:040x}", i)),
    );
    map.insert("balance".to_string(), ColumnValue::BigInt(i as i64 * 1000));
    map.insert("is_active".to_string(), ColumnValue::Boolean(i % 2 == 0));
    map.insert("block_number".to_string(), ColumnValue::BigInt(i as i64));
    Row {
        columns: map,
        operation: None, // INSERT
    }
}

fn bench_db_write(c: &mut Criterion) {
    c.bench_function("create_1000_rows", |b| {
        b.iter(|| {
            let _rows: Vec<Row> = (0..1000).map(make_row).collect();
        })
    });

    c.bench_function("create_10000_rows", |b| {
        b.iter(|| {
            let _rows: Vec<Row> = (0..10000).map(make_row).collect();
        })
    });

    c.bench_function("column_value_to_sql_literal_1000", |b| {
        let rows: Vec<Row> = (0..1000).map(make_row).collect();
        b.iter(|| {
            for row in &rows {
                for (_col, val) in &row.columns {
                    let _sql = Row::to_sql_literal(val);
                }
            }
        })
    });

    c.bench_function("row_indexmap_lookup_1000", |b| {
        let rows: Vec<Row> = (0..1000).map(make_row).collect();
        b.iter(|| {
            for row in &rows {
                let _id = row.columns.get("id");
                let _addr = row.columns.get("address");
                let _bal = row.columns.get("balance");
            }
        })
    });
}

criterion_group!(benches, bench_db_write);
criterion_main!(benches);
