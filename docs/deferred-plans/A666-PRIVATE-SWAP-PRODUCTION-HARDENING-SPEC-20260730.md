# A666 Private Swap Production Hardening Specification

**Date:** 2026-07-30
**Priority:** P0
**Status:** Deferred — unfinished and not current execution work
**Product:** Ethereum USDC -> pfUSDC -> newly issued A666 -> Ethereum wA666,
with transparent or private PFTL execution and the inverse redemption path
**Governing economics:**
`A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`
**Current service baseline:**
`PFTL-PRIVATE-SWAP-INFRASTRUCTURE-AND-LATENCY-DIAGNOSTIC-20260730.md`
**Current combined status:**
`../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md`
**Latest qualification evidence:**
`../evidence/pftl-private-swap-p0-20260730/README.md`

`MUST`, `MUST NOT`, `REQUIRED`, and declarative “must” statements are
normative. `SHOULD` is the default unless a dated exception records the risk,
owner, and expiry. A checkbox may be marked `[x]` only when its stated evidence
exists.

## 1. Executive objective

Ship a production-capable primary A666 market in which a user can:

1. deposit canonical Ethereum-mainnet USDC;
2. receive reserve-backed pfUSDC on PFTL;
3. create new A666 supply at the governed `1.005 × NAV` issue price without
   buying through the Uniswap curve;
4. choose a transparent or private PFTL middle;
5. receive wA666 directly on Ethereum for transfer or Uniswap use;
6. return wA666, retire A666 through the governed `0.9995 × NAV` redemption
   facility, and receive mainnet USDC; and
7. recover safely from every restart, timeout, stale quote, stale NAV,
   partial export, or duplicate request without value creation or loss.

Production hardening does not change the product’s economics:

- A666 has no permanent maximum supply.
- New reserve principal and new A666 supply grow together.
- Redemption retires supply and releases the corresponding reserve principal.
- The `2,000,000 A666` issue and redemption quantities are policy capacities,
  not pre-funded inventory requirements.
- Uniswap is a secondary venue, not the NAV oracle or primary issuance
  mechanism.
- A buyer funds primary issuance; the operator does not supply existing A666
  inventory.

## 2. Current truth

As of the final 2026-07-30 audit:

- all six PFTL validators were converged at height `528`;
- A666 authorized valid supply was `31,489,197,455` atoms;
- settlement reserve was `112,995,855` pfUSDC atoms;
- the route invariant passed;
- active reservations and all mempools were zero;
- the production wA666 contract and wA666/USDC Uniswap v4 pool were deployed;
- transparent and private primary issue/redeem transitions had committed;
- a complete mainnet private issue/export/return/redeem/withdrawal flow had
  functionally passed;
- private redemption completed within 25 minutes;
- canonical private issue took `1,776 seconds`, missing the 25-minute target by
  `276 seconds`;
- the PFTL-only resident private service completed four qualification cycles;
- its four-sample issue p95 was `50.365 seconds`;
- its four-sample redeem p95 was `38.862 seconds`; and
- cycle 5 stopped before publication because finalized reserve-proof pricing was
  stale.

The current resident service is limited to one controlled wallet, one active
swap, loopback access, and `1.000000 A666` per request. It is useful
limited-availability infrastructure, not a public or large-capacity product.

## 3. Release profiles

Production readiness is divided into profiles so one successful canary cannot
be mistaken for general availability.

| Profile | Permitted use | Required gate |
|---|---|---|
| LA0 | Existing controlled one-wallet, one-A666 canary | Already open; no broader claim |
| LA1 | Authenticated managed production pilot with bounded value and explicit custody disclosure | Sections 7–15 plus Gate P1 |
| GA | Multi-user, large-capacity, repeatable primary issue/redeem with user-facing workflow | Gate P2 |
| Non-custodial GA | User-controlled spending authority and recoverable private wallet state | Gate P3 |

LA1 MUST be labeled managed/custodial if the service controls transparent keys,
private note openings, or spending authority. Passing LA1 does not authorize a
non-custodial claim.

## 4. Binding economic and trust model

### 4.1 Canonical parameters

