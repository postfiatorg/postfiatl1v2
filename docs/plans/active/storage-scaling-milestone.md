# Storage Scaling: Time-Budgeted Qualification and Release Gates

**Status:** Active — `redb` selected; certified-send remediation frozen; new G4 authorization, deployment, and public testnet blocked

**Decision date:** 2026-08-27

**Decision owner:** Post Fiat

**Candidate lineage:** failed pre-fix source `ae65844190f153cbdd49d1e5ac28ab96a19f7af4`, binary SHA-256 `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4`; corrected vote-lock source `442c5a4ddafed3aa0709f64e213fe0cedac5222d`, binary SHA-256 `29423cba098ce793ccab4a234ab26a2d30c6b11ad9eacd339b11b89cd6187c48`; certified-send-remediated source `e52e050269a2f9fdd28c5083c3888debf3a85063`, binary SHA-256 `6b130a1f9c81bd64bc9dc42043595f5a27e84185cf3f40b13b5f37a40d72a82e`; corrected runner `15d059d1`; persistent setup remediation `438fb29c`; v4 restore remediation `428fe7c9`; build/measurement workflow `90d68784`

**Research basis:** [Storage Scaling and Bounded Finality](../../architecture/storage-scaling-research-spec.md)

**Implementation contract:** [Storage Scaling Fix](../../architecture/storage-scaling-fix-spec.md)

**Independent review:** [Storage candidate review](../../handoffs/2026-08-27___dravlic__storage_candidate_review.md)

## Plain-English decision

Post Fiat selects the transactional `redb` store as the storage candidate. The
source design is credible enough to stop comparing alternative storage designs:
one finalized height commits atomically, post-activation proposal and commit
paths use an indexed membership set plus a fixed-size accumulator, and the
selected path does not intentionally rescan full history.

The node still retains finalized history. That is not the defect being fixed.
The transactional `redb` append path continues to pass its per-window
bounded-work gates. The first prepared-input campaign instead found a complete
JSON vote-lock scan before every signature. That defect was fixed and verified
in source `442c5a4d`; selected-path votes after the one-time migration now examine
at most two files and decode at most 314 bytes.

The exactly one authorized corrected campaign then ran on 2026-08-28. All ten
selected `redb` windows completed with literal accepted receipts,
six-validator convergence, bounded redb work, zero full-history storage reads,
and passing vote-lock work gates. The corrected candidate nevertheless failed:
height-5,000/height-50 consensus-round p95 was 2.693 and wallet-to-finality p95
was 2.649, both above the required 1.10.

The next height-dependent cost is in a different surface. Before named setup and
proposal timing, each proposer resumes its durable certified-send outbox. That
path validates the entire retained completed-tombstone set twice, rereading and
hashing `job.json`, `batch.json`, and `certificate.json` for every tombstone.
Validator 0 has 240 completed tombstones at height 50 and the retention cap of
1,024 at height 5,000; only validator-0 proposer rounds exhibit the recurring
slow pattern. The work is history-proportional until the cap and still costs
about 1.5 seconds per affected proposal at the cap.

That path is now remediated and frozen at source `e52e0502`. Normal resume reads
one bounded completed-set index and validates only jobs actually compacted or
pruned; full-set validation belongs to one-time migration, explicit repair, and
the touched entries. The release 1,024-tombstone spot check dropped from 66.893
ms / 6,144 retained-file reads to 2.098 ms / zero retained payload reads. The
new runner binds a certified-send work gate and a round-timing coverage residual
gate. The source/binary candidate portion of G1 and the binary-sensitive local
G2 refresh pass; campaign-input binding is pending authorization. No new G4
performance campaign has been run or authorized, so this is remediation
readiness, not qualification.

The first legacy control then failed the locked vote-lock gate because five
validators performed their single migration in finalized round 2 rather than
round 1. The runner stopped after 2,092.637 seconds, so no legacy window, final
campaign report, or packet exists. The checkpoint SHA-256 is
`847b60f924414825ac050fd901bc80b3dbb200d7db6d91c74f1357fc018cd6c1`.
This is a valid failed campaign, the no-retry allowance is consumed, and the
candidate is not qualified. See the
[corrected G4 campaign plan](corrected-g4-campaign-plan.md) for the complete gate
table and single diagnosis.

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
| Which performance campaign? | Selected `redb` at heights 50 and 5,000; same-binary legacy control at height 50. Build height 1→5,000 once, freeze it, then apply the four-hour budget to the unchanged 5+5+5 measurement matrix. | Decided by operator on 2026-08-28 | Do not rebuild inside the measurement clock or run the superseded three-lane/five-height matrix. |
| How does the selected lane move between high-height chunks? | By a content-hashed prepared transactional fleet and one persistent setup-only batch loop, not by exporting full history or restarting a process per block. | Decided; remediation smoke passes, fresh evidence required | Bind the separate canonical batch-builder binary, raw batches, certificates, every round, and the before/after fleet digests. Keep the portable snapshot only for the shared height-50 legacy control, and keep measured windows on the unchanged measurement path. |
| Is one height-924 validator directory needed? | Yes, for exact replay only. | Authorization and custodian still required | Finish G1, G2, the height-915 part of G3, and G4 without waiting for it. |
| Are six validator directories needed now? | No. They are needed only for G6 immediately before a deployment decision. | Deferred | Spend no time collecting or rehearsing six clones during offline qualification. |
| May this plan touch the controlled devnet? | No. | Not authorized | No fleet query, copy, service action, deployment, or mutation. |
| What follows storage? | Dynamic UNL supplies proposal content inside the DGA/Cobalt envelope; Option C is the evidence sequence. | Direction recorded; implementation deferred | Keep its milestone deferred until the storage boundary in G7. |
| Does a passing packet deploy anything? | No. | Separate later decision required | G5 can establish only `OFFLINE QUALIFIED`; G6 and written deployment authorization remain separate. |

