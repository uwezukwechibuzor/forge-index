//! forge-index — the top-level library crate for building EVM indexers.
//!
//! This is the public API that users import to define and run their indexer.
//!
//! # Quick Start
//! ```rust,ignore
//! use forge_index::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = ConfigBuilder::new()
//!         .chain("mainnet", |c| {
//!             c.chain_id = 1;
//!             c.rpc_http = "https://eth.rpc.example".to_string();
//!         })
//!         .contract("ERC20", |c| {
//!             c.abi_json = include_str!("erc20.json").to_string();
//!             c.chain_names = vec!["mainnet".to_string()];
//!             c.start_block = 18_000_000;
//!         })
//!         .schema(schema)
//!         .database(DatabaseConfig::postgres("postgres://localhost/mydb"))
//!         .build()?;
//!
//!     ForgeIndex::new()
//!         .config(config)
//!         .schema(schema)
//!         .on("ERC20:Transfer", handle_transfer)
//!         .run()
//!         .await
//! }
//! ```

pub mod builder;
pub mod context;
pub mod prelude;
pub mod runner;
pub mod shutdown;

pub use builder::ForgeIndex;
pub use runner::ForgeIndexRunner;

// Re-export key sub-crates for direct access
pub use forge_index_api as api;
pub use forge_index_config as config;
pub use forge_index_core as core;
pub use forge_index_db as db;
pub use forge_index_rpc as rpc;
pub use forge_index_sync as sync;
pub use forge_index_telemetry as telemetry;

#[cfg(test)]
mod tests;
