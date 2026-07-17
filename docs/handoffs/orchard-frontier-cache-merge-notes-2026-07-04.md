# Orchard Frontier Cache Merge Notes

Date: 2026-07-04

## Scope

Prepared merge review for the four validated frontier-cache commits on `orchard/zk-latency-instrument-0655`:

- `b5ac1eff` - Add non-consensus Orchard frontier cache
- `490ed24f` - Move swap VK metadata checks off proof hot path
- `27aacbb2` - Add warm shielded certifier loop path
- `4e7c1413` - Wait for shielded certifier loop readiness in e2e

Target checked: `origin/main` at `30b84d87` (`Complete Step 10 runner repair and zk latency prep`).

## Rebase / Cherry-Pick Result

The four-commit stack is not clean onto `origin/main` as of this check.

Evidence:

- Temporary worktree: `$POSTFIAT_REPO-frontier-merge-prep-20260704`
- `b5ac1eff` cherry-picked cleanly.
- `490ed24f` cherry-picked cleanly.
- `27aacbb2` conflicted in `wallet-proxy/server.js`.
- `4e7c1413` was not reached because the sequence stopped at the `27aacbb2` conflict.

Interpretation: the consensus/cache commit itself is not the blocker. The conflict is in the wallet-proxy warm certifier loop surface. Merge should either include/fold the precursor proxy latency/warm-routing commits that `origin/main` does not yet contain, or resolve the proxy conflict manually during the merge. No push and no merge were performed.

## Consensus Safety Invariant

The Orchard frontier cache must remain non-consensus local acceleration data.

Required invariant: `frontier_cache` must never be serialized by `append_orchard_pool_state`. The current implementation adds `frontier_cache` to `OrchardPoolState` with serde defaulting for local snapshot migration, but `crates/node/src/lib_parts/part_02_parts/part_02.rs:4302` continues to serialize canonical pool contents without the cache.

Tests to cite in review:

- `orchard_frontier_cache_does_not_affect_replicated_state_root`
- `orchard_frontier_cache_malformed_parts_fall_back_to_full_recompute`
- `orchard_frontier_snapshot_incremental_root_matches_full_after_each_append`
- `orchard_frontier_snapshot_rejects_malformed_cached_root`

The key state-root safety test constructs equivalent shielded state with and without `frontier_cache` and asserts the replicated state root is byte-identical.

## Serde Lazy Migration

Old snapshots deserialize safely because `frontier_cache` is `Option<OrchardFrontierCache>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

Migration behavior:

- Old state has `frontier_cache: None`.
- On first root-history/apply/warm path, the node reconstructs from canonical `output_commitments`.
- A valid cache then accelerates future appends.
- Missing, stale, ahead, or malformed cache data is ignored and rebuilt from canonical commitments.

This means old snapshots do not require a state migration and do not alter the replicated state root.

## VK Metadata Check Removal

The VK hot-path change moves repeated release metadata work out of every swap proof verification. The release-pinned swap verifying key remains validated when the cached key is constructed; proof verification then uses the cached verifying key and records timings only when `POSTFIAT_ORCHARD_TIMING_STDERR` is set.

Reviewer attention:

- Confirm release-pin validation is still covered at cached key construction.
- Confirm no runtime request can swap in unpinned VK material.
- Confirm the timing probes do not change proof inputs, public instances, or consensus bytes.

## Merge Recommendation

Do not claim the four commits rebase clean onto `origin/main` without a proxy conflict resolution. For a clean reviewer path, split or order the merge as:

1. Consensus/cache commit and VK hot-path commit.
2. Proxy warm-loop conflict resolution, with precursor proxy routing context included or manually reconciled.
3. E2E wait-for-certifier readiness harness commit.

Keep docs/evidence directories untracked unless explicitly requested for archival.

## Known Issues / Reviewer Follow-Up

The 2026-07-04 Task 0 fix addresses the stale one-round warm certifier loop, batch/report cross-checking, unhandled startup loop rejection, and stale-suffix current-root bug. The following lower-priority review items remain intentionally recorded for merge review:

- `wallet-proxy/server.js:9005` / `wallet-proxy/server.js:9050`: add stronger orphaned-child/PID tracking if the proxy is terminated mid-round. Current behavior keeps the spawned child in loop state and logs the PID at `wallet-proxy/server.js:11932`, but there is not yet a dedicated shutdown hook.
- `wallet-proxy/server.js:8989` / `wallet-proxy/server.js:9019`: `shieldedCertifierLoopStartHeight` is captured at loop spawn; reviewers should confirm this is still correct if startup prewarm is left idle for a long time before first batch.
- `crates/node/src/privacy_parts/part_01.rs:300` / `crates/node/src/privacy_parts/part_01.rs:341` and `crates/node/src/main_parts/cli_dispatch_parts/group_05.rs:1051`: document operator policy that the `orchard-frontier-cache-warm` read-modify-write path is node-stopped-only unless future locking is added.
- `wallet-proxy/server.js:9455`, `wallet-proxy/server.js:9597`, and `wallet-proxy/server.js:9973`: ensure laggard catch-up writes into the intended per-round artifact directory consistently when early-quorum is enabled, especially when warm-loop certification uses `certified_artifact_dir` from `wallet-proxy/server.js:9963`.
