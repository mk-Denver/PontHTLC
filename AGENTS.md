# AGENTS.md

Instructions for AI coding agents (opencode, Claude, Copilot, Cursor, etc.)
working on this repository.

## Build commands

All builds on Windows require the MSVC environment:

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo build -p ponthtlc-spike 2>&1"
```

On Linux/macOS: `cargo build -p ponthtlc-spike`.

## Running the spike

```bash
docker compose up -d                    # start bitcoind regtest container
cargo run -p ponthtlc-spike -- --scenario happy
cargo run -p ponthtlc-spike -- --scenario fail
cargo run -p ponthtlc-spike -- --scenario hold
```

A passing run ends with `SPIKE_RESULT: PASS scenario=<scenario>`.

## Codebase structure

```
crates/spike/src/
  main.rs           orchestration: parse_config, run_scenario, event pump
  node.rs           LDK node wiring: ChainMonitor, ChannelManager, PeerManager
  bitcoind_proxy.rs  RPC facade: createrawtransaction → fundrawtransaction → sign
  shamir.rs         GF(2^8) 2-of-3 split/reconstruct
```

## Key constraints

- **No comments in code** unless explaining a non-obvious design decision.
- **No new dependencies** without checking that the codebase already uses the
  library (check `Cargo.toml` and neighboring files first).
- **lightning crate**: resolved to `0.2.7` (not `0.2.0`). API grepping:
  `C:\Users\HomePC\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\lightning-0.2.7\src\`
- **lightning-invoice**: default features are empty — `Cargo.toml` must
  specify `features = ["std"]` for `InvoiceBuilder::current_timestamp()`.
- **`get_and_clear_pending_events()`** is test-only in 0.2.7. Use
  `process_pending_events_async(handler)` + `process_pending_htlc_forwards()`
  + `peer_manager.process_events()` in the pump loop.
- **`KVStoreSyncWrapper<Arc<FilesystemStore>>`** wraps the persister store;
  `MonitorUpdatingPersisterAsync::new` takes it by value.
- **`NetworkGraph::new(Network, logger)`** — takes `Network`, not a block hash.
- **`fail_htlc_backwards`** returns `()` in 0.2.7, not `Result`.
- **`ProbabilisticScorer::new(decay, graph, logger)`** — arg order is
  (decay_params, network_graph, logger).
- **`RouteParametersConfig`**: import from `lightning::routing::router`, not
  `lightning::ln::channelmanager`.
- **`NodeSigner`** trait must be in scope for `KeysManager::get_peer_storage_key()`.
- Channel value must be >= 1,000,000 sats for a 50,000,000 msat trade
  (default `max_htlc_value_in_flight_msat` is ~10% of channel value).

## Pontmore protocol

We conform to `pontmore/swap@1` (Experimental, PIP-02 v2). Do not invent
new profiles. The 2-of-3 Shamir split is a private-payload extension.
See [docs/spike-runbook.md](docs/spike-runbook.md) for the LDK-to-kernel
action mapping table.

The reference implementation is [`pontmore/pontmore`](https://github.com/pontmore/pontmore)
(`pontmored` daemon + economic connector framework). Our MVP wraps LDK
primitives as connector-compatible typed effects (`escrow_secure`,
`escrow_settle`, `escrow_refund`, `resolve_dispute`) so a future
`pontmored` deployment can dispatch them as subprocess operations.

## Testing before committing

```bash
cargo build -p ponthtlc-spike
cargo run -p ponthtlc-spike -- --scenario happy   # must print SPIKE_RESULT: PASS
cargo run -p ponthtlc-spike -- --scenario fail
cargo run -p ponthtlc-spike -- --scenario hold
```
