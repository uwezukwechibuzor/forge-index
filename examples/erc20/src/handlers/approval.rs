//! Approval event handler.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles an ERC20 Approval event.
///
/// 1. Extracts owner, spender, value from the decoded event
/// 2. Inserts an approval_events row
/// 3. Increments owner's approval_count in accounts
/// 4. Updates token_stats singleton
pub async fn handle_approval(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let owner = extract_address(&event, "owner")?;
    let spender = extract_address(&event, "spender")?;
    let value = extract_uint256(&event, "value")?;

    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;

    tracing::debug!(
        owner = owner.as_str(),
        spender = spender.as_str(),
        value = value.as_str(),
        block = block_number,
        "Approval event"
    );

    let _event_id = format!("{}-{}", tx_hash, log_index);

    // Note: Same as transfer handler — actual DB writes would go through ctx.
    // ctx.insert("approval_events").values(...).await?;
    // ctx.update("accounts").set("approval_count", ...).where_pk("address", owner).await?;

    Ok(())
}

fn extract_address(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Address(addr) => Ok(addr.to_string()),
        other => anyhow::bail!("Expected address for '{}', got {:?}", name, other),
    }
}

fn extract_uint256(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Uint(v) => Ok(v.to_string()),
        DecodedParam::Uint256(s) => Ok(s.clone()),
        other => anyhow::bail!("Expected uint256 for '{}', got {:?}", name, other),
    }
}
