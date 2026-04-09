//! Event handlers for the Uniswap V3 indexer.

pub mod initialize;
pub mod mint;
pub mod pool_created;
pub mod swap;

pub use initialize::handle_initialize;
pub use mint::handle_mint;
pub use pool_created::handle_pool_created;
pub use swap::handle_swap;
