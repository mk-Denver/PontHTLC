# Cargo targets need a working Rust toolchain. On Windows, run them from the VS developer prompt.
COMPOSE := docker compose -f docker-compose.mvp.yml

.PHONY: dev down bitcoind relay test-happy test-fail test-hold test-all test-api test-unit test-restart test-nostr

dev:
	$(COMPOSE) up --build

down:
	$(COMPOSE) down

bitcoind:
	$(COMPOSE) up -d --wait bitcoind

relay:
	$(COMPOSE) up -d --wait nostr-relay

test-happy: bitcoind
	cargo run -p ponthtlc-spike -- --scenario happy

test-fail: bitcoind
	cargo run -p ponthtlc-spike -- --scenario fail

test-hold: bitcoind
	cargo run -p ponthtlc-spike -- --scenario hold

test-all: test-happy test-fail test-hold

test-api: bitcoind
	cargo test -p ponthtlc --test api_happy

test-restart: bitcoind
	cargo test -p ponthtlc --test restart

test-nostr: relay
	cargo test -p ponthtlc-nostr --test relay_integration

test-unit:
	cargo test --workspace --lib
