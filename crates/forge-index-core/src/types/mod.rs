//! Core EVM types used across the forge-index framework.

mod address;
mod block;
mod event;
mod hash;
mod log;
mod trace;
mod transaction;

pub use address::Address;
pub use block::Block;
pub use event::Event;
pub use hash::Hash32;
pub use log::Log;
pub use trace::{Trace, TraceType};
pub use transaction::Transaction;
