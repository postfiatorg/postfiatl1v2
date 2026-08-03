# A666 Product Recovery and Demo Relaunch Specification

- **Date:** 2026-08-03
- **Status:** EXECUTION SPECIFICATION
- **Incident class:** Critical product-delivery failure
- **Repository:** `postfiatorg/postfiatl1v2`
- **Applies to:** existing A666 asset, PFTL validators, public reserve proofs,
  Ethereum bridge, and browser wallet
- **Supersedes:** the eight-hour schedule, but not its product acceptance
  contract
- **Parent acceptance contract:**
  `docs/plans/A666-PUBLIC-RESERVE-PRODUCT-8-HOUR-EXECUTION-SPEC-20260803.md`

## 0. Plain-language situation summary

Read this section first if you have lost the thread.

**What we had.** A666 had a working proof-of-reserves pipeline. Its reserve
evidence, valuation, aggregation, and proof construction ran inside StakeHub,
an internal operator application. The live chain, the asset, the balances, and
the Ethereum bridge identity all still exist and are unchanged (height 776,
supply intact, six validators converged). Nothing about the live asset was
lost or corrupted.

**What we tried.** Because StakeHub is internal, its proofs are not publicly
verifiable. We committed to deprecating StakeHub as the proof authority
(`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md`):
rebuild the entire reserve-proof pipeline in public code, pin a public
successor proof identity, and migrate the existing live A666 profile to it in
place — same asset, same history, publicly reproducible proofs.

**What went wrong.** The public proof artifacts themselves succeeded: two
six-source aggregate proofs for the successor identity verified independently,
with all claims cryptographic. The failure was the migration qualification.
The only end-to-end check was a roughly one-hour six-validator rehearsal, so
each lifecycle defect (fixture config, route caps, offline-validator dialing,
catch-up, epoch ordering, rejection semantics) surfaced one at a time at the
end of a long run. Six retries burned the schedule, the rehearsal never
passed, and the live migration was therefore never authorized. A demo was
planned ahead of an investor meeting with an explicit, communicated deadline;
the product was not ready, so no demo took place. The team responsible was
terminated on 2026-08-03. This document's incident language ("the demo was
missed") means exactly that: a planned demo did not happen because the
product had not passed its acceptance contract.

**Where that leaves us.** Live A666 still runs on the historical
StakeHub-lineage proof profile. The public successor is proven but not live.
StakeHub is still active. The recovery is not a rewrite: it is to finish the
same migration with a test system that finds defects in minutes instead of
hours, and to gate any new demo date behind evidence rather than an ETA.

## 1. Purpose

The A666 demo was missed. The recovery objective is not to produce another
presentation or a narrower fallback. It is to make the real A666 product work,
prove that it works, and prevent another demo from being scheduled before the
same release has passed the same user journey.

The required product remains:

```text
six public reserve sources + finalized pfUSDC overlay
  -> independently verified aggregate proof
  -> existing A666 profile migrated in place
  -> six-validator enforcement
  -> transparent and private issue/redeem
  -> Ethereum export and return
  -> browser-wallet completion and recovery
  -> clean public reproduction from postfiatorg/postfiatl1v2
```

StakeHub is not an acceptable proof authority, validator dependency, wallet
dependency, release dependency, or public reproduction dependency. There is no
legacy-demo fallback.

This document is deliberately an operational recovery contract. It replaces
deadline-driven optimism with short feedback loops, explicit evidence, named
decision roles, and gates that cannot be waived.

## 2. Incident truth and present boundary

The following statements are the recovery baseline as of this document:

- the existing A666 asset ID is
  `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c`;
- two fresh six-source aggregate proof artifacts have completed independent
  verification for the pinned public successor identity;
- all quantity and valuation claims in those proofs are cryptographic, with
  zero attested-value and zero controlled-value claims;
- the live chain baseline remains at height 776 with state root
  `10dfb17b640a69749ca4b00d66b9e0141fa33644df6bcd8f7008f85f4501a42424681edaf25a3fd0111cc55492b256f9`;
