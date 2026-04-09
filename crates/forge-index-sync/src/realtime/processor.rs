//! Realtime processor — main loop for processing live blocks after backfill.

use std::sync::Arc;

use forge_index_core::abi::decoder::DecodedEvent;
use forge_index_core::registry::EventRegistry;
use forge_index_db::{DbContext, WriteBuffer};
use forge_index_rpc::RpcCacheStore;
use futures::StreamExt;

use crate::backfill::planner::BlockRange;
use crate::backfill::progress::BackfillProgress;
use crate::backfill::worker::BackfillWorker;
use crate::error::SyncError;
use crate::realtime::finality::FinalityTracker;
use crate::realtime::subscriber::NewBlockSubscriber;
use crate::reorg::detector::ReorgDetector;
use crate::reorg::handler::ReorgHandler;

/// The main realtime processing loop after backfill completes.
pub struct RealtimeProcessor {
    /// The new block subscriber.
    subscriber: NewBlockSubscriber,
    /// Workers for fetching events per contract.
    workers: Vec<Arc<BackfillWorker>>,
    /// The reorg handler.
    reorg_handler: Arc<ReorgHandler>,
    /// The reorg detector.
    detector: Arc<ReorgDetector>,
    /// The event handler registry.
    registry: Arc<EventRegistry>,
    /// The write buffer.
    write_buffer: Arc<WriteBuffer>,
    /// Progress tracker.
    #[allow(dead_code)]
    progress: Arc<BackfillProgress>,
    /// Cache store for checkpoints.
    #[allow(dead_code)]
    cache_store: Arc<RpcCacheStore>,
    /// Finality tracker.
    finality_tracker: FinalityTracker,
    /// DB pool for creating contexts.
    db_pool: sqlx::PgPool,
    /// Postgres schema name.
    pg_schema: String,
    /// Signal sender for readiness.
    ready_tx: tokio::sync::watch::Sender<bool>,
    /// Chain ID.
    chain_id: u64,
    /// Table names for finality pruning.
    #[allow(dead_code)]
    table_names: Vec<String>,
    /// Block counter for periodic finality pruning.
    blocks_since_prune: std::sync::atomic::AtomicU64,
}

impl RealtimeProcessor {
    /// Creates a new realtime processor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        subscriber: NewBlockSubscriber,
        workers: Vec<Arc<BackfillWorker>>,
        reorg_handler: Arc<ReorgHandler>,
        detector: Arc<ReorgDetector>,
        registry: Arc<EventRegistry>,
        write_buffer: Arc<WriteBuffer>,
        progress: Arc<BackfillProgress>,
        cache_store: Arc<RpcCacheStore>,
        db_pool: sqlx::PgPool,
        pg_schema: String,
        ready_tx: tokio::sync::watch::Sender<bool>,
        chain_id: u64,
        table_names: Vec<String>,
    ) -> Self {
        Self {
            subscriber,
            workers,
            reorg_handler,
            detector,
            registry,
            write_buffer,
            progress,
            cache_store,
            finality_tracker: FinalityTracker::default(),
            db_pool,
            pg_schema,
            ready_tx,
            chain_id,
            table_names,
            blocks_since_prune: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Runs the realtime processing loop.
    pub async fn run(&self) -> Result<(), SyncError> {
        // Signal readiness
        let _ = self.ready_tx.send(true);

        // Subscribe to new blocks
        let stream = self.subscriber.subscribe().await?;
        futures::pin_mut!(stream);

        while let Some(block_result) = stream.next().await {
            let block = block_result?;

            // Check for reorgs
            let decision = self.detector.process_block(self.chain_id, &block).await?;

            match decision {
                crate::reorg::detector::ReorgDecision::Reorg {
                    chain_id,
                    fork_block,
                    new_tip,
                } => {
                    self.reorg_handler
                        .handle_reorg(chain_id, fork_block, new_tip)
                        .await?;
                    // After reorg handling, continue to next block
                    continue;
                }
                crate::reorg::detector::ReorgDecision::Normal => {}
            }

            // Fetch events for this single block from all workers
            let range = BlockRange {
                from: block.number,
                to: block.number,
            };

            let mut all_events: Vec<DecodedEvent> = Vec::new();
            for worker in &self.workers {
                match worker.fetch_range(&range).await {
                    Ok(events) => all_events.extend(events),
                    Err(e) => {
                        tracing::error!(
                            chain_id = self.chain_id,
                            block = block.number,
                            error = %e,
                            "Failed to fetch events for block"
                        );
                    }
                }
            }

            // Sort by log_index (all same block)
            all_events.sort_by_key(|e| e.raw_log.log_index);

            // Call handlers
            for event in &all_events {
                let handler_key = format!("{}:{}", event.contract_name, event.name);
                if let Some(handler) = self.registry.get(&handler_key) {
                    let _ctx = DbContext::new(
                        self.write_buffer.clone(),
                        self.db_pool.clone(),
                        self.pg_schema.clone(),
                    );
                    let ctx_json = serde_json::Value::Null;
                    let event_clone = event.clone();

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handler.call(event_clone, ctx_json)
                    }));

                    match result {
                        Ok(fut) => {
                            if let Err(e) = fut.await {
                                tracing::error!(
                                    handler = %handler_key,
                                    error = %e,
                                    "Handler returned error"
                                );
                            }
                        }
                        Err(panic_info) => {
                            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "unknown panic".to_string()
                            };
                            return Err(SyncError::HandlerPanic {
                                handler: handler_key,
                                message: msg,
                            });
                        }
                    }
                }
            }

            // Flush write buffer
            self.write_buffer
                .flush_all()
                .await
                .map_err(SyncError::Database)?;

            // Periodic finality pruning (every 100 blocks)
            let count = self
                .blocks_since_prune
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count > 0 && count.is_multiple_of(100) {
                let finalized = self.finality_tracker.finalized_block(block.number);
                // Prune would happen here on the reorg store
                tracing::debug!(
                    chain_id = self.chain_id,
                    finalized_block = finalized,
                    "Pruning shadow tables below block {}",
                    finalized
                );
            }
        }

        Ok(())
    }
}
