# Smart Contract Fuzzing (Property-Based, Coverage-Guided)

This repo uses **coverage-guided fuzzing** (libFuzzer via `cargo-fuzz`) to explore smart-contract edge cases that unit tests rarely hit (state-machine sequencing, time jumps, oracle manipulation, boundary math).

The fuzz targets live under `stellar-lend/fuzz/`.

## What is fuzzed

Targets (one binary per contract area):

- `lending_actions` — state-machine fuzzing for `stellarlend-lending`
- `amm_actions` — action fuzzing for `stellarlend-amm`
- `bridge_actions` — action fuzzing for `bridge`

Each target interprets the input as a sequence of fixed-size **32-byte actions**. This gives libFuzzer structure to mutate while still keeping the harness lightweight.

## Strategy (high level)

### Action model (protocol-specific)

For performance and coverage, the fuzzer uses a compact action encoding:

- One input file = `N` actions
- One action = 32 bytes (see `stellar-lend/fuzz/src/encoding.rs`)

Each harness maps those bytes to protocol calls (deposit/borrow/repay/withdraw, swaps/liquidity, bridge operations) and validates basic invariants after the sequence.

### Time-dependent properties

The harnesses mutate ledger time via `env.ledger().with_mut(|li| li.timestamp = ...)` to exercise:

- interest accrual and timestamp math
- deadline / timeout style checks

### Oracle manipulation during fuzzing

The lending fuzzer registers a fuzz-only oracle contract (`FuzzOracle`) and can change per-asset prices on the fly.

This specifically targets view logic (collateral value, debt value, health factor) under adversarial price changes.

### Large state spaces

The `*_actions` targets are state-machine fuzzers. Inputs encode *sequences*, not single calls, so the fuzzer can reach deep interleavings:

- borrow → time jump → repay partial → withdraw → view reads
- pause toggles + retries
- repeated protocol config changes

### Performance guardrails

To keep fuzzing fast and CI-friendly:

- actions per input are bounded
- time deltas are capped per step
- harnesses use `try_*` contract calls where possible to avoid panics and keep exploration going

## Custom mutators

`lending_actions` implements a **custom libFuzzer mutator** in `stellar-lend/fuzz/fuzz_targets/lending_actions.rs`:

- keeps inputs aligned to 32-byte action boundaries
- performs small, field-aware mutations (kind/user/asset selectors, amount bytes, time bytes)
- occasionally grows/shrinks by one full action to explore different sequence lengths

This is intentionally protocol-aware: it helps libFuzzer spend more time exploring meaningful contract state transitions rather than breaking the input structure.

## Corpus management

Seed corpora are checked into git:

- `stellar-lend/fuzz/corpus/lending_actions/`
- `stellar-lend/fuzz/corpus/amm_actions/`
- `stellar-lend/fuzz/corpus/bridge_actions/`

A minimum corpus size is enforced by `scripts/fuzz/check_corpus.sh` (default: **10** files per target; configurable via `MIN_CORPUS_FILES`).

## Running fuzzers locally

Prereqs:

- Rust nightly (`rustup toolchain install nightly`)
- `cargo-fuzz` (`cargo +nightly install cargo-fuzz`)
- LLVM/clang toolchain (required by libFuzzer)

Run:

```bash
cd stellar-lend
cargo +nightly fuzz run lending_actions -- -runs=50000 -timeout=5
```

Other targets:

```bash
cd stellar-lend
cargo +nightly fuzz run amm_actions -- -runs=50000 -timeout=5
cargo +nightly fuzz run bridge_actions -- -runs=50000 -timeout=5
```

## Reproducing a crash

When a crash is found, libFuzzer stores a reproducer in:

`stellar-lend/fuzz/artifacts/<target>/`

Use:

```bash
./scripts/fuzz/repro.sh lending_actions stellar-lend/fuzz/artifacts/lending_actions/crash-* -- -runs=1
```

## CI integration

CI runs a smoke fuzz pass (bounded number of executions per target) via:

- `scripts/fuzz/check_corpus.sh`
- `scripts/fuzz/run_ci_smoke.sh`

This keeps the pipeline deterministic while still exercising the fuzz harnesses continuously.

