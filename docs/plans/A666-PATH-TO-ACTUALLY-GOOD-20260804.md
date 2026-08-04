# A666: Path to Actually Good

- **Date:** 2026-08-04
- **Status:** EXECUTION SPECIFICATION
- **Authority:** company principal (sole interim decision and abort authority)
- **Builds on:** `docs/plans/A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`
  (gates R0-R10, which this document does not weaken)
- **Supersedes:** nothing; this adds the destination the recovery gates walk
  toward and the structural changes that keep us there.

## 1. What "actually good" means

Not "the demo worked." Good is a steady state with these properties:

1. **The product is live and boring.** Existing A666 migrated in place to the
   public successor; transparent and private issue/redeem and the Ethereum
   round trip work for real users through the browser wallet, every day,
   without an operator babysitting it.
2. **Anyone can check the money.** A stranger with a laptop can clone
   `postfiatorg/postfiatl1v2`, run one documented command, and independently
   verify the reserves, the proofs, and the supply — no StakeHub, no trust in
   us, no private state.
3. **Failures are cheap.** Any defect anywhere in the lifecycle is caught by
   a test that runs in seconds-to-minutes, produces a machine-readable report
   naming the first failing check, and becomes a permanent regression before
   it is fixed.
4. **No single human is a bus factor for operations** — and no schedule
   pressure can waive a gate, because the gates are enforced by CI and signed
   evidence, not by willpower.
5. **Status language is mechanically honest.** "Ready" is a computed boolean
   from required checks. Nobody can say "shipped" because nobody manually
   enters `ok: true` anywhere.

## 2. Verified current position (evidence-backed, 2026-08-04)

What is actually done, with evidence in
`docs/evidence/a666-public-reserve-product-20260803/`:

- **R0 truth freeze:** live baseline at height 776 captured; asset, supply,
  and proof identities pinned.
- **Proofs:** two six-source aggregate proofs verified for the pinned public
  successor identity; all claims cryptographic; zero attested, zero
  controlled.
- **R1 fast loop:** signed, content-addressed six-validator lifecycle
  checkpoint with a fail-closed importer (signature, schema, allowlist,
  symlink containment, hash and tuple binding) and reports written even on
  failure; seven adversarial import vectors pass in ~1 second.
- **R3 repeatability:** the complete product lifecycle — successor migration,
  transparent and private issue/redeem, Ethereum export/return, epoch
  advance, negative cases, validator outage, catch-up, restart, snapshot,
  rollback, global conservation — **passed four consecutive times**, the last
  three on the exact commit and identical binary (`r3-repeatability-gate.json`).
- **Three real defects found and permanently pinned** (AR-09 order-bound
  policy, AR-10 private-primary archive replay, AR-11 spread custody in
  supply conservation). AR-10 would have corrupted live validator recovery
  after the first real private issuance.
- **Cycle time:** 71-minute blind retries replaced by a 17-minute
  instrumented loop; one-time 69-minute history generation amortized into a
  reusable signed checkpoint; 12-second repack path.

Debt honestly stated: AR-01..08 not yet standalone; browser journey untested
against the candidate; no cold run since the fixes; release, reproduction,
and live gates untouched; all §11 roles vacant; live keys un-rotated.

## 3. Phase 1 — Close the technical gates (est. 2-4 working days)

Strictly ordered. Each item ends with committed evidence.

### 3.1 R2: regression closure (AR-01..08)

Extract each remaining contract from the cold-test monolith into a
standalone test under 2 minutes, in `crates/execution` or `crates/node --lib`
where possible:

| ID | Source of truth today | Target |
|---|---|---|
| AR-01 | pfUSDC fixture shape inside lifecycle body | execution unit test |
| AR-02 | `const` asserts in `atomic_swap_local_six.rs` | already compile-time; add a runtime production-inequality test against live route config |
| AR-03 | quorum-vs-offline-validator section of lifecycle | node lib test with 3-validator harness |
| AR-04 | authenticated catch-up section | node lib test using certified-delta vectors |
| AR-05 | entitlement-blocks-epoch-advance section | execution unit test (route state machine) |
| AR-06 | duplicate-submission section | execution unit test (admission vs finalized rejection) |
| AR-07 | fail-closed proof/profile/overlay/supply/NAV/packet rejections | execution unit tests, one per axis |
| AR-08 | already covered by `lifecycle_checkpoint_tests` + signed-snapshot tests | map in the regression manifest, no new code |

