# PostFiat L1 v2 Roadmap

Status: controlled-testnet roadmap
Date: 2026-05-13
Primary reference: [docs/whitepaper.md](docs/whitepaper.md)
Working burndown: [docs/status/controlled-testnet-burndown.md](docs/status/controlled-testnet-burndown.md)

This roadmap converts the whitepaper and research-review conclusions into the
execution plan from the current MVP to a defensible controlled testnet. It is a
milestone document, not a marketing document: each row should map to concrete
repo changes, runnable evidence, and an exit condition.

## Where We Are Now

PostFiat L1 v2 has a real controlled-testnet MVP foundation, not a finished
production L1. The repo demonstrates deterministic ledger execution, ML-DSA
authorization, Cobalt-style validator governance scaffolding, HotStuff-family
ordering evidence, local and remote validator harnesses, RPC/SDK surfaces,
release tooling, shielded-state plumbing, and bridge-simulation plumbing.

The implementation has moved past the original all-validators-sign scaffold.
The node now accepts and verifies `2f+1` block certificates, records signed
proposals, votes, timeout certificates, and equivocation evidence, routes remote
rounds through deterministic proposers, survives a 3-of-4 partial-outage and
recovery drill, passes a remote P0 gate through proposer-routed normal
`postfiat-node run --mode peer-certified` execution, restarts peer-certified
validator service epochs, catches lagging validators up from RPC evidence,
admits already-signed external transfers through bounded CLI/SDK/local-RPC and
opt-in remote-RPC paths, and produces replay-verifiable validator-registry root
transition evidence through node/operator CLI and readiness/P0 gates, including
a multi-height multi-process peer-certified soak after live rotate/admit/suspend
membership changes, a post-change partial-outage recovery replay, and a
post-recovery restart/resume height derived from node state, post-change
RPC catch-up from a peer RPC endpoint, a post-change below-quorum partition
safety drill, and a remote P0 live suspension that proves the first
post-suspension block is certified only by the reduced active validator set.
The latest 5-validator remote P0 run also proves a
fault-preserving membership change: after suspending one validator, the
resulting 4-validator active set finalizes the next height with one active
validator offline, replays the archived certificate into the recovered
validator, and converges. The same strict P0 path now also runs remote RPC
edge-load inside the readiness gate: each live validator rejects oversized
request envelopes, then accepts valid status RPC while the set remains
converged.

The minimum wallet path is no longer just a document gap. The node now has
versioned ML-DSA wallet key generation and restore commands, deterministic
backup metadata, private-file permission checks, and a readiness-wired smoke
that funds a derived account, binds the public key on first spend, submits an
externally signed transfer through the mempool path, applies the sealed batch,
and verifies account state through RPC.

The current limitation is integration and hardening. The HotStuff-family path
exists as evidence and operator workflows, but it still needs to become the
default long-running node loop under failed leaders, partitions, stale votes,
restarts, and load. Shielded value remains experimental until PQ note encryption
and a production proof backend exist. Bridge remains a state-machine harness,
not external asset custody.

The first network substrate is transparent PQ settlement: low-cost transparent
transfers, ML-DSA authorization, HotStuff-family finality, Cobalt validator
governance, burned-fee anti-spam controls, and reproducible operator evidence.
Privacy is now a first-class parallel product workstream: Confidential
Settlement v1 must replace the debug proof/encryption path with a production
zkVM/STARK proof backend, ML-KEM note envelopes, disclosure policy, wallet/RPC
flows, benchmarks, and audit artifacts. Production bridge custody remains out
of scope for controlled testnet.

## Milestone Burn-Down

| Milestone | Name | Status | Exit Criteria |
|---|---|---|---|
| PF-L1-TN0 | MVP foundation | Done | Deterministic state, ML-DSA authorization, transparent transfers, RPC/SDK, release tooling, local/remote validator workflows. |
| PF-L1-TN1 | BFT evidence path | Done for MVP, hardening remains | `2f+1` certificates, deterministic proposer/view metadata, signed proposals/votes/timeouts, equivocation evidence, and replay verifiers pass local and remote drills. |
| PF-L1-TN2 | External signed ingress | Done for MVP, edge hardening remains | Already-signed transparent transfers enter through bounded local and RPC paths and seal into certified batches. |
| PF-L1-TN3 | Default ordering loop | P0 gate passed; soak hardening remains | Peer-certified HotStuff-family loop is the normal transparent-chain node path and survives failed leaders, partitions, restarts, catch-up, and stale votes. |
| PF-L1-TN4 | Edge and mempool hardening | In progress next | Per-peer limits, invalid-signature metrics, bounded queues, and spam/load tests prove valid traffic is not starved. |
| PF-L1-TN5 | Validator registry and governance lifecycle | In progress; rotate-key, emergency stale-key rejection, emergency-rotation runbook, live manifest-bound remote emergency-rotation P0 proof, post-change `f + 1` halt-threshold evidence, post-change failed-leader view-change recovery, post-change partition safety, post-change stale-vote rejection, contiguous admit live mutation, key staging, non-contiguous active lists, split-key post-suspend certificates, local multi-height post-suspend ordering, local post-change outage recovery, local post-recovery restart/resume, local post-change RPC catch-up, remote live suspension/post-suspend certificate proof, and 5-validator fault-preserving remote suspension proof landed | Certificates use validator IDs plus registry bindings instead of repeated public keys; admission, removal, suspension, reactivation, key rotation, amendments, activation delay, and rollback are signed and replay-protected. |
| PF-L1-TN6 | Burned-fee/reserve policy completion | In progress | Native testnet-unit fees burn at execution; transparent fee/reserve checks and transparent account-creation state-expansion fees are visible in RPC, receipts, readiness gates, and operator reports. |
| PF-L1-TN7 | Wallet/address standard | Started; recovery/signing vectors landed | Seed recovery, ML-DSA derivation, versioned backup, local signing, first-spend public-key binding, and public recovery/signing test vectors are readiness-gated; SDK wrapping, account registration, key rotation, and discovery metadata remain. |
| PF-L1-TN8 | DDoS/key compromise drills | Started; remote RPC edge-load is P0-gated | Small-set liveness, key compromise, emergency key rotation, DDoS, and edge-exhaustion drills produce replayable evidence. |
| PF-L1-TN9 | Self-operated controlled testnet | Pending | Multi-machine validators run long soak with restart, partition, catch-up, load, topology, and release-join evidence. |
| PF-L1-TN10 | Public-testnet expansion | Later | Availability benchmark gate, funding diversity, independent operators, stronger topology diversity, published benchmarks, and external review artifacts are ready. |
| PF-L1-TN11 | Confidential settlement v1 | Critical parallel workstream | ML-KEM note encryption, production zkVM/STARK proof backend, disclosure envelopes, wallet/RPC flows, proof/ciphertext fee pricing, benchmarks, and audit package replace the debug privacy adapter. Bridge custody remains separate and out of scope. |

## Core Criticisms

- **Transaction ordering is the largest remaining hardening component.** The
  repo now has BFT quorum evidence, deterministic proposers, view metadata,
  timeout certificates, equivocation evidence, partial-outage drills, and RPC
  catch-up. The gap is promoting that path into the default long-running node
  loop and proving it under sustained faults and load.
- **Cobalt needs a narrower first role.** Cobalt is strongest as the
  validator-set, amendment, admission, removal, and evidence-governance layer.
  It should not be treated as the high-throughput transaction-ordering engine
  while the separate HotStuff-family ordering path is hardened and verified.
- **Post-quantum signatures change the performance model.** ML-DSA signatures
  and public keys are large. Validator certificates, mempool admission, RPC
  payload limits, and storage growth must be designed around that cost.
  Registry-backed public-key omission is mandatory beyond small MVP drills, not
  a later optimization.
- **Fee and reserve policy is consensus-critical.** Account reserves, byte-aware
  transfer fees, burned-fee execution, bounded mempool counts, and payload caps
  are implemented for transparent flows. Transparent recipient-account creation
  now pays an explicit state-expansion fee, and receipts, metrics, and
  readiness/P0 evidence expose it. The remaining gap is broader
  state-expanding operation pricing beyond transparent account creation,
  peer-level rate limiting, and invalid-signature load evidence.
- **The shielded layer is not production privacy yet, but it is not optional.**
  The current shielded proof path is debug-grade. The project needs
  Confidential Settlement v1: post-quantum note encryption, real proof backend,
  disclosure policy, wallet/RPC flows, fee pricing, benchmarks, and audit
  artifacts.
- **The bridge is simulation only.** Bridge code is useful as a state-machine
  harness, but it must not appear as production external-asset custody.
- **Wallet derivation needs a post-quantum-native design.** ML-DSA does not
  support BIP32-style non-hardened public derivation. A minimum deterministic
  CLI backup/restore/signing path and first public recovery/signing vectors now
  exist, but wallets still need SDK support, key rotation, and stronger
  account-management rules.
- **Validator topology must become progressively more realistic.** Four
  validator slots on two machines is useful engineering evidence, but not a
  final decentralization claim. It is not a blocker for a self-operated
  controlled testnet. Independent operators, independent infrastructure, and
  jurisdictional diversity are later evidence upgrades before broader public
  launch.
- **Coercive censorship is a separate threat.** A small federated set can be
  pressured even without Byzantine equivocation. The mitigation is independent
  operators, jurisdictional and infrastructure diversity, objective inclusion
  policy, censorship evidence where observable, and Cobalt-governed removal.
- **Validator diversity must include funding diversity.** If one founding entity
  pays enough validators to control a quorum or blocking minority, compensation
  becomes a correlated-control path. Broader public testnet needs evidence that
  no single funder, legal domain, operator group, or infrastructure provider can
  halt or capture finality.
- **Wallet UX is a real protocol issue.** ML-DSA hardened derivation means
  watch-only accounts, exchange deposit management, hardware wallets, and public
  child derivation will be worse than legacy EC chains unless we build explicit
  account-registration, key-rotation, and recovery tooling.
- **Ordering fairness needs an MVP rule.** Controlled testnet should use fixed
  fees, deterministic cutoff, and canonical non-auction ordering rather than a
  validator-local priority market.
- **Audit readiness is not just code.** The repo needs broader fuzzing,
  deterministic adversarial simulation coverage, domain-separation specs,
  cryptographic KATs, reproducible release evidence, and clear threat-model
  documentation tied to the exact shipped protocol surface.

## What We Are Doing to Address Them

- **Narrow controlled-testnet scope.** The first defensible controlled testnet
  will prioritize transparent transfers, real BFT ordering, ML-DSA
  authorization, Cobalt-governed validator-set changes, fees/reserves, RPC/SDK,
  and operational evidence.
- **Promote the HotStuff-family ordering layer.** The primitives exist now:
  deterministic leader rotation, views, votes, quorum certificates, timeout
  certificates, signed proposals, equivocation evidence, and partial-outage
  recovery. The next step is making this the default long-running node path.
- **Keep Cobalt where it is structurally strongest.** Cobalt governs validator
  admission, removal, suspension, reactivation, amendments, emergency pause,
  and parameter updates. Transaction ordering gets its own BFT protocol.
- **Make evidence first-class.** Proposals, votes, certificates, timeouts,
  validator-set changes, amendments, and equivocations should be signed,
  domain-separated, replay-protected, and inspectable.
