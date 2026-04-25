#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
SEED_FILE="$SCRIPT_DIR/default-seed.json"
NETWORK="${STELLAR_NETWORK:-local}"

usage() {
  cat <<EOF
Usage: ./scripts/dev/apply-seed-state.sh [--seed-file <path>]

The seed manifest is used to prime a local sandbox with realistic development defaults.
If ADMIN_SECRET_KEY and LENDING_CONTRACT_ID are provided, the script also applies the
emergency pause state to exercise fork-specific debugging paths.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --seed-file) SEED_FILE="$2"; shift 2 ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

command -v node >/dev/null 2>&1 || {
  echo "ERROR: node not found." >&2
  exit 1
}

if [[ ! -f "$SEED_FILE" ]]; then
  echo "ERROR: seed file not found: $SEED_FILE" >&2
  exit 1
fi

DEV_ENV_FILE="$REPO_ROOT/.dev/seed.env"
mkdir -p "$(dirname "$DEV_ENV_FILE")"

node - <<'NODE' "$SEED_FILE" "$DEV_ENV_FILE"
const fs = require('fs');
const [seedFile, envFile] = process.argv.slice(2);
const seed = JSON.parse(fs.readFileSync(seedFile, 'utf8'));

const lines = [
  `DEV_SEED_NAME=${seed.name}`,
  `DEV_SEED_REFERENCE_NETWORK=${seed.referenceNetwork}`,
  `DEV_SEED_MIN_COLLATERAL_RATIO=${seed.lending.minCollateralRatio}`,
  `DEV_SEED_LIQUIDATION_THRESHOLD=${seed.lending.liquidationThreshold}`,
  `DEV_SEED_EMERGENCY_PAUSED=${seed.lending.emergencyPaused}`,
  `DEV_SEED_AMM_DEFAULT_SLIPPAGE=${seed.amm.defaultSlippage}`,
  `DEV_SEED_AMM_MAX_SLIPPAGE=${seed.amm.maxSlippage}`,
  `DEV_SEED_AMM_AUTO_SWAP_THRESHOLD=${seed.amm.autoSwapThreshold}`,
  `DEV_SEED_TIMESTAMP_MODE=${seed.timestampMode}`,
];

fs.writeFileSync(envFile, `${lines.join('\n')}\n`);
NODE

echo "Wrote $DEV_ENV_FILE"

if [[ -n "${ADMIN_SECRET_KEY:-}" && -n "${LENDING_CONTRACT_ID:-}" ]]; then
  command -v stellar >/dev/null 2>&1 || {
    echo "ERROR: stellar CLI not found." >&2
    exit 1
  }

  EMERGENCY_PAUSED="$(node -e "const fs=require('fs'); const seed=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); console.log(seed.lending.emergencyPaused);" "$SEED_FILE")"

  echo "Applying emergency pause seed to $LENDING_CONTRACT_ID on $NETWORK"
  STELLAR_ARGS=(--id "$LENDING_CONTRACT_ID" --source "$ADMIN_SECRET_KEY" --network "$NETWORK")
  if [[ -n "${STELLAR_RPC_URL:-}" ]]; then
    STELLAR_ARGS+=(--rpc-url "$STELLAR_RPC_URL")
  fi

  stellar contract invoke "${STELLAR_ARGS[@]}" -- set_emergency_pause --caller "${ADMIN_ADDRESS:-$(stellar keys address "$ADMIN_SECRET_KEY" 2>/dev/null || echo '')}" --paused "$EMERGENCY_PAUSED"
else
  echo "Seed overlay prepared. Export ADMIN_SECRET_KEY and LENDING_CONTRACT_ID to apply on-chain pause state."
fi