# QA campaign log — 2026-09-15

This is the progress record for the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Work began on clean `main` at `d4037d14`. This campaign reviews source and makes no deployment or live activation.

**Status:** A1 done with the review limits below; A2–A5 and B pending. No live activation or fleet action.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Block finality, consensus artifacts, and signing | done | 0 | 2 | 1 | [Review](finality-consensus-review-20260915.md); findings `abb42827`; repairs `6ec35092` (consensus-affecting; full Rust suite verdict pending) |
| A2 | Canonical types and state commitment | done | 0 | 2 | 1 | [Review](types-state-commitment-review-20260915.md); findings `f5c8e88a`; repairs `0a1216c3` (consensus-affecting; full Rust suite verdict pending) |
| A3 | Node startup, release verification, and RPC serving | pending | — | — | — | — |
| A4 | Live shadow and swap services | pending | — | — | — | — |
| A5 | Node command tools and governance agent | pending | — | — | — | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **0 P1, 4 P2, 2 P3** (A1 and A2).

## Block finality, consensus artifacts, and signing review result

The [A1 review](finality-consensus-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2s are maximum-view timeout successor overflow and missing durable proposal-lock reservation before a proposer returns its signature. Repair `6ec35092` uses a checked successor and the existing durable lock; both focused regressions pass. Both changes tighten artifact or signing admissibility and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. The P3 records independently selected duplicate receipts and block links in the unaudited finality query and remains unfixed.

The A1 review examined focused paths within exactly five source files: `crates/node/src/block_finality.rs` (finality query, block proposal signing, vote and certificate aggregation, timeout and equivocation handling); `crates/node/src/consensus_artifacts.rs` (block vote/certificate and timeout validation, signed governance authorization); `crates/ordering_fast/src/lib.rs` (shared quorum and legacy certificate verification); `crates/ordering_fast/src/consensus_v2.rs` (typed proposal/vote/QC/TC verification and safety authorization); and `crates/crypto_provider/src/lib.rs` (ML-DSA signing, context verification, and hashing). This was a focused review of A1 behavior, not a whole-file audit. Sections that were not read for the A1 focus are detailed in Skips. No A2–A5 file or B inventory was reviewed.

## Canonical types and state commitment review result

The [A2 review](types-state-commitment-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2s are a FastPay recovery committee identity that omits its new-order admission window and a retained recovery certificate absent from the reveal state commitment; the same undercommitment in confirmed version fences was repaired with the shared canonical certificate encoder. Repair `0a1216c3` binds both admission heights in the committee root and length-prefixes retained canonical certificate bytes in recovery state commitments. The two type-module regressions and the adjacent version-fence regression pass. The P3 notes a long domain label length narrowed by the public genesis digest helper; constant-label genesis-registry callers are unaffected, and the P3 remains recorded without repair.

Both P2 repairs change hashed committee identities or state-root bytes and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. Archived replay and activation qualification are still required before any release-lineage use.

The A2 review read focused serialization, hashing, arithmetic, ordering, and version paths in `crates/types/src/consensus_v2_types.rs`, `core_chain.rs`, `ledger_assets.rs`, `genesis_registry.rs`, `fastswap_types.rs`, `account_owned_asset_types.rs`, `market_nav_asset_types.rs`, `fx_fix_types.rs`, `nav_reserve_public_values.rs`, `shielded_bridge_governance.rs`, `fastpay_recovery_types.rs`, and `crates/node/src/state_commitment.rs`. Large modules were sampled around the A2 focus, not audited in full. The pfUSDC and Ethereum bridge type files provided read-only schema context. The node root-test fixture was adjusted only to exercise a canonical retained certificate; it was not reviewed as a separate surface. No A3–A5 file or B inventory was reviewed. A1 and A2 are done; A3–A5 and B remain pending.

## Skips and boundary decisions

- No Task Node, fleet, host, chain, deployment, spend, or signup action occurred; pushes were only to this repository's `origin main`.
- `crates/node/src/block_finality.rs`: account-transaction indexing and presentation (lines 163–2216) and remaining batch simulation/archive helpers were not audited. `crates/node/src/consensus_artifacts.rs`: unrelated shielded, bridge, pfUSDC/Arc, owned-object, operator-manifest, snapshot, and batch-action helpers were not audited. Full artifact reload across non-A1 node store modules was not audited; the repair regression checked durable proposal-lock reload using a fresh store instance.
- `crates/ordering_fast/src/lib.rs`: previously reviewed simulation/legacy ordering model, admission receipts, and omission evidence were not re-reviewed. `crates/ordering_fast/src/consensus_v2.rs` and `crates/crypto_provider/src/lib.rs`: their A1 signing and verification paths were read; no A1 file was excluded in full. The ordering v2 test file and node consensus-history test file were used only as regression fixtures, not as new review surfaces; the latter was minimally changed to preserve an externally signed equivocation case.
- Excluded and previously reviewed crates and files, frozen artifacts, and A2–A5 and B were not reviewed or edited during A1. The A1 P3 hot-path observation was recorded without repair. The full workspace and long Orchard/Halo2 suites were not run for A1: its changes touch signing and artifact admission and do not cross an Orchard boundary; the full Rust suite remains CI's verdict.
- For A2, the large account-owned, market/NAV, FastSwap, governance/shielded, and state-commitment modules were reviewed at the named focus paths, not line by line. Bridge-specific, pfUSDC/Arc, excluded Orchard/proof, and historical-exception commitment bodies were not audited; the pfUSDC and Ethereum type files were schema context only. Already-reviewed storage, execution, governance/Cobalt, network, mempool, wallet, RPC SDK, and Python sources were not re-reviewed. A3–A5 and B were not started. The A2 P3 was recorded without repair. No Orchard boundary was changed, so neither the long Orchard/Halo2 suite nor the full workspace suite was run locally; the full Rust suite remains CI's verdict.

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

- `cargo check -p postfiat-types -p postfiat-node --locked`: passed (A2).
- `cargo test -p postfiat-types fastpay_recovery_type_tests --lib --locked`: 5 passed (A2, including the committee, reveal, and confirmed-fence regressions).
- `cargo test -p postfiat-node replicated_state_root_commits_every_fastlane_ledger_field --lib --locked`: initial run 0 passed, 1 failed because its old retained-certificate fixture had no votes; after adding one ordered vote, rerun 1 passed (A2).
- `cargo test -p postfiat-node fastpay_recovery_bootstrap_is_signed_future_activated_and_tamper_atomic --lib --locked`: 1 passed (A2).
- `cargo test -p postfiat-node ordered_fastpay_recovery_cancels_partial_and_withheld_certificates_and_replays --lib --locked`: 1 passed (A2).
- `cargo fmt --all -- --check` and `git diff --check`: passed (A2).
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before the A2 findings and repairs commits; A2 closeout gates passed before its commit. **Full Rust suite verdict pending** CI.

## Scores

The B inventory scoring gate has not started.

## Final summary

A1 is done with **0 P1, 2 P2, 1 P3**. Findings `abb42827`; repairs `6ec35092`. The checked maximum-view successor and durable proposer-signature interlock are both **consensus-affecting**, source-only, and await the full Rust CI verdict. The P3 unaudited finality-query duplicate pairing remains unfixed. Focused A1 paths within five files were reviewed; unrelated sections and cross-module full artifact reload were not audited. A2–A5 and B remain pending and were not started. No activation, deployment, fleet, Task Node, or frozen-artifact action occurred.