- **Make registry-backed certificates mandatory.** Validator certificates should
  carry validator IDs, signatures, and registry bindings, not repeated public
  keys, once the network moves past small MVP drills.
- **Finish fee/reserve anti-spam controls.** Account reserve, byte-aware fees,
  native testnet-unit fee burn, receipt/metrics visibility, envelope bounds, and
  mempool caps are in place for transparent transfers, and transparent
  recipient-account creation now carries a state-expansion surcharge. Remaining
  work is peer-level limits, invalid-signature metrics, broader
  state-expanding operation pricing, and sustained spam/load evidence.
- **Promote privacy from semantic scaffold to Confidential Settlement v1.**
  Shielded value remains explicitly non-production while it uses the debug
  adapter, but implementation now advances in parallel with remote-network
  hardening. Debug proofs can test notes, nullifiers, scanning, and turnstiles;
  the production path requires ML-KEM note encryption, a benchmarked zkVM/STARK
  proof backend, disclosure envelopes, and validator-side fail-closed checks.
- **Gate bridge custody separately.** Bridge remains harness-only and is
  excluded from production claims.
- **Harden the cryptographic interface.** The codebase should pin one ML-DSA
  implementation, run known-answer tests, freeze domain-separation labels,
  specify wallet derivation, specify address/account format, and preserve
  crypto-agility for future algorithm changes.
- **Make the minimum wallet path testnet-real.** The first controlled-testnet
  wallet path now derives and restores ML-DSA transparent accounts from a
  versioned private backup, signs locally, keeps private material out of RPC and
  public reports, first-spend binds the account public key, and is called by the
  readiness gate. Recovery/signing test vectors are now readiness-gated; the
  remaining work is SDK wrapping, key rotation, and better account-discovery UX.
- **Generate operator-grade evidence ourselves first.** The immediate path is a
  self-operated controlled testnet with separate validator identities, separate
  keys, validator manifests, topology reports, restart drills, partition
  drills, and long-running soak reports. Independent external operators are not
  a build blocker; they are a later validation layer.
- **Ratchet decentralization before broader public testnet.** The network must
  move from self-operated evidence to no-quorum/no-blocking-minority exposure by
  one operator, affiliate group, cloud provider, jurisdiction, legal domain, or
  funding source.

## Execution Progress

- **2026-05-11: HotStuff ordering core started.** The repo now has typed
  ordering primitives for validator sets, deterministic leaders, proposals,
  votes, quorum certificates, timeout certificates, equivocation evidence, and a
  2-chain commit candidate rule.
- **2026-05-11: Block evidence binds ordering context.** Block proposals, votes,
  certificates, certificate IDs, and block hashes now commit to height, view,
  proposer, chain id, genesis hash, and protocol version.
- **2026-05-11: Block certificates moved from unanimity to BFT quorum evidence.**
  The node accepts and verifies `2f+1` block certificates, including a
  3-of-4-validator regression. The SDK and MVP gate now validate certificate
  quorums as supermajority subsets rather than requiring every validator to
  sign.
- **2026-05-11: Adversarial ordering simulation started.** The ordering core now
  has deterministic simulation coverage for delayed proposals, duplicate votes,
  dropped votes, partitioned views, equivocated proposals/votes, timeout
  certificates, and the no-conflicting-commit safety invariant.
- **2026-05-11: Account reserve enforcement started.** Transparent transfers now
  enforce a consensus-level account reserve so senders cannot spend below the
  reserve and new recipient accounts cannot be created with dust balances. Devnet
  scripts and benchmarks now fund transparent recipients at the reserve floor.
- **2026-05-11: Byte-aware transparent transfer fees added.** Transfer execution
  now prices signed payload weight, including ML-DSA key/signature overhead, and
  node/benchmark signing paths converge on the consensus minimum before signing.
- **2026-05-11: Mempool admission limits added.** Mempool admission and
  verification now enforce global pending-count and per-sender pending limits,
  reducing the first cheap spam path before peer-level rate limiting lands.
- **2026-05-11: Transfer envelope bounds added.** Transfer validation now caps
  text fields plus ML-DSA public-key and signature hex payloads before decode and
  signature verification, reducing oversized external-batch attack surface.
- **2026-05-11: Batch transaction-count cap added.** Mempool batch references now
  reject transaction lists above the protocol cap, bounding another external
  batch amplification path.
- **2026-05-11: Batch payload and domain validation added.** Availability-batch
  references now reject malformed transfers, cross-domain transactions, and
  serialized payloads above the protocol byte cap.
- **2026-05-11: RPC request bounds added.** RPC request files and protocol
  params now enforce byte caps before command execution, limiting oversized
  local/remote request-envelope spam.
- **2026-05-11: Local JSON artifact bounds added.** Node-side JSON artifact
  readers now stat and reject oversized files before parsing, covering batch,
  proposal, certificate, key, registry, amendment, and manifest inputs.
- **2026-05-11: Mempool batch construction preflight tightened.** Mempool batch
  creation now rejects over-cap transaction requests and invalid mempool state
  before dry-run execution or batch-file writes.
- **2026-05-11: Remote RPC pre-parse cap added.** The read-only RPC server now
  rejects oversized request lines before JSON parsing or child command spooling.
- **2026-05-11: Read-query limits bounded.** Receipt, block, and batch-archive
  queries now default to a bounded window and reject over-cap `limit` params at
  both RPC validation and node execution.
- **2026-05-11: Batch-archive payload response cap added.** RPC response
  validation now rejects oversized archived payload JSON before parsing it.
- **2026-05-11: HotStuff certificate verifiers added.** Ordering certificates
  now have standalone quorum-certificate and timeout-certificate verification
  paths that reject domain, validator-set, quorum, vote-order, high-QC, and
  certificate-id tampering.
- **2026-05-11: Verified HotStuff commit helper added.** The 2-chain
  commit-candidate path now has a verifier-backed variant and adversarial
  simulation uses it before recording commits.
- **2026-05-11: Deterministic ML-DSA fixture support added.** The crypto
  provider now exposes seeded ML-DSA-65 key generation with regression coverage
  for reproducible keys, addresses, and seeded signatures.
- **2026-05-11: Failed-leader and stale-vote simulation added.** The HotStuff
  adversarial simulator now models failed leaders and stale votes, verifies
  timeout certificates before recording them, and proves recovery without
  conflicting commits.
- **2026-05-11: External certificate proposal binding tightened.** Apply-time
  block-certificate verification now recomputes the exact proposal hash for the
  batch being committed and rejects certificates with mismatched proposal
  evidence.
- **2026-05-11: Node proposal views exposed.** Batch proposal and certified
  round commands now accept explicit consensus views, allowing deterministic
  next-view proposer rotation after a failed leader instead of hard-pinning node
  proposals to view 0.
- **2026-05-11: Transport vote views bound.** Block-vote request and response
  envelopes now carry the consensus view alongside height, and validation
  rejects proposal/envelope/response view mismatches before signing or
  accepting votes.
- **2026-05-11: Signed timeout evidence added.** Validators can now emit
  ML-DSA-signed timeout votes and aggregate quorum timeout certificates that
  cross-check against the HotStuff timeout verifier, giving failed-view recovery
  a durable operator artifact.
- **2026-05-11: Timeout certificate replay verification added.** Timeout
  certificate files now have an independent verifier/CLI that rechecks domain,
  quorum, canonical vote order, ML-DSA signatures, HotStuff certificate id, and
  local certificate id.
- **2026-05-11: Nonzero-view proposals gated by timeout evidence.** Local and
  transport-certified batch proposal paths now require a verified timeout
  certificate from the previous view before producing a later-view block
  proposal, closing the first view-change bypass in the node workflow.
- **2026-05-11: Signed vote equivocation evidence added.** The node can now
  produce a replayable evidence file when one validator signs two conflicting
  proposal votes for the same height and view, with both ML-DSA signatures
  rechecked against their proposal targets.
- **2026-05-11: Signed block proposals added.** Proposal files can now carry
  ML-DSA proposer signatures, certified rounds sign with the deterministic
  proposer key, and vote creation verifies those signatures when present before
  signing onto a proposal target.
- **2026-05-11: Peer-certified node loop mode started.** The peer-certified
  batch loop can now require the local node to be the deterministic proposer
  before certifying queued batches. `scripts/node-run-peer-certified` exposes
  the strict signed transparent loop as an operator-facing node mode, and
  `scripts/testnet-node-run-peer-certified-smoke` records local-loop evidence
  with split validator keys and converged state.
- **2026-05-12: Peer-certified loop promoted into `postfiat-node run`.**
  `postfiat-node run --mode peer-certified` now drives the strict signed
  peer-certified transparent loop with operator defaults, derives the next
  start height from local node state when not supplied, reports the top-level
  local-proposer and signed-proposal policy, and keeps
  `scripts/node-run-peer-certified` as a compatibility shim over the normal run
  surface. The node-run peer-certified smoke now routes heights 1-3 through
  validators 1, 2, and 3 using that normal run path, proving local proposer
  rotation with split validator keys and converged state. Evidence:
  `reports/testnet-node-run-peer-certified/testnet-node-run-peer-certified-20260512T013122Z.json`.
- **2026-05-12: Readiness gate carries normal-run peer-certified evidence.**
  The aggregate readiness report now includes `node_run_peer_certified_smoke`
  and requires `node_run_peer_certified_ok`, matching the gate's configured
  round count and proposer-rotation proof, alongside the service-restart,
  partial-outage recovery, normal-run RPC catch-up, and parallel RPC write-edge
  evidence. Evidence:
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T022205Z.json`.
- **2026-05-12: Normal-run peer-certified restart smoke added.**
  `scripts/testnet-node-run-peer-certified-restart-smoke` now runs two
  `postfiat-node run --mode peer-certified` rounds with no explicit
  `START_HEIGHT`, proves start heights 1 and 2 are derived from persisted node
  state, restarts the validator service layer between rounds, and verifies
  proposer rotation plus final convergence. The readiness gate now carries this
  as `node_run_peer_certified_restart_ok`. Evidence:
  `reports/testnet-node-run-peer-certified-restart/testnet-node-run-peer-certified-restart-20260512T020223Z.json`.
- **2026-05-12: Normal-run peer-certified partial-outage smoke added.**
  `scripts/testnet-node-run-peer-certified-partial-outage-smoke` now drives
  `postfiat-node run --mode peer-certified` with one non-proposer validator
  offline and `ALLOW_PEER_FAILURES=1`, proving a 3-of-4 signed-proposal quorum
  finalizes while the offline validator remains at genesis, then restarts the
  offline validator service, replays the archived certified batch, and proves
  all validators converge at height 1. The readiness gate now carries this as
  `node_run_peer_certified_partial_outage_ok` and requires the recovery replay
  booleans. Evidence:
  `reports/testnet-node-run-peer-certified-partial-outage/testnet-node-run-peer-certified-partial-outage-20260512T015459Z.json`.
- **2026-05-12: Normal-run RPC catch-up smoke added.**
  `scripts/testnet-rpc-catchup-smoke` now creates a two-round lag through
  `postfiat-node run --mode peer-certified`, keeps one non-proposer validator
  offline while the online quorum converges, then uses the provisioned
  `rpc-catch-up-from-validator.sh` operator script to recover the lagging
  validator from source RPC. The readiness gate now carries this as
  `rpc_catchup_ok` and requires the normal-run lag, pre-catch-up divergence,
  catch-up, and final convergence booleans. Evidence:
  `reports/testnet-rpc-catchup/testnet-rpc-catchup-20260512T021926Z.json`.
- **2026-05-12: RPC child large-output drain fixed.** RPC catch-up over PQ
  signed blocks exposed that large block/archive responses could stall if the
  `rpc-serve` child process filled stdout before exit. `rpc-serve` now drains
  child stdout/stderr while polling the child timeout, and regression coverage
  exercises large stdout plus timeout enforcement.
- **2026-05-12: Provisioned peer-certified loop drivers moved to normal run
  mode.** Generated transparent and action peer-certified loop scripts now call
  `postfiat-node run --mode peer-certified` with installed local validator keys
  and explicit proposal keys, and the provision bundle self-check rejects any
  loop script that falls back to `transport-peer-certified-batch-loop`
  directly. Evidence:
  `reports/testnet-readiness-gate/logs/provision-bundles/testnet-provision-bundle-20260512T022207Z/manifest.json`.
- **2026-05-12: P0 network gate now requires remote normal-run ordering.**
  `scripts/testnet-p0-network-gate` aggregates the P0 network evidence into one
  report. In remote mode it runs the local normal-run readiness baseline,
  remote readiness through proposer-routed normal
  `postfiat-node run --mode peer-certified` ordering, full validator restart,
  one-validator partial outage, remote RPC catch-up, bounded write-edge
  evidence, final convergence, and private-key policy checks. The gate now
  requires `remote_normal_run_ordering_ok` from the actual remote SSH smoke
  round reports. Evidence:
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T035456Z.json`.
  Remote ordering advanced heights 19-21 through validators 3, 0, and 1 with
  signed proposals and local-proposer enforcement; the later catch-up drill
  finalized height 23 with a 3-of-4 quorum and restored validator-2 from RPC.
