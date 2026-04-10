# forge-index (WIP)

**The Rust EVM Indexing Framework**

A production-ready Rust port of [Ponder](https://ponder.sh) with the same developer experience, built for speed and reliability.

[![CI](https://github.com/example/forge-index/actions/workflows/ci.yml/badge.svg)](https://github.com/example/forge-index/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

---

## Why forge-index?

**If you know Ponder, you already know forge-index.** The configuration builder, schema definition, and event handler APIs are designed to feel identical to Ponder's TypeScript API, but run as compiled Rust with zero runtime overhead. The same indexer that takes minutes in JavaScript finishes in seconds.

**Production-grade from day one.** forge-index includes automatic chain reorganization handling with shadow tables, a Postgres-backed RPC response cache that eliminates redundant network calls across restarts, per-IP rate-limited SQL-over-HTTP for ad-hoc queries, and a Grafana dashboard with ten pre-built panels for monitoring throughput, latency, and lag.

**Self-hosted, no vendor lock-in.** Unlike The Graph's hosted service, forge-index runs anywhere you can run Postgres and a Rust binary. A single `docker-compose up` gives you the full stack: indexer, database, Prometheus, and Grafana. There are no subgraph deployments, no IPFS, and no token economics between you and your data.

## Quickstart

### 1. Create a new project

```bash
cargo init my-indexer && cd my-indexer
cargo add forge-index
cargo add tokio --features full
cargo add anyhow dotenvy
```

### 2. Add an ABI

Save your contract's ABI to `abis/ERC20.json`.

### 3. Write the indexer

```rust
// src/main.rs
use forge_index::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = std::env::var("RPC_URL_1").unwrap();
        })
        .contract("ERC20", |c| {
            c.abi_json = include_str!("../abis/ERC20.json").to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = AddressConfig::Single(Address::from(
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            ));
            c.start_block = 24_845_000;
        })
        .schema(schema())
        .database(DatabaseConfig::postgres(
            std::env::var("DATABASE_URL").unwrap(),
        ))
        .build()?;

    ForgeIndex::new()
        .config(config)
        .schema(schema())
        .on_db("ERC20:Transfer", |event, ctx| {
            Box::pin(async move {
                let mut row = Row::new();
                row.insert("id", event.raw_log.id.clone());
                row.insert("from_address", event.get("from")?.to_string());
                row.insert("to_address", event.get("to")?.to_string());
                row.insert("block_number", event.raw_log.block_number as i64);
                ctx.insert("transfers").row(row).execute()?;
                Ok(())
            })
        })
        .run()
        .await?;

    Ok(())
}

fn schema() -> Schema {
    SchemaBuilder::new()
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("from_address", ColumnType::Address).not_null()
                .column("to_address", ColumnType::Address).not_null()
                .column("block_number", ColumnType::BigInt).not_null()
        })
        .build()
}
```

### 4. Run

```bash
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/my_indexer
export RPC_URL_1=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
cargo run
```

The indexer starts backfilling from block 24,845,000 and serves:
- **Metrics** at `http://localhost:42069/metrics`
- **Health** at `http://localhost:42069/health`
- **SQL API** at `POST http://localhost:42069/sql`

## Core Concepts

**Config** defines what to index: which chains, which contracts, their ABIs, and starting blocks. The `ConfigBuilder` provides a fluent API that mirrors Ponder's `ponder.config.ts`. Each chain gets an RPC endpoint, and each contract gets an address and ABI.

**Schema** defines where to store it: tables, columns, types, and primary keys. The `SchemaBuilder` mirrors Ponder's `ponder.schema.ts`. Column types map directly to Postgres types (Text, Int, BigInt, Boolean, Address, Bytes, Json).

**Handlers** define how to process events. Register handlers with `.on_db("Contract:Event", handler)` to receive decoded events and a `DbContext` for database operations. Handlers are called in block order with automatic write buffering and checkpoint management.

## Feature Comparison

| Feature | forge-index | Ponder | The Graph |
|---------|------------|--------|-----------|
| Language | Rust | TypeScript | AssemblyScript |
| EVM chains | All | All | All |
| Multichain | Yes | Yes | No |
| Factory pattern | Yes | Yes | Yes |
| Block intervals | Yes | Yes | No |
| Reorg handling | Shadow tables | Shadow tables | Subgraph restart |
| GraphQL API | Auto-generated | Auto-generated | Defined in schema |
| SQL API | POST /sql | No | No |
| RPC cache | Postgres-backed | Postgres-backed | No |
| Hot reload | Yes | Yes | No |
| Self-hostable | Yes | Yes | Partial |
| Open source | MIT | MIT | Apache-2.0 |

## Performance

forge-index is designed for high-throughput indexing. The write buffer batches inserts and flushes via Postgres COPY for maximum write speed. The RPC cache eliminates redundant network calls across restarts. Run `cargo bench -p forge-index` to measure performance on your hardware.

Typical throughput on modern hardware:
- **Event processing**: 10,000+ events/second (handler + buffer)
- **RPC cache hit**: < 1ms (vs 50-200ms for network round-trip)
- **DB flush**: 10,000 rows in < 50ms via COPY

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
forge-index = "0.1"
tokio = { version = "1", features = ["full"] }
```

Minimum supported Rust version: **1.75**

## Documentation

- [Architecture](docs/architecture.md) - system overview and component deep-dive
- [Config Reference](docs/config-reference.md) - every configuration option
- [Schema Reference](docs/schema-reference.md) - table and column type reference
- [GraphQL API](docs/graphql-api.md) - auto-generated GraphQL documentation
- [SQL API](docs/sql-api.md) - SQL-over-HTTP endpoint reference
- [Deployment](docs/deployment.md) - Docker and production deployment guide
- [Performance](docs/performance.md) - tuning guide and bottleneck diagnosis
- [Migration from Ponder](docs/migration-from-ponder.md) - guide for Ponder users

## Examples

- [`examples/erc20`](examples/erc20) - USDC Transfer and Approval indexing
- [`examples/uniswap-v3`](examples/uniswap-v3) - Factory pattern with pool discovery
- [`examples/nft`](examples/nft) - NFT indexer with block intervals

## License

MIT

## Acknowledgements

Inspired by [Ponder](https://ponder.sh) by 0xOlias. forge-index aims to bring the same excellent developer experience to the Rust ecosystem.
