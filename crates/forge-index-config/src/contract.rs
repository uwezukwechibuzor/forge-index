//! Contract and factory configuration types.

use forge_index_core::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a factory-discovered contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryConfig {
    /// The factory contract addresses to watch.
    pub factory_address: Vec<Address>,
    /// The event signature emitted when a new child contract is deployed.
    pub event_signature: String,
    /// The parameter name in the event that contains the child address.
    pub address_parameter: String,
    /// The block number to start indexing from.
    pub start_block: u64,
}

/// A filter applied to contract events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// The name of the event to filter.
    pub event_name: String,
    /// Argument-level filter conditions.
    pub args: HashMap<String, serde_json::Value>,
}

/// How contract addresses are specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AddressConfig {
    /// A single known address.
    Single(Address),
    /// Multiple known addresses.
    Multiple(Vec<Address>),
    /// Addresses discovered via a factory pattern.
    Factory(FactoryConfig),
}

/// How to determine the end block for indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndBlock {
    /// Stop at a specific block number.
    Number(u64),
    /// Continue indexing up to the latest block forever.
    Latest,
}

/// Configuration for a contract to index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    /// Human-readable contract name (e.g., "UniswapV3Pool").
    pub name: String,
    /// The ABI as a JSON string.
    pub abi_json: String,
    /// The chain names this contract is deployed on.
    pub chain_names: Vec<String>,
    /// How addresses are resolved for this contract.
    pub address: AddressConfig,
    /// The block number to start indexing from.
    pub start_block: u64,
    /// Optional end block.
    pub end_block: Option<EndBlock>,
    /// Optional event-level filters.
    pub filter: Option<Vec<FilterConfig>>,
    /// Whether to include full transaction data in events.
    pub include_transaction: bool,
    /// Whether to include trace data in events.
    pub include_trace: bool,
}
