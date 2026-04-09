//! Event handlers for the ERC20 indexer.
//!
//! Production handlers that receive `DbContext` for database reads/writes.

pub mod approval;
pub mod transfer;

pub use approval::handle_approval;
pub use transfer::handle_transfer;
