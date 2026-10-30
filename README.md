# PontHTLC

> Built for [Dakar Bitcoin Hack 2026](https://dakar-bitcoin-hack-2026.devpost.com/)

Non-custodial HTLC-based Bitcoin/Lightning escrow for P2P disputed trades,
designed for the African context where 10,000-sat trades must be economical.

Built on the [Pontmore protocol](https://github.com/pontmore/protocol)
[`pontmore/swap@1`](https://github.com/pontmore/protocol/blob/main/profiles/swap-v1.md)
profile, with a 2-of-3 Shamir split preimage extension and Nostr
(`NIP-59`) as the transport layer.

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
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\<edition>\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build -p ponthtlc-spike 2>&1"
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

Week 1 de-risking spike: **passed**. All three scenarios (happy/fail/hold)
validate the LDK manual-claim hold-invoice primitive against `lightning 0.2.7` on
regtest. See [issues #1–#5](https://github.com/mk-Denver/PontHTLC/issues)
for the MVP plan.
