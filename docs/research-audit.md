# Research audit: pontmore/research against PontHTLC

Audit date: 4 November 2026. This document is the deliverable for issue 20. It records gaps, failure modes, and improvements. It does not change protocol code, application code, or the research notes it cites.

## What was read

Research sources are `pontmore/research` on GitHub `main`, fetched 4 November 2026:

| Source | What it is |
| --- | --- |
| `escrow-policies-and-exchange-patterns.md` | Mechanism taxonomy, property checks, exchange patterns. Snapshot against protocol `d9a1eb3` (14 September 2026). |
| `implementation.md` | Reference-node shape: deterministic core, `pontmored`, TCK, closed connector subprocesses. Protocol pin `d9a1eb3`. |
| `resolver-recoverable-preimage.md` | RRP draft. Cut-and-choose, scalar Shamir, DLEQ. Not a PIP. |
| `rrp/rrp.mjs`, `rrp/vectors.json` | Reference construction and test vectors for that draft. |
| `mostro.md`, `mostro-pontmore-leverage.md` | Mostro at `f1f7a62` / protocol `1ee50e4`. |
| `peer.md` | Peer / ZKP2P comparison. Protocol pin `edcf137` (5 October 2026). |
| `openbitcoin-africa-p2p-escrow.md` | Problem statement: a lock is not yet a fair escrow. |
| `bisq.md` | Bisq 1, Bisq Easy, MuSig. Wiki and `bisq-musig` `a9a9770`. |
| `openswap.md` | Coinswap marketplace. OpenSwap `08f9194`, spec `c747759`. |
| `vexl.md` | Social-graph marketplace. `vexl-it/vexl` `2b37c4e`. |
| `openpleb-pontmore-interop.md` | Delegated merchant payment. OpenPleb `f4c91e1`, Thârk `fb3d44d`, protocol `edcf137`. |
| `peerbadge.md` | Optional credentials. PeerBadge `f458f46`. Protocol pin `d9a1eb3`. |
| `sed.md` | Unmerged Service Discovery PR at `d0c82129`. |
| `plugins.md` | Same connector decision as `implementation.md`. |

The issue text says fifteen documents. The table in the issue lists fourteen files. The fifteenth item on the research tree is the `rrp/` directory (a script and vectors), which belongs to the RRP note. This audit covers all of them.

PontHTLC facts below are from `origin/main` at `06d014a` ("Publish PIP-02 Nostr events for every escrow action", pull request 17, merged 3 November 2026), plus the six commits that land with it: multi-node daemon (`1b7cd8c`, `1776869`), wallet load (`1c6b69e`), and the MVP compose file (`a553317`, `ae3b0ad`). The working tree this file was written in is `be4bc1f`, six commits behind `origin/main`. Reviewers should read the findings against `06d014a`.

The local `ponthtlc-protocol` reducer was read from the checkout. Those rules are unchanged by the six commits: `swap_v1.rs` still rejects an early `core/expire`, an early `core/authorize_refund`, and a `swap/fiat_sent` signed by the wrong role.

## How PontHTLC behaves on origin/main

The reducer in `crates/ponthtlc-protocol` can validate a `pontmore/swap@1` chain. The HTTP daemon on `06d014a` also builds and stores events. The browser still does not sign those events, and the daemon does not wait for the reducer before it moves Lightning or marks a local status.

Create (`POST /api/escrow/create`):

- The caller is identified by the header `X-Nostr-Pubkey`, or by the same value on the WebSocket query string. The header is not a signature.
- The daemon opens a hold invoice with `create_inbound_payment_for_hash`. Invoice expiry and the LDK expiry argument are both 3600 seconds. `min_final_cltv_expiry_delta` is 40.
- It derives four keys from the daemon Nostr seed: agent is the seed, customer is `seed[0] ^= 0x01`, escrow is `^= 0x02`, resolver is `^= 0x03`.
- The root signer is the derived customer. Terms default to direction `fiat_to_btc`, fiat `1` `USD`, channel `lightning-hold-invoice@1`. Deadlines are `expires_at = now + 1800`, `fiat_pay_by = now + 3600`, `fiat_confirm_by = now + 7200`.
- Local `protocol_status` becomes `proposed`. Agent definition, descriptor, and root are appended locally. Only the root is sent to the relay.
- `publish_event` ignores connect and publish errors. `append_event` errors are discarded with `let _ =`.
- The process default `--relay-url` is `ws://127.0.0.1:7777`, and the flag is stored as `Some`. The MVP container command in `docker-compose.mvp.yml` does not pass `--relay-url`.

