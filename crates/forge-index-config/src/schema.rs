//! Schema definition types and builders for defining the indexing data model.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The data type of a column in a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    /// Variable-length text.
    Text,
    /// Boolean value.
    Boolean,
    /// 32-bit integer.
    Int,
    /// Arbitrary-precision numeric (for u256 and similar large values).
    BigInt,
    /// Double-precision floating point.
    Float,
    /// Hex-encoded binary data stored as text.
    Hex,
    /// Ethereum address stored as checksummed hex text.
    Address,
    /// 32-byte hash stored as hex text.
    Hash,
    /// JSON data.
    Json,
    /// Raw bytes stored as hex text.
    Bytes,
    /// Unix timestamp in seconds (stored as BIGINT).
    Timestamp,
}

impl ColumnType {
    /// Returns the PostgreSQL type name for this column type.
    pub fn to_sql_type(&self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Boolean => "BOOLEAN",
            Self::Int => "INTEGER",
            Self::BigInt => "NUMERIC",
            Self::Float => "DOUBLE PRECISION",
            Self::Hex => "TEXT",
            Self::Address => "TEXT",
            Self::Hash => "TEXT",
            Self::Json => "JSONB",
            Self::Bytes => "TEXT",
            Self::Timestamp => "BIGINT",
        }
    }
}

/// A column definition within a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// The column name.
    pub name: String,
    /// The column data type.
    pub col_type: ColumnType,
    /// Whether the column allows NULL values.
    pub nullable: bool,
    /// Whether this column is the primary key.
    pub primary_key: bool,
    /// Optional foreign key reference as (table_name, column_name).
    pub references: Option<(String, String)>,
}

/// An index definition on a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDef {
    /// The columns included in the index.
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
}

/// A table definition within the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDef {
    /// The table name.
    pub name: String,
    /// The column definitions.
    pub columns: Vec<ColumnDef>,
    /// The index definitions.
    pub indexes: Vec<IndexDef>,
}

/// The complete schema defining all tables for the indexer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// All table definitions in the schema.
    pub tables: Vec<TableDef>,
}

impl Schema {
    /// Generates SQL CREATE TABLE and CREATE INDEX statements.
    ///
    /// For each table, generates:
    /// - The main table with all columns, constraints, and foreign keys
    /// - A `_reorg_{table}` shadow table with the same columns plus
    ///   `_operation TEXT` and `_block_number BIGINT`
    /// - All index statements for both tables
    pub fn to_create_sql(&self, pg_schema: &str) -> Vec<String> {
        let mut stmts = Vec::new();

        for table in &self.tables {
            // Main table
            stmts.push(self.table_create_sql(pg_schema, table, false));

            // Indexes on main table
            for idx in &table.indexes {
                stmts.push(self.index_create_sql(pg_schema, &table.name, idx));
            }

            // Shadow reorg table
            stmts.push(self.table_create_sql(pg_schema, table, true));
        }

        stmts
    }

    /// Computes a deterministic build ID (SHA-256 hash) of the schema.
    ///
    /// Two schemas with the same structure will always produce the same build ID.
    pub fn build_id(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn table_create_sql(&self, pg_schema: &str, table: &TableDef, is_reorg: bool) -> String {
        let table_name = if is_reorg {
            format!("_reorg_{}", table.name)
        } else {
            table.name.clone()
        };

        let mut col_defs: Vec<String> = table
            .columns
            .iter()
            .map(|col| {
                let mut parts = vec![
                    format!("\"{}\"", col.name),
                    col.col_type.to_sql_type().to_string(),
                ];
                if !col.nullable {
                    parts.push("NOT NULL".to_string());
                }
                if col.primary_key && !is_reorg {
                    parts.push("PRIMARY KEY".to_string());
                }
                parts.join(" ")
            })
            .collect();

        if is_reorg {
            col_defs.push("\"_operation\" TEXT NOT NULL".to_string());
            col_defs.push("\"_block_number\" BIGINT NOT NULL".to_string());
        }

        // Foreign key constraints (only on main table)
        if !is_reorg {
            for col in &table.columns {
                if let Some((ref_table, ref_col)) = &col.references {
                    col_defs.push(format!(
                        "FOREIGN KEY (\"{}\") REFERENCES \"{}\".\"{}\"(\"{}\")",
                        col.name, pg_schema, ref_table, ref_col
                    ));
                }
            }
        }

        format!(
            "CREATE TABLE IF NOT EXISTS \"{}\".\"{}\" ({});",
            pg_schema,
            table_name,
            col_defs.join(", ")
        )
    }

    fn index_create_sql(&self, pg_schema: &str, table_name: &str, idx: &IndexDef) -> String {
        let unique = if idx.unique { "UNIQUE " } else { "" };
        let cols: Vec<String> = idx.columns.iter().map(|c| format!("\"{}\"", c)).collect();
        let idx_name = format!("idx_{}_{}", table_name, idx.columns.join("_"));
        format!(
            "CREATE {}INDEX IF NOT EXISTS \"{}\" ON \"{}\".\"{}\" ({});",
            unique,
            idx_name,
            pg_schema,
            table_name,
            cols.join(", ")
        )
    }
}

/// Fluent builder for constructing a [`Schema`].
pub struct SchemaBuilder {
    tables: Vec<TableDef>,
}

impl SchemaBuilder {
    /// Creates a new empty schema builder.
    pub fn new() -> Self {
        Self { tables: Vec::new() }
    }

