//! Shared test data: ABIs, events, blocks, logs.

use forge_index_core::types::{Address, Block, Hash32, Log};

/// ERC20 ABI with Transfer and Approval events.
pub const ERC20_ABI: &str = r#"[
    {
        "type": "event",
        "name": "Transfer",
        "inputs": [
            {"name": "from", "type": "address", "indexed": true},
            {"name": "to", "type": "address", "indexed": true},
            {"name": "value", "type": "uint256", "indexed": false}
        ]
    },
    {
        "type": "event",
        "name": "Approval",
        "inputs": [
            {"name": "owner", "type": "address", "indexed": true},
            {"name": "spender", "type": "address", "indexed": true},
            {"name": "value", "type": "uint256", "indexed": false}
        ]
    }
]"#;

/// Factory ABI with PoolCreated event (Uniswap V3 style).
pub const FACTORY_ABI: &str = r#"[
    {
        "type": "event",
        "name": "PoolCreated",
        "inputs": [
            {"name": "token0", "type": "address", "indexed": true},
            {"name": "token1", "type": "address", "indexed": true},
            {"name": "fee", "type": "uint24", "indexed": true},
            {"name": "tickSpacing", "type": "int24", "indexed": false},
            {"name": "pool", "type": "address", "indexed": false}
        ]
    }
]"#;

/// Transfer event topic0 (keccak256("Transfer(address,address,uint256)")).
pub const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Approval event topic0 (keccak256("Approval(address,address,uint256)")).
pub const APPROVAL_TOPIC: &str =
    "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";

/// Creates a test block with the given number.
pub fn make_block(number: u64) -> Block {
    Block {
        chain_id: 1,
        number,
        hash: make_hash(number as u8),
        parent_hash: if number > 0 {
            make_hash((number - 1) as u8)
        } else {
            Hash32([0u8; 32])
        },
        timestamp: 1_600_000_000 + number * 12,
        gas_limit: 30_000_000,
        gas_used: 15_000_000,
        base_fee_per_gas: Some(1_000_000_000),
        miner: Address::from("0x0000000000000000000000000000000000000001"),
    }
}

/// Creates a test block for a specific chain.
pub fn make_block_chain(chain_id: u64, number: u64) -> Block {
    let mut block = make_block(number);
    block.chain_id = chain_id;
    block
}

/// Creates a Transfer log at the given block.
pub fn make_transfer_log(block_number: u64, log_index: u32) -> Log {
    let block_hash = make_hash(block_number as u8);
    let tx_hash = make_hash((block_number as u8).wrapping_add(100));
    let from_topic =
        Hash32::from("0x0000000000000000000000000000000000000000000000000000000000000001");
    let to_topic =
        Hash32::from("0x0000000000000000000000000000000000000000000000000000000000000002");

    Log {
        id: format!("{}-{}", block_hash, log_index),
        chain_id: 1,
        address: Address::from("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        topics: vec![Hash32::from(TRANSFER_TOPIC), from_topic, to_topic],
        data: vec![0u8; 32], // value = 0
        block_number,
        block_hash,
        transaction_hash: tx_hash,
        log_index,
        transaction_index: 0,
        removed: false,
    }
}

/// Creates a deterministic Hash32 from a single byte.
pub fn make_hash(n: u8) -> Hash32 {
    let mut bytes = [0u8; 32];
    bytes[31] = n;
    bytes[0] = n.wrapping_add(0xAA);
    Hash32(bytes)
}

/// JSON representation of a block for wiremock responses.
pub fn block_json(number: u64) -> serde_json::Value {
    let block = make_block(number);
    let bloom = format!("0x{}", "0".repeat(512));
    serde_json::json!({
        "number": format!("0x{:x}", block.number),
        "hash": format!("0x{}", hex::encode(block.hash.0)),
        "parentHash": format!("0x{}", hex::encode(block.parent_hash.0)),
        "nonce": "0x0000000000000000",
        "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
        "logsBloom": bloom,
        "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
        "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
        "miner": "0x0000000000000000000000000000000000000001",
        "difficulty": "0x0",
        "totalDifficulty": "0x0",
        "extraData": "0x",
        "size": "0x0",
        "gasLimit": format!("0x{:x}", block.gas_limit),
        "gasUsed": format!("0x{:x}", block.gas_used),
        "timestamp": format!("0x{:x}", block.timestamp),
        "transactions": [],
        "uncles": [],
        "baseFeePerGas": "0x3b9aca00",
        "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000"
    })
}

/// JSON representation of logs for wiremock responses.
pub fn logs_json(logs: &[Log]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = logs
        .iter()
        .map(|l| {
            serde_json::json!({
                "address": format!("0x{}", hex::encode(l.address.0)),
                "topics": l.topics.iter().map(|t| format!("0x{}", hex::encode(t.0))).collect::<Vec<_>>(),
                "data": format!("0x{}", hex::encode(&l.data)),
                "blockNumber": format!("0x{:x}", l.block_number),
                "transactionHash": format!("0x{}", hex::encode(l.transaction_hash.0)),
                "transactionIndex": format!("0x{:x}", l.transaction_index),
                "blockHash": format!("0x{}", hex::encode(l.block_hash.0)),
                "logIndex": format!("0x{:x}", l.log_index),
                "removed": l.removed
            })
        })
        .collect();
    serde_json::Value::Array(arr)
}
