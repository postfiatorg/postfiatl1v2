# Open-Source Capability Restoration Plan

**Date:** 2026-07-16
**Scope:** repair every productionization finding that was contained by removing,
disabling, hiding, or deployment-blocking a core capability
**Status:** implementation plan; containment is not closure

## 1. Non-negotiable outcome

The productionization program must produce a safer, fully functional blockchain.
It must not obtain a green checklist by deleting the product.

From this point forward:

1. A core capability is not `FIXED` merely because its RPC, UI, state transition,
   consensus mode, or deployment path is disabled.
2. `disabled`, `historical-replay-only`, `feature-contained`, `deployment
   blocked`, and `live-path removed` are temporary risk controls, not closure.
3. A finding closes only when the intended capability works through its real
   public boundary, its safety invariant is enforced, adversarial regressions
   pass, replay/migration is proven, and the integrated release battery is green.
4. No capability may be default-disabled as a substitute for solving its defect.
5. Tests may not be deleted or weakened to obtain a pass.
6. Protocol encodings, activation rules, and migration behavior must be explicit
   and versioned. No permissive legacy fallback is allowed on a live path.
7. FastPay remains a supported default capability. Its unresolved cancellation
   problem is an open engineering task, not grounds to hide the lane.

## 2. Corrected blocker accounting

The following dispositions are not complete fixes:

| Finding | Capability affected | Temporary containment currently present | Required real closure |
|---|---|---|---|
| `P0-CONSENSUS-01` | multi-view BFT progress | production rejects every nonzero view | implement safe view change and the claimed commit rule |
| `P0-GOVERNANCE-01` | live governance and validator rotation | unsigned live governance is rejected and no signed replacement is available | implement signed, registry-bound governance end to end |
| `P0-BRIDGE-01` | PFTL/Ethereum bridge and refunds | external transitions are historical-replay-only | implement authenticated finality/inclusion and mutually exclusive consume/refund |
| `P0-WALLET-BRIDGE-DEST-01` | wallet bridge deposits | route remains unavailable without an exact address/code-hash binding | ship governed route discovery and proof-bound wallet execution |
| `P0-SUPPLY-01` | Ethereum settlement-backed mint release | deployment is blocked without a concrete verifier | implement and deploy the real settlement verifier contract |
| `P1-FASTPAY-01` | FastPay owned payments | default availability and exact certificate-domain binding are restored; safe cancellation remains open | implement safe expiry/cancellation without regressing the fast path |

The bridge-related P0s are one integrated delivery program, not three unrelated
checkboxes. The legacy cleartext privacy-v1 decoder and the arbitrary-debit
development RPC are not core capabilities: they remain unavailable live because
secure replacement paths exist.

## 3. Execution order

Work proceeds continuously in the following order. Each workstream starts with a
real-boundary failing regression, then implementation, then targeted and
integrated gates. Documentation records evidence; documentation alone never
advances a gate.

### Phase 0 — correct the FastPay regression and the audit truth

**Objective:** ensure the productionization branch does not change FastPay from
available to unavailable by default.

Code boundaries:

- `crates/node/src/main_parts/cli_dispatch_parts/group_03.rs`
- `crates/node/src/rpc_cli.rs`
- `crates/node/src/main_parts/runtime_helpers.rs`
- `wallet-web/src/lib/rpc-client.js`
- `wallet-web/src/components/Send.jsx`
- `wallet-web/src/components/WalletHome.jsx`
- FastPay RPC and wallet regression tests

Actions:

1. Add a regression that starts the normal RPC server without experimental
   flags and proves `server_info.rpc.owned_lane_enabled == true` and that signed
   `owned_sign`, `owned_apply`, `owned_unwrap_sign`, and `owned_unwrap_apply`
   reach their authenticated validation boundaries.
2. Remove the default-off behavior introduced by the audit branch. A flag may
   explicitly disable FastPay for emergency operations, but ordinary startup
   must expose the lane.
