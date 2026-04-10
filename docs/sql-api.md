# SQL-over-HTTP API

The SQL endpoint allows ad-hoc read-only queries against your indexed data.

## Endpoint

```
POST /sql
Content-Type: application/json
```

## Request

```json
{
  "query": "SELECT address, balance FROM accounts WHERE balance > '1000' ORDER BY balance DESC LIMIT 10",
  "timeout_ms": 5000
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `query` | string | yes | - | SQL SELECT query |
| `timeout_ms` | number | no | 5000 | Statement timeout in milliseconds |

## Response

```json
{
  "data": {
    "columns": ["address", "balance"],
    "rows": [
      { "address": "0xAAA...", "balance": "50000" },
      { "address": "0xBBB...", "balance": "25000" }
    ]
  },
  "meta": {
    "row_count": 2,
    "execution_time_ms": 12
  },
  "error": null
}
```

## Validation Rules

1. **SELECT only**: INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, TRUNCATE, GRANT, REVOKE, COPY, EXECUTE are rejected
2. **No statement chaining**: Semicolons in the middle of the query are rejected
3. **No system catalog access**: `pg_catalog` and `information_schema` references are rejected
4. **No dollar quoting**: `$$` syntax is rejected to prevent injection
5. **Max length**: 10,000 characters
6. **LIMIT enforcement**: Queries without LIMIT get `LIMIT 1000` appended; LIMIT > 1000 is clamped to 1000
7. **Schema prefix**: Unqualified table names are automatically prefixed with the configured pg_schema

## Type Serialisation

| Postgres Type | JSON Type | Notes |
|--------------|-----------|-------|
| TEXT, VARCHAR | String | |
| INT, INTEGER | Number | |
| BIGINT | Number | |
| NUMERIC | String | Preserves precision for large numbers |
| BOOLEAN | Bool | |
| JSONB, JSON | Object/Array | Embedded as-is |
| BYTEA | String | 0x-prefixed hex |
| NULL | null | |

## Rate Limiting

The SQL endpoint is rate-limited to **10 requests per second per IP address**. When exceeded:

```
HTTP 429 Too Many Requests
Retry-After: 1
```

## Authentication (Production Mode)

When `FORGE_ENV=prod`, the SQL endpoint requires a Bearer token:

```
Authorization: Bearer YOUR_FORGE_API_KEY
```

Set via the `FORGE_API_KEY` environment variable. If `FORGE_API_KEY` is not set in prod mode, requests are allowed with a warning logged on each request.

In dev mode (`FORGE_ENV=dev` or unset), no authentication is required.

## Schema Endpoint

```
GET /schema
```

Returns metadata about all indexed tables:

```json
{
  "data": {
    "tables": [
      {
        "name": "transfers",
        "columns": [
          { "name": "id", "type": "text", "primary_key": true, "nullable": false },
          { "name": "from_address", "type": "text", "primary_key": false, "nullable": false }
        ],
        "row_count": 12450
      }
    ]
  }
}
```

Row counts are approximate (via `pg_class.reltuples`) for performance.
