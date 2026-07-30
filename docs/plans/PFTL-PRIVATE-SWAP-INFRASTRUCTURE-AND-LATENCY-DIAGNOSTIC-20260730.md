# PFTL Private Swap Infrastructure and Latency Diagnostic

Date: 2026-07-30
Status: P0 deployed and controlled roundtrip passed; longer qualification is NO-GO pending NAV-refresh automation and issue-latency work
Scope: resident private A666/pfUSDC issue and redeem service, plus PFTL latency that materially affects that service

## Executive determination

The private swap path works, but it is not yet production infrastructure.

The most recent clean high-core runs completed both directions:

- private pfUSDC to newly issued private A666;
- private A666 redemption back to private pfUSDC;
- exact A666 supply accounting remained intact;
- all six validators converged;
- no reservation or mempool residue remained.

Proof generation is no longer the dominant blocker. It has fallen from roughly
83–87 seconds to approximately 12.1–12.3 seconds on the high-core prover.
The remaining delay is predominantly duplicated deterministic execution and
verification around PFTL consensus.

The best overnight use of time is:

1. remove a measured 3.2–3.7 seconds of unnecessary full historical QC-graph
   verification from the resident swap service's view-zero completion path;
2. instrument that boundary so the improvement is directly attributable;
3. run a controlled issue/redeem qualification sequence;
4. if the issue is below 45 seconds with margin, run a ten-cycle smoke test;
5. use any remaining time to prototype, but not deploy, prepared execution
   reuse for the proposer and validators.

This first change is intentionally narrow. It does not alter consensus,
transaction semantics, shielded verification, supply accounting, validator
voting, or state transition rules.

A broad PFTL network or BFT rewrite is not justified by the evidence. Ordinary
PFTL payments have already measured approximately 290 ms p50 and 375 ms p95.
The 20–47 second behavior is specific to computation-heavy shielded asset swaps,
not baseline chain finality.

## Suggested action set

