# Arc/USDC current-main integration report

**Date:** 2026-09-01

**Status:** Code-level integration candidate; not deployed and not live-qualified

## Lineage

| Item | Commit |
| --- | --- |
| Integration branch base | `544130028fd928a5fd32177b27980d5d0522689a` |
| Effective current-main tree baseline | `2f8281d53a6ea9d74ae3565311efe75bfeb994a6` |
| Arc source branch tip | `fcdb7c8b96b0086be4498ec392b71c58c2ff4119` |
| Shared merge base | `8cc7d15edc58b5f5a0b745143fef2d45203465ff` |

The Arc branch was 6 commits ahead of the shared base. Current `main` was 159
commits ahead. Eleven paths had changes on both sides. Git merged ten of those
paths automatically; `crates/types/src/tests.rs` had the only textual
conflict. The resolution retained both current-main genesis-registry tests and
the Arc type tests.

The standalone prover lock required one additional `sha2 0.10.9` dependency
entry because the current-main `postfiat-types` path dependency now includes
that crate. No dependency version was intentionally upgraded.

## Current code gates

Passed on the integrated tree:

- `cargo test -p arc-conformance --locked`: 4 passed;
- `cargo test --manifest-path programs/pfusdc-arc-ingress/Cargo.toml --locked`:
  5 passed;
- `cargo test -p postfiat-types --locked`: 130 passed;
- `cargo test -p postfiat-execution --locked`: 190 passed;
- six focused node safety tests: governed-route replay, signed route
  authorization, invalid/rotated deposit rejection, signed snapshot tamper
  rejection, direct-mutation snapshot rejection, and offline-only storage
  migration;
- `cargo check -p postfiat-node --all-targets --locked`;
- Clippy with warnings denied for `postfiat-types`, `postfiat-execution`,
  `postfiat-node`, and `arc-conformance`;
- standalone `pfusdc-tier4-prover` Clippy with `--no-default-features`,
  `--locked`, and warnings denied;
- targeted Rustfmt for the four integrated workspace packages;
- public-secret scanner regression and complete tracked-tree scan; and
- `git diff --cached --check`.

The genesis-registry type tests require the archived, manifest-verified round
fixtures under the benchmark's ignored `rounds/` directory. Those frozen local
fixtures were supplied for the 130-test run.

## Gates not rerun

- Foundry is not installed on this host, so the Solidity suites were not rerun.
- The full workspace Rustfmt gate currently reports formatting differences in
  two current-main files not changed by this merge:
  `crates/consensus_cobalt/tests/genesis_registry_checker.rs` and
  `crates/storage/src/transactional.rs`.
- The full workspace test suite was not run.
- No Arc RPC fixture was recaptured, no new SP1 ELF/vkey/proof was generated,
  and no live six-validator deployment or round trip was attempted.

## Safety conclusion

Bringing the Arc/USDC code onto current `main` is mechanically and
semantically feasible: the merged Rust surfaces compile, the complete types and
execution suites pass, and the focused state/replay/governance checks pass.

This report does **not** promote the August evidence to current release
qualification. The checked-in proof and live-chain artifacts remain historical
benchmark evidence bound to their recorded programs, routes, contracts, and
source commits. Deployment requires fresh Solidity tests, complete release
gates, regenerated proof bindings, and a separately authorized live
qualification.
