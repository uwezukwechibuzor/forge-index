//! Backfill progress tracking and reporting.

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::Instant;

/// Per-chain progress state.
pub struct ChainProgress {
    /// Total blocks to process.
    pub total_blocks: AtomicU64,
    /// Blocks processed so far.
    pub processed_blocks: AtomicU64,
    /// Events processed so far.
    pub events_processed: AtomicU64,
    /// When the backfill started.
    pub started_at: Instant,
}

impl ChainProgress {
    /// Creates a new progress tracker.
    pub fn new(total_blocks: u64) -> Self {
        Self {
            total_blocks: AtomicU64::new(total_blocks),
            processed_blocks: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    /// Returns the percentage of blocks processed.
    pub fn percent_complete(&self) -> f32 {
        let total = self.total_blocks.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        let processed = self.processed_blocks.load(Ordering::Relaxed);
        (processed as f32 / total as f32) * 100.0
    }

    /// Returns the average blocks processed per second.
    pub fn blocks_per_second(&self) -> f64 {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }
        self.processed_blocks.load(Ordering::Relaxed) as f64 / elapsed
    }

    /// Returns the estimated time remaining in seconds.
    pub fn eta_seconds(&self) -> Option<f64> {
        let bps = self.blocks_per_second();
        if bps < 0.001 {
            return None;
        }
        let remaining = self.total_blocks.load(Ordering::Relaxed) as f64
            - self.processed_blocks.load(Ordering::Relaxed) as f64;
        Some(remaining / bps)
    }

    /// Records progress for a processed block range.
    pub fn record(&self, blocks: u64, events: u64) {
        self.processed_blocks.fetch_add(blocks, Ordering::Relaxed);
        self.events_processed.fetch_add(events, Ordering::Relaxed);
    }
}

/// Tracks backfill progress across all chains.
pub struct BackfillProgress {
    per_chain: DashMap<u64, ChainProgress>,
}

impl BackfillProgress {
    /// Creates a new progress tracker.
    pub fn new() -> Self {
        Self {
            per_chain: DashMap::new(),
        }
    }

    /// Initializes progress tracking for a chain.
    pub fn init_chain(&self, chain_id: u64, total_blocks: u64) {
        self.per_chain
            .insert(chain_id, ChainProgress::new(total_blocks));
    }

    /// Records progress for a chain.
    pub fn record(&self, chain_id: u64, blocks: u64, events: u64) {
        if let Some(progress) = self.per_chain.get(&chain_id) {
            progress.record(blocks, events);
        }
    }

    /// Logs the current status of all chains at INFO level.
    pub fn log_status(&self) {
        for entry in self.per_chain.iter() {
            let chain_id = *entry.key();
            let p = entry.value();
            let processed = p.processed_blocks.load(Ordering::Relaxed);
            let total = p.total_blocks.load(Ordering::Relaxed);
            let events = p.events_processed.load(Ordering::Relaxed);
            let bps = p.blocks_per_second();
            let pct = p.percent_complete();

            let eta_str = match p.eta_seconds() {
                Some(secs) if secs > 60.0 => {
                    format!("{}m {}s", secs as u64 / 60, secs as u64 % 60)
                }
                Some(secs) => format!("{}s", secs as u64),
                None => "unknown".to_string(),
            };

            tracing::info!(
                chain_id = chain_id,
                "[chain {}] Backfill {:.1}% complete — {}/{} blocks, {} events, {:.0} blocks/s, ETA {}",
                chain_id,
                pct,
                processed,
                total,
                events,
                bps,
                eta_str
            );
        }
    }

    /// Returns the progress for a specific chain.
    pub fn get_chain(
        &self,
        chain_id: u64,
    ) -> Option<dashmap::mapref::one::Ref<'_, u64, ChainProgress>> {
        self.per_chain.get(&chain_id)
    }
}

impl Default for BackfillProgress {
    fn default() -> Self {
        Self::new()
    }
}
