# Signing fix qualification hold

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-09 UTC

## BLUF

This session answered the open deployment question for consensus signing fix
[`bbb291ce`](https://github.com/postfiatorg/postfiatl1v2/commit/bbb291ce761673fc8df6aee874308dec0cff6da6)
with a [fresh read-only fleet observation](../status/chain-state-current.md), a
[local release qualification](https://github.com/postfiatorg/postfiatl1v2/tree/b6c13c9f675472a63b296994974042f41f102511/deployments/signing-fix-qualification-20260909),
and a harness-scored [decision sheet](../governance/signing-fix-deploy-decision-20260909.md).
The result is **HOLD — LEAN C**: the candidate built from `main` is not
deployment-qualified, chiefly because `main` omits validator-runtime behavior
present in the deployed A666 source lineage and consolidated in open PR #39.
No fleet or live-chain mutation occurred.

## Current state

- **Observed fleet:** The capture ran from `2026-09-09T09:24:54Z` through
  `09:27:00Z`. Validators 1–5 agreed at height 1020, state root
  `587c6526…d39bead6`, with empty mempools, 15 blocks beyond the September 7
  observation. Validator-0's RPC reads timed out; its host and services were
  up, but its ledger values remain unknown. Resolving that RPC state is a
  deployment precondition.
- **Deployed identity:** All 12 validator/RPC processes ran release
  `a666-source-route-20260907`, binary SHA-256
  `57b0f4d1…634eec83`. The signing fix is not deployed, so its cross-phase
  monotone-round invariant does not govern live signing. The observation used
  read-only queries and identity checks only.
- **Repository boundary:** Observation source
  [`4153376b`](https://github.com/postfiatorg/postfiatl1v2/commit/4153376b8097670ed1af0319affdf204ed85b2f5)
  produced candidate binary `af4ccc3e…947def`; pre-handoff `origin/main` is
  qualification commit
  [`b6c13c9f`](https://github.com/postfiatorg/postfiatl1v2/commit/b6c13c9f675472a63b296994974042f41f102511).
  Both commits and `bbb291ce` are reachable from `origin/main`, but repository
  ancestry alone is not deployment evidence.
- **Qualification result:** The locked build passed; exact-hash reproduction
  failed because the second build differed in six ELF `RUNPATH` bytes. The
  deployed-source manifest reconstructed 26/26 files, but the signing-only
  lineage gate failed. The signed height-931 archive and six loopback clones
  passed import, verification, transactional rebuild, both service start
  orders, finality from 933 to 934, root convergence, and restart. That is
  archival compatibility evidence, not a current-height rehearsal. The
  focused delayed-certificate regression, 20 consensus tests, three
  transport/RPC tests, and Clippy passed. A current-height all-six snapshot and
  the exact deployed rollback binary were unavailable locally. The
  [qualification receipt](https://github.com/postfiatorg/postfiatl1v2/blob/b6c13c9f675472a63b296994974042f41f102511/deployments/signing-fix-qualification-20260909/qualification-receipt.json)
  records `HOLD_NOT_DEPLOYMENT_QUALIFIED`; no release manifest or rollout
  authorization exists.
- **Lineage consequence:** A binary built from current `main` would omit the
  fleet's live Arc/PFETH source-route behavior. Open draft
  [PR #39](https://github.com/postfiatorg/postfiatl1v2/pull/39) consolidates
  that behavior and is stacked on the open draft
  [PR #37](https://github.com/postfiatorg/postfiatl1v2/pull/37) integration
  branch. Merge order or an equivalent lineage decision must precede a fresh
  qualification; neither PR is live authority.
- **Threat boundary:** The defect requires a delayed valid certificate and
  specific timing. No live exploit was demonstrated, but the pre-fix fleet
  remains exposed to the condition described by the
  [signing-round contract](../architecture/consensus-signing-rounds.md).
- **Other work:** The operator reports the workstation-side send/exec quoting
  and temporary-file race fixed; server-side code was unaffected. No Task Node
  action occurred. The separate
  [whitepaper and consensus handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/handoffs/2026-09-09___codex__whitepaper_and_consensus_to_dravlic.md)
  retains the unfinished abstract-count, consensus-section, and defect-inventory
  work and was not modified here.

## Next decision or action

1. Decide the release lineage: merge PR #37 then PR #39, or record an
   equivalent route that preserves the deployed A666 runtime.
2. Before reconsidering deployment, resolve validator-0 RPC, obtain an
   authorized current-height all-six snapshot, produce byte-identical release
   builds, retain the exact rollback binary, and rerun the decision sheet's
   deployment-exact gates.
3. Complete the whitepaper abstract-count and consensus-section corrections
   and the source-backed defect inventory.
4. Older open work remains: decide whether to push the
   [StakeHub PR #8 fix](../review/stakehub-pr8-review-20260907.md), adopt the
   [September 3 direction](2026-09-03___dravlic__agent_direction_decided_z3_planned.md),
   and begin [Z3 G1](../plans/active/z3-navcoin-roundtrip-plan.md).

## References

- [Canonical controlled-devnet state](../status/chain-state-current.md)
- [Signing-fix deployment decision](../governance/signing-fix-deploy-decision-20260909.md)
- [Qualification evidence](https://github.com/postfiatorg/postfiatl1v2/tree/b6c13c9f675472a63b296994974042f41f102511/deployments/signing-fix-qualification-20260909)
- [Consensus signing-round contract](../architecture/consensus-signing-rounds.md)
- [Separate whitepaper and consensus handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/handoffs/2026-09-09___codex__whitepaper_and_consensus_to_dravlic.md)
