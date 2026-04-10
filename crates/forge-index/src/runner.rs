//! ForgeIndexRunner — the main run loop that orchestrates all sub-systems.

use std::collections::HashMap;
use std::sync::Arc;

use forge_index_api::ApiServer;
use forge_index_config::{Config, DatabaseConfig, Schema};
use forge_index_core::abi::parser::parse_abi;
use forge_index_core::abi::LogDecoder;
use forge_index_core::error::ForgeError;
use forge_index_core::registry::EventRegistry;
use forge_index_db::handler::EventHandlerFn;
use forge_index_db::{DatabaseManager, ReorgStore, WriteBuffer};
use forge_index_rpc::{build_from_config, RpcCacheStore};
use forge_index_sync::backfill::coordinator::BackfillCoordinator;
use forge_index_sync::backfill::progress::BackfillProgress;
use forge_index_sync::backfill::worker::BackfillWorker;
use forge_index_sync::FactoryAddressTracker;
use forge_index_telemetry::{
    log_build_id_status, BuildIdStatus as TelemetryBuildIdStatus, LogMode,
};

/// The fully wired indexer, ready to run.
pub struct ForgeIndexRunner {
    config: Config,
    schema: Schema,
    registry: Arc<EventRegistry>,
    #[allow(dead_code)]
    db_handlers: HashMap<String, Arc<dyn EventHandlerFn>>,
    build_id: String,
}

impl ForgeIndexRunner {
    /// Creates a new runner (called by `ForgeIndex::build()`).
    pub(crate) fn new(
        config: Config,
        schema: Schema,
        registry: Arc<EventRegistry>,
        db_handlers: HashMap<String, Arc<dyn EventHandlerFn>>,
        build_id: String,
    ) -> Self {
        Self {
            config,
            schema,
            registry,
            db_handlers,
            build_id,
        }
    }

