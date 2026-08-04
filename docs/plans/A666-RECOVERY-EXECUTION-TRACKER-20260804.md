# A666 Recovery Execution Tracker

- **Date opened:** 2026-08-04
- **Status:** LIVE TRACKING DOCUMENT — update the checkbox in the same commit
  as the evidence that justifies it; a checked box without an evidence path
  is invalid.
- **Contracts:** `A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`
  (gates), `A666-PATH-TO-ACTUALLY-GOOD-20260804.md` (destination)
- **Evidence root:** `docs/evidence/a666-public-reserve-product-20260803/`
- **Authority:** company principal (sole abort authority; items marked
  **[DECISION]** cannot be checked by the implementer)

Legend: `[x]` done with evidence · `[ ]` open · **(gate)** = checking every
box in the section passes that recovery-spec gate.

---

## R0 — Truth freeze **(gate: PASSED)**

- [x] Live baseline captured: height 776, tip, state root, 6/6 identical —
  `baseline/a666-state-inventory.json`, `baseline/fleet-status.json`
- [x] Repository, wallet-runtime, private-swap, recovery-snapshot state
  recorded — `baseline/*.json`
- [x] Dirty worktree triaged into reviewed commits; secret scan clean;
  tracked tree clean (`e4e8ee5`..`f14499a`)

## R1 — Fast loop **(gate: PASSED)**

- [x] Controlled report written on success **and failure**, computed `ok`,
  run ID, revision, binary hash (`885d954`; unit test
  `controlled_report_is_written_even_on_failure`)
- [x] `POSTFIAT_A666_CONTROLLED_REPORT_FILE` mandatory; proving-chained
  runner disabled (`retry-6-evidence-integrity-failure.json`)
- [x] Signed six-validator lifecycle checkpoint module + fail-closed importer
  (`2099cea`), finalized-checkpoint basis pinned under signature (`e71d76d`)
- [x] Seven positive/adversarial import vectors passing in ~1s
  (`lifecycle_checkpoint_tests.rs`)
- [x] Cold test split: inputs / history generation / lifecycle; extraction
  and repack tests (`2cb37f3`)
- [x] Real pre-migration checkpoint extracted (height 792, signed, private
  sidecar separated); repack path proven at 12s

## R2 — Regression closure **(gate: OPEN)**

Observed-defect regressions (defect -> test, fix committed):

- [x] Defect 7: sub-minimum redemption order — **AR-09**
  `pftl_uniswap_v2_primary_redeem_enforces_production_shaped_policy_binding_ar09`
  (`81bacfc`, ~7s)
- [x] Defect 8: private-primary archive replay rejection — **AR-10**
  `ar10_private_primary_*_replay_matches_live_execution_path` (`f059bdb`,
  fails pre-fix, 262/262 suite green)
- [x] Defect 9: spread custody dropped from supply inventory — **AR-11**
  `ar11_issued_asset_supply_counts_non_nav_spread_custody` (`83ac75d`,
  178/178 suite green)

Mandatory set still to extract as standalone <2-minute tests:

- [ ] AR-01 pfUSDC reserve-account identity/balance fixture contract
- [ ] AR-02 production cap/order inequalities as runtime test against live
  route config (compile-time asserts exist)
- [ ] AR-03 quorum-first commit with intentionally offline validator;
  convergence after recovery
- [ ] AR-04 authenticated catch-up accepts only pinned height/tip/root,
  rejects every mismatch
- [ ] AR-05 active export entitlement blocks route-epoch advancement
- [ ] AR-06 duplicate submission: typed admission vs typed finalized
  rejection, never ambiguous success
- [ ] AR-07 fail-closed rejections: replay, stale proof, wrong profile,
  wrong overlay, wrong supply, wrong NAV, wrong packet (one test per axis)
- [ ] AR-08 snapshot-import verification mapped to existing
  signed-snapshot/checkpoint vectors in the manifest (no new code expected)
- [ ] Regression manifest JSON: defect->test traceability, runtimes, first
  passing commit; wired into CI

## R3 — Repeatability **(gate: PASSED)**

- [x] Run 4: first complete lifecycle pass in project history (16.7 min,
  `checkpoint-lifecycle-run4-report.json`)
- [x] Runs 5/6/7: three consecutive `ok:true` on exact commit `2eb9427`,
  identical binary, distinct run IDs, 1004/1009/1005s —
  `r3-repeatability-gate.json` (`5bfe466`), machine-checked consistency

## R4 — Browser readiness **(gate: OPEN)**

- [ ] Reload/reconnect/recovery e2e coverage for journey step 9 in
  `wallet-web` (currently zero coverage)
