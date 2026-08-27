# Storage candidate review

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-08-27 UTC

## BLUF

This was an independent read-only review of transactional-storage candidate
`0fdcc2b3` and the
[active milestone](../plans/active/storage-scaling-milestone.md) against the
[locked research specification](../architecture/storage-scaling-research-spec.md)
and [fix specification](../architecture/storage-scaling-fix-spec.md), using
checkout `aa449bdd`. The source-level claims hold, but the release-evidence
claims do not yet have repository-verifiable artifacts, the qualification
harness departs from the research specification's same-binary rule, and
`--verify-only` is not read-only. The review itself changed only
[the plans index](../plans/README.md) in `97afa8c6`. The other operator's
`/goal` was running the paired release campaign on this host during the review;
none of that active work was edited and no process was interrupted. An
end-of-session section below records the governance work done later the same
day.

## Current state

### Evidence audit

- At `aa449bdd`,
  [benchmarks/storage-scaling/SHA256SUMS.txt](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/SHA256SUMS.txt)
  binds only `README.md`, `e2-bounded-work.json`, and
  `verify_development_evidence.py`. The checked height-915 replay, 69-case
  tamper run, two-binary rollback, and six-clone rehearsal are cited by digest
  in the [milestone](../plans/active/storage-scaling-milestone.md), but their
  reports and receipts are not committed. They are operator-reported, not
  repository-verifiable.