- **2026-05-12: Registry-root compact certificates landed.** Block and timeout
  certificate vote verification now binds signatures to a canonical
  validator-registry root and verifies public keys from the registry instead of
  trusting vote-carried keys. New steady-state block and timeout certificate
  votes omit `public_key_hex`; legacy empty-root certificates remain supported
  for replay. The readiness and P0 gates now emit and require certificate-size
  metrics: `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T043839Z.json`
  recorded 16 certificate artifacts, 62 registry-root-bound votes, and zero
  vote-level public keys, while
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T044153Z.json`
  recorded 17 artifacts, 65 registry-root-bound votes, and zero vote-level
  public keys inside `core_p0_checks_ok`. Post-change remote P0 deploy evidence:
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T042032Z.json`.
- **2026-05-12: Cobalt validator-registry update evidence primitive added.**
  `postfiat-consensus-cobalt` now has canonical validator-registry update
  records for admit/remove/suspend/reactivate/key-rotation operations, binding
  activation height, previous/new registry roots, subject record state, quorum,
  support, vote IDs, certificate ID, and update ID. Replay verification rejects
  tampered roots, votes, support ordering, malformed lifecycle shapes, and
  malformed roots. Check: `cargo test -p postfiat-consensus-cobalt --lib`.
- **2026-05-12: Validator-registry update CLI and gates landed.**
  `postfiat-node validator-registry-root`, `validator-registry-update`, and
  `validator-registry-update-verify` now let operators compute registry roots,
  certify a root transition, and replay-verify it against the previous/new
  registry files. `scripts/testnet-validator-registry-update-smoke` proves a
  key-rotation transition at activation height 3, verifies both registry roots,
  and rejects a tampered new root. The readiness and local P0 gates now require
  `validator_registry_update_ok`. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T045932Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T045956Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T050311Z.json`.
- **2026-05-12: Validator-registry scoped apply path landed.**
  Registry updates now carry previous/new active-validator scopes so membership
  changes can prove roots over different active sets instead of over the voting
  set only. `postfiat-node validator-registry-update-apply` verifies the Cobalt
  update, enforces activation height, checks the previous root against the
  supplied registry, applies admit/remove/suspend/reactivate/rotate-key state
  transitions, and checks the resulting root before writing the output
  registry. The readiness/P0-covered smoke currently proves accepted rotate-key
  application plus early-activation, stale-root replay, tampered-subject, and
  tampered-root rejection. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T051715Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T051845Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T052207Z.json`.
- **2026-05-12: Validator-registry updates enter ordered governance evidence.**
  `GovernanceActionBatch` now carries Cobalt validator-registry update records
  alongside conventional amendments, keeps amendment-only legacy batch IDs
  replay-compatible, persists accepted registry updates in governance state, and
  rejects duplicate update IDs from later governance batches. The readiness/P0
  smoke proves a rotate-key update is recorded through `apply-governance-batch`,
  then proves a second governance batch can accept a normal amendment while
  rejecting the duplicate registry update. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T053654Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T053717Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T054048Z.json`.
- **2026-05-12: Non-rotation validator lifecycle fixtures are gate-backed.**
  `scripts/testnet-validator-registry-update-smoke` now proves
  admit/remove/suspend/reactivate transitions with previous/new active-validator
  scopes, registry-root verification, output-only apply, and the existing
  ordered-governance duplicate-replay checks. The 4-validator evidence admits
  `validator-4`, removes it, suspends `validator-1` from 4 to 3 active
  validators, and reactivates it from 3 to 4. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T055238Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T055250Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T055601Z.json`.
- **2026-05-12: Validator-registry history replay and live rotate-key activation landed.**
  Node init now preserves `validator_registry_genesis.json`, snapshots are
  versioned to include it, and block/state replay starts from that genesis
  registry before activating ordered registry updates by block height. Legacy
  validator-count amendments can still backfill missing contiguous validator
  records from the live registry, but existing validator keys are not replaced
  by the live file during historical verification. Ordered rotate-key updates
  with unchanged active-validator scope now live-apply to
  `validator_registry.json` after an activation-height block commits; the
  activation block itself still certifies under the previous root. The new
  regression rotates `validator-1` in the live registry after committed blocks
  and proves old block certificates still verify against the preserved registry
  root. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T062627Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T062640Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T062950Z.json`.
- **2026-05-12: Contiguous validator admission live mutation landed.**
  Live registry activation now accepts ordered admit updates when governance has
  already ratified the matching next contiguous active-validator count. Rotate
  updates can still activate over an earlier contiguous prefix after a later
  count amendment, and replay no longer lets a rotate-key update shrink the
  active validator count. The node regression admits `validator-2`, stages its
  key, certifies the next block with the expanded registry root, and verifies
  the historical chain. The readiness/P0 registry smoke now proves a pending
  rotate-key update followed by contiguous admission (`4 -> 5` in the 4-validator
  gates) live-applies to `validator_registry.json`. `validator-key-stage` now
  imports replacement/admitted validator private keys only after the live
  registry public key matches, and the smoke validates local keys for the
  expanded active set. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T071435Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T071529Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T071844Z.json`.
- **2026-05-12: Active-validator-list activation landed for non-contiguous suspend.**
  Governance state now carries a backward-compatible explicit
  `active_validators` list when the set is not contiguous, snapshot schema moved
  to version 5, and block proposal/certificate/replay paths use the active list
  instead of deriving every set from `active_validator_count`. Live registry
  activation can now remove a suspended validator at the activation boundary,
  and the first following block persists/certifies the non-contiguous set. The
  node regression suspends `validator-1` from a 3-validator set, then certifies
  the next block with `[validator-0, validator-2]`. The readiness/P0 registry
  smoke proves the same path after rotate plus admit (`5 -> 4` in the
  4-validator gates), with a signed proposal and split-key block certificate for
  the first post-suspend transfer. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T081820Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T081833Z.json`,
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T082157Z.json`.
- **2026-05-12: Post-suspend multi-height ordering soak landed.**
  `scripts/testnet-validator-registry-update-smoke` now carries the live
  rotate/admit/suspend sequence into a separate-node rehearsal instead of
  stopping at single-process certification. After the non-contiguous suspend
  activates, the smoke clones the public post-suspend state into one data dir
  per active validator, installs only that validator's split key, synthesizes a
  non-contiguous loopback topology, routes each post-change height through
  `block-proposer`, and runs strict signed
  `postfiat-node run --mode peer-certified` rounds against restarted validator
  services.
  The readiness gate now requires `multi_process_post_suspend.loop_verified`,
  proposer rotation, signed-proposal verification, certificate validator-list
  verification, split-key service verification, state verification, and
  convergence. The config, provision, and local harness bundle
  `validator_registry_genesis.json` as public replay state. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T091719Z/testnet-validator-registry-update-smoke.json`
  proves active validators `[validator-0, validator-2, validator-3, validator-4]`
  certified heights 6-8 through routed proposers `validator-3`, `validator-4`,
  and `validator-0` with split-key peers; aggregate gates:
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T091831Z.json`
  and
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T092228Z.json`.
- **2026-05-12: Post-membership-change partial outage recovery landed.**
  The registry update smoke now continues after the post-suspend multi-height
  soak with an active-validator outage drill. With active validators
  `[validator-0, validator-2, validator-3, validator-4]`, it routes height 9 to
  `validator-2`, leaves non-proposer `validator-4` offline, finalizes with a
  3-of-4 quorum, verifies the offline node remains at height 8, then replays
  the archived certified batch into the recovered validator and requires all
  active nodes to converge at height 9. The readiness gate now requires this
  non-skipped partial-outage report with `recovery_replay_verified` and
  `recovered_converged`. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T094219Z/testnet-validator-registry-update-smoke.json`,
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T094350Z.json`,
  and local fallback P0 aggregation
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T094806Z.json`
  (`status: "passed"`, `remote_blocked: true`, `p0_network_ok: false` because
  the run was intentionally local).
- **2026-05-12: Post-recovery restart/resume after membership change landed.**
  After the post-change outage recovery, the registry smoke now runs one more
  strict signed peer-certified height with `START_HEIGHT` omitted. The source
  node must derive height 10 from its local status after recovery, route to the
  deterministic proposer `validator-3`, collect all 4 active votes with no peer
  failures, and converge every active validator at height 10. The readiness gate
  now rejects skipped restart reports and requires
  `derived_start_height_verified` plus
  `recovered_converged_after_restart`. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T100206Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T100342Z.json`;
  latest local fallback P0 aggregation:
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T101054Z.json`.
- **2026-05-12: Post-membership-change RPC catch-up landed.** After the
  post-change outage and restart/resume checks, the registry smoke now leaves
  one active non-proposer lagging for height 11, finalizes with the remaining
  3-of-4 quorum, serves the source validator over read-only RPC, and runs
  `postfiat-node rpc-catch-up` on the lagging validator. The report verifies
  the lagging validator stayed at height 10 before catch-up, applied exactly
  one certified archived batch from `validator-4`, and converged with the active
  set at height 11. The readiness gate now requires this non-skipped
  post-change RPC catch-up report. Evidence:
  `reports/testnet-validator-registry-update/run-20260512T102729Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260512T102926Z.json`;
  latest local fallback P0 aggregation:
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T103758Z.json`.
- **2026-05-12: Remote P0 gate revalidated after normal-run idempotence fix.**
  `scripts/testnet-remote-ssh-smoke` now transports generated inline remote
  bash through a base64 wrapper and clears only the active per-height normal-run
  workspace before recreating it, so reruns do not fail on stale `processed`
  files. The remote P0 gate passed end to end in remote mode: local readiness
  stayed green, remote proposer-routed normal-run ordering finalized heights
  1-3, remote restart verified all services/state/RPC at height 3, remote
  partial outage finalized height 4 with validator-3 offline and recovered it,
  and remote RPC catch-up advanced validator-3 from height 4 to 5 from
  validator-1's RPC evidence. Evidence:
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T111446Z.json`,
  `reports/testnet-p0-network-gate/logs/remote/readiness/testnet-remote-readiness-gate.json`,
  `reports/testnet-p0-network-gate/logs/remote/restart/testnet-remote-restart-drill.json`,
  `reports/testnet-p0-network-gate/logs/remote/partial-outage/testnet-remote-partial-outage-drill-20260512T113003Z.json`,
  and
  `reports/testnet-p0-network-gate/logs/remote/rpc-catchup/testnet-remote-rpc-catchup-drill-20260512T113136Z.json`.
