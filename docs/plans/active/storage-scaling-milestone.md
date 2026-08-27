# Storage Scaling: Time-Budgeted Qualification and Release Gates

**Status:** Active — `redb` selected; qualification incomplete; deployment and public testnet blocked

**Decision date:** 2026-08-27

**Decision owner:** Post Fiat

**Candidate lineage:** transactional `redb` implementation through `f3907ad5`; G1 must pin the final clean source and binary

**Research basis:** [Storage Scaling and Bounded Finality](../../architecture/storage-scaling-research-spec.md)

**Implementation contract:** [Storage Scaling Fix](../../architecture/storage-scaling-fix-spec.md)

**Independent review:** [Storage candidate review](../../handoffs/2026-08-27___dravlic__storage_candidate_review.md)

## Plain-English decision

Post Fiat selects the transactional `redb` store as the storage candidate. The
source design is credible enough to stop comparing alternative storage designs:
one finalized height commits atomically, post-activation proposal and commit
paths use an indexed membership set plus a fixed-size accumulator, and the
selected path does not intentionally rescan full history.

Selection is not qualification, qualification is not deployment, and deployment
is not public-testnet authorization. The remaining work must prove the selected
candidate's safety, exact replay, bounded scaling, and migration behavior.

The exhaustive three-lane, five-height, five-window campaign is no longer the
current release gate. It was a research campaign for choosing among candidates,
but the candidate is now selected and the legacy lane becomes prohibitively slow
by design. The current release gate measures only what can change the decision:

1. `redb` at heights 50 and 5,000;
2. legacy JSON/JSONL at height 50 only, as the low-height regression baseline;
3. exact replay, tamper, crash, rollback, and read-only verification; and
4. the six-clone rehearsal only when deployment is actually being considered.

This is an operator-directed amendment to the execution breadth and order in the
research and implementation specifications. It does **not** weaken deterministic
replay, failure atomicity, tamper rejection, receipt acceptance, six-validator
convergence, Consensus v2 safety, Cobalt authority boundaries, or the requirement
for separate deployment authorization.

## Decision space

The current job is to qualify the selected storage implementation. It is **not**
another storage-design search, a devnet deployment, a public-testnet launch, or a
Dynamic UNL implementation. The only remaining operator decisions are the later
authorization boundaries shown below; missing authorization must not leave local
work idling.

| Question | Recorded answer | State | Effect on current work |
| --- | --- | --- | --- |
| Which storage implementation? | Transactional `redb`. | Decided | Freeze and qualify this candidate; do not restart candidate research. |
| Which performance campaign? | Selected `redb` at heights 50 and 5,000; same-binary legacy control at height 50. | Decided | Do not run the superseded three-lane/five-height matrix. |
| Is one height-924 validator directory needed? | Yes, for exact replay only. | Authorization and custodian still required | Finish G1, G2, the height-915 part of G3, and G4 without waiting for it. |
| Are six validator directories needed now? | No. They are needed only for G6 immediately before a deployment decision. | Deferred | Spend no time collecting or rehearsing six clones during offline qualification. |
| May this plan touch the controlled devnet? | No. | Not authorized | No fleet query, copy, service action, deployment, or mutation. |
| What follows storage? | Dynamic UNL supplies proposal content inside the DGA/Cobalt envelope; Option C is the evidence sequence. | Direction recorded; implementation deferred | Keep its milestone deferred until the storage boundary in G7. |
| Does a passing packet deploy anything? | No. | Separate later decision required | G5 can establish only `OFFLINE QUALIFIED`; G6 and written deployment authorization remain separate. |

## Decisions recorded

- **Storage candidate:** transactional `redb`.
- **Performance proof:** selected-path height 50 versus height 5,000, plus a
  same-binary legacy-versus-selected comparison at height 50.
- **Discarded release work:** bounded-JSONL performance qualification and
  legacy runs at heights 100, 500, 1,000, and 5,000. They remain optional
  diagnostics and cannot hold the release decision open.
