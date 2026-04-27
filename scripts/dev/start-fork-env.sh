#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DEV_DIR="$REPO_ROOT/.dev"
SOURCE_NETWORK="pubnet"
RESET=false
SEED_FILE="$SCRIPT_DIR/default-seed.json"

usage() {
  cat <<EOF
Usage: ./scripts/dev/start-fork-env.sh [options]

Options:
  --source-network <pubnet|testnet|futurenet>  Reference network to mirror (default: pubnet)
  --seed-file <path>                           Seed file to apply after local container start
  --reset                                      Stop existing Quickstart containers before booting
  --help                                       Show this help

This workflow uses Quickstart in two phases:
1. pubnet/testnet/futurenet as the read-only reference network.
2. local as the writable sandbox for contract testing.

Quickstart does not offer a mutable mainnet fork. The local sandbox is therefore seeded
from a reference-network-compatible manifest so developers can test against realistic state
without depending on testnet availability.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-network) SOURCE_NETWORK="$2"; shift 2 ;;
    --seed-file) SEED_FILE="$2"; shift 2 ;;
    --reset) RESET=true; shift ;;
    --help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

command -v stellar >/dev/null 2>&1 || {
  echo "ERROR: stellar CLI not found. Install it before starting the fork environment." >&2
  exit 1
}

mkdir -p "$DEV_DIR"

if $RESET; then
  stellar container stop "$SOURCE_NETWORK" >/dev/null 2>&1 || true
  stellar container stop local >/dev/null 2>&1 || true
fi

echo ">>> Starting reference network container: $SOURCE_NETWORK"
stellar container start "$SOURCE_NETWORK"

echo ">>> Stopping reference network container to free Quickstart ports for the writable sandbox"
stellar container stop "$SOURCE_NETWORK"

echo ">>> Starting writable local sandbox"
stellar container start local --limits testnet

cat > "$DEV_DIR/fork.env" <<EOF
STELLAR_FORK_REFERENCE_NETWORK=$SOURCE_NETWORK
STELLAR_FORK_SANDBOX_NETWORK=local
STELLAR_FORK_SEED_FILE=$SEED_FILE
STELLAR_FORK_NOTE=Quickstart local sandbox seeded from a $SOURCE_NETWORK-compatible manifest
EOF

echo "Wrote $DEV_DIR/fork.env"

if [[ -f "$SEED_FILE" ]]; then
  echo ">>> Applying seed file $SEED_FILE"
  STELLAR_NETWORK=local "$SCRIPT_DIR/apply-seed-state.sh" --seed-file "$SEED_FILE"
fi

echo "Fork development environment ready."
echo "Next steps:"
echo "  1. export STELLAR_NETWORK=local"
echo "  2. ./scripts/dev/watch-contracts.sh"
echo "  3. npm run trace:contract -- path/to/trace.json"