- **2026-05-12: Remote validator-registry suspension drill landed in P0.**
  `scripts/testnet-remote-validator-registry-drill` now orders a Cobalt-backed
  validator-registry `suspend` update through the deployed remote network,
  verifies the governance block was certified by the previous 4-validator set,
  then orders the next transparent block with the reduced active set. The P0
  gate now requires `remote_validator_registry_update_ok`: the post-suspension
  certificate must list `[validator-0, validator-1, validator-2]`, exclude the
  suspended `validator-3`, record the stale topology vote failure for the
  suspended node, converge active validator state, and verify active registry
  roots. Evidence:
  `reports/testnet-p0-network-gate-remote-registry-drill-rerun/testnet-p0-network-gate-20260512T123430Z.json`
  and
  `reports/testnet-p0-network-gate-remote-registry-drill-rerun/logs/remote/validator-registry/testnet-remote-validator-registry-drill-20260512T125127Z.json`.
- **2026-05-12: Minimum wallet path landed and joined readiness.**
  `postfiat-node wallet-keygen` and `wallet-restore` now create a versioned
  ML-DSA transparent account backup/keyfile pair using the existing node keyfile
  format. `scripts/testnet-wallet-minimum-smoke` proves deterministic restore,
  `0600` private-file modes, first-spend public-key binding for funded
  transparent accounts, local signing without RPC key exposure, signed mempool
  submission, sealed-batch apply, and RPC account queries. The readiness gate
  now requires `wallet_minimum_ok`. Evidence:
  `reports/testnet-wallet-minimum-smoke/manual/testnet-wallet-minimum-smoke.json`
  and
  `reports/testnet-readiness-gate-wallet-minimum-check/testnet-readiness-gate-20260512T143623Z.json`.
- **2026-05-12: Wallet recovery/signing test vectors landed.**
  `postfiat-node wallet-test-vector` emits deterministic public ML-DSA wallet
  vectors from a public fixture seed and deterministic signature seed, including
  address, public key, signing bytes, signing hash, signed transfer, minimum
  fee, and transaction id while redacting private key material.
  `scripts/testnet-wallet-test-vectors-smoke` generates three account indexes,
  proves byte-stable replay for index 0, proves unique account derivation,
  submits the deterministic signed transfer, applies the sealed batch, and is
  now required by `scripts/testnet-readiness-gate` as
  `wallet_test_vectors_ok`. P0 now surfaces `wallet_test_vectors_ok` plus vector
  metrics. Evidence:
  `reports/testnet-wallet-test-vectors-smoke/manual/testnet-wallet-test-vectors-smoke.json`
  and
  `reports/testnet-wallet-test-vectors-smoke/manual/wallet-test-vectors.public.json`.
  P0 aggregation evidence:
  `reports/testnet-p0-network-gate-local-wallet-vectors/testnet-p0-network-gate-20260512T205540Z.json`.
- **2026-05-12: Emergency key-rotation stale-key drill landed in readiness.**
  `scripts/testnet-validator-registry-update-smoke` now preserves the rotated
  validator's pre-rotation key as compromised material, proves
  `validator-key-stage` rejects staging that stale key after the live registry
  rotates, proves `block-vote` rejects a stale-key vote against the live
  registry, and proves the replacement staged key can produce a compact
  registry-root-bound vote. The readiness gate now requires
  `emergency_key_rotation_ok`. Evidence:
  `reports/testnet-validator-registry-update/emergency-key-rotation-20260512T145238Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate-emergency-key-rotation-check/testnet-readiness-gate-20260512T145446Z.json`;
  refreshed after the whitepaper/roadmap update at
  `reports/testnet-readiness-gate-emergency-key-rotation-doc-check/testnet-readiness-gate-20260512T150850Z.json`.
- **2026-05-12: Post-change `f + 1` halt-threshold drill landed in readiness.**
  The validator-registry smoke now continues after live suspend and
  post-change recovery into an explicit halt drill: with active validators
  `[validator-0, validator-2, validator-3, validator-4]`, `f = 1`, and two
  validators unavailable, the peer-certified command fails with insufficient
  block votes, records only two vote artifacts against quorum three, leaves the
  batch unarchived, and proves no active node advanced from height 9. Evidence:
  `reports/testnet-validator-registry-update/f-plus-one-halt-20260512T153304Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate-f-plus-one-halt-check/testnet-readiness-gate-20260512T153515Z.json`.
- **2026-05-12: Post-change stale-vote rejection landed in readiness.**
  After live rotate/admit/suspend, post-suspend ordering, outage recovery,
  restart/resume, and RPC catch-up, the validator-registry smoke now builds a
  fresh height-12 proposal and attempts certificate aggregation with a
  prior-height vote from height 11. Aggregation rejects the stale vote, writes
  no certificate, and proves all active validators remain at height 11.
  Evidence:
  `reports/testnet-validator-registry-update/stale-vote-20260512T155815Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate-stale-vote-check/testnet-readiness-gate-20260512T160029Z.json`.
- **2026-05-12: Post-change failed-leader view-change drill landed in readiness.**
  The validator-registry smoke now continues after post-change RPC catch-up
  into a view-change drill: view 0 routes to `validator-0`, the online quorum
  `[validator-2, validator-3, validator-4]` produces a timeout certificate,
  view 1 routes to `validator-2`, and the view-1 round finalizes height 12
  with exactly 3-of-4 active votes while `validator-0` remains at height 11.
  The drill then replays the certified batch into recovered `validator-0` and
  proves convergence. Evidence:
  `reports/testnet-validator-registry-update/failed-leader-20260512T162743Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate-failed-leader-check/testnet-readiness-gate-20260512T163011Z.json`.
- **2026-05-12: Post-change below-quorum partition drill landed in readiness.**
  The validator-registry smoke now continues after failed-leader recovery into
  an active-set partition drill. In the readiness run it split the 4-validator
  active set into 2/2 partitions, routed height 13 to `validator-2`, collected
  only two vote artifacts against quorum three, wrote no block certificate,
  archived no batch, and proved every active node stayed at height 12. The
  focused 5-validator run also split 2/3 against quorum four and failed with
  only two votes. Evidence:
  `reports/testnet-validator-registry-update/partition-20260512T165411Z/testnet-validator-registry-update-smoke.json`
  and
  `reports/testnet-readiness-gate-partition-check/testnet-readiness-gate-20260512T165731Z.json`.
- **2026-05-12: P0 local summary now exposes registry fault drills.**
  `scripts/testnet-p0-network-gate` now extracts the readiness-gated validator
  registry smoke into `local_validator_registry_faults` and requires
  `local_validator_registry_fault_drills_ok` inside `core_p0_checks_ok`. The
  P0 report now directly surfaces emergency stale-key rejection, `f + 1`
  below-quorum halt, failed-leader view-change recovery, below-quorum partition
  safety, and stale-vote rejection instead of hiding them behind the single
  `validator_registry_update_ok` aggregate. Evidence:
  `reports/testnet-p0-network-gate-local-fault-summary/testnet-p0-network-gate-20260512T171824Z.json`.
- **2026-05-12: Emergency key-rotation operator runbook packaged.**
  The operator procedure for compromised validator hot-key response now lives
  at `docs/runbooks/validator-emergency-key-rotation.md`. It maps the
  readiness/P0 emergency stale-key evidence to operator actions:
  stop signing, generate a replacement key, create and order a rotate-key
  registry update, stage only the replacement key after activation, reject stale
  key staging, reject stale-key block votes, verify replacement-key voting, and
  replay the P0 evidence without publishing private key material.
- **2026-05-12: Remote emergency key-rotation rehearsal wired into P0.**
  `scripts/testnet-remote-emergency-key-rotation-rehearsal` now binds the
  remote operator deploy plan to local emergency stale-key rejection evidence
  and the remote validator-registry/fault-tolerance drill, emits a sanitized
  public report, and is required by `scripts/testnet-p0-network-gate` in remote
  mode. Replay evidence:
  `reports/testnet-remote-emergency-key-rotation-rehearsal/replay-20260512T171824Z/testnet-remote-emergency-key-rotation-rehearsal-20260512T175102Z.json`
  and
  `reports/testnet-remote-emergency-key-rotation-rehearsal/p0-derivation-20260512T171824Z/testnet-remote-emergency-key-rotation-rehearsal-20260512T175658Z.json`.
  Local fallback regression with the remote rehearsal skipped:
  `reports/testnet-p0-network-gate-local-emergency-rehearsal-hook/testnet-p0-network-gate-20260512T175110Z.json`.
- **2026-05-12: Remote P0 revalidated with live emergency-rotation rehearsal.**
  A credential-backed 5-validator remote P0 run now produces
  `remote_emergency_key_rotation_rehearsal_ok: true` from the live remote path,
  alongside `p0_network_ok`, normal-run ordering, restart, partial outage, RPC
  catch-up, remote validator-registry suspension, and post-suspend
  fault-tolerance checks. Evidence:
  `reports/testnet-p0-network-gate-remote-emergency-rehearsal/testnet-p0-network-gate-20260512T180400Z.json`
  and
  `reports/testnet-p0-network-gate-remote-emergency-rehearsal/logs/remote/emergency-key-rotation/testnet-remote-emergency-key-rotation-rehearsal.json`.
- **2026-05-12: Remote topology capture profile wired into P0.**
  `scripts/testnet-remote-topology-capture-profile` reads the remote operator
  deploy plan and emits a sanitized profile of quorum and blocking thresholds
  without publishing hosts or credential material. It is now called by remote
  P0. Direct replay against the latest live remote plan reports 5 validators,
  quorum 4, blocking threshold 2, no single group able to reach quorum, and a
  current single-group blocking risk that must be fixed before broader public
  expansion. Evidence:
  `reports/testnet-remote-topology-capture-profile/live-remote-p0-20260512T180400Z/testnet-remote-topology-capture-profile-20260512T183657Z.json`.
  Local fallback regression with the profile skipped:
  `reports/testnet-p0-network-gate-local-topology-capture-hook/testnet-p0-network-gate-20260512T183707Z.json`.
