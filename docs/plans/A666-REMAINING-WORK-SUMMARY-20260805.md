# A666 Remaining Work Summary

- **Prepared:** 2026-08-05
- **Source of truth:** `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`
- **Purpose:** bookkeeping only. This document neither authorizes nor executes a
  journey, service change, live-chain action, credential operation, or
  StakeHub action.

## Executive status

- R0 through R3 are recorded as passed; R4 through R7 remain open; R8 through
  R10 are frozen ([tracker:18-134](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
  [tracker:136-279](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- Regression traceability is complete for 13 observed and 7 harness defect
  classes, with 26 standalone tests, three browser rows, and one wallet-unit
  row; it does not close the unchecked R4-R10 work
  ([regression-manifest-ci.txt](../evidence/a666-public-reserve-product-20260803/regressions/regression-manifest-ci.txt)).
- Three of eight principal decision records are decided: key rotation accepted
  risk, StakeHub restoration GO, and Phase 3 staffing accepted risk. Five
  remain open ([tracker:203-216](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
  [tracker:226-231](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
  [tracker:255-256](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
  [tracker:274-277](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## Critical path and schedulable demo date logic

1. **R4 close:** resolve DEFECT-13 staging scope, then complete two fresh §8
   browser passes and custody-boundary evidence ([tracker:147-174](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
2. **R5:** only after both R4 passes, run unchanged-commit cold qualification
   and its outage/catch-up/restart/replay/rollback path
   ([tracker:178-180](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
3. **R6:** produce the signed reproducible build, strict-CI, and release
   manifest chain ([tracker:186-189](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
4. **R7:** complete clean public reproduction, including the ELF and
   verifier-key reconciliation below ([tracker:193-202](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
5. **Demo-date selection:** only after R7, principal decision line 203 can
   schedule a demo or investor date ([tracker:203](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

**Estimate, not completion status:** R3 demonstrated a first full lifecycle in
16.7 minutes and three repeatable runs of 1004, 1009, and 1005 seconds
([tracker:130-134](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)). Reserve at
least one such measured lifecycle window for each clean R4/R5 attempt, plus a
separate STOP-no-retry remediation window if it fails; R5 returns a failure to
the fast loop ([tracker:180-182](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
No committed evidence supports a calendar completion estimate for R6 or R7.
R8-R10 remain decision-gated, not schedule-driven
([tracker:205-261](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R4

- [ ] **DEFECT-13:** fresh self-custody onboarding still needs pfUSDC/A666
  trustline closure. Holder-signed trustline creation is wallet product scope;
  A666 issuer authorization is direct rehearsal/operator scope
  ([tracker:147-154](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Complete §8 browser journey pass #1 in the checkpoint-restored,
  StakeHub-absent rehearsal and record receipts ([tracker:170-171](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Complete §8 browser journey pass #2 on the same candidate with a fresh
  run ([tracker:172](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Record custody-boundary evidence for no server-side spend signing, no
  seed egress, and pending-state survival across proxy restart/reload
  ([tracker:173-174](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- Step-9 recovery and successor wallet qualification are completed
  prerequisites, not substitutes for the two full passes
  ([tracker:156-169](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R5

- [ ] Run unchanged-commit prerequisites: R2, both R4 passes, fmt, strict
  clippy, release build, and artifact policy ([tracker:178-179](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Run one genesis-to-tip six-validator cold qualification covering outage,
  catch-up, restart, replay, and rollback. A failure is a fast-loop/AR-test
  event, not a blind retry ([tracker:180-182](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R6

- [ ] Produce a reproducible release build with recorded and signed binary
  hashes ([tracker:186](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Run strict CI on that exact revision: fmt, clippy, full suites, artifact
  policy, and secret scan ([tracker:187-188](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Sign and archive the release manifest ([tracker:189](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R7

- [ ] Make the guest-ELF pin repository-relative or archive it by hash in
  `identity-and-public-value-pins.json` ([tracker:193-194](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Explain or correct the `source_commit` versus `candidate_revision`
  discrepancy and reconcile the circulating-supply figures
  ([tracker:195-197](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Publish the stranger-runnable reproduction script: fresh clone, hash
  verification, proof verification, qualification environment, checkpoint
  lifecycle, generated-credential browser journey, receipt verification, and
  a hard failure on a StakeHub dependency ([tracker:198-201](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Archive its clean-clone transcript and report ([tracker:202](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

**Verifier-key reconciliation is an R7 blocker.** Historic `0x004e44…`
entered a stale rehearsal stack, while the candidate guest ELF and proof report
use `0x003af4…`; the current preflight records candidate-side equality but
does not claim public-reproduction reconciliation
([verifier-acceptance-preflight.json](../evidence/a666-public-reserve-product-20260803/browser/r4-construction/verifier-acceptance-preflight.json)).
The guest ELF must be archived/repository-pinned, its source commit reconciled
to the candidate, and the stranger reproduction script must exercise that
archived identity before the date decision can unblock.

## R8

- **DECIDED:** no credential/key rotation is the accepted-risk alternative;
  the R8 checkbox remains open because the other preflight requirements remain
  ([tracker:207-216](A666-RECOVERY-EXECUTION-TRACKER-20260804.md);
  [A666-DECISION-KEY-ROTATION-20260805.md](A666-DECISION-KEY-ROTATION-20260805.md)).
- [ ] Complete terminated-staff credential inventory, signed recovery snapshot,
  six-validator convergence/route pause/signer separation, and rollback
  rehearsal ([tracker:210-215](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Principal confirms the preflight report hash before R9
  ([tracker:216](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R9

- [ ] Deploy the signed release and prove all-six convergence before any A666
  mutation ([tracker:222](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Bind the pinned successor and verify epoch-7 proof on all six, then
  reopen bounded admission ([tracker:223-225](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Execute each transparent, private, and Ethereum canary only with the
  required principal confirmation; each must prove convergence and conservation
  ([tracker:226-231](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Publish the full live lifecycle, receipts, and conservation report
  ([tracker:232](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## R10

- **DECIDED:** StakeHub restoration GO executed with evidence commit
  `688b67a`; service state was restored 5/5
  ([tracker:255-256](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Remove StakeHub proof/runtime authority from the public stack only
  ([tracker:236-258](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Prove continued operation without it, publish computed
  `stakehub_deprecated=true`, and publish incident/regression mapping
  ([tracker:259-261](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## Phase 3

- [ ] Keep CI merge-blocking for all AR tests, checkpoint vectors, and wallet
  suites ([tracker:265](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Keep CI rejection coverage for direct test-path proving, success-only
  report emission, retired runtime identifiers, literal status fields, and
  secrets ([tracker:266-267](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- [ ] Operate the nightly lifecycle policy, two-week green streak, monthly
  signed-snapshot drill, report-hash runbooks, and external stranger verification
  ([tracker:268-273,278-279](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
- **DECIDED:** single operator plus automation is the staffing accepted-risk
  shape; Phase 3 remains open until operational work executes
  ([tracker:274-277](A666-RECOVERY-EXECUTION-TRACKER-20260804.md);
  [A666-DECISION-STAFFING-20260805.md](A666-DECISION-STAFFING-20260805.md)).

## Principal decisions

The tracker has eight actual principal decision records, excluding its
line-11 authority marker. Three are decided: key rotation accepted risk,
StakeHub restoration GO, and staffing accepted risk
([tracker:207-209](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
[tracker:255-256](A666-RECOVERY-EXECUTION-TRACKER-20260804.md),
[tracker:274-277](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)). The five
remaining decisions are exactly:

1. **Demo/investor date selection** after R7, which unblocks public scheduling
   only ([tracker:203](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
2. **R8 preflight/report-hash approval**, which unblocks the R9
   prior-step-evidence chain ([tracker:216-232](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
3. **Canary 1 approval** for the transparent issue/redeem canary only
   ([tracker:226-227](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
4. **Canary 2 approval** for the private issue/redeem canary only
   ([tracker:228-229](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).
5. **Canary 3 approval** for the Ethereum export/return canary only
   ([tracker:230-231](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

The company principal remains sole abort authority
([tracker:10-11](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)). Under the
single-authority live rule, automation prepares and validates but cannot
self-authorize a live mutation; every live step needs Principal confirmation
of that step's preflight report hash
([A666-PATH-TO-ACTUALLY-GOOD-20260804.md](A666-PATH-TO-ACTUALLY-GOOD-20260804.md),
§4).

## Named follow-ups

- **DEFECT-13-FOLLOW-UP — issuer TrustSet bundle-helper gap:** the certified
  bundle helper cannot express issuer-signed TrustSet source selection. The
  campaign requires the direct request path; helper support remains follow-up
  ([tracker:153-154](A666-RECOVERY-EXECUTION-TRACKER-20260804.md);
  [v7-trustline-stager-construction.json](../evidence/a666-public-reserve-product-20260803/browser/r4-pass1/v7-trustline-stager-construction.json)).
- **R7-reserve-proof-websocket-frame-request-binding:** implemented and
  load-bearing in V6 at commit `293abbc4425a0dd96a7b57bddbb8a09de02e850f`;
  the shared observer rejects unrelated packet-shaped frames and wrong request
  IDs ([step3-shared-choreography-green.json](../evidence/a666-public-reserve-product-20260803/browser/r4-construction/step3-shared-choreography-green.json);
  [regression-manifest.json](../evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json)).
- **FAST-LOOP-01 — proving-outlier feature gate:** the 2499.29-second
  Asset-Orchard lib-test proving outlier is a candidate feature gate and is out
  of scope for this campaign ([ar03-test.txt](../evidence/a666-public-reserve-product-20260803/regressions/ar03-test.txt);
  [regression-manifest.json](../evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json)).
- **PHASE3-5.1-CI-POLICY-REMAINDER:** the rejection-policy checker is green and
  merge-blocking, while the broader Phase-3 tracker checkbox remains open until
  sustained CI/stays-good work closes
  ([ci-policy-greps.txt](../evidence/a666-public-reserve-product-20260803/regressions/ci-policy-greps.txt);
  [tracker:265-273](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

## StakeHub and exclusions

> **Standing Sauron constraint, preserved in meaning:** StakeHub operates
> indefinitely as a product. It is never deleted, destroyed, uninstalled,
> decommissioned, defunded, emptied, or stripped of keys or data. R10 decouples
> only NAVCoin/NAV calculation and reserve-proof authority. A rehearsal stop is
> temporary and teardown requires restart of the inventory-matched units with a
> matching restarted-unit count
> ([tracker:236-247](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

**Official-window minimized-downtime rule:** pre-stage all dependencies; stop
the StakeHub dashboard as the final act before fire; restart immediately on
termination, refusal, failure, or success. Every absence/restart proof records
a stop timestamp, restart timestamp, and computed downtime duration
([tracker:249-253](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)). The completed
restoration decision records service state restored 5/5
([tracker:255-256](A666-RECOVERY-EXECUTION-TRACKER-20260804.md)).

This summary explicitly excludes live-chain mutations, key or credential
values, fund/data/configuration changes, journey firing, and execution of any
task. It records planning state only.

## Coverage table

Coverage is one row per unique tracker source line. Forty unchecked boxes,
nine decision markers, and five non-checkbox decision/authority lines yield
45 coverage rows. Of eight actual principal decisions, exactly five remain
open; line 11 is an authority marker, not a decision.

| Source item ID / text | Source line | Summary section | Gate | Status / evidence |
|---|---:|---|---|---|
| AUTHORITY-DECISION — implementer cannot check decisions | 11 | Principal decisions | all | Authority marker; [tracker:10-11](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R4-DEFECT-13 — self-custody trustlines | 147 | R4 | R4 | OPEN; [tracker:147-154](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R4-PASS-1 — §8 browser journey pass #1 | 170 | R4 | R4 | OPEN; [tracker:170-171](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R4-PASS-2 — §8 browser journey pass #2 | 172 | R4 | R4 | OPEN; [tracker:172](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R4-CUSTODY — custody/restart/reload evidence | 173 | R4 | R4 | OPEN; [tracker:173-174](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R5-PRECONDITIONS — unchanged-commit qualification inputs | 178 | R5 | R5 | OPEN; [tracker:178-179](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R5-COLD-RUN — six-validator lifecycle with outage | 180 | R5 | R5 | OPEN; [tracker:180-182](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R6-REPRODUCIBLE-BUILD — signed binary hashes | 186 | R6 | R6 | OPEN; [tracker:186](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R6-STRICT-CI — exact revision | 187 | R6 | R6 | OPEN; [tracker:187-188](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R6-RELEASE-MANIFEST — signed archive | 189 | R6 | R6 | OPEN; [tracker:189](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-GUEST-ELF — repository pin/archive | 193 | R7 | R7 | OPEN; [tracker:193-194](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-SOURCE-CANDIDATE — revision reconciliation | 195 | R7 | R7 | OPEN; [tracker:195](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-SUPPLY — circulating/outstanding reconciliation | 196 | R7 | R7 | OPEN; [tracker:196-197](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-REPRO-SCRIPT — stranger-runnable reproduction | 198 | R7 | R7 | OPEN; [tracker:198-201](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-CLEAN-CLONE — archived transcript/report | 202 | R7 | R7 | OPEN; [tracker:202](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R7-DATE-DECISION — demo/investor date | 203 | Principal decisions | R7 | OPEN DECISION; [tracker:203](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-KEY-ROTATION — accepted-risk record | 207 | R8 | R8 | DECIDED; [tracker:207-209](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-CREDENTIAL-INVENTORY — terminated staff | 210 | R8 | R8 | OPEN; [tracker:210-211](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-SIGNED-SNAPSHOT — independent verification | 212 | R8 | R8 | OPEN; [tracker:212](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-LIVE-CONVERGENCE — paused route and signer separation | 213 | R8 | R8 | OPEN; [tracker:213-214](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-ROLLBACK — signed snapshot rehearsal | 215 | R8 | R8 | OPEN; [tracker:215](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R8-PREFLIGHT-HASH — principal confirmation | 216 | Principal decisions | R8 | OPEN DECISION; [tracker:216](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-DEPLOY — signed release/all-six convergence | 222 | R9 | R9 | FROZEN; [tracker:218-222](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-BIND — successor/profile/epoch-7 | 223 | R9 | R9 | FROZEN; [tracker:223-224](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-ADMISSION — reopen bounded admission | 225 | R9 | R9 | FROZEN; [tracker:225](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-TRANSPARENT — issue/redeem | 226 | R9 | R9 | FROZEN; [tracker:226-227](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-TRANSPARENT-DECISION — principal confirmation | 227 | Principal decisions | R9 | OPEN DECISION; [tracker:226-227](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-PRIVATE — issue/redeem | 228 | R9 | R9 | FROZEN; [tracker:228-229](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-PRIVATE-DECISION — principal confirmation | 229 | Principal decisions | R9 | OPEN DECISION; [tracker:228-229](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-ETHEREUM — export/return | 230 | R9 | R9 | FROZEN; [tracker:230-231](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-CANARY-ETHEREUM-DECISION — principal confirmation | 231 | Principal decisions | R9 | OPEN DECISION; [tracker:230-231](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R9-FULL-LIFECYCLE — receipts/conservation | 232 | R9 | R9 | FROZEN; [tracker:232](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R10-RESTORATION-GO — service restoration 5/5 | 255 | R10 | R10 | DECIDED, commit `688b67a`; [tracker:255-256](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R10-DECOUPLE — public proof/runtime authority | 258 | R10 | R10 | FROZEN; [tracker:234-258](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R10-CONTINUED-OPERATION — proof without StakeHub | 259 | R10 | R10 | FROZEN; [tracker:259](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R10-COMPUTED-STATUS — publish status | 260 | R10 | R10 | FROZEN; [tracker:260](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| R10-INCIDENT-REGRESSION — review and mapping | 261 | R10 | R10 | FROZEN; [tracker:261](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-CI-MERGE-BLOCK — regressions/vectors/wallet | 265 | Phase 3 | Phase 3 | OPEN; [tracker:265](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-CI-REJECTIONS — policy rejection suite | 266 | Phase 3 | Phase 3 | OPEN; [tracker:266-267](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-NIGHTLY — reports/failure freeze | 268 | Phase 3 | Phase 3 | OPEN; [tracker:268-269](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-TWO-WEEK — green streak | 270 | Phase 3 | Phase 3 | OPEN; [tracker:270](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-MONTHLY-DRILL — snapshot/outage drill | 271 | Phase 3 | Phase 3 | OPEN; [tracker:271-272](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-RUNBOOKS — operator report-hash checks | 273 | Phase 3 | Phase 3 | OPEN; [tracker:273](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-STAFFING — single operator accepted risk | 274 | Principal decisions | Phase 3 | DECIDED; [tracker:274-277](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
| P3-STRANGER — external clean-clone transcript | 278 | Phase 3 | Phase 3 | OPEN; [tracker:278-279](A666-RECOVERY-EXECUTION-TRACKER-20260804.md) |
