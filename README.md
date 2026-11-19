# PontHTLC

> Built for [Dakar Bitcoin Hack 2026](https://dakar-bitcoin-hack-2026.devpost.com/)

HTLC hold-invoice Bitcoin/Lightning escrow for P2P disputed trades,
designed for the African context where 10,000-sat trades must be economical.

Built on the [Pontmore protocol](https://github.com/pontmore/protocol)
[`pontmore/swap@1`](https://github.com/pontmore/protocol/blob/main/profiles/swap-v1.md)
profile (PIP-00 + PIP-01 + PIP-02 v2), with a 2-of-3 Shamir split preimage
extension and Nostr (`NIP-59`) as the transport layer.

**Hosted node architecture**: users don't run their own LDK node — a
service provider runs `ponthtlc` and users interact via browser + Nostr.
The host never needs `P` to operate the channel, but the seller who
sampled `P` can still withhold it. The GF(2^8) split is a private
payload, not a resolver-enforceable escrow. See
[docs/infrastructure.md](docs/infrastructure.md).

## Why this matters for Africa

P2P Bitcoin trading is dominant across Africa (WhatsApp groups, Telegram,
in-person cash trades), but existing tools are either:

- **Custodial** (the platform holds your keys — trust risk)
- **Manual** (no escrow — counterparty risk on every trade)
- **Expensive** (requiring each trader to run their own Lightning node)

PontHTLC solves this with a **hosted hold-invoice escrow** that works for
small trades:

- **No node to run**: a hosted LDK node handles channels; users use a browser
- **Host does not hold P**: the seller samples the preimage and splits it
  2-of-3 Shamir as a private payload. The host starts with zero shares, but
  the seller already knows `P` and can withhold it after fiat is paid
- **Dispute share**: the daemon (node operator) holds s3 as the resolver.
  The seller submits s3 to the daemon after funding via
  `POST /api/escrow/:id/arb-share`. During dispute resolution the daemon
  can release s3 so the buyer can reconstruct P. That share is not
  cut-and-choose verified; a bad s3 is only discovered when reconstruction
  fails. Resolver-recoverable preimages (RRP) are post-MVP
- **Low cost**: $0 for users; the operator's VPS serves hundreds of traders

## Workspace

| Crate | Role | Depends on |
| --- | --- | --- |
| [`ponthtlc`](crates/ponthtlc) | Hosted LDK node daemon — channel management, bitcoind sync, HTTP API, escrow store | — |
| [`ponthtlc-protocol`](crates/ponthtlc-protocol) | Pure PIP-00 + PIP-01 + PIP-02 + `swap@1` protocol layer — Nostr event construction, chain validation, state machine, fork detection, NIP-69 offers | — |
| [`ponthtlc-nostr`](crates/ponthtlc-nostr) | NIP-04 encryption, NIP-59 gift-wrap transport (signed rumor → seal → gift wrap), WebSocket relay client | `ponthtlc-protocol` |
| [`ponthtlc-escrow`](crates/ponthtlc-escrow) | LDK bridge — maps protocol state transitions to `create_inbound_payment_for_hash` / `claim_funds` / `fail_htlc_backwards`, plus 2-of-3 Shamir secret sharing | `ponthtlc-protocol`, `lightning`, `bitcoin`, `rand` |
| [`ponthtlc-spike`](crates/spike) | One-shot integration test — exercises the full LDK hold-invoice lifecycle against bitcoind regtest (happy / fail / hold) | `ponthtlc` |
| [`frontend/`](frontend) | Svelte + Vite escrow UI — client-side Shamir reconstruction, NIP-59 share exchange, WebSocket events | hosted `ponthtlc` API |

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

The relay is the coordination surface: on startup the daemon fetches
event chains for every known escrow and reconciles local state. HTTP
endpoints are thin wrappers that sign, persist, and publish events to
the relay. The `POST /api/escrow/:id/action` endpoint accepts
client-signed kind 7301 events so a browser holding the user's Nostr
key can submit actions without the daemon re-signing. Publish failures
retry (3 attempts with backoff).

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

### Run the MVP stack

One command starts bitcoind, the Nostr relay, the hosted daemon, and the frontend:

```bash
docker compose -f docker-compose.mvp.yml --profile bootstrap up --build
```

The `bootstrap` profile runs `channel-init`, which waits for bitcoind and
the daemon, funds the regtest `spike` wallet, opens a 1,000,000-sat channel
between the daemon's buyer and seller nodes, mines confirmations, and
exits once the channel is usable. Omit `--profile bootstrap` if you prefer
to open the channel manually.

`http://127.0.0.1:9800/health/live` returns `ok`. The escrow dashboard is at `http://127.0.0.1:5173`. It is a phone-width flow: Wallet, New swap, the swap (Lock, Pay fiat, Confirm, Release), Chat, Me, and Desk. Create a swap with the buyer's Lightning address. The other person funds it and leaves their own Lightning address. Settled sats are paid to the buyer's address. A failed hold is paid to the seller's address. There is no withdraw button. `make test-happy`, `make test-fail`, and `make test-hold` run the spike scenarios. `scripts/integration-test.sh` brings the stack up, checks health, then runs the relay, API, and restart tests.

### Run the spike

```bash
.\scripts\spike.ps1 -Scenario happy   # or fail | hold
```

A passing run ends with `SPIKE_RESULT: PASS scenario=<scenario>`.

### Open a channel (native daemon)

When bitcoind runs in Docker and the daemon runs natively, open the
buyer↔seller channel from the host:

```powershell
.\scripts\open-channel.ps1      # Windows
# make channel                  # Linux / macOS (bash)
```

The script is idempotent: it exits immediately if a usable channel
already exists, otherwise it funds the `spike` wallet, opens a
1,000,000-sat channel, mines confirmations, and waits until it is usable.
Set `CAPACITY_SATOSHIS`, `CONFIRMATIONS`, or `DAEMON_URL` to override
the defaults.

## Testing

```bash
docker compose up -d                    # start bitcoind regtest + Nostr relay

# unit tests (no infrastructure needed)
cargo test -p ponthtlc-protocol         # 14 tests: PIP stack, state machine, offers
cargo test -p ponthtlc-escrow           # 8 tests: Shamir, LDK bridge mock
cargo test -p ponthtlc-spike            # 15 tests: GF(2^8) Shamir math
cargo test -p ponthtlc --lib            # 24 tests: HTTP API, escrow store, channel-open route, auth, dispute, resolve

# integration tests (require Docker containers)
cargo run -p ponthtlc-spike -- --scenario happy    # LDK hold-invoice lifecycle
cargo run -p ponthtlc-spike -- --scenario fail     # fail_htlc_backwards → PaymentFailed
cargo run -p ponthtlc-spike -- --scenario hold     # expiry past claim_deadline
cargo test -p ponthtlc-nostr --test relay_integration  # NIP-59 round-trip via strfry

# frontend (no daemon required for unit tests)
cd frontend && npm test && npm run dev
```

## Documentation

| Document | Description |
| --- | --- |
| [docs/infrastructure.md](docs/infrastructure.md) | Hosted node architecture (cost model, trust model, preimage flow) |
| [docs/spike-runbook.md](docs/spike-runbook.md) | Spike design, scenario matrix, API notes |
| [ROADMAP.md](ROADMAP.md) | Project roadmap (spike → MVP → post-MVP) |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [AGENTS.md](AGENTS.md) | Instructions for AI coding agents |
| [LICENSE](LICENSE) | License (MIT) |

## Status

Issues #1–#5, #11, and #23–#47 are **merged**. The hosted daemon,
PIP stack, REST API, browser desks, multi-node channel topology, MVP
Docker Compose, event-driven relay model, and security hardening are
live. The daemon runs buyer and seller LDK nodes in one process,
holds s3 as the resolver, reconciles event chains from the relay on
startup, and accepts client-signed kind 7301 events via
`POST /api/escrow/:id/action`. See [ROADMAP.md](ROADMAP.md) for the
full status table.
