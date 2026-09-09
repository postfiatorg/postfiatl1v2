# Consensus signing-fix deployment decision

**Status:** `HOLD — LEAN C`  
**Decision date:** 2026-09-09 UTC  
**Authority boundary:** Decision support only. This sheet authorizes no deploy,
restart, configuration change, host access, chain write, or rollout.

## Question

Should a release carrying consensus signing fix `bbb291ce` be deployed to the
six controlled-devnet validators?

## Decision summary

**Lean C — hold for a fuller release-lineage and deployment-exact campaign.**
The signing regression suites pass, but the candidate built from current
`main` is not deployment-qualified. Its second clean build did not reproduce
the same binary hash; current `main` is not a descendant of the deployed A666
source commit and omits deployed validator-runtime behavior; the available
local real-host snapshot is height 931 rather than the current height 1020;
and the exact deployed rollback binary is not present on this server. Moving
the work to another shift does not resolve those conditions.

## What is true

- The [2026-09-09 fleet observation](../status/chain-state-current.md) found
  validators 1–5 agreeing at height 1020, tip `9d02b8ee…b1768feb`, and state
  root `587c6526…d39bead6`, with empty mempools. Validator-0's three RPC reads
  timed out, so its ledger state remains unknown. Read-only process checks
  found all six hosts running release `a666-source-route-20260907`, binary
  SHA-256 `57b0f4d1…634eec83`. That binary does not contain `bbb291ce`.
- The qualification source is `4153376b8097670ed1af0319affdf204ed85b2f5`.
  A locked release build produced candidate SHA-256
  `af4ccc3e3f7ca626b309de92e1172ccd60f0f0854f2731a6edd3dbbc9a947def`.
  A clean second build produced
  `f82dc42ef177403f0d7b0f0f8252eeb7b32fa03115870807189f3ce07bb9611c`.
  The files have the same size and differ in six bytes: a randomized rustc
  temporary-directory component in the ELF `RUNPATH`. No post-build rewrite
  was accepted as release evidence. The exact-hash reproducibility gate fails.
- The deployed source record pins base commit `707e006f`, patch
  `deaf05d4…6586c`, and its source manifest. `707e006f` is not an ancestor of
  current `main`; their merge base is `3f393b76`. Exact tree comparison shows
  that current `main` omits deployed Arc/PFETH source-route types, custody,
  settlement, replay, and state-commitment behavior. Deploying this candidate
  would therefore be a runtime lineage change, not a signing-only update.
- The local gate used a signed real-host snapshot captured earlier at height
  931, tip `8e3639ee…665c4b`, and root `ef18f8ca…fd55e0f`. The candidate imported
  and verified that snapshot on six isolated clones, reproduced one shared
  transactional migration packet root `5f93a4e1…aa11d1`, started transport and
  RPC in both service orders, committed one local finality round, converged on
  one new root, and retained that identity after restart. This is an archival
  compatibility pass, not evidence about the present height-1020 state.
- The exact delayed-certificate regression passed. The documented focused
  library suites passed 20 tests, the transport/RPC suites passed 3 tests, and
  focused Clippy passed with warnings denied. These results establish the
  signing-fix test boundary, not release qualification.
- No validator host, validator service, fleet configuration, or live chain was
  touched during this qualification. Evidence and the loopback-only runner are
  in the [dated qualification record](https://github.com/postfiatorg/postfiatl1v2/tree/main/deployments/signing-fix-qualification-20260909).

## Validator-runtime differences

| Difference from the deployed release | Runtime consequence |
| --- | --- |
| `bbb291ce` adds one durable cross-phase signing floor across prepare, precommit, and timeout, including restored snapshots. | Intended safety fix; lower-round cross-phase signatures fail closed. |
| Current `main` omits the deployed A666 Arc/PFETH source-route lineage, including source custody, settlement execution, transaction types, replay handling, and state commitments. | Blocking compatibility risk. A height-1020 clone may replay differently or produce a different state root. |
| Merged YOLO target-receipt code adds receipt validation, state commitment, and RPC/query surfaces. | Validator runtime surface is larger, but activation remains unscheduled and default-disabled under the recorded option-A composition decision. |
| Subsequent CI, genesis-vector, Python UNL V2, report, and documentation commits do not alter active validator consensus behavior. | Test/tooling/documentation-only for this deployment question. |

## Options and consequences

### A — Deploy now through the documented rollout procedure

This would place the signing fix on the fleet sooner, but it would also deploy
an unreproduced binary from a source tree that does not preserve the running
A666 lineage. Validator-0's current ledger identity is unknown, no current
height-1020 six-clone gate exists, and the exact rollback binary is absent
locally. **A is not safe under the evidence available today.**

### B — Hold for the other operator to roll out on his shift

This changes the operator and timing, not the technical evidence. It is viable
only if that operator first completes every precondition below and records a
new deployment-exact receipt. A shift handoff alone is not qualification and
does not authorize a rollout.

### C — Hold for a fuller campaign

Construct a successor source that preserves the exact deployed A666 runtime
lineage while applying `bbb291ce`, reproduce its binary on two clean builders,
obtain authorized fresh copies of all six current validator states, and rerun
the deployment-exact migration, replay, service-order, finality, restart, and
exact rollback gates. This delays the fix, but it directly resolves the
identified lineage, current-state, rollback, and reproducibility risks.

## Preconditions for any option

These conditions apply whichever option is selected:

1. Resolve validator-0's RPC state and establish one contemporaneous height,
   tip, state root, registry, and mempool view across all six validators.
2. Reconcile the candidate with the deployed A666 source manifest. Preserve
   its live runtime behavior or explicitly migrate and qualify every changed
   state field and transition; do not treat divergent Git lineages as an
   additive commit range.
3. Produce byte-identical locked release binaries on two clean builders and
   bind the accepted hash, source tree, toolchain, lockfile, and build command
   in the release receipt.
4. Run the documented deployment-exact gate on fresh local copies of the
   current all-six state. Require replay and transactional rebuild to preserve
   every expected root; require both transport/RPC start orders, one certified
   continuation, root convergence, and restart identity.
5. Retain and hash the exact running A666 binary plus a schema-compatible
   pre-upgrade snapshot. Prove exact data-plus-binary rollback before canary.
6. Re-run the focused signing regression suites and all release/gate checks on
   the exact accepted candidate. A passing source test with a different binary
   hash does not satisfy this condition.
7. During an authorized canary, stop immediately unless heights advance,
   state roots agree across validators, and delayed-certificate rejection is
   observable in logs without new equivocation, finality, storage, or mempool
   errors.

## Lean

**Lean C.** The signing fix addresses a real cross-phase invariant and should
remain the goal, but the candidate is not a signing-only successor to the live
release and failed exact-hash reproduction. A fuller campaign is the shortest
path that tests the code actually intended for deployment rather than assuming
away the deployed lineage and current-state gaps.

## Non-authorization

This sheet records evidence, options, and a lean. It performs and authorizes
nothing. Deployment still requires a separate operator decision followed by
the documented safe-rollout procedure, signed manifest checks, canary stop
conditions, and retained rollback pair.

## References

- [Consensus signing-round contract](../architecture/consensus-signing-rounds.md)
- [Current controlled-devnet status](../status/chain-state-current.md)
- [Release process](../release-process.md)
- [Safe validator rollout](../runbooks/safe-validator-rollout.md)
- [Storage rollout plan](../plans/active/devnet-storage-rollout-plan.md)
- [Signing-fix qualification evidence](https://github.com/postfiatorg/postfiatl1v2/tree/main/deployments/signing-fix-qualification-20260909)
