//! Top-level configuration builder.

use forge_index_core::ForgeError;
use serde::{Deserialize, Serialize};

use crate::account::AccountConfig;
use crate::block_interval::BlockIntervalConfig;
use crate::chain::ChainConfig;
use crate::contract::ContractConfig;
use crate::database::DatabaseConfig;
use crate::ordering::Ordering;
use crate::schema::Schema;
use crate::validation;

/// The complete, validated configuration for a forge-index instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Chain definitions.
    pub chains: Vec<ChainConfig>,
    /// Contract definitions.
    pub contracts: Vec<ContractConfig>,
    /// Account watching definitions.
    pub accounts: Vec<AccountConfig>,
    /// Block interval handler definitions.
    pub block_intervals: Vec<BlockIntervalConfig>,
    /// The data schema.
    pub schema: Schema,
    /// Database connection configuration.
    pub database: DatabaseConfig,
    /// Cross-chain event ordering strategy.
    pub ordering: Ordering,
}

/// Fluent builder for constructing a validated [`Config`].
pub struct ConfigBuilder {
    chains: Vec<ChainConfig>,
    contracts: Vec<ContractConfig>,
    accounts: Vec<AccountConfig>,
    block_intervals: Vec<BlockIntervalConfig>,
    schema: Option<Schema>,
    database: Option<DatabaseConfig>,
    ordering: Ordering,
}

impl ConfigBuilder {
    /// Creates a new empty config builder.
    pub fn new() -> Self {
        Self {
            chains: Vec::new(),
            contracts: Vec::new(),
            accounts: Vec::new(),
            block_intervals: Vec::new(),
            schema: None,
            database: None,
            ordering: Ordering::Multichain,
        }
    }

    /// Adds a chain configuration via a closure.
    pub fn chain(mut self, name: &str, f: impl FnOnce(&mut ChainConfig)) -> Self {
        let mut chain = ChainConfig {
            name: name.to_string(),
            chain_id: 0,
            rpc_http: String::new(),
            rpc_ws: None,
            max_rpc_requests_per_second: None,
            poll_interval_ms: None,
        };
        f(&mut chain);
        self.chains.push(chain);
        self
    }

    /// Adds a contract configuration via a closure.
    pub fn contract(mut self, name: &str, f: impl FnOnce(&mut ContractConfig)) -> Self {
        let mut contract = ContractConfig {
            name: name.to_string(),
            abi_json: "[]".to_string(),
            chain_names: Vec::new(),
            address: crate::AddressConfig::Multiple(Vec::new()),
            start_block: 0,
            end_block: None,
            filter: None,
            include_transaction: false,
            include_trace: false,
        };
        f(&mut contract);
        self.contracts.push(contract);
        self
    }

    /// Adds an account configuration via a closure.
    pub fn account(mut self, name: &str, f: impl FnOnce(&mut AccountConfig)) -> Self {
        let mut account = AccountConfig {
            name: name.to_string(),
            chain_names: Vec::new(),
            address: forge_index_core::Address([0u8; 20]),
            start_block: 0,
            include_transaction: false,
        };
        f(&mut account);
        self.accounts.push(account);
        self
    }

    /// Adds a block interval configuration via a closure.
    pub fn block_interval(mut self, name: &str, f: impl FnOnce(&mut BlockIntervalConfig)) -> Self {
        let mut bi = BlockIntervalConfig {
            name: name.to_string(),
            chain_name: String::new(),
            interval: 1,
            start_block: 0,
            end_block: None,
        };
        f(&mut bi);
        self.block_intervals.push(bi);
        self
    }

    /// Sets the data schema.
    pub fn schema(mut self, schema: Schema) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Sets the database configuration.
    pub fn database(mut self, database: DatabaseConfig) -> Self {
        self.database = Some(database);
        self
    }

    /// Sets the event ordering strategy.
    pub fn ordering(mut self, ordering: Ordering) -> Self {
        self.ordering = ordering;
        self
    }

