//! Schema definition for the Uniswap V3 indexer.

use forge_index_config::{ColumnType, Schema, SchemaBuilder};

/// Builds the data schema for the Uniswap V3 indexer.
pub fn build() -> Schema {
    SchemaBuilder::new()
        .table("pools", |t| {
            t.column("address", ColumnType::Address)
                .primary_key()
                .column("token0", ColumnType::Address)
                .not_null()
                .column("token1", ColumnType::Address)
                .not_null()
                .column("fee_tier", ColumnType::Int)
                .not_null()
                .column("tick_spacing", ColumnType::Int)
                .not_null()
                .column("sqrt_price", ColumnType::Text)
                .not_null()
                .column("current_tick", ColumnType::Int)
                .not_null()
                .column("liquidity", ColumnType::Text)
                .not_null()
                .column("created_at_block", ColumnType::BigInt)
                .not_null()
                .index(&["token0"])
                .index(&["token1"])
        })
        .table("swaps", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("pool", ColumnType::Address)
                .not_null()
                .column("sender", ColumnType::Address)
                .not_null()
                .column("recipient", ColumnType::Address)
                .not_null()
                .column("amount0", ColumnType::Text)
                .not_null()
                .column("amount1", ColumnType::Text)
                .not_null()
                .column("sqrt_price_after", ColumnType::Text)
                .not_null()
                .column("liquidity_after", ColumnType::Text)
                .not_null()
                .column("tick_after", ColumnType::Int)
                .not_null()
                .column("block_number", ColumnType::BigInt)
                .not_null()
                .column("timestamp", ColumnType::BigInt)
                .not_null()
                .column("tx_hash", ColumnType::Hash)
                .not_null()
                .index(&["pool"])
                .index(&["block_number"])
        })
        .table("mints", |t| {
            t.column("id", ColumnType::Text)
                .primary_key()
                .column("pool", ColumnType::Address)
                .not_null()
                .column("owner", ColumnType::Address)
                .not_null()
                .column("tick_lower", ColumnType::Int)
                .not_null()
                .column("tick_upper", ColumnType::Int)
                .not_null()
                .column("amount", ColumnType::Text)
                .not_null()
                .column("amount0", ColumnType::Text)
                .not_null()
                .column("amount1", ColumnType::Text)
                .not_null()
                .column("block_number", ColumnType::BigInt)
                .not_null()
                .index(&["pool"])
        })
        .table("pool_stats", |t| {
            t.column("pool", ColumnType::Address)
                .primary_key()
                .column("total_swaps", ColumnType::BigInt)
                .not_null()
                .column("total_volume_token0", ColumnType::Text)
                .not_null()
                .column("total_volume_token1", ColumnType::Text)
                .not_null()
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_builds_with_four_tables() {
        let schema = build();
        assert_eq!(schema.tables.len(), 4);
        let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"pools"));
        assert!(names.contains(&"swaps"));
        assert!(names.contains(&"mints"));
        assert!(names.contains(&"pool_stats"));
    }

    #[test]
    fn pools_table_has_correct_columns() {
        let schema = build();
        let pools = &schema.tables[0];
        assert_eq!(pools.name, "pools");
        let col_names: Vec<&str> = pools.columns.iter().map(|c| c.name.as_str()).collect();
        assert!(col_names.contains(&"address"));
        assert!(col_names.contains(&"token0"));
        assert!(col_names.contains(&"token1"));
        assert!(col_names.contains(&"fee_tier"));
        assert!(col_names.contains(&"sqrt_price"));
        assert!(col_names.contains(&"liquidity"));
    }
}
