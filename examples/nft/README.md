# NFT Indexer Example

Indexes Bored Ape Yacht Club (BAYC) ERC721 transfers with block-interval
aggregated statistics using forge-index.

## Features Demonstrated

- **ERC721 Transfer events** — mints, burns, and transfers
- **Block interval handler** — computes stats every 100 blocks
- **Setup handler** — fetches collection metadata (name, symbol) before indexing

## Schema

| Table | Description |
|-------|-------------|
| `tokens` | All NFTs with current owner, tokenURI, transfer count |
| `transfers` | Every transfer event with from/to/tokenId |
| `holders` | Per-address token count |
| `collection_stats` | Singleton: total supply, transfers, unique holders |
| `collection_info` | Collection name and symbol from setup |

## Setup

1. Create the database:
```sql
CREATE DATABASE nft_indexer;
```

2. Copy `.env.sample` to `.env` and fill in your values:
```sh
cp .env.sample .env
```

3. Run:
```sh
cargo run
```

## Block Intervals

The `StatsUpdate` handler runs every 100 blocks to compute aggregate stats:
- Counts unique holders (addresses with token_count > 0)
- Updates `collection_stats.last_updated_block`

This avoids expensive aggregate queries on every transfer event.

## GraphQL Queries

**Get tokens owned by an address:**
```graphql
{
  tokenss(where: { owner: { eq: "0x..." } }, limit: 50) {
    items { token_id, owner, token_uri, transfer_count }
    totalCount
  }
}
```

**Get collection stats:**
```graphql
{
  collectionStats(id: "stats") {
    total_supply
    total_transfers
    unique_holders
    last_updated_block
  }
}
```

**Get recent transfers for a token:**
```graphql
{
  transferss(where: { token_id: { eq: "42" } }, orderBy: block_number, orderDirection: DESC) {
    items { from_address, to_address, block_number, tx_hash }
  }
}
```
