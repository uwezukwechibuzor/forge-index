//! PoolCreated event handler — registers newly deployed pools.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles a UniswapV3Factory PoolCreated event.
///
/// Extracts token0, token1, fee, tickSpacing, and pool address,
/// then inserts a new row into the pools table.
pub async fn handle_pool_created(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let token0 = extract_address(&event, "token0")?;
    let token1 = extract_address(&event, "token1")?;
    let fee = extract_uint(&event, "fee")?;
    let tick_spacing = extract_int(&event, "tickSpacing")?;
    let pool = extract_address(&event, "pool")?;
    let block_number = event.raw_log.block_number;

    tracing::info!(
        pool = pool.as_str(),
        token0 = token0.as_str(),
        token1 = token1.as_str(),
        fee = fee,
        "New Uniswap V3 pool: {} ({}/{} {}bps)",
        pool,
        token0,
        token1,
        fee / 100
    );

    // In production: ctx.insert("pools").values(...).execute()?;
    let _ = (token0, token1, fee, tick_spacing, pool, block_number);

    Ok(())
}

fn extract_address(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Address(addr) => Ok(addr.to_string()),
        other => anyhow::bail!("Expected address for '{}', got {:?}", name, other),
    }
}

fn extract_uint(event: &DecodedEvent, name: &str) -> Result<u128, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Uint(v) => Ok(*v),
        DecodedParam::Uint256(s) => s.parse().map_err(|e| anyhow::anyhow!("{}", e)),
        other => anyhow::bail!("Expected uint for '{}', got {:?}", name, other),
    }
}

fn extract_int(event: &DecodedEvent, name: &str) -> Result<i128, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Int(v) => Ok(*v),
        DecodedParam::Uint(v) => Ok(*v as i128),
        other => anyhow::bail!("Expected int for '{}', got {:?}", name, other),
    }
}