- **2026-05-12: Controlled-testnet topology gate made enforceable.**
  `TOPOLOGY_CAPTURE_REQUIRE_OK=1 scripts/testnet-remote-topology-capture-profile`
  now exits nonzero unless no single host/operator/operator-host group can
  block quorum or reach quorum. On the latest live 5-validator remote plan the
  gate correctly fails: quorum is 4, blocking threshold is 2, the required max
  validators per group is 1, and the current max group size is 3. Evidence:
  `reports/testnet-remote-topology-capture-profile/strict-live-remote-p0-20260512T180400Z/testnet-remote-topology-capture-profile-20260512T185129Z.json`.
  `scripts/testnet-p0-network-gate` also has
  `P0_REQUIRE_TOPOLOGY_CAPTURE_OK=1` for a full remote P0 promotion gate.
- **2026-05-12: Topology capture-threshold profile added.**
  `scripts/testnet-remote-topology-capture-profile` now emits the minimum
  independent host/operator/operator-host groups needed to block quorum and to
  reach quorum, plus a `--self-test` wired into `scripts/check`. On the latest
  live 5-validator plan the explicit capture-threshold replay shows
  `minimum_groups_to_block_quorum: 1` and
  `minimum_groups_to_reach_quorum: 2`; strict promotion still fails until
  placement moves to one validator per independent group. Evidence:
  `reports/testnet-remote-topology-capture-profile/strict-live-remote-p0-capture-threshold-v2/testnet-remote-topology-capture-profile-20260512T203541Z.json`.
  Local P0 fallback still passes after adding the new summary fields:
  `reports/testnet-p0-network-gate-local-capture-threshold-summary/testnet-p0-network-gate-20260512T203548Z.json`.
- **2026-05-13: Extra machine-bucket credentials are P0-visible.**
  The remote credential parser now accepts `SSH_EXTRA_CRED_FILES` and
  `MACHINE_BUCKET_FILE` in addition to `SSH_CRED_FILE`, and supports the
  shorthand `mN`/`pN` host/password bucket format by normalizing each source
  into canonical `machine_N_host/password` entries without writing a merged
  secret file. `scripts/testnet-remote-placement-capacity-profile --self-test`
  now covers the extra-bucket merge path. With the user's three-entry bucket
  plus the existing two-machine credential file, strict 5-validator placement
  capacity is green: 5 complete machine entries, 5 required independent groups,
  no machine reuse, and no credential/IP leakage in the report:
  `reports/testnet-remote-placement-capacity-profile/machinemucket-merged/testnet-remote-placement-capacity-profile-20260513T020728Z.json`.
  Strict remote P0 then advanced past local readiness into real SSH deployment
  and failed in `remote readiness gate` because one new bucket target account
  cannot run sudo, which the current systemd deploy path requires. Evidence:
  `reports/testnet-p0-network-gate-remote-machinemucket-strict/testnet-p0-network-gate-20260513T020757Z.json`.
  The two validators staged before that sudo failure were stopped using the
  same plan and merged credential sources, so the failed attempt should not
  leave an intentional two-node partial network running.
  `scripts/testnet-remote-ssh-smoke` now preflights sudo/systemd access across
  all requested targets before staging any validator; replaying the failed plan
  stops during preflight on the same non-sudo target with no deploy attempt:
  `reports/testnet-remote-ssh-smoke/machinemucket-sudo-preflight-v2/testnet-remote-ssh-smoke.json`.
  The same preflight is now an early `scripts/testnet-p0-network-gate` remote
  step, so strict remote P0 fails before local readiness or deployment when a
  target is not operator-ready:
  `reports/testnet-p0-network-gate-remote-operator-preflight-failfast-v2/testnet-p0-network-gate-20260513T023020Z.json`.
  The preflight now continues across all targets and emits a sanitized
  per-validator readiness profile. Current strict remote evidence shows exactly
  two operator-readiness failures: `validator-2` and `validator-3` both have
  SSH login but no sudo; validators 0, 1, and 4 pass SSH/sudo/systemd:
  `reports/testnet-p0-network-gate-remote-operator-preflight-profile/testnet-p0-network-gate-20260513T024202Z.json`.
  Local fallback P0 still passes with the new profile fields:
  `reports/testnet-p0-network-gate-local-operator-profile-fields/testnet-p0-network-gate-20260513T024253Z.json`.
  The next controlled-testnet blocker is therefore operator machine readiness
  on the new remote targets, not validator count or placement capacity.
- **2026-05-12: Strict remote placement capacity now fails fast.**
  `scripts/testnet-remote-placement-capacity-profile` reads only credential
  shape, emits sanitized counts, and is called before remote P0 work. With
  `P0_REQUIRE_TOPOLOGY_CAPTURE_OK=1`, P0 now stops before local readiness or
  remote deploy if the credential inventory cannot satisfy the no-single-group
  blocking rule. Current 5-validator/quorum-4 capacity evidence shows only 2
  complete machine entries against 5 required independent groups, so strict
  controlled-testnet promotion is blocked on additional independent placement
  targets/operator identities rather than another long P0 rerun. Evidence:
  `reports/testnet-remote-placement-capacity-profile/strict-current-creds/testnet-remote-placement-capacity-profile-20260512T191019Z.json`
  and fail-fast P0 report
  `reports/testnet-p0-network-gate-strict-placement-capacity-fail-v2/testnet-p0-network-gate-20260512T193224Z.json`.
  Local fallback regression still passes:
  `reports/testnet-p0-network-gate-local-placement-capacity-plumbing/testnet-p0-network-gate-20260512T191055Z.json`.
- **2026-05-12: Placement manifests are now P0-visible and redacted.**
  `scripts/testnet-remote-placement-capacity-profile` now accepts
  `PLACEMENT_MANIFEST` with schema `postfiat-testnet-placement-manifest-v1`.
  The manifest binds placement targets to credential machine indexes and counts
  host/operator/operator-host groups for controlled-testnet promotion without
  publishing group labels. It also records advisory public-expansion counts for
  cloud provider, region, jurisdiction, legal domain, and funding source, and
  `PLACEMENT_REQUIRE_PUBLIC_DIVERSITY=1` can make those dimensions strict. The
  example manifest lives at
  `docs/examples/controlled-testnet-placement-manifest.example.json`. Current
  credential-backed manifest evidence still fails as intended because only 2 of
  5 manifest targets are bindable to credentialed machines:
  `reports/testnet-remote-placement-capacity-profile/manifest-current-creds-shortfall/testnet-remote-placement-capacity-profile-20260512T200137Z.json`.
  The report now emits `capacity_shortfall`; the current strict profile is
  missing 3 bindable/complete controlled-testnet targets and 3 independent
  host/operator/operator-host groups.
  Strict remote P0 with that manifest stops before deployment:
  `reports/testnet-p0-network-gate-strict-placement-manifest-shortfall-v2/testnet-p0-network-gate-20260512T200215Z.json`
  and marks `remote_blocked: true` with
  `remote_placement_capacity_summary.capacity_shortfall`.
  Local fallback schema regression:
  `reports/testnet-p0-network-gate-local-placement-shortfall-summary/testnet-p0-network-gate-20260512T200227Z.json`.
  The placement-capacity script also has a `--self-test` mode wired into
  `scripts/check`; it covers credential-only strict pass, manifest plus
  public-diversity pass, and partial-inventory fail cases without live remote
  credentials.
- **2026-05-12: P0 report now exposes RPC write-edge pressure metrics.**
  The readiness-gated write-edge load drill was already part of P0; the P0
  report now surfaces the actual counters instead of only `write_edge_load_ok`.
  Latest local P0 evidence includes 6 invalid signed-transfer attempts rejected
  without persisting, a valid signed transfer admitted after invalid pressure, a
  global submit-limit rejection, and parallel invalid pressure that did not
  starve the valid transfer. Evidence:
  `reports/testnet-p0-network-gate-local-edge-metrics-summary/testnet-p0-network-gate-20260512T192431Z.json`.
- **2026-05-12: RPC request-envelope edge exhaustion is now P0-visible.**
  `scripts/testnet-rpc-serve-tamper` now sends a bounded oversized-request
  wave through `rpc-serve`, requires `rpc_request_too_large` rejection before
  JSON parsing, then requires a valid `status` request to succeed. The
  readiness gate records `rpc_edge_exhaustion_ok`, and P0 surfaces
  `rpc_edge_exhaustion_metrics`. Latest local P0 evidence:
  `reports/testnet-p0-network-gate-local-fee-edge-summary/testnet-p0-network-gate-20260512T202428Z.json`.
- **2026-05-12: P0 report now exposes fee/reserve policy metrics.**
  The readiness-gated fee/reserve policy smoke was already green; P0 now
  surfaces `fee_reserve_policy_ok` and `fee_reserve_policy_metrics` with
  charged fee, burned fee, minimum fee, account reserve, burned-fee total, fee
  byte quantum, fee-per-quantum, and the no-funded-fee-collector check.
  Latest local P0 evidence:
  `reports/testnet-p0-network-gate-local-fee-edge-summary/testnet-p0-network-gate-20260512T202428Z.json`.
- **2026-05-12: Transparent account-creation state-expansion fee landed.**
  New transparent recipient accounts now add an explicit consensus fee on top
  of byte-aware transfer pricing. Receipts expose `state_expansion_fee`, node
  metrics expose `transfer_account_creation_fee`, and the fee/reserve smoke plus
  P0 report prove the two match. Latest local P0 evidence:
  `reports/testnet-p0-network-gate-local-state-expansion-fee/testnet-p0-network-gate-20260512T211329Z.json`.
- **2026-05-12: Transfer fee quote RPC landed.** `postfiat-node
  transfer-fee-quote` and RPC method `transfer_fee_quote` now expose a
  ledger/mempool-aware transparent transfer quote before signing. The smoke
  proves CLI quote, RPC quote, and committed receipt agree for new-recipient
  account creation, then proves the surcharge disappears once the recipient
  exists. Latest local P0 evidence:
  `reports/testnet-p0-network-gate-local-transfer-fee-quote/testnet-p0-network-gate-20260512T213403Z.json`.
- **2026-05-12: Offline wallet signed-transfer command landed.**
  `postfiat-node wallet-sign-transfer` signs an exact fee quote without a node
  data directory: key file plus a quote JSON file, or explicit quoted chain id,
  genesis hash, protocol version, fee, and sequence in; standalone
  `SignedTransfer` JSON out for mempool submission. The new smoke proves RPC
  quote -> offline sign from the RPC quote file -> public RPC
  `mempool_submit_signed_transfer` -> batch apply -> receipt-fee match with no
  private-material leakage.
  Latest targeted evidence:
  `reports/testnet-wallet-sign-transfer-smoke/rpc-submit/testnet-wallet-sign-transfer-smoke.json`.
  Latest local P0 evidence:
  `reports/testnet-p0-network-gate-local-wallet-rpc-submit/testnet-p0-network-gate-20260512T221256Z.json`.
