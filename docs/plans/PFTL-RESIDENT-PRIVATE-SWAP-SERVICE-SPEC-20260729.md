# PFTL Resident Private Swap Service Specification

**Date:** 2026-07-29
**Priority:** P1 (first product-latency workstream after the optimization
campaign)
**Status:** amended implementation and qualification specification (v4)
**Parent research:** `CHAIN-OPTIMIZATION-STACKED-RESEARCH-20260729.md`
(S1.1, S1.3, S3.1, S6.1), `NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md`
(Tier 0.1, Tier 2.1)
**Measured baseline:** pass-6 evidence,
`docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass6/`
**Run precedents:** `transport-peer-certified-private-egress-loop`
(426ms hot certified rounds), pass-6 resident consensus workers, pass-5
one-round relay failure (mempool admission lesson)

`MUST`, `MUST NOT`, `SHOULD`, and `REQUIRED` are normative.

## Amendment note

This v4 amendment preserves the resident-prover, persistent-session, and
single-batch architecture. It tightens the product boundary and resolves
security and implementation ambiguities that would otherwise block a safe
implementation:

- the latency SLO covers execution after pfUSDC is already final on PFTL,
  not Ethereum ingress or finality;
- the first implementation is explicitly a controlled-wallet service, not
  a non-custodial public swap API;
- every spend requires an authenticated, signed intent bound to the quote
  and exact execution limits;
- proving-key readiness is defined by the actual proof-role inventory, not
  an assumed count of three circuits;
- retries preserve one durable intent lineage and cannot create a second
  spend attempt accidentally;
- the single-batch path remains gated on adversarial, multi-validator
  admission tests; and
- performance acceptance separates queueing, proving, consensus, and the
  optional transparent-egress action.

The v4 amendment also records a source-level proof dependency discovered
during implementation. The current private-primary binding hash commits to
the completed nested output-validity action, including its proof bytes.
Consequently the nested output-validity proof must finish before the outer
private-primary proof can be constructed. Claiming that those two proofs run
in parallel would be false. This specification now requires dependency-aware
scheduling and measurement; changing that dependency is a separately gated
consensus optimization, not a release blocker for the resident service.

### 2026-07-29 implementation checkpoint

The plan is no longer based only on architectural assumptions. The following
facts are implemented and locally verified:

- atomic shielded execution uses a trial ledger and Orchard state and commits
  no action unless every action accepts;
- the conformance harness recognizes all four canonical route shapes:
  issue-to-private, issue-to-transparent, redeem-to-private, and
  redeem-to-transparent;
- issue-to-private passed a valid baseline plus 11 invalid/replay cases with
  full rollback;
- issue-to-transparent passed a valid baseline plus 12 invalid/replay cases,
  including a corrupted egress proof, with full rollback;
- authenticated persistent-round health passed four- and six-validator
  failover integration tests;
- the resident asset service and `pftl-swapd` focused test suites pass; and
- exact private-primary proof-DAG timings are emitted into the durable swap
  journal.

The redemption route modes remain a release gate, not an inferred success.
Their baselines require a committed, controlled A666 note in the qualification
pre-state. The copied local chain correctly refused an artificial direct
commit because consensus v2 requires a precommit QC. The redemption vectors
therefore MUST be generated and run through the same conformance harness in a
real controlled certified round. No test-only state mutation or downgraded
finality path is acceptable evidence.

### 2026-07-29 PFTL-first delivery decision

The independently useful product is the fastest safe swap path that can be
delivered entirely on PFTL. It MUST NOT be held back by Ethereum finality,
Ethereum RPC health, or completion of the external bridge user experience.

The first shippable outcome is:

1. a user already has spendable transparent pfUSDC on PFTL;
2. the user issues A666 at the governed NAV price, increasing A666 supply;
3. the user receives private A666 by default, or explicitly requests
   transparent A666;
4. the user can later redeem A666 at the governed NAV price, decreasing A666
   supply and receiving private or transparent pfUSDC; and
5. both directions satisfy the PFTL latency, convergence, conservation,
   replay, and privacy gates in this specification.

