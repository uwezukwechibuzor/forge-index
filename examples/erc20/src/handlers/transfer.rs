//! Transfer event handler.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// Handles an ERC20 Transfer event.
///
/// 1. Extracts from, to, value from the decoded event
/// 2. Upserts sender account (debit balance)
/// 3. Upserts receiver account (credit balance)
/// 4. Inserts a transfer_events row
/// 5. Updates token_stats singleton
pub async fn handle_transfer(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let from = extract_address(&event, "from")?;
    let to = extract_address(&event, "to")?;
    let value = extract_uint256(&event, "value")?;

    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;

    tracing::debug!(
        from = from.as_str(),
        to = to.as_str(),
        value = value.as_str(),
        block = block_number,
        "Transfer event"
    );

    // The event ID is "{tx_hash}-{log_index}"
    let _event_id = format!("{}-{}", tx_hash, log_index);

    // Note: In a real implementation, we would use the ctx (DbContext) to
    // actually write to the database. Since the handler currently receives
    // serde_json::Value as context (placeholder), we just log the event.
    // When the full DbContext integration is complete, this handler would:
    //
    // ctx.insert("accounts").values(...).on_conflict_do_update(...).await?;
    // ctx.insert("transfer_events").values(...).await?;

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
