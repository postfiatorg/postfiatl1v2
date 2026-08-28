# Storage G4 structural vote-path failure

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

The completed offline G4 campaign did **not** qualify the transactional storage
candidate. The redb store itself passed the per-window correctness and
bounded-storage-work checks, but end-to-end latency became 2.8 times worse
between height 50 and height 5,000. Source tracing identifies the remaining
unbounded path: before a validator signs each block vote, it scans and
JSON-parses every historical anti-equivocation lock file in its data directory.
There are 49 lock files at height 50 and 4,999 at height 5,000 per validator.
That full-history scan runs on all six validators in the consensus vote path, so
the chain still becomes slower as history grows. The release report therefore
ends `PUBLIC TESTNET BLOCKED` and `evidence_eligible: false`.

## Current state

### What finished

The prepared-input G4 run completed at `2026-08-28T15:19:46Z` in
3,311.552 seconds, inside the 14,400-second measurement budget. It ran the full
unchanged matrix:

- five selected-redb windows at height 50;
- five selected-redb windows at height 5,000;
- five legacy-JSONL control windows at height 50;
- 50 finalized rounds per window, for 750 measured rounds total;
- six validators, exact signed inputs, literal receipts, and converged final
  roots in every window.

This was a local offline experiment. It made no network contact, did not query or
mutate the controlled devnet, did not deploy anything, and did not run a live
fleet probe.

The exact run identities are:

| Item | Bound value |
| --- | --- |
| Candidate source | `ae65844190f153cbdd49d1e5ac28ab96a19f7af4` |
| Runner source | `1f478e0c473de42ecf43b4dd0925893de8f181ed` |
| Node binary SHA-256 | `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4` |
| Prepared-input manifest SHA-256 | `9ac31841a41ba514855a82f52650e1951ed97c9f99d54a4048a07407d6734c61` |
| Final campaign report SHA-256 | `88502bca7aaa4e576e5e9684b3d9b72d8c1b66e24b6c6c8e746f11807ac7eabb` |
| Final checkpoint SHA-256 | `36a3287218f3e2a67071dda070ea5dd879ad4c7f1b6ff77bc229169d0af639dd` |

The private run directory is
`/home/postfiatchad/repos/postfiat-storage-g4-measurement-1f478e0c-ae658441-v1`.
It contains validator private material and must not be committed or published.

### The measured failure

| Measure | Height 50 | Height 5,000 | High/low ratio | Required |
| --- | ---: | ---: | ---: | ---: |
| Consensus-round p95 | 690.473 ms | 1,938.756 ms | **2.808** | ≤ 1.10 |
| Wallet-to-finality p95 | 706.950 ms | 1,952.852 ms | **2.762** | ≤ 1.10 |

The selected store was faster than the legacy JSONL control at height 50:
selected/legacy was 0.667 for consensus-round p95 and 0.673 for
wallet-to-finality p95. The selected store also reported bounded page work,
constant accumulator work, and zero full-history storage reads or scans in all
ten selected windows. The storage-apply and write-commit stages did not show a
material positive height relationship.

The failure moved elsewhere in the critical path:

| Stage | Modeled increase, height 50→5,000 | R² | Gate result |
| --- | ---: | ---: | --- |
| Local validator vote | +60.215 ms | 0.996 | material positive relationship |
| Remote vote requests | +52.946 ms | 0.991 | material positive relationship |
| Local state apply | +2.386 ms | 0.402 | not material |
| Storage write/commit | +1.552 ms | 0.541 | not material |

The first height-5,000 window also recorded four simultaneous remote
`vote_lock_reservation_ms` stalls of approximately 779, 978, 983, and 989 ms.
The whole-round maximum at height 5,000 was 2,829.692 ms. These stalls occurred
before ML-DSA signing; the target validation and signature work themselves were
small.

The campaign-level booleans are easy to misread. `comparison_windows_pass`
and `window_gates_pass` are true because each individual window finalized
correctly with bounded redb work. The release still fails because
`no_positive_linear_height_relationship` is false and both required
high-height ratios exceed 1.10. “Complete” means that the experiment finished,
not that the candidate passed.

### Why this is structurally collapsing

Every finalized block leaves a separate JSON vote-lock file under
`block_proposal_vote_locks`. The lock preserves the safety rule that a validator
must not sign two conflicting proposals for the same consensus slot.

