//! Uniswap V3 Indexer — indexes pools, swaps, mints using the factory pattern.
//!
//! Demonstrates dynamic contract discovery: the factory emits PoolCreated events
//! and the indexer automatically starts watching each new pool for Swap/Mint/Burn.

mod handlers;
mod schema;

use forge_index::prelude::*;
use handlers::{handle_initialize, handle_mint, handle_pool_created, handle_swap};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let rpc_url = std::env::var("RPC_URL_1").unwrap_or_else(|_| {
        eprintln!("Warning: RPC_URL_1 not set, using default localhost");
        "http://localhost:8545".to_string()
    });

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("Warning: DATABASE_URL not set, using default");
        "postgres://postgres:postgres@localhost:5432/uniswap_v3".to_string()
    });

    let max_block_range: u64 = std::env::var("MAX_BLOCK_RANGE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let max_rps: u32 = std::env::var("MAX_RPC_REQUESTS_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);

    let factory_address = Address::from("0x1F98431c8aD98523631AE4a59f267346ea31F984");

    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = rpc_url;
            c.max_block_range = Some(max_block_range);
            c.max_rpc_requests_per_second = Some(max_rps);
        })
        // The factory contract — emits PoolCreated events
        .contract("UniswapV3Factory", |c| {
            c.abi_json = include_str!("../abis/UniswapV3Factory.json").to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(factory_address);
            c.start_block = 12_369_621;
        })
        // Pool contract — uses factory pattern for dynamic address discovery
        .contract("UniswapV3Pool", |c| {
            c.abi_json = include_str!("../abis/UniswapV3Pool.json").to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Factory(FactoryConfig {
                factory_address: vec![factory_address],
                event_signature: "PoolCreated(address,address,uint24,int24,address)".to_string(),
                address_parameter: "pool".to_string(),
                start_block: 12_369_621,
            });
            c.start_block = 12_369_621;
        })
        .schema(schema::build())
        .database(DatabaseConfig::postgres(database_url))
        .build()?;

    ForgeIndex::new()
        .config(config)
        .schema(schema::build())
        .on("UniswapV3Factory:PoolCreated", handle_pool_created)
        .on("UniswapV3Pool:Initialize", handle_initialize)
        .on("UniswapV3Pool:Swap", handle_swap)
        .on("UniswapV3Pool:Mint", handle_mint)
        .run()
        .await?;

    Ok(())
}
