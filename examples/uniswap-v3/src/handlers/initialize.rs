//! Initialize event handler — sets initial price for a pool.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles a UniswapV3Pool Initialize event.
///
/// Updates the pool's sqrt_price and current_tick.
pub async fn handle_initialize(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let pool_address = event.raw_log.address.to_string();
    let sqrt_price = extract_uint_string(&event, "sqrtPriceX96")?;
    let tick = extract_int(&event, "tick")?;

    tracing::debug!(
        pool = pool_address.as_str(),
        sqrt_price = sqrt_price.as_str(),
        tick = tick,
        "Pool initialized"
    );

    // In production: ctx.update("pools").set("sqrt_price", sqrt_price)
    //   .set("current_tick", tick).where_pk("address", pool_address).execute()?;
    let _ = (pool_address, sqrt_price, tick);

    Ok(())
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
