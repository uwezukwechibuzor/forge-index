//! Configuration builder for the forge-index EVM indexing framework.
//!
//! Provides a fluent builder API to define chains, contracts, factory patterns,
//! block intervals, accounts, schema, and database connection.

mod account;
mod block_interval;
mod builder;
mod chain;
mod contract;
mod database;
mod ordering;
mod schema;
mod validation;

pub use account::AccountConfig;
pub use block_interval::BlockIntervalConfig;
pub use builder::{Config, ConfigBuilder};
pub use chain::{ChainConfig, TransportConfig};
pub use contract::{AddressConfig, ContractConfig, EndBlock, FactoryConfig, FilterConfig};
pub use database::DatabaseConfig;
pub use ordering::Ordering;
pub use schema::{ColumnDef, ColumnType, IndexDef, Schema, SchemaBuilder, TableBuilder, TableDef};