- [ ] `npm test`, `test:custody-browser`, `test:public-browser`, `build`
  green on the candidate
- [ ] Full §8 browser journey pass #1 against checkpoint-restored rehearsal
  environment, StakeHub absent, receipts captured
- [ ] Full §8 browser journey pass #2 (same candidate, fresh run)
- [ ] Custody-boundary evidence: no server-side spend signing, no seed
  egress, pending-state survival across proxy restart + reload

## R5 — Cold qualification **(gate: OPEN)**

- [ ] Preconditions green: all R2 boxes + both R4 journey passes + fmt,
  strict clippy, release build, artifact policy on unchanged commit
- [ ] One genesis-to-tip six-validator cold run passes with outage,
  catch-up, restart, replay, rollback (O1 report wrapper; no blind retries —
  any failure returns to the fast loop with a new AR test first)

## R6 — Release integrity **(gate: OPEN)**

- [ ] Reproducible release build; binary hashes recorded and signed
- [ ] Strict CI on the exact revision (fmt, clippy, full suites, artifact
  policy, secret scan)
- [ ] Release manifest signed and archived

## R7 — Clean public reproduction **(gate: OPEN)**

- [ ] Guest-ELF pin made repository-relative / archived-by-hash in
  `identity-and-public-value-pins.json`
- [ ] `source_commit` vs `candidate_revision` discrepancy explained or fixed
- [ ] `A666_CIRCULATING_SUPPLY` (31_597_197_455) vs live
  `outstanding_supply_atoms` (99_000_000) reconciled and documented
- [ ] Reproduction script: fresh clone -> hash verify -> proof verify ->
  qualification env -> checkpoint lifecycle -> browser journey with
  generated credentials -> receipt verification; fails on any StakeHub
  dependency
- [ ] Clean-clone transcript + report archived
- [ ] **[DECISION]** demo/investor date may now be scheduled (principal)

## R8 — Live preflight **(gate: FROZEN until R0-R7 and decisions)**

- [ ] **[DECISION]** key rotation: rotate live operator/signer/publisher
  keys, or signed accepted-risk record (principal)
- [ ] Credential inventory of terminated staff completed (hosts, RPC,
  bridge relays)
- [ ] Signed recovery snapshot of live chain, independently verified
- [ ] All-six live convergence verified; route paused; signer separation and
  double-sign prevention confirmed
- [ ] Rollback rehearsal from the signed snapshot passes
- [ ] **[DECISION]** preflight report hash confirmed by principal

## R9 — Live migration **(gate: FROZEN)**

Each step requires prior-step evidence plus principal confirmation:

- [ ] Deploy signed release; all-six convergence before any A666 mutation
- [ ] Bind pinned successor to existing A666 profile; epoch-7 proof verified
  on all six
- [ ] Reopen bounded admission
- [ ] Canary 1: transparent issue/redeem + convergence + conservation
  **[DECISION]**
- [ ] Canary 2: private issue/redeem + convergence + conservation
  **[DECISION]**
- [ ] Canary 3: Ethereum export/return + convergence + conservation
  **[DECISION]**
- [ ] Full live lifecycle; R9 receipts and conservation report published

## R10 — StakeHub deprecation **(gate: FROZEN)**

- [ ] StakeHub proof/runtime authority removed from the public stack
- [ ] Continued operation proven without it
- [ ] `stakehub_deprecated=true` published, computed from checks
- [ ] Incident review + permanent regression mapping published

## Phase 3 — Stays-good (rolling, starts now)

- [ ] CI blocks merges on all AR tests, checkpoint vectors, wallet suites
- [ ] CI rejects: `prove` in test paths, success-only report emission,
  StakeHub identifiers in runtime code, hand-set `ok` fields, secrets
- [ ] Nightly scheduled checkpoint-lifecycle run publishing reports; two
  consecutive failures freeze merges
- [ ] Two-week green streak on the nightly run
- [ ] Monthly recovery drill (restore from signed snapshot + outage
  catch-up) with archived report — first drill completed
- [ ] Runbooks: one page per operator action ending in a report-hash check
- [ ] **[DECISION]** staffing: operations/abort owner, protocol reviewer,
  wallet owner assigned — or signed accepted-risk record (principal)
- [ ] One external stranger completes clean-clone verification; transcript
  archived

---

## Scoreboard

| Gate | R0 | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | R10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| State | ✅ | ✅ | 3/12 | ✅ | 0/5 | 0/2 | 0/3 | 0/6 | 🔒 | 🔒 | 🔒 |

Next action: AR-01..AR-08 extraction (R2). Critical path to a schedulable
date: R2 -> R4 -> R5 -> R6 -> R7, estimated 2-4 working days at the current
17-minute loop cadence.
