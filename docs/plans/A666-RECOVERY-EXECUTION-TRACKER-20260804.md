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

## R2 — Regression closure **(gate: PASSED, 12/12)**

Observed-defect regressions (defect -> test, fix committed):

- [x] Defect 7: sub-minimum redemption order — **AR-09**
  `pftl_uniswap_v2_primary_redeem_enforces_production_shaped_policy_binding_ar09`
  (`81bacfc`, 1 matched, 7.616s). Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar09-ar11-tests.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`.
- [x] Defect 8: private-primary archive replay rejection — **AR-10**
  `ar10_private_primary_*_replay_matches_live_execution_path` plus the
  exact historical allowlist (`f059bdb`, three 1-test passes, 6.980-7.950s
  wall-clock; 262/262 first-passing suite green). Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar09-ar11-tests.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`.
- [x] Defect 9: spread custody dropped from supply inventory — **AR-11**
  `ar11_issued_asset_supply_counts_non_nav_spread_custody`
  (`83ac75d`, 1 matched, 176ms; 178/178 first-passing suite green).
  Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar09-ar11-tests.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`.

Mandatory set still to extract as standalone <2-minute tests:

- [x] AR-01 pfUSDC reserve-account identity/balance fixture contract.
  Evidence: `docs/evidence/a666-public-reserve-product-20260803/regressions/ar01-test.txt`;
  manifest: `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`c207ee93`, 1 matched, 838ms).
- [x] AR-02 production cap/order inequalities as runtime test against live
  route config. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar02-tests.txt`;
  manifest: `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`4d92a935`, 1 matched, 194ms).
- [x] AR-03 quorum-first commit with an intentionally offline ACTIVE
  validator; convergence after authenticated recovery. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar03-test.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`0e532e23`, 1 matched, 4.35s; full node-lib suite 264 passed,
  2 ignored, 0 failed in 2499.29s).
- [x] AR-04 authenticated catch-up accepts only pinned height/tip/root,
  rejects every mismatch. Three independent mutations reject with typed
  `prepared commit identity mismatch`, preserve status/ledger/block log,
  then the exact pin accepts and converges. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar04-test.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`bb85aa4`, 1 matched, 8.311s; independent rerun 8.743s).
- [x] AR-05 active export entitlement blocks route-epoch advancement.
  Evidence: `docs/evidence/a666-public-reserve-product-20260803/regressions/ar05-test.txt`;
  manifest: `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`f4d4115e`, 1 matched, 796ms).
- [x] AR-06 duplicate submission proves typed admission `bad_sequence`
  and typed finalized `duplicate_nav_reserve_packet` rejection, never
  ambiguous success. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar06-test.txt`;
  manifest: `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`3d051a5a`, 1 matched, 3.231s; full execution suite 185/185 in 27.14s).
- [x] AR-07 fail-closed rejections: replay, stale proof, wrong profile,
  wrong overlay, wrong supply, wrong NAV, wrong packet (one test per axis).
  Seven standalone tests assert typed rejection plus full-ledger equality;
  evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar07a-test.txt`
  and `docs/evidence/a666-public-reserve-product-20260803/regressions/ar07b-test.txt`;
  manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (`b51c6efc`, `95bc3c4`; full execution suite 189/189 in 28.07s).
- [x] AR-08 snapshot-import verification mapped to existing
  signed-snapshot/checkpoint vectors in the manifest. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/ar08-tests.txt`;
  manifest: `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
  (8 vectors, 7.592-8.253s each).