Exit: a regression manifest JSON mapping every defect class (now 9) and
every AR ID to a test name, its runtime, and the commit where it first
passed; wired into CI.

### 3.2 R4: browser readiness

1. Add reload/reconnect/recovery e2e coverage (journey step 9) to
   `wallet-web` — the only journey step with zero coverage today.
2. Run the full §8 browser journey twice against the exact candidate with
   StakeHub stopped, using the six-validator rehearsal environment restored
   from the signed checkpoint. Capture receipts and redacted recordings.

Exit: two browser reports with receipts; custody-boundary checks green
(no server-side signing, no seed egress, pending-state survival).

### 3.3 R5: one cold run, then stop

One genesis-to-tip cold qualification on the frozen commit, using the O1
report wrapper. It may run only after 3.1 and 3.2 are green and must not be
retried blindly: any failure goes back through the fast loop with a new AR
test first. Given four consecutive checkpoint-lifecycle passes, this is
expected to pass; it exists to prove the checkpoint didn't hide a
history-generation defect.

### 3.4 R6-R7: release and clean public reproduction

1. Reproducible release build, hashed and signed; strict CI (fmt, clippy,
   full test suites, artifact policy, secret scan) on the exact revision.
2. Fix the two known reproduction blockers recorded in §2.1 of the recovery
   spec: repository-relative guest-ELF pin and the
   `source_commit`/`candidate_revision` explanation in
   `identity-and-public-value-pins.json`.
3. From a fresh public clone: verify hashes, verify both proofs, launch the
   qualification environment, run the checkpoint lifecycle, run the browser
   journey with generated test credentials, verify receipts. Scripted; the
   script fails on any StakeHub dependency.

Exit: clean-clone transcript and report. **Only after this may a demo or
investor date be scheduled** (recovery spec §10 is unchanged: date only
after R0-R7, dress rehearsal at D-1, automated preflight, no degraded-success
mode — a no-go is reported as a no-go).

## 4. Phase 2 — Live migration under single-authority reality (est. 1-2 days, gated on decisions)

Live mutation stays frozen until every item here has an explicit sign-off
from the principal. Since implementation and operation are currently one
agent, the structural rule is:

> **The implementer proposes; the principal disposes.** Every live step below
> is executed only after the principal has confirmed the preflight report
> hash for that step. No confirmation, no step. This is the two-party control
> that replaces the vacant roles table until real roles are staffed.

Ordered live sequence (recovery spec §14, unchanged in substance):

1. **Key rotation decision first.** The live operator/signer keys were
   provisioned by terminated staff. Recommendation: rotate operator, signer,
   and snapshot-publisher keys before any live mutation, and inventory every
   external credential (Ethereum RPC, bridge relays, host access) the prior
   team held. This is a decision item, not something the implementer does
   unilaterally.
2. Signed recovery snapshot of the live chain; independent verification.
3. Pause A666 admission; deploy the signed release; verify all-six
   convergence.
4. Bind the successor to the existing A666 profile; verify epoch-7 proof on
   all six validators.
5. Canaries: one transparent, one private, one Ethereum round trip — each
   followed by all-six convergence and conservation checks, each individually
   confirmed by the principal.
6. Full live lifecycle; publish R9 evidence.
7. Remove StakeHub proof/runtime authority; prove continued operation;
   publish `stakehub_deprecated=true` computed from checks (R10).

Rollback triggers and safety-over-liveness rules are exactly those of the
recovery spec §14.

## 5. Phase 3 — Make it stay good (the part the last team never got to)

### 5.1 CI as the enforcement mechanism

- Every AR test, the checkpoint import vectors, and `wallet-web` unit/e2e
  suites run on every PR; merge is blocked on failure. No exceptions field
  exists.
