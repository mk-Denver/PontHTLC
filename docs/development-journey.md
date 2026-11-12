# PontHTLC Development Journey — Context Handoff

## Project

**PontHTLC** — Non-custodial HTLC-based Bitcoin/Lightning escrow for P2P disputed trades, built for the [Dakar Bitcoin Hack 2026](https://dakar-bitcoin-hack-2026.devpost.com/). Built on the [Pontmore protocol](https://github.com/pontmore/protocol) `pontmore/swap@1` profile, with a 2-of-3 Shamir split preimage extension and Nostr (NIP-59) transport. Designed for the African context where small trades (via M-Pesa, mobile money, cash) must be economical — users don't run their own LDK node; a hosted service provider runs `ponthtlc` and users interact via browser + Nostr.

## Repository

- **GitHub**: https://github.com/mk-Denver/PontHTLC
- **Language**: Rust (backend) + Svelte 5 / TypeScript (frontend)
- **License**: MIT
- **Branch (current)**: `main` at `9d12852` — PRs #13, #14, #17, #18, #21, #38, #39 all merged. `feat/simplify-ui-swap-flow` branch has additional work (pontspark flow, NIP-69 offers, role-based UI) in PR #38 that was merged but the follow-up commits (accept/lock/fiat-sent/fiat-confirmed separation, agent/buyer mode toggle) are on the branch and not yet merged.

## Architecture

### Hosted node service

A service provider runs one `ponthtlc` daemon (LDK + bitcoind) serving many users via REST API + WebSocket. Users interact via browser SPA + Nostr. Non-custodiality is preserved because the preimage P is reconstructed client-side in the browser — the hosted node never stores P, only passes it through to `claim_funds(P)`.

### Crates

```
crates/spike/          — Week-1 de-risking spike (happy/fail/hold scenarios, all pass)
crates/ponthtlc/        — Hosted LDK daemon (multi-tenant, dual LDK nodes, REST + WS)
  src/bin/ponthtlc.rs   — Binary entry point (config parsing, dual node startup)
  src/node.rs           — LDK node wiring (ChainMonitor, ChannelManager, PeerManager)
  src/http.rs           — Axum REST + WebSocket server (all endpoints)
  src/dual.rs           — DualNode: buyer + seller LDK nodes in one process
  src/bitcoind.rs       — bitcoind RPC facade (regtest)
  src/tenancy.rs        — EscrowStore: per-user escrow records, event chains, balances
  src/lnurl.rs           — LNURL-pay URL parsing + invoice fetching
  src/settlement.rs     — Auto-payout planning (PaymentSent → buyer, PaymentFailed → seller)
  src/events.rs         — Event handler (FundingGenerationReady, PaymentClaimable, etc.)
  src/invoice.rs         — BOLT11 hold invoice builder + parser
crates/ponthtlc-protocol/ — PIP-00 + PIP-01 + PIP-02 + swap@1 protocol layer (no I/O)
  src/swap_v1.rs         — State machine, reconstruct(), 12 states, fork detection
  src/events.rs          — Event templates (root, action, offer, agent definition, descriptor)
  src/constants.rs        — Kind constants, timing, dispute classes
  src/offer.rs           — NIP-69 offer parsing, quote, root-against-offer validation
  src/event.rs            — Nostr event signing (Schnorr), canonical JSON, keypair
crates/ponthtlc-nostr/    — Nostr transport (NIP-04, NIP-59, relay WebSocket)
crates/ponthtlc-escrow/   — LDK bridge + Shamir secret sharing
```

### Frontend

```
frontend/
  src/App.svelte         — Router (6 routes)
  src/pages/flow/         — 6 screens:
    Wallet.svelte         — Buyer: balance + swaps list. Agent: desk dashboard + incoming swaps
    NewSwap.svelte        — Buyer: discover offers, select one, enter fiat amount, create proposal
    Swap.svelte           — Role-based: 4-step tracker, agent actions (accept/lock/confirm), buyer actions (fiat paid/claim)
    Chat.svelte           — In-swap messaging (BroadcastChannel)
    Me.svelte             — Mode toggle (Personal/Agent), name, Lightning address, Nostr pubkey
    Desk.svelte           — Agent: channel status, publish offers
  src/lib/
    api.ts                — REST + WebSocket API client
    wallet.svelte.ts      — Per-tab wallet (sessionStorage, isAgent flag)
    shamir.ts             — GF(2^8) Shamir split/reconstruct (client-side)
    router.svelte.ts     — Hash-based router
    nip59.ts             — NIP-59 gift wrap
    nostr.ts             — Key generation
    hex.ts               — Hex utilities
```

## API Endpoints

```
GET  /health/live
POST /api/escrow/create           — Buyer creates swap proposal (registers hold invoice)
GET  /api/escrows                 — List visible escrows (any with protocol_status)
GET  /api/escrow/:id              — Get escrow by ID (any caller)
GET  /api/escrow/:id/events       — Signed PIP-02 event chain
POST /api/escrow/:id/accept       — Agent accepts (core/accept)
POST /api/escrow/:id/lock         — Agent locks sats (pays hold invoice, core/secure)
POST /api/escrow/:id/fund        — Legacy: combines accept + lock (backward compat)
POST /api/escrow/:id/fiat-sent   — Buyer marks fiat paid (swap/fiat_sent)
POST /api/escrow/:id/fiat-confirmed — Agent confirms fiat (swap/fiat_confirmed + authorize_settlement)
POST /api/escrow/:id/claim       — Buyer reconstructs P, claims sats (core/settle)
POST /api/escrow/:id/fail        — Refund (core/authorize_refund + core/refund)
POST /api/escrow/:id/expire      — Expire unaccepted (core/expire)
POST /api/escrow/:id/dispute     — Open dispute (core/open_dispute)
POST /api/invoice/pay            — Pay arbitrary BOLT11
POST /api/channel/open           — Open internal channel
GET  /api/channel/status         — Channel status
GET  /api/node/info              — Node info
GET  /api/node/channels           — Channel list
GET  /api/balance                — Owed balance
POST /api/offer/publish          — Agent publishes NIP-69 offer
GET  /api/offers                 — List live offers
GET  /api/ws                     — WebSocket event stream
```

## PIP-02 Event Flow (fiat_to_btc)

```
1. Buyer: create  → root (kind 7300) + agent definition (30360) + descriptor (30361)
2. Agent: accept   → core/accept (kind 7301)
3. Agent: lock     → pays hold invoice → core/secure (7301)
4. Buyer: fiat-sent → swap/fiat_sent (7301)
5. Agent: fiat-confirmed → swap/fiat_confirmed + core/authorize_settlement (7301)
6. Buyer: claim    → reconstructs P from Shamir shares → claim_funds(P) → core/settle (7301)
```

## Swap Flow (pontmore/swap@1, fiat_to_btc)

```
Buyer creates proposal (registers hold invoice with payment hash H)
  → Agent accepts (core/accept)
  → Agent locks sats (pays hold invoice, core/secure)
  → Buyer sees "Locked in wallet" badge
  → Buyer marks "I've paid the fiat" (swap/fiat_sent)
  → Agent confirms "Fiat received" (swap/fiat_confirmed + core/authorize_settlement)
  → Buyer reconstructs P from 2 Shamir shares, claims sats (core/settle)
  → Settled

Refund path: Agent clicks Refund from secured state (core/authorize_refund + core/refund)
```

## Key Technical Decisions

1. **Lightning 0.2.7** (not 0.2.0) — API drift required multiple compile cycles. Key: `get_and_clear_pending_events` is test-only; use `process_pending_events_async` + `process_pending_htlc_forwards` + `peer_manager.process_events()`.
2. **`KVStoreSyncWrapper<Arc<FilesystemStore>>`** — bridges sync `FilesystemStore` to async persister.
3. **`NetworkGraph::new(Network, logger)`** — takes `Network`, not `BlockHash`.
4. **`lightning-invoice` default features are empty** — must add `features = ["std"]` for `current_timestamp()`.
5. **Channel value >= 1,000,000 sats** for 50,000,000 msat trade (default `max_htlc_value_in_flight_msat` is ~10%).
6. **DualNode** — buyer + seller LDK nodes in one process, connected by internal channel. The seller pays the buyer's hold invoice through the channel.
7. **Per-tab wallet** — `sessionStorage` gives each browser tab its own Nostr keypair, enabling two-tab local testing (buyer + agent).
8. **Shamir split** — GF(2^8) 2-of-3 shares: s1→Buyer, s2→Seller, s3→Arbitrator. Client-side reconstruction in browser.
9. **PIP-02 compliance** — root signed by customer key (proposer), agent signs `core/accept` (non-proposer). 4 distinct role keys derived from daemon's `nostr_secret`.
10. **Auto-payout** — settled sats route to buyer's Lightning address via LNURL-pay; failed sats route to seller's address.

## Test Status

- **Rust unit tests**: `cargo test -p ponthtlc --lib` — 21/21 pass (including reconstruct test)
- **Rust integration**: `cargo test -p ponthtlc --test api_happy` — passes against bitcoind regtest
- **Spike**: `cargo test -p ponthtlc-spike` — 15/15 pass
- **Frontend**: `npm test` — 15/15 pass, `npm run check` — 0 errors
- **Frontend E2E** (Playwright): not run in this session (tests exist in `frontend/tests/e2e.spec.ts`)

## Open Issues (from research audit)

Critical:
- #22 — Shamir split is not RRP; seller holds P and can withhold it
High:
- #23 — Wrong signers for btc_to_fiat direction
- #24 — Invoice expires before dispute window
- #25 — No API authentication (X-Nostr-Pubkey is not a signature)
- #26 — Fail/expire write chains the reducer rejects
- #27 — Dispute doesn't bind the root's resolver
- #28 — Agent definition + descriptor not published to relay; compose relay URL is wrong
- #29 — Money moves before chain is written (no durable intent)
Medium:
- #30 — Share transport leaks on BroadcastChannel
- #31 — Fiat payment is unproven
- #33 — Counterparty amounts not validated
- #34 — Six stale doc statements
Low:
- #32, #35, #36, #37 — Various post-MVP items

## Docker Compose

```
docker-compose.yml          — bitcoind regtest only (for spike)
docker-compose.mvp.yml      — bitcoind + nostr-relay (strfry) + ponthtlc + frontend (4 services)
```

## Build (Windows)

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build -p ponthtlc 2>&1"
```

## Build (Linux/macOS)

```bash
cargo build -p ponthtlc
```

## Run Locally

```bash
docker compose up -d                    # start bitcoind regtest
cargo run -p ponthtlc -- --data-dir spike-data --port 9800 --ln-port 9735 --seller-ln-port 9736
cd frontend && npm run dev              # frontend on :5173
```

## What's Left for the Hackathon

1. ~~Merge PR #19~~ (merged as `9d12852` includes LNURL payout + UI v1)
2. ~~Merge PR #38~~ (merged as `c5f1862` — simplified UI with pontspark flow, NIP-69 offers, role-based UI)
3. Merge the remaining `feat/simplify-ui-swap-flow` branch (accept/lock/fiat-sent/fiat-confirmed separation, agent mode toggle, isAgent detection from escrow owner)
4. Test the full flow end-to-end from two browser tabs
5. Record a demo video
6. Submit to Devpost

## Known Limitations (MVP)

- No authentication (X-Nostr-Pubkey is a plain header, not a signature)
- No RRP (seller holds P and can withhold it — documented in issue #22)
- No Nostr relay integration in docker-compose.mvp.yml (--relay-url not set)
- `btc_to_fiat` direction uses wrong signers (issue #23)
- Invoice expiry (3600s) is shorter than dispute window (7200s) (issue #24)
- No auto-escalation when fiat_sent but not confirmed by deadline