- [x] Regression manifest JSON: all nine defect classes and AR-01..11
  map to exact test names, runtimes, first-passing commits, and evidence.
  CI executes the validator and every manifest test on each PR via
  `scripts/check-a666-recovery-regression-manifest --run` in
  `.github/workflows/rust-ci.yml` (`6884268`, `d677e14`, `13ae124`).
  Manifest:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`;
  validator evidence:
  `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest-ci.txt`.

## R3 — Repeatability **(gate: PASSED)**

- [x] Run 4: first complete lifecycle pass in project history (16.7 min,
  `checkpoint-lifecycle-run4-report.json`)
- [x] Runs 5/6/7: three consecutive `ok:true` on exact commit `2eb9427`,
  identical binary, distinct run IDs, 1004/1009/1005s —
  `r3-repeatability-gate.json` (`5bfe466`), machine-checked consistency

## R4 — Browser readiness **(gate: OPEN)**

- **[SPEC-INTERPRETATION] §8 step 3 — subject to Sauron veto.** “Show six
  source identities” is satisfied by cryptographically finalized packets where
  every packet attests `source_count=6`, together with two distinct aggregate
  proof/packet identities. Named provider identities remain withheld by
  deliberate provider-neutral policy. Basis:
  `crates/node/src/tests/nav_reserve_proof_status_tests.rs:7` and
  `crates/node/src/lifecycle_queries.rs:1886-1959`, which expose
  ledger-backed provider-neutral status rather than provider names.

- [ ] **DEFECT-13 (observed; OPEN).** A fresh generated self-custody wallet
  cannot create the pfUSDC/A666 trustlines required for onboarding. RED
  evidence: `docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/v7-add-asset-red.json`
  (commit `9db2f52`). **Scope fence:** holder-signed trustline creation is
  product wallet; pfUSDC auto-authorizes; A666 issuer authorization is
  rehearsal/operator staging via DIRECT certified-asset-ops; no issuer tool/key in wallet.
  **Follow-up:** certified bundle helper cannot express issuer-signed TrustSet;
  direct request is required this campaign, helper support remains follow-up.

- [x] Reload/reconnect/recovery e2e coverage for journey step 9 in
  `wallet-web`: production `PftlPrivatePrimary` recovery path survives
  actual proxy SIGTERM/restart and durable Chromium reload, including the
  permanent redacted-receipt download and recovery-record custody assertions.
  Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/browser/journey-step-9-e2e.txt`
  and
  `docs/evidence/a666-public-reserve-product-20260803/browser/successor-candidate-requalification.txt`
  (successor candidate `39f7fae`, exact step 9 1/1, 8.098s).
- [x] `npm test`, `test:custody-browser`, `test:public-browser`, `build`
  green on the successor candidate. Evidence:
  `docs/evidence/a666-public-reserve-product-20260803/browser/successor-candidate-requalification.txt`
  (`39f7fae`; 233/233, 1/1, 2/2, build 1,805 modules; 26/26 Rust
  manifest-validator tests in 203.988s).
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

- [ ] **[DECISION] [DECIDED] 2026-08-05** key rotation: no credential/key
  rotation; accepted-risk record `docs/plans/A666-DECISION-KEY-ROTATION-20260805.md`
  satisfies the alternative to rotate live operator/signer/publisher keys (principal)
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

> **Standing Sauron constraint:** R10 means decoupling StakeHub as proof/runtime
> authority for the layer1v2 public stack only. StakeHub remains in use and must
> never be deleted, destroyed, decommissioned, uninstalled, defunded, or have
> funds, balances, keys, software, services, or data touched or emptied.
> **Principal directive (2026-08-05):** "Stakehub is a product i dont want it deleted or destroyed -- i just want it decoupled from NAVCoin calculation."
> **Binding interpretation:** StakeHub operates indefinitely as a product; R10
> removes it only from NAVCoin/NAV calculation and reserve-proof authority. R4
> may temporarily stop its user units using the mode-600 restart inventory, but
> teardown must restart the inventoried units and prove the restarted unit count
> matches the inventory. Any StakeHub action beyond unit start/stop requires
> Sauron's explicit sign-off. CI identifier rejection is scoped to L1v2 runtime
> paths, never external StakeHub tooling on the host.
>
> **Official-window minimized-downtime rule:** pre-stage all dependencies; stop
> the StakeHub dashboard as the final act before fire; restart it immediately on
> termination, refusal, failure, or success. Every absence/restart proof records
> stop timestamp, restart timestamp, and computed downtime duration. StakeHub
> remains a live product indefinitely.

- **[DECISION] [DECIDED] 2026-08-05** StakeHub restoration GO executed; evidence
  commit `688b67a`; service state restored 5/5.

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
- [ ] **[DECISION] [DECIDED] 2026-08-05** staffing: single operator plus
  automation is the accepted-risk record `docs/plans/A666-DECISION-STAFFING-20260805.md`;
  it satisfies the alternative to roles assigned (principal). The Phase 3 checkbox
  remains open until its operational work executes.
- [ ] One external stranger completes clean-clone verification; transcript
  archived

---

## Scoreboard

| Gate | R0 | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | R10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| State | ✅ | ✅ | 12/12 ✅ | ✅ | 2/5 | 0/2 | 0/3 | 0/6 | 🔒 | 🔒 | 🔒 |

Next action: restore the six-validator rehearsal environment from the signed
checkpoint, prove StakeHub absence and exact R2 wallet revision, then execute
full browser journey pass #1 once under STOP-no-retry. Pass #2 remains gated
on acceptance of pass #1.
