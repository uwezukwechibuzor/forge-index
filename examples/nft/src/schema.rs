//! Schema definition for the NFT indexer.

use forge_index_config::{ColumnType, Schema, SchemaBuilder};

/// Builds the data schema for the NFT indexer.
pub fn build() -> Schema {
    SchemaBuilder::new()
        .table("tokens", |t| {
            t.column("token_id", ColumnType::BigInt)
                .primary_key()
                .column("owner", ColumnType::Address)
                .not_null()
                .column("token_uri", ColumnType::Text)
                .nullable()
                .column("transfer_count", ColumnType::Int)
                .not_null()
                .column("minted_at_block", ColumnType::BigInt)
                .not_null()
                .column("last_transfer_block", ColumnType::BigInt)
                .not_null()
                .index(&["owner"])
        })
        .table("transfers", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("token_id", ColumnType::BigInt)
                .not_null()
                .column("from_address", ColumnType::Address)
                .not_null()
                .column("to_address", ColumnType::Address)
                .not_null()
                .column("block_number", ColumnType::BigInt)
                .not_null()
                .column("timestamp", ColumnType::BigInt)
                .not_null()
                .column("tx_hash", ColumnType::Hash)
                .not_null()
                .index(&["token_id"])
                .index(&["block_number"])
        })
        .table("holders", |t| {
            t.column("address", ColumnType::Address)
                .primary_key()
                .column("token_count", ColumnType::Int)
                .not_null()
                .column("first_acquired_block", ColumnType::BigInt)
                .not_null()
        })
        .table("collection_stats", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("total_supply", ColumnType::BigInt)
                .not_null()
                .column("total_transfers", ColumnType::BigInt)
                .not_null()
                .column("unique_holders", ColumnType::BigInt)
                .not_null()
                .column("last_updated_block", ColumnType::BigInt)
                .not_null()
        })
        .table("collection_info", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("name", ColumnType::Text)
                .not_null()
                .column("symbol", ColumnType::Text)
                .not_null()
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_builds_with_five_tables() {
        let schema = build();
        assert_eq!(schema.tables.len(), 5);
        let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"tokens"));
        assert!(names.contains(&"transfers"));
        assert!(names.contains(&"holders"));
        assert!(names.contains(&"collection_stats"));
        assert!(names.contains(&"collection_info"));
    }

    #[test]
    fn tokens_table_has_nullable_token_uri() {
        let schema = build();
        let tokens = schema.tables.iter().find(|t| t.name == "tokens").unwrap();
        let uri_col = tokens
            .columns
            .iter()
            .find(|c| c.name == "token_uri")
            .unwrap();
        assert!(uri_col.nullable);
    }
}