## Critical path and operator calls

The corrected G4 campaign is closed failed and cannot be retried under its
one-run rule. The certified-send remediation plan is now complete with a new
source/binary freeze and refreshed local G2 evidence. Local qualification's next
boundary is an explicit yes/no decision on exactly one new 5+5+5 campaign; G3
still independently requires corrected replay and authorized height-924 input.
The four tracks are:

| Order | Track | Start condition | Finish condition | Operator input |
| ---: | --- | --- | --- | --- |
| 1 | Local qualification | Explicit authorization names candidate `e52e0502`, runner `15d059d1`, the frozen inputs, and exactly one new 5+5+5 campaign | That single campaign passes the unchanged latency/correctness gates plus the new certified-send and timing-coverage gates | Decide yes or no on the new campaign. The closed corrected campaign may not be retried, and the devnet remains out of scope. |
| 2 | Exact height-924 replay | A custodian names one complete quiescent directory and separately authorizes a read-only copy | G3 replay and mutation sentinels pass and enter the packet | Name the custodian and authorize the copy. Do not wait idle for this input. |
| 3 | Pre-deployment rehearsal | G5 is `OFFLINE QUALIFIED` and controlled-devnet deployment is actually the next decision | G6 passes on six distinct stopped copies | Separately authorize six copies and then make a separate deploy/no-deploy decision. |
| 4 | Dynamic UNL milestone | G5 closes, or the decision owner explicitly changes priority | A separately activated governance plan exists | No choice is open now: the DGA/Cobalt envelope and Option C are the recorded direction. |

The dependency chain is:

```text
local:     fix/freeze/gates PASS -> separately authorized new G4 --+
                                                                   +-> complete G5 -> OFFLINE QUALIFIED
external:  corrected height-915 + authorized height-924 G3 --------+

OFFLINE QUALIFIED -> separate deployment decision -> G6
```

A missing height-924 copy blocks only the final `OFFLINE QUALIFIED` label. It
must not trigger a 20-hour wait, an exhaustive legacy campaign, or collection of
six fleet directories. The vote-lock and certified-send source fixes are both
verified, but no post-certified-send G4 result exists. The corrected G4 output
remains final failed evidence; a future run must start from the new frozen
candidate under separate authorization rather than resume or relabel it.

## Decisions recorded

- **Storage candidate:** transactional `redb`.
- **Performance proof:** selected-path height 50 versus height 5,000, plus a
  same-binary legacy-versus-selected comparison at height 50.
- **High-height campaign state:** a content-hashed six-node transactional fleet
  is the selected lane's checkpoint and restart boundary. Signed-corpus creation
  uses a byte-verified disposable canonical clone, binds its before/after digests
  and expected sequence, proves the frozen source is unchanged, and discards the
  scratch clone. Full portable snapshots remain required only at height 50 for
  the legacy comparison.
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
- Each G4 remediation gets one focused test/smoke cycle capped at 30 minutes.
  The prepared v4 build was reused once and the measurement-only matrix completed
  in 3,311.552 seconds, but its scaling ratios failed. That result is closed and
  must not be resumed or relabeled. The vote-lock-corrected campaign then used
  its one-run allowance and failed after 2,092.637 seconds: all ten selected
  windows completed, both scaling ratios failed, and the first legacy control
  hit a binding vote-lock migration-position failure. It also is closed and
  must not be resumed, retried, or relabeled. Any later campaign requires a new
  reviewed plan and fresh candidate freeze. Reaching a budget remains a
  recorded `TIME_BUDGET_EXCEEDED` result, not permission to continue silently.
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
| G1 — candidate freeze | **REMEDIATED SOURCE/BINARY FREEZE PASS / CAMPAIGN INPUT BINDING PENDING** | Preserve source `e52e0502`, binary `6b130a…a82e`, and manifest `895ec7…ffe`; if a campaign is authorized, bind its topology/corpus/prepared fleets before execution. | Any new node source or binary change restarts the binary-sensitive gates; no G4 may start without the input binding. |
| G2 — safety | **REMEDIATED LOCAL PASS / PACKET BINDING PENDING** | Preserve manifest `dc01f9…78e7`, rollback `af37f0…04be`, and tamper `df45e0…2ceb`; raw output remains private. | Redaction-safe repository packet binding remains open; another candidate change requires another refresh. |
| G3 — exact replay | **PRIOR HEIGHT 915 PASS / REMEDIATED REPLAY REQUIRED / HEIGHT 924 AUTHORIZATION BLOCKED** | Preserve the old height-915 receipt as history; rerun height 915 with binary `6b130a…a82e`. Run height 924 only from a separately authorized copy. | Never wait idle for the height-924 copy, but do not claim offline qualification without both remediated replay receipts. |
| G4 — scaling | **PRIOR CAMPAIGN CLOSED FAILED / NEW CAMPAIGN NOT AUTHORIZED** | Preserve checkpoint `847b60…d6c1`, failure receipt `ce8703…8c38`, and diagnosis `4c7bb6…32f8`. Candidate `e52e0502` is ready only for a separately authorized new run. | Decide yes/no on exactly one 5+5+5 campaign. Do not resume, retry, or relabel the prior output. |
| G5 — offline packet | **BLOCKED BY REMEDIATED G3 AND MISSING NEW G4 PASS** | Do not package the failed prior campaign or private G1/G2 material. | No qualification claim until remediated G3, a separately authorized G4 pass, redaction-safe packet binding, and the complete offline verifier all pass. |
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
- [x] Start from a clean checkout and record exact candidate source
      `ae65844190f153cbdd49d1e5ac28ab96a19f7af4`.
