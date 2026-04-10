# GraphQL API

forge-index auto-generates a GraphQL API from your schema definition. The API is served at `POST /graphql` with a playground at `GET /graphql`.

## Auto-Generated Types

For each table in your schema, forge-index generates:

- **Object type**: `Transfers` with fields matching your columns
- **Page type**: `TransfersPage` with `items`, `pageInfo`, `totalCount`
- **Filter input**: `TransfersFilter` with comparison operators per field
- **OrderBy enum**: `TransfersOrderBy` with one value per column

## Query Types

### Single Record by Primary Key

```graphql
query {
  transfer(id: "0xabc-0") {
    id
    fromAddress
    toAddress
    value
    blockNumber
  }
}
```

### Paginated List

```graphql
query {
  transfers(
    where: { blockNumber: { gte: "1000" } }
    orderBy: blockNumber
    orderDirection: desc
    limit: 10
  ) {
    items {
      id
      fromAddress
      toAddress
    }
    pageInfo {
      hasNextPage
      endCursor
    }
    totalCount
  }
}
```

### Cursor-Based Pagination

```graphql
# Page 1
query {
  transfers(limit: 10) {
    items { id }
    pageInfo { endCursor hasNextPage }
  }
}

# Page 2
query {
  transfers(limit: 10, after: "eyJwa192YWx1ZSI6...") {
    items { id }
    pageInfo { endCursor hasNextPage }
  }
}
```

## Filter Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equal | `{ address: { eq: "0xABC" } }` |
| `gt` | Greater than | `{ balance: { gt: "1000" } }` |
| `gte` | Greater than or equal | `{ balance: { gte: "1000" } }` |
| `lt` | Less than | `{ balance: { lt: "1000" } }` |
| `lte` | Less than or equal | `{ balance: { lte: "1000" } }` |
| `in` | In list | `{ status: { in: ["active", "pending"] } }` |
| `notIn` | Not in list | `{ status: { notIn: ["deleted"] } }` |
| `contains` | String contains | `{ name: { contains: "alice" } }` |

Multiple filters are AND-combined.

## Naming Conventions

- Table names are converted to camelCase for queries: `transfer_events` becomes `transferEvents`
- Type names use PascalCase: `transfer_events` becomes `TransferEvents`
- Column names stay as-is in the schema

## BigInt Handling

`BigInt` columns are serialised as JSON strings in GraphQL to avoid JavaScript number precision issues. A `BigInt` value of `9999999999999999999` is returned as `"9999999999999999999"`, not truncated to a floating-point approximation.

## Error Responses

```json
{
  "errors": [
    {
      "message": "Field 'nonexistent' not found on type 'Transfers'",
      "locations": [{ "line": 3, "column": 5 }]
    }
  ]
}
```