This is the short execution order. The detailed, evidence-bearing checklist is
under [Recommended overnight execution plan](#recommended-overnight-execution-plan).

- [x] Ship the view-aware resident finality observer and its bounded
  `processed_finality_verify_ns` metric.
  - Pass condition: view-zero work ignores irrelevant historical QC files,
    nonzero-view verification remains fail-closed, and validator consensus code
    is unchanged.
  - Result: deployed from `8c73735`; observed verification fell from an
    estimated 3.211–3.686 seconds to 0.293–0.408 seconds.
- [x] Prove recovery and one complete private pfUSDC -> A666 -> pfUSDC
  roundtrip on the live six-validator fleet.
  - Pass condition: exact-once recovery, six-node convergence, exact supply
    restoration, zero active reservations, and no mempool residue.
  - Result: passed at heights 513–515, including a restart after publication.
- [!] Complete the unattended ten-cycle qualification.
  - Pass condition: 10/10 issues and redeems, no operator intervention, fresh
    governed NAV throughout, and every correctness and latency gate below.
  - Result: stopped safely in cycle 5 after four complete cycles. The fifth
    issue failed before publication because the finalized StakeHub NAV packet
    exceeded the consensus freshness window (`stale_pftl_uniswap_pricing`).
- [ ] Add governed StakeHub NAV refresh to the qualification runner.
  - Fetch and verify real StakeHub reserves, publish/finalize the next NAV
    packet through the ordinary PFTL path, wait for six-node convergence, then
    obtain a new quote. Never bypass the freshness check or reuse a quote whose
    pricing epoch changed.
  - Pass condition: a test campaign crosses the NAV freshness boundary without
    manual intervention and never submits against stale pricing.
- [ ] Make the runner readiness-aware at every state boundary.
  - Wait for the resident service, high-core prover mirror, round driver, and
    all six validators to report the same committed height/state before quote,
    proof, publication, and private-output reuse.
  - Pass condition: no transient mirror-lag `409`/`503` retries in ten
    consecutive cycles.
- [ ] Reduce private issue latency before a 100/100 campaign.
  - Current four-cycle accepted-to-commit issue p95 is 50.365 seconds, versus
    the 42-second gate. Redeem p95 is 38.862 seconds and passes.
  - The next narrow target is content-bound prepared execution reuse, behind a
    disabled feature flag until differential, stale-parent, conflicting-view,
    tamper, eviction, and restart tests pass.
  - Pass condition: ten-cycle issue p95 at or below 42 seconds without changing
    receipts, state roots, independent validator verification, or persistence.
- [ ] Re-run the full ten-cycle gate and only then authorize 100/100.
  - Pass condition: all section F checkboxes pass. Until then the explicit
    campaign decision remains `NO-GO`.

## Current measured state

### Latest clean private rounds

| Round | Operation | Proof DAG | Prepublication | Publish to commit | Accepted to committed | Result |
|---|---:|---:|---:|---:|---:|---|
| h508 | private issue | 12.322 s | 17.329 s | 29.924 s | 47.371 s | correct, over 45 s target |
| h509 | private redeem | 12.295 s | 15.688 s | 23.041 s | 38.864 s | correct, under 45 s target |
| h511 | private issue | 12.141 s | not separately recorded | 29.757 s | 47.299 s | correct, over 45 s target |
| h512 | private redeem | 12.111 s | not separately recorded | 20.058 s | 35.742 s | correct, under 45 s target |

For h508 and h509, the consensus-stage decomposition was:

| Stage | h508 issue | h509 redeem |
|---|---:|---:|
| client-visible finality | 26.356 s | 19.965 s |
| proposal | 9.991 s | 6.887 s |
| vote requests | 5.965 s | 4.303 s |
| certificate | 4.074 s | 4.319 s |
| local apply | 5.548 s | 3.722 s |
| certified sends/convergence | 7.400 s | 5.646 s |

The issue path is consistently slower because it contains two shielded actions:

1. the pfUSDC Orchard ingress; and
2. the private-primary A666 issue.

The redemption path contains one private-primary action. The issue path therefore
performs more proof verification, ledger validation, state cloning, and state
application at every independent execution boundary.

### Functional and safety state

At the latest audited baseline:

- chain height was 512;
- six of six validators were converged;
- validator mempools were empty;
- A666 supply was exactly `31,489,197,455` base units;
- the supply invariant passed;
- there were no live reservations;
- the returned pfUSDC was private.

The correctness result matters: this is now a latency and service-hardening
problem, not evidence that private issue/redeem semantics are fundamentally
broken.

### Service-level objectives

The existing resident-service targets remain appropriate:

- request accepted to committed and converged: p50 at or below 20 seconds;
- request accepted to committed and converged: p95 at or below 45 seconds;
- proof generation: p95 at or below 35 seconds;
- consensus publish to certificate: p95 at or below 3 seconds;
- 100 successful private roundtrips out of 100 attempts;
- exact supply and reserve invariants after every round;
- restart and failure recovery without manual state surgery.

The current proof path passes its target. Redemption currently passes the
single-sample end-to-end target. Issue misses it by approximately 2.3 seconds.
The system has not yet earned the 100/100 claim.

## Latency diagnosis

### 1. The resident service re-verifies irrelevant historical QC state

The clearest immediate defect is in
`crates/node/src/bin/pftl_swapd.rs`, in
`verify_processed_swap_finality`.

When the service observes a completed swap, it loads and cryptographically
walks the full persisted consensus-v2 QC graph before accepting the commit.
It does this even for a view-zero commit.

For view zero:

- timeout evidence is not allowed;
- a valid-round QC is not allowed;
- the commit has no historical QC dependency.

The validator hot path already represents this rule. The existing
`read_consensus_v2_qc_graph_for_view` helper returns an empty graph for view
zero and loads dependencies only for a nonzero view. The existing
`verify_consensus_v2_commit_for_block` path uses that view-aware helper.

The resident service does not. This is duplicated work outside consensus and
outside state execution.

The unaccounted time supports this diagnosis:

- h508: `47.371 - 17.329 - 26.356 = 3.686 seconds`;
- h509: `38.864 - 15.688 - 19.965 = 3.211 seconds`.

That cost is larger than the issue's current 2.3-second SLO miss.

#### Recommended fix

Change the resident service finality observer to load the QC graph using the
commit proposal's view:

- view zero: verify the exact commit without loading historical QC dependencies;
- nonzero view: retain the full dependency loading and validation;
- all views: retain certificate, signer, quorum, proposal, payload, parent,
  height, and batch binding checks.

Add a dedicated `processed_finality_verify_ns` measurement around this work.

Estimated result, based on current samples:

- private issue: approximately 43.6–44.1 seconds;
- private redeem: approximately 32–36 seconds.

This estimate must be proven by a fresh live run. It is not a guarantee.

### 2. The batch is fully simulated before consensus

`pftl-swapd::execute_prepublication` runs `simulate_shielded_batch` before
publication. The latest measurements attribute approximately:

- 4.365 seconds to the issue simulation;
- 2.766 seconds to the redeem simulation.

This is currently a safety boundary. It catches an invalid swap before consensus
and ensures the complete batch—not merely an individual proof response—executes
against the expected state.

It should not be deleted merely because upstream proof generation already
verifies a proof. A safe optimization requires a content-bound execution or
verification receipt that proves all of the following:

- exact pre-state identity;
- exact ordered batch bytes or batch hash;
- exact proof and public-input bytes;
- exact post-state root and receipt;
- circuit/VK/build fingerprint;
- no intervening state change.

Until that artifact exists, prepublication simulation stays.

### 3. State execution performs nested full-state cloning

The atomic shielded batch path clones ledger and shielded state to provide
all-or-nothing execution. Individual action implementations then clone some of
the same state again before validation and commit.

The issue path compounds this because it has two actions. The observed effect is:

- issue state execution: roughly 4.45 seconds;
- redeem state execution: roughly 2.63 seconds;
- remote vote construction: approximately 4.65–4.92 seconds for issue and
  2.90–3.09 seconds for redeem.

The cloning is not merely inefficient code that can be removed mechanically.
It implements rollback behavior. The replacement should be an explicit
transactional execution context or copy-on-write overlay with identical error
and atomicity semantics.

### 4. The proposer executes the same deterministic batch twice

The proposer executes the batch to build the proposal and resulting state root.
After collecting a quorum certificate, it executes the certified batch again
to apply it locally.

Remote validators must independently execute before voting. That must not
change. However, the proposer can safely reuse its own exact prepared execution
if the artifact is rigidly bound to:

- pre-state identity and state root;
- parent block and height;
- consensus domain, round, and view;
- ordered batch hash;
- proposal hash and expected post-state root;
- exact validator software/protocol version.

After certification, the node would verify the QC and all bindings, then commit
the prepared post-state through the existing write-ahead and persistence path.
Any mismatch, restart, or cache miss falls back to current full execution.

Potential saving:

- approximately 5.5 seconds on issue;
- approximately 3.7 seconds on redeem.

This is the strongest PFTL-wide optimization identified, but it requires more
testing than the resident-service view-zero fix.

### 5. Validators repeat execution after already voting on it

A validator independently executes the proposal to decide whether to sign a
prepare vote. When the certified batch returns, it executes the same exact batch
again before applying it.

A bounded in-memory prepared-execution cache can reuse the result only when the
certified payload is byte-for-byte and context-for-context identical to the
voted proposal. It must reject:

- a different parent or pre-state;
- a different height, view, or round;
- a different batch or proposal hash;
- a different expected state root;
- stale cache entries;
- entries created by another software or protocol version.

Restart or eviction falls back to deterministic re-execution.

This could reduce the current 5–7 second certified-send/convergence tail. It must
not weaken the requirement that every validator independently executes before
voting.

### 6. Proof generation is improved but operationally fragile

The high-core proof path reduced proof time to about 12 seconds, but the current
deployment relies on:

- a mounted validator data directory over SSHFS;
- a reverse SSH tunnel;
- a separate high-core prover;
- warm process state;
- proving-key construction that has taken roughly 330–447 seconds when cold.

This is useful qualification infrastructure, not a durable production boundary.
A restart can turn a 12-second proof into a multi-minute outage. A broad
filesystem mount also exposes much more state than a prover needs.

### 7. The circuit binding makes proof generation serial

The private-primary proof is currently bound to the nested output proof bytes.
Consequently, the outer proof cannot be generated until the nested proof exists.

Verification can potentially be parallelized once all proof bytes exist, but
generation remains serial without a circuit/protocol revision.

A future circuit version should bind the outer statement to stable public
commitments rather than the nested proof encoding, or replace the pair with a
composite/batched proof. This could move the proof component from roughly
12 seconds toward one proof's cost, but it requires:

- new circuits and verification keys;
- explicit versioning and activation;
- cross-version rejection tests;
- replay and downgrade tests;
- a fleet rollout plan.

It is not an overnight production change.

## Private swap infrastructure required for production

### Dedicated authenticated prover protocol

Replace the SSHFS and reverse-tunnel arrangement with a narrow prover API.

The custody/wallet side should:

- select notes and construct the transaction intent;
- retain spend authority locally;
- construct only the bounded witness material required for proving;
- send a request with a stable request ID, content hash, circuit version, and
  deadline.

The prover should:

- accept only authenticated and size-bounded jobs;
- hold no spend keys;
- return the proof, public inputs, build/VK fingerprint, timing breakdown, and
  a canonical response hash;
- deduplicate retries by request ID and content hash;
- enforce queue and concurrency limits;
- cancel expired work and zeroize witness memory where practical.

Use an authenticated encrypted channel such as mTLS or Noise. Do not expose
the validator's entire data directory to the proving host.

### Proving-key lifecycle

Cold proving-key construction is a release and availability risk. The target
state is:

- a versioned serialized proving-key artifact;
- cryptographic binding to circuit, VK, protocol, and build fingerprints;
- integrity verification before use;
- memory-mapped or bounded-time loading;
- a measured restart-to-ready target below 60 seconds;
- one warm active prover and one warm standby until cold loading is fixed.

The standby must be exercised, not merely installed.

### Authenticated state/frontier delivery

Replace remote filesystem access with a minimal state feed:

- signed or otherwise authenticated snapshots;
- monotonic sequence numbers;
- state/frontier root binding;
- incremental updates;
- rollback/reorg handling appropriate to finalized PFTL state;
- bounded retention and a full resync path.

The prover must refuse a job whose referenced state cannot be authenticated.

### Durable private wallet journal

The resident service needs exact-once recovery across:

- accepted but not yet proved;
- proved but not published;
- published but not committed;
- committed but not locally indexed;
- private output created but not yet restored for the next test cycle.

Use an append-only write-ahead journal plus compact snapshots, or an already
approved transactional embedded store. Preserve atomic rename/fsync semantics
for snapshots. Avoid introducing a new database merely to hide ambiguous state
transitions.

Recovery tests must kill the service at every transition and prove:

- no duplicate issue or redeem;
- no lost note;
- no reused nullifier;
- no incorrect reservation release;
- no supply divergence.

### Automated private-output handling

The qualification loop currently needs controlled private pfUSDC egress or
restoration between issue/redeem pairs. That should be a first-class state in
the service and test harness, not a manual note-path operation.

The 100/100 campaign needs either:

- an automated, audited private-output-to-next-input loop; or
- a pre-generated set of independent valid private inputs.

Without this, an unattended campaign can stop for wallet plumbing even when
the swap protocol is correct.

### Deployment and observability

Production packaging should include:

- signed and reproducible release artifacts;
- persistent service units, not transient shells;
- resource and queue limits;
- active/standby health and failover;
- a rehearsed restart and rollback procedure;
- explicit circuit/VK/build fingerprints in health output;
- per-stage latency histograms;
- queue depth, failure class, restart recovery, and convergence metrics.

Metrics must not expose note references, paths, proofs, signatures, nullifiers,
wallet identifiers, or private amounts beyond an explicitly approved policy.

### Concurrency model

The first production service may process one wallet mutation at a time, but it
should not globally serialize forever.

The safe future model is:

- serialize jobs that conflict on wallet, input note, nullifier, reservation,
  or frontier dependency;
- prove disjoint jobs concurrently within CPU and memory limits;
- preserve deterministic publication order;
- revalidate state immediately before publication;
- expire and rebuild stale jobs instead of forcing them through.

## PFTL latency improvement roadmap

| Priority | Change | Expected scope | Estimated impact | Risk |
|---|---|---|---:|---|
| P0 | view-aware QC graph in `pftl-swapd` completion verification | resident swaps only | 3.2–3.7 s | low |
| P1 | proposer prepared-execution reuse after QC | heavy PFTL batches | 3.7–5.5 s | medium |
| P1 | validator vote-to-certified prepared-state reuse | fleet convergence | 5–7 s tail | medium/high |
| P1 | stage metrics and production flamegraphs | diagnosis | prevents blind work | low |
| P2 | transactional execution overlay to remove nested clones | shielded execution | several seconds, to measure | medium/high |
| P2 | parallel pure proof verification with deterministic error order | multi-proof batches | subsecond to low seconds | medium |
| P2 | serialized proving keys and warm failover | private service availability | removes multi-minute cold start | medium |
| P2 | circuit v2/composite proof | private issue proof DAG | potentially about 6 s | high/protocol |
| P2 | GPU proof generation | private service | unknown until benchmarked | high |
| P3 | copy-on-write or structured state storage | chain-wide heavy state | workload dependent | high |
| P3 | transport/consensus redesign | chain-wide | not presently justified | very high |

All impact figures are estimates derived from current stage timings. They must be
validated independently and must not be added together as if every stage were
fully serial.

## Recommended overnight execution plan

### Agent execution checklist

This is the authoritative markable checklist for the overnight session.

Status convention:

- `[ ]` not started;
- `[x]` completed and supported by recorded evidence;
- `[!]` attempted but blocked or failed; add the reason and evidence path on
  the same line or immediately below it.

Items must not be marked complete merely because code was written. A live or
qualification item is complete only when its stated postconditions and evidence
exist.

#### A. Diagnosis and starting state

- [x] Diagnose the current private issue/redeem latency using h508, h509, h511,
  and h512 measurements.
- [x] Confirm that proof generation is approximately 12.1–12.3 seconds on the
  high-core path.
- [x] Identify the resident service's unconditional full QC-graph read as the
  first narrow optimization target.
- [x] Confirm that the validator hot path already has a view-aware QC-graph
  helper.
- [x] Record the latest audited baseline in this report: height 512, six
  converged validators, empty mempools, A666 supply `31,489,197,455`, invariant
  true, and zero active reservations.
- [x] Reconfirm the live baseline immediately before making or deploying any
  change.
- [x] Record the exact source commit, resident-service build fingerprint,
  circuit/VK fingerprint, service configuration, and deployment manifest.
- [x] Preserve the current mount-unit hardening without sweeping unrelated
  deployment or evidence files into the change.
- [x] Confirm and document how the currently private pfUSDC output will be used
  or restored for the controlled issue/redeem run.

#### B. Implement the P0 resident-service fix

- [x] Replace the unconditional `read_consensus_v2_qc_graph` call in
  `verify_processed_swap_finality` with the existing view-aware helper using
  `commit.proposal.round.view`.
- [x] Retain exact domain, validator-set, quorum, signer, signature, proposal,
  parent, height, view, payload, batch, block-hash, and certificate binding
  checks.
- [x] Confirm that view zero uses no historical dependency graph.
- [x] Confirm that nonzero views still load and cryptographically verify all
  required historical dependencies.
- [x] Add an explicit `processed_finality_verify_ns` stage timer.
- [x] Add the timer to the resident-service report and operational metrics
  without exposing private wallet or note material.
- [x] Review the diff and confirm that it changes neither consensus rules nor
  validator state execution.

#### C. Test the P0 change

- [x] Add a view-zero success test with an empty dependency graph.
- [x] Add a view-zero test with a large irrelevant historical graph and verify
  that work remains bounded.
- [x] Add a nonzero-view test that proves the required QC history is still
  loaded and verified.
- [x] Add rejection tests for a tampered prepare QC.
- [x] Add rejection tests for a tampered precommit QC.
- [x] Add rejection tests for incorrect signers, quorum, and signatures.
- [x] Add rejection tests for mutated proposal, parent, height, view, payload,
  batch, and block hash.
- [x] Confirm exact commit-to-archived-batch binding equivalent to the clean
  h508/h509 behavior.
- [x] Confirm journal and idempotency behavior is unchanged.
- [x] Run the affected unit and integration tests.
- [x] Run `cargo check` for every affected target.
- [x] Run affected clippy checks with no new warnings.
- [x] Run deterministic replay or differential checks and confirm identical
  receipts and state roots.
- [x] Run a six-node local view-zero round.
- [x] Run a forced nonzero-view round.
- [x] Restart between publication and finality observation and confirm exact
  recovery.
- [x] Record test commands, results, timings, and evidence paths.

Evidence for sections A through C is recorded in
`docs/evidence/pftl-private-swap-p0-20260730/README.md`. The restart-window
test passed during the controlled issue at height 514: the service was stopped
after durable `PUBLISHED`, the block committed, and the restarted observer
recorded the same swap exactly once.

#### D. Build and deploy the resident service

- [x] Build a pinned resident-service artifact from the reviewed source commit.
- [x] Record its artifact hash and build fingerprint.
- [x] Back up the current resident-service binary, configuration, journal, and
  service definition.
- [x] Prepare and verify a rollback command and artifact before deployment.
- [x] Deploy only `pftl-swapd`; do not upgrade validator binaries for the P0
  observer change.
- [x] Restart the service and verify health, build fingerprint, circuit/VK
  fingerprint, queue state, wallet state, and current chain tip.
- [x] Exercise one controlled service restart before the live swap.

#### E. Run one controlled private roundtrip

- [x] Prepare or restore the private pfUSDC input without manual mutation of
  consensus or wallet state.
- [x] Record the pre-issue fleet, supply, reserve, reservation, mempool, wallet,
  and note-state baseline.
- [x] Submit one private pfUSDC-to-A666 issue.
- [x] Confirm the issue commits and all six validators converge.
- [x] Confirm exact A666 supply increase and route invariant.
- [x] Confirm no unexpected reservation, mempool, or journal residue.
- [x] Record proof, prepublication, finality-observer, proposal, vote,
  certificate, local-apply, convergence, and total issue timings.
- [!] Confirm issue latency is below 45 seconds with practical margin.
  - Failed: the normal qualification samples were 46.118–50.365 seconds
    accepted-to-commit. The restart-injected controlled issue is not a valid
    latency sample.
- [x] Submit one private A666-to-pfUSDC redeem.
- [x] Confirm the redeem commits and all six validators converge.
- [x] Confirm exact A666 supply restoration and route invariant.
- [x] Confirm the returned pfUSDC is private and indexed exactly once.
- [x] Confirm no unexpected reservation, mempool, or journal residue.
- [x] Record the same complete timing decomposition for redemption.
- [x] Confirm redeem latency remains below 45 seconds.
- [x] Compare the new finality-observer cost against the 3.211–3.686 second
  historical estimate and explain any discrepancy.
- [x] Store a complete redacted evidence bundle for the roundtrip.

#### F. Ten-cycle qualification gate

- [!] Confirm the private-output-to-next-input loop can run unattended.
  - Failed at cycle 5 because the runner did not refresh the governed StakeHub
    NAV before its consensus freshness window elapsed.
- [x] Confirm a restart recovery test passed before beginning the campaign.
- [!] Run ten consecutive private issue/redeem cycles without manual state
  edits.
  - Stopped safely after 4/10 complete cycles; cycle 5 was rejected during local
    simulation before publication with `stale_pftl_uniswap_pricing`.
- [!] Confirm 10/10 issues and 10/10 redeems committed exactly once.
  - 4/4 completed issues and 4/4 completed redeems committed exactly once; the
    required ten-cycle sample was not reached.
- [x] Confirm six-validator convergence after every committed operation.
- [x] Confirm supply and reserve invariants after every committed operation.
- [x] Confirm zero unexplained reservations and mempool residue after every
  operation.
- [x] Confirm swap proof p95 is at or below 35 seconds.
  - Four-cycle private-primary proof-DAG maxima were 12.496 seconds for issue
    and 12.526 seconds for redeem.
- [!] Confirm issue p95 is at or below 42 seconds.
  - Failed: four-cycle accepted-to-commit p95 was 50.365 seconds.
- [x] Confirm redeem p95 is at or below 45 seconds.
  - Four-cycle accepted-to-commit p95 was 38.862 seconds.
- [!] Confirm there were no unexplained retries, journal repairs, or wallet
  interventions.
  - No journal repair or direct wallet/state edit occurred. The campaign did
    encounter explained mirror-readiness retries and the cycle-5 stale-pricing
    rejection, so it does not meet the unattended criterion.
- [!] Publish the ten-cycle timing distribution and redacted evidence index.
  - A partial four-cycle distribution and failure evidence are published; a
    ten-cycle distribution does not exist because the fail-closed gate stopped
    the campaign.
- [x] Record an explicit `GO` or `NO-GO` decision for the 100/100 campaign.
  - `NO-GO`: automate governed NAV refresh, eliminate readiness races, and pass
    the 42-second issue p95 gate before starting 100/100.

#### G. Optional prepared-execution prototype

Start this section only after the P0 live roundtrip and ten-cycle gate are
complete or an explicit decision ends further live qualification.

Decision: not started in this session. The ten-cycle gate ended with `NO-GO`,
and the higher-priority next action is to make NAV freshness and readiness part
of the qualification runner. The unchecked boxes below remain a future,
local-only prototype backlog; they are not omitted work from the P0 release.

- [ ] Define a `PreparedShieldedExecution` artifact bound to exact pre-state,
  parent, height, round, view, ordered batch, proposal, post-state root, and
  software/protocol version.
- [ ] Implement proposer-side reuse behind a disabled feature flag.
- [ ] Preserve current WAL and persistence semantics.
- [ ] Fall back to full deterministic execution on every mismatch, restart, or
  cache miss.
- [ ] Add differential receipt and state-root tests.
- [ ] Add stale-parent, conflicting-view, tampered-batch, tampered-QC, restart,
  and cache-eviction tests.
- [ ] Run a six-node local test with the feature both disabled and enabled.
- [ ] Measure proposer local-apply savings independently.
- [ ] Document the result and remaining safety work.
- [ ] Confirm the prototype was not enabled on the live fleet overnight.

#### H. End-of-session handoff

- [x] Record the final live height, fleet convergence, mempools, A666 supply,
  route invariant, reservations, and relevant private wallet state.
- [x] Record every source commit, artifact hash, deployment, rollback, and
  service restart performed during the session.
- [x] Link all test and live evidence.
- [x] List every failed or incomplete checkbox with its concrete blocker.
- [x] State whether the resident service is unchanged, improved but still
  limited-availability, qualified for 100/100, or rolled back.
- [x] State the single next action that should begin the following session.

Final classification: **improved but still limited-availability**. The live
resident service remains on the reviewed P0 build and is healthy. It is not
qualified for 100/100.

Single next action: add an authenticated, fail-closed governed StakeHub NAV
refresh step to the qualification runner, including six-validator convergence
and fresh-quote acquisition, then restart the ten-cycle gate from cycle 1.

### Phase 0: preserve the current known-good state

Before another live mutation:

- preserve the current mount-unit hardening change;
- record the current release/build fingerprints;
- record height, fleet convergence, supply, reservations, mempools, and wallet
  note state;
- confirm a recovery path for the currently private pfUSDC output.

Do not sweep unrelated evidence or deployment artifacts into a broad commit.

### Phase 1: implement the view-zero resident-service fix

Change only the resident service's finality-observer dependency loading:

- call the existing view-aware QC-graph helper using the commit proposal view;
- retain all exact commit and batch binding checks;
- add `processed_finality_verify_ns`;
- make no validator or consensus-state changes.

Required tests:

1. a view-zero commit succeeds with an empty dependency graph;
2. adding a very large irrelevant historical graph does not change view-zero
   work materially;
3. a nonzero-view commit still loads and verifies its required history;
4. tampered prepare QC is rejected;
5. tampered precommit QC is rejected;
6. signer, quorum, signature, proposal, parent, height, payload, and batch
   mutations are rejected;
7. the exact h508/h509-style commit-to-batch binding remains enforced;
8. journal and idempotency behavior is unchanged.

### Phase 2: qualify locally before touching the live service

Run:

- targeted unit and integration tests;
- `cargo check` for affected targets;
- affected clippy checks;
- deterministic replay/differential checks;
- a six-node local harness round for view zero;
- a forced nonzero-view round;
- restart between publication and observation.

No live rollout occurs if a security check is bypassed, a nonzero-view test
regresses, or deterministic receipts/state roots differ.

### Phase 3: deploy only the resident service

Deploy the new `pftl-swapd` build without a validator fleet upgrade.

The live sequence should be:

1. controlled private pfUSDC restoration/egress as required by the current note
   state;
2. one private issue;
3. exact supply, reservation, wallet, mempool, height, certificate, and six-node
   convergence check;
4. one private redeem;
5. the same exact postconditions;
6. compare the new finality-verification timer with h508/h509/h511/h512.

### Phase 4: go/no-go gate for a smoke campaign

Run ten consecutive private issue/redeem cycles only if:

- issue is below 45 seconds with practical margin, not a rounding victory;
- redeem remains below 45 seconds;
- proof time remains below 35 seconds;
- no unexplained retries or manual state edits occur;
- all six validators converge after every round;
- supply and reservation invariants pass every round;
- the private-output loop works unattended;
- a service restart has already recovered correctly.

Do not begin 100/100 merely because one issue measures 44.9 seconds. A useful
gate is a ten-cycle issue p95 at or below 42 seconds, leaving room for normal
variance.

### Phase 5: prototype prepared execution, but keep it off live consensus

If the P0 qualification passes early, use the remainder of the session to build
a proposer-side `PreparedShieldedExecution` prototype behind a disabled feature
flag.

The prototype should:

- carry exact pre-state and consensus bindings;
- commit through the current persistence/WAL path;
- fall back to normal execution on every mismatch or cache miss;
- produce state roots and receipts identical to current re-execution;
- remain local-only until crash, stale-parent, conflicting-view, tampered-batch,
  and six-node differential tests pass.

Do not deploy prepared-state reuse to the live fleet overnight.

## Go/no-go validation matrix

| Gate | Pass condition | Failure action |
|---|---|---|
| correctness | exact receipts, state roots, supply, reserve, and note accounting | stop; retain current service |
| view safety | view-zero and forced nonzero-view tests pass | stop; do not deploy |
| cryptographic binding | every mutated QC/signature/proposal/batch case rejects | stop; do not deploy |
| latency attribution | finality observer drops by about 3 s without moving cost elsewhere | continue |
| issue SLO | ten-cycle p95 at or below 42 s | continue diagnosis; no 100/100 |
| redeem SLO | ten-cycle p95 at or below 45 s | continue diagnosis; no 100/100 |
| restart | accepted/proved/published/committed crash points recover exactly once | continue |
| fleet | six validators converge after every round | stop campaign and diagnose |
| unattended wallet loop | no manual note-path or state edits | eligible for longer run |

## Expected latency envelope

These are planning estimates, not promises:

| State | Private issue | Private redeem |
|---|---:|---:|
| current measured | about 47.3 s | about 35.7–38.9 s |
| after resident view-zero fix | about 43.6–44.1 s | about 32–36 s |
| plus safe proposer prepared reuse | about 38–40 s | about 29–33 s |
| plus validator reuse and clone/verification work | low/mid 30s possible | high 20s possible |
| future circuit/prover architecture | 15–25 s plausible | 15–25 s plausible |

The existing p50 target of 20 seconds probably requires the future circuit/prover
layer as well as execution reuse. It should not be represented as an overnight
outcome.

## Work that should not consume the overnight session

Do not spend tonight on:

- replacing TCP with QUIC;
- changing the BFT phase structure;
- weakening validator independent execution;
- removing prepublication simulation without an exact prepared receipt;
- a GPU proving port without a pinned benchmark and fallback;
- public multi-wallet API design;
- raising capacity or queue limits;
- another 100/100 run before the ten-cycle and restart gates pass;
- Ethereum finality work, which is not the present private-swap bottleneck;
- changing proof or circuit semantics in the live protocol.

## Final recommendation

Use the overnight session to turn the current working private swap path into a
measured, repeatable path—not to redesign PFTL.

The immediate deliverable should be a small resident-service release that
removes unnecessary view-zero historical QC verification, adds exact stage
timing, and survives one controlled issue/redeem plus ten unattended cycles.

The next engineering milestone should be content-bound prepared execution reuse.
That is where the largest remaining safe PFTL latency reduction is likely to
come from. In parallel, the SSHFS/tunnel prover arrangement must be replaced by
a dedicated authenticated prover service with pinned proving keys, bounded
jobs, warm failover, and durable wallet journaling.

Only after those layers pass fault, restart, and 100/100 testing should the
private swap service be described as generally production-ready.

## Related internal material

- `docs/plans/PFTL-RESIDENT-PRIVATE-SWAP-SERVICE-SPEC-20260729.md`
- `docs/plans/CHAIN-OPTIMIZATION-STACKED-RESEARCH-20260729.md`
- `docs/plans/NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md`
- `docs/status/PFTL-RESIDENT-SWAP-LIMITED-AVAILABILITY-20260729.md`
- `docs/status/A666-CHAIN-OPTIMIZATION-RUN-REPORT-20260729.md`
- `crates/node/src/bin/pftl_swapd.rs`
- `crates/node/src/consensus_v2_store.rs`
- `crates/node/src/consensus_v2_finality.rs`
- `crates/node/src/execution_actions.rs`
- `crates/node/src/orchard_state_application.rs`
