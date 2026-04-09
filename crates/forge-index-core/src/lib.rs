//! Core types and error definitions for the forge-index EVM indexing framework.

pub mod abi;
pub mod error;
pub mod registry;
pub mod types;

pub use error::ForgeError;
pub use types::*;