- [x] Build one release binary and record its SHA-256, Rust toolchain, locked
      dependency identity, build command, host, and storage device.
- [x] Freeze topology, keys, accounts, transaction corpus, timeouts,
      instrumentation schema, the authenticated height-50 snapshot identity,
      and the height-5,000 prepared-fleet identity.
- [x] Run the focused storage/node regression suites, formatting, and
      warnings-denied Clippy against that source.

The pinned binary SHA-256 is
`891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4`.
It embeds `build_git_revision: ae658441` and `profile: release`. The recorded
toolchain is Rust 1.95.0, with `Cargo.lock` SHA-256
`200cffda3ccd4a4a358c90077f2f94e654e1a95356b06b98aef826696e1d577f`.
The storage suite passed 83 tests with two manual tests ignored; the process
crash integration test, four bounded transport shutdown tests, 14 replicated
state activation tests, formatting, and warnings-denied storage/node Clippy all
passed. That pre-fix binary is retained only as failed-campaign history.

The corrected freeze passed at source
`442c5a4ddafed3aa0709f64e213fe0cedac5222d`, which was clean and present on
`origin/main`. Release binary SHA-256 is
`29423cba098ce793ccab4a234ab26a2d30c6b11ad9eacd339b11b89cd6187c48`;
G1 manifest SHA-256 is
`b4a580f7f4c61db4992f83f823d6715cd712589eaaada9debde3f45622f1bf01`.
The clean release build, 83 storage tests, 15 vote-lock tests, 5,000-lock release
spot check, bounded accept tests, replicated-state activation tests, formatting,
and workspace warnings-denied Clippy passed. The corrected prepared-input
manifest SHA-256
`9d48530539eaf05a18879dbafb3d7c62862617c28b843ae300dc1d87ed05cb88`
freezes the public inputs and binds the unchanged prepared fleets to the
corrected measurement binary. The candidate stayed unchanged through the one
corrected campaign and is now frozen failed evidence.

The certified-send remediation freeze passed at source
`e52e050269a2f9fdd28c5083c3888debf3a85063`, from a clean detached checkout
matching `origin/main` at freeze. Release binary SHA-256 is
`6b130a1f9c81bd64bc9dc42043595f5a27e84185cf3f40b13b5f37a40d72a82e`;
G1 manifest SHA-256 is
`895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe`.
The complete Step-7 suite passed, including the locked workspace tests, and the
release 1,024-tombstone proposer-rotation check passed with 2.098 ms resume,
zero retained payload reads/hashes, one bounded index read, and a 2.054 ms
proposer/peer delta. Runner `15d059d1` passed 95 focused runner, packager, and
independent-verifier tests. No prepared campaign input or performance run is
implied by this candidate freeze. For `e52e0502`, the source/binary portion is
complete and the frozen campaign-input portion remains pending explicit campaign
authorization.

**Exit:** one clean source, one binary, and one frozen input set control G2
through G5. Any source or binary change returns the plan to G1.

## G2 — safety and failure atomicity

**Purpose:** prove the faster path did not weaken storage integrity.

- [x] Re-run the complete 69-case tamper/crash classification from the pinned
      source.
- [x] Prove every rejected case has a stable reason and produces zero durable
      mutation.
- [x] Prove each crash cut recovers either the complete prior height or exactly
      one complete new height.
- [x] Prove clean build, snapshot restore, transactional rebuild, restart, and
      canonical export produce identical logical records, commitments, roots,
      and tips.
- [x] Re-run the compatible two-binary rollback path from the same certified tip
      without deleting or reinterpreting finalized blocks.
- [x] Re-run whole-directory mutation sentinels for every `--verify-only`
      success and refusal path.
- [ ] Commit redaction-safe reports and receipts, bind them in
      `benchmarks/storage-scaling/SHA256SUMS.txt`, and make the independent
      verifier reject a missing or altered artifact.

**Exit:** all safety checks pass and are repository-verifiable. Any ambiguity,
partial mutation, panic on untrusted storage, or unverifiable report is
`REMEDIATION_REQUIRED`.

Local receipts currently bind a passing 69-case tamper matrix SHA-256
`67f5f30b3916eed18492e3b19cad52b43b92e7fdf8bde2a12df1e00b6b4ba62f`
and compatible rollback report SHA-256
`76a121c6a5529480f4f22101fba0f16d1ee4423489b11e5a50498414d7cbb59a`.
They are not yet a G5 packet; the final unchecked G2 item is their redaction-safe
packet binding and independent packet verification.

