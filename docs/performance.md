# Performance Tuning Guide

## RPC Provider Choice

The RPC provider is the primary bottleneck during backfill. Latency and rate limits directly determine indexing speed.

| Provider | Free Tier Rate Limit | Recommended `MAX_BLOCK_RANGE` | Notes |
|----------|---------------------|------------------------------|-------|
| Alchemy Free | 330 req/s | 10 | Good free tier |
| Alchemy Growth | 660 req/s | 2000 | Best for production |
| Infura Free | 10 req/s | 3 | Very slow for backfill |
| QuickNode | Varies | 2000 | Check plan limits |
| Self-hosted (Erigon) | Unlimited | 10000+ | Best performance |

Set in `.env`:
```bash
MAX_BLOCK_RANGE=2000
MAX_RPC_REQUESTS_PER_SECOND=25
```

## Chunk Size Tuning

`MAX_BLOCK_RANGE` controls how many blocks are queried per `eth_getLogs` call:
- **Too small** (1-10): Many RPC calls, slow due to overhead
- **Too large** (10000+): May hit provider response size limits or timeouts
- **Optimal** (500-2000): Balances call overhead with response size

For busy contracts (USDC, WETH), use smaller ranges (100-500). For quiet contracts, use larger ranges (2000-5000).

## Write Buffer Tuning

The write buffer batches database inserts:
- **Max size**: 10,000 rows per table (triggers immediate flush)
- **Flush interval**: 500ms (background flush)

For high-throughput indexers, the default settings work well. The buffer prevents per-event database round-trips.

## Postgres Configuration

Add to `postgresql.conf` for indexing workloads:

```ini
# Connection pool
max_connections = 50

# Memory (adjust based on available RAM)
shared_buffers = 256MB
work_mem = 64MB
effective_cache_size = 1GB

# Write performance
wal_buffers = 16MB
checkpoint_completion_target = 0.9
synchronous_commit = off  # Safe for indexing (data can be re-derived)

# Autovacuum (important for tables with high write rates)
autovacuum_vacuum_cost_delay = 10ms
autovacuum_vacuum_scale_factor = 0.05
```

## Reading the Grafana Dashboard

### Identifying Bottlenecks

| Panel | What to Look For |
|-------|------------------|
| **Events Indexed/s** | Should increase during backfill, stable during realtime |
| **Blocks Processed/s** | Flat at 0 = stuck; spiky = RPC throttling |
| **Indexer Lag** | Blocks behind chain tip; should decrease during backfill, stay near 0 realtime |
| **Backfill Progress** | 0-100%; slow progress = RPC bottleneck |
| **RPC Duration p99** | > 1s = provider is slow or throttling |
| **DB Flush Duration** | > 100ms = Postgres needs tuning |
| **Write Buffer Size** | Consistently at 10,000 = flush is the bottleneck |
| **HTTP Error Rate** | Spikes = handler panics or validation errors |
| **RPC Error Rate** | Spikes = provider issues or rate limiting |

### Common Bottleneck Patterns

1. **RPC p99 > 500ms + low blocks/s**: Provider is slow. Switch providers or increase rate limit.
2. **Write buffer always full + high flush duration**: Postgres is the bottleneck. Tune Postgres settings.
3. **Low events/s + low RPC latency**: Handlers are slow. Optimise handler code.
4. **Backfill stuck at 0%**: Check logs for errors. Likely an RPC auth or connection issue.
