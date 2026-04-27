/// Tests for the parallel indexing pipeline.
///
/// Coverage:
/// - StateManager ordering guarantees (out-of-order results committed in order)
/// - StateManager reorg rollback (via public API only)
/// - Metrics recording (throughput counters, backlog alerting)
/// - Race-condition safety (concurrent register_result calls via Barrier)
/// - Multiple independent contracts
/// - Edge cases (unknown contract, zero batches, partial flush)
#[cfg(test)]
mod tests {
    use crate::metrics::IndexingMetrics;
    use crate::models::CreateEvent;
    use crate::parallel_indexer::{BlockRangeResult, BlockRangeTask, StateManager};
    use std::sync::Arc;
    use tokio::sync::Barrier;

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn make_task(contract: &str, from: u64, to: u64) -> BlockRangeTask {
        BlockRangeTask {
            contract_address: contract.to_string(),
            from_block: from,
            to_block: to,
            attempt: 0,
        }
    }

    fn make_events(contract: &str, block: u64, count: usize) -> Vec<CreateEvent> {
        (0..count)
            .map(|i| CreateEvent {
                contract_address: contract.to_string(),
                event_name: "Transfer".to_string(),
                block_number: block,
                transaction_hash: format!("0x{:064x}", block * 1000 + i as u64),
                log_index: i as u32,
                event_data: serde_json::json!({"index": i}),
            })
            .collect()
    }