- **2026-05-13: Governance replay package v0 landed.**
  `postfiat-node governance-replay-verify --package-file ...` now verifies an
  archived canonical Cobalt-derived validator-registry governance package
  offline. The package binds the previous registry file, registry-update file,
  new registry file, optional ordered governance batch, expected update id, and
  expected batch id to the local chain domain, then rechecks previous/new
  registry roots and governance-batch inclusion. The package also verifies
  optional post-change block/certificate evidence using the same canonical
  payload serialization as consensus. The smoke now starts from two validators,
  admits `validator-2`, orders the governance batch, certifies the next
  transparent block under the expanded three-validator registry root, and
  rejects a tampered expected update id. The readiness gate and local P0
  fallback now require `governance_replay_package_ok` and surface the replay
  metrics.
  Evidence:
  `reports/testnet-governance-replay-package-smoke/run-20260513T014215Z/testnet-governance-replay-package-smoke.json`,
  `reports/testnet-readiness-gate-governance-replay-full/testnet-readiness-gate-20260513T014815Z.json`,
  and
  `reports/testnet-p0-network-gate-local-governance-replay-check/testnet-p0-network-gate-20260513T015357Z.json`.
- **2026-05-11: Signed proposal equivocation evidence added.** The node can now
  produce replayable evidence when a proposer signs two conflicting block
  proposals for the same height and view; unsigned proposals cannot be used as
  proposal-equivocation proof.
- **2026-05-11: Equivocation evidence replay verification added.** Vote and
  proposal equivocation evidence files now have CLI/library verifiers that
  recompute the evidence from the signed artifacts and reject tampered evidence
  files.
- **2026-05-11: Peer-certified proposal signing path added.** Networked
  peer-certified round and loop commands can now take a proposer key file,
  produce signed proposal artifacts, report the proposer/signature binding, and
  exercise that path in the local peer-certified smoke harness.
- **2026-05-11: Validator strict proposal-signature policy added.** Validator
  vote services can now require signed block proposals before producing votes,
  and the peer-certified round/loop harnesses run those services in strict mode.
- **2026-05-11: Local-proposer peer-certified round enforced.** The
  peer-certified round command can now require the local validator to be the
  deterministic proposer, and the round smoke runs from the proposer with only
  that validator's split local key.
- **2026-05-11: Proposer routing report added.** Operators can now ask a node
  for the deterministic proposer at a height/view, and the peer-certified round
  smoke uses that report to route the round before signing.
- **2026-05-11: Routed peer-certified loop added.** The multi-round
  peer-certified loop smoke now routes each height to its deterministic
  proposer and signs with only that validator's split local key, removing the
  combined proposer-key shortcut from the loop evidence path.
- **2026-05-11: Peer-certified proposer signing made fail-closed.** Networked
  peer-certified round and loop commands can now require signed proposals before
  contacting peers, and the routed smokes assert that requirement in their
  evidence.
- **2026-05-11: Remote peer-certified rounds routed by proposer.** Provisioned
  one-round operator commands now run against an explicit local validator,
  preflight the deterministic proposer, require signed proposals, and the remote
  smoke plan routes each height to the proposer instead of hardcoding
  validator-0.
- **2026-05-11: Remote smoke defaults to routed rounds.** The credential-backed
  remote smoke now defaults to proposer-routed one-round execution instead of
  single-source loop mode, keeping the default test path aligned with signed
  proposer evidence.
- **2026-05-11: Credential-backed routed remote smoke passed.** A 4-validator
  remote deploy/smoke completed one transparent peer-certified round at height
  1 using validator-1 as deterministic proposer, with signed proposal evidence,
  local-proposer enforcement, and all validators converged at block height 1.
- **2026-05-11: Remote proposer rotation smoke passed.** Reusing the deployed
  4-validator remote network, a 3-round transparent smoke finalized heights
  2-4 through validators 2, 3, and 0 respectively, with signed proposer
  evidence and convergence at block height 4.
- **2026-05-11: Remote action batch smokes passed.** The same 4-validator
  remote network finalized governance, shielded, and bridge action rounds at
  heights 5-7 through validators 1, 2, and 3 respectively, each with signed
  proposer evidence and convergence through block height 7.
- **2026-05-11: Remote restart drill passed.** All 4 remote validator/RPC
  service pairs restarted, verified local state, served RPC reads, and remained
  converged at block height 7.
- **2026-05-11: Remote snapshot drill passed.** All 4 remote validators
  exported and imported snapshots at block height 7, restored matching chain
  state, verified restored state, and excluded private key files from snapshot
  manifests.
- **2026-05-11: Short remote soak passed.** A 2-iteration no-redeploy remote
  soak ran 4 total rounds across transparent and governance traffic, with
  continuity, observability, and RPC tamper checks passing and final convergence
  at block height 11.
- **2026-05-11: Partial-outage quorum path added.** Peer-certified rounds now
  have an explicit `--allow-peer-failures` mode for outage drills; a 4-validator
  local smoke finalized with a 3-of-4 quorum while validator-3 was offline,
  proving online convergence and offline non-application.
- **2026-05-11: Partial-outage mode exposed to operators.** Provisioned
  one-round scripts and remote action wrappers now accept
  `POSTFIAT_ALLOW_PEER_FAILURES=1`, keeping outage tolerance opt-in while
  preserving signed-proposer and local-proposer requirements.
- **2026-05-11: Remote partial-outage drill passed.** A fresh 4-validator
  remote network stopped validator-3's transport service, finalized height 2
  through validator-2 with a 3-of-4 signed certificate, recorded the expected
  failed vote/send paths for validator-3, proved online convergence, restarted
  validator-3, replayed the certified block, and restored 4-of-4 convergence.
  Evidence:
  `reports/testnet-remote-partial-outage-drill/testnet-remote-partial-outage-drill-20260511T192057Z.json`.
- **2026-05-11: Archived-block catch-up primitive added.** The node can now
  reconstruct a full external block-certificate file from an archived block
  record plus archived batch payload, rechecking chain domain, validator set,
  proposal hash, certificate id, block hash, and ML-DSA vote evidence before
  writing the certificate. This removes dependence on proposer-local artifact
  directories for replaying a missed certified block.
- **2026-05-11: Remote RPC catch-up drill passed.** A fresh 4-validator remote
  network finalized height 2 while validator-3's transport service was offline,
  restarted validator-3, fetched the missing block and batch archive over
  validator-2 RPC, reconstructed the certificate on validator-3, applied the
  missing block locally, verified state, and restored 4-of-4 convergence without
  using proposer-local artifact paths. Evidence:
  `reports/testnet-remote-rpc-catchup-drill/testnet-remote-rpc-catchup-drill-20260511T194053Z.json`.
- **2026-05-11: Node-native RPC catch-up added.** Lagging validators can now
  run `postfiat-node rpc-catch-up` directly against a source RPC endpoint,
  verify the chain domain, fetch a bounded forward range of missing
  block/archive evidence, reconstruct block certificates locally, apply the
  certified blocks, and verify height, tip, and state-root convergence. The
  local smoke recovered two consecutive missing blocks at
  `reports/testnet-rpc-catchup/testnet-rpc-catchup-20260511T201824Z.json`; the
  remote drill recovered one missing remote block at
  `reports/testnet-remote-rpc-catchup-drill/testnet-remote-rpc-catchup-drill-20260511T201403Z.json`.
- **2026-05-11: RPC catch-up exposed to operators.** Provision and release
  bundles now include `scripts/rpc-catch-up-from-validator.sh`, which maps
  validator IDs to local data/log paths and source RPC endpoints before invoking
  the node-native catch-up command. The generated operator script recovered a
  two-block-lagging local validator at
  `reports/testnet-rpc-catchup/testnet-rpc-catchup-20260511T202450Z.json`, and
  fresh provision, release package, and release join smokes passed, ending with
  `reports/testnet-release-join-smoke-rpc-catchup-operator3/testnet-release-join-smoke-20260511T202534Z.json`.
- **2026-05-11: External signed-transfer mempool ingress added.** Nodes can now
  admit an already-signed transparent transfer from a bounded local JSON file,
  reject forged signatures and duplicates without persisting them, verify the
  resulting mempool state, expose the same path through SDK/local-RPC requests,
  accept signed transfer JSON through an explicit opt-in remote RPC write mode,
  and seal accepted external transfers into mempool batches. Evidence:
  `reports/devnet-signed-mempool-ingress/devnet-signed-mempool-ingress-20260511T204802Z.json`.
- **2026-05-11: Whitepaper current state aligned.** The whitepaper now describes
  Cobalt as validator-set governance, the HotStuff-family path as transaction
  ordering, the current BFT-quorum and signed-ingress evidence already shipped,
  and the remaining controlled-testnet blockers.
- **2026-05-11: Burned-fee visibility started.** Transparent transfer execution
  now burns fees instead of crediting a fee collector, receipts record charged
  and burned fees plus minimum fee/reserve policy, node metrics expose
  burned-fee totals and the fee/reserve schedule, and
  `scripts/testnet-fee-reserve-policy-smoke` is wired into the readiness gate.
- **2026-05-12: RPC write-edge rejection metrics started.** Remote `rpc-serve`
  reports now classify invalid signed-transfer submissions, duplicate
  transactions, and disallowed methods, and `scripts/devnet-signed-mempool-ingress`
  proves an invalid remote signed transfer is rejected without persistence while
  a valid transfer still enters the mempool.
- **2026-05-12: Per-peer remote mempool write cap added.** Remote `rpc-serve`
  now enforces `--max-mempool-submit-per-peer` for opt-in signed-transfer writes,
  reports rate-limited attempts, and the signed-mempool ingress smoke proves
  excess invalid writes are capped before child RPC execution without persisting
  mempool state. Evidence:
  `reports/devnet-signed-mempool-ingress/devnet-signed-mempool-ingress-20260512T004640Z.json`.
- **2026-05-12: RPC write-edge load, global write budget, and local parallel
  pressure evidence added.** Remote
  `rpc-serve` reports its configured bounded request window and enforces
  `--max-mempool-submit-total` across all peers. The write-edge load smoke
  proves six invalid signed-transfer attempts are rejected without mempool
  persistence while a following valid signed transfer is admitted and sealed,
  then proves an exhausted global budget rejects further signed-transfer writes
  before child RPC execution, and finally proves a concurrent local wave of
  invalid signed transfers does not starve a valid signed transfer. Evidence:
  `reports/testnet-rpc-write-edge-load/testnet-rpc-write-edge-load-20260512T011918Z.json`.
- **2026-05-12: RPC child-handler timeout added.** Remote `rpc-serve` now
  enforces `--child-timeout-ms` around spawned child RPC handlers, reports
  `rpc_child_timeout_count`, and generated systemd RPC units expose
  `POSTFIAT_RPC_CHILD_TIMEOUT_MS`. Evidence:
  `reports/testnet-rpc-serve-tamper/testnet-rpc-serve-tamper-20260512T005855Z.json`
  and
  `reports/testnet-provision-bundles/testnet-provision-bundle-20260512T005856Z/manifest.json`.

## Core Action Items

