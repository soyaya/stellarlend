# Arithmetic safety policy

## Goals

- **No wrapping arithmetic** in protocol-critical paths.
- **Overflow/underflow must return an error** (or fail the call) rather than silently saturating/wrapping.
- **Division by zero must be guarded** and treated as an error.

## Numeric domains used

- **Soroban contract amounts/prices**: typically `i128`.
  - **Max magnitude**: \(2^{127}-1\) (≈ 1.7e38). Any `amount * price` style calculation must use checked math or widened arithmetic (`I256`) before narrowing back.
- **Ledger timestamps**: `u64`.
  - Always guard subtraction (`now >= then`) before computing deltas.
- **Basis points**: `i128` / `u32` depending on module.
  - Must remain within \([0, 10_000]\) unless explicitly documented.

## Implementation guidance (Rust/Soroban)

- Prefer `checked_add / checked_sub / checked_mul / checked_div`.
- For high-range intermediate calculations (e.g. `amount * price`), use `soroban_sdk::I256` or scale down earlier to avoid `i128` overflow.
- Avoid `saturating_*` in protocol-critical accounting; it can mask faults. If saturating is used for **analytics-only counters**, document it explicitly.

## Gas cost note

- Checked arithmetic adds a small number of branches compared to raw ops.
- Most heavy arithmetic in this repo already uses `I256` for safety in view/math paths; the additional overhead is generally dominated by storage IO and cross-contract calls.

## Testing

- Add edge-case tests for:
  - `i128::MAX`/`i128::MIN` boundaries
  - multiplication overflow (`MAX * 2`)
  - underflow (`0 - 1`)
  - division-by-zero
  - rounding/precision (basis points + fixed-point scale)
