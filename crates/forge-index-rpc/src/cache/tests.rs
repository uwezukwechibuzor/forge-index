//! Tests for the RPC cache layer using testcontainers for real Postgres.
//!
//! These tests require Docker. Run with: `cargo test -p forge-index-rpc -- --ignored`

#[cfg(test)]
mod tests {
    use crate::cache::store::RpcCacheStore;
    use crate::cached_client::CachedRpcClient;
    use crate::client::RpcClient;
    use crate::dedup::RequestDedup;
    use crate::rate_limiter::RateLimiter;
    use crate::transport::build_http_provider;
    use crate::types::LogFilter;
    use forge_index_core::{Address, Block, Hash32, Log, Transaction};
    use sqlx::PgPool;
    use std::sync::Arc;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;
    use wiremock::matchers::{body_string_contains, method};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    async fn setup_pg() -> (PgPool, testcontainers::ContainerAsync<Postgres>) {
        let container = Postgres::default()
            .with_host_auth()
            .start()
            .await
            .expect("Failed to start Postgres container");

        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
        let url = format!("postgres://postgres@127.0.0.1:{}/postgres", port);

        let pool = loop {
            match PgPool::connect(&url).await {
                Ok(pool) => break pool,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            }
        };

        (pool, container)
    }

    fn sample_block() -> Block {
        Block {
            chain_id: 1,
            number: 1000,
            hash: Hash32::from(
                "0x1111111111111111111111111111111111111111111111111111111111111111",
            ),
            parent_hash: Hash32::from(
                "0x2222222222222222222222222222222222222222222222222222222222222222",
            ),
            timestamp: 1_600_000_000,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: Some(1_000_000_000),
            miner: Address::from("0x0000000000000000000000000000000000000001"),
        }
    }

    fn sample_transaction() -> Transaction {
        Transaction {
            chain_id: 1,
            hash: Hash32::from(
                "0x3333333333333333333333333333333333333333333333333333333333333333",
            ),
            block_number: 1000,
            block_hash: Hash32::from(
                "0x1111111111111111111111111111111111111111111111111111111111111111",
            ),
            from: Address::from("0x0000000000000000000000000000000000000001"),
            to: Some(Address::from("0x0000000000000000000000000000000000000002")),
            value: 1_000_000_000_000_000_000,
            gas: 21_000,
            gas_price: Some(20_000_000_000),
            input: vec![0xDE, 0xAD],
            nonce: 7,
            transaction_index: 0,
        }
    }

    fn sample_log() -> Log {
        let block_hash =
            Hash32::from("0x1111111111111111111111111111111111111111111111111111111111111111");
        Log {
            id: format!("{}-0", block_hash),
            chain_id: 1,
            address: Address::from("0x0000000000000000000000000000000000000001"),
            topics: vec![Hash32::from(
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            )],
            data: vec![0x01, 0x02, 0x03],
            block_number: 1000,
            block_hash,
            transaction_hash: Hash32::from(
                "0x3333333333333333333333333333333333333333333333333333333333333333",
            ),
            log_index: 0,
            transaction_index: 0,
            removed: false,
        }
    }

    fn sample_filter() -> LogFilter {
        LogFilter {
            address: vec![Address::from("0x0000000000000000000000000000000000000001")],
            topics: vec![Some(vec![Hash32::from(
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            )])],
            from_block: 0,
            to_block: 1000,
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn setup_creates_all_tables_and_schema() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool.clone());
        store.setup().await.expect("setup failed");

        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'ponder_sync' ORDER BY table_name",
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();
        assert!(table_names.contains(&"blocks"), "missing blocks table");
        assert!(table_names.contains(&"logs"), "missing logs table");
        assert!(
            table_names.contains(&"transactions"),
            "missing transactions table"
        );
        assert!(
            table_names.contains(&"eth_calls"),
            "missing eth_calls table"
        );
        assert!(
            table_names.contains(&"checkpoints"),
            "missing checkpoints table"
        );
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn put_logs_then_get_logs_with_matching_filter_returns_same_logs() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let filter = sample_filter();
        let logs = vec![sample_log()];
        store.put_logs(1, &filter, &logs).await.unwrap();

        let cached = store.get_logs(1, &filter).await.unwrap();
        assert!(cached.is_some(), "expected cache hit");
        let cached_logs = cached.unwrap();
        assert_eq!(cached_logs.len(), 1);
        assert_eq!(cached_logs[0].block_number, 1000);
        assert_eq!(cached_logs[0].log_index, 0);
        assert_eq!(cached_logs[0].data, vec![0x01, 0x02, 0x03]);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn get_logs_with_different_filter_returns_none() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let filter = sample_filter();
        let logs = vec![sample_log()];
        store.put_logs(1, &filter, &logs).await.unwrap();

        let different_filter = LogFilter {
            address: vec![Address::from("0x0000000000000000000000000000000000000001")],
            topics: vec![],
            from_block: 2000,
            to_block: 3000,
        };

        let cached = store.get_logs(1, &different_filter).await.unwrap();
        assert!(cached.is_none(), "expected cache miss for different filter");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn put_block_then_get_block_returns_same_block() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let block = sample_block();
        store.put_block(1, &block).await.unwrap();

        let cached = store.get_block(1, 1000).await.unwrap();
        assert!(cached.is_some(), "expected cache hit");
        let cached_block = cached.unwrap();
        assert_eq!(cached_block.number, 1000);
        assert_eq!(cached_block.chain_id, 1);
        assert_eq!(cached_block.gas_limit, 30_000_000);
        assert_eq!(cached_block.gas_used, 15_000_000);
        assert_eq!(cached_block.base_fee_per_gas, Some(1_000_000_000));
        assert_eq!(cached_block.timestamp, 1_600_000_000);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn put_transaction_then_get_transaction_returns_same_transaction() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let tx = sample_transaction();
        store.put_transaction(1, &tx).await.unwrap();

        let cached = store.get_transaction(1, &tx.hash).await.unwrap();
        assert!(cached.is_some(), "expected cache hit");
        let cached_tx = cached.unwrap();
        assert_eq!(cached_tx.hash, tx.hash);
        assert_eq!(cached_tx.block_number, 1000);
        assert_eq!(cached_tx.nonce, 7);
        assert_eq!(cached_tx.value, 1_000_000_000_000_000_000);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn put_eth_call_then_get_eth_call_with_same_params_returns_same_result() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let to = Address::from("0x0000000000000000000000000000000000000001");
        let data = vec![0xAB, 0xCD];
        let result = vec![0x00, 0x01, 0x02, 0x03];

        store
            .put_eth_call(1, &to, &data, 1000, &result)
            .await
            .unwrap();

        let cached = store.get_eth_call(1, &to, &data, 1000).await.unwrap();
        assert!(cached.is_some(), "expected cache hit");
        assert_eq!(cached.unwrap(), result);
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn get_eth_call_with_different_block_returns_none() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let to = Address::from("0x0000000000000000000000000000000000000001");
        let data = vec![0xAB, 0xCD];
        let result = vec![0x00, 0x01, 0x02, 0x03];

        store
            .put_eth_call(1, &to, &data, 1000, &result)
            .await
            .unwrap();

        let cached = store.get_eth_call(1, &to, &data, 2000).await.unwrap();
        assert!(cached.is_none(), "expected cache miss for different block");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn put_checkpoint_then_get_checkpoint_returns_saved_value() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        store
            .put_checkpoint(1, "0xContractAddr", 5000)
            .await
            .unwrap();

        let val = store.get_checkpoint(1, "0xContractAddr").await.unwrap();
        assert_eq!(val, Some(5000));

        store
            .put_checkpoint(1, "0xContractAddr", 6000)
            .await
            .unwrap();

        let val = store.get_checkpoint(1, "0xContractAddr").await.unwrap();
        assert_eq!(val, Some(6000));
    }

