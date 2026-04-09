//! NewBlockSubscriber — WebSocket new block stream with reconnection.

use std::sync::Arc;
use std::time::Duration;

use forge_index_core::types::Block;
use forge_index_rpc::CachedRpcClient;
use futures::{Stream, StreamExt};

use crate::error::SyncError;

/// Maximum number of reconnection attempts.
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// Wraps the WebSocket stream from `CachedRpcClient` with reconnection logic.
pub struct NewBlockSubscriber {
    /// The RPC client.
    client: Arc<CachedRpcClient>,
    /// The chain ID.
    chain_id: u64,
}

impl NewBlockSubscriber {
    /// Creates a new block subscriber.
    pub fn new(client: Arc<CachedRpcClient>, chain_id: u64) -> Self {
        Self { client, chain_id }
    }

    /// Subscribes to new block headers.
    ///
    /// Returns a stream of blocks. On disconnect, attempts to reconnect
    /// with exponential backoff up to `MAX_RECONNECT_ATTEMPTS` times.
    pub async fn subscribe(
        &self,
    ) -> Result<impl Stream<Item = Result<Block, SyncError>> + '_, SyncError> {
        let stream = self.connect().await?;
        let chain_id = self.chain_id;

        Ok(stream.map(move |block| {
            tracing::debug!(
                chain_id = chain_id,
                block_number = block.number,
                "Realtime sync: new block #{} on chain {}",
                block.number,
                chain_id
            );
            Ok(block)
        }))
    }

    /// Establishes the WebSocket connection, retrying on failure.
    async fn connect(&self) -> Result<futures::stream::BoxStream<'static, Block>, SyncError> {
        let mut delay = Duration::from_secs(1);

        for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
            match self.client.subscribe_new_heads().await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if attempt == MAX_RECONNECT_ATTEMPTS {
                        return Err(SyncError::SubscriptionLost {
                            chain_id: self.chain_id,
                            message: format!(
                                "Failed after {} attempts: {}",
                                MAX_RECONNECT_ATTEMPTS, e
                            ),
                        });
                    }
                    tracing::warn!(
                        chain_id = self.chain_id,
                        attempt = attempt,
                        error = %e,
                        "WebSocket connection failed, retrying in {:?}",
                        delay
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }

        unreachable!()
    }

    /// Returns the chain ID.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}
