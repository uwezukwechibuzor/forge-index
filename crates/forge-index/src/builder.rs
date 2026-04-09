//! ForgeIndex builder — the main entry point for users.

use std::collections::HashMap;
use std::sync::Arc;

use forge_index_config::{Config, Schema};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::error::ForgeError;
use forge_index_core::registry::{EventRegistry, HandlerFn, SetupHandlerFn};
use forge_index_db::handler::{EventHandlerFn, SetupEventHandlerFn};
use forge_index_telemetry::{compute_build_id, BuildInput};

use crate::runner::ForgeIndexRunner;

/// The main builder for a forge-index application.
///
/// # Example
/// ```rust,ignore
/// use forge_index::prelude::*;
///
/// ForgeIndex::new()
///     .config(config)
///     .schema(schema)
///     .on("ERC20:Transfer", handle_transfer)
///     .run()
///     .await?;
/// ```
pub struct ForgeIndex {
    config: Option<Config>,
    schema: Option<Schema>,
    /// Legacy registry (handlers take serde_json::Value context).
    legacy_registry: EventRegistry,
    /// Production registry (handlers take DbContext).
    db_handlers: HashMap<String, Arc<dyn EventHandlerFn>>,
    db_setup_handlers: HashMap<String, Arc<dyn SetupEventHandlerFn>>,
    /// Track all registered keys for validation.
    all_handler_keys: Vec<String>,
}

impl ForgeIndex {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self {
            config: None,
            schema: None,
            legacy_registry: EventRegistry::new(),
            db_handlers: HashMap::new(),
            db_setup_handlers: HashMap::new(),
            all_handler_keys: Vec::new(),
        }
    }

    /// Sets the application configuration.
    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the data schema.
    pub fn schema(mut self, schema: Schema) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Registers an event handler (legacy: takes `serde_json::Value` context).
    pub fn on(mut self, event_key: &str, handler: impl HandlerFn) -> Self {
        self.legacy_registry.register(event_key, handler);
        self.all_handler_keys.push(event_key.to_string());
        self
    }

    /// Registers an event handler (production: takes `DbContext`).
    pub fn on_db(mut self, event_key: &str, handler: impl EventHandlerFn) -> Self {
        self.db_handlers
            .insert(event_key.to_string(), Arc::new(handler));
        self.all_handler_keys.push(event_key.to_string());
        self
    }

    /// Registers a setup handler (legacy: takes `serde_json::Value`).
    pub fn setup(mut self, contract: &str, handler: impl SetupHandlerFn) -> Self {
        self.legacy_registry.register_setup(contract, handler);
        self
    }

    /// Registers a setup handler (production: takes `DbContext`).
    pub fn setup_db(mut self, contract: &str, handler: impl SetupEventHandlerFn) -> Self {
        self.db_setup_handlers
            .insert(contract.to_string(), Arc::new(handler));
        self
    }

    /// Validates configuration, schema, and handlers, then builds a runner.
    pub fn build(self) -> Result<ForgeIndexRunner, ForgeError> {
        let config = self
            .config
            .ok_or_else(|| ForgeError::Config("Config is required".to_string()))?;

        let schema = self
            .schema
            .ok_or_else(|| ForgeError::Config("Schema is required".to_string()))?;

        // Validate that every registered handler key references a known contract + event
        let mut known_events = std::collections::HashSet::new();
        for contract in &config.contracts {
            if let Ok(parsed) = parse_abi(&contract.abi_json) {
                for event in &parsed.events {
                    let key = format!("{}:{}", contract.name, event.name);
                    known_events.insert(key);
                }
            }
        }

        for handler_key in &self.all_handler_keys {
            if !known_events.contains(handler_key) {
                return Err(ForgeError::Config(format!(
                    "Handler '{}' references unknown contract or event. \
                     Ensure the contract is in the config and the event is in its ABI.",
                    handler_key
                )));
            }
        }

        // Warn for events in the ABI with no registered handler
        for event_key in &known_events {
            if !self.legacy_registry.has_handler(event_key)
                && !self.db_handlers.contains_key(event_key)
            {
                tracing::warn!(
                    event_key = event_key.as_str(),
                    "Event '{}' defined in ABI but no handler registered — events will be skipped",
                    event_key
                );
            }
        }

        // Compute build ID
        let build_id = compute_build_id(BuildInput {
            config: &config,
            schema: &schema,
            handler_keys: &self.all_handler_keys,
        });

        Ok(ForgeIndexRunner::new(
            config,
            schema,
            Arc::new(self.legacy_registry),
            self.db_handlers,
            build_id,
        ))
    }

    /// Shortcut: build() then run().
    pub async fn run(self) -> Result<(), ForgeError> {
        let runner = self.build()?;
        runner.run().await
    }
}

impl Default for ForgeIndex {
    fn default() -> Self {
        Self::new()
    }
}
