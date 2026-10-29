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

### Fee model

There are **two fee models** depending on the topology:

#### MVP (single hosted node, direct channel)

The LDK routing fee does **not** apply because the buyer's node is the
final hop (payee), not a forwarding node. `skimmed_fee_msat` is only
calculated when forwarding to a next hop (`outgoing_amt - amt_to_forward`).
On a direct channel, the buyer receives the full trade amount.

The node operator charges a **platform service fee** at the API layer:

| Fee | Amount | Who pays | When |
|---|---|---|---|
| Escrow creation | 100 msat (0.1 sat) | Seller | `POST /api/escrow/create` |
| Escrow claim | 100 msat (0.1 sat) | Buyer | `POST /api/escrow/:id/claim` |
| Escrow fail | 100 msat (0.1 sat) | Buyer | `POST /api/escrow/:id/fail` |

The API deducts the fee from the trade amount before calling the LDK
primitive. Example for a 10,000 sat trade:

```
Seller funds HTLC:       10,000,000 msat (10,000 sats)
API deducts fee:           -100 msat (0.1 sat)  [creation fee from seller]
Buyer receives claim:     10,000,000 msat (full amount, LDK direct)
API deducts fee:           -100 msat (0.1 sat)  [claim fee from buyer]
Buyer net received:        9,999,900 msat (9,999.9 sats)
Node operator earns:         200 msat (0.2 sats)  [per complete trade]
```

The fee is **not** a Lightning routing fee — it is a platform fee for
using the hosted escrow infrastructure. It is deducted at the API layer,
not the LDK layer, so LDK sees the full HTLC amount.

At ~200 msat (0.2 sats) per trade and ~100 trades/day per channel,
the operator earns ~20 sats/day per active channel. At 500 channels,
that's ~10,000 sats/month — covering a $20-50/mo VPS.

#### Production (multi-hop, multiple providers)

When buyer and seller are on **different providers**, the payment
routes through the Lightning network. Each forwarding node (including
ours) charges a Lightning routing fee:

| Parameter | Value | Effect |
|---|---|---|
| `fee_base_msat` | 1000 | 1 sat base fee per forwarded HTLC |
| `fee_proportional_millionths` | 100 | 100 ppm = 0.01% of forwarded amount |

The sender pays all hop fees. Our node skims `fee_base + amount * ppm`
when forwarding to the next hop. This is standard Lightning economics —
no separate platform fee needed.

**How it works**: the hosted node configures the channel with a
routing fee (`fee_base_msat` + `fee_proportional_millionths`). When
the seller funds the HTLC, the LDK protocol automatically deducts
the fee from the amount the buyer receives on settlement.

| Parameter | Value | Effect |
|---|---|---|
| `fee_base_msat` | 1000 | 1 sat base fee per HTLC (covers channel overhead) |
| `fee_proportional_millionths` | 100 | 100 ppm = 0.01% of trade amount |

**Examples**:

| Trade | HTLC amount | Fee | Buyer receives | Node earns |
|---|---|---|---|---|
| 10,000 sats | 10,000,000 msat | 2,000 msat (20 sats) | 9,998,000 msat | 20 sats |
| 50,000 sats | 50,000,000 msat | 6,000 msat (60 sats) | 49,994,000 msat | 60 sats |
| 100,000 sats | 100,000,000 msat | 11,000 msat (110 sats) | 99,989,000 msat | 110 sats |

**Fee formula**: `fee = fee_base_msat + (amount_msat * fee_proportional_millionths / 1_000_000)`

- 10k sat trade: 0.2% fee (20 sats) — economical for African P2P
- 50k sat trade: 0.12% fee (60 sats)
- 100k sat trade: 0.11% fee (110 sats)

The fee is **already in the LDK channel config** (`ChannelHandshakeConfig` /
`UserConfig`). The spike already sets `fee_base_msat: 1000` and
`fee_proportional_millionths: 0` in `node.rs`. For production, set
`fee_proportional_millionths: 100` (100 ppm). No new code needed —
just a config change.

**Who pays**: the fee is deducted from the **buyer's received amount**
(the payee side), same as a Lightning routing fee. The seller sends the
full trade amount; the node operator keeps the fee; the buyer receives
the amount minus the fee.

### Cost model### Cost model for 10k sat trades

- Node operator: 1 VPS (~$20-50/mo) serves hundreds of users
- User: $0 (browser + free Nostr account)
- Per-trade: ~10k sats locked in channel (seller funds; returned on settle/refund)
- MVP fee: 0.2 sats per trade (platform service fee, API layer)
- Production fee: Lightning routing fee (LDK layer, multi-hop only)
- No subscription, no upfront payment — fee is per-trade
