# CI repair, UNL hardening, and StakeHub review

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-07 UTC

## BLUF

This session took ownership of the open CI-repair item. Four repair rounds
([`4a961eeb`](https://github.com/postfiatorg/postfiatl1v2/commit/4a961eeb1bcfd64dd6e186c55307476e8e0058c3),
[`343a2fe9`](https://github.com/postfiatorg/postfiatl1v2/commit/343a2fe923cbed5265062e2713e3f0a5de9f0a5a),
[`a02b29fb`](https://github.com/postfiatorg/postfiatl1v2/commit/a02b29fb5543ad513d94920594d595b3a7ac80cd),
and
[`09cb4e00`](https://github.com/postfiatorg/postfiatl1v2/commit/09cb4e0057803fbb01c2ed924386b025b0d53cae))
addressed twelve distinct causes across the six previously red jobs, with each
target reproduced using its workflow command and no check weakened. The final
`product-security-ci` and `rust-ci` runs on `09cb4e00` were still executing
when this handoff was written. In parallel, the Task Node UNL MVP was hardened
and retired to completed plans, its proposal received a quantitative
[attack simulation](../governance/tasknode-unl-attack-simulation-20260907.md),
PR #38 received a
[deploy decision sheet](../governance/yolo-deploy-decision-20260907.md), and
StakeHub PR #8 received a full independent
[review](../review/stakehub-pr8-review-20260907.md) with three P1 blockers.

## Current state

### CI repair

- [`4a961eeb`](https://github.com/postfiatorg/postfiatl1v2/commit/4a961eeb1bcfd64dd6e186c55307476e8e0058c3)
  restored the A666 regression manifest byte-for-byte from committed history,
  replaced validator-IP defaults with a required environment variable, aligned
  Python SDK assertions with intentional upstream behavior, and restored wallet
  browser coverage.
- [`343a2fe9`](https://github.com/postfiatorg/postfiatl1v2/commit/343a2fe923cbed5265062e2713e3f0a5de9f0a5a)
  updated the reviewed direct-proving test positions to 445 and 636, classified
  `pfusdc_ingress_preflight` and `yolo_target_receipt` as public reads,
  repaired the provider-neutral wallet boundary with narrow documented
  exclusions, and redirected reserve fuzzing from deleted documentation
  evidence to the committed benchmark corpus.
- [`a02b29fb`](https://github.com/postfiatorg/postfiatl1v2/commit/a02b29fb5543ad513d94920594d595b3a7ac80cd)
  hash-pinned twelve new public artifacts and pinned the absent epoch-7
  observation qualification to its reachable
  [verification record](https://github.com/postfiatorg/postfiatl1v2/commit/651bfa6282b730f0f7c09871f1c8e999ae1eda8b)
  and
  [recorded removal](https://github.com/postfiatorg/postfiatl1v2/commit/de115b542d990d40ff43416c2db7d531bbb883d0);
  no missing archive was invented.
- [`09cb4e00`](https://github.com/postfiatorg/postfiatl1v2/commit/09cb4e0057803fbb01c2ed924386b025b0d53cae)
  audited three new ML-DSA seed-keygen call sites as synthetic or drill-only and
  corrected two stale node test fixtures. The consensus fixture now reflects
  eager migration markers introduced by
  [`ff2b3532`](https://github.com/postfiatorg/postfiatl1v2/commit/ff2b3532f5f6e12c9a5ad4e4424e4bcaa4ef7e05);
  the replicated-state fixture now reuses the operation-scoped transactional
  handle introduced by
  [`f0013c29`](https://github.com/postfiatorg/postfiatl1v2/commit/f0013c294335340e1c6d9aaecd9039521480aa13).
- The final full-suite verdict remained pending in
  [product-security-ci run 34125688652](https://github.com/postfiatorg/postfiatl1v2/actions/runs/34125688652)
  and
  [rust-ci run 34125688620](https://github.com/postfiatorg/postfiatl1v2/actions/runs/34125688620).
- One safety inventory remains deliberately unpinned. A complete comparison
  against the
  [proof-input inventory](../status/OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json)
  finds eleven current mismatches: ten reserve-proof source files from the
  weekend merges plus `scripts/check-nav-reserve-proof-fuzz-smoke`, changed in
  `343a2fe9`. The exact policy command therefore remains red and the source
  hashes require per-file review before repinning; this is not a green-CI
  claim.

### Task Node UNL MVP

- The two 2026-09-06 review P2s were fixed fail-closed with regressions in
  [`1267df6a`](https://github.com/postfiatorg/postfiatl1v2/commit/1267df6a8cfd563136a8256ee06da563ec01372c);
  admission-path fixture coverage was restored in
  [`7f266371`](https://github.com/postfiatorg/postfiatl1v2/commit/7f2663717a07f515ff9478910b2416cdaaf100cb).
- [`5261a8b8`](https://github.com/postfiatorg/postfiatl1v2/commit/5261a8b89c8a31ecdbfff465d2dd64ab815df5c2)
  added the reproducible shadow-diff command and dated round-21 baseline. Its
  first committed
  [diff](../governance/tasknode-unl-shadow-run-20260904/round-21-shadow-diff-20260907.json)
  reports 0 added, 0 removed, 20 retained, and 3 held. This is historical,
  read-only, `SHADOW_ONLY` evidence and grants no live authority.
- The focused Task Node UNL suite passes 107 tests and 36 subtests locally. The
  [MVP plan](../plans/completed/tasknode-unl-mvp-plan.md) moved to completed in
  [`d46f1cd8`](https://github.com/postfiatorg/postfiatl1v2/commit/d46f1cd8344a25bb7b42707350528dfa0787987e).

### Attack simulation

The deterministic, synthetic, local, shadow-only
[attack simulation](../governance/tasknode-unl-attack-simulation-20260907.md)
and
[benchmark packet](https://github.com/postfiatorg/postfiatl1v2/tree/363084b8ea3b7ffbbb7f50647919b47f69b99e5f/benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907)
landed in
[`363084b8`](https://github.com/postfiatorg/postfiatl1v2/commit/363084b8ea3b7ffbbb7f50647919b47f69b99e5f)
with a recorded harness average of 91.27. Byte-identical reruns found no seat
within the swept budgets for vouch rings, identity farming, or Foundation
support, but one bought aged account gained a seat and one dust transfer could
deny an otherwise eligible rival through the first-funder edge. The
connectivity floor, walk count, and damping are load-bearing: changing the
floor to `1/N`, using 10 steps, or using damping 0.90 eliminated all honest
admissions in the tested population. These findings support proposal review,
not a registry change.

### PR #38 deploy decision

The
[deploy decision sheet](../governance/yolo-deploy-decision-20260907.md)
landed in
[`566accbf`](https://github.com/postfiatorg/postfiatl1v2/commit/566accbf894286a86a7bb9b0407e324e8f2d8db6).
It leans A: carry the merged, default-disabled code in the next routine
validator release and leave activation to a separate later decision. The
supplied 88.73 score is not retained in the repository, so it is not presented
here as repository evidence.

### StakeHub PR #8

The read-only
[review](../review/stakehub-pr8-review-20260907.md) landed in
[`336a271f`](https://github.com/postfiatorg/postfiatl1v2/commit/336a271f844e7529251d20dcb53b578251420685)
with recommendation **merge after fixes**. Its P1 blockers are policy denials
falling through to unrestricted direct signing, operational scripts performing
live actions merely on invocation, and failure paths that can strand wallet
keys or private notes on validators. P2 findings include a committed recovery
archive that contradicts the consolidation handoff's “not in Git” claim
(secret scans were clean), a hardcoded live public Ethereum RPC, an untested
irreversible Hyperliquid master-key operation, and an archive manifest that
depends on sixteen absent files. P3 records withdrawal amounts crossing binary
floats before signing.

### Authority and environment boundaries

- Devnet was not touched. No live probe, deployment, on-chain write, signing,
  transaction submission, or value movement occurred. Network access was
  read-only: GitHub CI APIs and one unauthenticated scoring-service round fetch
  retained for the shadow baseline.
- The repository branch was `main`; source through `d46f1cd8` was present
  before this documentation-only handoff. Merged source is not evidence of
  deployment. [Current State](../status/chain-state-current.md) remains the
  authority for the last observed fleet state and deployed lineage.
- No Task Node action occurred.

## Next decision or action

1. Read the
   [attack simulation](../governance/tasknode-unl-attack-simulation-20260907.md)
   before finalizing Task Node emission design. A transfer-of-control rule and
   a minimum-value floor on funding edges are candidate proposal amendments,
   not implemented decisions; the proposal remains authoritative.
2. Check the final verdict on `09cb4e00`, but do not treat the merge queue as
   unblocked while the exact proof-input inventory command is red. Review and
   repin the eleven changed sources individually.
3. Resolve or explicitly disposition the three P1 blockers in
   [StakeHub PR #8's review](../review/stakehub-pr8-review-20260907.md) before
   merge.
4. Adopt or reject the PR #38 decision sheet with one line.
5. The
   [direction decision](../governance/ai-governance-direction-20260903.md) and
   [Z3 G1 envelope](../plans/active/z3-navcoin-roundtrip-plan.md) remain open
   from 2026-09-03.

## References

- [Four CI repair commits](https://github.com/postfiatorg/postfiatl1v2/compare/4a961eeb1bcfd64dd6e186c55307476e8e0058c3^...09cb4e0057803fbb01c2ed924386b025b0d53cae)
- [Task Node UNL completed plan](../plans/completed/tasknode-unl-mvp-plan.md)
- [Task Node UNL attack simulation](../governance/tasknode-unl-attack-simulation-20260907.md)
- [YOLO target-receipt deploy decision](../governance/yolo-deploy-decision-20260907.md)
- [StakeHub PR #8 review](../review/stakehub-pr8-review-20260907.md)
- [Other 2026-09-07 operator-lane consolidation handoff](https://github.com/postfiatorg/postfiatl1v2/blob/integrate/arc-tier4-current-v2-20260901/docs/handoffs/2026-09-07___dravlic__pr38_merged_arc_deck_unl_proposal.md)
- [2026-09-03 direction and Z3 handoff](2026-09-03___dravlic__agent_direction_decided_z3_planned.md)
- [Pending operator decisions](../governance/pending-operator-decisions.md)
- [Current State](../status/chain-state-current.md)