| Parameter | Binding value |
|---|---:|
| A666 precision | 6 |
| Permanent maximum supply | none |
| Issue price | `NAV × 1.005` |
| Redeem price | `NAV × 0.9995` |
| Issue capacity per active policy epoch | `2,000,000 A666` |
| Redemption capacity per active policy epoch | `2,000,000 A666` |
| Maximum primary order | `1,000,000 A666` |
| Ethereum export packet maximum | `250,000 A666` |
| Net wrapped exposure cap | `2,000,000 A666` |
| PFTL -> Ethereum verification | `TRUSTLESS_FINALITY` using the pinned SP1 verifier |
| Ethereum -> PFTL return verification | disclosed `BFT_CHECKPOINT` plus receipt inclusion |
| Uniswap pool | wA666 / canonical mainnet USDC, fee `500`, tick spacing `10` |

At NAV `$1.00`, issuing `100,000 A666` requires `100,500 USDC`. Issuing the
maximum `1,000,000 A666` requires `1,005,000 USDC`. A user depositing exactly
`1,000,000 USDC` receives the amount implied by the governed multiplier; the
system MUST NOT silently reinterpret a USDC deposit as an equal-number A666
order.

### 4.2 Trust boundaries

The product MUST disclose:

- Ethereum deposits, mints, burns, withdrawals, accounts, amounts, and timing
  are public.
- PFTL supply changes, route policy, commitments, nullifiers, and block timing
  are public protocol state.
- “Private” protects the PFTL note ownership/value path to the extent enforced
  by Asset-Orchard; it does not make Ethereum boundaries private.
- PFTL -> Ethereum export is verified by the immutable SP1-bound receipt
  verifier.
- Ethereum -> PFTL return currently depends on the disclosed PFTL validator
  checkpoint policy and is not equivalent to an Ethereum light client.
- Reserve-proof correctness does not prove an omitted asset or liability does
  not exist; it proves the declared reserve computation under the pinned
  source and proof policy.

No UI or API may collapse these boundaries into an unqualified “fully
trustless and private” claim.

## 5. Non-negotiable invariants

Every implementation and run MUST preserve:

1. **A666 conservation**

   ```text
   authorized_valid_supply
   = native_spendable
   + private_custody
   + outstanding_bridge_claims
   + other_registered_venue_supply
   + explicitly modeled in-flight terms
   ```

2. **pfUSDC conservation**

   Every PFTL pfUSDC unit is linked to canonical source USDC or an explicitly
   modeled pending/finalized bridge state. Issue spread and redemption spread
   are not reserve principal.

3. **Atomic primary issue**

   Reserve principal is consumed and A666 supply is created in one
   deterministic state transition. Neither side may commit alone.

4. **Atomic primary redemption**

   A666 supply is retired and redeemable settlement value is released in one
   deterministic state transition. No separate pre-funded redemption bucket
   is required.

5. **Single global supply**

   Exported A666 is not simultaneously spendable on PFTL. A return import
   cannot become native spendable until the corresponding wrapped claim is
   burned and proven under the route policy.

6. **Replay exclusion**

   Deposit IDs, note nullifiers, subscription nonces, reservations, export
   packets, proof receipts, return nonces, burn events, withdrawals, signed
   intents, and idempotency keys are each consumable at most once.

7. **Deterministic execution**

   The same ordered inputs and pre-state produce byte-identical receipts and
   state roots on every validator and after restart/replay. Consensus code must
   not use floating point, wall-clock time, process randomness, unordered map
   iteration, or nondeterministic parallel reduction.

8. **Fail-closed freshness**

   Stale NAV, stale quotes, changed policy, changed route, changed proof key,
   changed chain tip, and changed note frontier stop before publication.

9. **No manual state surgery**

   No production recovery may edit ledger, consensus, wallet, nullifier, note
   index, journal, receipt, bridge, or route files.

## 6. Architecture required for production

The production path consists of independently health-checked components:

```text
Ethereum watcher / finality source
  -> SP1 ingress proof service
  -> pfUSDC relay
  -> provider-neutral reserve-proof/NAV publisher
  -> authenticated swap API and durable workflow journal
  -> private wallet / note service
  -> resident proof service
  -> PFTL certified-round driver
  -> SP1 export proof service
  -> proof-gated Ethereum controller
  -> wA666 delivery / Uniswap venue
```

Each arrow is a typed, versioned, bounded interface. No component may infer
success from file existence, process liveness, or transaction submission
alone. Success requires the next component’s verified final state.

The current SSH tunnel/SSHFS-style remote prover arrangement MUST be replaced
before GA by a dedicated authenticated prover protocol with:

