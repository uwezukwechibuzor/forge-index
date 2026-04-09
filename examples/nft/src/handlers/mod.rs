//! Event handlers for the NFT indexer.

pub mod block;
pub mod setup;
pub mod transfer;

#[allow(unused_imports)]
pub use block::handle_block;
pub use setup::handle_setup;
pub use transfer::handle_transfer;
