# Infrastructure Architecture

## Problem

The spike proves the LDK hold-invoice primitive with both buyer and seller
running their own `ponthtlc` daemon. For an African P2P exchange handling
10,000-sat trades, requiring every user to run an LDK node + bitcoind sync
is uneconomical — the infrastructure cost exceeds the trade value.

## Solution: Hosted Node Service

A **service provider** runs the LDK node(s). and bitcoind. Users
interact via a lightweight web client (browser) + Nostr. Non-custodiality
is preserved because the hosted node never knows the preimage `P`.

### How non-custodiality survives without running your own node

| Concern | Resolution |
|---|---|
| Host sees preimage P? | No. P is reconstructed **client-side** in the browser from Shamir shares (s1+s2). The browser POSTs P to the API; the hosted node passes P through to `claim_funds(P)` without storing it. P exists in transit only. |
| Host can steal HTLC funds? | No. Settling the HTLC requires P. P is split 2-of-3. The host has 0 shares. The host cannot reconstruct P. |
| Host can claim/fail without user? | No. API calls require the user's Nostr-key-signed authorization. The host's LDK node only executes what the user's API request instructs. |
| Host can log P and replay later? | They could log P in transit (same trust as any payment processor seeing a card number). But a logged P is useless post-claim — the HTLC is already settled. P cannot authorize a new transfer. |
| User loses funds if host rugs? | The user risks **channel capacity** (locked in the channel the host manages). The **in-flight HTLC** is safe — it fails back to the seller on rug. For 10k sats this risk is acceptable. same as any custodial Lightning wallet. but with the HTLC layer being non-custodial. |

### Roles

| Role | Who | Runs what | Cost |
|---|---|---|---|
| **Node operator** | Service provider (1 per region) | `ponthtlc` daemon + bitcoind + Nostr relay | ~$20-50/mo VPS |
| **Buyer** | User (browser) | Nostr keypair, receives s1, reconstructs P, calls claim/fail via API | $0 |
| **Seller** | User (browser) | Nostr keypair, holds s2, funds invoice via hosted node | $0 |
| **Arbitrator** | Nostr-only process | Holds s3, resolves disputes | $0 |

### Trust model

The node operator is trusted for **availability** (uptime, channel
management, block sync) but NOT for **custody** (they cannot
reconstruct P, cannot claim/fail without the user's signed API call,
cannot steal in-flight HTLC funds).

### Pontmore protocol alignment

In `pontmore/swap@1` terms:
- `swap/agent` = the hosted node service (provides Lightning infrastructure)
- `swap/customer` = the user (provides authorization via Nostr key + Shamir shares)
- `core/escrow` = the LDK channel (managed by the hosted node)
- `core/resolver` = the arbitrator (Nostr-only)

The [`pontmore/pontmore`](https://github.com/pontmore/pontmore) reference implementation's
`pontmored` daemon is exactly this pattern: a central service with
economic connectors. PontHTLC's hosted node is an LDK-backed
connector that a future `pontmored` deployment can dispatch.

### Channel topology (MVP)

```
Buyer browser ──API──> Hosted ponthtlc node (buyer side)
                              │
                              ├── LDK channel ──> Hosted ponthtlc node (seller side)
                              │                     │
                              └── bitcoind regtest ──┘
```

Both buyer and seller LDK nodes run on the same host (single provider
for MVP). The channel is internal but is a real HTLC. For production,
different providers can peer via Lightning network.

### Preimage flow

```
1. Seller originates P, splits 2-of-3 Shamir: s1→buyer, s2→seller, s3→arb
2. Buyer browser → POST /api/escrow/create → hosted node calls create_inbound_payment_for_hash(H)
3. Seller browser → POST /api/escrow/:id/fund → hosted node calls pay_for_bolt11_invoice
4. Hosted node → PaymentClaimable { preimage: None } fires (hold verified)
5. Seller delivers s1 to buyer via Nostr NIP-59 (encrypted, host sees ciphertext only)
6. Buyer browser reconstructs P = combine(s1, s2)  [client-side, s2 obtained via Nostr or direct]
7. Buyer browser → POST /api/escrow/:id/claim { preimage: P }
8. Hosted node → claim_funds(P) → pass-through, P not stored
9. Seller's hosted node → PaymentSent (preimage auto-recovered)
```

### What this changes vs the spike

| Spike (Week 1) | MVP (hosted) |
|---|---|
| Both parties run `ponthtlc` daemon in-process | One hosted `ponthtlc` daemon serves many users |
| Shamir shares passed in-memory | Shares delivered via Nostr NIP-59 (encrypted) |
| Preimage reconstruction in Rust | Preimage reconstruction in browser (client-side) |
| Event pump in Rust loop | Event pump in hosted daemon; user sees events via WebSocket |
| Each run wipes data dirs | Data dirs persist; channels survive restart |