Fund publishes `core/accept` (agent key) and `core/secure` (escrow key) and sets `protocol_status` to `secured`. The hold invoice already exists from create.

Claim checks `sha256(preimage)` against the stored hash, calls `claim` on the node, then signs, in order, `swap/fiat_sent`, `swap/fiat_confirmed`, `core/authorize_settlement`, and `core/settle`. The payment reference on both fiat actions is the payment-hash hex string. Local status becomes `settled`.

`secret_for_action` does not look at direction. `swap/fiat_sent` is always the customer key. `swap/fiat_confirmed`, `core/authorize_settlement`, `core/authorize_refund`, `core/expire`, and `core/open_dispute` are always the agent key. `core/secure`, `core/settle`, and `core/refund` are the escrow key. For the default `fiat_to_btc` that matches `fiat_sender` / `fiat_receiver` in `swap_v1.rs`. For `btc_to_fiat` the reducer wants the opposite fiat signers, and this map still uses the `fiat_to_btc` keys.

Fail requires local status `secured`, calls `fail` on the node immediately, then appends `core/authorize_refund` and `core/refund` and sets local status `refunded`. It does not read `fiat_pay_by`. The reducer allows `core/authorize_refund` only from `Secured`, by the bitcoin provider, and only when `t >= fiat_pay_by`. Before that it returns `fiat_pay_by has not elapsed`.

Expire requires local status `proposed`, then appends `core/expire` and sets `expired`. It does not read `expires_at`. The reducer returns `not yet expired` while `t < expires_at`.

Dispute accepts local statuses `accepted`, `secured`, `fiat_sent`, and `fiat_confirmed`. The HTTP path never writes `accepted`, `fiat_sent`, or `fiat_confirmed`, so the practical case is `secured`. The action data is always `{"class": "timeout"}`. The `resolver` field on the request receives two kind-1059 wraps asking for an arbitrator share. That pubkey is not the root's `core/resolver`. The root resolver is the derived `^= 0x03` key.

The UI seller generates `P`, splits GF(2^8) shares at x = 1, 2, 3, keeps all three in the seller tab, and sends s1 and s3 immediately. `sendShare` builds a NIP-59 wrap and also puts that wrap on a `BroadcastChannel`, then publishes it if a relay URL is set in the tab. Offers and BOLT11 invoices travel on that channel in the clear. The tab secret and the share map are written to `sessionStorage`.

`docker-compose.yml` is still only bitcoind. `docker-compose.mvp.yml` adds strfry, the daemon, and the frontend. The daemon service depends on bitcoind and does not depend on the relay. `origin/main` `bitcoind.rs` calls `loadwallet` when `createwallet` reports that the wallet already exists. The checkout at `be4bc1f` does not.

## Findings

Severity is about this repository. High means sats can be taken, a paid fiat sender can be left with nothing, or a stored chain will not replay under the reducer this repo already ships. Medium means a `swap@1` rule or a research control is missing and the loss is conditional. Low means a comparison or an operational note.

### 1. High. The hold invoice is pattern B1, and the Shamir split does not make it B2

Source: `escrow-policies-and-exchange-patterns.md` §2 pattern B, §4.1, §8; `resolver-recoverable-preimage.md` problem statement; `openbitcoin-africa-p2p-escrow.md`.

Pattern B1 is: settle needs the invoice holder and the preimage; refund is cancel or timeout. A resolver who cannot produce the preimage cannot force settlement. When the lock expires, the payer is refunded. A bitcoin seller who has already received fiat can withhold `P` and keep both sides.

PontHTLC's split is a private-payload extension on top of that lock. It is not RRP.

- The seller samples `P` and therefore already knows it. Two shares are not required for the seller to settle or to refuse.
- Shares are additive GF(2^8) points at fixed x values 1, 2, and 3. RRP shares are scalars on a degree `t-1` polynomial over the secp256k1 field, masked with ECDH to the resolver's own key. The seller must not be able to hand the resolver a share the recipient cannot check.
- s3 is sent at originate time. RRP's resolver is offline at setup and recovers a share later from the commitment and `r_m`.
- There is no cut-and-choose, no signed commitment, no opening, and no DLEQ. A seller can send an s3 that does not lie on the polynomial. The buyer discovers that only when reconstruction fails, which can be after fiat has moved.
- The buyer must finish RRP verification before fiat. This UI sends s1 before the invoice exists and has no verification step at all.
- On a provider-wins dispute, RRP sends nothing and the invoice holder cancels. This daemon's fail path cancels from the host, immediately, and the named arbitrator is not in that decision.
- `rrp/vectors.json` is the test suite for that construction. Nothing in this repo consumes it.