Ethereum USDC -> pfUSDC ingress and PFTL -> Ethereum/Uniswap export remain
important adapters around this product, but they are separate state machines
with separate clocks. A pending or degraded Ethereum adapter MUST NOT make an
otherwise healthy PFTL-resident swap service unready.

## 1. Purpose

Make PFTL-native private NAV swaps interactive-speed. A user who already
holds transparent pfUSDC on PFTL and wants private A666 at the governed NAV
(or holds private A666 and wants pfUSDC back) currently waits ~5-6 minutes.
The measured cost is almost entirely orchestration wrapper, not physics:

| Component | Pass-6 measured | Intrinsic cost |
|---|---:|---:|
| Up to 3 consensus rounds | ~190s | 426ms/round hot certified |
| Chained Orchard proof/action creation | ~140s | 5.8s/proof hot, 66ms verify |

**Target: p50 <= 20s, p95 <= 45s** from accepted swap request to verified
six-validator convergence, with zero weakening of proof verification,
replay protection, supply conservation, or private-material discipline.

This is a **PFTL execution-layer SLO**. The service begins only after the
input pfUSDC is final and spendable on PFTL and ends when the PFTL output is
committed and converged. It does not reduce Ethereum finality or make the
first USDC -> pfUSDC bridge claim complete in 45 seconds. It makes repeated
private trading by PFTL-resident users fast and decouples that trading from
Ethereum.

Two clocks MUST be reported separately:

1. `pftl_execution_latency`: authenticated request accepted by
   `pftl-swapd` -> committed, six-validator-converged PFTL output. This is
   the normative 20s/45s SLO.
2. `user_journey_latency`: external-chain deposit initiation -> final
   requested output. This is an informational end-to-end measurement and
   includes any Ethereum finality or bridge delay.

The default private issue/redeem result remains shielded. A requested
transparent output adds an egress action and is measured as a distinct
route mode.

This boundary is a delivery decision, not merely a measurement convention.
The PFTL-resident service MUST be deployable, operable, and acceptance-tested
without an Ethereum endpoint. Bridge adapters may call or feed the service,
but the core service MUST NOT synchronously call Ethereum during quote,
authorization, proving, publication, commitment, or recovery.

## 2. Scope

In scope (PFTL-only flows, no Ethereum excursion):

1. **Private issue swap:** transparent pfUSDC -> Orchard pfUSDC ingress ->
   private-primary A666 issue at `issue_multiplier_bps` x NAV -> private
   A666 note (optionally -> private A666 egress to a transparent A666
   position, when requested).
2. **Private redeem swap:** private A666 -> private-primary A666 redemption
   at `redeem_multiplier_bps` x NAV -> private pfUSDC -> (optional) pfUSDC
   egress to transparent pfUSDC.

Version 1 is restricted to the governed `pfUSDC/A666` pair and those two
directions. A generic `asset_pair` field is not permission to route
unreviewed assets.

Out of scope (unchanged, covered elsewhere): Ethereum USDC ingress and the
finality floor (governed fast lane decision), PFTL->Ethereum export/mint,
acceptance-campaign evidence flows, any change to circuits, public inputs,
verification keys, NAV formulas, or route policy.

### 2.1 Delivery boundary

The implementation is divided into two independently releasable layers:

| Layer | Required for this specification | Readiness dependency |
|---|---|---|
| PFTL-resident issue/redeem service | Yes | PFTL node, governed NAV/policy, Orchard state, proving contexts, quorum |
| Ethereum ingress/export adapters | No; separate integration work | Ethereum finality, RPC, contracts, relayer/export proof |

No PFTL quote or swap may imply that an Ethereum deposit or export is
complete. Conversely, Ethereum unavailability is not a valid reason to
reject an otherwise valid PFTL-resident issue or redemption.

### 2.2 Trust and custody boundary

The protocol remains trust-minimized with respect to issuance and
redemption correctness: every validator independently verifies the proofs,
NAV/policy inputs, nullifiers, replay protection, route capacity, and supply
transition. `pftl-swapd` cannot cause invalid NAV issuance merely because it
builds or proposes the batch.

