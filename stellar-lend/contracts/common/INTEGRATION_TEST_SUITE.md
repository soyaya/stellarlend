# Cross-Contract Integration Test Suite

The `protocol_integration_test.rs` suite validates contract-level primitives used by cross-contract flows.

## Covered scenarios

- Ordered message publish/dequeue semantics via `message_bus`.
- Failure injection (`mark_failed`) and retry (`retry_failed`) paths.
- Replay protection on duplicate acknowledgements (`confirm_delivery`).
- Cache usage semantics (`set_cached`, `get_cached`) with hit/miss metrics.
- Shared type compatibility and version tagging checks.

## CI behavior

Run from workspace root:

```bash
cargo test -p stellarlend-common --manifest-path stellar-lend/Cargo.toml
```

The suite is designed to catch regressions in protocol integration primitives before higher-level contract changes are merged.
