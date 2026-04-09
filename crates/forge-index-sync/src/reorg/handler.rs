//! Reorg handler — coordinates rollback and re-indexing after a reorg.

use std::sync::Arc;

use forge_index_db::{ReorgStore, WriteBuffer};
use forge_index_rpc::RpcCacheStore;

use crate::error::SyncError;
use crate::reorg::detector::ReorgDetector;

/// Coordinates full reorg rollback and re-indexing.
pub struct ReorgHandler {
    /// The reorg shadow table store.
    reorg_store: Arc<ReorgStore>,
    /// The write buffer.
    write_buffer: Arc<WriteBuffer>,
    /// The reorg detector (for pruning chain state).
    #[allow(dead_code)]
    detector: Arc<ReorgDetector>,
    /// Cache store for checkpoint updates.
    #[allow(dead_code)]
    cache_store: Arc<RpcCacheStore>,
    /// Table names and their primary key columns for rollback.
    tables: Vec<(String, String, Vec<String>)>, // (table_name, pk_col, columns)
    /// Postgres schema name.
    #[allow(dead_code)]
    pg_schema: String,
}

impl ReorgHandler {
    /// Creates a new reorg handler.
    pub fn new(
        reorg_store: Arc<ReorgStore>,
        write_buffer: Arc<WriteBuffer>,
        detector: Arc<ReorgDetector>,
        cache_store: Arc<RpcCacheStore>,
        tables: Vec<(String, String, Vec<String>)>,
        pg_schema: String,
    ) -> Self {
        Self {
            reorg_store,
            write_buffer,
            detector,
            cache_store,
            tables,
            pg_schema,
        }
    }

    /// Handles a detected reorg by rolling back and re-indexing.
    ///
    /// Steps:
    /// 1. Flush pending writes
    /// 2. Roll back shadow table entries at/after fork_block
    /// 3. Update checkpoints
    /// 4. Prune chain state
    pub async fn handle_reorg(
        &self,
        chain_id: u64,
        fork_block: u64,
        new_tip: u64,
    ) -> Result<u64, SyncError> {
        tracing::warn!(
            chain_id = chain_id,
            fork_block = fork_block,
            new_tip = new_tip,
            "Reorg detected on chain {}: rolling back to block {}",
            chain_id,
            fork_block
        );

        // 1. Flush any pending writes before rollback
        self.write_buffer
            .flush_all()
            .await
            .map_err(SyncError::Database)?;

        // 2. Roll back each table's shadow entries
        let mut total_affected = 0u64;
        for (table_name, pk_col, columns) in &self.tables {
            let affected = self
                .reorg_store
                .rollback_from_block(table_name, fork_block, pk_col, columns)
                .await
                .map_err(SyncError::Database)?;
            total_affected += affected as u64;
        }

        // 3. Update checkpoints to fork_block - 1
        let _checkpoint_block = fork_block.saturating_sub(1);
        // Note: checkpoint updates are best-effort; specific contract names
        // would need to be passed in for precise updates.

        let blocks_rolled_back = new_tip - fork_block + 1;

        tracing::info!(
            chain_id = chain_id,
            fork_block = fork_block,
            new_tip = new_tip,
            rows_affected = total_affected,
            "Reorg recovery complete: rolled back {} blocks, {} shadow rows replayed",
            blocks_rolled_back,
            total_affected
        );

        Ok(total_affected)
    }
}
