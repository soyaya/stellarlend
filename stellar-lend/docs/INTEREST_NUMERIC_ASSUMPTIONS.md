# Interest Numeric Assumptions and Safety Limits

This note documents numeric assumptions for long-horizon interest accrual and the overflow/underflow protections validated by tests.

## Scope

- `contracts/lending/src/borrow.rs` (`calculate_interest`, `get_user_debt`)
- `contracts/hello-world/src/interest_rate.rs` (`calculate_borrow_rate`, `calculate_accrued_interest`)

## Assumptions

- Arithmetic uses signed `i128` values for balances and rates.
- Time is represented in seconds (`u64` timestamps).
- `lending` interest model is simple APR at fixed `500` bps (5%/year).
- `hello-world` rate model is utilization-based and bounded by configured floor/ceiling.

## Numeric Safety Properties

### Lending contract (`borrow.rs`)

- Interest calculation uses `I256` intermediates to avoid intermediate multiplication overflow.
- Conversion back to `i128` is clamped with `unwrap_or(i128::MAX)`, producing a saturating upper bound.
- `get_user_debt` applies `saturating_add` when accumulating interest, preventing overflow on repeated reads/accrual events.

### Hello-world contract (`interest_rate.rs`)

- Accrued interest uses checked arithmetic (`checked_mul`, `checked_div`) and returns `InterestRateError::Overflow` instead of panicking.
- Borrow rate is explicitly clamped with:
  - `max(rate_floor_bps)`
  - `min(rate_ceiling_bps)`
- Utilization is capped at 100% (`10000` bps), even when borrows exceed deposits.

## Long-Horizon / Extreme Scenarios Covered

- Multi-decade to centuries-scale timestamp jumps (including `u64::MAX` in lending tests).
- Maximum configured annual rate (10000 bps) for accrued-interest monotonicity checks.
- Overflow boundary test where the last safe elapsed second succeeds and the next second returns overflow.
- Extreme high-utilization + aggressive configuration + emergency adjustment still clamped to ceiling.
- Extreme negative emergency adjustment still clamped to floor.

## Security Notes

- No test relies on unchecked casts for financial results.
- Expected behavior under extreme inputs is deterministic:
  - Saturation in `lending`
  - Explicit error in `hello-world`
- This prevents silent wraparound and protects debt/accounting invariants under adversarial time jumps and parameter settings.