- the live A666 profile has not been migrated to the successor;
- the live product lifecycle has not passed;
- StakeHub remains active and has not been deprecated;
- the browser-wallet journey has not passed against the successor release;
- clean public reproduction has not passed;
- controlled qualification retry 6 terminated 2026-08-03T19:27:42Z after 71
  minutes, failing with mempool admission rejection
  `pftl_uniswap_redemption_policy_mismatch` — a seventh defect class not among
  the six in section 3; the pre-O1 binary wrote no report (confirming the
  success-only emission defect); the harness state (1.7G) and journal are
  frozen at `~/.pft/public-reserve-qualification/20260803-controlled-retry6-frozen`
  and recorded in
  `docs/evidence/a666-public-reserve-product-20260803/qualification/retry-6-outcome.json`;
  the controlled gate remains open, not passed, and no further cold run may
  start before AR-09 reproduces this defect at a fast layer;
- prior retries left no frozen artifacts (empty retry working directory,
  failed transient units only), the historical runner
  `continue-a666-public-proof-qualification.sh` chained throttled sequential
  proving and never set the report variable (now disabled), and the cold test
  emits its report only after all assertions pass, so a failing run produces
  no report at all — recorded in
  `docs/evidence/a666-public-reserve-product-20260803/qualification/retry-6-evidence-integrity-failure.json`
  and fixed under objective O1;
- the candidate source and evidence are in a dirty, partly staged worktree and
  are not a clean public release.

No process exit code, compilation success, partial receipt, screenshot, or
operator statement overrides a missing acceptance report.

### 2.1 Codebase audit deltas (2026-08-03)

An audit of the worktree against this specification confirmed the following
concrete gaps. Each is an input to the locked objectives, not a new
requirement:

- `a666_public_successor_lifecycle_from_signed_checkpoint`, the checkpoint
  manifest schema, and the signed-checkpoint importer do not exist yet
  (gate R1 is fully open);
- AR-01 through AR-08 do not exist as standalone fast tests; AR-02's
  inequalities exist only as `const` assertions inside the 90-minute cold test
  in `crates/node/tests/atomic_swap_local_six.rs` (gate R2 is fully open);
- the browser e2e suites contain no reload/reconnect/recovery coverage for
  journey step 9 (gate R4 gap); the minimum `wallet-web` npm checks do exist;
- `qualification/identity-and-public-value-pins.json` pins the guest ELF at an
  absolute private operator path and records `source_commit` different from
  `candidate_revision`; both must be made repository-relative or explicitly
  explained before clean reproduction (gate R7 blocker);
- the cold test's `A666_CIRCULATING_SUPPLY` constant (31_597_197_455) and the
  live `outstanding_supply_atoms` (99_000_000) must be reconciled and the
  distinction documented before live migration (gate R9 precondition);
- in-crate StakeHub coupling is reduced to a single test-fixture string
  (`fee_replay_execution_tests.rs`); guard scripts already reject premature
  deprecation claims; runtime deprecation (R10) is correctly unstarted.

## 3. What failed in the execution system

The primary failure was not one isolated code defect. The workflow used a
roughly one-hour genesis-to-height-784 six-validator test as its main debugging
loop. Product-lifecycle defects were therefore discovered near the end of a
long run, one at a time.

The controlled retries exposed at least these defect classes:

1. a pfUSDC fixture used absent or incorrect reserve-account configuration;
2. route caps and order bounds differed from production constraints;
3. the outage test continued dialing a deliberately offline validator;
4. a restarted validator lacked authenticated historical catch-up;
5. route-epoch advancement was attempted while an export entitlement was
   active;
6. a duplicate-operation test confused admission rejection with a finalized
   rejection block;
7. (retry 6, 2026-08-03) a redemption submit was rejected at admission with
   `pftl_uniswap_redemption_policy_mismatch` — redemption expiry, policy
   binding, amount, or nonce invalid — 71 minutes into the run.

These defects matter, but the systemic causes are more important:

- no short test started immediately before the successor lifecycle;
- lifecycle ordering was encoded mainly as imperative test steps rather than
  as an explicit state-machine contract;
- rejection behavior did not distinguish mempool/admission failure from a
  finalized state transition;
- restart recovery was integrated before its independent contract was proven;
- the full cold-chain run was restarted before every observed failure had a
  fast, permanent regression test;
- demo readiness was treated as an ETA rather than a release gate.

## 4. Terminology and truth rules

### 4.1 Controlled qualification chain

A controlled qualification chain is an isolated six-validator rehearsal under
our operational control. It may reproduce the live chain's configuration,
history, asset identities, proofs, policies, and state. It is useful for fault
injection and destructive testing.