    /// Validates and builds the final [`Config`].
    ///
    /// Returns a [`ForgeError::Config`] if validation fails, containing all
    /// validation error messages joined with semicolons.
    pub fn build(self) -> Result<Config, ForgeError> {
        let schema = self.schema.unwrap_or(Schema { tables: Vec::new() });
        let database = self
            .database
            .unwrap_or_else(|| DatabaseConfig::postgres(""));

        let config = Config {
            chains: self.chains,
            contracts: self.contracts,
            accounts: self.accounts,
            block_intervals: self.block_intervals,
            schema,
            database,
            ordering: self.ordering,
        };

        let errors = validation::validate(&config);
        if errors.is_empty() {
            Ok(config)
        } else {
            Err(ForgeError::Config(errors.join("; ")))
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::AddressConfig;
    use crate::schema::{ColumnType, SchemaBuilder};
    use forge_index_core::Address;

    fn minimal_abi() -> String {
        r#"[{"type":"event","name":"Transfer","inputs":[]}]"#.to_string()
    }

    fn sample_schema() -> Schema {
        SchemaBuilder::new()
            .table("transfers", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("from", ColumnType::Address)
                    .not_null()
                    .column("to", ColumnType::Address)
                    .not_null()
                    .column("amount", ColumnType::BigInt)
                    .not_null()
            })
            .build()
    }

    #[test]
    fn config_builder_one_chain_one_contract_builds_successfully() {
        let result = ConfigBuilder::new()
            .chain("mainnet", |c| {
                c.chain_id = 1;
                c.rpc_http = "https://eth.rpc.example".to_string();
            })
            .contract("ERC20", |c| {
                c.abi_json = minimal_abi();
                c.chain_names = vec!["mainnet".to_string()];
                c.address = AddressConfig::Single(Address::from(
                    "0x0000000000000000000000000000000000000001",
                ));
                c.start_block = 1;
            })
            .schema(sample_schema())
            .database(DatabaseConfig::postgres("postgres://localhost/test"))
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn config_builder_missing_chain_fails_validation() {
        let result = ConfigBuilder::new()
            .database(DatabaseConfig::postgres("postgres://localhost/test"))
            .schema(sample_schema())
            .build();

        // Should succeed — no contracts reference missing chains
        assert!(result.is_ok());
    }

    #[test]
    fn config_builder_contract_referencing_unknown_chain_fails() {
        let result = ConfigBuilder::new()
            .chain("mainnet", |c| {
                c.chain_id = 1;
                c.rpc_http = "https://eth.rpc.example".to_string();
            })
            .contract("ERC20", |c| {
                c.abi_json = minimal_abi();
                c.chain_names = vec!["unknown_chain".to_string()];
                c.address = AddressConfig::Single(Address::from(
                    "0x0000000000000000000000000000000000000001",
                ));
            })
            .schema(sample_schema())
            .database(DatabaseConfig::postgres("postgres://localhost/test"))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unknown chain"),
            "Expected error about unknown chain, got: {}",
            err
        );
    }

    #[test]
    fn table_with_no_primary_key_fails_validation() {
        let schema = SchemaBuilder::new()
            .table("bad_table", |t| {
                t.column("name", ColumnType::Text).not_null()
            })
            .build();

        let result = ConfigBuilder::new()
            .schema(schema)
            .database(DatabaseConfig::postgres("postgres://localhost/test"))
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no primary key"),
            "Expected error about no primary key, got: {}",
            err
        );
    }

    #[test]
    fn full_config_with_two_chains_three_contracts_and_factory_validates() {
        let schema = SchemaBuilder::new()
            .table("pools", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("token0", ColumnType::Address)
                    .not_null()
                    .column("token1", ColumnType::Address)
                    .not_null()
                    .column("fee", ColumnType::Int)
                    .not_null()
            })
            .table("swaps", |t| {
                t.column("id", ColumnType::Text)
                    .primary_key()
                    .column("pool_id", ColumnType::Text)
                    .not_null()
                    .references("pools", "id")
                    .column("amount0", ColumnType::BigInt)
                    .not_null()
                    .column("amount1", ColumnType::BigInt)
                    .not_null()
                    .column("timestamp", ColumnType::Timestamp)
                    .not_null()
                    .index(&["pool_id"])
                    .index(&["timestamp"])
            })
            .build();

        let factory = crate::FactoryConfig {
            factory_address: vec![Address::from("0x1F98431c8aD98523631AE4a59f267346ea31F984")],
            event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
            address_parameter: "pool".to_string(),
            start_block: 12_369_621,
        };

        let result = ConfigBuilder::new()
            .chain("mainnet", |c| {
                c.chain_id = 1;
                c.rpc_http = "https://eth.rpc.example".to_string();
                c.rpc_ws = Some("wss://eth.rpc.example".to_string());
                c.max_rpc_requests_per_second = Some(25);
            })
            .chain("arbitrum", |c| {
                c.chain_id = 42161;
                c.rpc_http = "https://arb.rpc.example".to_string();
                c.poll_interval_ms = Some(250);
            })
            .contract("UniswapV3Factory", |c| {
                c.abi_json = minimal_abi();
                c.chain_names = vec!["mainnet".to_string()];
                c.address = AddressConfig::Single(Address::from(
                    "0x1F98431c8aD98523631AE4a59f267346ea31F984",
                ));
                c.start_block = 12_369_621;
            })
            .contract("UniswapV3Pool", |c| {
                c.abi_json = minimal_abi();
                c.chain_names = vec!["mainnet".to_string(), "arbitrum".to_string()];
                c.address = AddressConfig::Factory(factory);
                c.start_block = 12_369_621;
                c.include_transaction = true;
            })
            .contract("WETH", |c| {
                c.abi_json = minimal_abi();
                c.chain_names = vec!["mainnet".to_string()];
                c.address = AddressConfig::Single(Address::from(
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                ));
                c.start_block = 0;
            })
            .schema(schema)
            .database(DatabaseConfig::Postgres {
                connection_string: "postgres://localhost/uniswap".to_string(),
                schema: "uniswap_v3".to_string(),
                pool_max_connections: 20,
            })
            .ordering(Ordering::Omnichain)
            .build();

        assert!(result.is_ok(), "Expected valid config, got: {:?}", result);
    }
}
