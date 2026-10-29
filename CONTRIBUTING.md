# Contributing to PontHTLC

## Development setup

See [README.md](README.md) for prerequisites and setup instructions.

## Code style

- Rust edition 2021, stable toolchain.
- Follow `cargo fmt` and `cargo clippy` before submitting a PR.
- No comments in code unless explaining a non-obvious design decision.
- Match existing patterns in the codebase; check neighboring files before
  introducing a new dependency or pattern.

## Testing

The spike binary has three integration scenarios that run against a live
regtest `bitcoind` container:

```bash
# start bitcoind first
docker compose up -d

# run each scenario
cargo run -p ponthtlc-spike -- --scenario happy
cargo run -p ponthtlc-spike -- --scenario fail
cargo run -p ponthtlc-spike -- --scenario hold
```

A passing run prints `SPIKE_RESULT: PASS scenario=<scenario>`.

Before submitting changes that touch the LDK wiring or event pump, verify
all three scenarios still pass.

## Commit conventions

- Conventional commit messages (`feat:`, `fix:`, `docs:`, `refactor:`).
- Keep commits atomic — one logical change per commit.
- Do not commit secrets, private keys, or `spike-data/` contents.

## Pull request process

1. Open an issue first for non-trivial changes.
2. Branch from `main`, name as `<type>/<short-description>`.
3. Ensure `cargo build` and all three spike scenarios pass.
4. Reference the issue in the PR description.
5. Keep PRs focused — one feature or fix per PR.

## Pontmore protocol conformance

We conform to the existing `pontmore/swap@1` profile. Do not invent new
Pontmore profiles or alter the public root/action schema. The 2-of-3
Shamir split is an implementation-private extension that stays within the
swap profile's private-payload boundary. See
[docs/spike-runbook.md](docs/spike-runbook.md) for the LDK-to-kernel
action mapping.
