//! Sync engine for the forge-index EVM indexing framework.
//!
//! Provides the historical backfill engine, realtime block processing via
//! WebSocket subscription, and chain reorganization detection and rollback.

pub mod backfill;
pub mod error;
pub mod factory;
pub mod realtime;
pub mod reorg;

#[cfg(test)]
mod tests;

pub use backfill::{
    BackfillCoordinator, BackfillPlan, BackfillProgress, BackfillWorker, BlockRange,
};
pub use error::SyncError;
pub use factory::FactoryAddressTracker;
pub use realtime::{FinalityTracker, NewBlockSubscriber, RealtimeProcessor};
pub use reorg::{ChainState, ReorgDecision, ReorgDetector, ReorgHandler};
