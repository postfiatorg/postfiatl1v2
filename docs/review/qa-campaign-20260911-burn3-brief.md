# QA campaign burn 3 brief — 2026-09-11

Third autonomous reset-burning QA campaign, following the completed [burn 1](qa-campaign-20260910.md) and its burn 2 section. Same operator ask: heavy adversarial code review, burn the available usage on it. Burn 3 covers the shipped surfaces that burns 1 and 2 did not review. Work in priority order until the surfaces are exhausted or usage stops progress; commit and push after every completed unit; never accumulate unpushed work.

## Boundaries (absolute)

- No Task Node action of any kind.
- No fleet mutation: no deploy, restart, configuration change, or write to any validator/RPC host or live chain. Read-only ledger/status endpoints only, and only if a unit needs them.
- No pushes to StakeHub or any destination other than this repository's origin main.
- No spends, rentals, external service signups, or system-level toolchain installs. Network use: git, documented read-only endpoints, and OpenRouter for Text Improvement Harness scoring only.
- Frozen artifacts stay frozen: the V1 attack-simulation outputs, the V2 gate outputs, all deployment evidence directories, benchmarks receipts, docs/whitepaper.md, docs/whitepaper_legacy.md, and the locked amendment and lock records. No whitepaper work in this burn.
- Out of scope, do not review or edit: crates/privacy_orchard, crates/privacy, crates/bridge, crates/ethereum-contracts, crates/pfusdc_proofs, crates/pftl_uniswap_proofs, and programs/. Another lane is reworking that bridge and Orchard code off this repository; changes here would collide. If a finding elsewhere depends on that code, record it with the dependency named and do not fix it.
- A repair that changes a consensus rule, a state-transition result, or an on-disk storage format is consensus-affecting: it is allowed with a regression test, but the log and the inventory row must say so, and it must not be activated, deployed, or presented as live behaviour.
- Before every commit: the strict docs build (.venv-docs/bin/mkdocs build --strict), scripts/public-doc-links, and scripts/public-secret-scan must all pass. Plain single-sentence commit messages. Findings and fixes are separate commits.
- When uncertain whether an action crosses a boundary: skip it and log the skip.

## Campaign log

Create and maintain docs/review/qa-campaign-20260911.md: per-surface status (pending, reviewing, fixing, done), finding counts by severity, fix commits, skipped items with reasons, verification results, and scores. Push an update at least once per completed surface. A reader of only this file must understand the campaign's state.

## A. Review surfaces, priority order

Per surface: an adversarial fresh-eyes review (clear or compact context between surfaces so each is seen cold), a findings document docs/review/<surface>-review-20260911.md with severity, file and line, and a concrete failure scenario per finding; then fix that surface's P1 and P2 findings with minimal changes and regression tests (P3 findings are recorded, not fixed); rerun the surface's focused tests; commit findings and fixes separately.

1. Storage and snapshots: crates/storage in full, plus the snapshot export, restore, checkpoint, and writer-lease paths in crates/node. The transactional redb storage has been live on the devnet since 2026-08-31 with a single-writer lease; the fleet-wide snapshot finalized-checkpoint export defect at block 924 is recorded as open in docs/status/chain-state-current.md. Focus: durability under crash and partial write, lease and writer-fencing races, restore from a tampered or truncated snapshot, migration and rollback paths, unbounded growth.
2. Execution: crates/execution in full and the state-transition entry points it serves in crates/node. Focus: fee and balance arithmetic (overflow, rounding, negative paths), replay and nonce handling, ordering dependence within a block, registry and validator-set transitions, resource bounds on committed state beyond the YOLO registration bound repaired on 2026-09-10 (commit f9f13ead).
3. Cobalt ratification: crates/consensus_cobalt and its decision and adversarial oracles (crates/cobalt_decision_oracle, crates/cobalt_adversarial_oracle). Cobalt-ratified transitions are the sole authority on the devnet. Focus: ratification quorum and threshold arithmetic, equivocation and replay of ratification messages, decision determinism across nodes, publication and evidence binding beyond the immutable-bytes check repaired in commit acdbb1f2.
4. Network and mempool admission: crates/network, crates/mempool_dag, and the non-signing parts of crates/ordering_fast. Focus: message size and rate bounds, peer admission and eviction, memory growth under hostile peers, malformed message handling, DoS surfaces reachable by an unauthenticated peer.
5. Operational Python CLIs in python/postfiat_rpc other than the tasknode_unl modules reviewed on 2026-09-10: cobalt.py, navcoin.py, hyperliquid.py, pftl_transfer.py, wallet.py, storage_scaling.py, genesis_registry.py, client.py, persistent_client.py. Focus: key and secret handling, fail-closed behaviour on malformed remote responses, silent wrong answers, and any path that could mutate a live system without an explicit interlock.

## B. Defect inventory

Extend docs/review/defect-inventory-20260910.md: add every burn 3 finding as a new row with a new prefix per surface (STO-, EXE-, COB-, NET-, OPS-), keeping every existing row and its wording unchanged except for status updates that burn 3 repairs justify. Update the summary counts. Near campaign end, score the inventory with the Text Improvement Harness at the established gate (average at least 86; three judges, five reviews each, the exact models, prompt, and procedure recorded in qa-campaign-20260910.md for the burn 2 gate) and record the score, run group, and file SHA-256 in the campaign log. If the score falls below the gate, rewrite only wording, never findings, and rescore once.

## Ordering and stop

Order: A1, A2, A3, A4, A5, then the final B with its Text Improvement Harness gate. If a usage limit pauses work, stop cleanly at a pushed state; the campaign log is the resume point. Never leave the tree dirty between units. Finish with a "Final summary" section in the campaign log: findings by severity, repair commits, consensus-affecting repairs listed explicitly, remaining risks.