The README sentence "no single party (including the host) can reconstruct it alone" is false for the seller, who sampled `P` and stored every share. Left unchanged. See the statements section.

### 2. High. One daemon seed is every role on the root

Source: escrow note §4.4 and §1 (agent-derived escrow and resolver keys); Mostro comparison in `mostro.md` §9; `peer.md` on Pontspark's withholding risk.

`secret_for_role` makes agent, customer, escrow, and resolver four pubkeys from one seed. They differ as byte strings. They are one operator. Property 4.4 asks for pairwise-distinct participant, operator, and resolver keys, and asks that resolver keys not be keys the agent announced. Derivation from one seed fails that in substance. The party who pays fiat does not choose the resolver.

The browser identity in `X-Nostr-Pubkey` is not the customer pubkey on the root. A later signature from the tab key will not match `root.customer` until create stores the caller's key and the caller signs. That field is not on the create body in `06d014a`.

`POST /api/escrow/:id/claim` then signs `swap/fiat_sent` and `swap/fiat_confirmed` with those derived keys. The reference is the Lightning payment hash, which passes `valid_payment_reference` because it is alphanumeric and at most 128 characters. The kernel sees a fiat confirmation. The event means "someone posted a preimage to the host." Peer’s gap, stated in `peer.md`, is this one: a valid coordination statement is not evidence that fiat arrived.

A create body may set `direction` to `btc_to_fiat`. `secret_for_action` still signs `swap/fiat_sent` with the customer key. `fiat_sender` for that direction is the agent. The reducer rejects the action. The daemon still settles the HTLC first, then appends the events, then sets local status `settled`.

### 3. High. The lock dies before the dispute window the deadlines describe

Source: escrow note §4.3; RRP "Timing"; Mostro deadline guardian; `constants.rs` `timing`.

Create binds:

- `expires_at` = now + 1800 seconds
- `fiat_pay_by` = now + 3600
- `fiat_confirm_by` = now + 7200

The invoice and `create_inbound_payment_for_hash` both expire in 3600 seconds. `fiat_confirm_by` is two hours after the hold is gone. The crate's own `timing::CONFIRM_WINDOW` is three hours and `HTLC_GRACE` is one hour. The invoice builder does not read those constants.

The escrow note's inequality is `T_def >= T_res >= last profile deadline + dispute window + settle margin`. Here `T_def` is the hold expiry, about one hour, and the last profile deadline is two hours after that. Pattern B1's default is a payer refund. There is no client that opens a dispute by itself when `fiat_sent` is present and `fiat_confirm_by` passes. Mostro's guardian moves a `fiat-sent` trade into dispute before LND cancels it. This daemon has no such loop.

A claim late in the hour can also fail the reducer for a second reason: `swap/fiat_sent` is rejected when `t >= fiat_pay_by`. The node has already called `claim`.

### 4. High. The host can refund the HTLC, and the API does not authenticate the caller

Source: `docs/infrastructure.md` trust table; escrow note pattern A versus B; `implementation.md` on client-callable connectors.

The buyer route creates the invoice, so the invoice-holding LDK node is a process the operator runs. `fail_htlc_backwards` does not need a preimage. The operator can refund a held payment by calling the node, with or without `POST /api/escrow/:id/fail`.

`infrastructure.md` says the host cannot claim or fail without the user's Nostr-key-signed authorization. The HTTP check is a 64-hex header. Any client that knows a pubkey can call create, fund, claim, and fail as that pubkey. WebSocket accepts the same pubkey on the query string. The document was not edited.

`06d014a` also runs the buyer node and the seller node in one daemon. A channel between them can make fund succeed, which a single node paying its own invoice cannot. Both ends are still the operator. The operator sees the HTLC from both sides.

The claim request carries `P` in the body. The host can log it. After a successful claim that preimage cannot authorize a second payment. A log taken before `claim_funds` returns is enough for the host to settle or to race the user. `infrastructure.md` treats that as the same exposure as a card processor. Combined with finding 1, the host is not limited to logging: the host already runs the invoice node.

### 5. High. Fail and expire write chains the reducer rejects, and the local status says they succeeded

Source: `swap_v1.rs` `core/expire` and `core/authorize_refund`; `06d014a` `fail_escrow` and `expire_escrow`.

Fail from `secured`, immediately after fund:

1. `node.fail` runs.
2. `core/authorize_refund` is stored.
3. Local status becomes `refunded`.