It is not the live chain, not public user evidence, and not a product demo.
Passing it authorizes consideration of a live rollout; it does not establish
that the live product works.

### 4.2 Demo

A demo is a browser user completing the accepted product lifecycle against the
exact release candidate in the declared demo environment. An operator CLI,
direct ledger mutation, synthetic UI success state, pre-recorded video, or
controlled-test receipt is not a demo.

### 4.3 Passed

A gate is passed only when its machine-readable report exists, validates its
schema, identifies the exact commit and binary hashes, contains every required
check, and records `ok: true`. Absence, parse failure, an unknown check, or a
hash mismatch is failure.

### 4.4 Shipped

`shipped`, `ready`, `functional`, `StakeHub deprecated`, and equivalent claims
are forbidden until all gates in section 9 pass. Status language before that
point must identify the highest completed gate and the next blocking failure.

## 5. Non-negotiable constraints

- Preserve the existing A666 asset, balances, supply, receipts, export
  entitlements, and Ethereum wA666 identity.
- Do not create an A666-like replacement asset.
- Do not introduce a manually asserted NAV, reserve value, or packet hash.
- Do not accept attested or controlled reserve values as proof.
- Do not use StakeHub to define balances, liabilities, prices, haircuts, NAV,
  freshness, or proof acceptance.
- Do not require StakeHub from validator, proof verifier, wallet, proxy,
  bridge, release, or public reproduction code.
- Do not mutate live A666 until the controlled, release, rollback, and clean
  reproduction gates authorize it.
- Prefer safety over liveness on state-root divergence, proof-identity
  mismatch, supply inconsistency, or bridge-conservation failure: halt the
  affected operation, preserve evidence, and keep the route paused.
- Never copy secrets, key files, seeds, tokens, or private wallet material into
  the repository or evidence bundle.
- Do not delete or bulk-clean the existing dirty worktree. Stage only named,
  reviewed files; never use `git add .`.
- A deadline cannot waive a gate.

## 6. Recovery architecture

The recovery uses four test layers. A slower layer may start only after all
faster layers are green.

```text
targeted regression (under 2 minutes)
  -> signed-checkpoint lifecycle (target under 15 minutes)
  -> full six-validator cold qualification (target under 90 minutes)
  -> clean public browser reproduction
  -> guarded live migration and live lifecycle
```

### 6.1 Layer A: targeted regressions

Every defect observed in any longer run must first become the smallest
deterministic test that reproduces it. The fix is not complete until that test
fails on the pre-fix revision and passes on the candidate revision.

The initial mandatory regression set is:

| ID | Contract |
|---|---|
| AR-01 | pfUSDC reserve-account identity and balance source match the production-shaped fixture |
| AR-02 | route supply cap, packet notional cap, policy capacity, minimum order, and maximum order satisfy the production inequalities |
| AR-03 | quorum-first commit does not require an intentionally offline validator, while later propagation converges after recovery |
| AR-04 | authenticated catch-up accepts only the pinned expected height, tip, and state root and rejects every mismatch |
| AR-05 | an active export entitlement prevents route-epoch advancement until destination consumption and return complete |
| AR-06 | duplicate submission accepts either typed admission rejection with no new block or typed finalized rejection with unchanged state root, never ambiguous success |
| AR-07 | replay, stale proof, wrong profile, wrong overlay, wrong supply, wrong NAV, and wrong packet all fail closed |
| AR-08 | snapshot import verifies publisher signature, content hash, chain ID, height, tip, and state root before startup |
| AR-09 | redemption submit against the production-shaped route policy admits a valid expiry/binding/amount/nonce tuple and rejects each invalid variant with `pftl_uniswap_redemption_policy_mismatch` (reproduces the retry-6 failure) |

Each regression records a deterministic seed or canonical input vector. A
failed seed becomes permanent corpus data.

### 6.2 Layer B: signed-checkpoint lifecycle loop

Implement a new ignored integration test named
`a666_public_successor_lifecycle_from_signed_checkpoint`. It must begin from a
content-addressed checkpoint at the last state immediately before successor
migration, rather than regenerating 784 blocks on every run.

The checkpoint manifest must contain:

- schema version;
- source commit and dirty-state declaration;
- release binary SHA-256;
- chain ID and genesis hash;
- height, tip hash, and state root;
- archive/block-history hash;
- validator topology hash;
- A666 asset, successor profile, source manifest, valuation policy, guest ELF,
  and verification-key identities;
- epoch-7 and epoch-8 proof/public-value hashes;
- signer public key and detached signature;
- creation command and timestamp;
- explicit declaration that private material is absent.

Checkpoint import must:

1. verify the signature against a configured trusted public key;
2. verify all content hashes before extracting or loading state;
3. validate chain ID, genesis hash, height, tip, and state root;
4. replay the bounded tail needed to validate archive continuity;
5. reject extra files, symlinks escaping the checkpoint root, schema drift,
   missing fields, and any hash mismatch;
6. start each validator from its own verified state directory;
7. write an import report before the lifecycle begins.

This accelerated checkpoint is qualification infrastructure. It cannot
replace the final genesis-to-tip cold run or clean public reproduction.

The short loop must execute, in order:

1. all six validators agree on pre-migration height, tip, and state root;
2. epoch-7 proof and all six source records verify;
3. the existing A666 profile migrates in place to the pinned successor;
4. transparent issuance finalizes and conservation holds;
5. transparent redemption finalizes and conservation holds;
6. private issuance finalizes without exposing wallet secrets;
7. private redemption finalizes without exposing wallet secrets;
8. A666 export debits native state and creates the exact entitlement;
9. Ethereum consumes the export and finality is verified;
10. return burns/releases the Ethereum representation and restores native
    A666 exactly once;
11. only after the entitlement is closed, epoch-8 advances the route/profile;
12. all negative cases in AR-06 and AR-07 fail closed;
13. one validator is stopped, quorum continues, and the validator catches up
    using authenticated certified deltas;
14. all six converge on identical height, tip, and state root;
15. snapshot/export, cold restart, replay, and rollback rehearsal pass;
16. reserve, supply, wallet, vault, and bridge conservation all equal zero
    unexplained delta.

The loop must emit a single report even on failure. The report identifies the
first failed step, expected and observed public values, log locations, exact
seed, and whether any downstream steps were skipped.

### 6.3 Layer C: full six-validator cold qualification

The existing test
`a666_public_successor_proof_migrates_and_survives_six_validator_restart`
remains the cold qualification. It must start from clean validator directories
and reproduce the chain through the lifecycle without importing the
acceleration checkpoint.

It may run only when:

- all targeted regressions pass;
- the signed-checkpoint lifecycle passes three consecutive times from clean
  test directories;
- browser E2E against the same candidate passes twice;
- formatting, strict clippy, release build, and public artifact policy pass;
- no input, verifier, fixture, policy, or lifecycle code changed after those
  passes.

Any cold-run failure invalidates the corresponding fast-layer coverage. The
failure must be reproduced and fixed in Layer A or B before another cold run.
Blind cold-run retries are prohibited.

### 6.4 Layer D: clean public reproduction

From a new clone of `postfiatorg/postfiatl1v2` at the exact release commit, a
public operator with no StakeHub checkout, token, path, process, or private
operator state must be able to:

1. verify the release source and binary hashes;
2. reproduce or independently verify both aggregate proofs;
3. validate all six source records and the pfUSDC overlay;
4. launch the declared six-validator qualification environment;
5. run the checkpoint and cold qualification reports;
6. launch the browser wallet;
7. complete the public reproduction lifecycle using documented non-production
   test credentials generated for that run;
8. verify all receipts and conservation reports.

The reproduction script must fail if it discovers a StakeHub import, endpoint,
environment variable, token, filesystem path, process dependency, or accepted
artifact schema.

## 7. Explicit lifecycle state machine

The implementation and tests must use the following externally visible state
model. State transitions are deterministic functions of finalized state and
canonical transaction input; wall-clock time is not a transition input except
where a consensus-defined finalized timestamp is explicitly specified.