3. Restore the wallet FastPay control whenever the server reports the default
   capability. Do not restore unsafe unsigned `wrap_owned`; funding uses the
   signed consensus-admitted deposit path.
4. Keep `owned_safe_unlock` fail-closed until Phase 3 supplies a safe protocol.
   This avoids a double spend while the real fix is built, but is recorded as an
   open liveness defect.
5. Relabel `P1-FASTPAY-01` as `OPEN-IMPLEMENTATION`; remove language claiming
   default-disablement is remediation.

Acceptance:

- FastPay is available under the normal server invocation.
- Existing signed-envelope admission, distinct-validator certificate checks,
  durable lock-before-sign, and payment tests remain green.
- No live fleet change is part of this local phase.

### Phase 1 — implement production multi-view consensus

**Objective:** replace the nonzero-view rejection with one coherent, durable BFT
state machine that provides both safety and liveness.

Primary code boundaries:

- `crates/node/src/block_finality.rs`
- `crates/node/src/consensus_artifacts.rs`
- `crates/node/src/batch_snapshot.rs`
- `crates/node/src/storage_commit.rs`
- `crates/ordering_fast/src/lib.rs`
- consensus wire types under `crates/types/`

Protocol work:

1. Define a versioned proposal envelope binding chain/genesis domain, height,
   view, block ID, parent ID, parent state root, proposer, and a resolved
   justification QC or timeout certificate.
2. Define QCs and timeout certificates over canonical bytes. Every vote binds
   validator ID, committee epoch/root, height, view, proposal ID, and phase.
   Count distinct registered validators only.
3. Replace opaque `high_qc_id` ordering with a verified QC graph. A timeout vote
   carries its highest verified QC; a timeout certificate selects the highest
   verified QC by protocol view/height, never lexicographic text.
4. Persist per-validator consensus safety state atomically before signing:
   `highest_voted_view`, `locked_qc`, `high_qc`, committee epoch, and the exact
   signed proposal digest. Crash recovery must reconstruct the same state.
5. Enforce the safe-vote rule: a proposal must extend the locked branch or be
   justified by a strictly higher verified QC that safely unlocks it.
6. Implement one explicit commit rule. If the whitepaper retains two-chain
   HotStuff, a certified child commits its certified parent; a lone certificate
   cannot directly commit while this mode is active.
7. Add a versioned activation height. Pre-activation history replays under its
   original rules; post-activation blocks cannot use the old direct-cert/live
   path. Activation requires a complete committee and initialized safety state.
8. Add deterministic state transfer for lagging validators and reject conflicting
   finalized histories before rejoin.

Required failing tests before implementation:

- the existing cross-view conflicting-vote counterexample;
- fabricated/unresolved high-QC timeout certificate;
- two conflicting QCs across views under delay and restart;
- crash between durable vote reservation and signature return;
- lone-QC commit while two-chain mode is active;
- stale committee and wrong-domain votes.

Required positive/adversarial proof:

- model/property test for quorum intersection, lock monotonicity, and commit
  uniqueness for `n=4` and `n=6`;
- deterministic simulations with delay, drop, duplication, reorder, proposer
  failure, partitions, Byzantine equivocation, and node restart;
- failed view advances and commits without violating safety;
- replay from genesis, snapshot restore, catch-up, and activation-boundary tests;
- six-node local integration: repeated proposer failure followed by recovery,
  identical committed roots, and zero divergent commits.

Closure criterion: nonzero views are enabled by default, the adversarial suite is
green, and the exact whitepaper rule is implemented—not merely described.

### Phase 2 — implement authenticated governance and validator rotation

**Objective:** make governance usable without trusting caller-supplied validator
names.

Primary code boundaries:

- `crates/types/src/shielded_bridge_governance.rs`
- `crates/consensus_cobalt/src/internal_validation.rs`
- `crates/consensus_cobalt/src/trust_graph_governance.rs`
- `crates/consensus_cobalt/src/dabc_registry.rs`
- `crates/node/src/governance.rs`
- `crates/node/src/consensus_artifacts.rs`
- `crates/node/src/execution_actions.rs`
- `crates/node/src/state_commitment.rs`

