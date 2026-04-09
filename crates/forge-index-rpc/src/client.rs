//! The main RPC client type.

use std::sync::Arc;

use alloy::eips::BlockNumberOrTag;
use alloy::providers::Provider;
use alloy::rpc::types::BlockTransactionsKind;
use futures::stream::BoxStream;
use futures::StreamExt;

use crate::dedup::RequestDedup;
use crate::error::RpcError;
use crate::rate_limiter::RateLimiter;
use crate::retry;
use crate::types::{cache_key, LogFilter, TransactionReceipt};
use forge_index_core::{Address, Block, Hash32, Log, Transaction};

/// The main RPC client for interacting with an EVM chain.
///
/// Provides methods for all common JSON-RPC calls with built-in
/// retry logic, rate limiting, and request deduplication.
pub struct RpcClient {
    pub(crate) provider: Arc<alloy::providers::ReqwestProvider>,
    pub(crate) chain_id: u64,
    pub(crate) rate_limiter: Arc<RateLimiter>,
    pub(crate) dedup: Arc<RequestDedup>,
    pub(crate) has_ws: bool,
}

impl RpcClient {
    /// Fetches logs matching the given filter.
    pub async fn get_logs(&self, filter: LogFilter) -> Result<Vec<Log>, RpcError> {
        let alloy_filter = filter.to_alloy_filter();
        let key = cache_key("eth_getLogs", &alloy_filter);
        let provider = self.provider.clone();
        let rate_limiter = self.rate_limiter.clone();
        let chain_id = self.chain_id;

        let json_val = self
            .dedup
            .dedup(key, || async move {
                rate_limiter.acquire().await;
                retry::with_retry("eth_getLogs", chain_id, || {
                    let p = provider.clone();
                    let f = alloy_filter.clone();
                    async move {
                        let logs = p
                            .get_logs(&f)
                            .await
                            .map_err(|e| RpcError::Transport(e.to_string()))?;
                        serde_json::to_value(&logs).map_err(|e| RpcError::Decode {
                            method: "eth_getLogs".to_string(),
                            message: e.to_string(),
                        })
                    }
                })
                .await
            })
            .await?;

        let alloy_logs: Vec<alloy::rpc::types::Log> =
            serde_json::from_value(json_val).map_err(|e| RpcError::Decode {
                method: "eth_getLogs".to_string(),
                message: e.to_string(),
            })?;

        Ok(alloy_logs
            .iter()
            .map(|l| Log::from_alloy(l, chain_id))
            .collect())
    }

    /// Fetches a block by its number.
    pub async fn get_block_by_number(&self, n: u64) -> Result<Block, RpcError> {
        let key = cache_key("eth_getBlockByNumber", &n);
        let provider = self.provider.clone();
        let rate_limiter = self.rate_limiter.clone();
        let chain_id = self.chain_id;

        let json_val = self
            .dedup
            .dedup(key, || async move {
                rate_limiter.acquire().await;
                retry::with_retry("eth_getBlockByNumber", chain_id, || {
                    let p = provider.clone();
                    async move {
                        let block = p
                            .get_block_by_number(
                                BlockNumberOrTag::Number(n),
                                BlockTransactionsKind::Full,
                            )
                            .await
                            .map_err(|e| RpcError::Transport(e.to_string()))?;
                        let block = block.ok_or_else(|| RpcError::Decode {
                            method: "eth_getBlockByNumber".to_string(),
                            message: "block not found".to_string(),
                        })?;
                        serde_json::to_value(&block).map_err(|e| RpcError::Decode {
                            method: "eth_getBlockByNumber".to_string(),
                            message: e.to_string(),
                        })
                    }
                })
                .await
            })
            .await?;

        let alloy_block: alloy::rpc::types::Block =
            serde_json::from_value(json_val).map_err(|e| RpcError::Decode {
                method: "eth_getBlockByNumber".to_string(),
                message: e.to_string(),
            })?;

        Ok(Block::from_alloy(&alloy_block, chain_id))
    }

    /// Fetches a block by its hash.
    pub async fn get_block_by_hash(&self, hash: Hash32) -> Result<Block, RpcError> {
        let b256 = alloy::primitives::B256::from(hash.0);
        let key = cache_key("eth_getBlockByHash", &b256);
        let provider = self.provider.clone();
        let rate_limiter = self.rate_limiter.clone();
        let chain_id = self.chain_id;

        let json_val = self
            .dedup
            .dedup(key, || async move {
                rate_limiter.acquire().await;
                retry::with_retry("eth_getBlockByHash", chain_id, || {
                    let p = provider.clone();
                    async move {
                        let block = p
                            .get_block_by_hash(b256, BlockTransactionsKind::Full)
                            .await
                            .map_err(|e| RpcError::Transport(e.to_string()))?;
                        let block = block.ok_or_else(|| RpcError::Decode {
                            method: "eth_getBlockByHash".to_string(),
                            message: "block not found".to_string(),
                        })?;
                        serde_json::to_value(&block).map_err(|e| RpcError::Decode {
                            method: "eth_getBlockByHash".to_string(),
                            message: e.to_string(),
                        })
                    }
                })
                .await
            })
            .await?;

        let alloy_block: alloy::rpc::types::Block =
            serde_json::from_value(json_val).map_err(|e| RpcError::Decode {
                method: "eth_getBlockByHash".to_string(),
                message: e.to_string(),
            })?;

        Ok(Block::from_alloy(&alloy_block, chain_id))
    }