The corrected binary-sensitive refresh also passed. G2 manifest SHA-256 is
`132220922f0d5b6e3728861f227e6aff4f28ad87f36fce6d78119f9e75ef78c7`;
compatible rollback report SHA-256 is
`1a05fe1132c4993b52171288848f03b524230c085934cfe0b4b68fa1cc359970`;
and the rerun 69-case tamper/crash report SHA-256 is
`95c723e2f054f3feafd020cfa4e8116388b4105d89ca5b7bc1b6488912541f07`.
`storage-rebuild-transactional --verify-only` passed its two focused
read-only/fail-closed tests. These receipts bind corrected source and binary but
remain private local evidence until a later redaction-safe G5 packet exists.

The certified-send-remediated binary-sensitive refresh also passed. G2 manifest
SHA-256 is
`dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7`;
compatible rollback report SHA-256 is
`af37f0e4de9d23689b131532d0fded2593905b5d88a5de1600431c511d6904be`;
and the complete 69-case / 37-owner-test tamper/crash report SHA-256 is
`df45e0bb478299e7778bc50537fd6bb059f04a19c52f01d0c5adb444331c2ceb`.
The two `transactional_verify_only` tests passed again. All evidence was produced
offline with no devnet contact. It remains private local evidence, not the
unchecked redaction-safe G5 packet.

## G3 — exact replay

**Purpose:** prove compatibility with real recorded history rather than only
synthetic fixtures.

- [x] Publish or regenerate a verifier-bound report for the exact 915-block
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

The local height-915 replay passed from an immutable source tree SHA-256
`276ea5dc9c43e42a36235b520cffd1d9a15eed842fa9a683b04024933b769403`.
Its receipt SHA-256 is
`7de050bf57c4d348f992aec11964a3b369546c7b6888e9e6a289dcf137d1680d`.
No height-924 source exists locally, and this plan does not authorize obtaining
one from the controlled devnet.

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
prepared fleet through the selected bounded path; do not advance legacy storage
to height 5,000 merely to demonstrate the already-known defect.

Run order is selected height 50, selected height 5,000, then legacy height 50.
Stop immediately when a selected-path safety, convergence, literal-receipt, or
bounded-work gate fails.

The first `ae658441` segment was stopped early at a durable height-550
checkpoint after 1,740.9 seconds. Its selected path completed all five
height-50 windows and 500 further finalized heights with literal receipts,
six-validator convergence, zero full-history scans, and nearly flat measured
round work. Across its completed advance units, mean `consensus_round_ms` was
347.6, 376.0, 369.1, 376.2, 377.6, and 384.5 ms; the five 100-round resource
windows were 122.3, 121.3, 122.0, 122.8, and 123.4 seconds. The apparent growth
in total checkpoint wall time came from replay-importing an increasingly large
portable full-history snapshot into six new nodes between every 100-height
unit, not from the measured transactional append path.

Runner lineage `260bb990` through `81c35d5d` removes that evidence-harness
bottleneck without changing the candidate binary. It builds height 5,000 in
bounded 1,500-round selected-store chunks,
freezes a hash-bound six-node prepared fleet, and restores a content-verified
copy at the canonical database path for each independent selected window. The
legacy control still imports the shared portable height-50 snapshot. The packet
verifier now requires every selected window's prepared-fleet digest to match
the frozen height material. A real selected/legacy development smoke passed
with exact cross-backend final state; report SHA-256 is
`50d3d74edcc03e64c03293d54d20498fd73c54aaed1e4b24b6aacb84898f0994`.
The stopped height-550 run is diagnostic only and is not evidence eligible.
The campaign report records candidate source, embedded binary source, clean
runner-checkout revision, and exact runner file hashes as distinct provenance;
packet assembly preserves the same separation.
Every completed advance chunk also freezes a content-hashed prepared fleet;
the next chunk resumes from an independently verified copy at the canonical
database path. A controlled stop after an advance and resume passed before the
release rerun, so no chunk is both longer than the operator limit and
all-or-nothing.

The release rerun on runner `4f976290` completed the height-1-to-50 advance,
all five selected height-50 windows, and the height-50-to-1,550 advance. At
height 1,550 all six validators agreed, the prepared fleet SHA-256 was
`368aa0aacd9a7889e0de4a089e7cb91bb0230676c0c9433d67c03c52001d108e`,
and the run had consumed 3,830.6 seconds. Before the first round of the
height-1,550-to-3,050 unit, corpus setup tried to import the portable snapshot
and failed because `blocks.json` was 317,562,113 bytes while
`MAX_STATE_FILE_BYTES` is 268,435,456 bytes. The checkpoint is correctly marked
`FAILED`; there is no final report, and no part of this run is release evidence.
The completed prepared fleet is diagnostic input for remediation tests only.

### G4A — close the high-height harness boundary before another long run

- [x] Preserve the failed checkpoint, exact runner and candidate identities,
      height-1,550 prepared-fleet digest, elapsed time, and failure boundary.
- [x] Confirm the failure occurred during corpus setup before any round in the
      height-1,550-to-3,050 unit, not in transactional consensus or append.
