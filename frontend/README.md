# PontHTLC frontend

Svelte + Vite escrow UI for issue #4. The browser reconstructs the
preimage and talks to the hosted `ponthtlc` daemon.

```bash
# terminal 1 — hosted node (requires bitcoind regtest)
cargo run -p ponthtlc -- --data-dir test-data

# terminal 2
cd frontend
npm install
npm test
npm run dev
```

Open http://localhost:5173 in two tabs. Pick **buyer** in one and
**seller** in the other. Vite proxies `/api` and `/api/ws` to
`127.0.0.1:9800`.

## What this tab does

- Buyer: create a hold invoice from the seller’s payment hash, watch
  `PaymentClaimable`, reconstruct `P` from two Shamir shares, claim or fail
- Seller: originate `P`, send `s1` as a NIP-59 gift wrap, fund the
  invoice, release `s2` after cash is received
- Either tab: open a dispute and copy the arbitrator gift-wraps

Shares travel as NIP-59 wraps over `BroadcastChannel` so two tabs on
this origin work without a relay. Set a relay URL to also publish
kind 1059 events.

`P` is kept in memory for the claim request only. It is not written to
`sessionStorage`.
