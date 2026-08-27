# Storage verifier and campaign reset

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-27 UTC

## BLUF

The active [storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
is still open and public testnet remains blocked. Source `785806bd` makes
`storage-rebuild-transactional --verify-only` genuinely non-mutating and passes
the expanded storage/node qualification, but the interrupted three-binary
performance run is invalid evidence and the current paired runner conflicts
with the locked research specification's same-binary/same-snapshot rule.

## Current state

- The working branch is `main`. The code fix is
  `785806bdb13c417570b6676b39b1638bdc226517`; the milestone/current-state update
  immediately before this handoff is `8c9a8e82`.
- Read-only verification now requires existing source/target material, opens
  the source integrity boundary and `redb` target read-only, refuses a pending
  ordered-commit journal, reconstructs a missing chain tip only in memory, and
  never upgrades a v1 head or repairs a crash suffix.
- Whole-directory mutation sentinels cover missing source and target,
  successful existing-target verification, missing chain tip, pending journal,
  stale generation, v1 head, and partial crash suffix.
- `cargo test --locked -p postfiat-storage` passed 80 tests plus its process
  crash integration test; two manual scaling tests were ignored. `cargo test
  --locked -p postfiat-node --lib` passed 315 tests; two unrelated
  Foundry-gated tests were ignored. Formatting and Clippy for storage/node with
  warnings denied passed.
- The private partial campaign on `0fdcc2b3` completed every legacy height-50
  and height-100 window, then was stopped before a final report. It was built on
  the mutating verifier and used three binaries with lane-native snapshots, so
  it is evidence-ineligible and closes no performance gate.
- The checked runner still enforces three distinct historical binaries and
  snapshots. The locked research specification requires one binary with only
  storage mode changed and the same authenticated snapshot. Do not restart the
  full campaign until that invariant is implemented and verified.
- Exact height-924 replay and the clean six-clone height-924 migration remain
  unavailable locally. Fleet copy, probe, deployment, service restart, or live
  mutation still require separate operator authorization.
- No live probe occurred in this session. The last observed fleet state remains
  the point-in-time six-validator height-924 capture from
  `2026-08-26T06:34:55Z`–`06:35:50Z`, with deployed node source `8cc7d15e` and
  binary SHA-256 `d5e5ef63…c2696caf`; see [Current State](../status/chain-state-current.md).
- No Task Node or subagent was used. The Dynamic UNL milestone is deferred and
  is not active storage work.

## Next decision or action

Implement an explicit typed local storage-mode boundary so one release binary
can run legacy JSONL, bounded JSONL, and selected transactional modes from one
portable authenticated snapshot without changing consensus inputs. Then change
the paired runner, packet verifier, and tests to require one binary/source and
one snapshot identity across all lanes, run a three-lane development smoke, and
only then start the five-height release campaign. Keep exact height-924 and all
fleet actions open until separately authorized material is available.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Locked storage scaling research specification](../architecture/storage-scaling-research-spec.md)
- [State and storage architecture](../architecture/state-and-storage.md)
- [Storage evidence workflow](https://github.com/postfiatorg/postfiatl1v2/tree/785806bdb13c417570b6676b39b1638bdc226517/benchmarks/storage-scaling)
- [Independent storage candidate review](2026-08-27___dravlic__storage_candidate_review.md)
- [Current State](../status/chain-state-current.md)