Before creating the next signature, the code calls
`reserve_block_proposal_vote_lock`
([block_finality.rs:2477-2506](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/block_finality.rs#L2477-L2506)).
That function first calls `validate_prior_block_proposal_vote_locks`, opens the
lock directory, iterates every JSON file, reads it, and deserializes it before it
checks whether the record is even for the current height and validator
([block_finality.rs:2585-2658](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/block_finality.rs#L2585-L2658)).

The resulting work is proportional to accumulated chain history:

```text
per validator, per new block:
    enumerate all historical vote-lock files
    read and JSON-decode every file
    discard almost all of them because their height is not current
    only then reserve the new lock and sign
```

The prepared fleets make that growth concrete:

| Prepared height | Vote-lock files per validator | Directory size per validator |
| --- | ---: | ---: |
| 50 | 49 | about 208 KiB |
| 5,000 | 4,999 | about 21 MiB |

The local vote and remote validator requests run concurrently, but the round
joins the vote workers before certificate construction
([transport_runtime.rs:2975-3167](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/transport_runtime.rs#L2975-L3167)).
The round therefore inherits the slowest validator's directory scan. Adding
validators does not remove this cost; each validator repeats it against its own
growing history.

This is structural rather than a random slow run because:

1. the input, binary, host, validator count, and 5×50-round matrix were fixed;
2. all five high-height windows were slower than the low-height windows in the
   two affected vote stages;
3. the stage relationship has R² above 0.99 at the two measured heights;
4. the source contains the exact full-directory algorithm predicted by the
   timing data; and
5. the on-disk file count grows one-for-one with finalized height.

The experiment used two height levels, so it does not prove a universal linear
latency formula for every future height. It does prove a reproducible,
height-sensitive failure and exposes an O(chain-history) algorithm in the
consensus signing path. Re-running the same candidate cannot fix that algorithm.

### What is and is not broken

- The transactional redb append/state path is not the cause found by this run.
  Its bounded-work counters and individual-window gates passed.
- Consensus safety did not fail in the experiment. All measured transactions
  finalized, literal receipts matched, and the six validators converged.
- Consensus **scalability and liveness margin** failed. A safety mechanism that
  should be a bounded lookup instead performs a lifetime-history scan before
  every vote.
- The storage release remains unqualified even though its primary redb component
  behaved correctly, because public-testnet readiness is an end-to-end property.
- G3's exact height-924 replay remains separately open pending an expressly
  authorized read-only validator-data copy.
- G5 packet qualification and all deployment work remain blocked. No devnet
  conclusion follows from this offline run.

At handoff time the repository is `main` at
`1f478e0c473de42ecf43b4dd0925893de8f181ed`, synchronized with
`origin/main`. The vote-lock implementation in this commit is byte-identical to
the tested candidate's `crates/node/src/block_finality.rs`. The active storage
milestone has not yet been updated with this final failed result and must not be
read as newer than this handoff.

## Next decision or action

Do not repeat G4 and do not deploy this candidate. The next bounded work is a
vote-lock indexing fix with explicit safety gates:

- [ ] Specify a crash-safe anti-equivocation index keyed by chain, validator,
  height, and view. A current-slot lookup must be O(1) or O(log n); it must not
  enumerate historical lock files in the signing path.
- [ ] Preserve fail-closed behavior for conflicting proposals, legacy
  one-proposal-per-height locks, Consensus v2 one-proposal-per-(height, view)
  locks, restarts, partial writes, and mixed old/new data directories.
- [ ] Implement a one-time, resumable migration or index rebuild for existing
  JSON locks. Historical validation may happen during explicit migration or
  repair, but never on every vote.
- [ ] Add source-level and executable tests for same-slot replay, conflicting
  proposal rejection, restart durability, truncated/tampered index state,
  migration from legacy locks, and rollback of an interrupted migration.
- [ ] Add a direct gate that reports vote-lock files examined and bytes decoded
  per vote. Normal operation must remain bounded at height 50, 5,000, and a
  higher synthetic height.
- [ ] Re-run the unchanged 5+5+5 G4 measurement with one clean candidate binary.
  Require both high/low ratios ≤1.10, no material positive height relationship,
  all existing receipt/convergence gates, and bounded vote-lock work.
- [ ] Obtain separate authorization and a named custodian before copying any
  height-924 controlled-devnet validator directory for G3.
- [ ] Only after G3 and the corrected G4 pass, build and independently verify G5.
  Deployment remains a later, separately authorized action.
- [ ] Update the active milestone with this result before starting implementation
  so “campaign complete” cannot be mistaken for “candidate qualified.”

No Task Node work, controlled-devnet access, service action, deployment, or
network mutation is authorized by this handoff.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Storage G4 v4 preflight](2026-08-28___postfiatchad__storage_g4_v4_preflight.md)
- [Storage G4 time-budget decision](2026-08-28___dravlic__storage_g4_time_budget_decision.md)
- [Storage G4 timeout and persistent remediation](2026-08-28___postfiatchad__storage_g4_timeout_and_persistent_remediation.md)
- [Current recorded network state](../status/chain-state-current.md)
- [Vote creation and lock timing](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/block_finality.rs#L2477-L2658)
- [Consensus vote fan-out and joins](https://github.com/postfiatorg/postfiatl1v2/blob/1f478e0c473de42ecf43b4dd0925893de8f181ed/crates/node/src/transport_runtime.rs#L2975-L3167)
