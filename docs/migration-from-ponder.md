# Migration from Ponder

This guide maps every Ponder concept to its forge-index equivalent.

## Configuration

### Ponder (TypeScript)
```typescript
// ponder.config.ts
import { createConfig } from "@ponder/core";

export default createConfig({
  networks: {
    mainnet: {
      chainId: 1,
      transport: http(process.env.PONDER_RPC_URL_1),
    },
  },
  contracts: {
    ERC20: {
      network: "mainnet",
      abi: ERC20Abi,
      address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      startBlock: 6082465,
    },
  },
});
```

### forge-index (Rust)
```rust
let config = ConfigBuilder::new()
    .chain("mainnet", |c| {
        c.chain_id = 1;
        c.rpc_http = std::env::var("RPC_URL_1").unwrap();
    })
    .contract("ERC20", |c| {
        c.abi_json = include_str!("../abis/ERC20.json").to_string();
        c.chain_names = vec!["mainnet".to_string()];
        c.address = AddressConfig::Single(Address::from("0xA0b8...eB48"));
        c.start_block = 6_082_465;
    })
    .schema(schema)
    .database(DatabaseConfig::postgres(std::env::var("DATABASE_URL").unwrap()))
    .build()?;
```

## Schema

### Ponder
```typescript
// ponder.schema.ts
import { createSchema } from "@ponder/core";

export default createSchema((p) => ({
  Account: p.createTable({
    id: p.string(),
    balance: p.bigint(),
    isOwner: p.boolean(),
  }),
}));
```

### forge-index
```rust
let schema = SchemaBuilder::new()
    .table("accounts", |t| {
        t.column("id", ColumnType::Text).primary_key()
         .column("balance", ColumnType::BigInt).not_null()
         .column("is_owner", ColumnType::Boolean).not_null()
    })
    .build();
```

## Event Handlers

### Ponder
```typescript
// src/index.ts
import { ponder } from "@/generated";

ponder.on("ERC20:Transfer", async ({ event, context }) => {
  await context.db.Account.upsert({
    id: event.args.to,
    create: { balance: event.args.value, isOwner: false },
    update: ({ current }) => ({
      balance: current.balance + event.args.value,
    }),
  });
});
```

### forge-index
```rust
ForgeIndex::new()
    .config(config)
    .schema(schema)
    .on_db("ERC20:Transfer", |event, ctx| {
        Box::pin(async move {
            let to = event.get("to")?.to_string();
            let value = event.get("value")?.to_string();

            let mut row = Row::new();
            row.insert("id", to);
            row.insert("balance", ColumnValue::BigNumeric(value));
            row.insert("is_owner", false);
            ctx.insert("accounts").row(row).execute()?;
            Ok(())
        })
    })
    .run()
    .await?;
```

## Factory Pattern

### Ponder
```typescript
ERC20: {
  network: "mainnet",
  abi: ERC20Abi,
  factory: {
    address: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
    event: parseAbiItem("event PoolCreated(...)"),
    parameter: "pool",
  },
  startBlock: 12369621,
}
```

### forge-index
```rust
c.address = AddressConfig::Factory(FactoryConfig {
    factory_address: vec![Address::from("0x1F98...C756")],
    event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
    address_parameter: "pool".to_string(),
    start_block: 12_369_621,
});
```

## Database Operations

| Ponder | forge-index |
|--------|-------------|
| `context.db.Table.create({ id, ... })` | `ctx.insert("table").row(row).execute()?` |
| `context.db.Table.update({ id }, { ... })` | `ctx.update("table").pk("id", pk_val).set("col", val).execute()?` |
| `context.db.Table.delete({ id })` | `ctx.delete("table").pk("id", pk_val).execute()?` |
| `context.db.Table.findUnique({ id })` | `ctx.find_one::<T>("table").filter("id", val).execute().await?` |
| `context.db.Table.findMany({ where })` | `ctx.find_many::<T>("table").filter("col", val).execute().await?` |

## Project Structure

| Ponder | forge-index |
|--------|-------------|
| `ponder.config.ts` | Inline `ConfigBuilder` in `main.rs` |
| `ponder.schema.ts` | Inline `SchemaBuilder` in `main.rs` |
| `src/index.ts` | `.on_db()` handler registration |
| `abis/*.json` | `abis/*.json` (same format) |
| `.env.local` | `.env` |
| `package.json` | `Cargo.toml` |

## Known Limitations (v0.1)

Features Ponder supports that forge-index v0.1 does not yet:

- **Upsert operations**: Ponder's `.upsert()` with `create`/`update` callbacks. Use separate find + insert/update in forge-index.
- **Virtual tables**: Ponder's `p.createEnum()` and computed columns.
- **Multichain ordering**: Ponder's automatic cross-chain event ordering. forge-index processes chains independently.
- **GraphQL subscriptions**: Live query updates via WebSocket.
- **API authentication**: Ponder has built-in API key management. forge-index has Bearer token auth only.