The initial service is nevertheless a **controlled-wallet deployment**.
Because validator-2 holds the spending material for the service wallet, its
operator can authorize or censor spends from that wallet. This deployment
MUST NOT be described as a non-custodial public service or as eliminating
operator custody. Funds outside the explicitly configured service wallet
are out of scope.

A later non-custodial mode requires a reviewed design in which the user
retains exclusive spend authorization (for example, user-side proof
construction or a signed authorization cryptographically tied to the
shielded spend authority). It is not implied by this specification.

## 3. Architecture

Four components. All run on existing hosts. They add no new consensus trust,
but they operate within the custody boundary in section 2.2.

### 3.1 `pftl-swapd` — resident swap daemon (validator-2)

A long-lived service that replaces the one-shot
`asset-orchard-*-create` CLI invocations for product swaps.

State held resident, per requirements below:

- **Warm proving contexts** for every proof role required by each supported
  route. The implementation MUST first codify an exact route/proof matrix:
  ingress, private-primary issue output validity and input consumption,
  private-primary redeem equivalents, and optional egress. Roles that do
  not require a proof in the current protocol MUST be recorded as
  `not_applicable`; readiness MUST NOT assume that there are exactly three
  circuits. Contexts are built or loaded at startup before the service
  reports ready. When the pinned
  proving-key artifact (research S1.1) lands, `pftl-swapd` MUST load it
  fail-closed with fingerprint validation against the pinned VK; until
  then it MUST prewarm by building once at startup.
- **Live Orchard frontier mirror:** the note-commitment frontier and
  nullifier view, updated on every committed block via the validator-2
  node it is colocated with. Each build MUST pin `{height, block_id,
  state_root, orchard_root}`. A swap request MUST NOT trigger a cold state
  fetch, but a cheap exact-tip comparison is REQUIRED immediately before
  publish. A changed anchor invalidates the prepared batch and returns it
  to the same durable intent lineage for rebuild; it MUST NOT be published
  against stale state.
- **Note material:** seeds, openings, and spending keys, held with the
  same discipline as the current one-shot flow (section 7), restricted to
  the configured controlled-wallet inventory.

Readiness: `pftl-swapd` MUST expose a ready report that enumerates every
route/proof role and asserts (a) each role is warm with its expected
fingerprint or explicitly `not_applicable`, (b) frontier mirror height and
roots equal the colocated node, (c) verifier caches are warm, (d) durable
journal capacity is available, and (e) consensus sessions can reach quorum.
Orchestration MUST refuse to route swaps to a non-ready daemon (pass-4
lesson: readiness is asserted, not assumed).

### 3.2 Fleet round driver — persistent peer-certified sessions

Extends the `transport-peer-certified-private-egress-loop` precedent from
one flow to the swap batch class:

- Persistent authenticated node-transport connections from the driver to
  all six validators, opened at service start, health-checked, reconnected
  with bounded backoff. SSH `ControlMaster` MAY be used only as
  controlled-testnet scaffolding; it is not the production transport.
- One certified round per swap batch: propose -> votes -> certificate ->
  local-apply verification, targeting the demonstrated 426ms-class hot
  round.
- Fleet preflight is **continuous, not per-round**: a background loop
  maintains current tip/block ID, state roots, proposer, quorum health,
  and mempool state. The freshness age is a local readiness signal only
  and MUST NOT affect consensus. A swap round may use a <=5s-old cached
  view instead of a full `preflight-fleet` pass, but MUST perform the
  exact-tip/root gate in 3.1 immediately before publish.

### 3.3 Batch builder — single-batch shielded swap

A swap executes as **one atomically published shielded batch in one
consensus round**, containing the chained actions
(ingress -> issue -> optional egress, or the redeem mirror).

Requirements:

- **R-BATCH-1:** All actions in the batch MUST be admissible against
  pre-block state plus intra-batch effects under existing execution
  semantics. Intra-batch chaining is via note commitments created and
  consumed inside the same batch.