- [x] Confirm the failure is caused by portable full-history materialization,
      not by a positive full-history-read counter in the selected append path.
- [x] Make selected-lane corpus creation use a byte-verified disposable canonical
      clone of a stopped, content-hashed prepared fleet. Prove the frozen source
      digest is unchanged, bind the scratch before/after digests and expected
      sequence, discard the scratch, and restore a pristine clone for measurement.
- [x] Stop exporting result snapshots for prepared-fleet selected runs. Bind the
      source and result prepared-fleet digests instead; retain full snapshot
      export/import only for the height-50 cross-backend control.
- [x] Make checkpoint/resume and the packet verifier reject a missing, changed,
      or ambiguously located prepared fleet, corpus, or runner binding.
- [x] Add focused tests for frozen-source corpus generation on a disposable
      scratch clone, no selected high-height snapshot dependency, tamper
      rejection, and safe interruption.
- [x] Pass one real six-validator smoke that crosses a snapshot-free selected
      advance with portable snapshots forbidden, in at most 30 minutes, and
      leaves no process alive.
- [x] Start exactly one clean qualification run from height 1 after all G4A
      checks pass. Do not mutate or waive the old checkpoint into compatibility
      with a changed runner.

G4A closed on clean runner commit `f7b3d21d` without changing candidate source
or binary. Fifty-nine focused Python tests passed. The clean six-validator smoke
completed in 32.013 seconds with report SHA-256
`6c233e999edf87a289879f9bda11052fbeaf61a33bb1076fd8a43c5e94a4155b`
and checkpoint SHA-256
`554d07912428e07dfc47a0f78893fab9971851b729d675851d6804fcfb63e85e`.
Its height-3 material has null snapshot fields; its frozen prepared-fleet,
corpus-source, and scratch-before digests are identical; the scratch-after
digest differs and is recorded discarded; all selected windows passed literal
receipt, convergence, and bounded-work gates; and no process survived. A clean
controlled stop at snapshot-free height 3 resumed to report SHA-256
`b23e0b9ff21336d2fc672b667b9436efef0ffb93087cc8b68f92c934ee72bec3`
and checkpoint SHA-256
`1af632169d7db42ea702f225cccd7d4a34c6e28314fb38d5617284d4d484183f`.
Moving one file in a copied current prepared fleet made resume fail closed with
`campaign current prepared fleet changed`. These are local development proofs,
not release qualification evidence.

The one clean v2 G4 campaign started from runner checkout `8768866a` at
`2026-08-27T23:54:36Z` and reached its four-hour budget at
`2026-08-28T04:10:24Z`. Checkpoint SHA-256 is
`5c48b9b8ad7d97ac9797a5b7c2f9388328cc82beebe2f7e1f31b334caa36ef85`;
status is `INTERRUPTED`; aggregate elapsed time is 14,398.975 seconds; and no
final report exists. The durable fleet reached height 3,050 with SHA-256
`68d9e538d2f3727ab6a60db00238c9abaebfaad024c5eaf0f6aecde4a81dd8df`.
The completed height-50-to-1,550 and height-1,550-to-3,050 chunks reported p95
`consensus_round_ms` of 1,877.727 and 1,928.697 respectively, plus literal
accepted receipts, six-validator convergence, and zero full-history reads.
A partial height-3,050-to-4,550 unit completed 1,492 rounds through height 4,542
with diagnostic p95 1,928.775 ms and zero full-history reads, but it is not
release evidence. The v2 output is exhausted and must not be resumed or upgraded
in place.

**G4A exit:** high-height selected execution depends only on the verified
transactional prepared fleet and bounded corpus, while the shared height-50
legacy comparison still uses the authenticated portable snapshot. The candidate
binary remains unchanged; only the separately hash-bound evidence runner may
change.

### G4B — remove the per-block setup lifecycle after the real timeout

- [x] Preserve the exhausted v2 checkpoint, exact 14,398.975-second budget,
      durable height-3,050 fleet, incomplete height-4,542 diagnostic boundary,
      candidate identity, and absence of a final report.
- [x] Confirm that completed and partial high-height advances report zero
      full-history reads and nearly flat p95 round time; do not present partial
      output as release evidence.
- [x] Diagnose the remaining wall-time growth as one foreground node launch and
      validator-service restart cycle per setup block.
- [x] Add a separately hash-bound release helper that transforms the canonical
      signed transfer corpus into canonical transaction batches without changing
      the candidate node binary.
- [x] Use one persistent peer-certified batch loop only for setup advances. Keep
      selected and legacy measurement windows on the unchanged measured path.
- [x] Upgrade campaign, checkpoint, and packet bindings to v3 so resume and
      offline verification cover the helper binary, build revision, raw batches,
      certificates, processed rounds, fleet digests, and normalized report.
- [x] Pass two Rust helper tests, 60 focused Python tests, warnings-denied release
      clippy, and formatting checks.
- [x] Pass a real 12-round, six-validator persistent advance in one foreground
      process: height 1 to 13 in 5,655.333 ms, 72 committed writes, 4,896 page
      reads, 576 page writes, and zero full-history records or bytes read.
- [x] Pass a controlled stop/resume campaign and make resume reject a changed
      batch-builder binary before executing another unit.