| Priority | Milestone | Status | Next Implementation Action | Exit Criteria |
|---|---|---|---|---|
| P0 | PF-L1-TN3 Default Ordering Loop | P0 gate passed; soak hardening remains | Keep `scripts/testnet-p0-network-gate` as the regression command and extend from short drills to sustained failed-leader, stale-vote, partition, restart, catch-up, and load soaks after certificate-format work lands. | Multi-round local and remote loops finalize through deterministic proposers, timeout/view-change recovery, restart, partition/outage, and catch-up without conflicting commits, with one replayable report that requires remote normal-run ordering. |
| P0 | PF-L1-TN4 Edge And Mempool Hardening | In progress next | Keep bounded signed-transfer write-edge pressure inside the P0 network gate, reusing per-peer/global limits, local parallel load evidence, child-handler timeouts, invalid-signature metrics, and bounded-window load smoke. | Invalid or oversized traffic cannot starve valid signed transfers, and the P0 network report includes edge-spam metrics. |
| P1 | PF-L1-TN5 Validator Registry And Governance Lifecycle | In progress | Extend from live manifest-bound emergency-rotation evidence into capture-threshold and remote partition drills. | Validator-set and amendment changes are signed, domain-bound, deterministic, replay-protected, and independently verifiable from archived evidence without repeated public keys in every certificate. |
| P1 | PF-L1-TN6 Burned-Fee/Reserve Policy Completion | In progress | Extend from transparent account-creation state-expansion pricing and fee-quote RPC into remaining state-expanding surfaces, and keep burned-fee/reserve evidence in RPC, receipts, operator reports, and readiness gates. | No below-fee or below-reserve transaction can enter a committed block, and burned-fee/reserve state is inspectable by clients and operators. |
| P1 | PF-L1-TN7 Wallet/Address Standard | Started; recovery/signing vectors landed | Add SDK wrappers, account key rotation, and account-discovery metadata around the CLI backup/restore/signing/vector path. | CLI/SDK can create, restore, register, rotate, sign, and verify accounts from versioned test vectors. |
| P2 | PF-L1-TN8 DDoS And Key-Compromise Drills | Started | Extend from the readiness-gated stale-key and `f+1` halt drills into validator DDoS, edge exhaustion, and capture-threshold scenarios. | Reports show bounded resource use, explicit halt/capture thresholds, key-rotation recovery, and no unsafe finality after compromised-key evidence. |
| P2 | PF-L1-TN9 Self-Operated Testnet | Pending | Run validators ourselves across available local and remote machines with separate keys, manifests, topology reports, restart drills, partition drills, RPC catch-up, coercion/censorship scenarios, and long soak. | Controlled testnet produces replayable reports showing liveness recovery, bounded resource use, stable memory, topology diversity, and clean evidence replay. |
| P2 | PF-L1-TN9 Adversarial Soak | Partially done | Extend the simulator and remote drills for delayed, duplicated, dropped, partitioned, stale, equivocated, invalid-signature, and failed-leader cases under load. | Safety invariant holds: no two conflicting blocks commit at the same height; liveness recovers after faults clear within the stated model. |
| P3 | PF-L1-TN10 Benchmarks And Release Evidence | Pending | Publish transparent-transfer throughput, PQ signature byte overhead, compact and full certificate sizes, finality latency, CPU/memory/disk use, and release-join evidence. | Benchmark reports are reproducible from repo scripts, tied to a release package, and include the chained-HotStuff-versus-DAG availability decision gate. |
| P3 | PF-L1-TN10 Independent Operator Expansion | Later | Add external operators only after the self-operated controlled network is stable and no single funder, jurisdiction, operator group, or infrastructure provider can control a quorum or blocking minority. | Broader public-testnet readiness includes independent operators, topology diversity, funding diversity, and no single correlated blocking minority. |
| P1 | PF-L1-TN11 Confidential Settlement v1 | Critical parallel workstream | Replace the debug proof/encryption path with production journal/witness types, zkVM/STARK proof backend, ML-KEM envelopes, disclosure policy, wallet/RPC flows, and proof/ciphertext fee pricing. | Shielded value has production PQ note encryption, verifier-enforced real proofs, local/remote P0 evidence, benchmarks, and reviewed circuit/AIR constraints. |
| P4 | PF-L1-TN11 Bridge Custody | Later/R&D | Keep bridge work as a harness until custody, external-chain security, witness economics, and audits exist. | No external asset custody claim appears in controlled-testnet artifacts. |

## Current 8-Hour State: Validator Lifecycle Governance

The registry lifecycle path is now gate-backed through post-membership-change
ordering. The latest 4-validator registry smoke:

```text
reports/testnet-validator-registry-update/run-20260512T091719Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/run-20260512T094219Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/run-20260512T100206Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/run-20260512T102729Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/emergency-key-rotation-20260512T145238Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/f-plus-one-halt-20260512T153304Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/stale-vote-20260512T155815Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/failed-leader-20260512T162743Z/testnet-validator-registry-update-smoke.json
reports/testnet-validator-registry-update/partition-20260512T165411Z/testnet-validator-registry-update-smoke.json
```

has `validator_registry_update_ok: true`, live rotate/admit/suspend activation,
registry-checked key staging, emergency stale-key rejection, explicit
non-contiguous active validators
`[validator-0, validator-2, validator-3, validator-4]`, and
`multi_process_post_suspend.*` booleans true. The latest run certifies heights
6-8 through separate-node peer-certified rounds sourced by `validator-3`,
`validator-4`, and `validator-0`, with split-key validator services restarted
each height and convergence across all active nodes. It then certifies height 9
with proposer `validator-2` while `validator-4` is offline, records one failed
vote request/send, keeps quorum at 3-of-4, verifies `validator-4` stayed at
height 8 during the outage, replays the archived certified batch into the
recovered service, and converges all active nodes at height 9. The newest run
then starts the peer-certified loop again with no explicit `START_HEIGHT`;
`validator-3` derives height 10 from local state, gathers all 4 active votes,
and converges every active node after restart/resume. The newest run then
finalizes height 11 with `validator-3` lagging, serves `validator-4` over
read-only RPC, runs `postfiat-node rpc-catch-up` on `validator-3`, applies one
certified archived batch, and converges all active nodes at height 11.
The failed-leader view-change drill proves `failed_leader_verified: true`:
view 0 routes to offline `validator-0`, the remaining three active validators
form a timeout certificate, view 1 routes to `validator-2`, height 12 certifies
with exactly 3-of-4 active votes, and recovered `validator-0` catches up from
the certified batch.
The partition drill proves `partition_verified: true`: it splits the latest
post-change active set into non-quorum sides, collects only two vote artifacts
against quorum three at height 13, writes no certificate, archives no batch,
and keeps all active nodes at height 12.
The emergency key-rotation run proves `emergency_key_rotation_ok: true`,
`stale_key_stage_rejected: true`, `stale_key_vote_rejected: true`,
`replacement_key_vote_ok: true`, and `rotated_public_key_changed: true`.
The f+1 halt drill proves `f_plus_one_halt_verified: true`, `offline_count: 2`,
`quorum: 3`, `expected_vote_count: 2`, `command_failed_below_quorum: true`,
`batch_not_archived: true`, and `no_state_advance: true`.
The latest stale-vote drill proves `stale_vote_verified: true`, rejects a
height-12 vote against a height-13 proposal, writes no certificate, and keeps
all active nodes at height 12.

The aggregate readiness report:

```text
reports/testnet-readiness-gate/testnet-readiness-gate-20260512T091831Z.json
reports/testnet-readiness-gate/testnet-readiness-gate-20260512T094350Z.json
reports/testnet-readiness-gate/testnet-readiness-gate-20260512T100342Z.json
reports/testnet-readiness-gate/testnet-readiness-gate-20260512T102926Z.json
reports/testnet-readiness-gate-emergency-key-rotation-check/testnet-readiness-gate-20260512T145446Z.json
reports/testnet-readiness-gate-emergency-key-rotation-doc-check/testnet-readiness-gate-20260512T150850Z.json
reports/testnet-readiness-gate-f-plus-one-halt-check/testnet-readiness-gate-20260512T153515Z.json
reports/testnet-readiness-gate-stale-vote-check/testnet-readiness-gate-20260512T160029Z.json
reports/testnet-readiness-gate-failed-leader-check/testnet-readiness-gate-20260512T163011Z.json
reports/testnet-readiness-gate-partition-check/testnet-readiness-gate-20260512T165731Z.json
```

requires that multi-height post-suspend path and its non-skipped partial-outage
recovery, restart/resume, and RPC catch-up reports alongside the existing
restart, normal-run outage, RPC catch-up, write-edge, fee/reserve,
emergency key-rotation stale-key rejection, post-change f+1 halt-threshold
evidence, post-change failed-leader view-change recovery, post-change
partition safety, post-change stale-vote rejection, compact-certificate, and
certificate-size checks. The P0
wrapper:

```text
reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T092228Z.json
reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T094806Z.json
reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T101054Z.json
reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T103758Z.json
reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T111446Z.json
reports/testnet-p0-network-gate-remote-registry-drill-rerun/testnet-p0-network-gate-20260512T123430Z.json
reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/testnet-p0-network-gate-20260512T132218Z.json
```

now has a current 5-validator remote-mode pass:
`reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/testnet-p0-network-gate-20260512T132218Z.json`
with `status: "passed"`, `mode: "remote"`, `remote_blocked: false`,
`p0_network_ok: true`, `core_p0_checks_ok: true`,
`validator_registry_update_ok: true`, `remote_normal_run_ordering_ok: true`,
`remote_restart_recovery_ok: true`, `remote_partial_outage_quorum_ok: true`,
`remote_rpc_catchup_ok: true`, `remote_validator_registry_update_ok: true`,
`remote_validator_registry_fault_tolerance_ok: true`,
`final_convergence_ok: true`, and `certificate_size_metrics_ok: true`. The
previous 13:16 report remains useful as the latest explicit local-fallback
proof.

The latest remote registry drill report:

```text
reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/logs/remote/validator-registry/testnet-remote-validator-registry-drill-20260512T134336Z.json
```

suspends `validator-4` at governance height 6, certifies height 7 with
`[validator-0, validator-1, validator-2, validator-3]`, then certifies height 8
with `validator-3` offline. The height-8 certificate carries exactly 3-of-4
active votes, records failures for both `validator-3` and suspended
`validator-4`, replays the archived certified batch into recovered
`validator-3`, and converges the active set.

The minimum wallet path is also now gate-backed:

```text
reports/testnet-wallet-minimum-smoke/manual/testnet-wallet-minimum-smoke.json
reports/testnet-readiness-gate-wallet-minimum-check/testnet-readiness-gate-20260512T143623Z.json
```

has `wallet_minimum_ok: true`, `deterministic_restore_ok: true`,
`private_file_modes_ok: true`, `rpc_account_query_after_fund_ok: true`,
`signed_transfer_submission_ok: true`, `signed_transfer_apply_ok: true`,
`first_spend_public_key_binding_ok: true`, and `recipient_credit_ok: true`.
The smoke is called by `scripts/testnet-readiness-gate`, so the wallet path is
now regression-gated rather than an isolated demo.

The offline wallet signing path is now gate-backed:

```text
reports/testnet-wallet-sign-transfer-smoke/rpc-submit/testnet-wallet-sign-transfer-smoke.json
reports/testnet-p0-network-gate-local-wallet-rpc-submit/testnet-p0-network-gate-20260512T221256Z.json
```