Replay through `step` rejects the refund action with `fiat_pay_by has not elapsed` until an hour after create. If the caller waits that hour, the hold has also expired, so the Lightning outcome and the chain outcome depend on timing the reducer never sees.

Expire from `proposed`, immediately after create, stores `core/expire`. Replay rejects it with `not yet expired` until `expires_at`.

`append_event` failures are ignored. A disk error leaves local status updated and the chain short. `publish_event` failures are ignored. A peer who reconstructs from the relay can see a different history from the daemon's `event_chains` map.

### 6. High. Dispute does not bind the arbitrator who was asked, and it does not move the lock

Source: escrow note §4.1 and §7.3; RRP dispute path; `DISPUTE_CLASSES` in `constants.rs`.

`core/open_dispute` is signed by the agent key. Its class is the constant `timeout`, which is a legal class and is not chosen by the caller. The HTTP `resolver` is a delivery address for a NIP-59 request whose terms are `arbitrator-share`. The root's `core/resolver` is the derived key. A gift wrap to Fatou does not make Fatou the resolver the kernel will accept for `core/resolve_dispute`.

No route accepts the share, checks that two shares reconstruct `P`, or turns a ruling into `claim_funds` or `fail_htlc_backwards`. Resolution in PIP-02 does not move funds by itself. Something that can produce the preimage, or cancel, still has to run. That something is the host, using finding 4, or the seller, who already has `P`.

`incorrect_fiat_amount` has no partial outcome. A hashlock cannot split. The escrow note's answer on a hashlock is tranches, each a separate invoice. This API creates one invoice per escrow.

### 7. Medium. The published chain is not the chain a second client needs

Source: `implementation.md` ("complete signed events remain the portable evidence"); PIP-02 discovery of agent and descriptor; OpenSwap §5 on lossy watches.

Create stores agent, descriptor, and root. It publishes the root only. A client that has only the relay cannot fetch the descriptor the root names.

The MVP compose file starts strfry as `nostr-relay` on port 7777 and starts `ponthtlc` with a command that never sets `--relay-url`. Inside that container the default `ws://127.0.0.1:7777` is not strfry. The daemon does not `depends_on` the relay. Publish errors are discarded, so the stack looks healthy while nothing is published.

There is no second implementation, no TCK run, and no restart test that kills the daemon after `claim_funds` and requires the signed settle event to match the lock. `implementation.md` treats a shared Rust core as bindings, not as interoperability evidence. This repo has one Rust reducer and one daemon that does not call it on the HTTP path before it commits status.

### 8. Medium. Money moves, then the chain is written, and the client can ask for the money move

Source: `implementation.md` durable-effect sequence; `plugins.md` closed command set.

The research sequence is: valid signed authorization, durable intent, connector execute, inspect if the result is unknown, then a signed outcome event. Claim does the reverse for the economic step: `node.claim` runs, and only then are fiat and settle events signed. Fail does the same with `node.fail`.

A crash after the LDK call and before `append_event` leaves the HTLC settled or failed and the stored chain behind. There is no `inspect` operation and no idempotency key derived from coordination id plus triggering action. The connector is an in-process `LdkBridge`, not a checksummed subprocess with a closed enum.

The client surface is `POST /api/escrow/create`, `fund`, `claim`, `fail`, `dispute`, `expire`, plus `POST /api/channel/open`. `implementation.md` and `plugins.md` both say the client submits a complete signed event and listens to SSE, and that connector execution is not a public route. PontHTLC is a hosted LDK service with a REST API. That can still be a connector later. It is not that daemon shape now. Adding a plugin ABI would contradict both notes. This audit does not propose one.

### 9. Medium. Share transport and the seller tab

Source: RRP privacy paragraph (`P` must not be published before settle); `infrastructure.md` preimage flow; NIP-59 discussion in the Mostro and PeerBadge notes.

`sendShare` does wrap a signed rumor. That part matches the crate's NIP-59 path. The same function also emits the wrap on `BroadcastChannel('ponthtlc')`. Any tab on the origin can read it. Offers and invoices on that channel include the payment hash, the amount, and the BOLT11.

`rememberShares` stores s1, s2, and s3 in the seller tab before anything is sent. `persist` writes `secret` and `shares` to `sessionStorage`. A script on the page can reconstruct `P` without a second share. The arbitrator does not need to be online for the seller to have done this, and the buyer cannot tell a good s3 from a bad one.

`infrastructure.md` step 5 says the seller delivers s1 after the HTLC is held. `originate` sends s1 and s3 before create. Step 6 tells the buyer to combine s1 and s2. The seller still has s2 until `release`. The written sequence and the UI sequence differ. The file was not edited.

