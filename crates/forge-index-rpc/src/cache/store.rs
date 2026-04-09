//! Postgres-backed RPC response cache store.

use crate::cache::keys;
use crate::cache::migrations;
use crate::types::LogFilter;
use forge_index_core::{Address, Block, Hash32, Log, Transaction};
use sqlx::PgPool;

/// Durable Postgres-backed cache for RPC responses.
///
/// All data lives in the `ponder_sync` schema so it can be shared across
/// hot reloads and process restarts without redundant network calls.
pub struct RpcCacheStore {
    pool: PgPool,
}

impl RpcCacheStore {
    /// Creates a new cache store backed by the given Postgres connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Runs all CREATE SCHEMA / CREATE TABLE migrations.
    pub async fn setup(&self) -> Result<(), sqlx::Error> {
        for stmt in migrations::MIGRATIONS {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
        Ok(())
    }

    // ── Logs ──────────────────────────────────────────────────────────────

    /// Returns cached logs for the given filter, or `None` on cache miss.
    pub async fn get_logs(
        &self,
        chain_id: u64,
        filter: &LogFilter,
    ) -> Result<Option<Vec<Log>>, sqlx::Error> {
        let addresses: Vec<String> = filter.address.iter().map(|a| a.to_string()).collect();
        let topics: Vec<String> = filter
            .topics
            .iter()
            .filter_map(|t| {
                t.as_ref().map(|hashes| {
                    hashes
                        .iter()
                        .map(|h| h.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
            })
            .collect();

        // Check if we have any rows matching this exact filter window
        let count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM ponder_sync.logs
               WHERE chain_id = $1
                 AND from_block = $2
                 AND to_block = $3
                 AND address = $4
                 AND topics = $5"#,
        )
        .bind(chain_id as i64)
        .bind(filter.from_block as i64)
        .bind(filter.to_block as i64)
        .bind(&addresses)
        .bind(&topics)
        .fetch_one(&self.pool)
        .await?;

        if count.0 == 0 {
            // Check if we ever stored a zero-result query for this filter
            // by looking for a sentinel — if no rows match, it's a cache miss
            // We distinguish "cached empty" from "never queried" using a special approach:
            // We always insert at least a marker. But for simplicity, if count == 0
            // and we haven't stored a marker, return None.
            // For the MVP: just check if any log rows exist for this filter range.
            // A true cache miss returns None.
            return Ok(None);
        }

        let rows: Vec<LogRow> = sqlx::query_as(
            r#"SELECT chain_id, block_number, block_hash, tx_hash, tx_index,
                      log_address, log_topics, data, log_index, removed
               FROM ponder_sync.logs
               WHERE chain_id = $1
                 AND from_block = $2
                 AND to_block = $3
                 AND address = $4
                 AND topics = $5
               ORDER BY block_number, log_index"#,
        )
        .bind(chain_id as i64)
        .bind(filter.from_block as i64)
        .bind(filter.to_block as i64)
        .bind(&addresses)
        .bind(&topics)
        .fetch_all(&self.pool)
        .await?;

        let logs: Vec<Log> = rows.into_iter().map(|r| r.into_log(chain_id)).collect();
        Ok(Some(logs))
    }

    /// Stores logs for the given filter. Uses ON CONFLICT DO NOTHING.
    pub async fn put_logs(
        &self,
        chain_id: u64,
        filter: &LogFilter,
        logs: &[Log],
    ) -> Result<(), sqlx::Error> {
        let addresses: Vec<String> = filter.address.iter().map(|a| a.to_string()).collect();
        let topics: Vec<String> = filter
            .topics
            .iter()
            .filter_map(|t| {
                t.as_ref().map(|hashes| {
                    hashes
                        .iter()
                        .map(|h| h.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
            })
            .collect();

        for log in logs {
            let log_topics: Vec<String> = log.topics.iter().map(|t| t.to_string()).collect();
            sqlx::query(
                r#"INSERT INTO ponder_sync.logs
                   (chain_id, from_block, to_block, address, topics,
                    log_index, block_number, block_hash, tx_hash, tx_index,
                    log_address, log_topics, data, removed)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(chain_id as i64)
            .bind(filter.from_block as i64)
            .bind(filter.to_block as i64)
            .bind(&addresses)
            .bind(&topics)
            .bind(log.log_index as i32)
            .bind(log.block_number as i64)
            .bind(log.block_hash.to_string())
            .bind(log.transaction_hash.to_string())
            .bind(log.transaction_index as i32)
            .bind(log.address.to_string())
            .bind(&log_topics)
            .bind(&log.data)
            .bind(log.removed)
            .execute(&self.pool)
            .await?;
        }

        // For empty results, insert a sentinel row so we can distinguish
        // "queried and got zero results" from "never queried".
        if logs.is_empty() {
            sqlx::query(
                r#"INSERT INTO ponder_sync.logs
                   (chain_id, from_block, to_block, address, topics,
                    log_index, block_number, block_hash, tx_hash, tx_index,
                    log_address, log_topics, data, removed)
                   VALUES ($1, $2, $3, $4, $5, -1, -1, '', '', -1, '', '{}', '\x', false)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(chain_id as i64)
            .bind(filter.from_block as i64)
            .bind(filter.to_block as i64)
            .bind(&addresses)
            .bind(&topics)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    // ── Blocks ────────────────────────────────────────────────────────────

    /// Returns a cached block, or `None` on cache miss.
    pub async fn get_block(
        &self,
        chain_id: u64,
        number: u64,
    ) -> Result<Option<Block>, sqlx::Error> {
        let row: Option<BlockRow> = sqlx::query_as(
            r#"SELECT chain_id, block_number, block_hash, parent_hash,
                      timestamp, miner, gas_limit, gas_used,
                      base_fee::TEXT as base_fee
               FROM ponder_sync.blocks
               WHERE chain_id = $1 AND block_number = $2"#,
        )
        .bind(chain_id as i64)
        .bind(number as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into_block()))
    }

    /// Stores a block in the cache. Uses ON CONFLICT DO NOTHING.
    pub async fn put_block(&self, chain_id: u64, block: &Block) -> Result<(), sqlx::Error> {
        let raw = serde_json::to_value(block).unwrap_or_default();
        sqlx::query(
            r#"INSERT INTO ponder_sync.blocks
               (chain_id, block_number, block_hash, parent_hash,
                timestamp, miner, gas_limit, gas_used, base_fee, raw)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(chain_id as i64)
        .bind(block.number as i64)
        .bind(block.hash.to_string())
        .bind(block.parent_hash.to_string())
        .bind(block.timestamp as i64)
        .bind(block.miner.to_string())
        .bind(block.gas_limit as i64)
        .bind(block.gas_used as i64)
        .bind(block.base_fee_per_gas.map(|v| v.to_string()))
        .bind(raw)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Transactions ──────────────────────────────────────────────────────

    /// Returns a cached transaction, or `None` on cache miss.
    pub async fn get_transaction(
        &self,
        chain_id: u64,
        hash: &Hash32,
    ) -> Result<Option<Transaction>, sqlx::Error> {
        let row: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"SELECT raw FROM ponder_sync.transactions
               WHERE chain_id = $1 AND tx_hash = $2"#,
        )
        .bind(chain_id as i64)
        .bind(hash.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((raw,)) => {
                let tx: Transaction = serde_json::from_value(raw).map_err(|e| {
                    sqlx::Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                })?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    /// Stores a transaction in the cache. Uses ON CONFLICT DO NOTHING.
    pub async fn put_transaction(
        &self,
        chain_id: u64,
        tx: &Transaction,
    ) -> Result<(), sqlx::Error> {
        let raw = serde_json::to_value(tx).unwrap_or_default();
        sqlx::query(
            r#"INSERT INTO ponder_sync.transactions (chain_id, tx_hash, block_number, raw)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(chain_id as i64)
        .bind(tx.hash.to_string())
        .bind(tx.block_number as i64)
        .bind(raw)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── eth_call ──────────────────────────────────────────────────────────

    /// Returns a cached eth_call result, or `None` on cache miss.
    pub async fn get_eth_call(
        &self,
        chain_id: u64,
        to: &Address,
        data: &[u8],
        block: u64,
    ) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let call_key = keys::eth_call_key(to, data, block);
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            r#"SELECT result FROM ponder_sync.eth_calls
               WHERE chain_id = $1 AND call_key = $2"#,
        )
        .bind(chain_id as i64)
        .bind(&call_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(r,)| r))
    }

    /// Stores an eth_call result in the cache. Uses ON CONFLICT DO NOTHING.
    pub async fn put_eth_call(
        &self,
        chain_id: u64,
        to: &Address,
        data: &[u8],
        block: u64,
        result: &[u8],
    ) -> Result<(), sqlx::Error> {
        let call_key = keys::eth_call_key(to, data, block);
        sqlx::query(
            r#"INSERT INTO ponder_sync.eth_calls (chain_id, call_key, result, block_number)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(chain_id as i64)
        .bind(&call_key)
        .bind(result)
        .bind(block as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Checkpoints ───────────────────────────────────────────────────────

    /// Returns the last indexed block for a contract, or `None` if not set.
    pub async fn get_checkpoint(
        &self,
        chain_id: u64,
        contract_address: &str,
    ) -> Result<Option<u64>, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            r#"SELECT last_indexed_block FROM ponder_sync.checkpoints
               WHERE chain_id = $1 AND contract_address = $2"#,
        )
        .bind(chain_id as i64)
        .bind(contract_address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(v,)| v as u64))
    }

    /// Sets the last indexed block for a contract. Uses upsert.
    pub async fn put_checkpoint(
        &self,
        chain_id: u64,
        contract_address: &str,
        last_block: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO ponder_sync.checkpoints (chain_id, contract_address, last_indexed_block)
               VALUES ($1, $2, $3)
               ON CONFLICT (chain_id, contract_address)
               DO UPDATE SET last_indexed_block = EXCLUDED.last_indexed_block"#,
        )
        .bind(chain_id as i64)
        .bind(contract_address)
        .bind(last_block as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Internal row type for log queries.
#[derive(sqlx::FromRow)]
struct LogRow {
    #[allow(dead_code)]
    chain_id: i64,
    block_number: i64,
    block_hash: String,
    tx_hash: String,
    tx_index: i32,
    log_address: String,
    log_topics: Vec<String>,
    data: Vec<u8>,
    log_index: i32,
    removed: bool,
}

impl LogRow {
    fn into_log(self, chain_id: u64) -> Log {
        let block_hash = Hash32::from_hex(&self.block_hash).unwrap_or(Hash32([0u8; 32]));
        Log {
            id: format!("{}-{}", self.block_hash, self.log_index),
            chain_id,
            address: Address::from_hex(&self.log_address).unwrap_or(Address([0u8; 20])),
            topics: self
                .log_topics
                .iter()
                .filter_map(|t| Hash32::from_hex(t))
                .collect(),
            data: self.data,
            block_number: self.block_number as u64,
            block_hash,
            transaction_hash: Hash32::from_hex(&self.tx_hash).unwrap_or(Hash32([0u8; 32])),
            log_index: self.log_index as u32,
            transaction_index: self.tx_index as u32,
            removed: self.removed,
        }
    }
}

/// Internal row type for block queries.
#[derive(sqlx::FromRow)]
struct BlockRow {
    chain_id: i64,
    block_number: i64,
    block_hash: String,
    parent_hash: String,
    timestamp: i64,
    miner: String,
    gas_limit: i64,
    gas_used: i64,
    base_fee: Option<String>,
}

impl BlockRow {
    fn into_block(self) -> Block {
        Block {
            chain_id: self.chain_id as u64,
            number: self.block_number as u64,
            hash: Hash32::from_hex(&self.block_hash).unwrap_or(Hash32([0u8; 32])),
            parent_hash: Hash32::from_hex(&self.parent_hash).unwrap_or(Hash32([0u8; 32])),
            timestamp: self.timestamp as u64,
            miner: Address::from_hex(&self.miner).unwrap_or(Address([0u8; 20])),
            gas_limit: self.gas_limit as u64,
            gas_used: self.gas_used as u64,
            base_fee_per_gas: self.base_fee.and_then(|s| s.parse::<u128>().ok()),
        }
    }
}
