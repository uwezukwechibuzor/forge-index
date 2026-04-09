//! Mint event handler — records liquidity positions.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles a UniswapV3Pool Mint event.
///
/// Inserts a mint record with tick range and amounts.
pub async fn handle_mint(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let pool_address = event.raw_log.address.to_string();
    let owner = extract_address(&event, "owner")?;
    let tick_lower = extract_int(&event, "tickLower")?;
    let tick_upper = extract_int(&event, "tickUpper")?;
    let amount = extract_uint_string(&event, "amount")?;
    let amount0 = extract_uint_string(&event, "amount0")?;
    let amount1 = extract_uint_string(&event, "amount1")?;
    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;
    let mint_id = format!("{}-{}", tx_hash, log_index);

    tracing::debug!(
        pool = pool_address.as_str(),
        owner = owner.as_str(),
        tick_lower = tick_lower,
        tick_upper = tick_upper,
        "Mint in pool {}",
        pool_address
    );

    // In production:
    // ctx.insert("mints").values(MintRow { id: mint_id, pool, owner, ... }).execute()?;
    let _ = (
        mint_id,
        pool_address,
        owner,
        tick_lower,
        tick_upper,
        amount,
        amount0,
        amount1,
        block_number,
    );

    Ok(())
}

fn extract_address(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Address(addr) => Ok(addr.to_string()),
        other => anyhow::bail!("Expected address for '{}', got {:?}", name, other),
    }
}

fn extract_uint_string(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Uint(v) => Ok(v.to_string()),
        DecodedParam::Uint256(s) => Ok(s.clone()),
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
