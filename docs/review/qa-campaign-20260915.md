# QA campaign log — 2026-09-15

This is the progress record for the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Work began on clean `main` at `d4037d14`. This campaign reviews source and makes no deployment or live activation.

**Status:** A1 done with the review limits below; A2–A5 and B pending. No live activation or fleet action.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Block finality, consensus artifacts, and signing | done | 0 | 2 | 1 | [Review](finality-consensus-review-20260915.md); findings `abb42827`; repairs `6ec35092` (consensus-affecting; full Rust suite verdict pending) |
| A2 | Canonical types and state commitment | pending | — | — | — | — |
| A3 | Node startup, release verification, and RPC serving | pending | — | — | — | — |
| A4 | Live shadow and swap services | pending | — | — | — | — |
| A5 | Node command tools and governance agent | pending | — | — | — | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **0 P1, 2 P2, 1 P3** (A1 only).

## Block finality, consensus artifacts, and signing review result

The [A1 review](finality-consensus-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2s are maximum-view timeout successor overflow and missing durable proposal-lock reservation before a proposer returns its signature. Repair `6ec35092` uses a checked successor and the existing durable lock; both focused regressions pass. Both changes tighten artifact or signing admissibility and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. The P3 records independently selected duplicate receipts and block links in the unaudited finality query and remains unfixed.

The A1 review examined focused paths within exactly five source files: `crates/node/src/block_finality.rs` (finality query, block proposal signing, vote and certificate aggregation, timeout and equivocation handling); `crates/node/src/consensus_artifacts.rs` (block vote/certificate and timeout validation, signed governance authorization); `crates/ordering_fast/src/lib.rs` (shared quorum and legacy certificate verification); `crates/ordering_fast/src/consensus_v2.rs` (typed proposal/vote/QC/TC verification and safety authorization); and `crates/crypto_provider/src/lib.rs` (ML-DSA signing, context verification, and hashing). This was a focused review of A1 behavior, not a whole-file audit. Sections that were not read for the A1 focus are detailed in Skips. No A2–A5 file or B inventory was reviewed.

## Skips and boundary decisions

- No Task Node, fleet, host, chain, deployment, spend, or signup action occurred; pushes were only to this repository's `origin main`.
- `crates/node/src/block_finality.rs`: account-transaction indexing and presentation (lines 163–2216) and remaining batch simulation/archive helpers were not audited. `crates/node/src/consensus_artifacts.rs`: unrelated shielded, bridge, pfUSDC/Arc, owned-object, operator-manifest, snapshot, and batch-action helpers were not audited. Full artifact reload across non-A1 node store modules was not audited; the repair regression checked durable proposal-lock reload using a fresh store instance.
- `crates/ordering_fast/src/lib.rs`: previously reviewed simulation/legacy ordering model, admission receipts, and omission evidence were not re-reviewed. `crates/ordering_fast/src/consensus_v2.rs` and `crates/crypto_provider/src/lib.rs`: their A1 signing and verification paths were read; no A1 file was excluded in full. The ordering v2 test file and node consensus-history test file were used only as regression fixtures, not as new review surfaces; the latter was minimally changed to preserve an externally signed equivocation case.
- Excluded and previously reviewed crates and files, frozen artifacts, A2–A5, and B were not reviewed or edited. The P3 hot-path observation was recorded without repair. The full workspace and long Orchard/Halo2 suites were not run: these changes touch signing and artifact admission and do not cross an Orchard boundary; the full Rust suite remains CI's verdict.

## Verification

- `cargo check -p postfiat-node -p postfiat-ordering-fast --locked`: passed.
- `cargo test -p postfiat-ordering-fast consensus_v2_proposal_rejects_exhausted_timeout_view_without_overflow --locked`: 1 passed.
- `cargo test -p postfiat-ordering-fast consensus_v2::tests --locked`: 12 passed.
- `cargo test -p postfiat-node proposer_signature_reserves_durable_proposal_lock_before_returning --lib --locked`: 1 passed.
- `cargo test -p postfiat-node bridge_exit_root_activation_tests --lib --locked`: 3 passed.
- `cargo test -p postfiat-node block_vote --lib --locked`: 3 passed.
- `cargo test -p postfiat-node block_proposal --lib --locked`: initial run 2 passed, 1 failed because the evidence fixture used the newly locked signer twice; after changing that fixture to provide a test-key signed external artifact, the rerun passed 3 tests.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before each A1 commit (findings, repairs, and closeout). No full Rust suite was run locally; **full Rust suite verdict pending** CI.

## Scores

The B inventory scoring gate has not started.

## Final summary

A1 is done with **0 P1, 2 P2, 1 P3**. Findings `abb42827`; repairs `6ec35092`. The checked maximum-view successor and durable proposer-signature interlock are both **consensus-affecting**, source-only, and await the full Rust CI verdict. The P3 unaudited finality-query duplicate pairing remains unfixed. Focused A1 paths within five files were reviewed; unrelated sections and cross-module full artifact reload were not audited. A2–A5 and B remain pending and were not started. No activation, deployment, fleet, Task Node, or frozen-artifact action occurred.
