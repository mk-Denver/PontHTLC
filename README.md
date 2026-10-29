# PontLDK

Non-custodial HTLC-based Bitcoin/Lightning escrow for P2P disputed trades, using
[Pontmore protocol](https://github.com/anomalyco/pontmore) descriptors (`PIP-01`, `PIP-02`,
`pontmore/escrow-htlc@1`), a 2-of-3 Shamir split preimage, and Nostr (`NIP-59` gift wraps)
as the transport among buyer / seller / notifier / arbitrator.

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
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\<edition>\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build -p pontldk-spike 2>&1"
```

### Linux / macOS

```bash
# install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# start the bitcoind container
docker compose up -d
# build and run the spike
cargo run -p pontldk-spike -- --scenario happy
```

### Running the spike

```bash
# from repo root
.\scripts\spike.ps1 -Scenario happy   # or fail | hold
```

## Architecture sketch (spike-validated parts in bold)

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

## Planned layout

```
crates/spike     manual claim demo + regtest bitcoind harness (current)
crates/shamir    GF(2^8) 2-of-3 preimage split / reconstruct (spike-validated)
crates/nostr     NIP-59 share/publish handling (planned)
crates/arbitrator Nostr-listen-only arbitrator (planned)
docs/            runbooks + ADRs (claim_deadline semantics observed during spike)
```