- [x] Freeze a clean v3 runner and helper, then start exactly one new
      qualification output from height 1 under the approved four-hour aggregate
      budget. Do not resume or mutate the exhausted v2 output.

G4B is implemented at `438fb29c`. The v3 development campaign passed with
report SHA-256 `e1e87b854dc4fbae88f00e3ebfe9c4c848706090cd52d6c394c5cd2b8ea8518f`;
the controlled resume campaign passed with report SHA-256
`f8501bf2fa15d1fc5a5714f3f5a476b28c6419f250fed9176ebb3f4e0f1abc87`.
Both are development proofs, not release qualification evidence. The clean v3
release run used runner `2091d723`, candidate SHA-256 `891b…bf4`, and helper
SHA-256 `2ff319f7…9e38`. It started at `2026-08-28T04:51:12Z`, stopped once
at a durable unit boundary, resumed the same checksum-bound output, and reached
height 5,000. Five height-50 windows and four persistent advance chunks passed;
the final prepared-fleet SHA-256 was
`8a4618e7ea81df7d26c4547868d9941f712552fc5e8982c74bc8763909bccfeb`.
At `2026-08-28T08:45:40Z`, after 13,987.118 aggregate seconds, the first
height-5,000 window completed 50 valid measured rounds but failed the required
resource evidence with `RuntimeError`; no normalized window, campaign report,
or G4 pass exists. Checkpoint SHA-256 is
`e1ba0e166e801495f9a2a7f4a18b8264f5d84fbc827d4645d3dcd6f097082459`.
The v3 output is frozen failed evidence and cannot resume under v4.

### G4C — make sampling deterministic and restore large fleets inside budget

- [x] Preserve the v3 checkpoint, exact source/binary/helper identities, durable
      height-5,000 fleet, completed raw 50-round report, failure boundary, elapsed
      time, absence of a campaign report, and zero surviving processes.
- [x] Diagnose the resource failure: the sampler's initial full directory walk
      delayed its periodic loop while foreground measurement began immediately.
- [x] Make the sampler return only after its first complete sample, propagate a
      startup failure, and retain the rule that every foreground process needs at
      least two observations.
- [x] Diagnose the remaining budget cost as repeated full copies and hashes of
      178,214 files totaling 19,040,307,767 bytes.
- [x] Reuse the canonical workspace, restore changed files incrementally, verify
      the complete destination digest, and fall back to a full copy on any digest
      mismatch. A metadata-preserving content substitution test exercises the
      fallback.
- [x] Preserve the authenticated absolute generation-pointer boundary: corpus
      scratch runs only at the canonical database path, its changed digest is
      recorded, and the workspace is restored to the frozen digest before any
      setup or measurement. Reject a pointer bound anywhere else.
- [x] Upgrade campaign, checkpoint, packet, and offline-verifier bindings to v4,
      including the post-scratch restored-fleet digest.
- [x] Pass 64 focused Python tests and a 29-second real six-validator development
      smoke. Every measured foreground process had at least four samples, corpus
      scratch mutated and restored exactly, and no process survived.
- [x] Pass a development height-5,000 preflight against the preserved 19 GB
      fleet. A clean copy took 170.387 seconds; a forced reset of all six
      1,813,778,432-byte database files took 114.859 seconds; the source digest
      remained exact and the disposable workspace was removed.
- [x] Freeze the v4 runner at clean commit `03123ca0`, rebuild helper SHA-256
      `f8bb25f2df22a8337d571a404ae3d4799735978d162c086945d5ad95c6c1ca73`,
      repeat the real smoke and height-5,000 preflight, and commit the
      redaction-safe preflight report. The 29.332-second smoke report and
      checkpoint SHA-256 values are `c7be9957…30710` and `a8b85cba…0e9f`;
      every measured foreground process had at least four samples, scratch
      restored exactly, and no child survived. The clean preflight report is
      `benchmarks/storage-scaling/g4c-height5000-preflight-03123ca0.json`
      (file SHA-256 `84d5fa93…6e1fb`): the 19,040,307,767-byte clone took
      177.410 seconds, the forced 10,882,670,592-byte six-database reset took
      115.355 seconds, the source remained `8a4618e7…ccfeb`, and cleanup took
      3.995 seconds.
- [x] Start exactly one clean v4 qualification output from height 1 with the
      unchanged candidate binary and a fresh four-hour aggregate budget.
- [x] Preserve its final failed-closed boundary. Checkpoint SHA-256
      `8e3ed2c910b518157fe4a530f1e34f896c48dcacef291a088615d25d3a65b28d`
      records `INTERRUPTED`, height 5,000, 14,390.603 seconds, 15 completed
      units, no current unit, no final report, and no surviving process.
- [x] Verify the v4 build and selected measurements on their own merits. The five
      contiguous advances cover height 1→5,000; all five selected height-50 and
      five selected height-5,000 windows passed six-validator convergence,
      literal receipts, bounded point work, constant accumulator work, and zero
      full-history work. The old aggregate campaign failed only because the five
      legacy height-50 controls had not run before the clock expired.

**G4C exit:** the v4 harness defects are closed and the selected path has passing
raw evidence, but the old aggregate campaign rule is exhausted. G4 remains open
under the accepted build/measurement split below.

### G4D — build once, measure separately

