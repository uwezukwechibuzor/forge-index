# ERC20 Indexer Example

Indexes USDC (ERC20) Transfer and Approval events on Ethereum mainnet using forge-index.

## What it indexes

- **accounts** — tracks token balances and approval counts per address
- **transfer_events** — every Transfer event with from, to, value, block, timestamp
- **approval_events** — every Approval event with owner, spender, value
- **token_stats** — singleton with aggregate counts

## Prerequisites

- Rust toolchain (1.75+)
- PostgreSQL (14+)
- An Ethereum RPC URL (Alchemy, Infura, or local node)

## Setup

1. Create the database:
```sql
CREATE DATABASE erc20_indexer;
```

2. Create a `.env` file:
```env
RPC_URL_1=https://eth-mainnet.alchemyapi.io/v2/YOUR_KEY
DATABASE_URL=postgres://postgres:postgres@localhost:5432/erc20_indexer
```

## Run

Development mode (with hot reload):
```sh
forge dev
```

Or directly:
```sh
cargo run
```

Production mode:
```sh
forge start
```

## Query

Once running, the GraphQL API is available at `http://localhost:42069/graphql`.

### Example GraphQL queries

**Get accounts with highest balance:**
```graphql
{
  accountss(orderBy: balance, orderDirection: DESC, limit: 10) {
    items {
      address
      balance
      approval_count
    }
    totalCount
  }
}
```

**Get recent transfers:**
```graphql
{
  transferEventss(orderBy: block_number, orderDirection: DESC, limit: 20) {
    items {
      id
      from_address
      to_address
      value
      block_number
      tx_hash
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
```

**Get a specific account:**
```graphql
{
  accounts(address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48") {
    address
    balance
    approval_count
  }
}
```

### Example curl commands

Health check:
```sh
curl http://localhost:42069/health
```

Readiness:
```sh
curl http://localhost:42069/ready
```

Metrics:
```sh
curl http://localhost:42069/metrics
```

GraphQL query:
```sh
curl -X POST http://localhost:42069/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ tokenStatss(limit:1) { items { total_transfers total_approvals unique_holders } } }"}'
```
