//! Backfill coordinator — orchestrates historical event fetching across chains.

use std::collections::HashMap;
use std::sync::Arc;

use forge_index_config::ContractConfig;
use forge_index_core::abi::decoder::DecodedEvent;
use forge_index_core::registry::EventRegistry;
use forge_index_db::handler::EventHandlerFn;
use forge_index_db::{DbContext, WriteBuffer};

use crate::backfill::planner::{self, BackfillPlan};
use crate::backfill::progress::BackfillProgress;
use crate::backfill::worker::BackfillWorker;
use crate::error::SyncError;
use crate::factory::FactoryAddressTracker;

/// Orchestrates backfill across all chains and contracts.
pub struct BackfillCoordinator {
    /// Per-(chain_id, contract_name) workers.
    workers: HashMap<(u64, String), BackfillWorker>,
    /// Legacy event handler registry (serde_json::Value context).
    registry: Arc<EventRegistry>,
    /// Production event handlers (DbContext).
    db_handlers: HashMap<String, Arc<dyn EventHandlerFn>>,
    /// Write buffer for DB operations.
    write_buffer: Arc<WriteBuffer>,
    /// Cache store for checkpoint operations.
    cache_store: Arc<forge_index_rpc::RpcCacheStore>,
    /// Progress tracker.
    progress: Arc<BackfillProgress>,
    /// Factory address tracker.
    #[allow(dead_code)]
    factory_tracker: Arc<FactoryAddressTracker>,
    /// Contract configurations grouped by chain_id.
    chain_contracts: HashMap<u64, Vec<ContractConfig>>,
    /// Per-chain maximum block range for eth_getLogs requests.
    chain_chunk_sizes: HashMap<u64, u64>,
    /// DB context factory components.
    db_pool: sqlx::PgPool,
    /// Postgres schema name.
    pg_schema: String,
}

impl BackfillCoordinator {
    /// Creates a new backfill coordinator.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workers: HashMap<(u64, String), BackfillWorker>,
        registry: Arc<EventRegistry>,
        db_handlers: HashMap<String, Arc<dyn EventHandlerFn>>,
        write_buffer: Arc<WriteBuffer>,
        cache_store: Arc<forge_index_rpc::RpcCacheStore>,
        progress: Arc<BackfillProgress>,
        factory_tracker: Arc<FactoryAddressTracker>,
        chain_contracts: HashMap<u64, Vec<ContractConfig>>,
        chain_chunk_sizes: HashMap<u64, u64>,
        db_pool: sqlx::PgPool,
        pg_schema: String,
    ) -> Self {
        Self {
            workers,
            registry,
            db_handlers,
            write_buffer,
            cache_store,
            progress,
            factory_tracker,
            chain_contracts,
            chain_chunk_sizes,
            db_pool,
            pg_schema,
        }
    }

    /// Runs the backfill for a single chain.
    pub async fn run_chain(&self, chain_id: u64, current_block: u64) -> Result<(), SyncError> {
        let contracts = self
            .chain_contracts
            .get(&chain_id)
            .cloned()
            .unwrap_or_default();

        if contracts.is_empty() {
            return Ok(());
        }

        // Build plans for all contracts on this chain
        let mut plans: Vec<BackfillPlan> = Vec::new();
        for contract in &contracts {
            let checkpoint = self
                .cache_store
                .get_checkpoint(chain_id, &contract.name)
                .await
                .ok()
                .flatten();

            let chunk_size = self
                .chain_chunk_sizes
                .get(&chain_id)
                .copied()
                .unwrap_or(planner::DEFAULT_CHUNK_SIZE);

            let plan = planner::plan(contract, chain_id, current_block, checkpoint, chunk_size);
            plans.push(plan);
        }

        // Calculate total blocks for progress
        let total_blocks: u64 = plans.iter().map(|p| p.total_blocks).sum();
        self.progress.init_chain(chain_id, total_blocks);

        // Find the max number of ranges across all plans
        let max_ranges = plans.iter().map(|p| p.ranges.len()).max().unwrap_or(0);

        // Process ranges sequentially
        for range_idx in 0..max_ranges {
            let mut all_events: Vec<DecodedEvent> = Vec::new();

            for plan in &plans {
                if range_idx >= plan.ranges.len() {
                    continue;
                }
                let range = &plan.ranges[range_idx];
                let key = (chain_id, plan.contract_name.clone());

                if let Some(worker) = self.workers.get(&key) {
                    match worker.fetch_range(range).await {
                        Ok(events) => all_events.extend(events),
                        Err(e) => {
                            tracing::error!(
                                chain_id = chain_id,
                                contract = %plan.contract_name,
                                range_from = range.from,
                                range_to = range.to,
                                error = %e,
                                "Failed to fetch range"
                            );
                            return Err(e);
                        }
                    }
                }
            }

            // Sort merged events by (block_number, log_index)
            all_events.sort_by(|a, b| {
                let block_cmp = a.raw_log.block_number.cmp(&b.raw_log.block_number);
                block_cmp.then(a.raw_log.log_index.cmp(&b.raw_log.log_index))
            });

            // Process events through handlers
            let events_count = all_events.len() as u64;
            for event in &all_events {
                forge_index_telemetry::record_event_indexed(
                    chain_id,
                    &event.contract_name,
                    &event.name,
                );

                let handler_key = format!("{}:{}", event.contract_name, event.name);

                // Try db_handlers first (production: DbContext), then legacy registry
                if let Some(handler) = self.db_handlers.get(&handler_key) {
                    let ctx = DbContext::new(
                        self.write_buffer.clone(),
                        self.db_pool.clone(),
                        self.pg_schema.clone(),
                    );

                    let event_clone = event.clone();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handler.call(event_clone, ctx)
                    }));

                    match result {
                        Ok(fut) => {
                            if let Err(e) = fut.await {
                                tracing::error!(
                                    handler = %handler_key,
                                    error = %e,
                                    "Handler returned error (non-panic)"
                                );
                            }
                        }
                        Err(panic_info) => {
                            let msg = panic_message(panic_info);
                            return Err(SyncError::HandlerPanic {
                                handler: handler_key,
                                message: msg,
                            });
                        }
                    }
                } else if let Some(handler) = self.registry.get(&handler_key) {
                    // Legacy handler (serde_json::Value context)
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
                                    "Handler returned error (non-panic)"
                                );
                            }
                        }
                        Err(panic_info) => {
                            let msg = panic_message(panic_info);
                            return Err(SyncError::HandlerPanic {
                                handler: handler_key,
                                message: msg,
                            });
                        }
                    }
                }
            }

            // Flush write buffer after each range
            self.write_buffer
                .flush_all()
                .await
                .map_err(SyncError::Database)?;

            let blocks_in_range = plans
                .iter()
                .filter_map(|p| p.ranges.get(range_idx).map(|r| r.len()))
                .max()
                .unwrap_or(0);

            // Update checkpoints
            for plan in &plans {
                if range_idx < plan.ranges.len() {
                    let range = &plan.ranges[range_idx];
                    let _ = self
                        .cache_store
                        .put_checkpoint(chain_id, &plan.contract_name, range.to + 1)
                        .await;
                }
            }

            self.progress
                .record(chain_id, blocks_in_range, events_count);

            for _ in 0..blocks_in_range {
                forge_index_telemetry::record_block_processed(chain_id);
            }
        }

        Ok(())
    }
}

fn panic_message(panic_info: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic_info.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic_info.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
