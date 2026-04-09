//! Reorg detector — compares parent hashes to detect chain reorganizations.

use std::sync::Arc;

use dashmap::DashMap;
use forge_index_core::types::{Block, Hash32};
use forge_index_rpc::CachedRpcClient;

use crate::error::SyncError;
use crate::reorg::chain_state::{ChainState, DEFAULT_CAPACITY};

/// The result of checking a new block against our known chain state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReorgDecision {
    /// The block extends our known chain normally.
    Normal,
    /// A chain reorganization was detected.
    Reorg {
        /// The chain where the reorg occurred.
        chain_id: u64,
        /// The first block number that differs from our known chain.
        fork_block: u64,
        /// The new chain tip block number.
        new_tip: u64,
    },
}

/// Detects chain reorganizations by comparing parent hashes of incoming blocks
/// against the stored chain state.
pub struct ReorgDetector {
    /// Per-chain block state.
    chain_states: DashMap<u64, ChainState>,
    /// RPC clients per chain for walking back the chain.
    clients: DashMap<u64, Arc<CachedRpcClient>>,
}

impl ReorgDetector {
    /// Creates a new reorg detector.
    pub fn new() -> Self {
        Self {
            chain_states: DashMap::new(),
            clients: DashMap::new(),
        }
    }

    /// Registers an RPC client for a chain.
    pub fn register_client(&self, chain_id: u64, client: Arc<CachedRpcClient>) {
        self.clients.insert(chain_id, client);
    }

    /// Processes a new block and returns a reorg decision.
    ///
    /// If the block's parent hash matches our stored hash for `block.number - 1`,
    /// the block extends the chain normally. Otherwise, a reorg is detected and
    /// we walk back to find the fork point.
    pub async fn process_block(
        &self,
        chain_id: u64,
        block: &Block,
    ) -> Result<ReorgDecision, SyncError> {
        let mut state = self.chain_states.entry(chain_id).or_default();

        // If this is the first block we see, just accept it
        if state.is_empty() {
            state.push(block.number, block.hash);
            return Ok(ReorgDecision::Normal);
        }

        // Check if parent hash matches what we have for block.number - 1
        if block.number == 0 {
            state.push(block.number, block.hash);
            return Ok(ReorgDecision::Normal);
        }

        let parent_number = block.number - 1;
        match state.get_hash(parent_number) {
            Some(stored_hash) if stored_hash == block.parent_hash => {
                // Normal: parent matches
                state.push(block.number, block.hash);
                Ok(ReorgDecision::Normal)
            }
            Some(_stored_hash) => {
                // Mismatch: reorg detected — walk back to find fork point
                drop(state); // Release DashMap lock before async work
                let fork_block = self.find_fork_point(chain_id, block).await?;
                // Prune our state above the fork point
                let mut state = self.chain_states.get_mut(&chain_id).unwrap();
                state.prune_above(fork_block.saturating_sub(1));
                Ok(ReorgDecision::Reorg {
                    chain_id,
                    fork_block,
                    new_tip: block.number,
                })
            }
            None => {
                // We don't have the parent block stored — this can happen if we
                // skipped blocks. Just accept the block.
                state.push(block.number, block.hash);
                Ok(ReorgDecision::Normal)
            }
        }
    }

    /// Walks back from the current tip to find the common ancestor (fork point).
    async fn find_fork_point(&self, chain_id: u64, new_block: &Block) -> Result<u64, SyncError> {
        let client = self
            .clients
            .get(&chain_id)
            .ok_or(SyncError::ChainNotFound(chain_id))?
            .clone();

        let state = self
            .chain_states
            .get(&chain_id)
            .ok_or(SyncError::ChainNotFound(chain_id))?;

        let tip = new_block.number;
        let lookback = DEFAULT_CAPACITY as u64;
        let min_block = tip.saturating_sub(lookback);

        // Walk back from the new block's parent
        let mut current_number = new_block.number.saturating_sub(1);

        while current_number >= min_block {
            // Fetch the block at current_number from RPC (the "new" chain)
            let rpc_block = client
                .get_block_by_number(current_number)
                .await
                .map_err(|e| SyncError::Rpc {
                    chain_id,
                    source: e,
                })?;

            // Compare with our stored hash
            if let Some(stored_hash) = state.get_hash(current_number) {
                if stored_hash == rpc_block.hash {
                    // Found common ancestor — fork starts at current_number + 1
                    return Ok(current_number + 1);
                }
            } else {
                // We don't have this block stored, so the fork is at least here
                return Ok(current_number + 1);
            }

            if current_number == 0 {
                break;
            }
            current_number -= 1;
        }

        Err(SyncError::DeepReorg {
            chain_id,
            depth: lookback,
        })
    }

    /// Returns a reference to the chain state for the given chain.
    pub fn get_state(
        &self,
        chain_id: u64,
    ) -> Option<dashmap::mapref::one::Ref<'_, u64, ChainState>> {
        self.chain_states.get(&chain_id)
    }

    /// Seeds the chain state with a block (e.g., after backfill).
    pub fn seed_block(&self, chain_id: u64, number: u64, hash: Hash32) {
        self.chain_states
            .entry(chain_id)
            .or_default()
            .push(number, hash);
    }
}

impl Default for ReorgDetector {
    fn default() -> Self {
        Self::new()
    }
}
