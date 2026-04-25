#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
WATCH_TARGET="$REPO_ROOT/stellar-lend"
COMMAND="./scripts/build.sh --debug"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deploy-local)
      COMMAND="./scripts/build.sh --debug && ./scripts/deploy.sh --network local --build"
      shift
      ;;
    --help)
      echo "Usage: ./scripts/dev/watch-contracts.sh [--deploy-local]"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: cargo not found." >&2
  exit 1
}

command -v cargo-watch >/dev/null 2>&1 || {
  echo "ERROR: cargo-watch not found. Install it with: cargo install cargo-watch" >&2
  exit 1
}

cd "$REPO_ROOT"
echo "Watching $WATCH_TARGET"
cargo watch -w "$WATCH_TARGET" -s "$COMMAND"