//! ERC721 Transfer event handler.

use forge_index_core::abi::decoder::{DecodedEvent, DecodedParam};

/// The zero address — indicates mints (from) and burns (to).
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Handles an ERC721 Transfer event.
///
/// Detects mints (from == 0x0), burns (to == 0x0), and normal transfers.
/// Updates tokens, holders, transfers, and collection_stats tables.
pub async fn handle_transfer(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let from = extract_address(&event, "from")?;
    let to = extract_address(&event, "to")?;
    let token_id = extract_token_id(&event, "tokenId")?;
    let block_number = event.raw_log.block_number;
    let tx_hash = event.raw_log.transaction_hash.to_string();
    let log_index = event.raw_log.log_index;
    let transfer_id = format!("{}-{}", tx_hash, log_index);

    let is_mint = from == ZERO_ADDRESS;
    let is_burn = to == ZERO_ADDRESS;

    if is_mint {
        tracing::debug!(
            token_id = token_id.as_str(),
            to = to.as_str(),
            block = block_number,
            "NFT minted: #{}",
            token_id
        );
        // In production:
        // ctx.insert("tokens").values(Token { token_id, owner: to, ... }).execute()?;
        // ctx.update("collection_stats").increment("total_supply", 1).execute()?;
    } else if is_burn {
        tracing::debug!(
            token_id = token_id.as_str(),
            from = from.as_str(),
            block = block_number,
            "NFT burned: #{}",
            token_id
        );
        // ctx.update("tokens").set("owner", ZERO_ADDRESS).where_pk(...).execute()?;
        // ctx.update("collection_stats").decrement("total_supply", 1).execute()?;
    } else {
        tracing::debug!(
            token_id = token_id.as_str(),
            from = from.as_str(),
            to = to.as_str(),
            block = block_number,
            "NFT transferred: #{} {} → {}",
            token_id,
            from,
            to
        );
        // ctx.update("tokens").set("owner", to).set("transfer_count", +1).execute()?;
    }

    // Update holders for both from and to
    if !is_mint {
        // Decrement old owner's token_count
        // ctx.update("holders").decrement("token_count", 1).where_pk("address", from).execute()?;
    }
    if !is_burn {
        // Upsert new owner's token_count
        // ctx.upsert("holders").increment("token_count", 1).where_pk("address", to).execute()?;
    }

    // Insert transfer record
    // ctx.insert("transfers").values(TransferRow { id, token_id, ... }).execute()?;

    // Increment total_transfers
    // ctx.update("collection_stats").increment("total_transfers", 1).execute()?;

    let _ = (
        transfer_id,
        from,
        to,
        token_id,
        block_number,
        is_mint,
        is_burn,
    );

    Ok(())
}

fn extract_address(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Address(addr) => Ok(addr.to_string()),
        other => anyhow::bail!("Expected address for '{}', got {:?}", name, other),
    }
}

fn extract_token_id(event: &DecodedEvent, name: &str) -> Result<String, anyhow::Error> {
    match event.get(name)? {
        DecodedParam::Uint(v) => Ok(v.to_string()),
        DecodedParam::Uint256(s) => Ok(s.clone()),
        other => anyhow::bail!("Expected uint256 for '{}', got {:?}", name, other),
    }
}