- mutual authentication and pinned server identity;
- request and response schema/version binding;
- exact circuit, params, proving-key, verifying-key, and build fingerprints;
- bounded request and response sizes;
- one durable request identity and content hash;
- bounded concurrency, queue depth, timeouts, and cancellation;
- no raw spending key or unrestricted filesystem path from the caller;
- response verification by the caller;
- warm primary and failover capacity; and
- structured private-safe metrics.

## 7. Phase A — Freeze and reconcile the current live state

- [x] Re-audit all six validators for exact height, tip, state root, release,
  topology, mempool count, supply, reserve, reservations, and bridge exposure.
- [x] Verify the live `pftl_swapd` binary and unit hashes against the recorded
  `pftl-finality-view0-8c73735` manifest.
- [x] Verify the height-528 cycle-5 journal lineage is terminal
  `FAILED_PREPUBLISH`, its pending output is `discarded`, and no publication
  artifact exists.
- [ ] Verify the egressed cycle-5 pfUSDC is represented exactly once in the
  controlled transparent balance and global conservation report.
- [ ] Freeze a signed, content-addressed checkpoint and independently import it
  into an isolated recovery directory.
- [ ] Record a rollback command that changes only the intended service and
  requires green readiness before re-admission.
- [ ] Publish a redacted baseline manifest; keep keys, note openings,
  nullifiers, signed private intents, and unrestricted paths outside the
  repository.

**Exit gate A:** six-node convergence and exact supply/reserve reconciliation;
no live, ambiguous, or unpublished value transition.

## 8. Phase B — Automate governed reserve-proof NAV freshness

The cycle-5 stale-pricing rejection proves freshness enforcement works and
that the runner is incomplete.

### 8.1 Required behavior

- [ ] Implement an authenticated NAV refresh worker that collects the governed
  source-adapter artifacts through the open reserve-proof kit and verifies
  their identity and freshness.
- [ ] Generate the next reserve packet and NAV mark using integer/fixed-point
  arithmetic only.
- [ ] Bind the packet to chain, genesis, protocol, asset, valuation policy,
  source set, observation interval, epoch, and prior finalized packet.
- [ ] Submit through the ordinary signed PFTL path; do not edit route or NAV
  state directly.
- [ ] Wait for six-validator convergence on the finalized NAV epoch and packet
  hash before allowing new quotes.
- [ ] Invalidate every unaccepted quote from an earlier NAV or policy epoch.
- [ ] Permit an already-published exact batch to finish only under its
  consensus-valid bindings; never rebuild it under new economics.
- [ ] Refresh proactively with a documented safety margin before expiry.
- [ ] Halt admission and alert if refresh fails; never extend freshness
  locally.
- [ ] Make duplicate packet publication idempotent and conflicting packets
  fail closed.

### 8.2 Tests

- [ ] Cross the NAV freshness boundary during an unattended campaign.
- [ ] Reject stale, future, skipped, duplicate, conflicting, wrong-source,
  wrong-policy, wrong-asset, wrong-chain, and invalid-proof packets.
- [ ] Restart before submission, after submission, and after finalization.
- [ ] Partition the publisher from a validator minority and majority.
- [ ] Prove identical state roots after deterministic replay.
- [ ] Record refresh lead time, publication latency, convergence latency, and
  remaining freshness margin.

**Exit gate B:** ten consecutive quote/issue attempts spanning at least one NAV
renewal without manual intervention or stale-price publication attempts.

## 9. Phase C — Harden the resident swap service

### 9.1 Admission and authorization

- [ ] Replace loopback trust with authenticated principals and explicit role
  policy.
- [ ] Require a signed intent bound to chain, genesis, protocol, route,
  direction, privacy mode, exact input, minimum output, maximum fee, quote,
  NAV epoch, policy hash, expiry height, destination, and idempotency key.
- [ ] Enforce per-principal, per-wallet, per-input, per-route, and global
  concurrency limits.
- [ ] Bound every identifier, body, collection, proof, path token, timeout,
  retry count, and response.
- [ ] Separate invalid/byzantine input, retryable dependency failure, and local
  fatal fault in durable error codes.
- [ ] Do not reveal whether another user owns a note, reservation, or request.
- [ ] Rate-limit by authenticated principal and source, with bounded metric
  cardinality.

### 9.2 Durable workflow and exact-once recovery

- [ ] Store one append-only workflow lineage from quote through terminal
  settlement.