- [x] Record the operator's 2026-08-28 approval to split setup from measurement.
      This is a time-accounting change only; the candidate, input corpus,
      5+5+5 matrix, ratios, receipts, convergence, counters, raw samples, and
      independent-verification rules remain unchanged.
- [x] Merge the prepared-input workflow at `90d68784`. The exporter accepts an
      interrupted source only after independently validating candidate/helper
      identities, contiguous advances from height 1, every receipt/report digest,
      zero full-history counters, six-validator convergence, frozen material
      digests, and the exact build-final fleet.
- [x] Pass all 77 focused runner, packet, and verifier tests plus Python compile
      and diff checks.
- [x] Export the v4 build manifest with SHA-256
      `9ac31841a41ba514855a82f52650e1951ed97c9f99d54a4048a07407d6734c61`.
      It binds candidate `ae658441`, node SHA-256 `891b…bf4`, helper SHA-256
      `dbbc…6685`, five contiguous advances through height 5,000, and final
      prepared-fleet SHA-256 `4e2f24b7…54086`.
- [x] Start exactly one fresh measurement-only output from the verified manifest.
      Its checkpoint started at zero elapsed seconds, performed no setup advance,
      and used the exact bound candidate and helper binaries.
- [x] Complete the unchanged three required rows inside the fresh four-hour
      measurement budget. The campaign completed in 3,311.552 seconds.
- [x] Verify literal accepted receipts and six-validator agreement on height,
      block hash, and state root for every measured round.
- [x] Prove the selected redb proposer/apply paths report zero full-history
      storage records and bytes, bounded indexed point work, and constant
      accumulator work.
- [x] Preserve the final report SHA-256
      `88502bca7aaa4e576e5e9684b3d9b72d8c1b66e24b6c6c8e746f11807ac7eabb`
      and its failed-closed result: `evidence_eligible: false` and
      `PUBLIC TESTNET BLOCKED`.
- [x] Remove the newly isolated O(chain-history) vote-lock directory scan from
      normal post-marker validator signing while preserving durable
      anti-equivocation semantics and legacy lock compatibility. The one allowed
      directory enumeration is the marker-absent, serialized migration.
- [x] Report bounded vote-lock files and bytes examined for every vote. The
      release-mode 5,000-history spot check examined two paths, decoded 312
      bytes, performed no migration, and reserved the lock in 0.118138 ms.
- [ ] Re-run the unchanged matrix with one corrected candidate binary and prove
      selected height-50 p95 is no more than 110% of legacy height-50 p95 for
      both required metrics.
- [ ] Prove corrected selected height-5,000 p95 is no more than 110% of selected
      height-50 p95 for both required metrics.
- [ ] Make the verifier independently recompute identities, distributions,
      ratios, storage and vote-lock counters, resource summaries, build
      bindings, and all pass/fail decisions for the corrected campaign.

**G4D exit:** the pre-fix completed campaign is valid failed evidence, not
qualification. It motivated one vote-lock-corrected campaign under the separate
[corrected G4 plan](corrected-g4-campaign-plan.md).

### G4E — corrected vote-lock campaign: closed fail

- [x] Repair the local Zig C/C++/archive wrappers and prove a clean-checkout
      release build from pushed `main`.
- [x] Freeze corrected source `442c5a4d`, node binary SHA-256 `29423c…7c48`,
      and G1 manifest SHA-256 `b4a580…bf01`.
- [x] Refresh binary-sensitive G2 safety evidence and bind manifest SHA-256
      `132220…78c7`.
- [x] Extend the runner with vote-lock work parsing and stable gates; pass 84
      focused tests and freeze runner `693855e3`.
- [x] Derive one read-only prepared-input manifest that distinguishes the old
      binary which built the frozen fleets from the corrected binary used for
      measurement. Manifest SHA-256 is `9d4853…cb88`.
- [x] Execute exactly one unchanged 5+5+5 campaign under one fresh four-hour
      measurement clock, selected windows first.
- [x] Preserve the failed checkpoint, legacy failure receipt, single diagnosis,
      exact gate values, and zero surviving campaign processes.
- [x] Stop without retrying, changing the candidate, enlarging the matrix, or
      contacting the devnet.

The runner work is committed locally on branch
`postfiatchad/corrected-g4-vote-lock-gate` at
`693855e3492bc3d37801653e90bc308969fbad85`. The three commits add the work
gate, preserve `prepared_by` build provenance, and make manifest derivation
read-only. They are not merged into this repository's `main`; the source hash is
the binding.

The first derivation attempt called candidate `status` on the private frozen
seed and changed exactly `postfiat-state-v1.redb`. The changed file was preserved
in a separate private recovery directory with SHA-256
`b19d36f15fce49eceb57f571a7e2e23f79c18c13967e410103e1fe887924fac6`.
The source file was restored byte-for-byte from its independently verified copy,
and the complete private bundle returned to its expected SHA-256
`fbb74d2352ac7a60058ac845dd1d4968ef07aa32ec9bf27278295996d3013a54`.
Runner `693855e3` then derives the manifest from the already frozen G1 identity
without opening candidate state. This incident did not alter the frozen input
used by the campaign.