    /// Adds a table to the schema using a closure that configures the table builder.
    pub fn table(mut self, name: &str, f: impl FnOnce(TableBuilder) -> TableBuilder) -> Self {
        let builder = f(TableBuilder::new(name));
        self.tables.push(builder.build());
        self
    }

    /// Builds the final [`Schema`].
    pub fn build(self) -> Schema {
        Schema {
            tables: self.tables,
        }
    }
}

impl Default for SchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for constructing a [`TableDef`].
pub struct TableBuilder {
    name: String,
    columns: Vec<ColumnDef>,
    indexes: Vec<IndexDef>,
}

impl TableBuilder {
    /// Creates a new table builder with the given table name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            columns: Vec::new(),
            indexes: Vec::new(),
        }
    }

    /// Adds a non-nullable column with the given name and type.
    pub fn column(mut self, name: &str, col_type: ColumnType) -> Self {
        self.columns.push(ColumnDef {
            name: name.to_string(),
            col_type,
            nullable: false,
            primary_key: false,
            references: None,
        });
        self
    }

    /// Marks the last added column as the primary key.
    pub fn primary_key(mut self) -> Self {
        if let Some(col) = self.columns.last_mut() {
            col.primary_key = true;
        }
        self
    }

    /// Marks the last added column as NOT NULL (default behavior).
    pub fn not_null(mut self) -> Self {
        if let Some(col) = self.columns.last_mut() {
            col.nullable = false;
        }
        self
    }

    /// Marks the last added column as nullable.
    pub fn nullable(mut self) -> Self {
        if let Some(col) = self.columns.last_mut() {
            col.nullable = true;
        }
        self
    }

    /// Adds a foreign key reference on the last added column.
    pub fn references(mut self, table: &str, column: &str) -> Self {
        if let Some(col) = self.columns.last_mut() {
            col.references = Some((table.to_string(), column.to_string()));
        }
        self
    }

    /// Adds a non-unique index on the given columns.
    pub fn index(mut self, columns: &[&str]) -> Self {
        self.indexes.push(IndexDef {
            columns: columns.iter().map(|s| s.to_string()).collect(),
            unique: false,
        });
        self
    }

    /// Adds a unique index on the given columns.
    pub fn unique_index(mut self, columns: &[&str]) -> Self {
        self.indexes.push(IndexDef {
            columns: columns.iter().map(|s| s.to_string()).collect(),
            unique: true,
        });
        self
    }

    /// Builds the final [`TableDef`].
    pub fn build(self) -> TableDef {
        TableDef {
            name: self.name,
            columns: self.columns,
            indexes: self.indexes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> Schema {
        SchemaBuilder::new()
            .table("transfers", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("from", ColumnType::Address)
                    .not_null()
                    .column("to", ColumnType::Address)
                    .not_null()
                    .column("amount", ColumnType::BigInt)
                    .not_null()
                    .column("block_number", ColumnType::BigInt)
                    .not_null()
                    .index(&["from"])
                    .index(&["to"])
            })
            .build()
    }

    #[test]
    fn schema_to_create_sql_generates_valid_sql_with_correct_column_types() {
        let schema = sample_schema();
        let sql = schema.to_create_sql("public");

        let create = &sql[0];
        assert!(create.contains("CREATE TABLE IF NOT EXISTS \"public\".\"transfers\""));
        assert!(create.contains("\"id\" TEXT NOT NULL PRIMARY KEY"));
        assert!(create.contains("\"from\" TEXT NOT NULL"));
        assert!(create.contains("\"to\" TEXT NOT NULL"));
        assert!(create.contains("\"amount\" NUMERIC NOT NULL"));
        assert!(create.contains("\"block_number\" NUMERIC NOT NULL"));
    }

    #[test]
    fn schema_to_create_sql_generates_reorg_shadow_tables() {
        let schema = sample_schema();
        let sql = schema.to_create_sql("public");

        let reorg_sql: Vec<&String> = sql
            .iter()
            .filter(|s| s.contains("_reorg_transfers"))
            .collect();
        assert!(!reorg_sql.is_empty());

        let reorg = reorg_sql[0];
        assert!(reorg.contains("\"_operation\" TEXT NOT NULL"));
        assert!(reorg.contains("\"_block_number\" BIGINT NOT NULL"));
        // Reorg table should NOT have PRIMARY KEY
        assert!(!reorg.contains("PRIMARY KEY"));
    }

    #[test]
    fn schema_build_id_is_deterministic() {
        let schema1 = sample_schema();
        let schema2 = sample_schema();
        assert_eq!(schema1.build_id(), schema2.build_id());
    }

    #[test]
    fn schema_build_id_changes_when_schema_changes() {
        let schema1 = sample_schema();
        let schema2 = SchemaBuilder::new()
            .table("transfers", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("from", ColumnType::Address)
                    .not_null()
                    .column("extra_column", ColumnType::Text)
                    .nullable()
            })
            .build();
        assert_ne!(schema1.build_id(), schema2.build_id());
    }

    #[test]
    fn column_type_address_maps_to_text() {
        assert_eq!(ColumnType::Address.to_sql_type(), "TEXT");
    }

    #[test]
    fn column_type_bigint_maps_to_numeric() {
        assert_eq!(ColumnType::BigInt.to_sql_type(), "NUMERIC");
    }
}
