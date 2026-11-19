# Cargo targets need a working Rust toolchain. On Windows, run them from the VS developer prompt.
COMPOSE := docker compose -f docker-compose.mvp.yml

.PHONY: dev dev-bootstrap down bitcoind relay channel channel-win test-happy test-fail test-hold test-all test-api test-unit test-restart test-nostr

dev:
	$(COMPOSE) up --build

dev-bootstrap:
	$(COMPOSE) --profile bootstrap up --build

down:
	$(COMPOSE) down

bitcoind:
	$(COMPOSE) up -d --wait bitcoind

relay:
	$(COMPOSE) up -d --wait nostr-relay

channel:
	bash scripts/open-channel.sh

channel-win:
	powershell -ExecutionPolicy Bypass -File scripts/open-channel.ps1

test-happy: bitcoind
	cargo run -p ponthtlc-spike -- --scenario happy

test-fail: bitcoind
	cargo run -p ponthtlc-spike -- --scenario fail

test-hold: bitcoind
	cargo run -p ponthtlc-spike -- --scenario hold

.NOTPARALLEL: test-all
test-all: bitcoind
	cargo run -p ponthtlc-spike -- --scenario happy
	cargo run -p ponthtlc-spike -- --scenario fail
	cargo run -p ponthtlc-spike -- --scenario hold

test-api: bitcoind
	cargo test -p ponthtlc --test api_happy

test-restart: bitcoind
	cargo test -p ponthtlc --test restart

test-nostr: relay
	cargo test -p ponthtlc-nostr --test relay_integration

test-unit:
	cargo test --workspace --lib
