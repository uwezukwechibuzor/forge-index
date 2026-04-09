//! In-flight request deduplication.

use crate::error::RpcError;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Result type used internally for deduplication.
pub(crate) type DedupResult = Result<serde_json::Value, RpcError>;

const DEDUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Deduplicates concurrent identical RPC requests.
///
/// If two tasks call the same RPC method with the same parameters simultaneously,
/// only one actual request is made. The second task waits and receives the same result.
pub struct RequestDedup {
    in_flight: DashMap<String, (broadcast::Sender<DedupResult>, Instant)>,
}

impl RequestDedup {
    /// Creates a new deduplication layer.
    pub fn new() -> Self {
        Self {
            in_flight: DashMap::new(),
        }
    }

    /// Executes `f` with deduplication.
    ///
    /// If another call with the same key is already in flight, waits for its result.
    /// If the in-flight request has been pending for more than 30s, it is evicted
    /// and a fresh request is made.
    pub async fn dedup<F, Fut>(&self, key: String, f: F) -> DedupResult
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = DedupResult>,
    {
        let mut f = Some(f);

        loop {
            match self.in_flight.entry(key.clone()) {
                Entry::Occupied(entry) => {
                    let (sender, created_at) = entry.get();
                    if created_at.elapsed() >= DEDUP_TIMEOUT {
                        let entry_ref = entry.into_ref();
                        let key = entry_ref.key().clone();
                        drop(entry_ref);
                        self.in_flight.remove(&key);
                        continue;
                    }
                    let mut rx = sender.subscribe();
                    drop(entry);
                    match rx.recv().await {
                        Ok(result) => return result,
                        Err(_) => continue,
                    }
                }
                Entry::Vacant(entry) => {
                    let (tx, _) = broadcast::channel(64);
                    let tx_clone = tx.clone();
                    let ref_mut = entry.insert((tx, Instant::now()));
                    drop(ref_mut);

                    let closure = f.take().expect("dedup closure already consumed");
                    let result = closure().await;

                    let _ = tx_clone.send(result.clone());
                    self.in_flight.remove(&key);

                    return result;
                }
            }
        }
    }
}

impl Default for RequestDedup {
    fn default() -> Self {
        Self::new()
    }
}