- **R-BATCH-2 (gate):** Before any funds-bearing use, an admission
  study MUST confirm that mempool/batch admission validates chained
  shielded actions against the batch-local commitment set. This is the
  pass-5 lesson generalized: pass-5's one-round pfUSDC relay failed
  because claim admission checked pre-block *account/trustline* state.
  The shielded trio chains *note commitments*, which are batch-local,
  but this MUST be proven on the controlled testnet, not assumed. If
  admission cannot see batch-local commitments, either (a) extend
  admission to validate shielded batches as a unit (consensus-visible
  change, release-gated), or (b) fall back to N rounds driven over the
  persistent sessions (still ~1-2s per round).
- **R-BATCH-3:** The batch MUST be all-or-nothing: no partial
  application, no committed round on validation failure of any member
  action. Rejection MUST be pre-mutation, as today.
- **R-BATCH-4:** Existing per-action verification is unchanged: every
  validator independently re-verifies every Orchard proof and every
  nullifier/replay check before voting. No proposer-trust shortcut.
- **R-BATCH-5:** Phase A MUST test both issue and redeem, with and without
  optional egress, plus invalid proof at each action position, duplicate
  nullifier, reordered action, missing batch-local commitment, stale
  anchor, changed NAV/policy/capacity, malformed bounds, and replay. Every
  rejected case MUST leave identical pre-state on all six validators.
  Valid cases MUST produce identical state roots under direct batch
  execution and the normal admission path.
- **R-BATCH-6:** If Phase A requires any consensus-visible admission or
  execution change, it MUST ship behind an explicit protocol/release
  feature gate with conformance vectors and a coordinated validator
  rollout. It is not a daemon-only optimization.

### 3.4 Async evidence writer

- The user-facing result returns at verified certificate + local-apply +
  convergence confirmation.
- Reports, fleet snapshots, and summaries are written asynchronously
  after the fact to a per-swap evidence directory.
- **R-EV-1:** Async evidence MUST be lossless: an unflushed evidence
  queue MUST survive daemon restart (write-ahead journal), and the swap
  record (request, batch hash, round certificate reference, heights)
  MUST be durably journaled *before* the batch is published.
- **R-EV-2:** Journals and evidence MUST contain hashes, public amounts,
  policy references, heights, and timing only—never note openings, seeds,
  spending keys, authorization signatures, or output note references.
  Journal and evidence queues MUST be size-bounded. If durable capacity is
  exhausted, readiness becomes false and new swaps are rejected before
  authorization or spend construction.
- Acceptance campaigns MAY continue to run synchronous evidence; this
  spec does not change them.

## 4. Dependency-aware proving

Once swap amounts, seeds, and the NAV mark are fixed, the full note chain is
deterministic: ingress note N1, primary action consumes N1 -> N2, optional
egress consumes N2 -> transparent output. Proof scheduling MUST follow the
actual cryptographic dependency graph:

1. build N2 and its nested output-validity witness;
2. prove and assemble the nested output-validity action;
3. hash that completed action into the private-primary binding, then prove
   the private-primary input spend; and
4. prove an optional user-requested egress as soon as its N2 witness and
   batch-local commitment path are available.

Steps 2 and 3 are serial in the current protocol. The dependency is visible
in
`crates/privacy_orchard/src/asset_orchard_action_builders.rs`,
`build_asset_orchard_private_primary_action`: the completed
`output_validity.action` is an input to
`asset_orchard_private_primary_issue_binding_hash`, and that hash is a public
field of the outer proof.

- **R-PROVE-1:** The builder MUST derive available witnesses before proof
  scheduling and execute proof roles according to the explicit dependency
  DAG. Roles with no dependency edge SHOULD run concurrently. A release
  report MUST show the DAG and per-role timings and MUST NOT represent the
  nested output-validity and outer private-primary proofs as parallel.
- **R-PROVE-2:** Proof outputs MUST be assembled into the batch in
  canonical action order regardless of proving completion order.
- **R-PROVE-3:** Proof work MUST be CPU- and memory-bounded. It runs on a
  dedicated blocking/prover worker, never on an async reactor, and no
  private-state or journal lock may be held during proof generation. The
  daemon MUST enforce global and per-caller concurrency/queue limits,
  deadlines, and early backpressure; an unbounded FIFO is forbidden.
