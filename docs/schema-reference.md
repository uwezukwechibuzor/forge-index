# Schema Reference

## SchemaBuilder

Defines the database tables that your indexer writes to.

```rust
let schema = SchemaBuilder::new()
    .table("transfers", |t| {
        t.column("id", ColumnType::Text)
            .primary_key()
            .column("from_address", ColumnType::Address)
            .not_null()
            .column("to_address", ColumnType::Address)
            .not_null()
            .column("value", ColumnType::BigInt)
            .not_null()
            .column("block_number", ColumnType::BigInt)
            .not_null()
    })
    .build();
```

## Column Types

| ColumnType | Postgres Type | JSON Serialisation | Notes |
|-----------|---------------|-------------------|-------|
| `Text` | `TEXT` | String | Variable-length text |
| `Int` | `INTEGER` | Number | 32-bit signed integer |
| `BigInt` | `BIGINT` | Number (or String via GraphQL) | 64-bit signed integer |
| `Float` | `DOUBLE PRECISION` | Number | 64-bit floating point |
| `Boolean` | `BOOLEAN` | Bool | true/false |
| `Address` | `TEXT` | String | 0x-prefixed 40-char hex |
| `Bytes` | `BYTEA` | String (0x-prefixed hex) | Arbitrary byte data |
| `Json` | `JSONB` | Object/Array | Embedded JSON |
| `BigNumeric` | `NUMERIC` | String | Arbitrary precision (U256) |

## Table Options

### Primary Key

Every table must have exactly one primary key column:

```rust
t.column("id", ColumnType::Text).primary_key()
```

### Not Null

Mark columns as non-nullable:

```rust
t.column("balance", ColumnType::BigInt).not_null()
```

### Indexes

Add indexes for query performance:

```rust
t.column("block_number", ColumnType::BigInt)
    .not_null()
    .index()
```

## Internal Tables

forge-index automatically creates these tables alongside your schema:

| Table | Purpose |
|-------|---------|
| `_forge_meta` | Stores build ID and metadata |
| `_reorg_{table}` | Shadow table for reorg rollback per user table |

## ponder_sync Schema

The `ponder_sync` schema stores RPC cache and checkpoint data:

| Table | Purpose |
|-------|---------|
| `ponder_sync.checkpoints` | Last indexed block per (chain_id, contract) |
| `ponder_sync.logs` | Cached eth_getLogs responses |
| `ponder_sync.blocks` | Cached block data |
| `ponder_sync.transactions` | Cached transaction data |
| `ponder_sync.eth_calls` | Cached eth_call results |