- [ ] Persist intent acceptance before proof work.
- [ ] Persist proof identity before publication.
- [ ] Persist `PUBLISHED` before making the batch visible to the round driver.
- [ ] Resolve publication from certified chain state, not from process memory.
- [ ] Make every retry content-identical or require a newly signed intent.
- [ ] Supersede only terminal prepublication failures; never supersede an
  ambiguous or published input.
- [ ] Recover after `SIGTERM`, `SIGKILL`, OOM, disk-full, partial write, and
  dependency restart without double publication.
- [ ] Detect corrupt journals, halt admission, preserve forensic evidence, and
  restore only from a verified backup.

### 9.3 State anchoring

- [ ] Capture exact pre-state height, block, state root, Orchard root, route
  state, NAV epoch, and policy before proof construction.
- [ ] Revalidate all anchors immediately before publication.
- [ ] Expire and rebuild stale unaccepted work; never force it through.
- [ ] Verify exact certificate, proposal, parent, height, view, payload, batch,
  block hash, archive, signer, signature, committee, and quorum after
  publication.
- [ ] Preserve full nonzero-view QC dependency verification.

**Exit gate C:** every crash point recovers exactly once, every ambiguous state
halts safely, and the adversarial admission suite passes.

## 10. Phase D — Production private wallet and note lifecycle

- [ ] Encrypt note records and wallet metadata at rest with documented key
  derivation and rotation.
- [ ] Keep spending authority out of logs, metrics, evidence, command lines,
  environment dumps, prover requests, and repository files.
- [ ] Derive spendable/pending/spent/egressed/discarded status from chain data
  plus a durable local journal.
- [ ] Rescan encrypted outputs and nullifiers from a trusted checkpoint.
- [ ] Detect duplicate commitments, duplicate nullifiers, missing frontiers,
  wrong anchors, wrong pool domains, and corrupted ciphertext.
- [ ] Ensure failed prepublication outputs become unusable and auditable.
- [ ] Ensure committed private outputs are indexed exactly once.
- [ ] Prove private-output-to-next-input operation without manual note-index
  edits.
- [ ] Back up encrypted wallet state and rehearse restore on a clean host.
- [ ] Define custody explicitly:
  - LA1 may use service-controlled keys only with managed-custody disclosure,
    access controls, and withdrawal/recovery procedures.
  - Non-custodial GA requires user-held spending authority and a client wallet
    that can recover from chain ciphertext without server custody.
- [ ] Run a forbidden-material scan over API responses, journals, metrics,
  traces, logs, crash dumps, and public evidence.

**Exit gate D:** 100% note-state recovery in fault tests and zero private
material in public/operational telemetry.

## 11. Phase E — Dedicated proving infrastructure

- [ ] Define a pinned production hardware profile for the primary and failover
  prover.
- [ ] Prewarm all required proving and verifying material before readiness.
- [ ] Record circuit ID, `k`, params hash, proving-key hash, verifying-key hash,
  binary hash, CPU/GPU profile, and thread count.
- [ ] Enforce a bounded job queue and memory/CPU limits with admission
  backpressure.
- [ ] Isolate proof CPU work from API and consensus I/O threads.
- [ ] Make request cancellation safe without publishing partial output.
- [ ] Verify every remote proof locally before assembly.
- [ ] Reject stale cached proof responses by exact request and state binding.
- [ ] Deploy warm failover and prove takeover without duplicate work or key
  exposure.
- [ ] Rehearse key/artifact corruption, version mismatch, timeout, disconnect,
  slow proof, malformed response, and failover loss.
- [ ] Run a 24-hour sustained proof soak with RSS, queue depth, latency,
  failures, and artifact cache stability recorded.

**Performance gates:**

- private-primary proof DAG p95 `<= 35 seconds`;
- proof failure rate `< 0.1%` excluding intentionally invalid requests;
- cold readiness time measured and alerted;
- warm failover restores admission within the documented recovery SLO.

## 12. Phase F — PFTL latency and execution hardening

The view-aware finality observer is complete. Remaining work MUST preserve
independent validator verification.

- [ ] Profile issue and redeem with per-stage p50/p95/p99 measurements.
- [ ] Implement content-bound proposer prepared execution behind a disabled
  runtime gate.
- [ ] Bind prepared state to exact pre-state, parent, height, round, view,
  ordered batch, proposal, post-state root, receipts, build, and protocol.
- [ ] Reuse prepared execution only on an exact match.
- [ ] Fall back to full deterministic execution on restart, mismatch, cache
  miss, stale parent, changed view, eviction, or corruption.
