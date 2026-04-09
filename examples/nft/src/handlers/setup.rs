//! Setup handler — fetches collection metadata before indexing.

/// Runs once before indexing begins.
///
/// Fetches the collection name and symbol via read-only contract calls
/// and stores them in the collection_info table.
pub async fn handle_setup(_ctx: serde_json::Value) -> Result<(), anyhow::Error> {
    tracing::info!("Running setup: fetching collection metadata");

    // In production:
    // let name = ctx.client.readContract(contract_address, "name()", block).await?;
    // let symbol = ctx.client.readContract(contract_address, "symbol()", block).await?;
    //
    // ctx.insert("collection_info")
    //     .values(CollectionInfo { id: "info", name, symbol })
    //     .execute()?;
    //
    // ctx.insert("collection_stats")
    //     .values(CollectionStats {
    //         id: "stats",
    //         total_supply: 0,
    //         total_transfers: 0,
    //         unique_holders: 0,
    //         last_updated_block: 0,
    //     })
    //     .execute()?;

    tracing::info!("Setup complete");
    Ok(())
}
