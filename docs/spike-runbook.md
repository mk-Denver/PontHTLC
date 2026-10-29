# Week 1 de-risking spike: manual claim on a buyer-held hash

## Purpose

Validate the core primitive of the PontHTLC architecture before building the Nostr layer:

> The buyer's LDK node registers `H = sha256(P)` from an HTLC's DHT preimage that it does NOT know (via `create_inbound_payment_for_hash`), signs a BOLT11 hold invoice with the preimage unknown, and keeps the HTLC in-flight until a manual `claim_funds(...)` / `fail_htlc_backwards(...)` call, or LDK expiry semantics take over past `claim_deadline`.

If this spike passes, the non-custodial HTLC escrow design (PIP-1: 2-of-3 shares of preimage, arb as a Nostr-only participant, seller-funded / buyer-held invoices) may proceed.

## How to run

Prerequisites: Docker Desktop running, Rust toolchain installed.

```powershell
# from repo root
.\scripts\spike.ps1 -Scenario happy   # or fail | hold
```

The script starts a regtest `bitcoind` container (RPC port `18923`, user/pass `ponthtlc`), then builds and runs the `spike` binary with the chosen scenario.

Scenario flags in the binary (also standalone runnable):

```
cargo run -p spike -- --scenario happy
cargo run -p spike -- --scenario fail
cargo run -p spike -- --scenario hold
```

## What is validated

| Step | Scenario/s | Assertion | Marker |
| --- | --- | --- | --- |
| 0-connect | all | seller and buyer peers connected | `SPIKE_PASS: 0-connect` |
| 1-open-channel | all | direct channel off-chain ready (12 blocks) | `SPIKE_PASS: 1-open-channel` |
| 2-register-invoice | all | buyer signs invoice with a hash it does not know the preimage of | `SPIKE_INFO: hold invoice` |
| 3-payment-claimable | all | incoming HTLC arrives; `Event::PaymentClaimable` arrives at buyer with `payment_preimage: None` — no auto-claim | `SPIKE_PASS: 3-payment-claimable` |
| 4a-shamir-reconstruction | happy | 2 shares reconstruct `P`, sha256(P)==H | `SPIKE_PASS: 4a-shamir-reconstruction` |
| 4b-release-claim | happy | `claim_funds` → `PaymentClaimed` at buyer + `PaymentSent` with preimage discovery at seller | `SPIKE_PASS: 4b-release-claim` |
| 4c-manual-fail | fail | `fail_htlc_backwards` → `PaymentFailed` at seller | `SPIKE_PASS: 4c-manual-fail` |
| 4d-hold-expiry | hold | mine past `claim_deadline`; observe LDK auto-fail semantics | `SPIKE_PASS` + `SPIKE_NOTE` or `SPIKE_INFO` |

## Scenario semantics

- `happy`: seller funds the invoice; buyer releases their share via off-band channel (obviously in-spike: shares passed in-memory between the two nodes), reconstruction verified, `claim_funds(P)` with proof-verified preimage; seller's outbound payment auto-recovers preimage.
- `fail`: dispute semantics — buyer calls `fail_htlc_backwards`; HTLC fails back to seller's outbound as a `PaymentFailed`.
- `hold`: the HTLC stays claimed by nothing, and passes beyond `claim_deadline`; note in the ADR whether LDK dark-failed automatically, or not.

## API surface notes (lightning ~0.2 / lightning-invoice ~0.34)

- `channel_manager::create_inbound_payment_for_hash(hash, Some(amount), expiry_secs, None)` → registers the hash with a secret only; returns `PaymentSecret`. Payment preimage is NOT known, auto-claim cannot occur.
- The intel: respond with a BOLT11 invoice signed by the buyer's node key:
  `lightning_invoice::InvoiceBuilder::new(...).payment_hash(hash).payment_secret(buyer_secret).min_final_cltv_expiry_delta(40).build_signed(...)`
- `claim_funds(PaymentPreimage)` / `fail_htlc_backwards(&PaymentHash)` both manual selection at the current `PaymentClaimable` holder node only.
- `claim_deadline` in `PaymentClaimable` (Option<u32> blockheight) is from where the buyer node's auto-cancel buffer derives; see ADR-0001 for full ref.

## What this spike does NOT validate (explicit limits)

1. The Nostr / NIP-59 share exchange and arb key handshakes (out of scope — shares are passed in-memory).
2. `create_channel` of the seller fund node to ANY wallet (a self-custody wallet like phoenix would auto-claim; buyer wallet MUST be a raw node — that's the design: buyer-as-own-escrow-capable wallet).
3. Multi-node routing via gossip (we configured no-use of network graph discovery for route finding).
4. Full crash recovery / restart persistence — node data dirs are wiped per-run.

## Known tradeoffs taken

- `ChainMonitor/ChainManager` fresh-state only (no restore from filesystem) — each run does `remove_dir_all(spike-data)`.
- No `lightning_background_processsor` — a manual event pump loops `get_and_clear_pending_events` + `chain_monitor.process_pending_events()` every 20 ms.
- No gossip: `route_handler`/`onion_message_handler`/`custom_message_handler` are `IgnoringMessageHandler`.
- Fees are hard-coded constants rather than `estimatesmartfee`.

## Pontmore reference implementation

The [`pontmore/pontmore`](https://github.com/pontmore/pontmore) repo is the reference
implementation of the Pontmore protocol (`pontmored` daemon + economic
connector framework). It defines the production architecture:

- `pontmored` — PostgreSQL-backed durable signed-event storage, SSE
  event stream (`GET /v1/stream`), `POST /v1/events` ingestion
- Economic connectors — `escrow_secure`, `escrow_settle`, `escrow_refund`,
  `resolve_dispute` as typed subprocess effects (see
  [`deploy/connectors.example.json`](https://github.com/pontmore/pontmore/blob/main/deploy/connectors.example.json))
- `pontmore-tck` — compatibility vector runner
- PIP-02 boundary: `validate_swap_coordination` accepts signed kind
  `7300`/`7301` events + `DescriptorBinding`

Our MVP does not replicate the full `pontmored` stack. Instead, the
escrow state machine (issue #2) wraps LDK primitive calls as
connector-compatible typed effects matching the `pontmored` connector
manifest schema, so a future `pontmored` deployment can dispatch
PontHTLC's LDK node as an economic connector subprocess.

Protocol baseline pinned to Pontmore protocol commit
`d9a1eb3e8a24e2101f5a4024df1ce85a8d934f75` (2026-09-14).
