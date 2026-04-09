//! Configuration validation logic.

use crate::builder::Config;
use std::collections::HashSet;

/// Validates a complete configuration and returns all validation errors.
///
/// Returns an empty vec if the configuration is valid.
pub fn validate(config: &Config) -> Vec<String> {
    let mut errors = Vec::new();

    let chain_names: HashSet<&str> = config.chains.iter().map(|c| c.name.as_str()).collect();

    // Every chain has a non-empty rpc_http URL
    for chain in &config.chains {
        if chain.rpc_http.is_empty() {
            errors.push(format!("Chain '{}' has an empty rpc_http URL", chain.name));
        }
    }

    // Every chain has a unique chain_id
    let mut seen_chain_ids = HashSet::new();
    for chain in &config.chains {
        if !seen_chain_ids.insert(chain.chain_id) {
            errors.push(format!("Duplicate chain_id: {}", chain.chain_id));
        }
    }

    // Every contract references a chain name that exists
    for contract in &config.contracts {
        for chain_name in &contract.chain_names {
            if !chain_names.contains(chain_name.as_str()) {
                errors.push(format!(
                    "Contract '{}' references unknown chain '{}'",
                    contract.name, chain_name
                ));
            }
        }
    }

    // Every contract has valid JSON in abi_json
    for contract in &config.contracts {
        if serde_json::from_str::<serde_json::Value>(&contract.abi_json).is_err() {
            errors.push(format!("Contract '{}' has invalid ABI JSON", contract.name));
        }
    }

    // Every account references a chain name that exists
    for account in &config.accounts {
        for chain_name in &account.chain_names {
            if !chain_names.contains(chain_name.as_str()) {
                errors.push(format!(
                    "Account '{}' references unknown chain '{}'",
                    account.name, chain_name
                ));
            }
        }
    }

    // Every block_interval references a chain name that exists
    for bi in &config.block_intervals {
        if !chain_names.contains(bi.chain_name.as_str()) {
            errors.push(format!(
                "BlockInterval '{}' references unknown chain '{}'",
                bi.name, bi.chain_name
            ));
        }
    }

    // Schema validation: no duplicate table names
    let mut seen_tables = HashSet::new();
    for table in &config.schema.tables {
        if !seen_tables.insert(&table.name) {
            errors.push(format!("Duplicate table name: '{}'", table.name));
        }
    }

    // Schema validation: each table has exactly one primary key and no duplicate columns
    for table in &config.schema.tables {
        let pk_count = table.columns.iter().filter(|c| c.primary_key).count();
        if pk_count == 0 {
            errors.push(format!("Table '{}' has no primary key column", table.name));
        } else if pk_count > 1 {
            errors.push(format!(
                "Table '{}' has {} primary key columns, expected exactly 1",
                table.name, pk_count
            ));
        }

        let mut seen_cols = HashSet::new();
        for col in &table.columns {
            if !seen_cols.insert(&col.name) {
                errors.push(format!(
                    "Table '{}' has duplicate column name: '{}'",
                    table.name, col.name
                ));
            }
        }
    }

    // Database validation
    match &config.database {
        crate::DatabaseConfig::Postgres {
            connection_string, ..
        } => {
            if connection_string.is_empty() {
                errors.push("Postgres connection_string is empty".to_string());
            }
        }
        crate::DatabaseConfig::PGlite { .. } => {}
    }

    errors
}
