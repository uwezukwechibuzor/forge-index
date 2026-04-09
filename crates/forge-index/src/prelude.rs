//! Prelude — everything a user needs in one import.
//!
//! ```rust,ignore
//! use forge_index::prelude::*;
//! ```

// Core types
pub use forge_index_core::error::ForgeError;
pub use forge_index_core::types::{Address, Block, Event, Hash32, Log, Transaction};

// ABI types
pub use forge_index_core::abi::{DecodedEvent, DecodedParam};

// Config types
pub use forge_index_config::{
    ColumnType, Config, ConfigBuilder, DatabaseConfig, FactoryConfig, Ordering, Schema,
    SchemaBuilder, TableBuilder,
};

// Database context
pub use forge_index_db::context::DbContext;

// Registry
pub use forge_index_core::registry::{EventRegistry, HandlerFn, SetupHandlerFn};

// Builder
pub use crate::builder::ForgeIndex;

// Convenience re-export
pub use anyhow::Result;