- **R-PROVE-4:** Service resource limits (thread count, CPU weight/affinity,
  and memory maximum) MUST prevent proving load from starving validator
  consensus, RPC, or storage work. Exceeding a service limit rejects or
  queues work; it MUST NOT degrade validator liveness.
- **R-PROVE-5:** Removing the serial edge between nested output validity and
  the outer private-primary proof requires a versioned binding preimage and
  coordinated protocol activation. It MUST have new conformance vectors,
  historical replay coverage, and a fleet rollout gate. It is an optional
  post-release optimization; the resident PFTL service MUST ship and be
  measured with the current binding first.
- GPU proving (research S1.2) is an optional later backend behind the
  same interface; it MUST NOT change proof bytes/transcripts relative to
  the pinned VK.

## 5. Service API

Versioned minimal surface, local-only (owner-restricted Unix socket
preferred; never a public listener):

- `quote(direction, amount_atoms)` -> priced quote for the fixed v1
  `pfUSDC/A666` pair against
  the current governed NAV mark: output atoms, spread atoms, NAV epoch,
  `pricing_reserve_packet_hash`, route mode, maximum fee, and quote expiry
  height.
- `swap(quote_id, idempotency_key, signed_intent)` -> executes; returns
  `{swap_id, batch_hash, height, certificate_ref, output_note_refs}`.
- `status(swap_id | idempotency_key)` -> `pending | committed(height) |
  rejected(reason) | failed_pre_publish(reason) | unknown`.
- `ready()` -> readiness report (3.1).

Requirements:

- **R-API-1:** `swap` MUST be idempotent on `idempotency_key`: a retry
  after a network failure MUST return the original outcome and MUST NOT
  build a second batch. This is the daemon-level form of the standing
  rule from the Phase-9 deposit: never create a second attempt against
  the same intent; resume the lineage.
- **R-API-2:** A quote MUST pin the NAV epoch and policy hash it priced
  against; `swap` MUST fail closed if the governed profile, policy hash,
  or NAV epoch changed after quoting, or if the NAV mark exceeds
  `max_nav_age_blocks` at execution height.
- **R-API-3:** All amounts respect existing route/policy limits
  (`min_order_atoms`, `max_order_atoms`, capacity remaining); the daemon
  enforces them pre-build in addition to consensus enforcement.
- **R-API-4:** Every swap requires an authenticated principal and a signed,
  expiring intent binding chain ID, route/direction, controlled-wallet
  identity, input reference or reservation, exact input amount, minimum
  output, maximum fee, quote ID/hash, NAV epoch, policy hash, expiry height,
  and idempotency key. Any mismatch fails before proving. Unix peer
  credentials alone are not spend authorization.
- **R-API-5:** Request sizes, quote lifetime, per-principal request rate,
  in-flight proof jobs, and queue depth MUST have enforced bounds. Output
  note references are returned only over the authenticated response and
  MUST NOT appear in `status`, metrics, logs, journals, or evidence.
- **R-API-6:** Quotes do not reserve consensus capacity in v1. Execution
  rechecks capacity and all quote-bound state immediately before build and
  publish. A future soft-reservation design requires its own authenticated
  quota, expiry, crash-release, and denial-of-service review.

## 6. Failure and resume model

State machine per swap: `QUOTED -> AUTHORIZED -> JOURNALED -> PROVING ->
PREPARED -> PUBLISHED -> COMMITTED | REJECTED`, with
`INTERRUPTED_PREPUBLISH` as a resumable non-consensus state.

- **R-FAIL-1:** Crash before `PUBLISHED`: on restart the daemon MUST
  mark the swap `INTERRUPTED_PREPUBLISH` and MUST NOT auto-publish.
  A retry with the same idempotency key and byte-identical signed intent
  MAY resume or rebuild only within the original durable lineage after
  revalidating quote, anchor, policy, capacity, and exclusive input
  reservation. It is not a fresh attempt. A different intent under the
  same key is rejected. A terminal failure requires a new idempotency key.
