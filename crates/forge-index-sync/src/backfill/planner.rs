//! Backfill planner — computes work chunks for historical event fetching.

use forge_index_config::ContractConfig;

/// Default chunk size in blocks.
pub const DEFAULT_CHUNK_SIZE: u64 = 2000;

/// A contiguous range of blocks to process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRange {
    /// Start block (inclusive).
    pub from: u64,
    /// End block (inclusive).
    pub to: u64,
}

impl BlockRange {
    /// Returns the number of blocks in this range.
    pub fn len(&self) -> u64 {
        if self.to >= self.from {
            self.to - self.from + 1
        } else {
            0
        }
    }

    /// Returns true if this range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The result of planning a backfill for one contract on one chain.
#[derive(Debug, Clone)]
pub struct BackfillPlan {
    /// The contract name.
    pub contract_name: String,
    /// The chain ID.
    pub chain_id: u64,
    /// The ordered list of block ranges to process.
    pub ranges: Vec<BlockRange>,
    /// Total number of blocks across all ranges.
    pub total_blocks: u64,
}

/// Computes a backfill plan for a contract.
///
/// Splits the range `[start, current_block]` into chunks of `chunk_size` blocks.
/// If a checkpoint exists, starts from the checkpoint instead of `start_block`.
pub fn plan(
    config: &ContractConfig,
    chain_id: u64,
    current_block: u64,
    checkpoint: Option<u64>,
    chunk_size: u64,
) -> BackfillPlan {
    let start = checkpoint.unwrap_or(config.start_block);

    if start > current_block {
        return BackfillPlan {
            contract_name: config.name.clone(),
            chain_id,
            ranges: vec![],
            total_blocks: 0,
        };
    }

    let mut ranges = Vec::new();
    let mut from = start;

    while from <= current_block {
        let to = (from + chunk_size - 1).min(current_block);
        ranges.push(BlockRange { from, to });
        from = to + 1;
    }

    let total_blocks = current_block - start + 1;

    BackfillPlan {
        contract_name: config.name.clone(),
        chain_id,
        ranges,
        total_blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_index_config::{AddressConfig, ContractConfig};
    use forge_index_core::Address;

    fn test_contract() -> ContractConfig {
        ContractConfig {
            name: "TestToken".to_string(),
            abi_json: "[]".to_string(),
            chain_names: vec!["mainnet".to_string()],
            address: AddressConfig::Single(Address::from(
                "0x0000000000000000000000000000000000000001",
            )),
            start_block: 0,
            end_block: None,
            filter: None,
            include_transaction: false,
            include_trace: false,
        }
    }

    #[test]
    fn planner_produces_correct_chunks() {
        let config = test_contract();
        let plan = plan(&config, 1, 10000, None, 2000);

        assert_eq!(plan.contract_name, "TestToken");
        assert_eq!(plan.total_blocks, 10001);
        assert_eq!(plan.ranges.len(), 6); // 0-1999, 2000-3999, 4000-5999, 6000-7999, 8000-9999, 10000
        assert_eq!(plan.ranges[0], BlockRange { from: 0, to: 1999 });
        assert_eq!(
            plan.ranges[1],
            BlockRange {
                from: 2000,
                to: 3999
            }
        );
        assert_eq!(
            plan.ranges[5],
            BlockRange {
                from: 10000,
                to: 10000
            }
        );
    }

    #[test]
    fn planner_resumes_from_checkpoint() {
        let config = test_contract();
        let plan = plan(&config, 1, 10000, Some(5000), 2000);

        assert_eq!(plan.total_blocks, 5001);
        assert_eq!(plan.ranges[0].from, 5000);
    }

    #[test]
    fn planner_with_end_less_than_start_returns_empty() {
        let mut config = test_contract();
        config.start_block = 5000;
        let plan = plan(&config, 1, 3000, None, 2000);

        assert!(plan.ranges.is_empty());
        assert_eq!(plan.total_blocks, 0);
    }
}