Protocol work:

1. Replace bare `GovernanceVote { validator, accept }` authorization with a
   versioned signed vote envelope.
2. Domain-separate and bind every vote to chain/genesis, governance protocol
   version, proposal kind and complete payload hash, proposal sequence/slot,
   old registry root, committee/key epoch, validator ID, activation height, and
   expiration height.
3. Verify the signature against the old active registry at admission, proposal
   construction, block validation, execution, replay, and state verification.
4. Deduplicate validator IDs and calculate the threshold from the old active
   committee. Reject missing, duplicate, stale, wrong-domain, wrong-registry,
   unknown-key, and post-expiry votes without mutation.
5. Make registry rotation explicitly old-rules-authorize-new-rules. Activate new
   keys only after the certified amendment commits and its activation delay
   passes. Preserve a bounded overlap solely for verifying pre-activation
   artifacts.
6. Sign and verify authoritative RBC/ABBA messages if they remain part of the
   production governance claim; otherwise replace their call sites with the
   signed governance envelope rather than keeping an unsigned parallel path.
7. Version state-root commitments and historical decoders so old blocks replay
   without permitting unsigned live amendments.

Tests:

- preserve the current no-key forgery as a pre-fix reproduction;
- missing/duplicate/wrong-domain/wrong-chain/wrong-slot/stale-key/stale-registry
  and altered-payload votes reject with no state change;
- old committee authorizes a new committee, new committee cannot pre-activate,
  old committee cannot authorize post-activation changes;
- pause/unpause, protocol activation, FastSwap policy, and registry rotation all
  execute through the same signed boundary;
- partition/restart/replay and concurrent-amendment ordering tests;
- six-node integration proves one amendment and one rotation converge exactly.

Closure criterion: the public RPC and block path can execute a real signed
amendment and rotation; the genesis registry is no longer permanently fixed.

### Phase 3 — fix FastPay cancellation without sacrificing the fast path

**Objective:** preserve sub-second normal payments while making abandoned locks
recoverable without allowing a delayed certificate to double spend an unlocked
object.

Primary code boundaries:

- owned-object/order/certificate types under `crates/types/`
- `crates/execution/src/owned_transfer.rs`
- `crates/node/src/consensus_artifacts.rs`
- `crates/node/src/tests/fastpay_payment_safety.rs`
- wallet/proxy FastPay routing and certificate outbox code

Protocol design to implement and model first:

1. Give each owned object a monotonic `object_version`; each lock has a unique
   `lock_id`, canonical order digest, committee epoch/root, and bounded validity
   window tied to finalized chain height.
2. Persist the lock and the validator's vote atomically before returning any
   signature. A validator may sign only one order digest for an object version.
3. Define a decision certificate domain distinct from a prepare/lock vote. A
   transfer is final only under the specified quorum decision certificate; a
   collection of preparatory locks is not user-visible finality.
4. Disseminate votes and assembled certificates through durable validator
   outboxes and retrieval RPCs so broker withholding cannot make existing
   evidence undiscoverable.
5. Implement cancellation as a version/fence transition ordered against the
   finalized chain-height validity window. The atomic state transition either:
   applies the valid payment decision and consumes version `v`, or records a
   certified cancellation fence and advances to `v+1`. It can never do both.
6. Reject every delayed certificate whose object version, lock ID, committee
   epoch, decision domain, or validity window is no longer current.
7. Make apply/cancel crash atomic using the durable store transaction/WAL. A
   restart must expose the same consumed-or-cancelled result.
8. Keep the normal prepare/decision/apply path consensusless. Only abandoned
   lock recovery may use the finalized base-chain height as an ordering/fencing
   oracle; ordinary payment latency must not wait for a new consensus block.

