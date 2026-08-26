# Evidence review and next specifications

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-08-26 UTC

## BLUF

Completed a read-only review of the overnight Cobalt adversarial campaign,
pushed four documentation and process corrections, and proposed two
harness-scored research specifications as the next milestone candidates:
[storage scaling](../architecture/storage-scaling-research-spec.md) and
[Dynamic UNL proposal sourcing](../governance/dynamic-unl-proposal-source-research-spec.md).
No experiment was started, no Task Node action was taken, and the controlled
devnet was not probed or touched.

## Current state

### Evidence review

The consolidated campaign packet passed at the campaign-completion revision with
`adversarial-packet-ok` and checksum-manifest root
`a789372819c173d3c290f84b7ad10bea3ddef01ffc5a012e837ba3dc32d36368`.
The packet's own `SHA256SUMS.txt` still verifies. A semantic verifier run
against current `main` now stops at its publication-document binding because
the corrections below changed packet-bound documentation and `mkdocs.yml`;
the experiment packets and consolidated packet were not modified. See the
[completion handoff](2026-08-26___postfiatchad__cobalt_adversarial_verification.md)
for the original passing run and the
[packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-adversarial-verification/packet)
for the immutable campaign evidence.

The review found:

1. **E1 fixture defect, not an oracle or production defect.** The 8,534 initial
   disagreements came from the harness adapter supplying 64-character genesis
   and registry-root fixtures where production requires 96 lowercase hex
   characters. Commit
   [`43606cd9`](https://github.com/postfiatorg/postfiatl1v2/commit/43606cd9f7dbd467b9c04a9bf091aa48d88666f7)
   corrected the fixtures; the unchanged 10,240-case corpus then passed. The
   [mismatch review](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-adversarial-verification/e1/initial/mismatch-review.md)
   retains the initial disagreement evidence.
2. **E4 measured chain-height growth, not a different setup.** E4's first 50
   baseline rounds reproduce the activation result, about 1,664 ms versus 1,660
   ms p95. Latency then grows nearly linearly to about 14.9 seconds by round 500
   (correlation approximately 0.9998). The cause is the JSON/JSONL path
   rescanning the authenticated chain on every append in
   [`read_jsonl_tail`](https://github.com/postfiatorg/postfiatl1v2/blob/e1567605fbdf68b778913e8ca6db815d68bd697a/crates/storage/src/lib.rs#L876)
   and proposal/state construction rebuilding full ordered history. The 5% gate
   remains valid as a paired, same-length A/B comparison; the absolute latency
   is not a finality SLA and storage scaling blocks any public testnet.
3. **The independent-operator specification lacked its stated lock evidence.**
   It had been marked `Locked` without a Text Improvement Harness score or Task
   Node lock record. It now records a passing 88.73 score and says the Task Node
   lock is pending the operator's decision.
4. **The first E5 return was not fully rehearsed.** The height-921 return bound a
   stale trust root because the rollback-then-return sequence and its
   protocol-native post-return trust-root binding had not been clone-rehearsed.
   The signed corrective rollback and return committed at heights 922 and 923.
   The [E5 packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-adversarial-verification/e5)
   preserves both the first attempt and the correction.

### Corrections pushed on `main`

Four corrections landed in three commits:

- [`31092413`](https://github.com/postfiatorg/postfiatl1v2/commit/310924134d8777cd9264043c8c2e52d7486f7998)
  names the 50-round `consensus_round_ms` and 500-round
  `wallet_to_finality_ms` metrics separately and marks them non-comparable in
  both completed milestones. It also adds the standing clone-first rehearsal
  rule for every live authority transition, including return-to-Cobalt trust
  binding, to [Cobalt governance](../governance/cobalt.md).
- [`5448da87`](https://github.com/postfiatorg/postfiatl1v2/commit/5448da87756c13f9cd82c56f31f9a753825d99f3)
  records the independent-operator specification's 88.73 harness score and
  leaves its Task Node lock pending.
- [`3d1080b4`](https://github.com/postfiatorg/postfiatl1v2/commit/3d1080b4fc8757cc95152614d0ff084671c7895b)
  records the E4 chain-height finding in the
  [adversarial results](../governance/cobalt-adversarial-verification-results.md)
  and [Current State](../status/chain-state-current.md), and defers storage
  scaling in [the deferred-plan index](../deferred-plans/README.md). The E4
  packet was untouched and its checksums pass.

### Proposed next specifications

Neither specification is Task Node-locked:

- [Storage Scaling and Bounded Finality](../architecture/storage-scaling-research-spec.md)
  passed the harness at 88.67 in
  [`2613ca5f`](https://github.com/postfiatorg/postfiatl1v2/commit/2613ca5f41f1c56301bb3ad6a54c2bc1da705aa0).
  It requires bounded per-height storage and proposal cost, clone-rehearsed
  migration, p95 finality at height 5,000 within 10% of height 50, and
  byte-identical archive replay.
- [Dynamic UNL Proposal Source](../governance/dynamic-unl-proposal-source-research-spec.md)
  passed at 89.13 in
  [`e1567605`](https://github.com/postfiatorg/postfiatl1v2/commit/e1567605fbdf68b778913e8ca6db815d68bd697a).
  It evaluates the PFT Ledger Dynamic UNL pipeline's sealed, converged result as
  deterministic proposal content for Cobalt-ratified registry changes. It
  complements the independent-operator specification, which governs who may
  submit, and positions Dynamic UNL as the concrete validator evaluator needed
  by the DGA constitutional policy layer.

### Task Node, references, and operational boundary

- Per the operator's instruction, Task Node was not used for E2-E6 or today's
  review and specification work. The repository still records
  `task_91aebe5c632d90e03e7e151a6ffeb736` as accepted before E2 and unused for
  execution or completion. The local Task Node session has expired, so no
  server-side status was checked and no Task Node action was taken today.
- Read-only reference clones were added at
  `~/repos/dynamic-unl-scoring` (`f18e6b4c`) and
  `~/repos/validator-scoring-sidecar` (`e6907faa`); both remain clean.
- The pre-handoff repository state was `main` at `e1567605`, matching
  `origin/main`. The commit containing this handoff is its documentation-only
  descendant.
- No live fleet probe, deployment, service restart, validator write, or
  experiment occurred in this session. The last observed fleet state and
  deployed binary/source lineage remain the dated evidence in
  [Current State](../status/chain-state-current.md); merged documentation and
  research specifications are not deployment evidence.

## Next decision or action

The decision owner should:

1. Choose the next milestone: storage scaling, which blocks a public testnet;
   Dynamic UNL proposal sourcing, which addresses the proposal-source half of
   E6's “who proposes” gap; or both in a stated sequence.
2. Confirm whether the AGENTS.md Task Node rule still applies. If it does, lock
   the chosen specification through Task Node and then request its concise
   milestone document through Task Node before implementation.
3. Confirm which chain receives the second operator's hours. On the PFT Ledger
   Dynamic UNL roadmap, G.6 sidecar governance verification and E.1 signed-vote
   publication remain open.

## References

- [Storage Scaling and Bounded Finality Research Specification](../architecture/storage-scaling-research-spec.md)
- [Dynamic UNL Proposal Source Research Specification](../governance/dynamic-unl-proposal-source-research-spec.md)
- [Independent-Operator Proposal Path Research Specification](../governance/cobalt-independent-operator-proposal-path-research-spec.md)
- Correction commits:
  [`31092413`](https://github.com/postfiatorg/postfiatl1v2/commit/310924134d8777cd9264043c8c2e52d7486f7998),
  [`5448da87`](https://github.com/postfiatorg/postfiatl1v2/commit/5448da87756c13f9cd82c56f31f9a753825d99f3),
  and
  [`3d1080b4`](https://github.com/postfiatorg/postfiatl1v2/commit/3d1080b4fc8757cc95152614d0ff084671c7895b)
- [Current State](../status/chain-state-current.md)
- [Consolidated adversarial packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-adversarial-verification/packet)
- [Cobalt adversarial-verification completion handoff](2026-08-26___postfiatchad__cobalt_adversarial_verification.md)
- [Dynamic UNL roadmap](https://github.com/postfiatorg/dynamic-unl-scoring/blob/f18e6b4c40d3df17afb46804ef4f2bb0d879b439/docs/CurrentRoadmap.md)
