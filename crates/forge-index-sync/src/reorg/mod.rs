//! Chain reorganization detection and rollback.

pub mod chain_state;
pub mod detector;
pub mod handler;

pub use chain_state::ChainState;
pub use detector::{ReorgDecision, ReorgDetector};
pub use handler::ReorgHandler;