| State | Allowed next state | Required invariant |
|---|---|---|
| `LegacyProfilePaused` | `SuccessorBound` or rollback | same A666 asset and supply; route paused |
| `SuccessorBound` | `RouteOpenEpoch7` or rollback | pinned profile, manifest, policy, ELF, verifier, proof |
| `RouteOpenEpoch7` | `Issued` or `Redeemed` or `ExportDebited` | proof fresh; order and supply caps hold |
| `Issued` | `RouteOpenEpoch7` | exact reserve and supply deltas |
| `Redeemed` | `RouteOpenEpoch7` | exact burn and reserve-release deltas |
| `ExportDebited` | `DestinationConsumed` | one live entitlement; native amount unavailable |
| `DestinationConsumed` | `Returned` | Ethereum finality proven; no duplicate consume |
| `Returned` | `RouteOpenEpoch7` | representation removed; native amount restored once |
| `RouteOpenEpoch7` with no entitlement | `RouteOpenEpoch8` | epoch-8 proof and monotonicity verified |
| any accepted state | same state on admission rejection | height, tip, root, balances, supply, and receipts unchanged |
| any accepted state | next height on finalized rejection | typed rejection receipt; state root and conserved values unchanged |

Private and transparent ownership modes share the same reserve, supply, NAV,
route, entitlement, and conservation rules. Privacy changes disclosed fields,
not economic authorization.

## 8. Browser product and demo contract

The browser wallet is the product boundary. It must provide one documented
entry point and no operator-only completion step.

The accepted browser journey is:

1. open the release wallet and show its build/revision identity;
2. connect or create a browser-controlled wallet;
3. show six source identities, source freshness/finality, both aggregate proof
   identities, and the verified A666 NAV;
4. show the existing A666 and pfUSDC balances;
5. complete transparent issue and redeem;
6. complete private issue and redeem with signing inside the wallet boundary;
7. export A666 to Ethereum and show both-chain finality;
8. return it to native A666 and show exact conservation;
9. reload the browser, reconnect, recover pending/finalized operations, and
   obtain the same final balances and receipt identities;
10. expose downloadable public receipts without secrets.

The browser tests must prove:

- no user spend is signed by the proxy or server;
- no seed or owner key leaves the browser boundary;
- the wallet independently validates asset IDs, proof/profile identities,
  route terms, amounts, expiry/finality, receipts, and replay status;
- every pending state survives proxy restart and browser reload;
- the UI renders failure for stale, mismatched, replayed, or rejected data;
- StakeHub is stopped or absent during the accepted run;
- no manual database edit, CLI transaction, or hidden operator endpoint is
  used to make the browser flow complete.

`npm test`, `npm run test:custody-browser`,
`npm run test:public-browser`, and `npm run build` in `wallet-web/` are minimum
checks, not substitutes for the real browser lifecycle.

## 9. Recovery gates

No gate may be waived. A downstream gate is invalid if its source revision,
binary, proof input, configuration, or fixture changes.

| Gate | Requirement | Exit evidence |
|---|---|---|
| R0 Truth freeze | capture live state, services, repository, proofs, and secrets-exclusion inventory | signed baseline manifest |
| R1 Fast loop | signed checkpoint importer and lifecycle test exist and fail closed | import vectors plus one complete report |
| R2 Regression closure | AR-01 through AR-09 pass and each observed retry defect has a permanent test | regression manifest |
| R3 Repeatability | checkpoint lifecycle passes three consecutive clean runs on the exact commit | three reports with distinct run IDs and identical terminal invariants |
| R4 Browser readiness | exact browser journey passes twice, including reload/recovery, with StakeHub absent | two browser reports, receipts, and redacted captures |
| R5 Cold qualification | genesis-to-tip six-validator test passes once with outage, catch-up, restart, replay, and rollback | authoritative controlled report |
| R6 Release integrity | exact release is reproducibly built, hashed, signed, archived, and passes strict CI/security policy | release and CI manifests |
| R7 Clean public reproduction | a fresh public clone completes proof verification, qualification, and browser journey | clean-clone report and command transcript |
| R8 Live preflight | signed recovery snapshot, live convergence, paused route, signer separation, and rollback rehearsal pass | preflight decision record |
| R9 Live migration | existing A666 migrates in place and live transparent/private plus Ethereum round trips pass | live receipts and conservation report |
| R10 StakeHub deprecation | public stack operates with StakeHub proof/runtime dependencies absent and readiness is published | `stakehub_deprecated=true` record |

## 10. Demo scheduling and go/no-go

A demo date may be announced only after R0 through R7 pass on the exact release
candidate. The demo release is then frozen except for a declared emergency
change, which invalidates all affected gates.

Required readiness sequence:

- **D-2 or earlier:** release freeze; artifact hashes and operator runbook
  published internally;
- **D-1:** complete dress rehearsal from a reset demo environment, using the
  exact browser journey and no manual repair;
- **T-2h:** automated preflight verifies services, six-validator convergence,
  proof freshness, route state, Ethereum connectivity/finality, wallet build,
  balances, capacity, and evidence storage;
- **T-30m:** a bounded smoke check uses the same public interfaces without
  consuming or corrupting the prepared demo state;
- **T-0:** the abort authority confirms the preflight report hash before the
  operator begins.

Automatic no-go conditions include:

- any unknown or failed preflight check;
- proof freshness insufficient for the scheduled session plus worst-case
  duration;
- any validator disagreement or peer count below the required quorum;
- release, config, proof, or wallet hash drift;
- insufficient balances, route capacity, gas, or bridge finality budget;
- StakeHub required or running in a role prohibited by this specification;
- an unrehearsed manual step;
- missing rollback operator, backup demo operator, or evidence recorder.

There is no degraded-success mode. A no-go is reported as a no-go, not repaired
live in front of users.

## 11. Roles and accountability

**Status as of 2026-08-03:** every previously assigned role is vacant; the
team responsible for the missed delivery was terminated. The company
principal is the sole interim decision authority. Consequences of the
vacancy, binding until roles are reassigned:

- no live mutation of any kind (R8+ remains frozen regardless of gate state);
- the abort authority must be independent of whoever makes the final code or
  configuration change, so recovery execution cannot self-authorize a live
  migration under any circumstances;
- rotation of live operator and signer key material is an open decision for
  the principal: people with historical access to deployed key paths have
  left, and the constrained-signer daemon still runs under the prior team's
  configuration.

Names must be assigned in the execution manifest before R3.

| Role | Accountable for | Cannot self-approve |
|---|---|---|
| Incident lead | scope, priority, status truth, blocker escalation | protocol correctness |
| Protocol lead | state machine, validator determinism, migration invariants | final demo go decision alone |
| Proof lead | six sources, aggregate proofs, verifier identities, reproduction | wallet completion |
| Wallet lead | custody boundary, browser lifecycle, recovery UX | validator qualification |
| Release lead | exact revision, reproducible binary, CI, signed artifacts | own failed checks |
| Demo operator | rehearsed user journey and evidence capture | release acceptance |
| Abort authority | independent go/no-go and immediate stop | remediation implementation |

R8 requires signatures from the protocol lead, release lead, demo operator,
and abort authority. A person may fill more than one implementation role, but
the abort authority must be independent of the person making the final code or
configuration change.

Accountability is evidence-based. The incident review records decisions,
inputs, missing controls, and corrective actions; it does not speculate about
individual motives.

## 12. Retry policy and time budgets

### 12.0 Proving is not part of the qualification loop

The dominant wall-clock cost of prior retries was not NAV verification, which
takes seconds, and not the lifecycle itself. It was SP1 aggregate-proof
*generation*, rerun inside the retry pipeline under severe self-imposed
throttling: CPU prover, `RAYON_NUM_THREADS=6`, and every SP1 worker and buffer
knob forced to 1 on a 32-core, 122 GB machine, with epoch 7 and epoch 8 proved
sequentially before the qualification test even started. That turned a
fast-verifying artifact into a multi-hour prefix on every retry.

The following rules are therefore binding:

1. The two archived aggregate proofs are pinned, content-addressed **inputs**
   to qualification. Every layer consumes them by hash from
   `docs/evidence/a666-public-reserve-product-20260803/qualification/` and
   runs only `verify` (seconds). No qualification, regression, checkpoint,
   cold, browser, or reproduction run may invoke `prove`.
2. Re-proving is required only when the witness, guest ELF, verification key,
   source manifest, or valuation policy identity changes — and any such change
   already invalidates the affected gates, so proving is by definition an
   out-of-band artifact-production job, never a test step.
3. If re-proving is ever needed, it runs as its own job with resources sized
   to the machine: both epochs in parallel, worker and thread limits raised to
   measured-safe values, and throttling applied only in response to an
   observed memory failure recorded in evidence — not copied forward from an
   old bounded-memory incident.
4. A pipeline that blocks a lifecycle test on a proving service
   (`wait_for_service` on a prover unit) is a defect. Proof availability is a
   precondition checked by hash, not a stage.