- **Real-chain replay input:** one complete, quiescent, read-only height-924
  validator data directory is enough for exact replay.
- **Deployment rehearsal input:** six distinct stopped validator directories
  are required only for the final six-clone migration gate.
- **Governance direction after storage:** Dynamic UNL is the intended canonical
  proposal-content source inside hard L1 DGA limits; independent admitted
  operators submit unchanged proposal bytes, Cobalt ratifies validator-trust
  changes, and Consensus v2 orders them. The deterministic formula stays a
  published shadow baseline and a separately activated fail-closed fallback.
- **Dynamic UNL evidence sequence:** Option C. PFT Ledger results may exercise a
  governed-binding adapter in `SHADOW_ONLY` mode while an L1-native observer,
  evidence profile, scoring replay, and sidecar-convergence path are built.
  Nothing receives registry-mutation authority without a later recorded
  decision and the complete governance gates.
- **Operations boundary:** no Task Node, fleet probe, data copy, service change,
  deployment, or live mutation is authorized by this plan.

## Status vocabulary

| State | Meaning |
| --- | --- |
| **SELECTED** | Source review and existing tests justify concentrating qualification on `redb`. This is the current state. |
| **OFFLINE QUALIFIED** | G0 through G5 pass from pinned clean source with a verifier-bound packet. |
| **CLONE QUALIFIED** | G6 passes on six distinct, stopped, fleet-derived clones. |
| **AUTHORIZED FOR CONTROLLED DEVNET** | A separate operator decision pins source, binary, data, packet, activation, and rollback identities. |
| **DEPLOYED** | A later fleet receipt proves what actually runs. |
| **PUBLIC TESTNET ELIGIBLE** | All release, operational, security, and launch gates—not only this storage plan—pass. |

No document or interface may collapse these states into “fixed” or “live.”

## Time controls

Every execution command must declare its expected duration and hard timeout
before it starts.

- Any command expected to exceed 30 minutes must support checkpoint/resume or be
  split into independently verifiable units.
- No single unattended command may run longer than 2 hours without a new,
  evidence-backed operator decision.
- G4 has a 4-hour aggregate wall-clock budget. Reaching the budget is a recorded
  `TIME_BUDGET_EXCEEDED` result, not permission to continue silently.
- A failed or timed-out gate is diagnosed once. It is not automatically restarted
  with a larger matrix.
- Run selected-path evidence before expensive legacy controls.
- Preserve a stopped run only when its completed units are independently
  verifiable. Partial output never becomes a fabricated final report.

## Gated to-do list

A gate passes only when every checkbox in its detailed section is closed and
the independent verifier accepts the bound artifacts. A failed gate stops work
that depends on it. An unavailable external input is recorded and skipped while
independent local gates continue.

| Gate | Current state | Work allowed now | Budget and advance rule |
| --- | --- | --- | --- |
| G0 — campaign control | **PASS** | None; do not rerun the old campaign. | Reopen only if checkpoint/resume itself changes. |
| G1 — candidate freeze | **IN PROGRESS** | Pin clean source, build one binary, freeze inputs, and run focused regressions. | No command over 2 hours; any source or binary change restarts G1. |
| G2 — safety | **QUEUED AFTER G1** | Tamper/crash, canonical equality, rollback, and read-only sentinels. | Split into independently verifiable units; stop on mutation, ambiguity, panic, or unverifiable output. |
| G3 — exact replay | **PARTLY LOCAL / EXTERNAL INPUT OPEN** | Regenerate the height-915 evidence after G1. Run height 924 only from a separately authorized copy. | Never wait idle for the copy; record `WAITING_FOR_AUTHORIZED_INPUT` and continue to G4. |
| G4 — scaling | **QUEUED AFTER G1** | Run the three-row selected-first matrix only. | Four hours aggregate; stop on a selected-path failure or `TIME_BUDGET_EXCEEDED`. |
| G5 — offline packet | **BLOCKED BY G1–G4** | Package only after every preceding evidence gate passes. | No qualification claim until the offline verifier passes the complete packet. |
| G6 — six-clone rehearsal | **DEFERRED** | Nothing until offline qualification is complete and deployment is the next real decision. | Requires separate data-copy authorization and six distinct stopped directories. |
| G7 — Dynamic UNL handoff | **DIRECTION RECORDED / IMPLEMENTATION DEFERRED** | Preserve the recorded architecture decision only. | Spend no implementation time before the stated storage boundary or a new operator priority decision. |