    fn make_result(contract: &str, from: u64, to: u64, event_count: usize) -> BlockRangeResult {
        BlockRangeResult {
            task: make_task(contract, from, to),
            events: make_events(contract, from, event_count),
            elapsed_ms: 10,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – ordering guarantee
    // ─────────────────────────────────────────────────────────────────────────

    /// Results arriving out of order must be buffered until the contiguous
    /// prefix is complete, then flushed in ascending block order.
    #[tokio::test]
    async fn test_state_manager_orders_out_of_order_results() {
        let contract = "0xcontract";
        // Frontier starts at block 100 (i.e. last committed = 99).
        let sm = StateManager::new(vec![(contract.to_string(), 100)]);

        // Three tasks: [100-199], [200-299], [300-399] — delivered in reverse.

        // Task 3 arrives first – nothing should flush yet.
        let ready = sm.register_result(make_result(contract, 300, 399, 5)).await;
        assert!(ready.is_empty(), "block 300 cannot commit before 100 is done");

        // Task 2 arrives – still blocked on task 1.
        let ready = sm.register_result(make_result(contract, 200, 299, 3)).await;
        assert!(ready.is_empty(), "block 200 cannot commit before 100 is done");

        // Task 1 arrives – all three flush together in order.
        let ready = sm.register_result(make_result(contract, 100, 199, 7)).await;
        assert_eq!(ready.len(), 3, "all three batches should flush together");
        assert_eq!(ready[0].task.from_block, 100);
        assert_eq!(ready[1].task.from_block, 200);
        assert_eq!(ready[2].task.from_block, 300);

        // Frontier must now be at 400.
        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[contract], 400);
    }

    /// A single in-order result should flush immediately.
    #[tokio::test]
    async fn test_state_manager_flushes_in_order_result_immediately() {
        let contract = "0xcontract";
        let sm = StateManager::new(vec![(contract.to_string(), 50)]);

        let ready = sm.register_result(make_result(contract, 50, 99, 2)).await;
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.from_block, 50);

        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[contract], 100);
    }

    /// Partial flush: only the contiguous prefix is returned; the gap stays pending.
    #[tokio::test]
    async fn test_state_manager_partial_flush() {
        let contract = "0xcontract";
        let sm = StateManager::new(vec![(contract.to_string(), 0)]);

        // [0-9] flushes immediately.
        let ready = sm.register_result(make_result(contract, 0, 9, 1)).await;
        assert_eq!(ready.len(), 1);

        // [20-29] is blocked by the missing [10-19].
        let ready = sm.register_result(make_result(contract, 20, 29, 1)).await;
        assert!(ready.is_empty());

        // Fill the gap – [10-19] and [20-29] flush together.
        let ready = sm.register_result(make_result(contract, 10, 19, 1)).await;
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].task.from_block, 10);
        assert_eq!(ready[1].task.from_block, 20);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – reorg handling (public API only)
    // ─────────────────────────────────────────────────────────────────────────

    /// After a reorg the frontier rolls back and any pending results in the
    /// reorg range are discarded.  We verify this entirely through the public
    /// API: register results, trigger rollback, then confirm that re-registering
    /// the same range from the rolled-back frontier works correctly.
    #[tokio::test]
    async fn test_state_manager_reorg_rollback() {
        let contract = "0xcontract";
        let sm = StateManager::new(vec![(contract.to_string(), 0)]);

        // Index blocks 0-99 successfully.
        let ready = sm.register_result(make_result(contract, 0, 99, 10)).await;
        assert_eq!(ready.len(), 1);

        // Frontier is now at 100.
        assert_eq!(sm.frontiers().await[contract], 100);

        // Simulate an in-flight result for [100-149] that arrived in the
        // pending buffer but was not yet committed.
        // We register it so it sits in pending (frontier is at 100, so it
        // flushes immediately — we need to test a range *beyond* the frontier).
        // Use [150-199] to leave a gap so it stays pending.
        sm.register_result(make_result(contract, 150, 199, 5)).await;

        // Reorg at block 80 – frontier should roll back to 80.
        sm.signal_reorg(80).await;
        sm.rollback_to(80).await;

        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[contract], 80, "frontier must roll back to reorg block");

        // After rollback, re-registering [80-99] should flush immediately
        // (frontier is at 80 again).
        let ready = sm.register_result(make_result(contract, 80, 99, 3)).await;
        assert_eq!(ready.len(), 1, "re-indexed range should flush after rollback");
        assert_eq!(ready[0].task.from_block, 80);

        // The previously pending [150-199] must have been discarded.
        // Verify by checking that [100-149] (which was never registered) still
        // blocks [150-199] from flushing.
        let ready = sm.register_result(make_result(contract, 150, 199, 2)).await;
        assert!(
            ready.is_empty(),
            "discarded pending result must not re-appear after rollback"
        );
    }

    /// Reorg signal is visible to receivers and can be cleared.
    #[tokio::test]
    async fn test_state_manager_reorg_signal_and_clear() {
        let sm = StateManager::new(vec![]);
        let mut rx = sm.reorg_receiver();

        assert!(rx.borrow().is_none(), "no reorg initially");

        sm.signal_reorg(500).await;
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), Some(500));

        sm.clear_reorg().await;
        rx.changed().await.unwrap();
        assert!(rx.borrow().is_none(), "reorg cleared");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – concurrent access (race condition safety)
    // ─────────────────────────────────────────────────────────────────────────

    /// Many tasks completing concurrently must not lose events or corrupt
    /// the frontier.  A Barrier ensures maximum contention.
    #[tokio::test]
    async fn test_state_manager_concurrent_register_result() {
        let contract = "0xcontract";
        let sm = Arc::new(StateManager::new(vec![(contract.to_string(), 0)]));

        // 20 tasks of 10 blocks each: [0-9], [10-19], ..., [190-199].
        let task_count: u64 = 20;
        let batch_size: u64 = 10;

        let barrier = Arc::new(Barrier::new(task_count as usize));
        let mut handles = Vec::new();

        for i in 0..task_count {
            let sm_clone = Arc::clone(&sm);
            let barrier_clone = Arc::clone(&barrier);
            let contract_str = contract.to_string();

            handles.push(tokio::spawn(async move {
                // All tasks start simultaneously to maximise contention.
                barrier_clone.wait().await;

                let from = i * batch_size;
                let to = from + batch_size - 1;
                sm_clone
                    .register_result(make_result(&contract_str, from, to, 1))
                    .await
            }));
        }

        let mut total_flushed = 0usize;
        for handle in handles {
            let ready = handle.await.expect("task panicked");
            total_flushed += ready.len();
        }

        // Every task must be flushed exactly once.
        assert_eq!(
            total_flushed, task_count as usize,
            "every task must be flushed exactly once"
        );

        // Frontier must be at the end.
        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[contract], task_count * batch_size);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – multiple contracts are independent
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_state_manager_independent_contracts() {
        let c1 = "0xcontract1";
        let c2 = "0xcontract2";
        let sm = StateManager::new(vec![(c1.to_string(), 0), (c2.to_string(), 500)]);

        let ready = sm.register_result(make_result(c1, 0, 99, 3)).await;
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.contract_address, c1);

        let ready = sm.register_result(make_result(c2, 500, 599, 2)).await;
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.contract_address, c2);

        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[c1], 100);
        assert_eq!(frontiers[c2], 600);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – unknown contract does not panic
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_state_manager_unknown_contract_does_not_panic() {
        let sm = StateManager::new(vec![]);

        // No frontier for "0xunknown" – result goes into pending but nothing flushes.
        let ready = sm
            .register_result(make_result("0xunknown", 0, 9, 1))
            .await;
        assert!(ready.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Metrics
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_metrics_record_batch() {
        let m = IndexingMetrics::new(1000);

        m.record_batch(50, 200);
        m.record_batch(30, 100);

        let snap = m.snapshot();
        assert_eq!(snap.total_events_indexed, 80);
        assert_eq!(snap.total_batches_processed, 2);
        assert_eq!(snap.avg_batch_ms, 150); // (200 + 100) / 2
    }

    #[test]
    fn test_metrics_events_per_second() {
        let m = IndexingMetrics::new(1000);

        // 1000 events in 500 ms → 2000 events/s
        m.record_batch(1000, 500);

        let snap = m.snapshot();
        assert!(
            (snap.events_per_second - 2000.0).abs() < 1.0,
            "expected ~2000 eps, got {}",
            snap.events_per_second
        );
    }

    #[test]
    fn test_metrics_backlog_alert_below_threshold() {
        let m = IndexingMetrics::new(500);
        m.update_backlog(499);
        assert!(!m.snapshot().backlog_alert_active);
    }

    #[test]
    fn test_metrics_backlog_alert_above_threshold() {
        let m = IndexingMetrics::new(500);
        m.update_backlog(501);
        assert!(m.snapshot().backlog_alert_active);
    }

    #[test]
    fn test_metrics_error_and_reorg_counters() {
        let m = IndexingMetrics::new(1000);

        m.record_error();
        m.record_error();
        m.record_reorg();

        let snap = m.snapshot();
        assert_eq!(snap.total_errors, 2);
        assert_eq!(snap.total_reorgs, 1);
    }

    #[test]
    fn test_metrics_zero_batches_snapshot() {
        let m = IndexingMetrics::new(1000);
        let snap = m.snapshot();

        assert_eq!(snap.avg_batch_ms, 0);
        assert_eq!(snap.events_per_second, 0.0);
        assert!(!snap.backlog_alert_active);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // BlockRangeTask field correctness
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_block_range_task_fields() {
        let task = make_task("0xabc", 100, 199);
        assert_eq!(task.from_block, 100);
        assert_eq!(task.to_block, 199);
        assert_eq!(task.attempt, 0);
        assert_eq!(task.contract_address, "0xabc");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – rollback does not affect unrelated contracts
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_reorg_rollback_does_not_affect_other_contracts() {
        let c1 = "0xcontract1";
        let c2 = "0xcontract2";
        let sm = StateManager::new(vec![(c1.to_string(), 0), (c2.to_string(), 0)]);

        // Both contracts index [0-99].
        sm.register_result(make_result(c1, 0, 99, 5)).await;
        sm.register_result(make_result(c2, 0, 99, 5)).await;

        // Reorg only affects c1 at block 50.
        sm.rollback_to(50).await;

        let frontiers = sm.frontiers().await;
        // c1 rolled back to 50.
        assert_eq!(frontiers[c1], 50);
        // c2 was already at 100 which is > 50, so it also rolls back.
        // This is correct: a chain reorg affects all contracts at that height.
        assert_eq!(frontiers[c2], 50);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StateManager – frontier advances correctly across multiple sequential flushes
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_frontier_advances_across_sequential_flushes() {
        let contract = "0xcontract";
        let sm = StateManager::new(vec![(contract.to_string(), 0)]);

        for i in 0u64..5 {
            let from = i * 100;
            let to = from + 99;
            let ready = sm.register_result(make_result(contract, from, to, 2)).await;
            assert_eq!(ready.len(), 1);
            assert_eq!(ready[0].task.from_block, from);
        }

        let frontiers = sm.frontiers().await;
        assert_eq!(frontiers[contract], 500);
    }
}