- [ ] Preserve the existing WAL, atomic persistence, and state-root checks.
- [ ] Add differential tests proving byte-identical receipts and roots with
  reuse enabled and disabled.
- [ ] Add stale-parent, conflicting-view, tampered-batch, tampered-QC,
  restart, disk-full, and cache-eviction tests.
- [ ] Measure proposer savings separately from validator and observer cost.
- [ ] Consider validator prepared-state reuse only in a separate reviewed
  change after proposer reuse passes.
- [ ] Do not deploy a circuit, consensus, transport, or persistence redesign
  merely to improve one average measurement.

**PFTL-only performance gates:**

- private issue accepted-to-committed p95 `<= 42 seconds`;
- private redeem accepted-to-committed p95 `<= 45 seconds`;
- proof DAG p95 `<= 35 seconds`;
- observer verification p95 `<= 1 second`;
- no sample exceeds the request timeout;
- six-validator convergence after every committed operation.

## 13. Phase G — Large-order and concurrency qualification

The resident service cap MUST NOT jump from one A666 directly to production
capacity.

### 13.1 Amount ladder

Qualify primary orders in A666 units:

```text
1
100
1,000
10,000
100,000
250,000
1,000,000
```

At each level:

- [ ] Verify issue quote arithmetic and exact reserve/supply increase.
- [ ] Verify private and transparent output modes.
- [ ] Verify redemption arithmetic and exact reserve/supply decrease.
- [ ] Verify insufficient balance, capacity, reserve, fee, and expiry
  rejection.
- [ ] Verify six-node convergence and zero residual reservations/mempools.
- [ ] Verify proof, batch, body, journal, and evidence sizes remain bounded.
- [ ] Verify restart recovery at every durable state.

Live-value runs above the currently authorized canary require a separately
signed financial authorization specifying principal, gas, proof cost,
destinations, refund path, and maximum loss. Code or simulation evidence does
not authorize moving user or operator funds.

### 13.2 Packet splitting

A `1,000,000 A666` order exceeds the `250,000 A666` export packet cap.

- [ ] Split export into at least four independently replay-protected packets.
- [ ] Bind every packet to the same originating entitlement and total order.
- [ ] Prevent the sum of packets from exceeding the remaining entitlement.
- [ ] Make partial delivery resumable without reminting A666.
- [ ] Permit safe cancellation/refund only under the governed state machine.
- [ ] Prove duplicate, reordered, omitted, conflicting, and over-total packets
  cannot increase wrapped supply.
- [ ] Show the user delivered, pending, failed, and refundable quantities.

### 13.3 Concurrency ladder

- [ ] Run 1, 2, 4, 8, and the intended production concurrency.
- [ ] Serialize conflicting wallet, note, nullifier, reservation, and frontier
  dependencies.
- [ ] Prove disjoint jobs may execute concurrently without nondeterministic
  publication order.
- [ ] Enforce queue and per-principal limits under bursts.
- [ ] Demonstrate no starvation and bounded cancellation.

**Exit gate G:** the `100,000 A666` route is qualified before any
large-capacity claim; the `1,000,000 A666` route and four-packet delivery pass
before advertising the maximum order.

## 14. Phase H — Ethereum bridges and wA666 delivery

### 14.1 Mainnet preflight

Before each release or live-value campaign:

- [ ] Re-read Ethereum chain ID, finalized checkpoint, contract addresses,
  runtime code hashes, implementation slots, pause state, controller bindings,
  SP1 program vkeys, token supply, controller outstanding, packet caps, and
  consumed packet set.
- [ ] Re-read canonical USDC token and pfUSDC vault/verifier bindings.
- [ ] Verify PFTL route identifiers, code hashes, policy, NAV, supply, bridge
  claims, and return-import state against Ethereum.
- [ ] Fail closed on any mismatch; no report value is accepted as live truth
  without readback.

### 14.2 Ingress and export

- [ ] Preserve canonical Ethereum finality for the canonical lane.
- [ ] Continuously construct finality/witness scaffolding while waiting for
  finality.
- [ ] Precompute every SP1 component not dependent on the final checkpoint.
- [ ] Keep ingress and export proving resources warm.
- [ ] Make proof jobs content-addressed and resumable.
- [ ] Verify proofs locally before Ethereum submission.
- [ ] Require successful transaction receipt and final state readback, not
  transaction broadcast.
