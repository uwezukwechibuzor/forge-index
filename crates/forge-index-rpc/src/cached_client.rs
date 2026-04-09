//! Cache-through wrapper around `RpcClient`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures::stream::BoxStream;

use crate::cache::RpcCacheStore;
use crate::client::RpcClient;
use crate::error::RpcError;
use crate::types::{LogFilter, TransactionReceipt};
use forge_index_core::{Address, Block, Hash32, Log, Transaction};

/// Per-session cache hit/miss statistics.
#[derive(Debug)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Hit rate as a ratio (0.0 to 1.0).
    pub hit_rate: f64,
}

/// RPC client with a Postgres-backed cache layer.
///
/// All read methods check the cache first, fall through to the live RPC on
/// cache miss, and write successful results back to the cache.
///
/// `get_block_number` and `subscribe_new_heads` are never cached.
pub struct CachedRpcClient {
    inner: RpcClient,
    cache: RpcCacheStore,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CachedRpcClient {
    /// Creates a new cached RPC client.
    pub fn new(inner: RpcClient, cache: RpcCacheStore) -> Self {
        Self {
            inner,
            cache,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Returns current cache statistics.
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        };
        CacheStats {
            hits,
            misses,
            hit_rate,
        }
    }

    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Fetches logs, checking cache first.
    pub async fn get_logs(&self, filter: LogFilter) -> Result<Vec<Log>, RpcError> {
        let chain_id = self.inner.chain_id;

        if let Ok(Some(logs)) = self.cache.get_logs(chain_id, &filter).await {
            // Filter out sentinel rows (log_index == -1 mapped to u32::MAX won't appear
            // since we store them as i32 -1, but the conversion yields max u32)
            let real_logs: Vec<Log> = logs
                .into_iter()
                .filter(|l| l.log_index != u32::MAX)
                .collect();
            self.record_hit();
            return Ok(real_logs);
        }

        self.record_miss();
        let logs = self.inner.get_logs(filter.clone()).await?;
        let _ = self.cache.put_logs(chain_id, &filter, &logs).await;
        Ok(logs)
    }

    /// Fetches a block by number, checking cache first.
    pub async fn get_block_by_number(&self, n: u64) -> Result<Block, RpcError> {
        let chain_id = self.inner.chain_id;

        if let Ok(Some(block)) = self.cache.get_block(chain_id, n).await {
            self.record_hit();
            return Ok(block);
        }

        self.record_miss();
        let block = self.inner.get_block_by_number(n).await?;
        let _ = self.cache.put_block(chain_id, &block).await;
        Ok(block)
    }

    /// Fetches a block by hash — delegates to inner (no separate cache path by hash).
    pub async fn get_block_by_hash(&self, hash: Hash32) -> Result<Block, RpcError> {
        self.inner.get_block_by_hash(hash).await
    }

    /// Fetches a transaction, checking cache first.
    pub async fn get_transaction(&self, hash: Hash32) -> Result<Transaction, RpcError> {
        let chain_id = self.inner.chain_id;

        if let Ok(Some(tx)) = self.cache.get_transaction(chain_id, &hash).await {
            self.record_hit();
            return Ok(tx);
        }

        self.record_miss();
        let tx = self.inner.get_transaction(hash).await?;
        let _ = self.cache.put_transaction(chain_id, &tx).await;
        Ok(tx)
    }

    /// Fetches a transaction receipt — always live, no cache.
    pub async fn get_transaction_receipt(
        &self,
        hash: Hash32,
    ) -> Result<TransactionReceipt, RpcError> {
        self.inner.get_transaction_receipt(hash).await
    }

    /// Returns current block number — NEVER cached.
    pub async fn get_block_number(&self) -> Result<u64, RpcError> {
        self.inner.get_block_number().await
    }

    /// Executes an eth_call, checking cache first.
    pub async fn eth_call(
        &self,
        to: Address,
        data: Vec<u8>,
        block: u64,
    ) -> Result<Vec<u8>, RpcError> {
        let chain_id = self.inner.chain_id;

        if let Ok(Some(result)) = self.cache.get_eth_call(chain_id, &to, &data, block).await {
            self.record_hit();
            return Ok(result);
        }

        self.record_miss();
        let result = self.inner.eth_call(to, data.clone(), block).await?;
        let _ = self
            .cache
            .put_eth_call(chain_id, &to, &data, block, &result)
            .await;
        Ok(result)
    }

    /// Subscribes to new block headers — NOT cached (streaming).
    pub async fn subscribe_new_heads(&self) -> Result<BoxStream<'static, Block>, RpcError> {
        self.inner.subscribe_new_heads().await
    }

    /// Starts a background task that logs cache statistics every 60 seconds.
    pub fn start_stats_logger(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let stats = this.stats();
                tracing::info!(
                    hits = stats.hits,
                    misses = stats.misses,
                    hit_rate = format!("{:.1}%", stats.hit_rate * 100.0),
                    "RPC cache: {} hits, {} misses ({:.1}% hit rate)",
                    stats.hits,
                    stats.misses,
                    stats.hit_rate * 100.0,
                );
            }
        });
    }
}
