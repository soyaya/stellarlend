# Differential Test Report

## What This Tests

Differential testing runs the **same inputs against two independent contract instances** and asserts their outputs are identical. This catches subtle behavioral regressions that unit tests miss — especially after upgrades or refactors.

## Files

| File | Purpose |
|---|---|
| `src/tests/diff_harness.rs` | `HwAdapter`, `PositionSnapshot`, `DivergenceReport` — core harness |
| `src/tests/differential_test.rs` | Property comparison tests (deposit, borrow, repay, zero-amount, sequential) |
| `src/tests/migration_verification_test.rs` | Storage layout survives upgrade (collateral, debt, admin, multi-user) |

## Running Locally

```bash
cd stellar-lend
# All differential tests
cargo test --package hello-world --lib tests::differential_test -- --nocapture
cargo test --package hello-world --lib tests::migration_verification_test -- --nocapture
```

## How Divergences Are Reported

If two instances return different results for the same input, the test panics with:

```
[DIVERGENCE] deposit: v1=Ok(true) v2=Err(())
[DIVERGENCE] get_position: v1=PositionSnapshot { collateral: 1000, debt: 0 } v2=PositionSnapshot { collateral: 999, debt: 0 }
```

## Edge Cases Covered

| Edge Case | How Handled |
|---|---|
| Non-deterministic behavior | Ledger timestamp pinned via `env.ledger().set_timestamp()` before each test |
| State-dependent outputs | Tests run full sequences: deposit → borrow → repay → check position |
| Zero-amount inputs | Explicit test asserting both instances reject consistently |
| Storage layout across upgrades | `migration_verification_test.rs` reads raw storage keys via `env.as_contract()` |
| Multiple users | Multi-user migration test with 5 users and distinct amounts |

## CI Integration

Differential tests run as a dedicated CI step in `.github/workflows/ci-cd.yml` and upload `differential-test-report.txt` as an artifact on every push/PR. A failure here means a behavioral regression was introduced.

## Known Acceptable Divergences

None currently. If an intentional behavioral change is made, document it here with the PR number and rationale before merging.