    /// Fetches a transaction by its hash.
    pub async fn get_transaction(&self, hash: Hash32) -> Result<Transaction, RpcError> {
        let b256 = alloy::primitives::B256::from(hash.0);
        let key = cache_key("eth_getTransactionByHash", &b256);
        let provider = self.provider.clone();
        let rate_limiter = self.rate_limiter.clone();
        let chain_id = self.chain_id;

        let json_val = self
            .dedup
            .dedup(key, || async move {
                rate_limiter.acquire().await;
                retry::with_retry("eth_getTransactionByHash", chain_id, || {
                    let p = provider.clone();
                    async move {
                        let tx = p
                            .get_transaction_by_hash(b256)
                            .await
                            .map_err(|e| RpcError::Transport(e.to_string()))?;
                        let tx = tx.ok_or_else(|| RpcError::Decode {
                            method: "eth_getTransactionByHash".to_string(),
                            message: "transaction not found".to_string(),
                        })?;
                        serde_json::to_value(&tx).map_err(|e| RpcError::Decode {
                            method: "eth_getTransactionByHash".to_string(),
                            message: e.to_string(),
                        })
                    }
                })
                .await
            })
            .await?;

        let alloy_tx: alloy::rpc::types::Transaction =
            serde_json::from_value(json_val).map_err(|e| RpcError::Decode {
                method: "eth_getTransactionByHash".to_string(),
                message: e.to_string(),
            })?;

        Ok(Transaction::from_alloy(&alloy_tx, chain_id))
    }

