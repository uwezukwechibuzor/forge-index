//! Swap event handler — records swaps and updates pool state.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles a UniswapV3Pool Swap event.
///
/// Inserts a swap record, updates the pool's price/tick/liquidity,
/// and accumulates volume in pool_stats.
pub async fn handle_swap(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let pool_address = event.raw_log.address.to_string();
    let sender = extract_address(&event, "sender")?;
    let recipient = extract_address(&event, "recipient")?;
    let amount0 = extract_int_string(&event, "amount0")?;
    let amount1 = extract_int_string(&event, "amount1")?;
    let sqrt_price = extract_uint_string(&event, "sqrtPriceX96")?;
    let liquidity = extract_uint_string(&event, "liquidity")?;
    let tick = extract_int(&event, "tick")?;

    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;
    let swap_id = format!("{}-{}", tx_hash, log_index);

    tracing::debug!(
        pool = pool_address.as_str(),
        amount0 = amount0.as_str(),
        amount1 = amount1.as_str(),
        tick = tick,
        "Swap in pool {}",
        pool_address
    );

    // In production:
    // ctx.insert("swaps").values(SwapRow { id: swap_id, pool, sender, ... }).execute()?;
    // ctx.update("pools").set("sqrt_price", sqrt_price).set("current_tick", tick)
    //   .set("liquidity", liquidity).where_pk("address", pool_address).execute()?;
    // ctx.upsert("pool_stats").increment("total_swaps", 1)
    //   .add("total_volume_token0", abs(amount0)).execute()?;
    let _ = (
        swap_id,
        pool_address,
        sender,
        recipient,
        amount0,
        amount1,
        sqrt_price,
        liquidity,
        tick,
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

fn extract_int_string(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Int(v) => Ok(v.to_string()),
        DecodedParam::Int256(s) => Ok(s.clone()),
        DecodedParam::Uint(v) => Ok(v.to_string()),
        DecodedParam::Uint256(s) => Ok(s.clone()),
        other => anyhow::bail!("Expected int/uint for '{}', got {:?}", name, other),
    }
}

fn extract_int(event: &DecodedEvent, name: &str) -> Result<i128, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Int(v) => Ok(*v),
        DecodedParam::Uint(v) => Ok(*v as i128),
        other => anyhow::bail!("Expected int for '{}', got {:?}", name, other),
    }
}