### 10. Medium. Fiat is unproven, reusable as a reference, and uncovered by a bond

Source: escrow note §5 and §6 (cash and M-Pesa are not escrowable; reversibility and provability); `peer.md`; Mostro payer-account history; Bisq account signing; OpenPleb receipt-upload failure.

`swap@1` is the "one escrowable leg, one not" pattern. The escrow protects the side that pays fiat first only if a resolver can force the bitcoin outcome. Finding 1 says this lock cannot. There is no reversal bond, no tranche, no attestation oracle, and no per-reference nullifier.

The reference stored at claim time is the Lightning payment hash. It is unique per invoice and says nothing about which M-Pesa payment or which cash handover it stands for. Two coordinations cannot be tied to one fiat payment because the reference is not a fiat payment. A seller can still take fiat outside the app and withhold `P`.

`openpleb-pontmore-interop.md` records a concrete version of the same hole: a receipt upload can fail after fiat has moved, and a pubkey string is not a signature. PontHTLC has no receipt object. Silence and a lost request look the same.

Cash in person has provability `none`. The UI can name an arbitrator and still has no checklist committed as `private_terms` (Vexl's useful piece). The agreed amount lives in the seller's tab and in the invoice amount.

### 11. Medium. Two Shamir implementations disagree on `gf_inv(0)`

Source: this repo, not the research notes. Stated because a later RRP or share change will trip on it.

`crates/ponthtlc-escrow/src/shamir.rs` `gf_inv` asserts and panics on 0. `crates/spike/src/shamir.rs` `gf_inv` returns `gf_pow(0, 254)`, which is 0. The spike test is `gf_inv_zero_returns_zero`. Share x values are 1, 2, and 3, so a normal combine does not divide by zero. The two functions are still different code. This audit does not rename the test and does not make them the same.

Neither function is the RRP polynomial. Reusing the GF(2^8) split as "the Shamir" in an RRP implementation would not satisfy the draft.

### 12. Medium. Counterparty amounts and quotes are not checked the way the May 2026 Bisq loss says to check them

Source: `bisq.md` §5 (negative fee, 11.59 BTC). The lesson in that note is to validate every number a counterparty supplies.

Create takes `amount_msat` from the body and puts `amount_msat / 1000` on the root as sats. Fiat amount defaults to the string `"1"` in USD even when the invoice is for a different bitcoin amount. A caller can set `fiat_amount` to any positive decimal the kernel accepts. Nothing ties that decimal to the invoice. Nothing caps the invoice against a quote signed by the seller. The UI amount and the chain amount can disagree with the fiat terms a resolver would read.

OpenSwap's fee note is the stricter version: if the agreed numbers do not reproduce the price, stop. PIP-01 already wants exact quotes. This create path invents fiat terms.

### 13. Low. Reputation, bonds, fidelity, and social proof are absent, and the research says they stay out of the kernel

Source: `openswap.md` §2; `peerbadge.md`; `sed.md`; `vexl.md`; `bisq.md` §2; PIP-02 as summarized in those notes (reputation, Sybil resistance, and bonding are application concerns).

There is no fidelity bond, no slashable per-trade bond, no costly reputation, no PeerBadge presentation, no SeD availability signal, and no follow-graph signal. A new key can create a daemon-derived "customer" only by using this host, but anyone can open a tab, generate a key, and be shown as a peer. The public order book Mostro and NIP-69 assume is not what the UI uses. Offers move on the broadcast channel.

`peerbadge.md` says the whitepaper has no wire format, vectors, or second implementation, and that a badge must not become a PIP-02 transition. `sed.md` says the discovery PR was unmerged on 15 September 2026 and that `kind:30078` is a poor public-discovery kind. Using either as a money gate would go past what those notes support.

`openswap.md` is right that a bond is a Sybil cost and not a refund for a cheated fiat sender. A small M-Pesa agent cannot lock a 3–6 month UTXO. If a bond is added later, the note says clients must not require it for listing, and resolvers are the better first subjects. That is a protocol-team decision. It is not a PIP drafted here.

### 14. Low. Mostro's operational list is a catalogue of missing product behavior

Source: `mostro.md` §10 and `mostro-pontmore-leverage.md` §6.

Not present in PontHTLC:

- per-trade keys and a private proof of a long-term identity
- signed chat with read-only disclosure to a solver
- solver permission split (read versus write) enforced in the process that can move funds
- a guardian that escalates paid trades and refunds unpaid ones
- proof of work on first contact, and expiration on private payloads
- portable reputation attestations
- private payer-account history
- bonds slashed only by an explicit solver order
- health and maintenance events

Mostro Lightning mode holds the preimage at the operator and is briefly custodial at payout. Copying that would make finding 4 the intended design and would need `trust(operator)` declared on a descriptor. The current descriptor path does not say that. The leverage note also says GPL and AGPL Mostro repositories stay out of private apps, and the protocol book has no licence, so this repo must not paste that text. MIT components are a later decision, not this audit.

### 15. Low. OpenPleb is a different trade

Source: `openpleb-pontmore-interop.md` §4.

OpenPleb pays a merchant who is not one of the two swap parties. The operator holds Cashu proofs during the trade. Bonds can be split. A receipt timeout can pay the taker. `core/expire` in `swap@1` applies to an unaccepted root, and OpenPleb's `EXPIRED` label is also used for a successful payout. Mapping those words onto PontHTLC's hold invoice would record the wrong outcome.

PontHTLC has no Cashu wallet, no merchant QR, and no bond accounting. A merchant-payment profile would be a new profile. This issue says not to propose one. The finding is that `pontmore/swap@1` on this daemon must not be described as that product.

### 16. Low. Vexl's graph is the wrong substrate; its checklist is the useful piece

Source: `vexl.md` §2 and §7.

Vexl stores a phone-number graph on a server that can rebuild it. The note says not to copy that. Nostr follow lists are the suggested substitute, and the note leaves open whether they are dense enough for a Kenyan mobile-money market. PontHTLC does not compute them.

The checklist (amount, frozen price, place, time, reveal) is a private commitment the resolver can be shown. This UI has free-text roles and a broadcast offer. Nothing commits those bytes in the root's `commitments`.

### 17. Low. Relay absence is treated as absence of an event

Source: `openswap.md` §5.

A client that does not see `swap/fiat_sent` may refund, and a client that does not see `core/open_dispute` may settle, when the event was only missed. This daemon never faces that choice because it does not subscribe before it acts. It acts on the HTTP call. The gap lands on any future client that reconstructs from relays. The OpenSwap rule is: query again with `since` before the deadline, on more than one relay, wait for EOSE, and treat no reply as unknown. PontHTLC has one default relay URL and ignores failure.

### 18. Low. The research notes disagree with each other on pins and on what "fifteen documents" includes

Stated so a later reader does not merge them into one specification.

- Escrow, RRP, Mostro, Bisq, Vexl, PeerBadge, and SeD are dated against protocol `d9a1eb3` or against apps (clink, pontspark), not against this repo.
- `peer.md`, `openswap.md`, and `openpleb-pontmore-interop.md` use protocol `edcf137`.
- `peer.md` quotes DeFiLlama figures from 16 October 2026 and says they were not reconciled to chain history. They are not a current market measurement.
- `openswap.md` says the written fee defaults and the code defaults differ. That is their finding about OpenSwap, not a number to copy.
- `openpleb-pontmore-interop.md` says merged Pontmore main, at `edcf137`, does not define NIP-69 offers, and that offers PR 28 was unmerged. This repo's `constants.rs` has `KIND_OFFER = 38383`. The daemon on `06d014a` does not publish standing offers. Those two facts can both be true.
- RRP, the escrow note, and the Mostro leverage note sketch PIP text. They label it as not normative. Issue 20 says not to propose new PIPs. Those sketches are inputs to the protocol team, listed under open questions.

## Recommendations

Grouped where the work would live. Each item is a change to this repo's configuration or to how it uses rules the protocol already has, or a question that has to be answered before code. None of these are new PIP text.

### PIP-00

Do not put ratings, badges, SeD heartbeats, or bond scores on the agent definition. `peerbadge.md`, `sed.md`, and `openswap.md` all say a signed capability is authorship, not solvency. When this daemon grows a real agent event that other clients fetch, publish the one it already builds, and leave corridor availability in a short-lived signed signal the protocol team has actually adopted.

### PIP-01

The descriptor this daemon signs is a `lightning_hold_invoice` with a one-day expiry. It does not say the horizon (3600 seconds in the invoice builder), that split is impossible, that the default is refund to the payer, or that the operator runs both LDK nodes. Those are facts a counterparty needs before `core/accept`. Putting them in the descriptor is a protocol-team change if the current PIP-01 schema has no field for them. Until that field exists, the HTTP create response and `docs/infrastructure.md` should stop saying the host cannot refund. This audit does not edit that file. The correction belongs in a docs change the maintainers accept: the operator runs the invoice node and can fail the HTLC.

`escrow_type: custodial_escrow` is what the OpenPleb note suggests for an operator-held Cashu balance. It is the wrong label for this LDK hold invoice, and it is also the wrong label if the team later admits operator control of `fail`. The honest current label remains `lightning_hold_invoice`, plus an explicit statement that the operator can cancel.

### PIP-02 and the swap@1 profile

Use the reducer this crate already has before writing local status or calling LDK.

- Do not append `core/expire` unless `t >= expires_at` and status is still `proposed`. The reducer already returns `not yet expired`.
- Do not call `node.fail`, and do not append `core/authorize_refund`, unless status is `Secured`, no `fiat_sent` is in the chain, and `t >= fiat_pay_by`. Otherwise the stored chain and the HTLC diverge.
- Do not sign `swap/fiat_sent` from `claim`. Fiat actions need a signed reference from the fiat sender and a matching confirmation from the fiat receiver, under the deadlines. The preimage post is `core/settle` material after `SettlementAuthorized`, or it is a private share release. It is not a fiat receipt.
- `secret_for_action` has to follow `fiat_sender` and `fiat_receiver` for the root's direction. The default `fiat_to_btc` map is not valid for `btc_to_fiat`.
- Publish the agent definition and the descriptor, then the root. Surface relay errors to the caller when a relay URL is configured.
- Pass `--relay-url ws://nostr-relay:7777` from `docker-compose.mvp.yml`, and depend on the relay. The file as merged dials loopback inside the daemon container.
- Stop discarding `append_event` errors.
- Let `open_dispute` take a class from `DISPUTE_CLASSES`. Keep `timeout` as one allowed value, not the only value.
- The `resolver` query must be the root's `core/resolver`, or the call should be rejected. A wrap addressed to a third key does not rebind the root.
- Derive customer, escrow, and resolver only when those parties are actually different keys. A browser `customer_pubkey` that the daemon signs for is still the host playing the customer. The tab should sign `swap/fiat_sent` and `core/accept` with its own key. That is the existing profile rule, not a new action.

`core/settle` and `core/refund` should record an outcome the node observed. Signing them in the same function that requested the outcome, without reading back `PaymentSent` or `PaymentFailed`, records an intention.

### Implementation

- Keep GF(2^8) shares as a private payload with the limits in finding 1 written next to them. Adopting RRP means a different construction, the vectors in `rrp/vectors.json`, and a check that runs in the buyer tab before fiat. It also means the seller must not keep a raw `P` the resolver path is supposed to withhold. That design is not decided here.
- Leave `gf_inv_zero_returns_zero` named as it is. If the two `gf_inv` functions are unified later, do it in a change that states which behavior is kept.
- Stop persisting the seller's full share set and the tab secret in a place page script can read, or accept that the hosted page is inside the trust boundary and say so in `infrastructure.md`.
- Keep offers and invoices off the cleartext broadcast channel once a relay is actually used. Same-origin tabs are a demo bus.
- Build the invoice expiry from `fiat_confirm_by` plus the grace the timing module already names, or shorten the profile deadlines until they fit inside 3600 seconds. Today the constants and the invoice disagree. Changing the numbers without a protocol decision just picks a new arbitrary window. The safe implementation step is to refuse create when the invoice horizon is shorter than `fiat_confirm_by`.
- Tie `amount_msat` to the bitcoin amount on the root, and refuse a fiat amount the caller invented with no seller signature.
- Model connector calls as intent, execute, inspect. The smallest step that matches `plugins.md` without a new ABI: persist "claim requested" before `claim_funds`, and do not mark `settled` until a `PaymentSent` for that hash is observed.
- Do not add Wasm, a plugin trait, or a public "run connector" route.

### What not to build from these notes

- A new profile for merchant payment, fidelity bonds, or bonds. Ask the protocol team. OpenPleb and OpenSwap describe products this daemon does not run.
- A global reputation score on PIP-00.
- Vexl's server-side contact hash.
- Mostro GPL or AGPL code, or verbatim protocol-book text.
- Peer enclave verification inside the PIP-02 reducer. `peer.md` says payment evidence stays in a service. The kernel should keep checking signatures and deadlines only.

## Open questions

These are the points the research set does not settle, plus the ones this codebase cannot answer alone.

1. Is the intended PontHTLC lock pattern B1 (payer refund, seller holds `P`), pattern A (this host is a declared operator who may cancel and who must be trusted), or B2 (RRP, so a resolver who is not the host can force settlement)? The README says B2's promise. The code is B1 plus a share the seller can forge. The merged event path adds a derived resolver who is the host.
2. May the resolver learn `P`? RRP says the recipient settles and `P` is not published, because a routing node that sees `P` can claim the incoming HTLC. This claim API posts `P` to the host by design.
3. Who holds s3 before a dispute if the team keeps GF(2^8) shares? Sending it at originate requires the arbitrator to be named and online, and it gives them material they cannot prove was honest. Holding it on the seller until a dispute means the seller can refuse to send it, which is the withholding case again.
4. Should `customer` on the root be the browser key? Until that is yes, tab signatures cannot authorize `swap/fiat_sent`. The protocol team has to say whether a hosted node may sign as customer. This audit's reading of `swap_v1.rs` is that the fiat sender signs, and the fiat sender for `fiat_to_btc` is the customer.
5. What is `T_def` for LDK hold invoices on a direct channel in this deployment? Invoice expiry is 3600 seconds and CLTV delta is 40. The research note uses Spark's roughly four-hour hold and LND's `holdexpirydelta` as examples. This repo needs one number, and the deadlines have to fit under it.
6. Does a missed relay equal "no event"? OpenSwap says no. This daemon currently treats the HTTP call as the event. A second client reconstructing from strfry needs the rule written down.
7. For M-Pesa and cash, which loss is accepted: no adjudicator (Open Bitcoin Africa's challenge), a trusted resolver who can force the HTLC (RRP or a real 2-of-3 the buyer verified), a trusted operator (Mostro), or a bond that pays the loser after the fact (Peer, Bisq deposits)? The notes show these are different. They do not pick one for a 10,000 sat trade.
8. Is one fidelity bond allowed to back both an OpenSwap maker and a Pontmore agent? `openswap.md` leaves that open. PontHTLC has neither.
9. Are NIP-02 follows dense enough to replace a phone graph for the users this UI names? `vexl.md` leaves that open. This repo has no measurement.
10. PeerBadge has no interoperable credential format in the note's own test. SeD was an unmerged PR. Neither can be a dependency of claim or fund.
11. The research pins differ (`d9a1eb3` and `edcf137`). Which protocol commit should PontHTLC's reducer track? The crate does not record a pin. `implementation.md` phase 0 says to pin one.

## Statements already in this repo

These are wrong or stale relative to `06d014a` and to the UI. They are listed so they can be corrected later. This change does not edit them.

| Place | What it says | What the code does |
| --- | --- | --- |
| `README.md` opening | No single party, including the host, can reconstruct `P`. The user authorizes with a Nostr key. | The seller samples `P` and stores every share. The host holds the invoice node and can fail the HTLC. `X-Nostr-Pubkey` is not a signature. |
| `README.md` crate table | `ponthtlc-escrow` depends on `ponthtlc`. | `crates/ponthtlc-escrow/Cargo.toml` depends on `ponthtlc-protocol`, `lightning`, `bitcoin`, and `rand`. |
| `docs/infrastructure.md` trust table | API calls require the user's Nostr-key-signed authorization, and the host has zero shares. | Header pubkey only. The seller tab holds all shares. The daemon holds none, and still runs `fail`. |
| `docs/infrastructure.md` steps 5 and 6 | s1 moves after the HTLC is held; the buyer combines s1 and s2 obtained over Nostr. | s1 and s3 move at `originate`, before create. s2 moves only if the seller calls `release`. Wraps also cross `BroadcastChannel`. |
| `ROADMAP.md` | Issues 1 through 5 are open or in progress. The MVP compose file and browser definition of done are unchecked. | On `origin/main`, pull requests 6, 7, 10, 13, 14, 17, and 18 are merged. `docker-compose.mvp.yml` exists. The roadmap text is from before that. Issue 4 was still open on GitHub after the UI merged; the roadmap's status column is not a reliable board. |
| `docs/infrastructure.md` channel picture | Two hosted nodes, one provider. | `06d014a` runs both LDK nodes in one process. The checkout at `be4bc1f` does not contain that code yet. |

`ensure_wallet` on `be4bc1f` treats "wallet already exists" as success and does not `loadwallet`. `origin/main` `bitcoind.rs` does call `loadwallet`. That bug is fixed on the branch this audit describes and is still present in the checkout. Left unchanged here.

## What this audit does not do

- It does not implement RRP, a policy language, a bond profile, or a merchant-payment profile.
- It does not audit `pontmore/pontmore`. `implementation.md` describes that design. Findings about PontHTLC say where this daemon differs from that design.
- It does not re-test Live Lightning. The behavioral claims are from reading `06d014a` and the reducer. A failing replay of an immediate expire or an immediate fail is what `step` returns on those inputs; the HTTP handlers do not call `step`.