The corrected campaign checkpoint is private at
`~/repos/postfiat-storage-g4-measurement-693855e3-442c5a4d-v1` and has SHA-256
`847b60f924414825ac050fd901bc80b3dbb200d7db6d91c74f1357fc018cd6c1`.
It records ten completed selected windows and failure in
`legacy-jsonl/height-50-window-1` at `2026-08-28T22:33:12Z`. The selected
latency ratios are 2.693 and 2.649; all selected correctness, redb work, and
vote-lock gates pass. The legacy failure receipt SHA-256 is
`ce8703dfc16c22c3930508b314231c6992c82fa20c54bc9d9fa2254da9c98c38`.
The redaction-safe private diagnosis SHA-256 is
`4c7bb67b8622de967b240a6583a34bd554e9a1c7f19672ef43a06f88ef7832f8`.
No final report or packet was created because the campaign failed before the
legacy controls completed.

The single diagnosis implicates the uninstrumented proposer outbox-resume phase:
`transport_peer_certified_batch_round` calls
`resume_durable_certified_send_outbox` before named setup timing; compaction and
pruning each validate the complete retained certified-send tombstone set. That
set grows from 240 tombstones at height 50 to the 1,024 cap at height 5,000.
Every retained job causes `job.json`, `batch.json`, and `certificate.json` to be
read and both payloads rehashed twice per affected proposal. The slow rounds are
exactly validator-0 proposer rounds in all ten selected windows.

**G4E exit:** **FAIL; no retry authorized.** The vote-lock fix worked, but the
candidate remains unqualified. The next source owner is certified-send
tombstone resume/retention in `crates/node/src/transport_cli.rs`, with the
missing phase timer in `crates/node/src/transport_runtime.rs`. Any fix or later
campaign requires a new reviewed plan, new source/binary freeze, and refreshed
binary-sensitive gates.

### G4F — certified-send remediation freeze: ready, no campaign authorized

- [x] Implement the reviewed
      [certified-send tombstone bounding plan](certified-send-tombstone-bounding-plan.md)
      at source `e52e0502` without changing consensus bytes or durable delivery
      semantics.
- [x] Move full completed-set validation from every proposal to one-time index
      migration, explicit repair, and entries touched by compaction/pruning;
      preserve fail-closed migration, repair, quarantine, crash recovery,
      retention, and compatible rollback.
- [x] Add `outbox_resume_ms`, bounded-work counters, per-validator first-resume
      migration rules, independently recomputed certified-send work receipts,
      and a round-coverage residual gate in runner `15d059d1`.
- [x] Prove flat retained-payload work at 0/240/1,024 tombstones and pass the
      release proposer-rotation check: 2.098 ms at 1,024, zero retained payload
      reads/hashes, one bounded index read, and 2.054 ms peer delta.
- [x] Complete the round-path `read_dir` audit with zero unbounded synchronous
      round-history sites.
- [x] Pass focused node/storage tests, formatting, workspace check,
      warnings-denied workspace Clippy, the complete locked workspace tests, and
      95 focused runner/packager/verifier tests.
- [x] Freeze binary SHA-256 `6b130a…a82e` and G1 manifest SHA-256
      `895ec7…ffe`; refresh G2 manifest `dc01f9…78e7`, rollback
      `af37f0…04be`, and tamper/crash `df45e0…2ceb`.
- [x] Stop before any performance campaign, devnet contact, height-924 access,
      deployment, or G5 packaging.

**G4F exit:** remediation source, telemetry, runner gates, the G1 source/binary
candidate freeze, and private local G2 evidence are ready. Campaign-input
binding remains pending. **G4 is not passed.** Exactly one new 5+5+5 campaign may
start only after separate operator authorization pins the candidate, runner,
frozen inputs, timeout, stop rules, and evidence identities.

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

1. Preserve every prior campaign and the corrected checkpoint as immutable
   failed evidence. Do not resume, retry, merge outputs, or relabel partial work.
2. Preserve the closed `442c5a4d` campaign lineage separately from remediated
   source `e52e0502`, binary `6b130a…a82e`, G1 `895ec7…ffe`, G2
   `dc01f9…78e7`, and runner `15d059d1`.
3. Make an explicit yes/no decision on exactly one new 5+5+5 campaign. A yes
   must pin the remediated candidate, runner, prepared inputs, timeout, stop
   rules, and evidence identities before execution; silence is not permission.
4. If authorized, run selected windows first under one fresh four-hour
   measurement clock and enforce the certified-send bounded-work and
   round-coverage residual gates. Stop after that single result; do not contact
   the devnet or expand the matrix.
5. Treat any new node-source or binary change as a new G1/G2 freeze. Treat any
   runner or prepared-input change as a newly bound campaign identity.
6. Rerun height 915 for the remediated candidate. Close height 924 only after a
   custodian and separate read-only copy authorization exist; do not wait idle
   for that decision.
7. Build G5 and record `OFFLINE QUALIFIED` only after remediated G3, a future G4
   pass, redaction-safe packet binding, and the complete offline verifier pass.
8. Run G6 only if controlled-devnet deployment is actually the next decision
   and its separate data-copy authorization has been recorded.
9. Activate the deferred Dynamic UNL milestone only at the G7 boundary or after
   an explicit operator reprioritization.

The milestone remains active and public testnet remains blocked. The vote-lock
and certified-send source fixes are implemented and locally verified. The
immediate decision is whether to authorize exactly one new campaign against the
remediated freeze; this document records readiness but does not authorize it.
