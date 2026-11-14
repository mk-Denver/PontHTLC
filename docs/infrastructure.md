# Infrastructure Architecture

## Problem

The spike proves the LDK hold-invoice primitive with both buyer and seller
running their own `ponthtlc` daemon. For an African P2P exchange handling
10,000-sat trades, requiring every user to run an LDK node + bitcoind sync
is uneconomical — the infrastructure cost exceeds the trade value.

## Solution: Hosted Node Service

A **service provider** runs the LDK node(s) and bitcoind. Users
interact via a lightweight web client (browser) + Nostr. The host does
not sample the preimage `P`, but the seller who does can withhold it.
The GF(2^8) 2-of-3 split is a **private payload**, not a
resolver-enforceable escrow (that would need RRP, which is post-MVP).

### What the host and the seller can and cannot do

| Concern | Resolution |
|---|---|
| Host sees preimage P at setup? | No. The seller samples P and keeps it. Shares move as private payloads. On claim, the browser POSTs P; the hosted node passes it through to `claim_funds(P)`. |
| Seller can withhold P? | **Yes.** The seller already knows P. After fiat is paid, withholding P until the hold expires refunds the seller and leaves the buyer without sats. The Shamir split does not stop that. |
| Host can reconstruct P from shares alone? | No. The host starts with zero shares. Settling still requires someone to present P (or a verified recoverable construction such as RRP, not implemented here). |
| Buyer can verify s3 before paying fiat? | **No (MVP).** There is no cut-and-choose, signed commitment, or DLEQ. A bad arbitrator share is only discovered when reconstruction fails. |
| Host can log P in transit? | Yes, same as any payment processor seeing a secret in flight. After a successful claim that P cannot authorize a new transfer. |
| User loses funds if host rugs? | The user risks **channel capacity** (locked in the channel the host manages). An in-flight HTLC that nobody claims fails back to the seller when the lock expires. |

### Roles

| Role | Who | Runs what | Cost |
|---|---|---|---|
| **Node operator** | Service provider (1 per region) | `ponthtlc` daemon + bitcoind + Nostr relay | ~$20-50/mo VPS |
| **Buyer** | User (browser) | Nostr keypair, receives s1, reconstructs P when a second share is released, calls claim/fail via API | $0 |
| **Seller** | User (browser) | Nostr keypair, samples P, holds all shares at split time, funds invoice via hosted node | $0 |
| **Arbitrator** | Nostr-only process | May hold s3. Cannot force settlement without a recoverable-preimage design | $0 |

### API identity and authentication

Every API call carries `X-Nostr-Pubkey` (or the same value on the
WebSocket query string). That header identifies the caller; it is **not
a signature**. With default settings, anyone who knows a party's pubkey
can act as that party on the hosted node. This is a deliberate MVP
shortcut — the host is the trust root for availability anyway — but it
must be understood before pointing the daemon at real sats.

The daemon accepts an optional NIP-98-style proof: a signed kind-27235
event in `X-Nostr-Auth` whose `u`/`method` tags match the request and
whose event id equals its own SHA-256. Start with `--require-auth true`
to make the proof mandatory; otherwise the header is verified when
present and ignored when absent. Signed authorization for
`swap/fiat_sent`, `core/secure`, and dispute resolution remains a
separate Nostr-protocol concern: those actions live on the 7300 chain,
not in HTTP headers.

### Trust model

The node operator is trusted for **availability** (uptime, channel
management, block sync). The host does not start with P, but the
**seller** does: after fiat moves, withholding P is a real failure mode
until RRP or another resolver-enforceable lock is adopted. Do not treat
the GF(2^8) split as "non-custodial by math."

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
1. Seller originates P (seller already knows P), splits 2-of-3 Shamir: s1→buyer, s2→seller, s3→arb
2. Buyer browser → POST /api/escrow/create → buyer-side node calls create_inbound_payment_for_hash(H)
3. Seller browser → POST /api/escrow/:id/fund → seller-side node pays through the internal channel
4. Hosted node → PaymentClaimable { preimage: None } fires (hold verified)
5. Seller delivers s1 (and may send s3) as a private payload; seller still holds P and s2
6. After fiat, seller releases s2 (or arb sends s3). Buyer reconstructs P client-side.
   If the seller withholds every path to P, the lock expires and sats return to the seller.
7. Buyer browser → POST /api/escrow/:id/claim { preimage: P }
8. Hosted node → claim_funds(P) → pass-through, P not stored by design
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
