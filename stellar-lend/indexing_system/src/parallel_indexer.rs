/// Parallel block processing pipeline for high-throughput indexing.
///
/// Architecture overview:
///
/// ```text
///  ┌─────────────────────────────────────────────────────────┐
///  │                    ParallelIndexer                       │
///  │                                                          │
///  │  Dispatcher ──► [Worker 0] ──► BlockProcessor           │
///  │               ──► [Worker 1] ──► BlockProcessor         │
///  │               ──► [Worker N] ──► BlockProcessor         │
///  │                        │                                 │
///  │                   result_tx ──► Commit loop             │
///  │                                   │                      │
///  │                              StateManager               │
///  │                              (ordering + reorg)         │
///  └─────────────────────────────────────────────────────────┘
/// ```
///
/// Key guarantees:
/// - Blocks are committed to the DB in ascending order regardless of which
///   worker finishes first (ordering guarantee via StateManager).
/// - A reorg detected by any worker pauses all workers via a `watch` channel,
///   rolls back the affected range, and re-queues the blocks.
/// - Metrics are updated atomically so dashboards always see consistent
///   throughput numbers.
use crate::cache::CacheService;
use crate::config::Config;
use crate::error::{IndexerError, IndexerResult};
use crate::metrics::{IndexingMetrics, MetricsSnapshot};
use crate::models::{CreateEvent, EventUpdate, UpdateType};
use crate::parser::EventParser;
use crate::repository::EventRepository;
use ethers::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock, Semaphore};
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Public message types
// ─────────────────────────────────────────────────────────────────────────────

/// A unit of work dispatched to a worker.
#[derive(Debug, Clone)]
pub struct BlockRangeTask {
    /// Contract being indexed.
    pub contract_address: String,
    /// Inclusive start block.
    pub from_block: u64,
    /// Inclusive end block.
    pub to_block: u64,
    /// Attempt number (0-based) for retry tracking.
    pub attempt: u32,
}

/// Result produced by a worker after processing a task.
#[derive(Debug)]
pub struct BlockRangeResult {
    pub task: BlockRangeTask,
    pub events: Vec<CreateEvent>,
    /// Wall-clock time the worker spent on this task (ms).
    pub elapsed_ms: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// StateManager
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks in-flight tasks and enforces ordered DB commits.
///
/// Workers finish in arbitrary order; the StateManager buffers results and
/// only flushes a contiguous prefix to the database so that
/// `last_indexed_block` always advances monotonically.
///
/// # Locking discipline
/// `register_result` acquires both `commit_frontier` and `pending` under a
/// single combined lock scope so the insert-then-drain is atomic — no TOCTOU
/// race between two concurrent callers.
pub struct StateManager {
    /// Lowest block that has NOT yet been committed for each contract.
    /// Invariant: value == last_committed_block + 1.
    pub(crate) commit_frontier: Arc<Mutex<std::collections::HashMap<String, u64>>>,

    /// Completed results waiting to be committed, keyed by (contract, from_block).
    pub(crate) pending: Arc<Mutex<BTreeMap<(String, u64), BlockRangeResult>>>,

    /// Set when a reorg is detected; workers check this before processing.
    reorg_signal: Arc<watch::Sender<Option<u64>>>,

    /// Receiver side – cloned into every worker.
    reorg_rx: watch::Receiver<Option<u64>>,
}

impl StateManager {
    pub fn new(initial_frontiers: Vec<(String, u64)>) -> Self {
        let mut frontier_map = std::collections::HashMap::new();
        for (addr, block) in initial_frontiers {
            frontier_map.insert(addr, block);
        }

        let (tx, rx) = watch::channel(None);

        Self {
            commit_frontier: Arc::new(Mutex::new(frontier_map)),
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            reorg_signal: Arc::new(tx),
            reorg_rx: rx,
        }
    }

