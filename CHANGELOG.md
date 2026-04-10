# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-04-10

### Added

#### Core Framework
- Workspace with 14 crates organised by domain (core, config, rpc, sync, db, api, telemetry, cli)
- Fluent `ConfigBuilder` API for chain, contract, and database configuration
- Fluent `SchemaBuilder` API for table and column definitions with type-safe column types
- Event handler registration via `.on()` (legacy) and `.on_db()` (production with DbContext)
- SHA-256 build ID generation for schema change detection

#### Indexing Engine
- Historical backfill with configurable chunk sizes and per-chain rate limiting
- Checkpoint-based resume: restarts pick up where they left off via `ponder_sync.checkpoints`
- Realtime block processing via WebSocket subscription with automatic reconnection
- Chain reorganization detection via parent hash comparison with configurable depth (128 blocks)
- Shadow table reorg rollback system (`_reorg_*` tables) for safe block reversal
- Factory pattern support for dynamically discovered contract addresses
- Block interval configuration for periodic handler invocation
- Multichain support with independent per-chain backfill and processing

#### RPC Client
- HTTP and WebSocket transport via alloy-rs providers
- Automatic retry with exponential backoff and jitter (5 attempts, 1-30s delays)
- Per-chain token bucket rate limiting
- Request deduplication for concurrent identical requests
- Postgres-backed RPC response cache (`ponder_sync` schema) for logs, blocks, transactions, eth_call

#### Database
- Automatic schema creation and migration from `SchemaBuilder` definitions
- In-memory write buffer with batched flush via Postgres COPY protocol
- Advisory lock-based schema locking (one indexer instance per schema)
- `DbContext` fluent API: insert, update, delete, find_one, find_many
- Reorg shadow tables for recording inverse operations

#### HTTP API
- Health (`GET /health`) and readiness (`GET /ready`) endpoints
- Prometheus metrics (`GET /metrics`) with 10 metric families
- Auto-generated GraphQL API with cursor-based pagination, filtering, and ordering
- SQL-over-HTTP (`POST /sql`) with read-only validation, LIMIT enforcement, and schema prefixing
- Schema metadata (`GET /schema`) with table, column, and approximate row count information
- Per-IP token bucket rate limiting for the SQL endpoint (10 req/s)
- Bearer token authentication for SQL endpoint in production mode

#### Monitoring
- Prometheus scrape configuration for the indexer metrics endpoint
- Grafana dashboard with 10 panels: events/s, blocks/s, indexer lag, backfill progress, RPC latency percentiles, DB flush duration, write buffer size, HTTP request rate, HTTP error rate, RPC error rate
- Grafana provisioning for automatic datasource and dashboard setup

#### CLI
- `forge start` - production mode
- `forge dev` - development mode with hot reload via file watching

#### Deployment
- Multi-stage Dockerfile with cargo-chef for layer-cached dependency compilation
- Production `docker-compose.yml` with Postgres, indexer, Prometheus, and Grafana
- Development `docker-compose.dev.yml` with Postgres and indexer only
- Makefile with 16 targets (build, test, docker-build, dev, prod, etc.)
- `.env.example` with all configuration variables documented

#### Testing
- Unit tests across all crates (SQL parser, rate limiter, filters, pagination, types)
- Integration tests for backfill, reorg detection, factory pattern, GraphQL, SQL API
- Deployment artifact validation tests (Dockerfile, docker-compose, Prometheus, Grafana)
- Criterion benchmarks for event processing, RPC cache, and database write throughput

#### Examples
- ERC20 indexer (USDC on mainnet): Transfer and Approval events with account balances
- Uniswap V3 indexer: Factory pattern with pool discovery and swap events
- NFT indexer: Block intervals with setup handler
