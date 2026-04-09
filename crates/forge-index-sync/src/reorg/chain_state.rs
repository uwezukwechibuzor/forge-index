//! In-memory recent block hash store for reorg detection.

use forge_index_core::types::Hash32;
use std::collections::VecDeque;

/// Default number of recent blocks to keep.
pub const DEFAULT_CAPACITY: usize = 128;

/// Stores the last N block hashes in memory to enable reorg detection.
///
/// Uses a `VecDeque` of `(block_number, block_hash)` pairs, ordered by
/// block number ascending. When capacity is exceeded, the oldest entry
/// is removed.
pub struct ChainState {
    recent_blocks: VecDeque<(u64, Hash32)>,
    capacity: usize,
}

impl ChainState {
    /// Creates a new chain state with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            recent_blocks: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Pushes a new block into the state, evicting the oldest if at capacity.
    pub fn push(&mut self, number: u64, hash: Hash32) {
        if self.recent_blocks.len() >= self.capacity {
            self.recent_blocks.pop_front();
        }
        self.recent_blocks.push_back((number, hash));
    }

    /// Returns the stored hash for the given block number, if present.
    pub fn get_hash(&self, number: u64) -> Option<Hash32> {
        self.recent_blocks
            .iter()
            .find(|(n, _)| *n == number)
            .map(|(_, h)| *h)
    }

    /// Returns the latest (highest) block number and hash.
    pub fn latest_block(&self) -> Option<(u64, Hash32)> {
        self.recent_blocks.back().copied()
    }

    /// Removes all blocks with number strictly greater than `block_number`.
    pub fn prune_above(&mut self, block_number: u64) {
        while let Some(&(n, _)) = self.recent_blocks.back() {
            if n > block_number {
                self.recent_blocks.pop_back();
            } else {
                break;
            }
        }
    }

    /// Returns the number of blocks currently stored.
    pub fn len(&self) -> usize {
        self.recent_blocks.len()
    }

    /// Returns true if no blocks are stored.
    pub fn is_empty(&self) -> bool {
        self.recent_blocks.is_empty()
    }
}

impl Default for ChainState {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> Hash32 {
        Hash32([n; 32])
    }

    #[test]
    fn push_and_get_hash() {
        let mut state = ChainState::new(10);
        state.push(1, hash(0x01));
        state.push(2, hash(0x02));
        state.push(3, hash(0x03));

        assert_eq!(state.get_hash(1), Some(hash(0x01)));
        assert_eq!(state.get_hash(2), Some(hash(0x02)));
        assert_eq!(state.get_hash(3), Some(hash(0x03)));
        assert_eq!(state.get_hash(4), None);
    }

    #[test]
    fn latest_block() {
        let mut state = ChainState::new(10);
        assert_eq!(state.latest_block(), None);

        state.push(5, hash(0x05));
        state.push(6, hash(0x06));

        assert_eq!(state.latest_block(), Some((6, hash(0x06))));
    }

    #[test]
    fn prune_above_removes_correct_entries() {
        let mut state = ChainState::new(10);
        for i in 1..=10 {
            state.push(i, hash(i as u8));
        }
        assert_eq!(state.len(), 10);

        state.prune_above(7);

        assert_eq!(state.len(), 7);
        assert_eq!(state.latest_block(), Some((7, hash(7))));
        assert_eq!(state.get_hash(8), None);
        assert_eq!(state.get_hash(7), Some(hash(7)));
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut state = ChainState::new(3);
        state.push(1, hash(0x01));
        state.push(2, hash(0x02));
        state.push(3, hash(0x03));
        state.push(4, hash(0x04)); // should evict block 1

        assert_eq!(state.len(), 3);
        assert_eq!(state.get_hash(1), None);
        assert_eq!(state.get_hash(2), Some(hash(0x02)));
        assert_eq!(state.get_hash(4), Some(hash(0x04)));
    }
}