    /// Fetches a transaction receipt by transaction hash.
    pub async fn get_transaction_receipt(
        &self,
        hash: Hash32,
    ) -> Result<TransactionReceipt, RpcError> {
        let b256 = alloy::primitives::B256::from(hash.0);
        let provider = self.provider.clone();
        let rate_limiter = self.rate_limiter.clone();
        let chain_id = self.chain_id;

        rate_limiter.acquire().await;
        let receipt = retry::with_retry("eth_getTransactionReceipt", chain_id, || {
            let p = provider.clone();
            async move {
                let receipt = p
                    .get_transaction_receipt(b256)
                    .await
                    .map_err(|e| RpcError::Transport(e.to_string()))?;
                receipt.ok_or_else(|| RpcError::Decode {
                    method: "eth_getTransactionReceipt".to_string(),
                    message: "receipt not found".to_string(),
                })
            }
        })
        .await?;

        let logs: Vec<Log> = receipt
            .inner
            .logs()
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let block_hash = receipt
                    .block_hash
                    .map(Hash32::from)
                    .unwrap_or(Hash32([0u8; 32]));
                Log {
                    id: format!("{}-{}", block_hash, i),
                    chain_id,
                    address: Address::from(l.address()),
                    topics: l.topics().iter().map(|t| Hash32::from(*t)).collect(),
                    data: l.data().data.to_vec(),
                    block_number: receipt.block_number.unwrap_or(0),
                    block_hash,
                    transaction_hash: Hash32::from(receipt.transaction_hash),
                    log_index: i as u32,
                    transaction_index: receipt.transaction_index.unwrap_or(0) as u32,
                    removed: false,
                }
            })
            .collect();

        Ok(TransactionReceipt {
            transaction_hash: Hash32::from(receipt.transaction_hash),
            block_number: receipt.block_number.unwrap_or(0),
            block_hash: receipt
                .block_hash
                .map(Hash32::from)
                .unwrap_or(Hash32([0u8; 32])),
            from: Address::from(receipt.from),
            to: receipt.to.map(Address::from),
            gas_used: receipt.gas_used as u64,
            status: receipt.inner.status(),
            logs,
            contract_address: receipt.contract_address.map(Address::from),
        })
    }

    /// Returns the current block number.
    pub async fn get_block_number(&self) -> Result<u64, RpcError> {
        self.rate_limiter.acquire().await;
        let provider = self.provider.clone();
        let chain_id = self.chain_id;

        retry::with_retry("eth_blockNumber", chain_id, || {
            let p = provider.clone();
            async move {
                p.get_block_number()
                    .await
                    .map_err(|e| RpcError::Transport(e.to_string()))
            }
        })
        .await
    }

    /// Executes a read-only call against a contract.
    pub async fn eth_call(
        &self,
        to: Address,
        data: Vec<u8>,
        block: u64,
    ) -> Result<Vec<u8>, RpcError> {
        self.rate_limiter.acquire().await;
        let provider = self.provider.clone();
        let chain_id = self.chain_id;
        let to_alloy: alloy::primitives::Address = to.into();

        retry::with_retry("eth_call", chain_id, || {
            let p = provider.clone();
            let input = data.clone();
            async move {
                let tx = alloy::rpc::types::TransactionRequest::default()
                    .to(to_alloy)
                    .input(alloy::primitives::Bytes::from(input).into());
                let result = p
                    .call(&tx)
                    .block(alloy::eips::BlockId::from(block))
                    .await
                    .map_err(|e| RpcError::Transport(e.to_string()))?;
                Ok(result.to_vec())
            }
        })
        .await
    }

    /// Subscribes to new block headers via WebSocket.
    ///
    /// Returns an error if the client has no WebSocket transport configured.
    pub async fn subscribe_new_heads(&self) -> Result<BoxStream<'static, Block>, RpcError> {
        if !self.has_ws {
            return Err(RpcError::NoWebSocket {
                chain_id: self.chain_id,
            });
        }

        let chain_id = self.chain_id;
        let subscription = self
            .provider
            .subscribe_blocks()
            .await
            .map_err(|e| RpcError::Transport(e.to_string()))?;

        let stream = subscription.into_stream().map(move |header| {
            use alloy::consensus::BlockHeader;
            Block {
                chain_id,
                number: header.number(),
                hash: Hash32::from(header.hash),
                parent_hash: Hash32::from(header.parent_hash()),
                timestamp: header.timestamp(),
                gas_limit: header.gas_limit(),
                gas_used: header.gas_used(),
                base_fee_per_gas: header.base_fee_per_gas().map(|v| v as u128),
                miner: Address::from(header.beneficiary()),
            }
        });

        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::build_http_provider;
    use wiremock::matchers::{body_string_contains, method};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    /// Custom wiremock responder that echoes the JSON-RPC request ID.
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

    /// Same as JsonRpcResponder but with an artificial delay.
    struct DelayedJsonRpcResponder {
        result: serde_json::Value,
        delay: std::time::Duration,
    }

    impl wiremock::Respond for DelayedJsonRpcResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let id = body.get("id").cloned().unwrap_or(serde_json::Value::Null);
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": self.result,
                }))
                .set_delay(self.delay)
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

    fn logs_json() -> serde_json::Value {
        serde_json::json!([{
            "address": "0x0000000000000000000000000000000000000001",
            "topics": ["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"],
            "data": "0x",
            "blockNumber": "0x3e8",
            "transactionHash": "0x3333333333333333333333333333333333333333333333333333333333333333",
            "transactionIndex": "0x0",
            "blockHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
            "logIndex": "0x0",
            "removed": false
        }])
    }

    async fn create_test_client(server: &MockServer) -> RpcClient {
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
    async fn get_logs_returns_correct_decoded_logs() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getLogs"))
            .respond_with(JsonRpcResponder {
                result: logs_json(),
            })
            .mount(&server)
            .await;

        let client = create_test_client(&server).await;
        let filter = LogFilter {
            address: vec![],
            topics: vec![],
            from_block: 0,
            to_block: 1000,
        };

        let logs = client.get_logs(filter).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].block_number, 1000);
        assert_eq!(logs[0].log_index, 0);
    }

    #[tokio::test]
    async fn get_block_by_number_returns_correct_block() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(JsonRpcResponder {
                result: block_json(),
            })
            .mount(&server)
            .await;

        let client = create_test_client(&server).await;
        let block = client.get_block_by_number(1000).await.unwrap();

        assert_eq!(block.number, 1000);
        assert_eq!(block.chain_id, 1);
        assert_eq!(block.gas_limit, 30_000_000);
        assert_eq!(block.base_fee_per_gas, Some(1_000_000_000));
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        tokio::time::pause();
        let server = MockServer::start().await;

        // Mount success response first (lower priority)
        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(JsonRpcResponder {
                result: block_json(),
            })
            .mount(&server)
            .await;

        // Mount 500 response second (higher priority, limited to 2)
        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .up_to_n_times(2)
            .mount(&server)
            .await;

        let client = create_test_client(&server).await;
        let result = client.get_block_by_number(1000).await;
        assert!(
            result.is_ok(),
            "Expected success after retries: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn retry_exhausted_returns_max_retries_exceeded() {
        tokio::time::pause();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let client = create_test_client(&server).await;
        let result = client.get_block_by_number(1000).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, RpcError::MaxRetriesExceeded { .. }),
            "Expected MaxRetriesExceeded, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn dedup_sends_exactly_one_request_for_concurrent_calls() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(DelayedJsonRpcResponder {
                result: block_json(),
                delay: std::time::Duration::from_millis(200),
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = Arc::new(create_test_client(&server).await);
        let mut handles = Vec::new();

        for _ in 0..5 {
            let c = client.clone();
            handles.push(tokio::spawn(
                async move { c.get_block_by_number(1000).await },
            ));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Task failed: {:?}", result);
        }

        // MockServer::drop checks expect(1) assertion
    }

    #[tokio::test]
    async fn subscribe_new_heads_returns_no_websocket_on_http_client() {
        let server = MockServer::start().await;
        let client = create_test_client(&server).await;

        let result = client.subscribe_new_heads().await;
        match result {
            Err(RpcError::NoWebSocket { chain_id }) => {
                assert_eq!(chain_id, 1);
            }
            Err(other) => panic!("Expected NoWebSocket, got: {:?}", other),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }
}
