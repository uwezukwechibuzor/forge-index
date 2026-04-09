//! Build ID generation from config, schema, and handler keys.
//!
//! A build ID is a SHA-256 fingerprint of the entire application definition.
//! If config, schema, or registered handlers change between restarts,
//! the build ID changes, signaling that a re-index may be needed.

use forge_index_config::{Config, Schema};
use sha2::{Digest, Sha256};

/// Inputs used to compute the build ID.
pub struct BuildInput<'a> {
    /// The full application configuration.
    pub config: &'a Config,
    /// The data schema definition.
    pub schema: &'a Schema,
    /// Sorted list of registered handler keys (e.g. `["ERC20:Approval", "ERC20:Transfer"]`).
    pub handler_keys: &'a [String],
}

/// The result of comparing the current build ID against the stored one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildIdStatus {
    /// No previous build ID was found in the database.
    New,
    /// The build ID matches the stored one — nothing changed.
    Same,
    /// The build ID differs from the stored one.
    Changed {
        /// The previously stored build ID.
        old: String,
    },
}

/// Computes a deterministic build ID from the application definition.
///
/// The build ID is a SHA-256 hex digest of canonical (sorted-key) JSON
/// representations of the config, schema, and handler keys.
pub fn compute_build_id(input: BuildInput<'_>) -> String {
    let mut hasher = Sha256::new();

    // 1. Canonical config JSON
    let config_json = canonical_json(&serde_json::to_value(input.config).unwrap_or_default());
    hasher.update(config_json.as_bytes());

    // 2. Canonical schema JSON
    let schema_json = canonical_json(&serde_json::to_value(input.schema).unwrap_or_default());
    hasher.update(schema_json.as_bytes());

    // 3. Sorted handler keys
    let mut keys = input.handler_keys.to_vec();
    keys.sort();
    for key in &keys {
        hasher.update(key.as_bytes());
    }

    hex::encode(hasher.finalize())
}

/// Returns `true` if the old and new build IDs differ.
pub fn build_id_changed(old: &str, new: &str) -> bool {
    old != new
}

/// Logs the build ID status at the appropriate log level.
pub fn log_build_id_status(status: &BuildIdStatus, new_id: &str) {
    match status {
        BuildIdStatus::New => {
            tracing::info!(
                build_id = new_id,
                "Starting fresh — no previous build found (build_id: {})",
                new_id
            );
        }
        BuildIdStatus::Same => {
            tracing::debug!(build_id = new_id, "Build unchanged (build_id: {})", new_id);
        }
        BuildIdStatus::Changed { old } => {
            tracing::warn!(
                old_build_id = old.as_str(),
                new_build_id = new_id,
                "Config or schema changed since last run (old: {}, new: {}). \
                 Consider wiping the database for a clean re-index.",
                old,
                new_id
            );
        }
    }
}

/// Produces canonical JSON with object keys sorted alphabetically.
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, canonical_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", parts.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_index_config::{ColumnType, DatabaseConfig, SchemaBuilder};

    fn make_config() -> Config {
        use forge_index_config::ConfigBuilder;

        ConfigBuilder::new()
            .chain("mainnet", |c| {
                c.chain_id = 1;
                c.rpc_http = "https://eth.rpc.example".to_string();
            })
            .schema(
                SchemaBuilder::new()
                    .table("transfers", |t| {
                        t.column("id", ColumnType::Text).primary_key()
                    })
                    .build(),
            )
            .database(DatabaseConfig::postgres("postgres://localhost/test"))
            .build()
            .unwrap()
    }

    fn make_schema() -> Schema {
        SchemaBuilder::new()
            .table("transfers", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("amount", ColumnType::BigInt)
                    .not_null()
            })
            .build()
    }

    #[test]
    fn build_id_is_deterministic() {
        let config = make_config();
        let schema = make_schema();
        let keys = vec!["ERC20:Transfer".to_string()];

        let id1 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys,
        });
        let id2 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys,
        });

        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn build_id_differs_when_config_changes() {
        let config1 = make_config();
        let mut config2 = make_config();
        config2.chains[0].rpc_http = "https://other.rpc".to_string();

        let schema = make_schema();
        let keys = vec!["ERC20:Transfer".to_string()];

        let id1 = compute_build_id(BuildInput {
            config: &config1,
            schema: &schema,
            handler_keys: &keys,
        });
        let id2 = compute_build_id(BuildInput {
            config: &config2,
            schema: &schema,
            handler_keys: &keys,
        });

        assert_ne!(id1, id2);
    }

    #[test]
    fn build_id_differs_when_schema_changes() {
        let config = make_config();
        let schema1 = make_schema();
        let schema2 = SchemaBuilder::new()
            .table("transfers", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("extra", ColumnType::Text)
                    .nullable()
            })
            .build();

        let keys = vec!["ERC20:Transfer".to_string()];

        let id1 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema1,
            handler_keys: &keys,
        });
        let id2 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema2,
            handler_keys: &keys,
        });

        assert_ne!(id1, id2);
    }

    #[test]
    fn build_id_differs_when_handler_keys_change() {
        let config = make_config();
        let schema = make_schema();
        let keys1 = vec!["ERC20:Transfer".to_string()];
        let keys2 = vec!["ERC20:Transfer".to_string(), "ERC20:Approval".to_string()];

        let id1 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys1,
        });
        let id2 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys2,
        });

        assert_ne!(id1, id2);
    }

    #[test]
    fn build_id_independent_of_handler_key_order() {
        let config = make_config();
        let schema = make_schema();
        let keys1 = vec!["ERC20:Transfer".to_string(), "ERC20:Approval".to_string()];
        let keys2 = vec!["ERC20:Approval".to_string(), "ERC20:Transfer".to_string()];

        let id1 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys1,
        });
        let id2 = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &keys2,
        });

        assert_eq!(id1, id2, "build_id should be order-independent");
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let value = serde_json::json!({ "z": 1, "a": 2, "m": 3 });
        let canonical = super::canonical_json(&value);
        assert_eq!(canonical, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn build_id_changed_check() {
        assert!(build_id_changed("abc", "def"));
        assert!(!build_id_changed("abc", "abc"));
    }
}
