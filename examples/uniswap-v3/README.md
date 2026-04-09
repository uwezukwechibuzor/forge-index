# Uniswap V3 Indexer Example

Indexes Uniswap V3 pools, swaps, and liquidity positions using the **factory pattern** —
pools are dynamically discovered as the factory emits `PoolCreated` events.

## Factory Pattern

This example demonstrates forge-index's key feature: **dynamic contract discovery**.

1. The `UniswapV3Factory` contract is watched for `PoolCreated` events
2. Each `PoolCreated` event contains the address of a newly deployed pool
3. forge-index automatically starts watching that pool for `Swap`, `Mint`, `Burn`, and `Initialize` events
4. No need to know pool addresses in advance

## Schema

| Table | Description |
|-------|-------------|
| `pools` | All discovered pools with token pair, fee tier, current price/tick/liquidity |
| `swaps` | Every swap with amounts, price impact, tick movement |
| `mints` | Liquidity provision events with tick range and amounts |
| `pool_stats` | Per-pool aggregate statistics (total swaps, volume) |

## Setup

1. Create the database:
```sql
CREATE DATABASE uniswap_v3;
```

2. Copy `.env.sample` to `.env` and fill in your values:
```sh
cp .env.sample .env
```

3. Run:
```sh
cargo run
```

## GraphQL Queries

**Get all pools sorted by creation block:**
```graphql
{
  poolss(orderBy: created_at_block, orderDirection: DESC, limit: 10) {
    items {
      address
      token0
      token1
      fee_tier
      sqrt_price
      current_tick
      liquidity
    }
    totalCount
  }
}
```

**Get recent swaps for a pool:**
```graphql
{
  swapss(where: { pool: { eq: "0x8ad599c3..." } }, orderBy: block_number, orderDirection: DESC, limit: 20) {
    items {
      id
      sender
      recipient
      amount0
      amount1
      tick_after
      block_number
    }
  }
}
```

**Get pool stats:**
```graphql
{
  poolStats(pool: "0x8ad599c3...") {
    total_swaps
    total_volume_token0
    total_volume_token1
  }
}
```
