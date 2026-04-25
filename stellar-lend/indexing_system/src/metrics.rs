/// Indexing throughput metrics and backlog alerting.
///
/// All counters use atomic operations so they can be updated from multiple
/// worker tasks without a mutex.
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::warn;

/// Atomic metrics store shared across all workers.
///
/// `AtomicU64` is `Send + Sync` by definition, so no unsafe impls are needed.
pub struct IndexingMetrics {
    /// Total events indexed since startup.
    pub total_events_indexed: AtomicU64,

    /// Total block-range batches processed.
    pub total_batches_processed: AtomicU64,

    /// Total processing time across all batches (milliseconds).
    pub total_processing_ms: AtomicU64,

    /// Number of permanent worker errors.
    pub total_errors: AtomicU64,

    /// Number of chain reorganisations handled.
    pub total_reorgs: AtomicU64,

    /// Current backlog in blocks (updated each dispatcher cycle).
    pub current_backlog_blocks: AtomicU64,

    /// Threshold above which a backlog alert is emitted.
    backlog_alert_threshold: u64,
}

/// Point-in-time snapshot of all metrics (non-atomic copy for reporting).
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_events_indexed: u64,
    pub total_batches_processed: u64,
    /// Average milliseconds per batch (0 if no batches yet).
    pub avg_batch_ms: u64,
    /// Events per second averaged over all batches (0 if no data).
    pub events_per_second: f64,
    pub total_errors: u64,
    pub total_reorgs: u64,
    pub current_backlog_blocks: u64,
    pub backlog_alert_active: bool,
}

impl IndexingMetrics {
    /// Create a new metrics instance.
    ///
    /// `backlog_alert_threshold` – number of blocks behind tip that triggers
    /// a warning log.
    pub fn new(backlog_alert_threshold: u64) -> Self {
        Self {
            total_events_indexed: AtomicU64::new(0),
            total_batches_processed: AtomicU64::new(0),
            total_processing_ms: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            total_reorgs: AtomicU64::new(0),
            current_backlog_blocks: AtomicU64::new(0),
            backlog_alert_threshold,
        }
    }

    /// Record the completion of one batch.
    ///
    /// * `event_count` – number of events found in this batch.
    /// * `elapsed_ms`  – wall-clock time the worker spent on this batch.
    pub fn record_batch(&self, event_count: u64, elapsed_ms: u64) {
        self.total_events_indexed
            .fetch_add(event_count, Ordering::Relaxed);
        self.total_batches_processed
            .fetch_add(1, Ordering::Relaxed);
        self.total_processing_ms
            .fetch_add(elapsed_ms, Ordering::Relaxed);
    }

    /// Record a permanent worker error.
    pub fn record_error(&self) {
        self.total_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a reorg event.
    pub fn record_reorg(&self) {
        self.total_reorgs.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the current backlog and emit an alert if it exceeds the threshold.
    pub fn update_backlog(&self, backlog_blocks: u64) {
        self.current_backlog_blocks
            .store(backlog_blocks, Ordering::Relaxed);

        if backlog_blocks > self.backlog_alert_threshold {
            warn!(
                backlog_blocks = backlog_blocks,
                threshold = self.backlog_alert_threshold,
                "Indexing backlog exceeds alert threshold – indexer is falling behind"
            );
        }
    }

    /// Take a consistent snapshot of all metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let batches = self.total_batches_processed.load(Ordering::Relaxed);
        let total_ms = self.total_processing_ms.load(Ordering::Relaxed);
        let total_events = self.total_events_indexed.load(Ordering::Relaxed);
        let backlog = self.current_backlog_blocks.load(Ordering::Relaxed);

        let avg_batch_ms = if batches > 0 { total_ms / batches } else { 0 };

        let events_per_second = if total_ms > 0 {
            (total_events as f64) / (total_ms as f64 / 1_000.0)
        } else {
            0.0
        };

        MetricsSnapshot {
            total_events_indexed: total_events,
            total_batches_processed: batches,
            avg_batch_ms,
            events_per_second,
            total_errors: self.total_errors.load(Ordering::Relaxed),
            total_reorgs: self.total_reorgs.load(Ordering::Relaxed),
            current_backlog_blocks: backlog,
            backlog_alert_active: backlog > self.backlog_alert_threshold,
        }
    }
}