- [ ] Record destination consume on PFTL exactly once after Ethereum mint.

### 14.3 Return and withdrawal

- [ ] Burn exact wA666 under a unique return nonce.
- [ ] Verify receipt-local log inclusion under the disclosed return policy.
- [ ] Import the native claim exactly once.
- [ ] Redeem A666 and burn/withdraw pfUSDC through the canonical route.
- [ ] Prove exact replay rejection for burn, return import, redemption,
  withdrawal, and settlement.
- [ ] Reconcile Ethereum vault balance, wA666 supply, controller outstanding,
  PFTL bridge claims, A666 supply, and pfUSDC obligations.

### 14.4 Full-route latency

The canonical issue SLO remains deposit inclusion to spendable wA666 in no
more than 25 minutes under the supported envelope.

- [ ] Run at least ten clean small-value canonical issue samples.
- [ ] Require every qualifying sample and p95 to be `<= 1,500 seconds`.
- [ ] Record Ethereum finality, ingress proof, PFTL relay, private issue,
  export proof, Ethereum inclusion, and final readback separately.
- [ ] Do not weaken finality to pass this gate.
- [ ] If a faster risk-bearing lane is desired, specify it separately with
  explicit caps, pricing, fronted liquidity, reconciliation, and user
  disclosure. It must not silently replace the canonical lane.

## 15. Phase I — Uniswap venue hardening

The pool is not part of primary execution, but users receiving wA666 must be
able to identify and use the correct venue safely.

- [ ] Re-read the canonical PoolManager, pool ID, currencies, fee, tick
  spacing, hooks, current liquidity, current tick, and initialized state.
- [ ] Verify the wA666 and USDC runtime code hashes.
- [ ] Verify all temporary Permit2, router, PositionManager, and ERC-20
  allowances are zero unless a documented active operation requires them.
- [ ] Monitor liquidity withdrawal, abnormal tick movement, swap failures, and
  wrong-token/wrong-pool routing.
- [ ] Expose the canonical pool ID in the wallet and API.
- [ ] Never use the pool spot or TWAP as the primary NAV oracle.
- [ ] Deliver wA666 directly to the user; a secondary swap is optional and
  separately authorized.
- [ ] Do not promise a large secondary exit merely because a large primary
  issue is supported. Show current venue liquidity and expected secondary
  slippage separately.

**Exit gate I:** direct wA666 delivery works regardless of pool depth, and an
optional small secondary swap passes against fresh on-chain pool readback.

## 16. Phase J — Observability, security, and operations

### 16.1 Metrics and tracing

- [ ] Emit bounded metrics for admission, queueing, proof stages, publication,
  proposal, votes, certificate, apply, convergence, bridge proofs, Ethereum
  inclusion, NAV freshness, and total workflow latency.
- [ ] Record p50, p95, p99, maximum, queue depth, failure class, and retry
  class.
- [ ] Expose `/healthz`, `/readyz`, `/metrics`, and `/version` on authenticated
  or appropriately isolated interfaces.
- [ ] Use structured logs and one trace/workflow ID across components.
- [ ] Never label metrics with wallet IDs, note references, nullifiers,
  transaction hashes, paths, proofs, signatures, or unbounded error text.

### 16.2 Alerts

- [ ] NAV refresh margin below threshold.
- [ ] Validator height/root divergence or peer count below quorum.
- [ ] Mempool or reservation residue beyond timeout.
- [ ] Proof queue saturation, prover unavailable, or artifact mismatch.
- [ ] Journal corruption, disk below 20%, fd pressure, memory growth, or OOM.
- [ ] Bridge/code-hash/pause/controller mismatch.
- [ ] Supply or reserve invariant failure.
- [ ] Uniswap pool identity or liquidity anomaly.
- [ ] Workflow stuck beyond its stage deadline.

Every alert MUST have an owned runbook and a tested safe response.

### 16.3 Service and key operations

- [ ] Run services as unprivileged identities with minimal read/write paths.
- [ ] Replace root-installed operational artifacts with staged,
  owner-verified atomic deployment.
- [ ] Store signing keys in a dedicated signer/HSM or document the temporary
  managed-custody exception.
- [ ] Enforce single-active signer semantics and backup signing state.
- [ ] Rotate API credentials and rehearse revocation.
- [ ] Capture encrypted backups of journals, wallet state, route manifests,
  and signed checkpoints.
