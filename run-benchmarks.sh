#!/usr/bin/env bash
# run-benchmarks.sh — Local gas benchmark runner for StellarLend
#
# Usage:
#   ./run-benchmarks.sh                        # Run all benchmarks
#   ./run-benchmarks.sh --compare              # Compare against baseline
#   ./run-benchmarks.sh --update-baseline      # Run and update baseline.json
#   ./run-benchmarks.sh --help                 # Show help

set -euo pipefail

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

BENCH_DIR="stellar-lend"
BASELINE="stellar-lend/benchmarks/baseline.json"
OUTPUT="stellar-lend/benchmark-results.json"

# ── Argument parsing ──────────────────────────────────────────────────────────
COMPARE=false
UPDATE_BASELINE=false
SHOW_HELP=false

for arg in "$@"; do
    case $arg in
        --compare)       COMPARE=true ;;
        --update-baseline) UPDATE_BASELINE=true ;;
        --help|-h)       SHOW_HELP=true ;;
    esac
done

if $SHOW_HELP; then
    echo ""
    echo "  StellarLend Gas Benchmark Runner"
    echo ""
    echo "  Usage:"
    echo "    ./run-benchmarks.sh                  Run all benchmarks"
    echo "    ./run-benchmarks.sh --compare        Compare against baseline (fail on regression)"
    echo "    ./run-benchmarks.sh --update-baseline  Run and save results as new baseline"
    echo ""
    echo "  Output:"
    echo "    benchmark-results.json               Latest results (always written)"
    echo "    benchmarks/baseline.json             Baseline for regression detection"
    echo ""
    exit 0
fi

# ── Prerequisites ─────────────────────────────────────────────────────────────
echo -e "${BLUE}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       StellarLend Gas Benchmark Suite                    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""

if ! command -v cargo &>/dev/null; then
    echo -e "${RED}✗ Rust/Cargo not found. Install from https://rustup.rs${NC}"
    exit 1
fi

if [ ! -d "$BENCH_DIR" ]; then
    echo -e "${RED}✗ stellar-lend directory not found. Run from project root.${NC}"
    exit 1
fi

# ── Build ─────────────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Building benchmark suite...${NC}"
(cd "$BENCH_DIR" && cargo build --bin run_benchmarks --release 2>&1)
echo -e "${GREEN}✓ Build complete${NC}"
echo ""

# ── Run benchmarks ────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Running gas benchmarks...${NC}"
echo ""

if $COMPARE; then
    RESULTS_COUNT=$(python3 -c "import json; d=json.load(open('$BASELINE')); print(len(d.get('results', [])))" 2>/dev/null || echo "0")
    if [ "$RESULTS_COUNT" -gt "0" ]; then
        echo -e "${CYAN}  Comparing against baseline ($RESULTS_COUNT operations)...${NC}"
        (cd "$BENCH_DIR" && cargo run --bin run_benchmarks --release -- \
            --compare "../$BASELINE" \
            --output "../$OUTPUT")
        EXIT_CODE=$?
        if [ $EXIT_CODE -ne 0 ]; then
            echo ""
            echo -e "${RED}✗ Gas regression detected! Review benchmark-results.json${NC}"
            exit $EXIT_CODE
        fi
    else
        echo -e "${YELLOW}  No baseline results found — running without comparison.${NC}"
        echo -e "${YELLOW}  Run with --update-baseline to create a baseline.${NC}"
        (cd "$BENCH_DIR" && cargo run --bin run_benchmarks --release -- --output "../$OUTPUT")
    fi
elif $UPDATE_BASELINE; then
    echo -e "${CYAN}  Running benchmarks and updating baseline...${NC}"
    (cd "$BENCH_DIR" && cargo run --bin run_benchmarks --release -- --output "../$OUTPUT")
    cp "$OUTPUT" "$BASELINE"
    echo ""
    echo -e "${GREEN}✓ Baseline updated: $BASELINE${NC}"
    echo -e "${YELLOW}  Commit this file to track gas usage over time.${NC}"
else
    (cd "$BENCH_DIR" && cargo run --bin run_benchmarks --release -- --output "../$OUTPUT")
fi

echo ""
echo -e "${GREEN}✓ Benchmarks complete. Results: $OUTPUT${NC}"

# ── Quick summary ─────────────────────────────────────────────────────────────
if command -v python3 &>/dev/null && [ -f "$OUTPUT" ]; then
    echo ""
    python3 - <<'EOF'
import json

with open("stellar-lend/benchmark-results.json") as f:
    report = json.load(f)

total = report["total_benchmarks"]
passed = report["passed"]
failed = report["failed"]

print(f"  Summary: {total} benchmarks | {passed} passed | {failed} failed")

if failed > 0:
    print("\n  Over-budget operations:")
    for r in report["results"]:
        if not r["within_budget"] and r["budget"] > 0:
            print(f"    ✗ {r['operation']}: {r['instructions']:,} (budget: {r['budget']:,})")
EOF
fi
