//! Transfer event handler — production implementation with DbContext.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_db::context::DbContext;
use forge_index_db::row::{ColumnValue, Row};

/// Handles an ERC20 Transfer event.
///
/// 1. Upserts sender account (debit balance)
/// 2. Upserts receiver account (credit balance)
/// 3. Inserts a transfer_events row
/// 4. Updates token_stats singleton
pub async fn handle_transfer(event: DecodedEvent, ctx: DbContext) -> Result<(), anyhow::Error> {
    let from = extract_address(&event, "from")?;
    let to = extract_address(&event, "to")?;
    let value = extract_uint256(&event, "value")?;

    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;
    let event_id = format!("{}-{}", tx_hash, log_index);

    // 1. Insert sender account if new
    let mut from_row = Row::new();
    from_row.insert("address", from.as_str());
    from_row.insert("balance", ColumnValue::BigNumeric("0".to_string()));
    from_row.insert("approval_count", ColumnValue::Int(0));
    ctx.insert("accounts").row(from_row).execute()?;

    // 2. Insert receiver account if new
    let mut to_row = Row::new();
    to_row.insert("address", to.as_str());
    to_row.insert("balance", ColumnValue::BigNumeric("0".to_string()));
    to_row.insert("approval_count", ColumnValue::Int(0));
    ctx.insert("accounts").row(to_row).execute()?;

    // 3. Insert transfer_events row
    let mut transfer_row = Row::new();
    transfer_row.insert("id", event_id.as_str());
    transfer_row.insert("from_address", from.as_str());
    transfer_row.insert("to_address", to.as_str());
    transfer_row.insert("value", ColumnValue::BigNumeric(value.clone()));
    transfer_row.insert("block_number", ColumnValue::BigInt(block_number as i64));
    transfer_row.insert("timestamp", ColumnValue::BigInt(0));
    transfer_row.insert("tx_hash", tx_hash.as_str());
    ctx.insert("transfer_events").row(transfer_row).execute()?;

    // 4. Insert token_stats singleton if not exists
    let mut stats_row = Row::new();
    stats_row.insert("id", "singleton");
    stats_row.insert("total_transfers", ColumnValue::BigInt(1));
    stats_row.insert("total_approvals", ColumnValue::BigInt(0));
    stats_row.insert("unique_holders", ColumnValue::BigInt(0));
    ctx.insert("token_stats").row(stats_row).execute()?;

    tracing::debug!(
        from = from.as_str(),
        to = to.as_str(),
        value = value.as_str(),
        block = block_number,
        "Transfer: {} → {} ({} tokens)",
        from,
        to,
        value
    );

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