    /// Register a completed result and return the contiguous slice that is
    /// now safe to commit (in ascending block order).
    ///
    /// Both the insert and the drain happen under the same pair of locks so
    /// two concurrent callers cannot race on the same contract's frontier.
    pub async fn register_result(&self, result: BlockRangeResult) -> Vec<BlockRangeResult> {
        let contract = result.task.contract_address.clone();
        let from = result.task.from_block;

        // Hold both locks for the entire insert + drain to prevent TOCTOU.
        let mut frontier_map = self.commit_frontier.lock().await;
        let mut pending = self.pending.lock().await;

        pending.insert((contract.clone(), from), result);

        // Drain the contiguous prefix starting at the current frontier.
        let frontier = match frontier_map.get(&contract) {
            Some(f) => *f,
            None => return vec![],
        };

        let mut ready = Vec::new();
        let mut next = frontier;

        loop {
            let key = (contract.clone(), next);
            if let Some(r) = pending.remove(&key) {
                next = r.task.to_block + 1;
                ready.push(r);
            } else {
                break;
            }
        }

        if next != frontier {
            frontier_map.insert(contract, next);
        }

        ready
    }

    /// Signal all workers that a reorg occurred at `block`.
    pub async fn signal_reorg(&self, block: u64) {
        let _ = self.reorg_signal.send(Some(block));
    }

    /// Clear the reorg signal after recovery is complete.
    pub async fn clear_reorg(&self) {
        let _ = self.reorg_signal.send(None);
    }

    /// Subscribe to reorg notifications.
    pub fn reorg_receiver(&self) -> watch::Receiver<Option<u64>> {
        self.reorg_rx.clone()
    }

    /// Roll back the commit frontier for all contracts to `reorg_block` and
    /// discard any pending results that overlap the reorg range.
    pub async fn rollback_to(&self, reorg_block: u64) {
        let mut frontier_map = self.commit_frontier.lock().await;
        for frontier in frontier_map.values_mut() {
            if *frontier > reorg_block {
                *frontier = reorg_block;
            }
        }

        let mut pending = self.pending.lock().await;
        pending.retain(|(_, from_block), _| *from_block < reorg_block);
    }

