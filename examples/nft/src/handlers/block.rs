//! Block interval handler — computes aggregated stats every N blocks.

use forge_index_core::abi::decoder::DecodedEvent;

/// Handles a block interval event (runs every 100 blocks).
///
/// Computes unique holders count and updates collection_stats.
#[allow(dead_code)]
pub async fn handle_block(
    event: DecodedEvent,
    _ctx: serde_json::Value,
) -> Result<(), anyhow::Error> {
    let block_number = event.raw_log.block_number;

    tracing::debug!(
        block = block_number,
        "Block interval: updating collection stats at block {}",
        block_number
    );

    // In production:
    // let (unique_holders,): (i64,) = ctx.query_one(
    //     "SELECT COUNT(*) FROM holders WHERE token_count > 0"
    // ).await?;
    //
    // ctx.update("collection_stats")
    //     .set("unique_holders", unique_holders)
    //     .set("last_updated_block", block_number)
    //     .where_pk("id", "stats")
    //     .execute()?;
    //
    // tracing::debug!(
    //     block = block_number,
    //     unique_holders = unique_holders,
    //     "Stats updated: {} holders",
    //     unique_holders
    // );

    Ok(())
}
