# Deployment Guide

## Quick Start with Docker Compose

```bash
cp .env.example .env
# Edit .env with your RPC URLs and Postgres password
docker-compose up -d
```

This starts four services:
- **postgres**: PostgreSQL 16 database
- **indexer**: Your forge-index binary
- **prometheus**: Metrics collection (port 9090)
- **grafana**: Dashboards (port 3000, default login: admin/admin)

## Development Mode

```bash
docker-compose -f docker-compose.dev.yml up
```

Dev mode runs only Postgres and the indexer (no monitoring stack). The full project directory is mounted for hot reload.

## Building the Docker Image

```bash
# Build locally
make docker-build

# Push to registry
docker push your-registry/forge-index:latest
```

The Dockerfile uses a multi-stage build with cargo-chef for efficient layer caching:
1. **chef**: Installs cargo-chef on the Rust base image
2. **planner**: Computes dependency recipe from Cargo.lock
3. **builder**: Compiles dependencies (cached), then source code
4. **runtime**: Copies binary to minimal Debian image (~50MB)

## Environment Variables

See [Config Reference](config-reference.md) for the complete list.

Essential variables for production:
```bash
DATABASE_URL=postgresql://forge:forge@postgres:5432/forge
RPC_URL_1=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
FORGE_ENV=prod
FORGE_API_KEY=your-secret-key
```

## Production Checklist

- [ ] Set `FORGE_ENV=prod` to enable API authentication
- [ ] Set `FORGE_API_KEY` for SQL endpoint security
- [ ] Use a paid RPC provider (Alchemy Growth, Infura Pro, or self-hosted)
- [ ] Set `MAX_BLOCK_RANGE` appropriate for your RPC provider (10 for Alchemy free, 2000 for paid)
- [ ] Configure Postgres `max_connections` (at least 20)
- [ ] Set up Postgres backups
- [ ] Monitor via Grafana dashboard at port 3000
- [ ] Set up alerting on `forge_indexer_lag_blocks` > threshold

## Scaling

### Vertical Scaling

forge-index is single-threaded for event processing (handlers run sequentially). To increase throughput:
- Use a faster RPC provider (lower latency = faster backfill)
- Increase `MAX_BLOCK_RANGE` for fewer RPC calls
- Tune Postgres (see [Performance Guide](performance.md))

### Horizontal Scaling

Run one forge-index instance per chain using the schema locking feature:
```bash
# Instance 1: Ethereum mainnet
FORGE_SCHEMA=mainnet DATABASE_URL=... RPC_URL_1=... forge start

# Instance 2: Base
FORGE_SCHEMA=base DATABASE_URL=... RPC_URL_8453=... forge start
```

Each instance gets its own Postgres schema and advisory lock.
