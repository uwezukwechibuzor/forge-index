//! EVM block type.

use super::{Address, Hash32};
use serde::{Deserialize, Serialize};

/// Represents a finalized EVM block with header fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// The chain ID this block belongs to.
    pub chain_id: u64,
    /// The block number (height).
    pub number: u64,
    /// The block hash.
    pub hash: Hash32,
    /// The parent block's hash.
    pub parent_hash: Hash32,
    /// The block's Unix timestamp.
    pub timestamp: u64,
    /// The gas limit for this block.
    pub gas_limit: u64,
    /// The gas used in this block.
    pub gas_used: u64,
    /// The base fee per gas (EIP-1559), if present.
    pub base_fee_per_gas: Option<u128>,
    /// The address of the block's miner/proposer.
    pub miner: Address,
}

impl Block {
    /// Creates a `Block` from an alloy RPC block type.
    ///
    /// The `chain_id` must be supplied externally since the RPC block type
    /// does not carry it.
    pub fn from_alloy(block: &alloy::rpc::types::Block, chain_id: u64) -> Self {
        use alloy::consensus::BlockHeader;

        Self {
            chain_id,
            number: block.header.number(),
            hash: Hash32::from(block.header.hash),
            parent_hash: Hash32::from(block.header.parent_hash()),
            timestamp: block.header.timestamp(),
            gas_limit: block.header.gas_limit(),
            gas_used: block.header.gas_used(),
            base_fee_per_gas: block.header.base_fee_per_gas().map(|v| v as u128),
            miner: Address::from(block.header.beneficiary()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::Header;
    use alloy::primitives::{Address as AlloyAddress, Sealed, B256};

    fn make_alloy_block() -> alloy::rpc::types::Block {
        let block_hash = B256::from([0x11; 32]);
        let header = Header {
            number: 42,
            parent_hash: B256::from([0x22; 32]),
            timestamp: 1_700_000_000,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: Some(1_000_000_000),
            beneficiary: AlloyAddress::from([0xAA; 20]),
            ..Default::default()
        };
        let sealed = Sealed::new_unchecked(header, block_hash);
        alloy::rpc::types::Block {
            header: alloy::rpc::types::Header::from_consensus(sealed, None, None),
            ..Default::default()
        }
    }

    #[test]
    fn block_from_alloy_preserves_all_fields() {
        let alloy_block = make_alloy_block();
        let block = Block::from_alloy(&alloy_block, 1);

        assert_eq!(block.chain_id, 1);
        assert_eq!(block.number, 42);
        assert_eq!(block.hash, Hash32::from(B256::from([0x11; 32])));
        assert_eq!(block.parent_hash, Hash32::from(B256::from([0x22; 32])));
        assert_eq!(block.timestamp, 1_700_000_000);
        assert_eq!(block.gas_limit, 30_000_000);
        assert_eq!(block.gas_used, 15_000_000);
        assert_eq!(block.base_fee_per_gas, Some(1_000_000_000));
        assert_eq!(block.miner, Address::from(AlloyAddress::from([0xAA; 20])));
    }
}