- **R-FAIL-2:** Crash after `PUBLISHED`, outcome unknown: on restart the
  daemon MUST resolve the batch hash against chain state before
  answering any `status` call or accepting new swaps that touch the same
  notes. Exactly-once semantics come from consensus (nullifier/replay
  protection), and the daemon MUST rely on that, not on local memory.
- **R-FAIL-3:** A failed/rejected batch MUST NOT leak note material into
  logs or evidence; failure evidence contains hashes and heights only.
- **R-FAIL-4:** Fleet degradation: if the persistent-session pool cannot
  reach vote quorum, the daemon MUST stop accepting swaps (fail-closed)
  rather than fall back to slower ad-hoc transport silently; the
  degraded mode MUST be visible in `ready()`.
- **R-FAIL-5:** The authorized intent record and exclusive input/nullifier
  reservation MUST be durably fsynced before proving, and the prepared
  batch hash plus `PUBLISHED` transition MUST be durably fsynced before
  network publication. After publication, cancellation is forbidden and
  recovery follows R-FAIL-2.
- **R-FAIL-6:** On `SIGTERM`, stop admission, drain or mark in-flight
  pre-publish work interrupted, fsync the journal, and exit. Abrupt
  restart MUST preserve the state machine and MUST never produce two
  lineages for one intent.

## 7. Private material discipline (unchanged, restated as binding)

- Controlled-wallet note seeds, openings, and spending keys remain on
  validator-2 only,
  files mode `0600`, owner-restricted; the daemon runs as the same user
  as the current one-shot flow.
- **R-KEY-1:** Key material MUST never appear in the API surface,
  evidence, logs, journals, or crash dumps; in-memory copies MUST be
  zeroized on drop where the type supports it.
- **R-KEY-2:** The daemon MUST NOT add any remote key-fetch path. Keys
  given as file paths are read at use time; no copies are made to new
  locations.
- **R-KEY-3:** Spend-authorizing principals, wallet mapping, socket
  ownership, and signer rotation/revocation are explicit deployment
  configuration. A caller that can reach the socket but lacks a valid
  signed intent cannot spend.

## 8. What explicitly does not change

- Circuits, public inputs, verification keys, proof sizes.
- Independent validator re-execution and re-verification before votes.
- Pre-mutation rejection and replay protection semantics.
- NAV formulas, `issue_multiplier_bps`/`redeem_multiplier_bps`, route
  caps, governed profile validation.
- Six-validator quorum rules and convergence requirements.
- The acceptance-campaign orchestration (separate, synchronous, frozen).

## 9. Performance acceptance criteria

Measured on the live controlled fleet with machine-stamped monotonic time
at every stage boundary (extends `issue_timing.v2` with a per-stage schema
— no more mtime archaeology). Queue delay and rejected admission are
reported separately from accepted-request service time. Cold-start and
warm-path samples are never mixed.

Release qualification requires at least 100 completed swaps per direction
for the default shielded-output route. Optional transparent-egress routes
require their own sample and may not borrow the shielded route's percentile.
Report p50, p95, p99, maximum, sample count, hardware/build fingerprint, and
validator missed-round rate:

| Metric | Target |
|---|---|
| Swap request -> committed + converged, p50 | <= 20s |
| Swap request -> committed + converged, p95 | <= 45s |
| Current proof DAG wall time, p95 | <= 35s for first release; <= 10s only after an independently qualified binding/prover optimization |
| Consensus round (publish -> certificate), p95 | <= 3s |
| Daemon cold start -> ready (with PK artifact) | <= 60s |
| Daemon cold start -> ready (PK build fallback) | <= 20 min, off critical path |
| Sustained load | 100 sequential swaps, no latency drift > 20%, converged fleet, empty mempools after |
| Bounded burst | 10 concurrent requests: complete within configured capacity or reject before proving; no validator missed-round regression |

Functional gates (all MUST pass on every scored run): six-validator
convergence, supply invariant, replay rejection on double-publish
attempt, no private material in evidence (redaction scan), idempotent
retry test, crash-resume test at each state-machine stage, invalid-action
atomicity matrix from R-BATCH-5, and proof that proving load does not
degrade validator liveness.

