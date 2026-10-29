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

Track via [GitHub issues #1–#5](https://github.com/mk-Denver/PontHTLC/issues).

| Issue | Component | Status |
| --- | --- | --- |
| #1 | HTLC node daemon (persistent, restart recovery) | open |
| #2 | Nostr escrow protocol (NIP-59 + `pontmore/swap@1` state machine) | open |
| #3 | Backend API server (REST + WebSocket) | open |
| #4 | Frontend web UI (escrow dashboard) | open |
| #5 | Docker Compose orchestration + integration tests | open |

### MVP definition of done

- `docker compose -f docker-compose.mvp.yml up --build` starts all services
- Two browser tabs (buyer + seller) complete a full escrow end-to-end
- `make test-all` passes (happy/fail/hold integration tests via API)
- Restart restores channel state from disk

## Phase 3: Post-MVP

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
