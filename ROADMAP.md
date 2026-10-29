# Roadmap

## Phase 1: De-risking spike (done)

Validate the core LDK primitive: `create_inbound_payment_for_hash` gives
guaranteed manual-claim hold semantics (`payment_preimage=None`, claim/fail/
expiry paths all work).

- [x] Two-node regtest LDK demo (happy/fail/hold scenarios)
- [x] GF(2^8) 2-of-3 Shamir split/reconstruct
- [x] Regtest bitcoind harness (docker-compose)
- [x] `pontmore/swap@1` kernel-action mapping documented
- [x] All three scenarios pass: `SPIKE_RESULT: PASS`

## Phase 2: MVP (in progress)

Hosted node service architecture — one `ponthtlc` daemon serves many users
via browser + Nostr. Users don't run their own node. See
[docs/infrastructure.md](docs/infrastructure.md) for the full design.

Preimage reconstruction is client-side (browser); the hosted node
never stores P.

Track via [GitHub issues #1–#5](https://github.com/mk-Denver/PontHTLC/issues).

| Issue | Component | Status |
| --- | --- | --- |
| #1 | Hosted HTLC node daemon (multi-tenant, persistent, restart recovery) | open |
| #2 | Nostr escrow (NIP-59 + `pontmore/swap@1` state machine, client-side preimage) | open |
| #3 | Backend API (REST + WebSocket, preimage pass-through) | open |
| #4 | Frontend web UI (Svelte, client-side Shamir reconstruction) | open |
| #5 | Docker Compose (4 services: bitcoind, relay, ponthtlc, frontend) | open |

### MVP definition of done

- `docker compose -f docker-compose.mvp.yml up --build` starts 4 services
- Two browser tabs (buyer + seller) complete a full escrow against one hosted node
- `make test-all` passes (happy/fail/hold via browser-driven integration tests)
- Restart restores channel state
- Preimage P never stored on the hosted node (pass-through only)

## Phase 3: Post-MVP

- `pontmored` integration: wire PontHTLC's LDK node as an economic connector
  in a [`pontmore/pontmore`](https://github.com/pontmore/pontmore) deployment
  (connector manifest matching `escrow_secure`/`escrow_settle`/`escrow_refund`/
  `resolve_dispute` effect types)
- CI/CD pipeline (GitHub Actions)
- Multi-escrow support (concurrent escrows)
- Public channel gossip / routing (beyond direct-peer)
- Production deployment (TLS, reverse proxy, auth)
- Monitoring / metrics / tracing
- Mobile-responsive frontend
- Multi-language UI (i18n)
- Kubernetes manifests

## Non-goals

- Custodial wallet (the buyer must run their own LDK node)
- Centralized matching / order book (P2P direct trades only)
- Altcoin support (Bitcoin + Lightning only for MVP)
- Regulatory compliance / KYC
