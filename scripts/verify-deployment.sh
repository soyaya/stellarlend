#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NETWORK="${STELLAR_NETWORK:-testnet}"
DEPLOY_DIR="$SCRIPT_DIR/deployed/$NETWORK"
MANIFEST_FILE="${DEPLOYMENT_MANIFEST:-$DEPLOY_DIR/deployment-manifest.json}"
LENDING_CONTRACT_ID="${LENDING_CONTRACT_ID:-}"
AMM_CONTRACT_ID="${AMM_CONTRACT_ID:-}"
EXPECT_MIN_RATIO="${EXPECTED_MIN_COLLATERAL_RATIO:-11000}"
EXPECT_LIQ_THRESHOLD="${EXPECTED_LIQUIDATION_THRESHOLD:-10500}"
SKIP_AMM=false

usage() {
  cat <<EOF
Usage: ./scripts/verify-deployment.sh [options]

Options:
  --network <net>                     Target network (default: testnet)
  --lending-contract-id <id>          Override lending contract id
  --amm-contract-id <id>              Override AMM contract id
  --manifest <path>                   Override deployment manifest path
  --expected-min-collateral-ratio <n> Expected lending collateral ratio
  --expected-liquidation-threshold <n> Expected liquidation threshold
  --skip-amm                          Skip AMM verification
  --help                              Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --network) NETWORK="$2"; DEPLOY_DIR="$SCRIPT_DIR/deployed/$NETWORK"; MANIFEST_FILE="${DEPLOYMENT_MANIFEST:-$DEPLOY_DIR/deployment-manifest.json}"; shift 2 ;;
    --lending-contract-id) LENDING_CONTRACT_ID="$2"; shift 2 ;;
    --amm-contract-id) AMM_CONTRACT_ID="$2"; shift 2 ;;
    --manifest) MANIFEST_FILE="$2"; shift 2 ;;
    --expected-min-collateral-ratio) EXPECT_MIN_RATIO="$2"; shift 2 ;;
    --expected-liquidation-threshold) EXPECT_LIQ_THRESHOLD="$2"; shift 2 ;;
    --skip-amm) SKIP_AMM=true; shift ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

command -v stellar >/dev/null 2>&1 || {
  echo "ERROR: stellar CLI not found." >&2
  exit 1
}

if [[ -z "$LENDING_CONTRACT_ID" && -f "$DEPLOY_DIR/lending_contract_id.txt" ]]; then
  LENDING_CONTRACT_ID="$(cat "$DEPLOY_DIR/lending_contract_id.txt")"
fi

if [[ -z "$AMM_CONTRACT_ID" && -f "$DEPLOY_DIR/amm_contract_id.txt" ]]; then
  AMM_CONTRACT_ID="$(cat "$DEPLOY_DIR/amm_contract_id.txt")"
fi

if [[ -z "$LENDING_CONTRACT_ID" ]]; then
  echo "ERROR: Lending contract id not provided and could not be loaded from $DEPLOY_DIR." >&2
  exit 1
fi

COMMON_ARGS=(--network "$NETWORK")
if [[ -n "${STELLAR_RPC_URL:-}" ]]; then
  COMMON_ARGS+=(--rpc-url "$STELLAR_RPC_URL")
fi
if [[ -n "${ADMIN_SECRET_KEY:-}" ]]; then
  COMMON_ARGS+=(--source "$ADMIN_SECRET_KEY")
fi

invoke_readonly() {
  local contract_id="$1"
  local method="$2"
  stellar contract invoke --id "$contract_id" "${COMMON_ARGS[@]}" -- "$method" 2>/dev/null | tr -d '"' || true
}

assert_equals() {
  local label="$1"
  local expected="$2"
  local actual="$3"

  if [[ "$expected" != "$actual" ]]; then
    echo "ERROR: $label mismatch. expected=$expected actual=$actual" >&2
    return 1
  fi

  echo "OK: $label = $actual"
}

echo "======================================================================"
echo " StellarLend deployment verification"
echo " Network              : $NETWORK"
echo " Lending contract ID  : $LENDING_CONTRACT_ID"
if [[ -n "$AMM_CONTRACT_ID" ]]; then
  echo " AMM contract ID      : $AMM_CONTRACT_ID"
fi
echo "======================================================================"

ADMIN_ADDRESS_VALUE="$(invoke_readonly "$LENDING_CONTRACT_ID" get_admin)"
MIN_RATIO_VALUE="$(invoke_readonly "$LENDING_CONTRACT_ID" get_min_collateral_ratio)"
LIQ_THRESHOLD_VALUE="$(invoke_readonly "$LENDING_CONTRACT_ID" get_liquidation_threshold)"
EMERGENCY_PAUSED_VALUE="$(invoke_readonly "$LENDING_CONTRACT_ID" is_emergency_paused)"

assert_equals "min_collateral_ratio" "$EXPECT_MIN_RATIO" "$MIN_RATIO_VALUE"
assert_equals "liquidation_threshold" "$EXPECT_LIQ_THRESHOLD" "$LIQ_THRESHOLD_VALUE"
assert_equals "is_emergency_paused" "false" "$EMERGENCY_PAUSED_VALUE"

if [[ -n "${ADMIN_ADDRESS:-}" && -n "$ADMIN_ADDRESS_VALUE" ]]; then
  assert_equals "admin_address" "$ADMIN_ADDRESS" "$ADMIN_ADDRESS_VALUE"
fi

if [[ -f "$MANIFEST_FILE" ]]; then
  MANIFEST_WASM="$(node -e "const fs=require('fs'); const manifest=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); console.log(manifest.lending.wasm || '');" "$MANIFEST_FILE")"
  MANIFEST_SHA="$(node -e "const fs=require('fs'); const manifest=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); console.log(manifest.lending.sha256 || '');" "$MANIFEST_FILE")"

  if [[ -n "$MANIFEST_WASM" && -f "$MANIFEST_WASM" ]]; then
    LOCAL_SHA="$(shasum -a 256 "$MANIFEST_WASM" | awk '{print $1}')"
    assert_equals "lending_wasm_sha256" "$MANIFEST_SHA" "$LOCAL_SHA"
  fi
fi

if ! $SKIP_AMM && [[ -n "$AMM_CONTRACT_ID" ]]; then
  AMM_SETTINGS="$(invoke_readonly "$AMM_CONTRACT_ID" get_amm_settings)"
  if [[ -z "$AMM_SETTINGS" ]]; then
    echo "ERROR: AMM settings could not be read from $AMM_CONTRACT_ID" >&2
    exit 1
  fi

  echo "OK: AMM settings readable"
fi

echo "Verification complete."