With archived proofs reused and Layers A/B in place, the full recovery
critical path (regressions, checkpoint lifecycle x3, browser x2, one cold run,
clean reproduction) is bounded by roughly half a working day of compute, not
multiple days.

Targets are diagnostic controls, not acceptance shortcuts:

- targeted regression: under 2 minutes;
- checkpoint import validation: under 2 minutes;
- complete checkpoint lifecycle: under 15 minutes;
- browser lifecycle: under 10 minutes excluding external finalized-chain
  confirmation explicitly recorded by the test;
- cold six-validator qualification: under 90 minutes.

On failure:

1. stop the affected layer and preserve the exact inputs, logs, seed, state,
   and first failing assertion;
2. classify the failure as proof/source, deterministic state transition,
   migration ordering, validator quorum, catch-up/restart, bridge/finality,
   browser/custody, release integrity, or evidence integrity;
3. reproduce it at the fastest possible layer;
4. add the permanent regression before applying or accepting the fix;
5. rerun the changed layer and every invalidated downstream layer;
6. update the defect-to-test traceability table;
7. resume only from the last still-valid gate.

If the same symptom recurs twice, stop retries and perform a root-cause review.
If a new failure appears late in three consecutive cold runs, freeze cold runs
and review the lifecycle state machine and checkpoint coverage. Service-manager
success without the required report is an evidence-integrity defect, not a
pass.

## 13. Evidence and release manifest

Every report must be JSON with a versioned schema and include:

- run ID and UTC start/end times;
- source revision and explicit clean/dirty status;
- Rust toolchain, dependency lock hash, build command, binary hash, and feature
  set;
- chain, genesis, height, tip, and state-root identities;
- validator topology and per-validator terminal tuple;
- A666, profile, manifest, valuation policy, ELF, verifier, proof, overlay, and
  route identities;
- ordered checks with expected, observed, outcome, and evidence paths;
- transaction, receipt, Ethereum, and finality identities;
- supply/reserve/wallet/vault/bridge conservation values;
- outage, catch-up, restart, replay, snapshot, and rollback results;
- browser build and browser version;
- StakeHub absence/dependency scan result;
- secret scan result;
- first failure and skipped downstream checks;
- overall `ok` boolean computed from required checks, never manually entered.

Reports and manifests are hashed. Release manifests and recovery snapshots are
signed. Evidence paths must be repository-relative where public and clearly
identified as redacted where operational details cannot be published. No
secret value or sensitive key path is public evidence.

## 14. Live migration and rollback

Live mutation remains prohibited until R8 passes.

The live sequence is:

1. record all-six live height, tip, state root, profile, route, packet, supply,
   balances, vault, bridge, and service state;
2. pause A666 issue, redeem, export, and return admission;
3. create and independently verify a signed recovery snapshot;
4. deploy the exact signed release with one active signer per validator and
   double-sign prevention intact;
5. verify all-six convergence before any governed A666 mutation;
6. bind the pinned public successor to the existing A666 profile;
7. verify epoch-7 proof and route invariants on all six validators;
8. reopen bounded admission;
9. execute one transparent, one private, and one Ethereum export/return
   canary, each followed by all-six convergence and conservation checks;
10. advance only when each canary report passes;
11. complete the full live lifecycle and publish R9 evidence;
12. remove StakeHub proof/runtime authority, prove continued operation, and
   publish R10.

Immediately pause and roll back from the last signed checkpoint on state-root
divergence, proof-identity mismatch, unexplained conservation delta, duplicate
entitlement/return, loss of signer exclusivity, or inability to recover a
validator deterministically. Preserve the failed state for forensics; do not
repair a potentially corrupted database in place.

## 15. Immediate execution order

The next work is strictly ordered:

1. let the running retry 6 terminate without changing its inputs, then freeze
   and hash its report and journal output; the historical proving-chained
   runner is already disabled and the evidence-integrity observation is
   recorded in section 2 (section 12.0 binds all future runs);
2. extract the pre-successor state into a signed, content-addressed checkpoint;
3. implement and validate the checkpoint importer with positive and adversarial
   vectors;
