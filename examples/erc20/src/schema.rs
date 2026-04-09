//! Schema definition for the ERC20 indexer.

use forge_index_config::{ColumnType, Schema, SchemaBuilder};

/// Builds the data schema for the ERC20 indexer.
pub fn build() -> Schema {
    SchemaBuilder::new()
        .table("accounts", |t| {
            t.column("address", ColumnType::Address)
                .primary_key()
                .column("balance", ColumnType::BigInt)
                .not_null()
                .column("approval_count", ColumnType::Int)
                .not_null()
        })
        .table("transfer_events", |t| {
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
                .column("timestamp", ColumnType::BigInt)
                .not_null()
                .column("tx_hash", ColumnType::Hash)
                .not_null()
                .index(&["from_address"])
                .index(&["to_address"])
                .index(&["block_number"])
        })
        .table("approval_events", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("owner", ColumnType::Address)
                .not_null()
                .column("spender", ColumnType::Address)
                .not_null()
                .column("value", ColumnType::BigInt)
                .not_null()
                .column("block_number", ColumnType::BigInt)
                .not_null()
                .column("tx_hash", ColumnType::Hash)
                .not_null()
                .index(&["owner"])
        })
        .table("token_stats", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("total_transfers", ColumnType::BigInt)
                .not_null()
                .column("total_approvals", ColumnType::BigInt)
                .not_null()
                .column("unique_holders", ColumnType::BigInt)
                .not_null()
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_builds_without_error() {
        let schema = build();
        assert_eq!(schema.tables.len(), 4);
        assert_eq!(schema.tables[0].name, "accounts");
        assert_eq!(schema.tables[1].name, "transfer_events");
        assert_eq!(schema.tables[2].name, "approval_events");
        assert_eq!(schema.tables[3].name, "token_stats");
    }

    #[test]
    fn schema_sql_is_valid() {
        let schema = build();
        let sql = schema.to_create_sql("public");
        assert!(!sql.is_empty());
        // Should create both main and reorg tables
        assert!(sql.iter().any(|s| s.contains("\"accounts\"")));
        assert!(sql.iter().any(|s| s.contains("\"_reorg_accounts\"")));
    }
}
