# Z3 G5 failure and recovery rehearsal — 2026-09-17

**Offline rehearsal passed. Z3 remains OPEN.** G5 boxes 1, 2, 3 and 5 are
supported within the fixture/no-value scope. Box 4 remains open because the
stage limits below are proposals requiring operator confirmation.

The [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md#gate-g5-qualify-failure-and-recovery-before-repetition)
controls the failure, retry and environmental-interruption rules.
Main started at `b8e54ff4` after `git pull --rebase origin main`.
Failure fixtures were committed in `b150b14e`; recovery and request-retention
changes in `9269278b`. The commit adding this record adds the per-scenario
invariant assertions and annotations. Both prior units were pulled, rebased
and pushed before starting the next unit.

## Evidence boundary

All responses, receipts, proof reports, amounts and confirmations in these
tests are synthetic. The tests execute the real 39-command compiler,
checkpoint guards, reserve-driver validation/build functions, attempt markers
and manifest verifier. Checkout/binary availability is stubbed **only inside
the rehearsal tests**; process and socket tripwires prohibit external execution.
The existing main-checkout refusal remains covered by the original tests.
Unsigned driver requests use an empty temporary holder placeholder whose
contents cannot be opened by the rehearsal.

No Task Node, fleet, signing, live transaction, Arc/validator call, protected
release branch or protected checkout was used. The only remote operations
were the requested Git pulls/pushes. Neither wallet credential file was read.
No cryptographic proof, consensus execution, live rollback, selected route
qualification, clean integrated cycle or sustained-window authorization is
claimed.

Sources:

- [Failure fixtures](../../python/tests/fixtures/z3/failure_rehearsal.json) and
  [failure tests](../../python/tests/test_z3_failure_rehearsal.py).
- [Recovery tests](../../python/tests/test_z3_recovery_rehearsal.py).
- [Wrapper](../../python/postfiat_rpc/z3_composition.py),
  [manifest/verifier](../../python/postfiat_rpc/z3_cycle.py), and
  [reserve driver](../../scripts/a666-pfusdc-reserve-demo.py).
- [G2 accounting trace](z3-g2-route-compatibility-20260917.md#g24-pass-reserve-counted-once-settlement-identities-cannot-be-substituted)
  for source escrow, principal, spread and reserve counting.

## Failure scenarios

Each row invokes the wrapper with a valid ordered predecessor checkpoint,
changes the specified synthetic response and proves rejection before dispatch
to any builder or submitter. Driver-owned cases also call the actual builder
and prove no request construction or nonce generation. For bridge-only cases,
the wrapper is the rejection authority: the driver consumes the same synthetic
accounting through its route, balance and source-custody validators; it has no
Arc-proof or Arc-replay validator.

All nine retained attempt manifests are `unclean`, verify as `FAIL`, pause
the campaign and require the consecutive count to become zero **after** root
cause and corrective work are recorded. `unsafe_preflight` records the
plan's stop-condition failure; it does not invent an on-chain rejected receipt.
The separate `test_failure_cannot_pass_cycle_verifier[scenario]` checks nine
negative complete-packet projections; those are independent verifier tests,
not purported continuation of failed cycles.

| Scenario / trigger | Stage | Expected rejection point / plan reason | Invariant verdict | Test name |
| --- | --- | --- | --- | --- |
| Stale NAV: quote not current; old NAV packet in driver | prepare-redemption | Wrapper quote-current check; driver requires fresh NAV/route; proof mismatch | PASS: minted 1,000 A666 and 1,000 reserve remain; entitlement already zero | test_failure_rejected_before_build[stale_nav] |
| Stale Arc proof: expiry below finalized height | ingress-bundle | Wrapper proof-height gate; proof mismatch | PASS: 1,005 uncredited deposit remains in vault; no source mint | test_failure_rejected_before_build[stale_arc_proof] |
| Wrong route: substituted checkpoint/driver route | prepare-subscription | Wrapper identity binding; driver route check; reconciliation mismatch | PASS: 1,005 source atoms remain in wallet; no reservation, issue or reserve delta | test_failure_rejected_before_build[wrong_route] |
| Wrong asset: substituted source-series identity | prepare-subscription | Wrapper identity binding; driver selected-source check; reconciliation mismatch | PASS: exact source wallet and family/series supply unchanged | test_failure_rejected_before_build[wrong_asset] |
| Duplicate deposit: exact retained deposit nonce reused | arc-deposit | Wrapper unused-nonce gate; unsafe preflight | PASS: original uncredited 1,005 deposit remains once; no second wallet debit, gas or vault credit | test_failure_rejected_before_build[duplicate_deposit] |
| Duplicate subscription nonce: prior used nonce with current reservation | subscribe | Wrapper unused-nonce gate; unsafe preflight | PASS: one reservation and 1,005 source escrow remain; no A666 issue, principal or spread movement | test_failure_rejected_before_build[duplicate_subscription_nonce] |
| Active entitlement: 1,000 unused export atoms remain | prepare-redemption | Wrapper entitlement check; driver active-order check; unsafe preflight | PASS: entitlement, issued supply and reserve retained; no redemption | test_failure_rejected_before_build[active_entitlement] |
| Duplicate burn: original burn ID and pending withdrawal retained | prepare-burn | Wrapper pending-egress queue gate, before rebuilding burn; unsafe preflight | PASS: 899 already burned remains one pending withdrawal; no second supply debit | test_failure_rejected_before_build[duplicate_burn] |
| Duplicate Arc release: exact consumed withdrawal nullifier reused | arc-release | Wrapper unused-identity gate; unsafe preflight | PASS: original 899 release remains once; pending/released-unsettled both 899; no second vault debit or gas | test_failure_rejected_before_build[duplicate_arc_release] |

The duplicate release submission is distinct from the mandatory read-only
`release-replay-check` in a clean cycle. An expected read-only replay rejection
does not itself make a clean cycle unclean.

## Recovery scenarios

Synthetic interruption raises `KeyboardInterrupt` after retaining an accepted,
finalized terminal response and applying exactly one modeled transition.
A fresh wrapper invocation compiles the identical command and reuses the exact
request bytes. The retained marker rejects replay; the runner is never called
again. Terminal response, request, output and marker bytes remain unchanged.
The wrapper does not automatically resume the campaign or infer confirmation
from an exit code.

| Recovery point | State retained after one confirmation | Resume result and manifest | Test name |
| --- | --- | --- | --- |
| Arc deposit | Vault +1,005; uncredited +1,005; wallet -1,005 atoms and 17 synthetic gas wei; all other values unchanged | Exact command replay refused; one invocation; unclean / FAIL | test_confirmed_interruption_exact_identity_rejected_as_replay[arc-deposit] |
| Primary subscription | Source escrow -1,005; reservation/order terminal; A666 supply/wallet +1,000; principal/reserve +1,000; spread +5; entitlement 1,000 | Exact request digest and nonce retained; replay refused; unclean / FAIL | test_confirmed_interruption_exact_identity_rejected_as_replay[subscribe] |
| PFTL burn | Wallet/family/series -899; redeemed +899; pending withdrawal +899; source vault unchanged | Original burn request and withdrawal retained; replay refused; unclean / FAIL | test_confirmed_interruption_exact_identity_rejected_as_replay[burn] |
| Arc release | Vault -899; wallet +899 atoms less 23 synthetic gas wei; released-unsettled +899; pending withdrawal still 899 | Same proof-bearing command retained; replay refused; unclean / FAIL | test_confirmed_interruption_exact_identity_rejected_as_replay[arc-release] |
| Preflight interruption, no submission | Entire baseline unchanged; preparation marker retained | NOT_A_CYCLE; count stays 4; no reset; marker cannot be silently overwritten | test_environmental_interruption_depends_on_prior_submission[preflight-False] |
| Ingress capture interrupted after deposit | Original uncredited deposit remains; no further change | Unclean / FAIL; count must reset after correction | test_environmental_interruption_depends_on_prior_submission[ingress-capture-True] |
| Issue/redeem builder restarted | Entire ledger and unsigned request files unchanged | Existing output refused before generating a replacement nonce or request | test_driver_restart_never_replaces_request_identity[issue], [redeem] |
| Changed subscription nonce on restart | Only original modeled subscription remains | Retained marker still refuses dispatch despite updated operation hash | test_changed_request_cannot_bypass_retained_attempt |
| Declared command timeout at deposit or ingress proof | Synthetic state unchanged; possible publication remains conservatively recorded | No automatic retry; unclean / FAIL, pause and reset required | test_declared_command_timeout_pauses_without_retry[arc-deposit], [ingress-proof] |

Additional negative tests reject an environmental exemption when predecessor
submission evidence or an Arc/PFTL attempt marker contradicts it. Attempt
verification exits nonzero for both `FAIL` and `NOT_A_CYCLE`; neither can be
counted as a clean-cycle success by a shell caller.

## Invariants and retained evidence

`assert_ledger_unchanged` compares **all 23 accounting fields** and itemized
deposit, reservation, entitlement, withdrawal and consumed-identity records.
It also invokes the reserve driver's economic snapshot, exact account/asset
balance checks and source principal/spread/escrow delta validator. A runner
tripwire makes any unexpected builder/submission dispatch fail the test.

Independent assertions cover family/series conservation, baseline holders,
source escrow versus reservation contents, entitlement amount/count, withdrawal
amount/nullifier/state, and:

~~~text
source vault = source supply + uncredited deposits
               + pending burned egress - released unsettled
source supply = other holders + wallet + principal + spread + escrow
native supply = other native holders + native wallet
~~~

Every successful synthetic prefix has an explicit delta assertion; retry has
zero additional delta. Mutating any of the six requested invariant categories,
source escrow, Arc gas balance or withdrawal identity makes the audit fail.
Retained before/after states, checkpoint, observed reason, and recovery
marker/terminal response are SHA-256-bound in the attempt manifest.

These are tooling/no-dispatch and fixture-accounting proofs, not a second
implementation of the consensus engine. A stopped live attempt would still
need authenticated readbacks and the existing authorized reconciliation path.
Uncredited deposits, active reservations/entitlements and pending withdrawals
are retained for that path, never erased or manually rolled back here.

## Latency bounds and pause thresholds

**All stage values below are PROPOSED; the operator must confirm them before
they become campaign declarations.** No stage-duration measurement is recorded
in the plan's 2026-09-02 evidence table or the G2 document. Those sources retain
receipt blocks/heights and identities, not elapsed stage timings. No number
below is represented as derived from that bridge run; block differences,
synthetic gas, and Python-test wall time cannot supply those timings.

For each stage, measure elapsed time from its first command to its last required
verified receipt/readback, including preparation and proof work. Pause before
any next command when elapsed time exceeds the threshold, or immediately on
any other stop condition. Do not lengthen a threshold or retry a possibly
published request to keep a cycle clean.

| Cycle stage | Proposed bound | Proposed pause threshold | Basis |
| --- | --- | --- | --- |
| Preflight | 300 s | >300 s | Operator proposal; checkpoint validity itself is independently capped at 300 s by check_checkpoint |
| Deposit | 180 s | >180 s | Operator proposal; no September 2 elapsed measurement in plan/G2 |
| Ingress proof and claim | 7,200 s | >7,200 s | Operator proposal, includes capture/proof and propose/finalize/claim |
| Subscription | 300 s | >300 s | Operator proposal, includes build, reserve, subscribe and readbacks |
| Entitlement release | 180 s | >180 s | Operator proposal, includes accepted release and zero-entitlement reconciliation |
| NAV and route epoch | 7,200 s | >7,200 s | Operator proposal, includes proof, NAV finalization, pause/advance/resume |
| Redemption | 300 s | >300 s | Operator proposal, includes bounded quote, submit and exact-delta reconciliation |
| Burn and egress | 7,200 s | >7,200 s | Operator proposal, includes burn, proof, release, read-only replay and egress settlement |
| Final convergence | 300 s | >300 s | Operator proposal, includes six-validator agreement and complete packet audit |

The wrapper currently enforces one operator-provided
`latency_bound_seconds` **per command**, not these aggregate stage timers.
The timeout tests use a synthetic 45-second parameter and raise immediately;
they prove parameter forwarding, retention and no retry, not elapsed latency.
Aggregate thresholds require operator monitoring and confirmation. Checkpoints
must remain fresh before every command even inside a long proof stage.
G5.4 is deliberately unchecked until the proposed stage limits are confirmed.

A non-environmental rejection before submission still stops the attempt.
Only an environmental interruption before **any** submission preserves the
consecutive count. After possible publication, retain uncertainty, pause,
reconcile exact identity and record root cause/correction before resetting
the count; do not infer that timeout cancelled a transaction.

## Resolution and validation

**No scenario needed a consensus change.** Changes are limited to Python
tooling, fixtures/tests and documentation: explicit replay diagnostics and
request digests in markers; early output-existence checks before driver nonce
generation; and a redaction-safe incomplete-attempt manifest/verifier. The
attempt packet records an operator/test projection of observed evidence; the
wrapper does not automatically author it. It cannot produce a clean-cycle PASS.
No Rust, contract, protocol, state-root or signing implementation was changed.

Focused command:

~~~bash
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=python python3 -m pytest \
  python/tests/test_z3_failure_rehearsal.py \
  python/tests/test_z3_recovery_rehearsal.py \
  python/tests/test_z3_cycle.py python/tests/test_z3_composition.py \
  scripts/test-a666-pfusdc-reserve-demo.py -q
~~~

Final result: **101 passed, 67 subtests passed**: 29 failure/invariant tests,
15 recovery tests, 12 original cycle tests, 20 original composition tests and
25 original driver tests. Unit 1 had 77 passed / 67 subtests; unit 2 had
88 passed / 67 subtests. No focused tests were skipped.

Before each of the three unit commits: `.venv-docs/bin/mkdocs build --strict`,
`PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` and
`scripts/public-secret-scan` passed. The record is versioned in Git;
`docs/review/` remains excluded from the public MkDocs site by existing config.

No Rust, workspace or Orchard/Halo2 suite was run: no affected consensus or
Orchard boundary changed, and this task explicitly requested focused Python
tests only. G1, selected-state G2, G4, G6 and all live authorization requirements
remain open. The remaining G5 action is operator confirmation of stage limits;
this record does not ask to begin the sustained window.