Release evidence MUST contain a PFTL-only timing report that can be reproduced
with Ethereum access disabled. Any end-to-end bridge demonstration is reported
separately and MUST show at least:

- Ethereum confirmation/finality time;
- bridge observation and claim time;
- PFTL-resident swap time;
- PFTL export-proof time; and
- Ethereum submission/finality time.

The combined user-journey number MUST NOT replace or obscure the normative
PFTL service number.

## 10. Rollout phases

1. **Phase A — route inventory, authorization, and admission study.**
   Freeze the v1 route/proof-role matrix and controlled-wallet custody
   boundary; implement signed-intent test vectors; then run the complete
   R-BATCH-5 matrix on the controlled testnet. Produce a written
   determination/ADR with source locations and conformance vectors. No
   mainnet funds. If batch-local admission is negative, adopt the N-round
   fallback and re-scope Phase C targets (~+3s). If a consensus change is
   needed, follow R-BATCH-6.
2. **Phase B — first usable PFTL-only release.** Deploy `pftl-swapd` with
   warm PKs, frontier mirror, readiness, persistent fleet sessions,
   continuous preflight, and bounded dependency-aware proving. Swaps MAY
   still use N certified rounds if Phase A has not admitted the atomic path.
   This release has no synchronous Ethereum dependency and is
   acceptance-tested for both issue and redeem. Expected: ~5-6 min -> tens
   of seconds.
3. **Phase C — fastest PFTL path.** Enable the admitted single atomic batch,
   retain dependency-aware proof scheduling, and remove remaining
   per-request orchestration startup. Expected: -> ~10-45s under the
   current serial proof edge, subject to the measured qualification
   distribution rather than a one-off demonstration.
4. **Phase D — durable async evidence, recovery, resource isolation, and
   per-stage timing schema.** Expected: p50 <= 20s met.
5. **Phase E — controlled-value canary and performance qualification.**
   Run restart/fault injection at every state, bounded-load tests, the
   required issue/redeem samples, and rollback rehearsal before increasing
   value limits.
6. **Phase F (optional, independent adapters) — pinned PK artifact
   integration (S1.1), GPU backend (S1.2), and redemption-mirror wiring
   into the Ethereum egress flow. Ethereum work may proceed in parallel,
   but it is not a release gate for Phases B-E and is measured under the
   separate `user_journey_latency` clock.**

Each phase ships behind the standard release/regression-gate treatment
(pinned-VK gate precedent) and its own controlled-testnet evidence packet
before touching mainnet value.

## 11. Implementation and release gates

Each normative requirement MUST map to its implementation location and at
least one test/evidence reference in the Phase A determination and release
packet. Consensus-visible changes require state-transition vectors runnable
by every validator implementation. Required release checks include format,
lint, unit/property tests for amount and state invariants, multi-validator
integration tests, malformed/replay tests, restart recovery, and
before/after latency profiles on the controlled fleet.

The first production canary MUST use a low governed value cap, one active
controlled wallet, and a single-writer input reservation. Raise limits or
enable multiple wallets only after the qualification sample passes without
supply divergence, note leakage, duplicate lineage, or validator liveness
regression.

## 12. Open questions

1. **Resolved in implementation, pending controlled-fleet qualification:**
   atomic shielded admission now evaluates batch-local note commitments
   against a trial state and commits only if every action accepts. Local
   conformance and four-/six-validator transport tests pass; the funds-bearing
   gate remains closed until the complete R-BATCH-5 matrix passes on the
   controlled fleet.
2. Egress-to-transparent as part of the same batch or a user-optional
   second step (privacy tradeoff: immediate egress links timing).
   Default SHOULD be to leave output notes shielded.
3. Which non-custodial authorization/proving design should replace the
   controlled-wallet mode if the service becomes public?
4. After the single-wallet canary, should multi-wallet concurrency use
   disjoint per-wallet workers or a shared reservation manager? The first
   release serializes conflicting input/frontier work rather than guessing.

Ethereum finality is deliberately not an open question for this plan: it is
outside the PFTL execution SLO and cannot block the PFTL-resident release.
