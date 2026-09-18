# QA campaign burn 5 brief — 2026-09-16

Fifth reset-burning QA campaign, following [burns 1 and 2](qa-campaign-20260910.md), [burn 3](qa-campaign-20260911.md), and [burn 4](qa-campaign-20260915.md). Same operator ask: heavy adversarial code review of shipped code, one surface at a time, each seen cold. Burn 5 covers crates/node modules no earlier burn reviewed, chosen so that none of them is changed by the release candidate on branch release/combined-devnet-20260915 (draft PR #41). Commit and push after every completed unit; never accumulate unpushed work.

## Boundaries (absolute)

- No Task Node action of any kind.
- No fleet mutation: no deploy, restart, configuration change, or write to any validator/RPC host or live chain.
- No pushes to any destination other than this repository's origin main. Do not touch branch release/combined-devnet-20260915 or the checkout ~/repos/postfiatl1v2-release-20260915.
- No spends, rentals, external service signups, or system-level toolchain installs. Network use: git and OpenRouter for Text Improvement Harness scoring only.
- Frozen artifacts stay frozen: attack-simulation outputs, gate outputs, deployment evidence directories, benchmarks receipts, docs/whitepaper.md, docs/whitepaper_legacy.md, locked amendment and lock records. No whitepaper work.
- Out of scope, do not review or edit: crates/privacy_orchard, crates/privacy, crates/bridge, crates/ethereum-contracts, crates/pfusdc_proofs, crates/pftl_uniswap_proofs, crates/proofs, programs/, and every file the release candidate changes relative to main — in crates/node: block_finality.rs, block_replay_wallet.rs, execution_actions.rs, fastswap_service.rs, governance.rs, governance_agent_parts/, lib.rs, lib_tests.rs, lifecycle_queries.rs, main_parts/tests/, market_bridge.rs, rpc_cli.rs, rpc_dispatch.rs, rpc_serve_runtime.rs, state_commitment.rs, state_commitment_governance.rs, tests/, transport_protocol.rs, transport_runtime.rs, transport_runtime_tests.rs, vault_bridge_conservation.rs, vault_bridge_workflows.rs; in crates/types, crates/execution, crates/storage, crates/consensus_cobalt, crates/ordering_fast: the files git diff origin/main origin/release/combined-devnet-20260915 --name-only lists. If a finding's minimal repair would have to touch one of those files, record the finding with the dependency named and do not fix it.
- Already reviewed, do not re-review: the surfaces of burns 1–4 (their review documents in docs/review/ name the files).
- A repair that changes a consensus rule, a state-transition result, an on-disk storage format, or the bytes any validator signs or hashes is consensus-affecting: allowed with a regression test, but the log and the inventory row must say so, and it must not be activated, deployed, or presented as live behaviour.
- Before every commit: the strict docs build (.venv-docs/bin/mkdocs build --strict), scripts/public-doc-links, and scripts/public-secret-scan must all pass. Plain single-sentence commit messages. Findings and fixes are separate commits.
- Word findings and repairs neutrally: input validation, fail-closed handling, bounded resources, interlocks.
- When uncertain whether an action crosses a boundary: skip it and log the skip.

## Campaign log

Create and maintain docs/review/qa-campaign-20260916.md with the structure of docs/review/qa-campaign-20260915.md: per-surface status, finding counts by severity, fix commits, skipped items with reasons, verification results (exact test commands and counts), and scores. Push an update at least once per completed surface.

## A. Review surfaces, priority order

Per surface: an adversarial fresh-eyes review, a findings document docs/review/<surface>-review-20260916.md with severity, file and line, and a concrete failure scenario per finding; then fix that surface's P1 and P2 findings with minimal changes and regression tests (P3 findings are recorded, not fixed); run cargo check and the focused tests of every touched module; commit findings and fixes separately. The full Rust suite is the CI verdict; the log says "full Rust suite verdict pending" for each repair until rust-ci is green on it.

1. Mempool proposals (docs/review/mempool-proposals-review-20260916.md): crates/node/src/mempool_proposals.rs. Focus: proposal admission and ordering, duplicate and conflicting proposals, size and count bounds, fee and nonce handling at the proposal boundary, behaviour on malformed or replayed proposals.
2. Vote locks and view recovery (docs/review/vote-locks-review-20260916.md): crates/node/src/vote_locks.rs, finality_view_recovery.rs, storage_vote_guard.rs, node_types_block_vote_timing.rs. Focus: a lock that can be bypassed or released early, equivocation across views, recovery after a crash mid-vote, timing assumptions, persistence and reload of lock state.
3. Cobalt handoff and authority (docs/review/cobalt-handoff-review-20260916.md): crates/node/src/cobalt_handoff.rs, cobalt_authority_certificate.rs, cobalt_shadow_runtime.rs, cobalt_handoff_rehearsal.rs. Focus: authority transitions accepted without the required certificate or quorum, certificate replay, rehearsal code paths reachable in live mode, shadow runtime gaining write authority, fail-closed on malformed certificates.
4. Storage migration and activation, certified-send index (docs/review/storage-migration-review-20260916.md): crates/node/src/storage_migration.rs, storage_activation_cli.rs, storage_backend_config.rs, certified_send_completed_index.rs. Focus: migration interrupted midway, activation without the preconditions, backend config that silently selects the wrong store, index inconsistency after crash, unbounded growth.
5. Swap and recovery services (docs/review/swap-recovery-services-review-20260916.md): crates/node/src/pftl_swap_service.rs, atomic_swap_rpc.rs, atomic_swap_rpc_server.rs, fastpay_recovery_node.rs, operator_attestations.rs. Focus: amount arithmetic and rounding, replay of swap or recovery messages, attestation verification, interlocks on any path that changes state, fail-closed on malformed remote input.

## B. Defect inventory

Extend docs/review/defect-inventory-20260910.md: add every burn 5 finding as a new row with a new prefix per surface (MPL-, VLK-, CHO-, SMG-, SWP-), keeping every existing row and its wording unchanged except for status updates that burn 5 repairs justify. Update the summary counts, the final disposition totals, and the completeness audit. Score the inventory with the Text Improvement Harness at the established gate (average at least 86; three judges, five reviews each, the exact models, prompt, and procedure recorded in qa-campaign-20260910.md for the burn 2 gate) and record the score, run group, and file SHA-256 in the campaign log. If the score falls below the gate, rewrite only wording, never findings, and rescore once.

## Ordering and stop

Order: A1, A2, A3, A4, A5, then B with its gate. Each surface is a separate task with a fresh context. If a usage limit or time box stops work, stop cleanly at a pushed state; the campaign log is the resume point. Never leave the tree dirty between units. Finish with a "Final summary" section in the campaign log: findings by severity, repair commits, consensus-affecting repairs listed explicitly, remaining risks.
