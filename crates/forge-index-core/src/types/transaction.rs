//! EVM transaction type.

use super::{Address, Hash32};
use serde::{Deserialize, Serialize};

/// Represents an EVM transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// The chain ID this transaction belongs to.
    pub chain_id: u64,
    /// The transaction hash.
    pub hash: Hash32,
    /// The block number containing this transaction.
    pub block_number: u64,
    /// The hash of the block containing this transaction.
    pub block_hash: Hash32,
    /// The sender address.
    pub from: Address,
    /// The recipient address (`None` for contract creation).
    pub to: Option<Address>,
    /// The value transferred in wei.
    pub value: u128,
    /// The gas limit.
    pub gas: u64,
    /// The gas price (legacy transactions).
    pub gas_price: Option<u128>,
    /// The transaction input data.
    pub input: Vec<u8>,
    /// The sender's nonce.
    pub nonce: u64,
    /// The index of this transaction within the block.
    pub transaction_index: u32,
}

impl Transaction {
    /// Creates a `Transaction` from an alloy RPC transaction type.
    ///
    /// The `chain_id` must be supplied externally.
    pub fn from_alloy(tx: &alloy::rpc::types::Transaction, chain_id: u64) -> Self {
        use alloy::consensus::Transaction as _;

        Self {
            chain_id,
            hash: Hash32::from(*tx.inner.tx_hash()),
            block_number: tx.block_number.unwrap_or(0),
            block_hash: tx.block_hash.map(Hash32::from).unwrap_or(Hash32([0u8; 32])),
            from: Address::from(tx.from),
            to: tx.inner.to().map(Address::from),
            value: tx.inner.value().to::<u128>(),
            gas: tx.inner.gas_limit(),
            gas_price: tx.inner.gas_price(),
            input: tx.inner.input().to_vec(),
            nonce: tx.inner.nonce(),
            transaction_index: tx.transaction_index.unwrap_or(0) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::TxLegacy;
    use alloy::primitives::{Address as AlloyAddress, TxKind, B256, U256};

    #[test]
    fn transaction_from_alloy_preserves_all_fields() {
        let from_addr = AlloyAddress::from([0xBB; 20]);
        let to_addr = AlloyAddress::from([0xCC; 20]);

        let inner = TxLegacy {
            nonce: 7,
            gas_price: 20_000_000_000,
            gas_limit: 21_000,
            to: TxKind::Call(to_addr),
            value: U256::from(1_000_000_000_000_000_000u128),
            input: alloy::primitives::Bytes::from(vec![0xDE, 0xAD]),
            chain_id: Some(1),
        };

        let tx_hash = B256::from([0x44; 32]);
        let signed = alloy::consensus::TxEnvelope::Legacy(alloy::consensus::Signed::new_unchecked(
            inner,
            alloy::primitives::PrimitiveSignature::new(U256::from(1u64), U256::from(2u64), false),
            tx_hash,
        ));

        let rpc_tx = alloy::rpc::types::Transaction {
            inner: signed,
            from: from_addr,
            block_hash: Some(B256::from([0x55; 32])),
            block_number: Some(100),
            transaction_index: Some(3),
            effective_gas_price: None,
        };

        let tx = Transaction::from_alloy(&rpc_tx, 1);

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.hash, Hash32::from(tx_hash));
        assert_eq!(tx.block_number, 100);
        assert_eq!(tx.block_hash, Hash32::from(B256::from([0x55; 32])));
        assert_eq!(tx.from, Address::from(from_addr));
        assert_eq!(tx.to, Some(Address::from(to_addr)));
        assert_eq!(tx.value, 1_000_000_000_000_000_000u128);
        assert_eq!(tx.gas, 21_000);
        assert_eq!(tx.gas_price, Some(20_000_000_000));
        assert_eq!(tx.input, vec![0xDE, 0xAD]);
        assert_eq!(tx.nonce, 7);
        assert_eq!(tx.transaction_index, 3);
    }
}