- [ ] Rehearse restore on a clean host.
- [ ] Run controlled rolling upgrades with one-validator-at-a-time convergence
  checks and a frozen rollback artifact.

## 17. Phase K — Test and qualification matrix

### 17.1 Required code gates

- [ ] `cargo fmt --all -- --check`
- [ ] affected `cargo check`
- [ ] affected clippy with warnings denied
- [ ] focused unit and integration tests
- [ ] full relevant workspace regression suite
- [ ] deterministic replay and state-root differential tests
- [ ] property tests for supply, reserve, idempotency, and packet totals
- [ ] fuzzing for API, intent, quote, proof, packet, certificate, and journal
  parsers
- [ ] concurrency model tests for journal/publication and note-index updates
- [ ] six-node view-zero and forced nonzero-view runs
- [ ] state snapshot import, replay, downgrade, and interrupted-upgrade tests
- [ ] dependency and license audit
- [ ] reproducible release build with signed checksums

Consensus-affecting changes require a dated amendment/ADR, protocol-version
analysis, conformance vectors, deterministic simulation, and a separately
reviewed rollout. Runtime feature flags alone may not select different
consensus rules.

### 17.2 Fault matrix

Inject failure:

- [ ] before and after intent journaling;
- [ ] during each proof stage;
- [ ] after proof but before simulation;
- [ ] during simulation;
- [ ] after prepared persistence;
- [ ] after `PUBLISHED` but before outbox rename;
- [ ] after outbox visibility but before proposal;
- [ ] during proposal, prepare, precommit, certificate, and local apply;
- [ ] after local apply but before remote convergence;
- [ ] during NAV publication;
- [ ] during SP1 proof generation and Ethereum submission;
- [ ] after Ethereum receipt but before PFTL destination consume;
- [ ] during return import, redemption, and withdrawal;
- [ ] under disk-full, corrupt file, network partition, slow peer, Byzantine
  peer, process kill, host loss, and prover loss.

Each case must end in exactly one of:

- committed exactly once;
- rejected before mutation;
- safely pending with deterministic recovery;
- canceled/refunded through the governed path; or
- halted with funds and state provably accounted for.

### 17.3 Campaigns

- [ ] 10 consecutive private issue/redeem cycles with zero intervention.
- [ ] 100 private issues and 100 private redeems, each exactly once.
- [ ] Equivalent transparent-output campaign.
- [ ] Mixed transparent/private campaign.
- [ ] Bounded-burst and concurrency campaign.
- [ ] NAV-refresh-boundary campaign.
- [ ] Large-order and packet-splitting campaign.
- [ ] Canonical Ethereum roundtrip latency campaign.
- [ ] 24-hour service and prover soak.
- [ ] Restart/upgrade/rollback campaign.

## 18. Phase L — User-facing product

- [ ] Provide one quote endpoint returning governed economics, NAV age, policy
  capacity, maximum order, estimated gas/prover cost, privacy boundary, and
  expiry.
- [ ] Provide one signed-intent submission endpoint.
- [ ] Return durable workflow state: accepted, proving, prepared, published,
  committed, exporting, delivered, redeeming, withdrawing, completed,
  recoverable, or failed.
- [ ] Distinguish PFTL finality from Ethereum finality and token delivery.
- [ ] Show transparent versus private PFTL modes accurately.
- [ ] Show direct wA666 delivery separately from optional Uniswap trading.
- [ ] Show packet-split progress for large exports.
- [ ] Provide cancellation/refund only where the governed state machine allows
  it.
- [ ] Provide chain-derived transaction, proof, certificate, and conservation
  evidence without exposing private wallet material.
- [ ] Document custody, trust, privacy, latency, capacity, and recovery
  limitations in the UI and API.
- [ ] Publish operator and user recovery runbooks.

## 19. Release gates

### Gate P1 — Managed production pilot

All must pass:

- [ ] Phases A–F complete.
- [ ] Ten-cycle private campaign passes unattended.
- [ ] Private issue p95 `<= 42 seconds`.
- [ ] Private redeem p95 `<= 45 seconds`.
- [ ] Automated NAV refresh crosses a renewal boundary.
- [ ] Restart and fault matrix passes for the resident PFTL path.
- [ ] Authenticated API, custody disclosure, monitoring, backup, and rollback
  are live.
- [ ] Pilot amount and concurrency caps are explicit and no greater than the
  largest qualified values.

### Gate P2 — General availability and large capacity