has `wallet_sign_transfer_ok: true`, `rpc_fee_quote_ok: true`,
`signed_transfer_matches_quote: true`,
`rpc_signed_transfer_submission_ok: true`, `signed_transfer_apply_ok: true`,
`receipt_matches_quote: true`, and `first_spend_public_key_binding_ok: true`.
The readiness gate now calls the smoke, and P0 surfaces
`wallet_sign_transfer_ok` plus `wallet_sign_transfer_metrics`, including RPC
submit request/OK/error counters.

Wallet vectors are now gate-backed too:

```text
reports/testnet-wallet-test-vectors-smoke/manual/testnet-wallet-test-vectors-smoke.json
reports/testnet-wallet-test-vectors-smoke/manual/wallet-test-vectors.public.json
```

has `wallet_test_vectors_ok: true`, `deterministic_vector_replay_ok: true`,
`vectors_unique_ok: true`, `vector_signatures_ok: true`,
`signed_transfer_apply_ok: true`, `first_spend_public_key_binding_ok: true`,
and `recipient_credit_ok: true`.

Keep the P0 gate as the regression command. It emits
`reports/testnet-p0-network-gate/*.json` with these top-level booleans and
summary fields:

- `p0_network_ok`
- `configured_gate_ok`
- `mode` as `remote` or `local_fallback`
- `remote_blocked` plus a reason when machine access or credentials are absent
- `provision_ok`
- `normal_rounds_ok`
- `restart_recovery_ok`
- `partial_outage_quorum_ok`
- `rpc_catchup_ok`
- `rpc_edge_exhaustion_ok`
- `rpc_edge_exhaustion_metrics`
- `fee_reserve_policy_ok`
- `fee_reserve_policy_metrics`
- `write_edge_load_ok`
- `write_edge_load_metrics`
- `validator_registry_update_ok`
- `local_validator_registry_fault_drills_ok`
- `wallet_minimum_ok`
- `wallet_sign_transfer_ok`
- `wallet_sign_transfer_metrics`
- `wallet_test_vectors_ok`
- `wallet_test_vectors_metrics`
- `final_convergence_ok`
- `private_key_policy_ok`
- `certificate_size_metrics_ok`
- `remote_normal_run_ordering_ok`
- `remote_restart_recovery_ok`
- `remote_partial_outage_quorum_ok`
- `remote_rpc_catchup_ok`
- `remote_validator_registry_update_ok`
- `remote_validator_registry_fault_tolerance_ok`
- `remote_emergency_key_rotation_rehearsal_ok`
- `remote_placement_capacity_profile_recorded`
- `remote_placement_capacity_gate_ok`
- `remote_placement_manifest_present`
- `remote_placement_public_expansion_capacity_gate_ok`
- `remote_placement_controlled_missing_independent_groups`
- `remote_placement_public_missing_independent_groups`
- `remote_topology_capture_profile_recorded`
- `remote_topology_single_group_can_block_quorum`
- `remote_topology_single_group_can_reach_quorum`
- `remote_topology_minimum_groups_to_block_quorum`
- `remote_topology_minimum_groups_to_reach_quorum`
- `remote_topology_capture_threshold_profile_ok`
- `remote_topology_capture_thresholds`
- `remote_topology_capture_profile_ok`

Next timebox:

1. Done: the manually chained remote P0 evidence has been collapsed into one
   strict 5-validator remote gate with topology capture required, and rerun
   after adding remote RPC edge-load to the readiness/P0 path:
   `reports/testnet-p0-network-gate-remote-edge-load-full/testnet-p0-network-gate-20260513T045612Z.json`.
   It reports `status: "passed"`, `p0_network_ok: true`,
   `configured_gate_ok: true`, `remote_blocked: false`,
   `remote_readiness_ok: true`, `remote_normal_run_ordering_ok: true`,
   `remote_rpc_edge_load_ok: true`,
   `remote_restart_recovery_ok: true`, `remote_partial_outage_quorum_ok: true`,
   `remote_rpc_catchup_ok: true`,
   `remote_validator_registry_fault_tolerance_ok: true`, and
   `remote_emergency_key_rotation_rehearsal_ok: true`.
2. Done: the controlled-testnet placement manifest now lives at
   `docs/status/controlled-testnet-placement-manifest.json`. It is sanitized,
   source-evidence-bound to the passing remote P0 report, and verified by
   `scripts/testnet-placement-manifest-verify`. Manifest-backed capacity
   evidence:
   `reports/testnet-remote-placement-capacity-profile/current-manifest/testnet-remote-placement-capacity-profile-20260513T043654Z.json`.
   Release package/gate/candidate-gate paths fail closed if the manifest is
   missing, stale, unverifiable, or leaks credential/IP material. Debug
   release-package evidence:
   `reports/testnet-release-packages-placement-manifest-check/testnet-release-package-20260513T043743Z/manifest.json`.
   Release packages, release gates, and release-candidate gates also bind to
   passed remote P0 evidence through `P0_NETWORK_GATE_REPORT` and package a
   `p0_network_gate` evidence record. Debug package evidence:
   `reports/testnet-release-packages-p0-network-gate-check/testnet-release-package-20260513T052724Z/manifest.json`.
3. Done: `scripts/testnet-remote-rpc-edge-load` is called by remote readiness
   and surfaced by P0 as `remote_rpc_edge_load_ok`. It sends oversized request
   envelopes to all five live validators, requires fail-closed
   `rpc_request_too_large` responses, then proves valid read RPC and
   convergence still hold:
   `reports/testnet-p0-network-gate-remote-edge-load-full/logs/remote/readiness/logs/remote-rpc-edge-load.json`.
4. Done: release host and join evidence now work on the three-machine current
   bucket without remote `jq`. The host preflight passed with 5 validators and
   3 machines:
   `reports/testnet-release-remote-preflight-live-nojq/testnet-release-remote-preflight-20260513T053552Z.json`.
   The remote join dry-run passed across all 5 validator slots:
   `reports/testnet-release-remote-join-dry-run-live-nojq/testnet-release-remote-join-dry-run-20260513T053928Z.json`.
5. Done: a fresh 5-validator current-bucket deploy converged at height 1, then
   a short remote soak advanced transparent and governance rounds to height 3
   with RPC tamper and restart evidence:
   `reports/testnet-remote-ssh-smoke-current-bucket/testnet-remote-ssh-smoke-20260513T-current-bucket.json`,
   `reports/testnet-remote-soak-live-release-gate-short/testnet-remote-soak-20260513T055258Z.json`, and
   `reports/testnet-remote-soak-live-release-gate-short/checkpoints/testnet-remote-soak-checkpoint-20260513T-live-release-gate-short.json`.
6. Done: the debug release gate now passes with exact remote join enabled, so
   the package generated by the gate is the same artifact fake-root joined on
   all 5 validator slots:
   `reports/testnet-release-gate-live-current-bucket-exact-join/testnet-release-gate-20260513T-live-current-bucket-exact-join.json`.
   It reports `release_gate_ok=true`, `same_package_rehearsed=true`,
   `exact_remote_join_dry_run_ok=true`, and
   `exact_remote_join_matches_artifact=true`.
7. Started: an 8-hour current-bucket soak is running as AI job
   `remote-soak-current-bucket-8h` against
   `reports/testnet-remote-deploy-plans-current-bucket/testnet-remote-deploy-plan-20260513T054436Z.json`.
   It is configured for 5 validators, transparent/governance rounds,
   `TAMPER_EVERY=4`, `RESTART_EVERY=12`, `SNAPSHOT_EVERY=24`, and
   `DURATION_SECONDS=28800`. The live checkpoint after one iteration is
   `reports/testnet-remote-soak-current-bucket-8h/checkpoints/testnet-remote-soak-checkpoint-live-running.json`
   with `checkpoint_ok=true`, latest height 5, zero lag, and gates passing.
   The final-candidate readiness report is intentionally pending at
   `reports/testnet-release-final-candidate-current-bucket-pending/testnet-release-final-candidate-pending.json`
   with blockers: dirty worktree, soak job not yet succeeded, and soak duration
   below 8 hours.
8. Done: `scripts/testnet-remote-soak` now supports sparse tamper cadence for
   long soaks. `all_tamper_ok` now means every tamper run passed, not that every
   iteration had a tamper run, so `TAMPER_EVERY > 1` no longer creates a false
   final-report failure.
9. Done: release-candidate evidence plumbing now handles the no-`jq` release
   path. `scripts/testnet-release-candidate-gate` discovers latest host
   preflight and remote join evidence across all matching report directories,
   including `live-nojq`; `scripts/testnet-release-final-candidate` and
   `scripts/testnet-release-final-candidate-watch` pass explicit placement,
   P0, host-preflight, and remote-join evidence paths through to the candidate
   gate. Pending final-candidate evidence with those paths plumbed:
   `reports/testnet-release-final-candidate-current-bucket-pending-evidence-plumbed/testnet-release-final-candidate-pending.json`.
10. Next 120 minutes: turn this from debug release-gate evidence into
   release-candidate evidence. The blockers are a clean reviewable worktree,
   rerunning strict remote P0 on the current bucket if the packaged P0 evidence
   must be current-bucket-specific, and replacing the short soak with the
   required long-soak policy. Keep local release-gate scratch trees private:
   extracted join/tamper subtrees contain generated test private keys even
   though the package and top-level evidence reports exclude private material.
11. Final 120 minutes: rerun targeted verifier tests, `git diff --check`,
   `cargo fmt --check`, `scripts/testnet-readiness-gate`,
   `P0_MODE=local scripts/testnet-p0-network-gate`, and `scripts/check`; update
   this roadmap and the AI handoff with exact evidence.

Non-goals for this 8-hour window:

- no new privacy or bridge claims,
- no wallet UI beyond the minimum CLI/SDK path,
- no new isolated smoke scripts that are not called by readiness or P0 gates,
- no broad consensus rewrite beyond registry binding and certificate evidence.

## Near-Term Execution Order

1. Extend adversarial simulation and remote drills around partitions, stale
   votes, equivocations, restarts, and RPC catch-up.
2. Package operator emergency-rotation runbook evidence around the
   readiness-gated stale-key and stale-vote drills.
3. Complete remaining fee/reserve economics: broader state-expanding operation
   pricing, load evidence, and governed parameter-change planning.
4. Finish ML-DSA wallet SDK wrapping, key rotation, account-discovery metadata,
   and recovery test vectors.
5. Run the chained-HotStuff-versus-DAG availability decision gate before broader
   public testnet expansion.
6. Run the self-operated controlled testnet with long soak and release-join
   evidence, then add independent operators after the self-operated network is
   stable.
7. Advance Confidential Settlement v1 in parallel with the network substrate:
   production journal/witness types, zkVM/STARK proof backend, ML-KEM note
   envelopes, disclosure policy, wallet/RPC flows, and fee pricing. Keep bridge
   custody separate until its custody and external-chain assumptions are ready.

## Decision Posture

The reports do not invalidate the MVP. They clarify that the MVP is the
foundation for the controlled-testnet work, not the end state. The technical
judgment is to harden the narrow chain that matters first: transparent
settlement, default HotStuff-family ordering, registry-backed compact
certificates, Cobalt-governed validator evolution, post-quantum authorization,
edge/mempool safety, burned-fee accounting, and operator-grade evidence. Privacy
and bridge work continue, but they do not block the first self-operated
controlled testnet unless we choose to make private value or real external
custody part of that first scope.
