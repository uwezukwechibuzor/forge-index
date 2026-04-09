//! Dynamic GraphQL type mappings from config schema.

use forge_index_config::ColumnType;

/// Maps a ColumnType to its GraphQL type name.
pub fn column_type_to_gql(col_type: &ColumnType) -> &'static str {
    match col_type {
        ColumnType::Text => "String",
        ColumnType::Boolean => "Boolean",
        ColumnType::Int => "Int",
        ColumnType::BigInt => "String", // GQL Int is 32-bit; use String for i64/u256
        ColumnType::Float => "Float",
        ColumnType::Hex => "String",
        ColumnType::Address => "String",
        ColumnType::Hash => "String",
        ColumnType::Bytes => "String", // hex-encoded
        ColumnType::Json => "JSON",
        ColumnType::Timestamp => "String", // BigInt as string
    }
}

/// Converts a snake_case table name to camelCase for GQL queries.
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Converts a snake_case name to PascalCase for GQL type names.
pub fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Returns the appropriate filter type name for a column type.
pub fn filter_type_for_column(col_type: &ColumnType) -> &'static str {
    match col_type {
        ColumnType::Boolean => "BoolFilter",
        ColumnType::Int => "IntFilter",
        ColumnType::Float => "IntFilter",
        _ => "StringFilter", // text-based types use string comparison
    }
}