    /// Snapshot the current commit frontiers (for metrics / health checks).
    pub async fn frontiers(&self) -> std::collections::HashMap<String, u64> {
        self.commit_frontier.lock().await.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BlockProcessor
// ─────────────────────────────────────────────────────────────────────────────

/// Stateless, cloneable unit that fetches and parses a single block range.
/// Multiple instances run concurrently inside worker tasks.
#[derive(Clone)]
pub struct BlockProcessor {
    provider: Arc<Provider<Ws>>,
    parser: Arc<RwLock<EventParser>>,
}

impl BlockProcessor {
    pub fn new(provider: Arc<Provider<Ws>>, parser: Arc<RwLock<EventParser>>) -> Self {
        Self { provider, parser }
    }

    /// Fetch and parse all events for the task's contract in `[from, to]`.
    pub async fn process(&self, task: &BlockRangeTask) -> IndexerResult<Vec<CreateEvent>> {
        let address: Address = task
            .contract_address
            .parse()
            .map_err(|e| IndexerError::EventParsing(format!("Invalid address: {}", e)))?;

        let filter = Filter::new()
            .address(address)
            .from_block(task.from_block)
            .to_block(task.to_block);

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .map_err(|e| IndexerError::Rpc(format!("Failed to fetch logs: {}", e)))?;

        if logs.is_empty() {
            return Ok(vec![]);
        }

        let parser = self.parser.read().await;
        let mut events = Vec::with_capacity(logs.len());

        for log in &logs {
            if let Some(event) = parser.parse_log(log)? {
                events.push(event);
            }
        }

        Ok(events)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParallelIndexer
// ─────────────────────────────────────────────────────────────────────────────

/// High-throughput indexer that processes block ranges in parallel.
///
/// `start()` is designed to be called inside a `tokio::spawn` so that
/// `stop()` can be called concurrently from another task.
pub struct ParallelIndexer {
    provider: Arc<Provider<Ws>>,
    parser: Arc<RwLock<EventParser>>,
    repository: EventRepository,
    cache: Arc<RwLock<CacheService>>,
    config: Config,
    state_manager: Arc<StateManager>,
    metrics: Arc<IndexingMetrics>,
    is_running: Arc<RwLock<bool>>,
}

impl ParallelIndexer {
    /// Create a new parallel indexer.
    pub async fn new(
        config: Config,
        repository: EventRepository,
        cache: CacheService,
    ) -> IndexerResult<Self> {
        let provider = Provider::<Ws>::connect(&config.blockchain.ws_url)
            .await
            .map_err(|e| {
                IndexerError::Rpc(format!("Failed to connect to blockchain: {}", e))
            })?;

        info!(
            "ParallelIndexer connected to blockchain at {}",
            config.blockchain.ws_url
        );

        // Load existing frontiers from DB so we resume correctly after restart.
        let metadata_list = repository.get_active_metadata().await?;
        let frontiers: Vec<(String, u64)> = metadata_list
            .iter()
            .map(|m| (m.contract_address.clone(), (m.last_indexed_block + 1) as u64))
            .collect();

        let state_manager = Arc::new(StateManager::new(frontiers));
        let metrics = Arc::new(IndexingMetrics::new(config.indexer.backlog_alert_threshold));

        Ok(Self {
            provider: Arc::new(provider),
            parser: Arc::new(RwLock::new(EventParser::new())),
            repository,
            cache: Arc::new(RwLock::new(cache)),
            config,
            state_manager,
            metrics,
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// Register a contract for parallel indexing.
    pub async fn register_contract(
        &self,
        contract_address: &str,
        abi_json: &str,
        start_block: u64,
    ) -> IndexerResult<()> {
        let mut parser = self.parser.write().await;
        parser.register_contract(contract_address, abi_json)?;
        drop(parser);

        self.repository
            .get_or_create_metadata(contract_address, start_block)
            .await?;

        // Ensure the state manager knows about this contract.
        let mut frontier_map = self.state_manager.commit_frontier.lock().await;
        frontier_map
            .entry(contract_address.to_string())
            .or_insert(start_block);
        drop(frontier_map);

        info!(
            "Registered contract {} for parallel indexing from block {}",
            contract_address, start_block
        );
        Ok(())
    }

    /// Start the parallel indexing pipeline.
    ///
    /// Spawns `config.indexer.worker_count` worker tasks that pull from a
    /// shared task channel.  The dispatcher loop fills the channel; workers
    /// drain it concurrently.
    ///
    /// This method blocks until `stop()` is called from another task.
    pub async fn start(&self) -> IndexerResult<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!("ParallelIndexer is already running");
                return Ok(());
            }
            *running = true;
        }

        info!(
            "Starting ParallelIndexer with {} workers",
            self.config.indexer.worker_count
        );

        let worker_count = self.config.indexer.worker_count;

        // task_tx: dispatcher → workers (bounded for back-pressure).
        let (task_tx, task_rx) = mpsc::channel::<BlockRangeTask>(worker_count * 2);
        let task_rx = Arc::new(Mutex::new(task_rx));

        // result_tx: workers → commit loop.
        let (result_tx, mut result_rx) =
            mpsc::channel::<IndexerResult<BlockRangeResult>>(worker_count * 4);

        // Semaphore caps concurrent in-flight RPC calls (one permit per worker).
        // The semaphore is acquired *inside* the worker after receiving a task,
        // so it gates the RPC call — not the channel receive — avoiding deadlock.
        let semaphore = Arc::new(Semaphore::new(worker_count));

        // ── Spawn workers ────────────────────────────────────────────────────
        for worker_id in 0..worker_count {
            let task_rx = Arc::clone(&task_rx);
            let result_tx = result_tx.clone();
            let processor = BlockProcessor::new(
                Arc::clone(&self.provider),
                Arc::clone(&self.parser),
            );
            let sem = Arc::clone(&semaphore);
            let max_retries = self.config.indexer.max_retries;
            let retry_delay_ms = self.config.indexer.retry_delay_ms;
            let mut reorg_rx = self.state_manager.reorg_receiver();

            tokio::spawn(async move {
                loop {
                    // Receive a task first (no permit held yet).
                    let task = {
                        let mut rx = task_rx.lock().await;
                        rx.recv().await
                    };

                    let task = match task {
                        Some(t) => t,
                        None => {
                            debug!("Worker {} channel closed, exiting", worker_id);
                            break;
                        }
                    };

                    // Check for reorg before doing any work.
                    if reorg_rx.borrow_and_update().is_some() {
                        debug!("Worker {} skipping task due to reorg signal", worker_id);
                        // Drop the task; the dispatcher will re-queue after recovery.
                        continue;
                    }

                    // Acquire a semaphore permit to cap concurrent RPC calls.
                    let _permit = sem.acquire().await.expect("semaphore closed");

                    let start = Instant::now();
                    let mut last_err: Option<IndexerError> = None;

                    // Retry loop with exponential back-off.
                    for attempt in 0..=max_retries {
                        if attempt > 0 {
                            let delay = retry_delay_ms * (2u64.pow(attempt - 1));
                            sleep(Duration::from_millis(delay)).await;
                        }

                        match processor.process(&task).await {
                            Ok(events) => {
                                let elapsed_ms = start.elapsed().as_millis() as u64;
                                let _ = result_tx
                                    .send(Ok(BlockRangeResult {
                                        task: task.clone(),
                                        events,
                                        elapsed_ms,
                                    }))
                                    .await;
                                last_err = None;
                                break;
                            }
                            Err(e) => {
                                warn!(
                                    "Worker {} attempt {}/{} failed for {}-{}: {}",
                                    worker_id,
                                    attempt + 1,
                                    max_retries + 1,
                                    task.from_block,
                                    task.to_block,
                                    e
                                );
                                last_err = Some(e);
                            }
                        }
                    }

                    if let Some(e) = last_err {
                        error!(
                            "Worker {} permanently failed for blocks {}-{}: {}",
                            worker_id, task.from_block, task.to_block, e
                        );
                        let _ = result_tx.send(Err(e)).await;
                    }
                }
            });
        }

        // Drop the extra result_tx clone so the commit loop exits when all
        // workers have finished.
        drop(result_tx);

        // ── Commit loop ──────────────────────────────────────────────────────
        // Spawned as a separate task so the dispatcher loop can run concurrently.
        let repository = self.repository.clone();
        let cache = Arc::clone(&self.cache);
        let state_manager = Arc::clone(&self.state_manager);
        let metrics = Arc::clone(&self.metrics);
        let enable_realtime = self.config.indexer.enable_realtime;

        let commit_handle = tokio::spawn(async move {
            while let Some(result) = result_rx.recv().await {
                match result {
                    Err(e) => {
                        error!("Worker reported permanent failure: {}", e);
                        metrics.record_error();
                    }
                    Ok(block_result) => {
                        let event_count = block_result.events.len();
                        let elapsed_ms = block_result.elapsed_ms;

                        // Hand off to StateManager; get back the ordered slice
                        // that is safe to commit.
                        let ready = state_manager.register_result(block_result).await;

                        for batch in ready {
                            if batch.events.is_empty() {
                                // No events but still advance the metadata pointer.
                                if let Err(e) = repository
                                    .update_metadata(
                                        &batch.task.contract_address,
                                        batch.task.to_block,
                                    )
                                    .await
                                {
                                    error!("Failed to update metadata: {}", e);
                                }
                                continue;
                            }

                            match repository.create_events_batch(batch.events.clone()).await {
                                Ok(inserted) => {
                                    if let Err(e) = repository
                                        .update_metadata(
                                            &batch.task.contract_address,
                                            batch.task.to_block,
                                        )
                                        .await
                                    {
                                        error!("Failed to update metadata: {}", e);
                                    }

                                    // Invalidate caches.
                                    let mut c = cache.write().await;
                                    let _ = c.invalidate_queries().await;
                                    let _ = c.invalidate_stats().await;
                                    let _ = c.set_latest_block(batch.task.to_block).await;

                                    // Publish real-time updates.
                                    if enable_realtime {
                                        for event in &batch.events {
                                            let update = EventUpdate {
                                                update_type: UpdateType::New,
                                                event: crate::models::Event {
                                                    id: uuid::Uuid::new_v4(),
                                                    contract_address: event
                                                        .contract_address
                                                        .clone(),
                                                    event_name: event.event_name.clone(),
                                                    block_number: event.block_number as i64,
                                                    transaction_hash: event
                                                        .transaction_hash
                                                        .clone(),
                                                    log_index: event.log_index as i32,
                                                    event_data: event.event_data.clone(),
                                                    indexed_at: chrono::Utc::now(),
                                                    created_at: chrono::Utc::now(),
                                                },
                                                timestamp: chrono::Utc::now(),
                                            };
                                            let _ = c.publish("events:new", &update).await;
                                        }
                                    }

                                    info!(
                                        "Committed {} events for {} blocks {}-{}",
                                        inserted,
                                        batch.task.contract_address,
                                        batch.task.from_block,
                                        batch.task.to_block,
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "DB batch insert failed for {}-{}: {}",
                                        batch.task.from_block, batch.task.to_block, e
                                    );
                                    metrics.record_error();
                                }
                            }
                        }

                        metrics.record_batch(event_count as u64, elapsed_ms);
                    }
                }
            }
        });

        // ── Dispatcher loop ──────────────────────────────────────────────────
        // Runs in the current task; polls for new blocks and enqueues tasks.
        loop {
            if !*self.is_running.read().await {
                info!("ParallelIndexer stopping");
                break;
            }

            let metadata_list = match self.repository.get_active_metadata().await {
                Ok(list) => list,
                Err(e) => {
                    error!("Failed to fetch metadata: {}", e);
                    sleep(Duration::from_secs(self.config.indexer.poll_interval)).await;
                    continue;
                }
            };

            if metadata_list.is_empty() {
                debug!("No active contracts to index");
                sleep(Duration::from_secs(self.config.indexer.poll_interval)).await;
                continue;
            }

            let current_block = match self.get_current_block().await {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to get current block: {}", e);
                    sleep(Duration::from_secs(self.config.indexer.poll_interval)).await;
                    continue;
                }
            };

            let confirmed_tip =
                current_block.saturating_sub(self.config.indexer.confirmations);

            // Compute total backlog for alerting.
            let total_backlog: u64 = metadata_list
                .iter()
                .map(|m| {
                    let from = (m.last_indexed_block + 1) as u64;
                    if from <= confirmed_tip {
                        confirmed_tip - from + 1
                    } else {
                        0
                    }
                })
                .sum();

            self.metrics.update_backlog(total_backlog);

            // Dispatch tasks for each contract.
            for metadata in &metadata_list {
                let from_block = (metadata.last_indexed_block + 1) as u64;

                if from_block > confirmed_tip {
                    continue;
                }

                let mut batch_start = from_block;
                while batch_start <= confirmed_tip {
                    let batch_end = std::cmp::min(
                        batch_start + self.config.indexer.batch_size - 1,
                        confirmed_tip,
                    );

                    let task = BlockRangeTask {
                        contract_address: metadata.contract_address.clone(),
                        from_block: batch_start,
                        to_block: batch_end,
                        attempt: 0,
                    };

                    if task_tx.send(task).await.is_err() {
                        error!("Task channel closed unexpectedly");
                        break;
                    }

                    batch_start = batch_end + 1;
                }
            }

            sleep(Duration::from_secs(self.config.indexer.poll_interval)).await;
        }

        // Drop the task sender — workers will drain remaining tasks then exit.
        drop(task_tx);

        // Wait for the commit loop to drain all in-flight results.
        let _ = commit_handle.await;

        Ok(())
    }

    /// Stop the indexer gracefully.
    ///
    /// Call this from a separate task while `start()` is running.
    pub async fn stop(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
        info!("ParallelIndexer stop requested");
    }

    /// Handle a chain reorganization.
    ///
    /// Steps:
    /// 1. Signal all workers to skip in-flight tasks.
    /// 2. Roll back the StateManager's frontiers.
    /// 3. Delete DB events from `reorg_block` onwards.
    /// 4. Reset metadata pointers.
    /// 5. Invalidate caches.
    /// 6. Clear the reorg signal so workers resume.
    pub async fn handle_reorg(&self, reorg_block: u64) -> IndexerResult<()> {
        warn!("Handling reorg from block {} across all workers", reorg_block);

        // 1. Signal workers.
        self.state_manager.signal_reorg(reorg_block).await;

        // 2. Roll back state manager.
        self.state_manager.rollback_to(reorg_block).await;

        // 3. Delete DB events.
        let deleted = self
            .repository
            .delete_events_from_block(reorg_block)
            .await?;
        info!("Deleted {} events due to reorg at block {}", deleted, reorg_block);

        // 4. Reset metadata pointers.
        let metadata_list = self.repository.get_active_metadata().await?;
        for m in metadata_list {
            if m.last_indexed_block >= reorg_block as i64 {
                self.repository
                    .update_metadata(&m.contract_address, reorg_block.saturating_sub(1))
                    .await?;
            }
        }

        // 5. Invalidate caches.
        let mut cache = self.cache.write().await;
        cache.invalidate_queries().await?;
        cache.invalidate_stats().await?;
        drop(cache);

        // 6. Clear signal so workers resume.
        self.state_manager.clear_reorg().await;

        self.metrics.record_reorg();

        Ok(())
    }

    /// Expose a snapshot of current metrics.
    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Whether the indexer is currently running.
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    async fn get_current_block(&self) -> IndexerResult<u64> {
        self.provider
            .get_block_number()
            .await
            .map(|n| n.as_u64())
            .map_err(|e| IndexerError::Rpc(format!("Failed to get block number: {}", e)))
    }
}
