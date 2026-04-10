# Architecture

## System Overview

```
                    +-----------+
                    |  EVM Node |
                    |  (RPC)    |
                    +-----+-----+
                          |
            HTTP/WS       |       HTTP/WS
       +------------------+------------------+
       |                                     |
  +----v----+                           +----v----+
  | Backfill|                           | Realtime|
  | Workers |                           |Processor|
  +---------+                           +---------+
       |                                     |
       |    +-------------+                  |
       +--->| Log Decoder |<-----------------+
            +------+------+
                   |
            +------v------+
            |   Handlers  |
            | (user code) |
            +------+------+
                   |
            +------v------+     +-----------+
            | Write Buffer|---->|  Postgres  |
            +------+------+     +-----+-----+
                   |                   |
            +------v------+     +-----v-----+
            |  Checkpoint |     | GraphQL / |
            |   Manager   |     | SQL API   |
            +-------------+     +-----------+
                                      |
                                +-----v-----+
                                |   Client  |
                                +-----------+
```

## Crate Dependency Graph

```
forge-index-core          (types, ABI decoder, event registry)
    |
forge-index-config        (builder API, chain/contract/schema types)
    |
forge-index-rpc           (RPC client, cache, retry, rate limiting)
    |
forge-index-db            (write buffer, reorg store, DbContext)
    |
forge-index-sync          (backfill, realtime, reorg detection)
    |
forge-index-api           (HTTP server, GraphQL, SQL-over-HTTP)
    |
forge-index-telemetry     (metrics, logging, build ID)
    |
forge-index               (public API, ForgeIndex builder, runner)
    |
forge-index-cli           (CLI binary: forge start, forge dev)
```

## Startup Sequence

`ForgeIndexRunner::run()` executes these steps in order:

1. **Initialise logging** via tracing-subscriber with RUST_LOG filtering
2. **Connect to Postgres** and create a `DatabaseManager`
3. **Run schema migrations** — creates tables, indexes, shadow tables, `_forge_meta`
4. **Check build ID** — detects schema changes (NotFound, Same, Changed)
5. **Initialise cache store** — creates `ponder_sync` schema with checkpoint, log, block tables
6. **Build RPC clients** — one `CachedRpcClient` per chain with rate limiter
7. **Build backfill workers** — one per (chain_id, contract_name) pair
8. **Start HTTP server** — health, ready, metrics, GraphQL, SQL endpoints
9. **Run backfill** — `BackfillCoordinator::run_chain()` for each chain
10. **Signal readiness** — `ready_tx.send(true)`, GET /ready returns 200
11. **Wait for shutdown** — listens for SIGINT/SIGTERM
12. **Final flush** — flushes remaining write buffer contents
13. **Release lock** — releases Postgres advisory lock

## Backfill Pipeline

```
Planner ──> Worker ──> Coordinator ──> Handler ──> Buffer ──> Flush
```

1. **Planner** splits `[checkpoint..current_block]` into chunk-sized `BlockRange`s
2. **Worker** calls `eth_getLogs` for each range, decodes logs via `LogDecoder`
3. **Coordinator** merges events from all contracts, sorts by (block_number, log_index)
4. **Handler** receives each `DecodedEvent` and a `DbContext` for writes
5. **Buffer** accumulates rows in memory (`DashMap<table, Vec<Row>>`)
6. **Flush** bulk-inserts rows via batched SQL, updates checkpoint

## Realtime Pipeline

After backfill completes:

1. **Subscriber** opens WebSocket to `eth_subscribe("newHeads")`
2. **ReorgDetector** compares `block.parent_hash` against stored `ChainState`
3. **Normal**: fetch events for the block, run handlers, flush, record metrics
4. **Reorg detected**: rollback shadow tables, update checkpoints, re-index from fork point
5. **Disconnect**: automatic reconnection with exponential backoff (up to 3 attempts)

## Reorg Handling

```
Detection:  parent_hash(new_block) != stored_hash(new_block.number - 1)
                    |
Walk-back:  find_fork_point() via RPC (up to 128 blocks)
                    |
Rollback:   ReorgStore.rollback_from_block() replays shadow table entries
                    |
Re-index:   Resume normal processing from fork_block
```

Shadow tables (`_reorg_*`) record the inverse of every database operation:
- INSERT triggers a DELETE record
- DELETE triggers an INSERT record with the deleted row's data
- UPDATE triggers an UPDATE record with the old values

On rollback, these entries are replayed in reverse order to restore the pre-reorg state.

## RPC Cache

The `ponder_sync` schema in Postgres stores cached RPC responses:

| Table | Key | Value |
|-------|-----|-------|
| `logs` | (chain_id, from_block, to_block, address, topics) | Serialised log entries |
| `blocks` | (chain_id, block_number) | Full block data |
| `transactions` | (chain_id, tx_hash) | Full transaction data |
| `eth_calls` | (chain_id, call_key) | Raw call result bytes |
| `checkpoints` | (chain_id, contract_name) | Last indexed block number |

Cache lookups are lock-free via `CachedRpcClient` which wraps `RpcClient`. On cache miss, the response is fetched from the network and stored for future use.

## Write Buffer

`WriteBuffer` is a `DashMap<String, Vec<Row>>` that batches writes per table:

- **Max size**: 10,000 rows per table (triggers immediate flush)
- **Flush interval**: 500ms (background task)
- **Flush protocol**: Batched SQL INSERT statements
- **Metrics**: `forge_write_buffer_size` gauge per table, `forge_db_flush_duration_seconds` histogram

## Schema Locking

forge-index uses Postgres advisory locks to ensure only one indexer instance runs per schema:

1. On startup, `DatabaseManager` acquires `pg_advisory_lock(hash(schema_name))`
2. If the lock is already held, startup fails with `SchemaLocked` error
3. On shutdown, the lock is explicitly released via `pg_advisory_unlock`

## Factory Pattern

For contracts deployed by factory contracts (e.g. Uniswap V3 pools):

1. Configure `AddressConfig::Factory(FactoryConfig { ... })` with the factory address and event
2. During backfill, factory events are processed first to discover child addresses
3. `FactoryAddressTracker` stores discovered addresses in a `DashMap`
4. Child contract events are then fetched using the discovered addresses
5. On restart, addresses are reloaded from cached factory events

## Build ID

The build ID is a SHA-256 hash of:
- All ABI JSON strings
- All contract configurations
- The complete schema definition

When the build ID changes between runs (schema or ABI modification), the system can detect that reindexing may be needed.