- The E1 five-height measurements are the development-only synthetic
  [`e2-bounded-work.json`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/e2-bounded-work.json).
  The checked E3 claim "bounded through 5,000" rests on the retired fixed-bitmap
  prototype
  ([`benchmarks/storage-scaling/README.md:8-11`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/README.md#L8-L11)),
  and the six-clone rehearsal used dirty source and a height-501 fixture
  ([`docs/plans/active/storage-scaling-milestone.md:197-214`](../plans/active/storage-scaling-milestone.md)).
- Committed-test support covers v2 JSONL heads, crash-suffix recovery, per-log
  locks, deterministic indexes and a fixed-size accumulator, the
  activation-height genesis field, all four proposal kinds, replay across the
  activation boundary, the offline rebuild command, and the single-redb-
  transaction boundary
  ([`crates/node/src/tests/replicated_state_activation.rs:615-723`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/tests/replicated_state_activation.rs#L615-L723)).

### Gate comparison

The 110% height-5,000/height-50 ratio, five heights, five windows, six
validators, literal receipts, packet verifier, unchanged Cobalt/Consensus v2
authority, and no-live-mutation boundary are preserved. The following research
gates changed or lack a matching milestone gate:

1. One binary with only storage mode changed and the same authenticated
   snapshot
   ([`research-spec.md:214-219`](../architecture/storage-scaling-research-spec.md))
   became three owning release binaries with lane-native snapshots
   ([`benchmarks/storage-scaling/README.md:18-37`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/README.md#L18-L37)).
   E1.3/E3 therefore needs the decision owner's explicit approval.
2. A rebuilt index byte-identical to a clean index build
   ([`research-spec.md:208-210`](../architecture/storage-scaling-research-spec.md))
   became canonical logical equality
   ([`fix-spec.md:177-189`](../architecture/storage-scaling-fix-spec.md)).
3. The checksum freeze before candidate work begins
   ([`research-spec.md:168-169`](../architecture/storage-scaling-research-spec.md))
   has no receipt.
4. Exact 915/924 replay through each candidate, including the enumerated
   artifacts, was narrowed to the selected store
   ([`fix-spec.md:364-365`](../architecture/storage-scaling-fix-spec.md)).
5. The tamper corpus is no longer bound to replayed stores with a restart at
   every checkpoint
   ([`research-spec.md:228-229`](../architecture/storage-scaling-research-spec.md)
   versus
   [`milestone.md:101-110`](../plans/active/storage-scaling-milestone.md)).
6. "Consensus v2 never stops or forks"
   ([`research-spec.md:244-245`](../architecture/storage-scaling-research-spec.md))
   is absent from [`fix-spec.md:303-306`](../architecture/storage-scaling-fix-spec.md).
7. The selected height-50 p95 at no more than 110% of the frozen legacy
   height-50 baseline
   ([`fix-spec.md:293-294`](../architecture/storage-scaling-fix-spec.md)) has no
   milestone checkbox.

### Code spot checks

- After activation, the finality path returns before any legacy block,
  receipt, archive, or ordered JSONL append; its only file write is the
  `validator_registry.json` compatibility mirror
  ([`crates/node/src/storage_commit.rs:2441-2465`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_commit.rs#L2441-L2465)).
- One redb write transaction covers state, receipts, archive, ordered indexes,
  blocks, and tip metadata and commits once
  ([`crates/storage/src/transactional.rs:1154-1379`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/transactional.rs#L1154-L1379)).
- The ordered-history accumulator is fixed-size
  ([`crates/storage/src/ordered_history.rs:49-94`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/ordered_history.rs#L49-L94)).
- No `unwrap`, `expect`, or `panic` reachable from a transient I/O error was
  found in production code in
  [`storage_commit.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_commit.rs)
  or
  [`transactional.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/transactional.rs).
  Adjacent caveat: `NodeStore::new` panics if its integrity key cannot be loaded
  ([`crates/storage/src/lib.rs:112-117`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/lib.rs#L112-L117)).
- `--verify-only` is not read-only. Migration performs ordered-commit journal
  recovery before testing `verify_only`
  ([`crates/node/src/storage_migration.rs:69-76`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_migration.rs#L69-L76),
  [`:150-159`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_migration.rs#L150-L159));
  recovery can apply a pending commit and remove the journal
  ([`storage_commit.rs:2625-2691`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_commit.rs#L2625-L2691));
  and target opening uses `create_dir_all`, `load_or_create`, and
  `Database::create`
  ([`transactional.rs:492-507`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/transactional.rs#L492-L507)).
  The no-change claim in
  [`benchmarks/storage-scaling/README.md:113-116`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/README.md#L113-L116)
  needs a read-only constructor and a no-recovery path before it is true.

### Exact height-924 input

The missing replay input is one complete, quiescent legacy `NodeStore` data
directory at exact height 924 for chain `postfiat-wan-devnet-2`, with the
identities recorded in
[`docs/status/chain-state-current.md:62-69`](../status/chain-state-current.md).
[`run_replay_evidence.py`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/run_replay_evidence.py)
consumes it through `--authenticated-history-dir`
([`:366-380`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/run_replay_evidence.py#L366-L380));
the runner hashes the source tree and works on a scratch copy
([`:400-439`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/run_replay_evidence.py#L400-L439)).
One read-only copy from any converged validator host suffices for that replay
gate. The six-clone gate separately requires six distinct stopped copies for
validator-0 through validator-5 supplied as `--source-data-dir`
([`run_migration_rehearsal.py:1180-1190`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/run_migration_rehearsal.py#L1180-L1190),
[`402-430`](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/benchmarks/storage-scaling/run_migration_rehearsal.py#L402-L430)).
Fleet copying requires separate authorization.

### Documentation and operational boundary

- The [plans index](../plans/README.md) was corrected in `97afa8c6`.
  [Current State](../status/chain-state-current.md) still names `20c95ec2` as
  the transactional lineage at lines 26 and 46, while the
  [milestone](../plans/active/storage-scaling-milestone.md) names `0fdcc2b3`.
  The [previous handoff](2026-08-27___postfiatchad__transactional_storage_candidate.md)
  is now historical: its open-gate list predates later qualification claims and
  has no forward link. Those files were left untouched because the other
  operator's goal was editing them.
- This review made no devnet probe, deployment, validator write, or service
  change. The last observed fleet state remains the point-in-time six-validator
  convergence at height 924 during `2026-08-26T06:34:55Z`–`06:35:50Z`, and the
  deployed node source remains `8cc7d15e`; repository descendants are
  undeployed ([Current State](../status/chain-state-current.md)).
- No Task Node action was taken because Task Node was OFF under the
  2026-08-26/27 operator instruction; the research-specification locks remain
  pending.
- The
  [Dynamic UNL proposal-source research specification](../governance/dynamic-unl-proposal-source-research-spec.md),
  scored 89.13, is this operator's proposed next milestone candidate. Its
  deferred milestone draft is planned, not authorized work.

## Next decision or action

1. Approve or reject the same-binary deviation. Without an explicit amendment,
   the three-binary, lane-native-snapshot campaign does not satisfy E1.3/E3.
2. Commit the redaction-safe height-915 replay, tamper, rollback, and six-clone
   reports and receipts and bind them in `SHA256SUMS.txt`; otherwise uncheck the
   milestone boxes and label the claims operator-reported.
3. Identify the validator host holding a complete, quiescent height-924 legacy
   directory that may be copied read-only for `--authenticated-history-dir`,
   and decide whether six stopped validator-0..5 copies exist for the
   six-clone gate.
4. Make `--verify-only` read-only by skipping journal recovery and opening redb
   read-only; then correct the evidence README, add the missing fix-spec 110%
   legacy-baseline checkbox, and update Current State's lineage to `0fdcc2b3`.
5. Confirm whether Dynamic UNL proposal source is the milestone after storage.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage-scaling research specification](../architecture/storage-scaling-research-spec.md)
- [Storage-scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Transactional storage candidate handoff](2026-08-27___postfiatchad__transactional_storage_candidate.md)
- [Current State](../status/chain-state-current.md)
- [Storage-scaling evidence directory](https://github.com/postfiatorg/postfiatl1v2/tree/aa449bdd/benchmarks/storage-scaling)
- [Transactional store](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/transactional.rs),
  [atomic commit](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_commit.rs),
  [migration](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/node/src/storage_migration.rs), and
  [ordered history](https://github.com/postfiatorg/postfiatl1v2/blob/aa449bdd/crates/storage/src/ordered_history.rs)
- Review basis `aa449bdd`; candidate `0fdcc2b3`; plans-index correction
  `97afa8c6`

## End of session — validator evaluator for this L1

### Context

On this L1 the validator set is
[governed registry state](../governance/validator-registry.md), and
[Cobalt has ratified bounded registry and trust-graph changes since height 916](../governance/deterministic-governance-overview.md).
The [current chain state](../status/chain-state-current.md) records that every
proposal so far came from Foundation-administered validators, while the DGA
step in which “a policy proposes bounded validator-registry actions” is not
live. The second operator asked what should fill that evaluator/proposer slot
if decentralization and the Dynamic UNL philosophy are the goal.

### Delivered today

All items below were pushed to `main`. This operator used no Task Node or Cargo
and made no chain change for this work.

- [Validator Evaluator Alternatives Decision Note](../governance/validator-evaluator-alternatives-note.md)
  (`5e07f2c9`; harness 89.27: GPT 90.60, Fable 88.40, GLM 88.80;
  one scoring loop) compares six candidates against eight criteria derived
  from the Dynamic UNL philosophy: Dynamic UNL as content source; L1-native
  DGA policy; a deterministic formula baseline; direct validator voting;
  bond-gated admission plus formula; and the status quo. It recommends that
  Dynamic UNL supply the ranked target and proposal content, the L1 DGA policy
  supply the constitutional envelope and hard bounds, the independent-operator
  path supply the submitter, Cobalt ratify, and Consensus v2 order. The
  deterministic formula remains the published baseline and a separately
  activated fail-closed fallback.
- [Dynamic UNL L1 Evidence-Source Decision Note](../governance/dynamic-unl-l1-evidence-source-note.md)
  (`054c6c4c`; harness 89.80: GPT 92.60, Fable 89.20, GLM 87.60;
  one scoring loop) asks whose evidence scores L1 validators. Option A reuses
  the sealed PFT Ledger result plus a governed identity binding; Option B runs
  the pipeline on L1-native evidence and needs an L1 observer, anchor profile,
  scoring replay, and sidecar convergence; Option C shadows A while building
  B. It recommends Option C: use PFT Ledger results only for a governed-binding
  `SHADOW_ONLY` adapter-to-Cobalt integration on the controlled devnet while
  the L1-native path is built and qualified. Live authority still requires the
  research specification's gates and separate authorization.
- [Dynamic UNL Proposal Source Milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md)
  (`10abc3d7`) is the research specification's E1–E5 milestone draft, with 57
  unchecked boxes, gates, and human interfaces. It is listed in the
  [deferred-plan index](../deferred-plans/README.md) and is not authorized work.
- Earlier today, the [plans index](../plans/README.md) was corrected in
  `97afa8c6`, and this storage review was published. The other operator's
  [follow-up handoff](2026-08-27___postfiatchad__storage_verifier_and_campaign_reset.md)
  already acted on the review: `--verify-only` became read-only in `785806bd`,
  the interrupted campaign was declared evidence-ineligible, and a
  single-binary boundary was planned.

### Decisions for the operator

1. Record Dynamic UNL as the canonical proposal-content source inside a
   Cobalt-governed L1 DGA policy envelope, with the formula as a fail-closed
   fallback, using the alternatives note's “Exact decision to record”; or
   choose a different candidate.
2. Record Option C as the evidence-source sequence, using the exact wording in
   the evidence-source note; or choose Option A or B.
3. Answer the questions that change the plan: do the same operators run
   validators on both chains, and what share of each active set maps
   one-to-one; is an L1 observer service planned, and who owns it; is the
   bounded `payment_v2` memo lane acceptable for announcement and
   commit/reveal anchors, or does the L1 need a dedicated data-carrying
   transaction; and will enough independent operators bear pinned inference
   cost?
4. Confirm whether Dynamic UNL proposal source becomes the milestone after
   storage and whether the Task Node lock of the research specification should
   be requested. Task Node remains off under the operator's instruction.

### Boundaries

The devnet was not touched: no probe, deployment, or validator write occurred.
The last observed fleet state remains unchanged from the storage section above.
Nothing here grants authority or changes either chain.

### References

- [Validator Evaluator Alternatives Decision Note](../governance/validator-evaluator-alternatives-note.md)
- [Dynamic UNL L1 Evidence-Source Decision Note](../governance/dynamic-unl-l1-evidence-source-note.md)