    // ── CachedRpcClient tests ─────────────────────────────────────────

    struct JsonRpcResponder {
        result: serde_json::Value,
    }

    impl wiremock::Respond for JsonRpcResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let id = body.get("id").cloned().unwrap_or(serde_json::Value::Null);
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": self.result,
            }))
        }
    }

    fn block_json() -> serde_json::Value {
        let bloom = format!("0x{}", "0".repeat(512));
        serde_json::json!({
            "number": "0x3e8",
            "hash": "0x1111111111111111111111111111111111111111111111111111111111111111",
            "parentHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
            "nonce": "0x0000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "logsBloom": bloom,
            "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "miner": "0x0000000000000000000000000000000000000001",
            "difficulty": "0x0",
            "totalDifficulty": "0x0",
            "extraData": "0x",
            "size": "0x0",
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x0",
            "timestamp": "0x60000000",
            "transactions": [],
            "uncles": [],
            "baseFeePerGas": "0x3b9aca00",
            "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000"
        })
    }

    async fn create_test_rpc_client(server: &MockServer) -> RpcClient {
        let provider = build_http_provider(&server.uri()).unwrap();
        RpcClient {
            provider: Arc::new(provider),
            chain_id: 1,
            rate_limiter: Arc::new(RateLimiter::new(1000)),
            dedup: Arc::new(RequestDedup::new()),
            has_ws: false,
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn cached_client_second_call_hits_cache() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(JsonRpcResponder {
                result: block_json(),
            })
            .expect(1)
            .mount(&server)
            .await;

        let rpc_client = create_test_rpc_client(&server).await;
        let cached_client = CachedRpcClient::new(rpc_client, store);

        let block1 = cached_client.get_block_by_number(1000).await.unwrap();
        assert_eq!(block1.number, 1000);

        let block2 = cached_client.get_block_by_number(1000).await.unwrap();
        assert_eq!(block2.number, 1000);
    }

    #[test]
    fn cache_stats_hit_rate_correctness() {
        // Test the CacheStats struct directly (no Docker needed)
        let stats_init = crate::cached_client::CacheStats {
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
        };
        assert_eq!(stats_init.hit_rate, 0.0, "initial hit rate should be 0.0");

        let stats_one_hit = crate::cached_client::CacheStats {
            hits: 1,
            misses: 0,
            hit_rate: 1.0,
        };
        assert_eq!(stats_one_hit.hit_rate, 1.0, "one hit should be 1.0");

        let stats_mixed = crate::cached_client::CacheStats {
            hits: 1,
            misses: 1,
            hit_rate: 0.5,
        };
        assert_eq!(stats_mixed.hit_rate, 0.5, "1 hit 1 miss should be 0.5");
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn cache_stats_via_cached_client() {
        let (pool, _container) = setup_pg().await;
        let store = RpcCacheStore::new(pool);
        store.setup().await.unwrap();

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(JsonRpcResponder {
                result: block_json(),
            })
            .mount(&server)
            .await;

        let rpc_client = create_test_rpc_client(&server).await;
        let cached_client = CachedRpcClient::new(rpc_client, store);

        let s = cached_client.stats();
        assert_eq!(s.hit_rate, 0.0);

        let _ = cached_client.get_block_by_number(1000).await.unwrap();
        let s = cached_client.stats();
        assert_eq!(s.misses, 1);
        assert_eq!(s.hits, 0);

        let _ = cached_client.get_block_by_number(1000).await.unwrap();
        let s = cached_client.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
        assert!((s.hit_rate - 0.5).abs() < f64::EPSILON);
    }
}
