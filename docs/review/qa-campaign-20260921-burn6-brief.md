# QA campaign burn 6 brief — 2026-09-21

Sixth reset-burning QA campaign, following [burns 1 and 2](qa-campaign-20260910.md), [burn 3](qa-campaign-20260911.md), [burn 4](qa-campaign-20260915.md), and [burn 5](qa-campaign-20260916.md). The other lane asked for an audit of the NAVCoin machinery: verification of portfolios and swap rules. Burn 6 runs on the release branch release/combined-devnet-20260915 (the qualified combined release; tip a5b1e757 or later), because the code it covers landed there. Commit and push after every completed unit; never accumulate unpushed work.

## Boundaries (absolute)

- No Task Node action of any kind.
- No fleet mutation: no deploy, restart, configuration change, or write to any validator/RPC host or live chain.
- Work only in the dedicated worktree of the release branch; push only to origin release/combined-devnet-20260915. Do not touch the checkout ~/repos/postfiatl1v2-release-20260915. Do not push to main.
- No spends, rentals, external service signups, or system-level toolchain installs. Network use: git and OpenRouter for Text Improvement Harness scoring only.
- Frozen artifacts stay frozen: attack-simulation outputs, gate outputs, deployment evidence and qualification packets (deployments/), benchmarks receipts, docs/whitepaper.md, docs/whitepaper_legacy.md, locked amendment and lock records. No whitepaper work.
- Out of scope: crates/privacy_orchard, crates/privacy, crates/bridge, crates/ethereum-contracts, crates/pfusdc_proofs, crates/pftl_uniswap_proofs, crates/proofs, programs/ (proof circuits and contracts; findings there are recorded with the dependency named, not fixed).
- Already reviewed, do not re-review: the surfaces of burns 1–5 (their review documents in docs/review/ name the files), except where this brief names a file explicitly for a deeper pass.
- A repair that changes a consensus rule, a state-transition result, an on-disk storage format, or the bytes any validator signs or hashes is consensus-affecting: allowed with a regression test, but the log and the inventory row must say so; since this branch is the qualified release, every consensus-affecting repair also means the release tip must be re-qualified before deployment — say so in the log.
- Before every commit: the strict docs build (.venv-docs/bin/mkdocs build --strict), scripts/public-doc-links, and scripts/public-secret-scan must all pass. Plain single-sentence commit messages. Findings and fixes are separate commits.
- Word findings and repairs neutrally: correctness review, condition, observed and expected behaviour, suggested change.
- When uncertain whether an action crosses a boundary: skip it and log the skip.

## Campaign log

Create and maintain docs/review/qa-campaign-20260921.md with the structure of docs/review/qa-campaign-20260916.md: per-surface status, finding counts by severity, fix commits, skipped items with reasons, verification results (exact test commands and counts), and scores. Push an update at least once per completed surface.

## A. Review surfaces, priority order

Per surface: a fresh-eyes correctness review, a findings document docs/review/<surface>-review-20260921.md with severity, file and line, and a concrete condition and observed/expected behaviour per finding; then fix that surface's P1 and P2 findings with minimal changes and regression tests (P3 findings are recorded, not fixed); run cargo check and the focused tests of every touched module; commit findings and fixes separately.

1. Portfolio verification (docs/review/portfolio-verification-review-20260921.md): crates/execution/src/yolo_target_verifier.rs, crates/execution/src/yolo_collection_verifier.rs and their tests, crates/node/src/yolo_target_queries.rs, crates/types/src/yolo_collection_public_values.rs, crates/node/src/tests/yolo_target_receipt_tests.rs. Focus: a portfolio target or collection proof accepted although the holdings, weights, or totals do not match the public values; weights or amounts that do not sum; stale or replayed proofs; arithmetic overflow and rounding in targets; receipts that report success for a rejected target.
2. NAV and reserve verification (docs/review/nav-reserve-verification-review-20260921.md): crates/nav_reserve_protocol/src/lib.rs, crates/types/src/nav_reserve_public_values.rs, crates/node/src/market_bridge.rs, crates/node/src/tests/nav_reserve_proof_status_tests.rs, scripts/a666-build-live-nav-mark-ops.py, scripts/a666-build-route-epoch-advance.py. Focus: a NAV mark computed from stale, partial, or double-counted reserves; a reserve proof accepted for the wrong epoch, route, or asset; route-epoch ordering and replay; the same-cycle reserve counted twice; price inputs that can be steered by one party.
3. Swap and settlement execution in depth (docs/review/swap-settlement-execution-review-20260921.md): crates/execution/src/nav_vault_asset_execution.rs (subscription, entitlement release, redemption, burn accounting), crates/execution/src/pftl_source_settlement.rs, crates/execution/src/vault_bridge_profile_resolution.rs. Burn 3 sampled this crate; this pass reads the swap paths in full. Focus: fee and spread rounding that leaks value, conservation of supply and reserve across subscribe/redeem, replay and duplicate handling, source-series and family binding, entitlement accounting, ordering dependence within a block.
4. Bridge workflows the release added (docs/review/bridge-workflows-review-20260921.md): crates/node/src/vault_bridge_workflows.rs, crates/node/src/vault_bridge_conservation.rs, crates/node/src/ethereum_checkpoint_signing.rs, crates/node/src/pfusdc_tier4.rs. Focus: conservation checks that can be bypassed, checkpoint signing over the wrong bytes or without domain separation, egress identity binding, fail-closed on malformed remote data, replay of withdrawals.
5. Optional if time remains — node files skipped by earlier burns (docs/review/node-remaining-review-20260921.md): crates/node/src/block_replay_wallet.rs, crates/node/src/rpc_dispatch.rs, crates/node/src/transport_protocol.rs. Focus as burns 3–5.

## B. Defect inventory

Extend docs/review/defect-inventory-20260910.md (on the release branch): add every burn 6 finding as a new row with a new prefix per surface (PFV-, NAV-, SWX-, BRW-, NOD-), keeping every existing row and its wording unchanged except for status updates that burn 6 repairs justify. Update the summary counts, the final disposition totals, and the completeness audit. Score the inventory with the Text Improvement Harness at the established gate (average at least 86; three judges, five reviews each, the exact models, prompt, and procedure recorded in qa-campaign-20260910.md for the burn 2 gate). The last gate scored 86.33: tighten wording where rows are verbose before scoring, without changing any finding. Record the score, run group, and file SHA-256 in the campaign log; if below 86, rewrite wording only and rescore once.

## Ordering and stop

Order: A1, A2, A3, A4, then B with its gate; A5 only if time remains after B. Each surface is a separate task with a fresh context. If a usage limit or time box stops work, stop cleanly at a pushed state; the campaign log is the resume point. Never leave the tree dirty between units. Finish with a "Final summary" section in the campaign log: findings by severity, repair commits, consensus-affecting repairs listed explicitly (and the re-qualification note), remaining risks.