    /// Returns the build ID.
    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    /// Runs the full indexer lifecycle.
    pub async fn run(self) -> Result<(), ForgeError> {
        // 1. Init logging
        let log_mode = match std::env::var("FORGE_ENV").as_deref() {
            Ok("prod") | Ok("production") => LogMode::Prod,
            _ => LogMode::Dev,
        };
        // Logging may already be initialized — ignore errors
        let _ = std::panic::catch_unwind(|| {
            forge_index_telemetry::init_logging(log_mode, "info");
        });

        // 2. Log banner
        tracing::info!(
            build_id = self.build_id.as_str(),
            "forge-index starting up — build_id: {}",
            self.build_id
        );

        // 3. Database setup
        let (_connection_string, pg_schema, _pool_max) = match &self.config.database {
            DatabaseConfig::Postgres {
                connection_string,
                schema,
                pool_max_connections,
            } => (
                connection_string.clone(),
                schema.clone(),
                *pool_max_connections,
            ),
            DatabaseConfig::PGlite { .. } => {
                return Err(ForgeError::Config(
                    "PGlite is not yet supported".to_string(),
                ));
            }
        };

        let db_manager = DatabaseManager::new(&self.config.database)
            .await
            .map_err(|e| ForgeError::Config(format!("Database setup failed: {}", e)))?;

        db_manager
            .setup(&self.schema, &pg_schema)
            .await
            .map_err(|e| match e {
                forge_index_db::DbError::SchemaLocked { schema } => {
                    ForgeError::SchemaLocked { schema }
                }
                other => ForgeError::Config(format!("Schema migration failed: {}", other)),
            })?;

        // 4. Check build ID status
        let db_build_status = db_manager
            .check_build_id(&self.schema, &pg_schema)
            .await
            .map_err(|e| ForgeError::Config(format!("Build ID check failed: {}", e)))?;

        let telemetry_status = match db_build_status {
            forge_index_db::BuildIdStatus::NotFound => TelemetryBuildIdStatus::New,
            forge_index_db::BuildIdStatus::Same => TelemetryBuildIdStatus::Same,
            forge_index_db::BuildIdStatus::Changed { old, .. } => {
                TelemetryBuildIdStatus::Changed { old }
            }
        };
        log_build_id_status(&telemetry_status, &self.build_id);

        let pool = db_manager.get_pool();

        // 5. Initialize sub-systems
        let write_buffer = Arc::new(WriteBuffer::new(
            pool.clone(),
            pg_schema.clone(),
            &self.schema,
        ));
        let _reorg_store = Arc::new(ReorgStore::new(pool.clone(), pg_schema.clone()));

        // Install metrics
        let metrics_handle = forge_index_telemetry::install_metrics_recorder();

        // Build RPC clients per chain
        let mut rpc_clients: HashMap<u64, Arc<forge_index_rpc::CachedRpcClient>> = HashMap::new();
        let cache_store = Arc::new(RpcCacheStore::new(pool.clone()));
        cache_store
            .setup()
            .await
            .map_err(|e| ForgeError::Config(format!("Failed to set up cache store: {}", e)))?;

        for chain in &self.config.chains {
            let rpc_client = build_from_config(chain).map_err(|e| ForgeError::Rpc {
                chain_id: chain.chain_id,
                message: format!("Failed to build RPC client: {}", e),
            })?;
            let cached =
                forge_index_rpc::CachedRpcClient::new(rpc_client, RpcCacheStore::new(pool.clone()));
            rpc_clients.insert(chain.chain_id, Arc::new(cached));
        }

        // Build workers per (chain_id, contract)
        let mut workers: HashMap<(u64, String), BackfillWorker> = HashMap::new();
        let mut chain_contracts: HashMap<u64, Vec<forge_index_config::ContractConfig>> =
            HashMap::new();

        let chain_id_by_name: HashMap<String, u64> = self
            .config
            .chains
            .iter()
            .map(|c| (c.name.clone(), c.chain_id))
            .collect();

        for contract in &self.config.contracts {
            let parsed_abi = parse_abi(&contract.abi_json).map_err(|e| ForgeError::AbiDecode {
                message: format!("Failed to parse ABI for {}: {}", contract.name, e),
            })?;

            let decoder = Arc::new(LogDecoder::new(&parsed_abi));
            let selectors: Vec<_> = parsed_abi.events.iter().map(|e| e.selector).collect();

            for chain_name in &contract.chain_names {
                let chain_id = *chain_id_by_name.get(chain_name).ok_or_else(|| {
                    ForgeError::Config(format!(
                        "Contract '{}' references unknown chain '{}'",
                        contract.name, chain_name
                    ))
                })?;

                if let Some(client) = rpc_clients.get(&chain_id) {
                    let worker = BackfillWorker::new(
                        client.clone(),
                        decoder.clone(),
                        contract.clone(),
                        chain_id,
                        selectors.clone(),
                    );
                    workers.insert((chain_id, contract.name.clone()), worker);
                    chain_contracts
                        .entry(chain_id)
                        .or_default()
                        .push(contract.clone());
                }
            }
        }

        // 6. Setup handlers
        for contract in &self.config.contracts {
            if let Some(handler) = self.registry.get_setup(&contract.name) {
                tracing::info!(contract = contract.name.as_str(), "Running setup handler");
                let ctx = serde_json::Value::Null;
                if let Err(e) = handler.call(ctx).await {
                    tracing::error!(
                        contract = contract.name.as_str(),
                        error = %e,
                        "Setup handler failed"
                    );
                }
            }
        }

        // 7. Ready signal
        let (ready_tx, ready_rx) = tokio::sync::watch::channel(false);

        // 8. Start API server
        let port: u16 = std::env::var("FORGE_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(42069);

        let api_server = ApiServer::new(port, ready_rx, metrics_handle)
            .with_db(pool.clone(), pg_schema.clone());
        let api_handle = tokio::spawn(async move {
            if let Err(e) = api_server.run().await {
                tracing::error!(error = %e, "API server error");
            }
        });

        // 9. Build and run backfill
        let progress = Arc::new(BackfillProgress::new());
        let factory_tracker = Arc::new(FactoryAddressTracker::new());

        // Build per-chain chunk sizes from config
        let chain_chunk_sizes: HashMap<u64, u64> = self
            .config
            .chains
            .iter()
            .filter_map(|c| c.max_block_range.map(|r| (c.chain_id, r)))
            .collect();

        let coordinator = BackfillCoordinator::new(
            workers,
            self.registry.clone(),
            self.db_handlers,
            write_buffer.clone(),
            cache_store.clone(),
            progress.clone(),
            factory_tracker,
            chain_contracts.clone(),
            chain_chunk_sizes,
            pool.clone(),
            pg_schema.clone(),
        );

        // Run backfill for each chain
        let chain_ids: Vec<u64> = chain_contracts.keys().copied().collect();
        for &chain_id in &chain_ids {
            if let Some(client) = rpc_clients.get(&chain_id) {
                let current_block =
                    client
                        .get_block_number()
                        .await
                        .map_err(|e| ForgeError::Rpc {
                            chain_id,
                            message: format!("Failed to get current block: {}", e),
                        })?;

                tracing::info!(
                    chain_id = chain_id,
                    current_block = current_block,
                    "Starting backfill"
                );

                coordinator
                    .run_chain(chain_id, current_block)
                    .await
                    .map_err(|e| ForgeError::Config(format!("Backfill failed: {}", e)))?;
            }
        }

        // 10. Signal readiness
        let _ = ready_tx.send(true);
        tracing::info!("Backfill complete — indexer is ready");

        // 11. Wait for shutdown
        let signal = crate::shutdown::shutdown_signal().await;
        tracing::info!(signal = signal, "Received {} — shutting down", signal);

        // 12. Graceful shutdown
        write_buffer
            .flush_all()
            .await
            .map_err(|e| ForgeError::Config(format!("Final flush failed: {}", e)))?;

        db_manager
            .release_lock()
            .await
            .map_err(|e| ForgeError::Config(format!("Lock release failed: {}", e)))?;

        api_handle.abort();

        tracing::info!("forge-index shut down cleanly");
        Ok(())
    }
}