## G0 — stop the open-ended campaign and make runs resumable

**Purpose:** stop spending time on evidence that no longer changes the candidate
decision.

- [x] Interrupt the current exhaustive paired campaign through its normal signal
      path and verify that every child validator and benchmark process exits.
- [x] Record source, binary, completed units, stop reason, elapsed time, and
      remaining units in a redaction-safe stop receipt.
- [x] Mark the interrupted campaign `evidence_eligible: false` as a whole.
- [x] Retain a completed window only if the verifier can bind its source, binary,
      snapshot, signed corpus, raw iterations, literal receipts, counters, and
      final roots without inventing a missing campaign summary. No old window
      is currently admitted as release evidence.
- [x] Add checkpoint/resume plus an explicit frozen lane, height, and window task
      plan to the runner.
- [x] Make resume refuse a changed source, binary, runner, topology, validator
      identity, host allocation, snapshot, corpus, timeout, completed artifact,
      or output schema.
- [x] Pass an interrupt/resume smoke test and prove there are no orphaned child
      processes.

Evidence: the checksum-bound
[`campaign-stop-f3907ad5.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/campaign-stop-f3907ad5.json)
records 32 complete windows, one seven-round partial window, 1,607 measured
rounds, no final report, and zero surviving processes after the four-hour stop.
The development verifier checks that boundary. The final controlled
checkpoint/resume smoke stopped after the height-2 advance, resumed the same
bound output, completed selected and legacy windows, and emitted report SHA-256
`9494dd8d…004b0` plus checkpoint SHA-256 `cd9eca58…a3540`, with no surviving
campaign process. Five focused Python tests cover private atomic checkpoints,
non-resumable completion, recoverable partial-unit quarantine, the frozen
release matrix, and the two-height model.

**Exit:** G0 passed. The old campaign is stopped, no superseded output is
release evidence, and future long work advances through independently verified
checkpoints.

## G1 — freeze the selected release candidate

**Purpose:** bind every later claim to one reproducible candidate.

- [x] Select transactional `redb` and retire the fixed bitmap and
      bounded-JSONL performance lane from candidate selection.
- [x] Use one release binary whose authenticated node-local storage mode is the
      only comparison switch.
- [x] Make `storage-rebuild-transactional --verify-only` strictly read-only and
      fail closed when recovery or creation would be required.
- [ ] Start from a clean checkout and record the exact source revision.
- [ ] Build one release binary and record its SHA-256, Rust toolchain, locked
      dependency identity, build command, host, and storage device.
- [ ] Freeze topology, keys, accounts, transaction corpus, timeouts,
      instrumentation schema, and authenticated height-50 and height-5,000
      snapshot identities.
- [ ] Run the focused storage/node regression suites, formatting, and
      warnings-denied Clippy against that source.

**Exit:** one clean source, one binary, and one frozen input set control G2
through G5. Any source or binary change returns the plan to G1.

## G2 — safety and failure atomicity

**Purpose:** prove the faster path did not weaken storage integrity.

- [ ] Re-run the complete 69-case tamper/crash classification from the pinned
      source.
- [ ] Prove every rejected case has a stable reason and produces zero durable
      mutation.
- [ ] Prove each crash cut recovers either the complete prior height or exactly
      one complete new height.
- [ ] Prove clean build, snapshot restore, transactional rebuild, restart, and
      canonical export produce identical logical records, commitments, roots,
      and tips.
- [ ] Re-run the compatible two-binary rollback path from the same certified tip
      without deleting or reinterpreting finalized blocks.
- [ ] Re-run whole-directory mutation sentinels for every `--verify-only`
      success and refusal path.
- [ ] Commit redaction-safe reports and receipts, bind them in
      `benchmarks/storage-scaling/SHA256SUMS.txt`, and make the independent
      verifier reject a missing or altered artifact.

**Exit:** all safety checks pass and are repository-verifiable. Any ambiguity,
partial mutation, panic on untrusted storage, or unverifiable report is
`REMEDIATION_REQUIRED`.

## G3 — exact replay

**Purpose:** prove compatibility with real recorded history rather than only
synthetic fixtures.

- [ ] Publish or regenerate a verifier-bound report for the exact 915-block
      quarantine replay from the pinned source; an operator-reported digest
      without the report does not close this item.
- [ ] Name the authorized custodian of one complete, quiescent height-924
      validator data directory.
- [ ] Obtain separate authorization for the read-only copy; this plan does not
      grant fleet access.
- [ ] Hash the source tree before use, preserve it read-only, and perform all
      replay work on a scratch copy.
- [ ] Replay through exact height 924 and match the recorded chain, height, tip,
      state root, blocks, receipts, archive, ordered history, and pre-activation
      state commitments.
- [ ] Run independent `--verify-only` against the result and prove both source
      and target directory hashes remain unchanged.
- [ ] Bind the redaction-safe replay report and verifier output into the packet
      checksum manifest.

**Exit:** both the 915 archive and real height-924 history replay exactly. If no
authorized height-924 directory is available, record `WAITING_FOR_AUTHORIZED_INPUT`;
continue other local gates, but do not claim offline qualification or deployment
readiness.

## G4 — time-budgeted scaling qualification

**Purpose:** answer whether the selected store removes height-dependent work
without spending hours re-measuring a deliberately rejected legacy design.

**Required matrix:**

| Lane | Starting height | Windows × rounds | Why it exists |
| --- | ---: | ---: | --- |
| Selected `redb` | 50 | 5 × 50 | Low-height performance and regression anchor |
| Selected `redb` | 5,000 | 5 × 50 | Scaling decision |
| Legacy JSON/JSONL | 50 | 5 × 50 | Same-binary low-height regression baseline |

All rows use the G1 binary. At height 50, both lanes import the same authenticated
snapshot and consume the same signed corpus, topology, keys, accounts, host,
storage device, vote policy, timeout, and instrumentation. Build the height-5,000
snapshot through the selected bounded path; do not advance legacy storage to
height 5,000 merely to demonstrate the already-known defect.

Run order is selected height 50, selected height 5,000, then legacy height 50.
Stop immediately when a selected-path safety, convergence, literal-receipt, or
bounded-work gate fails.

- [ ] Complete the three required rows inside the 4-hour aggregate budget.
- [ ] Verify literal accepted receipts and six-validator agreement on height,
      block hash, and state root for every measured round.
- [ ] Prove proposer construction, every remote validator reconstruction, and
      every finalized apply report zero full-history records and bytes after
      activation.
- [ ] Prove indexed point work is `O(log n)` pages or better and the
      ordered-history accumulator update is constant work.
- [ ] Prove selected height-50 p95 `consensus_round_ms` and
      `wallet_to_finality_ms` are each no more than 110% of the corresponding
      legacy height-50 p95.
- [ ] Prove selected height-5,000 p95 for both metrics is each no more than 110%
      of selected height-50 p95.
- [ ] Publish raw iterations, p50/p95, variance, counters, CPU, RSS, disk,
      process I/O, host load, fsync, and network observations.
- [ ] Make the verifier independently recompute identities, distributions,
      ratios, counters, resource summaries, and all pass/fail decisions.

**Exit:** every item passes inside budget. A timeout, censored sample, missing
receipt, convergence failure, positive full-history counter, or failed ratio is
a real failed gate requiring focused remediation—not a reason to start the
exhaustive matrix.

## G5 — offline qualification packet

**Purpose:** make the result independently checkable and state exactly what it
does and does not authorize.

- [ ] Package G1 through G4 into one checksum-bound, redaction-safe packet.
- [ ] Make `python -m postfiat_rpc.storage_scaling verify PACKET` pass without
      network access and fail on every missing, changed, incomparable, stale, or
      inconsistent required artifact.
- [ ] Make the read-only browser consume only a successfully verified packet and
      expose no migration, activation, rollback, or mutation action.
- [ ] Run the proportional clean release suite and strict documentation,
      redaction, and public-link checks.
- [ ] Update Current State, State and Storage, the evidence index, and operator
      runbooks with the exact source/binary/packet identities and remaining
      boundaries.
- [ ] Record `OFFLINE QUALIFIED` only after every G0–G5 item passes.

**Exit:** the candidate may be described as offline qualified. Controlled-devnet
deployment and public testnet remain blocked.

## G6 — pre-deployment six-clone gate

**Purpose:** spend the six-node migration cost only when deployment is the next
real decision.

- [ ] Obtain separate authorization and six distinct, complete, stopped
      validator-0 through validator-5 data-directory copies.
- [ ] Bind each source and backup root, required disk, source/binary identity,
      activation record, cancellation boundary, and rollback binary.
- [ ] Rehearse side-by-side migration, independent verification, staged restart,
      pre-activation cancellation, activation, post-activation finality,
      compatible rollback, catch-up, and all-six convergence.
- [ ] Prove Cobalt authority and every Consensus v2 rule remain unchanged.
- [ ] Package and independently verify the clone receipts.
- [ ] Record `CLONE QUALIFIED` only after every G6 item passes.
- [ ] Require a separate written deployment decision. It must pin source,
      binary, packet, six source snapshots, activation height, rollback window,
      stop conditions, owners, and fleet evidence requirements.

**Exit:** passing G6 makes deployment eligible for a separate decision; it does
not deploy anything.

## G7 — next-milestone handoff

- [x] Record Dynamic UNL inside the DGA/Cobalt envelope as the intended
      proposal-content architecture, with the deterministic formula as a shadow
      baseline and separately activated fail-closed fallback.
- [x] Record Option C as the evidence-source sequence and keep all PFT-derived
      integration `SHADOW_ONLY`.
- [ ] Keep the [Dynamic UNL milestone](../../deferred-plans/dynamic-unl-proposal-source-milestone.md)
      deferred until G5 closes, or until the decision owner explicitly
      de-prioritizes storage after all locally executable gates finish.
- [ ] When activated, update that milestone before implementation to remove Task
      Node as a prerequisite, preserve the no-authority boundary, and name the
      L1-observer and independent-operator owners.
- [ ] Do not let governance work obscure an open storage deployment or
      public-testnet blocker.

## Immediate execution order

1. Do not rerun G0: the old campaign is stopped and resumability is proven.
2. Finish G1 and pin the clean candidate before generating qualification
   evidence.
3. Run G2 in independently verifiable units.
4. Regenerate the local height-915 evidence for G3. If no authorized height-924
   input exists, record that boundary and move directly to G4 instead of
   waiting.
5. Run G4 selected-first within its four-hour aggregate limit.
6. Close the height-924 part of G3 only after the copy is separately authorized
   and available.
7. Build and verify the G5 packet only when G1 through G4 are closed.
8. Run G6 only if controlled-devnet deployment is actually the next decision
   and its separate authorization has been recorded.
9. Activate the deferred Dynamic UNL milestone only at the G7 boundary or after
   an explicit operator reprioritization.

The plan is complete only when G0 through G6 pass and a separate decision either
authorizes deployment or explicitly records why the qualified candidate remains
undeployed. Until then, public testnet remains blocked.