All must pass:

- [ ] Gate P1 complete.
- [ ] 100/100 private campaign passes.
- [ ] Transparent and mixed-mode campaigns pass.
- [ ] `100,000 A666` primary issue/redeem and direct wA666 delivery pass.
- [ ] Maximum advertised order and packet splitting pass.
- [ ] Canonical mainnet issue SLO passes.
- [ ] Full reverse redemption/withdrawal passes cleanly.
- [ ] Bridge, Uniswap, chaos, soak, upgrade, and rollback gates pass.
- [ ] User-facing workflow and evidence package pass.
- [ ] Every Critical/High security finding is closed; every accepted residual
  risk has an owner and expiry.

### Gate P3 — Non-custodial claim

All must pass:

- [ ] User retains spending authority.
- [ ] Server cannot independently move user funds or private notes.
- [ ] Client can recover note state from chain ciphertext plus user backup.
- [ ] Prover interface does not receive unrestricted spending authority.
- [ ] Two independent fresh-user wallet recoveries and roundtrips pass.
- [ ] Threat model and external review cover client, prover, API, bridge, and
  recovery paths.

## 20. Stop and rollback rules

Stop admission immediately on:

- supply, reserve, or cross-venue invariant mismatch;
- validator state-root divergence;
- unexpected bridge/code-hash/controller change;
- stale NAV without successful renewal;
- ambiguous publication or double-spend evidence;
- corrupted journal, wallet index, checkpoint, or proof artifact;
- private key/note material exposure;
- proof verifier mismatch or invalid proof acceptance;
- repeated timeout beyond the frozen error budget; or
- any unclassified value transition.

Rollback is permitted only when:

1. no incompatible consensus state has committed;
2. the exact previous signed artifact and config are available;
3. current journals and wallet state are backed up;
4. the rollback target passes offline verification;
5. the affected service is drained;
6. post-rollback readiness, state, supply, and convergence pass; and
7. every user workflow is reconciled.

If a protocol/state migration has committed, recovery must use a forward fix or
governed downgrade designed for that state. Replacing binaries does not undo
chain state.

## 21. Evidence contract

Every gate produces a redacted, content-addressed evidence index containing:

- source commit and clean-tree status;
- build toolchain, feature tree, command, artifact hash, and signature;
- validator, service, circuit, VK, SP1 program, contract, and pool
  fingerprints;
- pre/post fleet state and conservation report;
- signed campaign manifest;
- request and workflow public summaries;
- certificate and transaction references;
- per-stage latency distribution;
- fault/restart results;
- proof and replay-negative results;
- deployment and rollback record;
- private-material scan; and
- explicit `PASS`, `FAIL`, `RECOVERY_REQUIRED`, or `NO-GO`.

Raw keys, seeds, note openings, unrestricted paths, signatures, proofs, and
private signed intents remain in restricted operational storage. A report is
not evidence unless its machine-verifiable source artifacts exist.

## 22. Immediate implementation order

This is the authoritative next-work checklist:

- [ ] **P0.1:** Reconcile and freeze the current height-528 state.
- [ ] **P0.2:** Implement and test automated governed provider-neutral reserve
  proof/NAV refresh.
- [ ] **P0.3:** Make the campaign runner wait for exact prover, service,
  round-driver, and six-validator mirror convergence at every boundary.
- [ ] **P0.4:** Complete ten unattended one-A666 private cycles.
- [ ] **P0.5:** Implement and qualify content-bound proposer prepared execution
  until issue p95 is at or below 42 seconds.
- [ ] **P0.6:** Run the complete restart/fault matrix.
- [ ] **P0.7:** Replace the tunnel/mount prover path with an authenticated,
  bounded, warm primary/failover prover service.
- [ ] **P0.8:** Open the authenticated managed pilot at the largest amount and
  concurrency actually qualified.
- [ ] **P1.1:** Execute the amount ladder through `100,000 A666`.
- [ ] **P1.2:** Qualify export packet splitting and then the maximum advertised
  order.
- [ ] **P1.3:** Pass 100 private issues and 100 private redeems.
- [ ] **P1.4:** Make the canonical Ethereum issue path pass the 25-minute SLO.
- [ ] **P1.5:** Ship the user workflow and open Gate P2 only after all
  production evidence passes.

The single next action is **P0.1 followed immediately by P0.2**. No further
latency or scale campaign should begin with an already stale NAV epoch or an
unreconciled baseline.
