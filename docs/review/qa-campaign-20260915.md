# QA campaign log — 2026-09-15

This is the progress record for the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Work began on clean `main` at `d4037d14`. This campaign reviews source and makes no deployment or live activation.

**Status:** A1 findings recorded and P2 source repairs tested; closeout pending. A2–A5 and B pending.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Block finality, consensus artifacts, and signing | fixing | 0 | 2 | 1 | [Review](finality-consensus-review-20260915.md); findings `abb42827`; tested source repairs in the current repair commit; closeout pending |
| A2 | Canonical types and state commitment | pending | — | — | — | — |
| A3 | Node startup, release verification, and RPC serving | pending | — | — | — | — |
| A4 | Live shadow and swap services | pending | — | — | — | — |
| A5 | Node command tools and governance agent | pending | — | — | — | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **0 P1, 2 P2, 1 P3** (A1 only).

## Block finality, consensus artifacts, and signing review result

The [A1 review](finality-consensus-review-20260915.md) records two P2 findings: maximum-view timeout successor overflow and missing durable proposal-lock reservation before a proposer returns its signature. The source repairs use a checked successor and the existing durable lock; focused regressions pass. Both repairs tighten artifact or signing admissibility and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. One P3 records independently selected duplicate receipts and block links in the unaudited finality query and remains unfixed. The reviewed portions and skipped portions of the five A1 source files are itemized in the findings document; the remaining sections were not audited.

## Skips and boundary decisions

No Task Node or fleet action. No other surface or B work started. The account-transaction index and unrelated action, shielded, bridge, pfUSDC/Arc, owned-object, and manifest helpers within the A1 files were not audited; already reviewed legacy ordering and other crates were not re-reviewed.

## Verification

`cargo check -p postfiat-node -p postfiat-ordering-fast --locked` passed. `cargo test -p postfiat-ordering-fast consensus_v2_proposal_rejects_exhausted_timeout_view_without_overflow --locked`: 1 passed. `cargo test -p postfiat-ordering-fast consensus_v2::tests --locked`: 12 passed. `cargo test -p postfiat-node proposer_signature_reserves_durable_proposal_lock_before_returning --lib --locked`: 1 passed. `cargo test -p postfiat-node bridge_exit_root_activation_tests --lib --locked`: 3 passed. `cargo test -p postfiat-node block_vote --lib --locked`: 3 passed. `cargo test -p postfiat-node block_proposal --lib --locked`: 3 passed after updating the signed-equivocation fixture. `cargo fmt --all -- --check` and `git diff --check` passed. The strict documentation build, public documentation link check, and public secret scan are required before every commit and have passed for the findings commit; repair gate pending.

## Scores

The B inventory scoring gate has not started.

## Final summary

A1 findings are recorded (0 P1, 2 P2, 1 P3); both P2 source repairs pass focused tests, are consensus-affecting, and await full Rust CI and A1 log closeout. The P3 is recorded without repair. A2–A5 and B have not been reviewed.
