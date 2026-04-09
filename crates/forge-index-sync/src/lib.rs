//! Sync engine for the forge-index EVM indexing framework.
//!
//! Provides the historical backfill engine that fetches all events from
//! startBlock to the current block for each configured contract.

pub mod backfill;
pub mod error;
pub mod factory;

#[cfg(test)]
mod tests;

pub use backfill::{
    BackfillCoordinator, BackfillPlan, BackfillProgress, BackfillWorker, BlockRange,
};
pub use error::SyncError;
pub use factory::FactoryAddressTracker;
