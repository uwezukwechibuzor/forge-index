//! SQL migrations for the ponder_sync schema.

/// All SQL statements to create the ponder_sync schema and tables.
pub const MIGRATIONS: &[&str] = &[
    "CREATE SCHEMA IF NOT EXISTS ponder_sync",
    r#"CREATE TABLE IF NOT EXISTS ponder_sync.logs (
  chain_id        BIGINT      NOT NULL,
  from_block      BIGINT      NOT NULL,
  to_block        BIGINT      NOT NULL,
  address         TEXT[]      NOT NULL,
  topics          TEXT[]      NOT NULL,
  log_index       INT         NOT NULL,
  block_number    BIGINT      NOT NULL,
  block_hash      TEXT        NOT NULL,
  tx_hash         TEXT        NOT NULL,
  tx_index        INT         NOT NULL,
  log_address     TEXT        NOT NULL,
  log_topics      TEXT[]      NOT NULL,
  data            BYTEA       NOT NULL,
  removed         BOOLEAN     NOT NULL DEFAULT false,
  PRIMARY KEY (chain_id, block_number, log_index)
)"#,
    r#"CREATE TABLE IF NOT EXISTS ponder_sync.blocks (
  chain_id        BIGINT      NOT NULL,
  block_number    BIGINT      NOT NULL,
  block_hash      TEXT        NOT NULL,
  parent_hash     TEXT        NOT NULL,
  timestamp       BIGINT      NOT NULL,
  miner           TEXT        NOT NULL,
  gas_limit       BIGINT      NOT NULL,
  gas_used        BIGINT      NOT NULL,
  base_fee        NUMERIC,
  raw             JSONB       NOT NULL,
  PRIMARY KEY (chain_id, block_number)
)"#,
    r#"CREATE TABLE IF NOT EXISTS ponder_sync.transactions (
  chain_id        BIGINT      NOT NULL,
  tx_hash         TEXT        NOT NULL,
  block_number    BIGINT      NOT NULL,
  raw             JSONB       NOT NULL,
  PRIMARY KEY (chain_id, tx_hash)
)"#,
    r#"CREATE TABLE IF NOT EXISTS ponder_sync.eth_calls (
  chain_id        BIGINT      NOT NULL,
  call_key        TEXT        NOT NULL,
  result          BYTEA       NOT NULL,
  block_number    BIGINT      NOT NULL,
  PRIMARY KEY (chain_id, call_key)
)"#,
    r#"CREATE TABLE IF NOT EXISTS ponder_sync.checkpoints (
  chain_id              BIGINT  NOT NULL,
  contract_address      TEXT    NOT NULL,
  last_indexed_block    BIGINT  NOT NULL,
  PRIMARY KEY (chain_id, contract_address)
)"#,
    r#"CREATE INDEX IF NOT EXISTS idx_logs_chain_block
  ON ponder_sync.logs (chain_id, block_number)"#,
];
