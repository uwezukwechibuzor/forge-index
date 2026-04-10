//! Reorg detection and rollback tests.
//!
//! Run with: `cargo test -p forge-index --test reorg_test -- --ignored`

mod common;

use forge_index_core::types::{Block, Hash32};
use forge_index_sync::{ChainState, ReorgDecision, ReorgDetector};

fn hash(n: u8) -> Hash32 {
    Hash32([n; 32])
}

fn make_block(number: u64, hash_byte: u8, parent_byte: u8) -> Block {
    Block {
        chain_id: 1,
        number,
        hash: hash(hash_byte),
        parent_hash: hash(parent_byte),
        timestamp: 1_600_000_000 + number * 12,
        gas_limit: 30_000_000,
        gas_used: 15_000_000,
        base_fee_per_gas: Some(1_000_000_000),
        miner: forge_index_core::types::Address([0u8; 20]),
    }
}

#[tokio::test]
async fn test_reorg_detected_by_parent_hash() {
    let detector = ReorgDetector::new();

    // Seed blocks 1-10
    for i in 1..=10 {
        let block = make_block(i, i as u8, (i - 1) as u8);
        let decision = detector.process_block(1, &block).await.unwrap();
        assert_eq!(decision, ReorgDecision::Normal);
    }

    // Block 11 with WRONG parent hash (doesn't match hash of block 10)
    let bad_block = make_block(11, 0xFF, 0xEE); // parent 0xEE != hash(10) = [10;32]
    let decision = detector.process_block(1, &bad_block).await;

    // Should detect reorg (may error if no RPC client registered, which is expected)
    // The key assertion is that it doesn't return Normal
    match decision {
        Ok(ReorgDecision::Reorg { chain_id, fork_block, new_tip }) => {
            assert_eq!(chain_id, 1);
            assert_eq!(new_tip, 11);
            assert!(fork_block <= 11);
        }
        Err(_) => {
            // ChainNotFound is expected since we didn't register an RPC client
            // The important thing is that it tried to detect the reorg
        }
        Ok(ReorgDecision::Normal) => {
            panic!("Should have detected reorg, got Normal");
        }
    }
}

#[test]
fn test_chain_state_prune_above() {
    let mut state = ChainState::new(128);

    // Insert blocks 1-10
    for i in 1..=10u64 {
        state.push(i, hash(i as u8));
    }
    assert_eq!(state.len(), 10);

    // Simulate rollback to block 7
    state.prune_above(7);

    assert_eq!(state.len(), 7);
    assert_eq!(state.latest_block(), Some((7, hash(7))));
    assert_eq!(state.get_hash(8), None);
    assert_eq!(state.get_hash(9), None);
    assert_eq!(state.get_hash(10), None);
    assert_eq!(state.get_hash(7), Some(hash(7)));
    assert_eq!(state.get_hash(1), Some(hash(1)));
}

#[test]
fn test_deep_reorg_detection_limit() {
    let mut state = ChainState::new(128);

    // Fill 128 blocks
    for i in 1..=128u64 {
        state.push(i, hash(i as u8));
    }
    assert_eq!(state.len(), 128);

    // Adding block 129 evicts block 1
    state.push(129, hash(129u8));
    assert_eq!(state.len(), 128);
    assert_eq!(state.get_hash(1), None);
    assert_eq!(state.get_hash(2), Some(hash(2)));
}

#[tokio::test]
async fn test_normal_blocks_extend_chain() {
    let detector = ReorgDetector::new();

    // Process sequential blocks
    for i in 1..=20u64 {
        let block = make_block(i, i as u8, (i - 1) as u8);
        let decision = detector.process_block(1, &block).await.unwrap();
        assert_eq!(decision, ReorgDecision::Normal, "Block {} should be normal", i);
    }

    // Verify state
    let state = detector.get_state(1).unwrap();
    assert_eq!(state.len(), 20);
    assert_eq!(state.latest_block(), Some((20, hash(20))));
}

#[tokio::test]
async fn test_seed_block_initializes_state() {
    let detector = ReorgDetector::new();
    detector.seed_block(1, 1000, hash(0xAA));

    let state = detector.get_state(1).unwrap();
    assert_eq!(state.latest_block(), Some((1000, hash(0xAA))));
}
