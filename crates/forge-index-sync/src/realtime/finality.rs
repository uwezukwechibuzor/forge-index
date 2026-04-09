//! Finality tracker — determines when blocks are safe to consider final.

/// Default confirmation depth for most EVM chains.
pub const DEFAULT_CONFIRMED_DEPTH: u64 = 32;

/// Tracks block finality to determine when shadow table pruning is safe.
pub struct FinalityTracker {
    /// Number of blocks after which a block is considered finalized.
    confirmed_depth: u64,
}

impl FinalityTracker {
    /// Creates a new finality tracker with the given confirmation depth.
    pub fn new(confirmed_depth: u64) -> Self {
        Self { confirmed_depth }
    }

    /// Returns `true` if the given block is considered finalized.
    pub fn is_finalized(&self, block_number: u64, current_block: u64) -> bool {
        block_number + self.confirmed_depth <= current_block
    }

    /// Returns the latest block number that is considered finalized.
    pub fn finalized_block(&self, current_block: u64) -> u64 {
        current_block.saturating_sub(self.confirmed_depth)
    }
}

impl Default for FinalityTracker {
    fn default() -> Self {
        Self::new(DEFAULT_CONFIRMED_DEPTH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_finalized_returns_false_for_recent_blocks() {
        let tracker = FinalityTracker::new(32);
        assert!(!tracker.is_finalized(100, 120)); // only 20 deep, need 32
        assert!(!tracker.is_finalized(100, 131)); // only 31 deep
    }

    #[test]
    fn is_finalized_returns_true_for_old_blocks() {
        let tracker = FinalityTracker::new(32);
        assert!(tracker.is_finalized(100, 132)); // exactly 32 deep
        assert!(tracker.is_finalized(100, 200)); // well past finality
        assert!(tracker.is_finalized(0, 32)); // genesis finalized at block 32
    }

    #[test]
    fn finalized_block_calculation() {
        let tracker = FinalityTracker::new(32);
        assert_eq!(tracker.finalized_block(100), 68);
        assert_eq!(tracker.finalized_block(32), 0);
        assert_eq!(tracker.finalized_block(10), 0); // saturating sub
    }
}
