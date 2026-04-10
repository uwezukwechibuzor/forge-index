//! Reusable wiremock setup for mocking EVM JSON-RPC endpoints.

use forge_index_core::types::{Block, Log};
use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::fixtures;

/// Wrapper around a wiremock MockServer preconfigured for JSON-RPC.
pub struct MockRpc {
    pub server: MockServer,
}

/// Responder that echoes back the JSON-RPC request ID.
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

impl MockRpc {
    /// Creates a new MockRpc with a fresh wiremock server.
    pub async fn new() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    /// Returns the mock server's URL.
    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Mocks eth_blockNumber returning the given block number.
    pub async fn mock_block_number(&self, n: u64) {
        Mock::given(method("POST"))
            .and(body_string_contains("eth_blockNumber"))
            .respond_with(JsonRpcResponder {
                result: serde_json::json!(format!("0x{:x}", n)),
            })
            .mount(&self.server)
            .await;
    }

    /// Mocks eth_getBlockByNumber for a specific block.
    pub async fn mock_block(&self, block: &Block) {
        let block_json = fixtures::block_json(block.number);
        Mock::given(method("POST"))
            .and(body_string_contains("eth_getBlockByNumber"))
            .respond_with(JsonRpcResponder {
                result: block_json,
            })
            .mount(&self.server)
            .await;
    }

    /// Mocks eth_getLogs returning the given logs.
    pub async fn mock_logs(&self, logs: &[Log]) {
        let logs_json = fixtures::logs_json(logs);
        Mock::given(method("POST"))
            .and(body_string_contains("eth_getLogs"))
            .respond_with(JsonRpcResponder {
                result: logs_json,
            })
            .mount(&self.server)
            .await;
    }

    /// Mocks eth_getLogs returning empty results.
    pub async fn mock_empty_logs(&self) {
        Mock::given(method("POST"))
            .and(body_string_contains("eth_getLogs"))
            .respond_with(JsonRpcResponder {
                result: serde_json::json!([]),
            })
            .mount(&self.server)
            .await;
    }

    /// Returns how many requests the mock server has received.
    pub async fn request_count(&self) -> usize {
        self.server.received_requests().await.unwrap_or_default().len()
    }
}
