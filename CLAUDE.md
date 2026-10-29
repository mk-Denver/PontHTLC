# CLAUDE.md

Specific instructions for Claude when working on this repository.

## You are

Claude, an AI coding assistant. Follow [AGENTS.md](AGENTS.md) for
general agent instructions. This file adds Claude-specific notes.

## Build environment

Windows with MSVC. Every `cargo` command must be prefixed with the
vcvars64 wrapper:

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo <cmd> 2>&1"
```

The Windows SDK 10.0.26100.0 is installed. If a build fails with linker
errors, the SDK is already present — the issue is in the source code.

## lightning 0.2.7 API gotchas

These cost significant debugging time during the spike:

| Issue | Fix |
| --- | --- |
| `get_and_clear_pending_events()` is `#[cfg(test)]` | Use `process_pending_events_async(handler)` instead |
| `process_pending_htlc_forwards()` is separate | Call it in the pump loop to fire `PaymentClaimable` |
| `peer_manager.process_events()` drains `pending_msg_events` | Without it, `SendOpenChannel` never reaches the peer |
| `KVStoreSyncWrapper` wraps `FilesystemStore` for async persister | `KVStoreSyncWrapper(Arc::new(FilesystemStore::new(...)))` |
| `NetworkGraph::new` takes `Network`, not `BlockHash` | `NetworkGraph::new(Network::Regtest, logger)` |
| `fail_htlc_backwards` returns `()` | No `map_err` needed |
| `InvoiceBuilder::current_timestamp` needs `std` feature | `lightning-invoice = { features = ["std"] }` |
| `ProbabilisticScorer::new` arg order | `(decay_params, network_graph, logger)` |
| `RouteParametersConfig` is private in channelmanager | Import from `lightning::routing::router` |
| `NodeSigner` trait for `get_peer_storage_key` | `use lightning::sign::NodeSigner;` |

## Scenario verification

After any change to `main.rs`, `node.rs`, or `bitcoind_proxy.rs`:

```powershell
cmd /c "call ""C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"" >nul && cargo run -p ponthtlc-spike -- --scenario happy 2>&1 | findstr SPIKE_"
```

Must see `SPIKE_RESULT: PASS scenario=Happy`. If not, check the full
output (not just findstr) for the failure reason.
