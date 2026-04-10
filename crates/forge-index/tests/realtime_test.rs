//! Realtime sync integration tests.
//!
//! Tests for the realtime processing components (finality, chain state).
//! Full WebSocket tests require a live node, so we test the components directly.

mod common;

use forge_index_sync::{ChainState, FinalityTracker, ReorgDecision, ReorgDetector};
use forge_index_core::types::Hash32;

fn hash(n: u8) -> Hash32 {
    Hash32([n; 32])
}

#[test]
fn test_finality_tracker_depth() {
    let tracker = FinalityTracker::default();

    // Block 100, with default depth 32, finalized = 100 - 32 = 68
    assert_eq!(tracker.finalized_block(100), 68);
    assert!(tracker.is_finalized(60, 100));
    assert!(tracker.is_finalized(68, 100));
    assert!(!tracker.is_finalized(69, 100));
    assert!(!tracker.is_finalized(100, 100));
}

#[test]
fn test_finality_tracker_early_blocks() {
    let tracker = FinalityTracker::default();

    // Block 10 with depth 32: finalized_block saturates to 0
    assert_eq!(tracker.finalized_block(10), 0);
    // Block 0 is NOT finalized at current_block 10 (0+32 > 10)
    assert!(!tracker.is_finalized(0, 10));
    // Block 0 is finalized at current_block 32 (0+32 <= 32)
    assert!(tracker.is_finalized(0, 32));
}

#[tokio::test]
async fn test_ready_state_after_backfill_simulation() {
    // Simulate the readiness signaling via watch channel
    let (tx, mut rx) = tokio::sync::watch::channel(false);

    // Initially not ready
    assert!(!*rx.borrow());

    // Signal ready (as backfill completion would)
    tx.send(true).unwrap();

    assert!(*rx.borrow());
}

#[tokio::test]
async fn test_reorg_detector_sequential_blocks() {
    let detector = ReorgDetector::new();

    // Process 10 sequential blocks normally
    for i in 1..=10u64 {
        let block = forge_index_core::types::Block {
            chain_id: 1,
            number: i,
            hash: hash(i as u8),
            parent_hash: hash((i - 1) as u8),
            timestamp: 1_600_000_000 + i * 12,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: Some(1_000_000_000),
            miner: forge_index_core::types::Address([0u8; 20]),
        };

        let decision = detector.process_block(1, &block).await.unwrap();
        assert_eq!(decision, ReorgDecision::Normal, "Block {} should be normal", i);
    }

    // Verify chain state
    let state = detector.get_state(1).unwrap();
    assert_eq!(state.latest_block(), Some((10, hash(10))));
}

#[tokio::test]
async fn test_multiple_chains_independent() {
    let detector = ReorgDetector::new();

    // Chain 1: blocks 1-5
    for i in 1..=5u64 {
        let mut block = forge_index_core::types::Block {
            chain_id: 1,
            number: i,
            hash: hash(i as u8),
            parent_hash: hash((i - 1) as u8),
            timestamp: 0,
            gas_limit: 0,
            gas_used: 0,
            base_fee_per_gas: None,
            miner: forge_index_core::types::Address([0u8; 20]),
        };
        detector.process_block(1, &block).await.unwrap();
    }

    // Chain 42: blocks 1-3
    for i in 1..=3u64 {
        let block = forge_index_core::types::Block {
            chain_id: 42,
            number: i,
            hash: hash(i as u8 + 100),
            parent_hash: hash(if i > 1 { (i - 1) as u8 + 100 } else { 0 }),
            timestamp: 0,
            gas_limit: 0,
            gas_used: 0,
            base_fee_per_gas: None,
            miner: forge_index_core::types::Address([0u8; 20]),
        };
        detector.process_block(42, &block).await.unwrap();
    }

    let state1 = detector.get_state(1).unwrap();
    let state42 = detector.get_state(42).unwrap();

    assert_eq!(state1.len(), 5);
    assert_eq!(state42.len(), 3);
}
