#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

COMPOSE=(docker compose -f docker-compose.mvp.yml)

cleanup() {
  "${COMPOSE[@]}" down
}
trap cleanup EXIT

"${COMPOSE[@]}" up --build --wait

live="$(curl -fsS http://127.0.0.1:9800/health/live)"
if [[ "$live" != "ok" ]]; then
  echo "ponthtlc health check returned: $live" >&2
  exit 1
fi

frontend_file="$(mktemp)"
frontend_code="$(curl -sS -o "$frontend_file" -w "%{http_code}" http://127.0.0.1:5173/)"
if [[ "$frontend_code" != "200" ]] || ! grep -q "escrow dashboard" "$frontend_file"; then
  echo "frontend did not return the escrow dashboard (HTTP $frontend_code)" >&2
  rm -f "$frontend_file"
  exit 1
fi
rm -f "$frontend_file"

"${COMPOSE[@]}" stop ponthtlc frontend
cargo test -p ponthtlc-nostr --test relay_integration
cargo test -p ponthtlc --test api_happy
cargo test -p ponthtlc --test restart

echo "integration checks passed"
