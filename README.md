# PontHTLC

Non-custodial HTLC-based Bitcoin/Lightning escrow for P2P disputed trades, using
the [Pontmore protocol](https://github.com/pontmore/protocol) `pontmore/swap@1`
profile as the coordination base, a 2-of-3 Shamir split preimage extension, and
Nostr (`NIP-59` gift wraps) as the transport among buyer / seller / arbitrator.

We do **not** invent a new Pontmore profile. We conform to the existing
[`pontmore/swap@1`](https://github.com/pontmore/protocol/blob/main/profiles/swap-v1.md)
profile (Experimental, PIP-02 v2), which already defines the escrow kernel
(`core/secure`, `core/settle`, `core/refund`, `core/authorize_settlement`,
`core/authorize_refund`, `core/open_dispute`, `core/resolve_dispute`) and
supports Lightning as a `bitcoin.network` value. Our 2-of-3 Shamir split
preimage is an implementation-private extension to the swap profile's
private-payload boundary — it does not alter the public root or action schema.

## Current status: Week 1 de-risking spike

The first goal is proving the LDK primitive that every one else depends on: a **manual-claim
hold invoice** registered at the buyer's node where the buyer never holds the preimage.

- Spike design and run instructions: [`docs/spike-runbook.md`](docs/spike-runbook.md)
- Spike crate: [`crates/spike`](crates/spike) (binary `spike`)

## Prerequisites

### All platforms

- **Rust** (stable, via [rustup](https://rustup.rs))
- **Docker Desktop** (for the regtest `bitcoind` container)

### Windows

The LDK crates link native C/C++ via `secp256k1` and `bitcoin_hashes`, which requires
MSVC + Windows SDK. On a fresh Windows machine:

```powershell
# from an elevated PowerShell — installs Rust + VS Build Tools + Windows SDK
.\scripts\setup-windows.ps1
```

If you already have Visual Studio 2022+ installed with the "Desktop development with C++"
workload, you only need the Windows 11 SDK (10.0.26100.0) component.

All `cargo build` / `cargo run` commands on Windows must go through the MSVC environment:

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\<edition>\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build -p ponthtlc-spike 2>&1"
```

### Linux / macOS

```bash
# install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# start the bitcoind container
docker compose up -d
# build and run the spike
cargo run -p ponthtlc-spike -- --scenario happy
```

### Running the spike

```bash
# from repo root
.\scripts\spike.ps1 -Scenario happy   # or fail | hold
```

## Architecture sketch (spike-validated parts in bold)

Mapped to `pontmore/swap@1` kernel actions:

1. **Seller originates a random 32-byte preimage `P` and publishes `H=sha256(P)` via Nostr.**
2. Seller splits `P` into 2-of-3 GF(2^8) Shamir shares:
   - s1 → Buyer, s2 → Seller (self-kept), s3 → Arbitrator (Nostr-only participant)
   - no single party alone can reconstruct `P`; only two quorum shares reconstruct.
3. **Buyer node registers `H` (it does not know `P`) and signs a BOLT11 hold invoice.**
4. Seller funds the invoice with their LDK wallet; the HTLC stays in-flight.
5. On a dispute-free close, buyer reconstructs `P` from two shares and calls
   `claim_funds(P)`; on a dispute, they call `fail_htlc_backwards`; arbitrator holds share
   s3 as tiebreaker.
6. Seller trusts the buyer node (per our dispute contract); the arbitrator is a nostr-only
   (non-listening) participant and cannot fund/settle unilaterally.

### LDK primitive → `pontmore/swap@1` kernel mapping

| LDK primitive | `pontmore/swap@1` action | Effect |
| --- | --- | --- |
| `create_inbound_payment_for_hash(H)` | (setup) | Buyer registers hold invoice |
| `pay_for_bolt11_invoice(...)` | `core/secure` | HTLC funded and in-flight |
| `PaymentClaimable { preimage: None }` | (secured state) | Hold verified — no auto-claim |
| `claim_funds(P)` | `core/authorize_settlement` + `core/settle` | Buyer releases preimage; seller sees `PaymentSent` |
| `fail_htlc_backwards(H)` | `core/authorize_refund` + `core/refund` | Buyer rejects; seller sees `PaymentFailed` |
| mine past `claim_deadline` | `core/expire` | HTLC auto-fails or manual note |
| arbitrator delivers s3 via Nostr | `core/open_dispute` + `core/resolve_dispute` | Dispute resolution with tiebreaker share |

## Planned layout

```
crates/spike       manual claim demo + regtest bitcoind harness (current)
crates/ponthtlc    persistent LDK node daemon (planned, issue #1)
crates/shamir      GF(2^8) 2-of-3 preimage split / reconstruct (spike-validated)
crates/nostr       NIP-59 share/publish handling (planned, issue #2)
crates/escrow      pontmore/swap@1 state machine + Shamir coordination (planned, issue #2)
crates/api         REST + WebSocket server (planned, issue #3)
frontend/          web UI (planned, issue #4)
docs/              runbooks + ADRs (claim_deadline semantics observed during spike)
```
