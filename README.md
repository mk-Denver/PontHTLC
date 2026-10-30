# PontHTLC

> Built for [Dakar Bitcoin Hack 2026](https://dakar-bitcoin-hack-2026.devpost.com/)

Non-custodial HTLC-based Bitcoin/Lightning escrow for P2P disputed trades,
designed for the African context where 10,000-sat trades must be economical.

Built on the [Pontmore protocol](https://github.com/pontmore/protocol)
[`pontmore/swap@1`](https://github.com/pontmore/protocol/blob/main/profiles/swap-v1.md)
profile (PIP-00 + PIP-01 + PIP-02 v2), with a 2-of-3 Shamir split preimage
extension and Nostr (`NIP-59`) as the transport layer.

**Hosted node architecture**: users don't run their own LDK node — a
service provider runs `ponthtlc` and users interact via browser + Nostr.
Non-custodiality is preserved because the preimage is reconstructed
client-side. See [docs/infrastructure.md](docs/infrastructure.md).

## Why this matters for Africa

P2P Bitcoin trading is dominant across Africa (WhatsApp groups, Telegram,
in-person cash trades), but existing tools are either:

- **Custodial** (the platform holds your keys — trust risk)
- **Manual** (no escrow — counterparty risk on every trade)
- **Expensive** (requiring each trader to run their own Lightning node)

PontHTLC solves this with a **non-custodial escrow** that works for
small trades:

- **No node to run**: a hosted LDK node handles channels; users use a browser
- **No trust required**: the preimage is split 2-of-3 Shamir; no single party
  (including the host) can reconstruct it alone
- **Dispute resolution**: an arbitrator (Nostr-only) holds the tiebreaker share
- **Low cost**: $0 for users; the operator's VPS serves hundreds of traders

## Workspace

| Crate | Role | Depends on |
| --- | --- | --- |
| [`ponthtlc`](crates/ponthtlc) | Hosted LDK node daemon — channel management, bitcoind sync, HTTP API, escrow store | — |
| [`ponthtlc-protocol`](crates/ponthtlc-protocol) | Pure PIP-00 + PIP-01 + PIP-02 + `swap@1` protocol layer — Nostr event construction, chain validation, state machine, fork detection, NIP-69 offers | — |
| [`ponthtlc-nostr`](crates/ponthtlc-nostr) | NIP-04 encryption, NIP-59 gift-wrap transport (signed rumor → seal → gift wrap), WebSocket relay client | `ponthtlc-protocol` |
| [`ponthtlc-escrow`](crates/ponthtlc-escrow) | LDK bridge — maps protocol state transitions to `create_inbound_payment_for_hash` / `claim_funds` / `fail_htlc_backwards`, plus 2-of-3 Shamir secret sharing | `ponthtlc-protocol`, `ponthtlc` |
| [`ponthtlc-spike`](crates/spike) | One-shot integration test — exercises the full LDK hold-invoice lifecycle against bitcoind regtest (happy / fail / hold) | `ponthtlc` |

## Protocol stack

```
Layer 4: pontmore/swap@1  (swap profile)
   terms: direction, fiat amount/currency, bitcoin amount, deadlines
   roles: swap/agent, swap/customer
   actions: swap/fiat_sent, swap/fiat_confirmed
   sits on: PIP-02 kernel actions (core/accept, core/secure, core/settle, ...)
         │
Layer 3: PIP-02  (coordination event chains)
   kind 7300 (root): immutable coordination root with terms + participant p-tags
   kind 7301 (action): append-only linked actions, prev-chain validation
   kernel actions: core/accept, core/decline, core/cancel, core/secure,
      core/authorize_settlement, core/settle, core/authorize_refund,
      core/refund, core/expire, core/open_dispute, core/resolve_dispute
   invariants: one linear chain, no action after terminal, settle/refund
      mutually exclusive, escrow-only secure/settle/refund,
      resolver-only resolve_dispute, fork detection
         │
Layer 2: PIP-01  (escrow descriptor) — kind 30361
   escrow_type: lightning_hold_invoice, networks: ["lightning"], expires_at
         │
Layer 1: PIP-00  (agent definition) — kind 30360
   capability discovery: "I support pontmore/swap@1"
   references the escrow descriptor via ["a", "30361:..."]
```

Private payloads (Shamir shares, payment instructions, preimages) travel
via NIP-59 gift wrap (kind 1059 → seal kind 13 → signed rumor kind 7310).
The relay sees only ciphertext. Shares never appear in PIP-02 `terms` or
action `data` fields.

## Setup

### Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs))
- **Docker Desktop** (for the regtest `bitcoind` container)

### Windows

The LDK crates require MSVC + Windows SDK. On a fresh machine:

```powershell
# elevated PowerShell — installs Rust + VS Build Tools + Windows SDK
.\scripts\setup-windows.ps1
```

All `cargo` commands on Windows must go through the MSVC environment:

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\<edition>\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build --workspace 2>&1"
```

### Linux / macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
docker compose up -d
cargo run -p ponthtlc-spike -- --scenario happy
```

### Run the spike

```bash
.\scripts\spike.ps1 -Scenario happy   # or fail | hold
```

A passing run ends with `SPIKE_RESULT: PASS scenario=<scenario>`.

## Testing

```bash
docker compose up -d                    # start bitcoind regtest + Nostr relay

# unit tests (no infrastructure needed)
cargo test -p ponthtlc-protocol         # 14 tests: PIP stack, state machine, offers
cargo test -p ponthtlc-escrow           # 7 tests: Shamir, LDK bridge mock
cargo test -p ponthtlc-spike            # 15 tests: GF(2^8) Shamir math
cargo test -p ponthtlc --lib            # 5 tests: HTTP API, escrow store

# integration tests (require Docker containers)
cargo run -p ponthtlc-spike -- --scenario happy    # LDK hold-invoice lifecycle
cargo run -p ponthtlc-spike -- --scenario fail     # fail_htlc_backwards → PaymentFailed
cargo run -p ponthtlc-spike -- --scenario hold     # expiry past claim_deadline
cargo test -p ponthtlc-nostr --test relay_integration  # NIP-59 round-trip via strfry
```

## Documentation

| Document | Description |
| --- | --- |
| [docs/infrastructure.md](docs/infrastructure.md) | Hosted node architecture (cost model, trust model, preimage flow) |
| [docs/spike-runbook.md](docs/spike-runbook.md) | Spike design, scenario matrix, API notes |
| [ROADMAP.md](ROADMAP.md) | Project roadmap (spike → MVP → post-MVP) |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [AGENTS.md](AGENTS.md) | Instructions for AI coding agents |
| [LICENSE.md](LICENSE.md) | License (CC0-1.0) |

## Status

Issues #1 (hosted node daemon) and #2 (Pontmore PIP stack) are **merged**.
The protocol layer, Nostr transport, and escrow bridge are live with 44
passing tests across the workspace. See
[issues #3–#5](https://github.com/mk-Denver/PontHTLC/issues) for the
remaining MVP plan (REST API, frontend, Docker orchestration).
