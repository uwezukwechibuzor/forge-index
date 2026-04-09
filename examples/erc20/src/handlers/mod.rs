//! Event handlers for the ERC20 indexer.

pub mod approval;
pub mod transfer;

pub use approval::handle_approval;
pub use transfer::handle_transfer;
