//! Database layer for the forge-index EVM indexing framework.
//!
//! Provides schema management, an in-memory write buffer with bulk flush,
//! a reorg shadow table system, and a high-level `DbContext` API for
//! user indexing handlers.

pub mod buffer;
pub mod context;
pub mod error;
pub mod handler;
pub mod manager;
pub mod query;
pub mod reorg;
pub mod row;

#[cfg(test)]
mod tests;

pub use buffer::{FlushStats, WriteBuffer};
pub use context::DbContext;
pub use error::DbError;
pub use handler::{EventHandlerFn, SetupEventHandlerFn};
pub use manager::{BuildIdStatus, DatabaseManager};
pub use query::{Dir, QueryBuilder};
pub use reorg::{Operation, ReorgStore};
pub use row::{ColumnValue, Row};
