# Config Reference

## ConfigBuilder

The fluent API for building indexer configuration.

```rust
let config = ConfigBuilder::new()
    .chain("mainnet", |c| { /* ChainConfig */ })
    .contract("ERC20", |c| { /* ContractConfig */ })
    .schema(schema)
    .database(DatabaseConfig::postgres("postgres://..."))
    .build()?;
```

## ChainConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `String` | (required) | Human-readable chain name |
| `chain_id` | `u64` | (required) | Numeric chain ID (e.g. 1 for Ethereum mainnet) |
| `rpc_http` | `String` | (required) | HTTP RPC endpoint URL |
| `rpc_ws` | `Option<String>` | `None` | WebSocket RPC endpoint for realtime subscriptions |
| `max_rpc_requests_per_second` | `Option<u32>` | `25` | Rate limit for RPC calls |
| `poll_interval_ms` | `Option<u64>` | `None` | Polling interval for new blocks (HTTP fallback) |
| `max_block_range` | `Option<u64>` | `2000` | Maximum blocks per `eth_getLogs` request |

### Example

```rust
.chain("mainnet", |c| {
    c.chain_id = 1;
    c.rpc_http = "https://eth-mainnet.g.alchemy.com/v2/KEY".to_string();
    c.rpc_ws = Some("wss://eth-mainnet.g.alchemy.com/v2/KEY".to_string());
    c.max_block_range = Some(10); // Alchemy free tier
    c.max_rpc_requests_per_second = Some(25);
})
```

## ContractConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `String` | (required) | Contract identifier used in handler keys |
| `abi_json` | `String` | (required) | Full ABI as JSON string |
| `chain_names` | `Vec<String>` | (required) | Chains this contract is deployed on |
| `address` | `AddressConfig` | (required) | Contract address(es) |
| `start_block` | `u64` | `0` | Block to start indexing from |
| `end_block` | `Option<EndBlock>` | `None` | Block to stop indexing at |
| `filter` | `Option<FilterConfig>` | `None` | Event-level filter conditions |
| `include_transaction` | `bool` | `false` | Include full transaction data in events |
| `include_trace` | `bool` | `false` | Include trace data in events |

### AddressConfig variants

```rust
// Single known address
AddressConfig::Single(Address::from("0xA0b8...eB48"))

// Multiple addresses
AddressConfig::Multiple(vec![addr1, addr2, addr3])

// Factory-discovered addresses
AddressConfig::Factory(FactoryConfig {
    factory_address: vec![Address::from("0x1F98...C756")],
    event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
    address_parameter: "pool".to_string(),
    start_block: 12_369_621,
})
```

## DatabaseConfig

```rust
DatabaseConfig::postgres("postgresql://user:pass@host:5432/dbname")
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | Postgres connection string |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | Postgres connection string | (required) |
| `RPC_URL_{CHAIN_ID}` | RPC endpoint per chain | (required) |
| `FORGE_ENV` | `dev` or `prod` | `dev` |
| `FORGE_PORT` | HTTP server port | `42069` |
| `FORGE_SCHEMA` | Postgres schema name | `public` |
| `FORGE_LOG_LEVEL` | Log level | `info` |
| `FORGE_API_KEY` | Bearer token for /sql (prod mode) | (none) |
| `FORGE_RPC_RATE_LIMIT` | RPC requests/second per chain | `25` |
| `MAX_BLOCK_RANGE` | Blocks per eth_getLogs request | `2000` |
| `START_BLOCK` | Override contract start block | (from config) |
