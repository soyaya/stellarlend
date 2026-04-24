#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NETWORK="${STELLAR_NETWORK:-testnet}"
EXECUTE=false

usage() {
  cat <<EOF
Usage: ./scripts/rollback.sh [--network <net>] [--execute]

Without --execute, prints the rollback plan derived from the previous deployment manifest.
With --execute, redeploys the previous lending and AMM artifacts and stores the new ids.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --network) NETWORK="$2"; shift 2 ;;
    --execute) EXECUTE=true; shift ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

command -v stellar >/dev/null 2>&1 || {
  echo "ERROR: stellar CLI not found." >&2
  exit 1
}

MANIFEST_FILE="$SCRIPT_DIR/deployed/$NETWORK/deployment-manifest.previous.json"
if [[ ! -f "$MANIFEST_FILE" ]]; then
  echo "ERROR: No previous deployment manifest found at $MANIFEST_FILE" >&2
  exit 1
fi

read_manifest_value() {
  node -e "const fs=require('fs'); const manifest=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); const path=process.argv[2].split('.'); let value=manifest; for (const part of path) { value = value?.[part]; } if (value !== undefined) console.log(value);" "$MANIFEST_FILE" "$1"
}

LENDING_WASM="$(read_manifest_value lending.wasm)"
AMM_WASM="$(read_manifest_value amm.wasm || true)"

if [[ -z "${ADMIN_SECRET_KEY:-}" && "$EXECUTE" == true ]]; then
  echo "ERROR: ADMIN_SECRET_KEY must be set to execute a rollback." >&2
  exit 1
fi

echo "Rollback plan for $NETWORK"
echo "  lending wasm: ${LENDING_WASM:-<missing>}"
echo "  amm wasm    : ${AMM_WASM:-<not configured>}"

if ! $EXECUTE; then
  echo "Dry run only. Re-run with --execute to redeploy the previous artifacts."
  exit 0
fi

OUTPUT_DIR="$SCRIPT_DIR/deployed/$NETWORK/rollback-$(date +%Y%m%d%H%M%S)"
mkdir -p "$OUTPUT_DIR"

COMMON_ARGS=(--source "$ADMIN_SECRET_KEY" --network "$NETWORK")
if [[ -n "${STELLAR_RPC_URL:-}" ]]; then
  COMMON_ARGS+=(--rpc-url "$STELLAR_RPC_URL")
fi

rollback_deploy() {
  local wasm="$1"
  local output_file="$2"
  local contract_id
  contract_id="$(stellar contract deploy --wasm "$wasm" "${COMMON_ARGS[@]}" 2>&1 | tail -1)"
  echo "$contract_id" > "$output_file"
  echo "$contract_id"
}

LENDING_ID="$(rollback_deploy "$LENDING_WASM" "$OUTPUT_DIR/lending_contract_id.txt")"
echo "Rollback lending contract id: $LENDING_ID"

if [[ -n "$AMM_WASM" ]]; then
  AMM_ID="$(rollback_deploy "$AMM_WASM" "$OUTPUT_DIR/amm_contract_id.txt")"
  echo "Rollback AMM contract id: $AMM_ID"
fi

echo "Rollback deployment artifacts written to $OUTPUT_DIR"