The exact confirm/cancel state machine must pass a small exhaustive model before
Rust implementation. The model must include a withheld broker, delayed vote,
delayed complete certificate, partial locks, expiry-boundary races, partition,
restart, Byzantine minority, and committee rotation. If the model finds a trace
where both payment and cancellation become valid, revise the protocol; do not
disable FastPay and do not ship the unsafe state machine.

Tests and performance gates:

- reproduce the late-certificate-after-unlock race against the old semantics;
- prove payment/cancel mutual exclusion for all modeled schedules;
- prove an abandoned partial lock becomes spendable after bounded recovery;
- prove a valid completed payment cannot be cancelled;
- prove duplicate/replayed/wrong-version/wrong-epoch certificates fail;
- crash injection at every lock, vote, decision, apply, cancel, and version-write
  boundary;
- `n=6`, Byzantine `f=1`, delayed/reordered/partitioned network simulation;
- real wallet wrap/deposit, send, receive, cancel-abandoned-lock, and unwrap;
- warm send latency must not regress from the established FastPay baseline.

Closure criterion: FastPay is enabled by default, normal payments remain fast,
and safe unlock is operational rather than permanently fail-closed.

### Phase 4 — implement the bridge and settlement verifier as one system

**Objective:** restore live PFTL/Ethereum movement with cryptographic evidence,
exact supply conservation, and no consume/refund race.

Primary code boundaries:

- `crates/bridge/src/lib.rs`
- `crates/execution/src/nav_vault_asset_execution.rs`
- `crates/execution/src/nft_escrow_asset_execution.rs`
- `crates/node/src/market_bridge.rs`
- `crates/node/src/vault_bridge_workflows.rs`
- bridge transaction/types under `crates/types/`
- `crates/ethereum-contracts/src/MintController.sol`
- `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol`
- `wallet-web/src/components/Bridge.jsx`
- wallet proxy bridge configuration/readiness code

#### 4A. Ethereum-to-PFTL evidence

1. Add governed Ethereum checkpoint state binding chain ID, finalized block
   number/hash, checkpoint authority epoch, and the exact bridge contract/token
   addresses and runtime code hashes.
2. Verify canonical receipt/log inclusion against the checkpointed header. Bind
   event signature, emitter, token, amount, sender, recipient, route, deposit
   nonce, transaction index, log index, and block hash.
3. Consume each deposit event exactly once. Reorged, non-final, wrong-chain,
   wrong-contract, wrong-code-hash, malformed-trie, duplicate, or altered events
   reject before mutation.
4. Make the trust model explicit. If the first implementation uses a governed
   checkpoint committee rather than an Ethereum light client, require distinct
   threshold signatures, rotation, slashable identities, and an exact public
   disclosure. Do not label it trustless.

#### 4B. PFTL-to-Ethereum settlement evidence

1. Implement the concrete `IMintSettlementVerifier`, not a mock or
   owner-assertion adapter. It must validate the selected PFTL finality proof or
   a precisely disclosed threshold checkpoint over the PFTL state/receipt root.
2. Bind chain/genesis, contract/controller, route, pending/escrow ID,
   beneficiary, token, exact amount, nonce, finalized height/root, and proof
   digest.
3. Pin verifier/controller/token addresses and runtime code hashes in governed
   route state and wallet readiness responses.
4. Make verifier replacement timelocked, code-hash-bound, and impossible while
   unresolved escrows exist unless a separately authorized migration proves all
   pending obligations.

#### 4C. Consume/refund mutual exclusion

1. Replace independent timeout checks with a single versioned bridge packet
   state machine: `Created -> FinalizedSource -> ConsumedDestination` or
   `Created -> CancelAuthorized -> RefundedSource`.
2. A refund requires cryptographic cancellation evidence that permanently
   prevents destination consumption. Elapsed local height alone is never enough.
3. The same packet/nonce key is committed on both sides. Consume and cancel are
   mutually exclusive and replay-protected across restarts and relayers.
4. Every accepted transition updates the aggregate public/private/FastLane/
   external supply oracle. Conservation is checked before commit and replay.