- The checkpoint lifecycle runs on a schedule (at minimum nightly) against
  HEAD, publishing its report; two consecutive scheduled failures freeze
  merges until a fix with a new regression lands.
- CI rejects: `prove` invocations in any test path, report emission that is
  success-only, StakeHub identifiers in runtime code, secrets in diffs, and
  manually-set `ok`/`accepted` fields in evidence schemas.

### 5.2 Operational steady state

- **Proof cadence:** epoch proofs produced on a fixed schedule by a dedicated
  proving job (never inline with anything), verified and archived by hash;
  route/profile epoch advancement automated with the entitlement guard
  (AR-05 semantics) enforced on-chain.
- **Monitoring computed from state, not logs:** all-six convergence, global
  supply conservation (AR-11 semantics), proof freshness margin, bridge
  conservation, route capacity. Any unexplained delta pauses the route
  automatically — the same fail-closed posture the lifecycle test proves.
- **Recovery drills:** monthly restore-from-signed-snapshot and
  catch-up-from-outage drills in the rehearsal environment, using the same
  checkpoint machinery, with reports archived. A recovery path that isn't
  exercised is assumed broken.
- **Runbooks:** one page per operator action, each ending in "verify this
  report hash." No undocumented manual steps; an unrehearsed step is a no-go
  by policy.

### 5.3 Staffing to remove the bus factor

Minimum viable, in priority order (contractors acceptable; the roles matter,
not the headcount):

1. **Abort authority / operations owner** other than the implementer —
   today this is the principal; it needs a permanent home.
2. **Protocol reviewer:** someone who reads consensus/execution diffs before
   they merge. The AR-10 class of bug (a stale gate that only fires in
   recovery paths) is exactly what a second pair of eyes exists for.
3. **Wallet/frontend owner** for the browser journey and custody boundary.

Until staffed: the principal-confirms-every-live-step rule of §4 stands, and
scope stays limited to what one implementer plus automation can honestly
operate — which the current architecture is deliberately designed to be.

### 5.4 Cultural rules, mechanically enforced

The last failure was not talent, it was physics: a 71-minute feedback loop,
deadline-driven optimism, and status by assertion. The permanent rules:

- A deadline can move a *date*; it can never waive a *gate*. Dates are
  scheduled after R7, never before.
- Every defect becomes the smallest deterministic test that reproduces it,
  before the fix lands. No blind retries, ever. Two identical symptoms stop
  retries and force root-cause review.
- Status reports name the highest passed gate and the next blocking failure,
  nothing else. "In progress" is a location, not an excuse.
- Wall-clock budgets are diagnostics: if the fast loop degrades past its
  budget, that is itself a defect to fix before feature work continues.

## 6. Timeline and decision points

Honest estimates assuming current velocity and no new defect classes:

| Milestone | Est. calendar | Blocking decision (principal) |
|---|---|---|
| R2 + R4 complete | +2-3 days | none |
| R5 cold pass + R6 release | +1 day | none |
| R7 clean public reproduction | +1 day | none |
| Key rotation executed | +0.5 day | **rotate: yes/no, and scope** |
| R8 preflight + R9 live migration | +1 day | **per-step confirmations** |
| R10 StakeHub deprecated | +0.5 day | final sign-off |
| Demo/investor date scheduling | after R7 | **date selection** |
| Phase 3 steady state (CI, drills, staffing) | rolling, starts now | staffing budget |

Every estimate degrades gracefully: a new defect costs one 17-minute loop
plus a fix plus a regression, not a lost day.

## 7. Definition of done for this document

- R0-R10 green for one exact public release (recovery spec §16 in full);
- live keys rotated or the principal has signed an explicit decision not to;
- CI enforces every rule in §5.1 on every merge;
- the nightly lifecycle run has a two-week green streak;
- one full recovery drill completed from signed snapshot with a passing
  report;
- roles in §5.3 assigned to named humans, or an explicit accepted-risk
  record signed by the principal;
- a stranger has actually performed the clean-clone verification and their
  transcript is archived.

Until all of that: the status line remains "in progress," and this document
is the map of exactly what "done" costs.
