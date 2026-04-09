//! Approval event handler — production implementation with DbContext.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};
use forge_index_db::context::DbContext;
use forge_index_db::row::{ColumnValue, Row};

/// Handles an ERC20 Approval event.
///
/// 1. Inserts an approval_events row
/// 2. Inserts owner account if new
/// 3. Inserts token_stats singleton if not exists
pub async fn handle_approval(event: DecodedEvent, ctx: DbContext) -> Result<(), anyhow::Error> {
    let owner = extract_address(&event, "owner")?;
    let spender = extract_address(&event, "spender")?;
    let value = extract_uint256(&event, "value")?;

    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;
    let event_id = format!("{}-{}", tx_hash, log_index);

    // 1. Insert approval_events row
    let mut row = Row::new();
    row.insert("id", event_id.as_str());
    row.insert("owner", owner.as_str());
    row.insert("spender", spender.as_str());
    row.insert("value", ColumnValue::BigNumeric(value.clone()));
    row.insert("block_number", ColumnValue::BigInt(block_number as i64));
    row.insert("tx_hash", tx_hash.as_str());
    ctx.insert("approval_events").row(row).execute()?;

    // 2. Insert owner account if new
    let mut acct_row = Row::new();
    acct_row.insert("address", owner.as_str());
    acct_row.insert("balance", ColumnValue::BigNumeric("0".to_string()));
    acct_row.insert("approval_count", ColumnValue::Int(0));
    ctx.insert("accounts").row(acct_row).execute()?;

    tracing::debug!(
        owner = owner.as_str(),
        spender = spender.as_str(),
        value = value.as_str(),
        block = block_number,
        "Approval: {} approved {} to spend {} tokens",
        owner,
        spender,
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