4. split the lifecycle from the cold history-generation path;
5. implement AR-01 through AR-09 and map each prior retry failure to a test;
6. run the checkpoint lifecycle until it passes three consecutive clean runs;
7. complete the browser flow twice with StakeHub absent;
8. run one full cold six-validator qualification on the unchanged commit;
9. create and sign the release, run exact-revision CI, and perform clean public
   reproduction;
10. conduct the live preflight and guarded migration;
11. deprecate StakeHub only after the live product and public evidence pass.

No further hour-long controlled retry should start before steps 1 through 5
are complete.

### 15.1 Locked work objectives (2026-08-03)

The execution order above is decomposed into locked objectives. Each maps to
audit findings in section 2.1 and to recovery gates in section 9. Objectives
are worked strictly in order; an objective is complete only when its exit
evidence exists.

| # | Objective | Concrete work | Gate | Exit evidence |
|---|---|---|---|---|
| O1 | Evidence-integrity closure | Record retry-6 termination-without-report; disable `continue-a666-public-proof-qualification.sh`; make the cold test in `crates/node/tests/atomic_swap_local_six.rs` emit a section-13 report on both success and failure paths with computed `ok`, run ID, revision, and binary hash; require `POSTFIAT_A666_CONTROLLED_REPORT_FILE` in every runner | R0/R2 | failure record plus a report-on-failure unit test |
| O2 | Signed checkpoint and importer | Extract pre-successor state (h776-equivalent controlled state) into a content-addressed, signed checkpoint per section 6.2; implement the importer with positive and adversarial vectors (bad signature, hash mismatch, symlink escape, schema drift, extra files) | R1 | import vectors plus one import report |
| O3 | Fast regressions AR-01..AR-09 | Implement each (now AR-01..AR-09 after the retry-6 defect) as a standalone deterministic test under 2 minutes; extract AR-02 from the cold-test `const` asserts; map all six retry defect classes (section 3) to permanent tests with recorded seeds | R2 | regression manifest with defect-to-test traceability |
| O4 | Checkpoint lifecycle loop | Implement `a666_public_successor_lifecycle_from_signed_checkpoint` executing section 6.2 steps 1-16, consuming archived proofs by hash (section 12.0), emitting a single report even on failure; run until three consecutive clean passes | R3 | three reports, distinct run IDs, identical terminal invariants |
| O5 | Browser readiness | Add reload/reconnect/recovery e2e coverage (journey step 9) to `wallet-web`; run the full section-8 journey twice against the candidate with StakeHub absent | R4 | two browser reports plus receipts |
| O6 | Evidence hygiene | Make the guest-ELF pin in `identity-and-public-value-pins.json` repository-relative or archived-by-hash; explain or fix `source_commit` vs `candidate_revision`; reconcile and document `A666_CIRCULATING_SUPPLY` (31_597_197_455) vs live `outstanding_supply_atoms` (99_000_000) | R6/R7/R9 | corrected pins file plus reconciliation note |
| O7 | Cold run, release, reproduction, live | One cold qualification on the frozen commit, then signed release with exact-revision CI, clean public reproduction, live preflight, guarded migration, and StakeHub deprecation per sections 6.3, 6.4, and 14 | R5-R10 | gate evidence per section 9 |

Rules binding all objectives:

- no `prove` invocation inside any objective's test path (section 12.0);
- no cold run before O1 through O4 are complete;
- no demo date before R0 through R7 pass (section 10);
- a change to proof inputs, fixtures, or lifecycle code re-opens every
  downstream objective.

## 16. Definition of done

Recovery is complete only when all of the following are true:

- R0 through R10 are green for one exact public release;
- the existing A666 asset migrated in place with continuous supply, balances,
  receipts, entitlements, and Ethereum identity;
- six public source families and two aggregate proofs are independently
  verifiable;
- all six validators deterministically enforce the successor and converge
  through outage, catch-up, restart, snapshot, replay, and rollback;
- transparent/private issue and redeem plus Ethereum export/return pass live;
- the browser performs and recovers the complete journey without hidden
  operator completion;
- a clean public checkout reproduces the declared product evidence;
- StakeHub is absent as proof authority and runtime dependency;
- the readiness record sets `stakehub_deprecated=true` from computed checks;
- the incident review and permanent regression mapping are published.

Until then, the only accurate status is:

> A666 recovery is in progress. The live product has not passed the acceptance
> contract, and StakeHub has not been deprecated.
