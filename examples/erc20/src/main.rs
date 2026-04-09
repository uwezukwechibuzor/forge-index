//! ERC20 Indexer — indexes USDC Transfer and Approval events on Ethereum mainnet.
//!
//! # Usage
//! ```sh
//! export RPC_URL_1="https://eth-mainnet.alchemyapi.io/v2/YOUR_KEY"
//! export DATABASE_URL="postgres://user@localhost:5432/erc20_indexer"
//! export MAX_BLOCK_RANGE=10  # for Alchemy free tier
//! cargo run
//! ```

mod handlers;
mod schema;

use forge_index::prelude::*;
use handlers::{handle_approval, handle_transfer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let rpc_url = std::env::var("RPC_URL_1").unwrap_or_else(|_| {
        eprintln!("Warning: RPC_URL_1 not set, using default localhost");
        "http://localhost:8545".to_string()
    });

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("Warning: DATABASE_URL not set, using default");
        "postgres://postgres:postgres@localhost:5432/erc20_indexer".to_string()
    });

    // Alchemy free tier limits eth_getLogs to 10-block ranges.
    // Set MAX_BLOCK_RANGE=10 in .env for free tier, or 2000+ for paid.
    let max_block_range: u64 = std::env::var("MAX_BLOCK_RANGE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    // Alchemy free tier: ~330 compute units/s ≈ 5 requests/s
    let max_rps: u32 = std::env::var("MAX_RPC_REQUESTS_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);

    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = rpc_url;
            c.max_block_range = Some(max_block_range);
            c.max_rpc_requests_per_second = Some(max_rps);
        })
        .contract("ERC20", |c| {
            c.abi_json = include_str!("../abis/ERC20.json").to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(Address::from(
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            ));
            c.start_block = 6_082_465;
        })
        .schema(schema::build())
        .database(DatabaseConfig::postgres(database_url))
        .build()?;

    ForgeIndex::new()
        .config(config)
        .schema(schema::build())
        .on("ERC20:Transfer", handle_transfer)
        .on("ERC20:Approval", handle_approval)
        .run()
        .await?;

    Ok(())
}