#### 4D. Wallet restoration

1. Discover the governed active route through authenticated chain state.
2. Require exact chain, address, runtime code hash, checkpoint freshness,
   finality mode, caps, fees, recipient, and amount in the pre-sign display.
3. Build/fetch inclusion evidence automatically; users must not paste trusted
   assertions.
4. Show pending, accepted, rejected, refundable, refunded, and unknown receipt
   semantics without treating convergence alone as success.

Bridge tests:

- malformed and adversarial MPT/header/log proofs;
- reorg and finality-depth boundaries;
- wrong chain/address/code hash/topic/token/amount/recipient/nonce;
- duplicate, replay, replacement, partial, front-run, and proof substitution;
- consume/refund races under message delay and restart;
- Foundry unit, invariant, fuzz, and pinned-mainnet-fork tests;
- Rust replay, crash, supply, and six-node convergence tests;
- two fresh wallets complete deposit, public/private swap, withdrawal, and refund
  paths with exact receipt-code and conservation checks.

Closure criterion: bridge, wallet deposit, and settlement-backed issuance are
live-capable under one reviewed protocol; the three bridge P0 entries close
together.

### Phase 5 — integrated migration, compatibility, and release proof

1. Assign activation versions/heights for consensus, governance, FastPay object
   versions, bridge packets, state roots, receipts, and checkpoints.
2. Generate deterministic old/new conformance vectors and reject cross-version
   ambiguity at live admission.
3. Replay all retained history byte-identically up to each activation boundary;
   verify new blocks under only the new rules.
4. Run upgrade, rollback-before-activation, crash-at-activation, lagging-node
   catch-up, snapshot restore, and mixed-binary refusal drills.
5. Run the exact integrated gates:
   - `cargo fmt --all -- --check`
   - `cargo check --workspace --all-targets --locked`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --all-targets --locked`
   - documentation, secret/history, artifact, determinism, replay, and SBOM gates
   - wallet and proxy regressions/build/audits
   - offline and pinned-fork Foundry suites
   - consensus/FastPay/bridge deterministic simulations and crash campaigns
6. Build a clean immutable candidate and rerun the complete battery from that
   exact tree. Record command, toolchain, result, duration, tree, and commit.
7. Update the productionization register. A capability-restoration finding may
   be `FIXED` only when its positive end-to-end acceptance test is present; a
   negative test proving the feature is unavailable is not closure evidence.

## 4. Commit and isolation strategy

Use separable commits so a safety regression can be bisected:

1. restore FastPay default surface and correct audit classification;
2. consensus types/model;
3. consensus durable state and view change;
4. consensus activation/replay;
5. governance signed types/verification;
6. governance activation/rotation;
7. FastPay model and versioned decision types;
8. FastPay durable apply/cancel implementation;
9. bridge checkpoint and Ethereum inclusion verifier;
10. PFTL settlement verifier contract;
11. bridge consume/refund state machine and supply integration;
12. wallet/proxy route discovery and complete UX;
13. integrated migrations and release evidence.

Do not combine unrelated formatting or bloat cleanup with these commits. Do not
deploy a protocol transition until its activation/replay gates are complete.

## 5. Definition of done

This plan is complete only when all of the following are true:

- FastPay is enabled by default and safe unlock works.
- consensus progresses through failed views without conflicting commits.
- signed governance can amend policy and rotate the validator registry.
- the PFTL/Ethereum bridge verifies real finality/inclusion evidence and cannot
  both consume and refund one packet.
- the real settlement verifier backs every Ethereum mint release.
- the wallet can execute all restored capabilities without hidden operator
  assertions or hardcoded unsafe destinations.
- every operation is proven at its real RPC/UI/contract boundary with accepted
  receipt codes, deterministic replay, and conservation.
- no P0/P1 is marked fixed solely because a core capability is absent.
- the immutable public candidate passes the complete release battery with zero
  open P0/P1 implementation findings.
