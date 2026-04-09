//! Event registry — maps event signatures to handlers.

use crate::registry::handler::{HandlerFn, SetupHandlerFn};
use std::collections::HashMap;
use std::sync::Arc;

/// Maps `"ContractName:EventName"` keys to handler functions.
///
/// This is the central routing table that determines which handler
/// processes each decoded event.
pub struct EventRegistry {
    handlers: HashMap<String, Arc<dyn HandlerFn>>,
    setup_handlers: HashMap<String, Arc<dyn SetupHandlerFn>>,
}

impl EventRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            setup_handlers: HashMap::new(),
        }
    }

    /// Registers an event handler.
    ///
    /// The key format is `"ContractName:EventName"` (e.g. `"ERC20:Transfer"`).
    pub fn register(&mut self, key: &str, handler: impl HandlerFn) {
        self.handlers.insert(key.to_string(), Arc::new(handler));
    }

    /// Registers a setup handler for a contract.
    pub fn register_setup(&mut self, contract: &str, handler: impl SetupHandlerFn) {
        self.setup_handlers
            .insert(contract.to_string(), Arc::new(handler));
    }

    /// Returns the handler for the given key, if registered.
    pub fn get(&self, key: &str) -> Option<Arc<dyn HandlerFn>> {
        self.handlers.get(key).cloned()
    }

    /// Returns the setup handler for the given contract, if registered.
    pub fn get_setup(&self, contract: &str) -> Option<Arc<dyn SetupHandlerFn>> {
        self.setup_handlers.get(contract).cloned()
    }

    /// Returns `true` if a handler is registered for the given key.
    pub fn has_handler(&self, key: &str) -> bool {
        self.handlers.contains_key(key)
    }

    /// Returns all registered handler keys.
    pub fn all_keys(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}
