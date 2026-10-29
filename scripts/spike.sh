#!/usr/bin/env bash
set -euo pipefail

SCENARIO="${1:-happy}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

docker compose up -d bitcoind

cargo run -p ponthtlc-spike -- --scenario "$SCENARIO"
