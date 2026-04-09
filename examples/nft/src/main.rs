//! NFT Indexer — indexes ERC721 transfers with block-interval aggregated stats.
//!
//! Demonstrates:
//! - ERC721 Transfer event handling (mints, burns, transfers)
//! - Block interval handler that computes stats every 100 blocks
//! - Setup handler that fetches collection metadata before indexing
//!
//! Indexes Bored Ape Yacht Club (BAYC) on Ethereum mainnet.

mod handlers;
mod schema;

use forge_index::prelude::*;
use handlers::{handle_setup, handle_transfer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let rpc_url = std::env::var("RPC_URL_1").unwrap_or_else(|_| {
        eprintln!("Warning: RPC_URL_1 not set, using default localhost");
        "http://localhost:8545".to_string()
    });

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("Warning: DATABASE_URL not set, using default");
        "postgres://postgres:postgres@localhost:5432/nft_indexer".to_string()
    });

    let max_block_range: u64 = std::env::var("MAX_BLOCK_RANGE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let max_rps: u32 = std::env::var("MAX_RPC_REQUESTS_PER_SECOND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);

    let bayc_address = Address::from("0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D");

    let config = ConfigBuilder::new()
        .chain("mainnet", |c| {
            c.chain_id = 1;
            c.rpc_http = rpc_url;
            c.max_block_range = Some(max_block_range);
            c.max_rpc_requests_per_second = Some(max_rps);
        })
        .contract("ERC721", |c| {
            c.abi_json = include_str!("../abis/ERC721.json").to_string();
            c.chain_names = vec!["mainnet".to_string()];
            c.address = forge_index_config::AddressConfig::Single(bayc_address);
            c.start_block = 12_287_507; // BAYC deployment block
        })
        // Block interval: compute stats every 100 blocks
        .block_interval("StatsUpdate", |bi| {
            bi.chain_name = "mainnet".to_string();
            bi.interval = 100;
            bi.start_block = 12_287_507;
        })
        .schema(schema::build())
        .database(DatabaseConfig::postgres(database_url))
        .build()?;

    ForgeIndex::new()
        .config(config)
        .schema(schema::build())
        .on("ERC721:Transfer", handle_transfer)
        .setup("ERC721", handle_setup)
        .run()
        .await?;

    Ok(())
}
