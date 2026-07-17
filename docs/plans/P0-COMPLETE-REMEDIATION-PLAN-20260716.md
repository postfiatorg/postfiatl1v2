# P0 Complete Remediation and Network-Preservation Plan

**Date:** 2026-07-16
**Owner:** Post Fiat protocol engineering
**Scope:** all confirmed P0 findings in the open-source productionization audit
**Status:** active master burn-down; an unchecked item is not complete
**Source register:** `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md`
**Capability design detail:** `docs/plans/OPEN-SOURCE-CAPABILITY-RESTORATION-PLAN-20260716.md`
**Eight-hour closure sequence:** `docs/plans/OPEN-SOURCE-EIGHT-HOUR-CLOSURE-PLAN-20260717.md`

> **URGENT — COMPANY-CRITICAL P0/P1 REMEDIATION:** This program is an immediate
> customer-demo, investor-credibility, and public-release priority. Work the
> confirmed findings continuously in strict P0-then-P1 order. Do not divert to
> discretionary feature work, cosmetic documentation work, or unrelated protocol
> experiments while a remediation item is actionable. Disabling, deleting, or
> hiding a core capability is not remediation: preserve or restore the capability
> through a production-safe implementation and prove it at the real boundary.
> Batch destructive migrations and shared-devnet resets to minimize downtime, but
> do not defer a required reset when it is the fastest safe path. Escalate only a
> genuine secret, spend/signing requirement, or human product decision; otherwise
> keep implementing, testing, and recording evidence until the burn-down is green.
>
> **FOUNDER URGENCY DIRECTIVE:** Treat this burn-down as an active company-level
> emergency. Restoring and proving production-safe core network functionality is
> the highest-priority engineering work; proceed continuously, report blockers
> immediately, and do not represent the repository as ready for a customer demo,
> investor review, or public release until every required P0 gate is evidence-green.

## 1. Completion rule

This file is the implementation plan for every P0. It is not a containment or
documentation exercise.

- `[ ]` means incomplete.
- `[x]` means the named evidence exists and has been rerun successfully against
  the exact implementation tree identified in the evidence entry.
- Disabling, deleting, hiding, or making a core capability historical-only does
  not complete its remediation checkbox.
- A local patch is not P0 closure. Closure requires the real-boundary regression,
  the positive capability test, integrated gates, migration/replay evidence, and
  immutable-candidate evidence.
- No test may be weakened, skipped, or deleted to obtain a checkmark.
- Every checked item must be accompanied in the audit lab book by the command,
  result, tree/commit, and artifact path or hash.

## 2. Global gates

- [x] Freeze one reviewed implementation tree containing all P0 remediations as
  separable commits.
- [x] Prove every protocol encoding and state transition has a version and an
  unambiguous activation rule.
- [x] Replay retained history byte-identically through every activation boundary.
- [x] Prove crash recovery, snapshot restore, lagging-node catch-up, and rollback
  before activation.
- [x] Pass `cargo fmt --all -- --check` on the immutable candidate.
- [x] Pass `cargo check --workspace --all-targets --locked` on the immutable candidate.
- [x] Pass `cargo clippy --workspace --all-targets --locked -- -D warnings` on the immutable candidate.
- [x] Pass `cargo test --workspace --all-targets --locked` without skipped privacy,
  consensus, replay, or migration suites. The ordinary workspace passed in
  full; every intentionally ignored security/release target was then selected
  explicitly: Orchard `17/17`, atomic-swap six-process `1/1`, FastSwap
  six-process/restart `1/1`, FastSwap 100-operation process and in-process gates
  `2/2`, and the two Foundry/Anvil bridge gates `2/2`.
- [x] Pass wallet-web and wallet-proxy tests, production build, and zero-vulnerability
  npm audits.
- [x] Pass offline Foundry and pinned official-mainnet-fork suites.
- [x] Pass strict documentation, link, claim-boundary, secret/history, artifact,
  dependency, provenance, and deterministic-SBOM gates.
- [x] Pass deterministic `n=4` and `n=6` consensus/FastPay simulations under delay,
  loss, duplication, reorder, partition, Byzantine behavior, and restart.
- [x] Pass a six-node rolling-upgrade and activation drill in an isolated clone;
  the shared devnet may use a planned clean reset when that is faster or safer.
- [x] Pass the complete customer flow from a fresh wallet with accepted receipt codes,
  exact conservation, and no secret crossing the browser boundary.
- [x] Produce a closure table tied to the exact public-candidate commit with zero
  open P0s. The provider owner confirmed destruction for `P0-SECRET-01`; the
  private terminal-action record and sanitized public GitHub clone pass the gate.

## 3. P0 remediation burn-down

### P0-CONSENSUS-01 — production multi-view consensus

**Problem:** production did not implement the claimed safe view-change/QC protocol;
the temporary local response rejects nonzero views and can halt after a failed proposer.

- [x] Preserve the real cross-view conflicting-vote counterexample as a regression.
- [x] Define canonical, domain-separated proposal, prepare/precommit vote, QC,
  timeout-vote, and timeout-certificate v2 encodings.
- [x] Bind every artifact to chain/genesis, committee epoch/root, height, view,
  proposal, parent, state root, validator, and phase.
- [x] Replace opaque/lexicographic `high_qc_id` handling with a verified QC graph.
  Pre-activation legacy aggregation fails closed on heterogeneous opaque IDs;
  activated production transport uses typed, signature-verified QC references.
- [x] Persist `highest_voted_view`, `locked_qc`, `high_qc`, and signed vote digest
  atomically before returning a signature. Prepare, precommit, and timeout
  transport signers all call the durable authorization store before signing.
- [x] Enforce the safe-vote rule across views and restarts.
- [x] Implement the selected explicit prepare/precommit rule in the production
  node path; only a non-nil precommit QC commits, and lone prepare/direct legacy
  certificates reject after activation.
- [x] Implement signed timeout certificates and deterministic proposer rotation.
- [x] Add versioned activation and byte-identical pre-activation replay.
  Genesis/topology carry an exact positive activation height; the height router
  preserves the legacy certificate/block-hash path below it and requires v2 at
  and above it. The legacy genesis golden hash remains byte-identical when the
  optional field is absent. A real six-node activation-height-2 TCP regression
  commits/replays the legacy height 1 before v2 view recovery at height 2.
  Snapshot v6 preserves signer safety/QC state; v5 restores only a never-activated
  genesis and fails closed for an activated signer. Existing networks therefore
  use the founder-authorized deterministic reset/new-genesis boundary.
- [x] Model quorum intersection, lock monotonicity, conflicting QCs, timeouts,
  equivocation, partitions, and crash recovery for `n=4` and `n=6`. The
  adversarial campaign covers delayed/reordered votes, duplicate delivery,
  under-quorum partitions, Byzantine conflicting reproposals, signed timeout
  recovery, and restart-preserved locks at both committee sizes.
- [x] Prove the shipping finality RPC automatically advances from a failed
  proposer to a timeout-certified later view and returns the committed result
  without operator orchestration. Commit `09125687` makes the normal proxy path
  collect a distinct-validator timeout quorum against one exact parent, submit
  the compressed bounded vote envelope to the deterministic later-view proposer,
  aggregate and verify the timeout certificate locally, and return the committed
  result. The real shipping RPC passes at `n=4` in 80.51 seconds and `n=6` in
  122.55 seconds; timeout-vote durability, truncated-envelope, proxy routing,
  browser exclusion, affected check, formatting and strict Clippy gates pass.
- [x] Remove the nonzero-view rejection only after all preceding safety/liveness gates pass.
  The activated v2 path accepts only the immediately following view with a
  verified signed timeout certificate and an exact high-QC ancestry; legacy and
  malformed later-view proposals still fail closed.
- [x] Update the whitepaper to the exact implemented commit/view-change protocol.

**Primary code:** `crates/node/src/block_finality.rs`,
`crates/node/src/consensus_artifacts.rs`, `crates/node/src/batch_snapshot.rs`,
`crates/ordering_fast/src/lib.rs`, and consensus wire types under `crates/types/`.

### P0-CUSTODY-01 — browser seed exported to wallet proxy

**Problem:** the browser could send its master seed/backup to a backend signer.

- [x] Remove backup-bearing wallet signer RPC methods from the public browser client.
- [x] Remove native custody signing and temporary backup-file dispatch from the public proxy.
- [x] Force owned-transfer and unwrap signing through local wallet/WASM code.
- [x] Add negative RPC tests for removed custodial methods.
- [x] Run a browser network-capture test covering every supported money operation
  and prove no seed, backup, private key, or signing authority leaves the browser.
- [x] Scan proxy logs, subprocess arguments, crash artifacts, and browser storage
  after the complete flow.
- [x] Rerun wallet/proxy tests and build on the immutable candidate.

### P0-CUSTODY-02 — redacted vector command printed caller seeds

**Problem:** a supposedly redacted CLI report serialized the supplied master and
signature seeds.

- [x] Remove secret fields from the report schema and version it as v2.
- [x] Add a unit regression rejecting both secret field names and supplied values.
- [x] Run the shipping CLI subprocess boundary and prove stdout omits names and values.
- [x] Scan stderr, logs, panic output, temporary files, and crash reports under
  successful and failing invocations.
- [x] Rerun the subprocess regression on the immutable candidate.

### P0-GOVERNANCE-01 — unsigned governance and validator rotation

**Problem:** caller-supplied validator names could stand in for cryptographic
governance authorization; the temporary response removed the live path.

- [x] Preserve the no-private-key governance forgery as a regression.
- [x] Define a versioned signed governance vote envelope.
- [x] Bind votes to chain/genesis, complete proposal payload/kind, proposal slot,
  old registry root, committee/key epoch, validator, activation, and expiry.
- [x] Verify distinct signatures against the old active registry at admission,
  proposal construction, block validation, execution, replay, and state verification.
- [x] Implement old-rules-authorize-new-rules validator rotation with delayed activation.
- [x] Implement signed pause/unpause, protocol activation, crypto-policy, bridge,
  and FastSwap policy changes through the same boundary.
- [x] Sign and verify authoritative RBC/ABBA messages or remove them from the
  production governance call graph in favor of the signed envelope.
- [x] Add missing/duplicate/wrong-domain/wrong-chain/wrong-slot/stale-key/
  stale-registry/altered-payload no-mutation tests.
- [x] Add partition, concurrent-amendment, crash, replay, rotation, and rollback tests.
- [x] Prove a real signed amendment and registry rotation converge on six nodes.
- [x] Restore live governance only through the signed path.

**Primary code:** `crates/types/src/shielded_bridge_governance.rs`,
`crates/consensus_cobalt/`, `crates/node/src/governance.rs`,
`crates/node/src/consensus_artifacts.rs`, and `crates/node/src/execution_actions.rs`.

### P0-PRIVACY-01 — legacy cleartext notes admitted as shielded

**Problem:** a legacy operation labeled shielded exposed owner, asset, amount, and memo.
The secure product capability is Asset-Orchard; the insecure encoding must never be live.

- [x] Preserve the pre-fix cleartext admission reproduction.
- [x] Reject new legacy cleartext mint/spend actions at live creation and admission.
- [x] Keep historical decoding isolated from live admission.
- [x] Route supported privacy operations through Asset-Orchard.
- [x] Prove every public wallet/proxy/CLI/API privacy mutation creates only the
  current Asset-Orchard action version.
  Repo-wide production call-site inventory finds no legacy action constructor
  in any client, RPC or CLI wrapper. Direct legacy mint/spend and both legacy
  batch builders reject without mutation; wallet/proxy ingress is schema-v2;
  consensus admission independently rejects manually serialized legacy actions.
- [x] Prove historical replay cannot be used to inject a new legacy action.
  Public shielded apply commits only a rejected receipt and leaves shielded
  state unchanged. Authenticated catch-up now also requires the archived block
  to attach to the exact current parent and its post-execution state root and
  receipt IDs to match deterministic recomputation (including only the exact
  chain/height-bound legacy root schemas required for retained history).
- [x] Run end-to-end deposit, transfer, swap, and egress with privacy leakage scans.
  The real ordered-store boundary passes encrypted v2 issued-asset ingress,
  generic private transfer, a K15 private atomic swap, chain-only output
  recovery and private egress. Every money receipt is accepted, spent inputs
  are nullified, the public egress delta is exact, global issued supply is
  conserved, and exact private material is absent from 13 public
  envelope/batch/archive/block/receipt/ledger/state artifacts. Evidence:
  `reports/open-source-p0-privacy-complete-flow-20260716T235019Z/ACCEPTANCE.json`
  (`sha256:b830ce023d078aeed4acc832679591072519827f60af72d778b144dc5d5672ec`).
- [x] Rerun full Orchard proving/verifying and replay suites on the immutable candidate.
  The complete workspace run passed the ordinary Orchard/replay surface
  (`83/83`, with 17 release-scale tests intentionally selected separately).
  The explicit release run passed all 16 proving/verifying, tamper, authority,
  anchor/path, conservation, egress and key-metadata tests; the seventeenth
  parameter-artifact writer passed separately and reproduced the committed
  2,097,220-byte artifact exactly (SHA-256
  `e1fb2974a4a0a87f8ac0dbaaa4c7ea3c4e9f293a560585f7ca6233b78f42d0dd`).

### P0-PRIVACY-02 — legacy ingress serialized complete note openings

**Problem:** v1 ingress and a browser fallback published note openings/plaintext.
The intended capability must remain available through the encrypted v2 path.

- [x] Reject v1 on live ingress while retaining authenticated archive replay.
- [x] Implement the v2 encrypted output envelope and browser decryption path.
- [x] Add serialized-field scans and issued-asset round-trip tests.
- [x] Capture the real wire, browser, proxy log, receipt, and ledger artifacts for
  all private operations and prove no note opening or spend authority is exposed.
  A real Chromium HTTP/WebSocket capture, wallet/proxy custody-boundary tests,
  and the ordered-store complete-flow scan cover the browser/wire, relay ingress,
  receipts, block/archive log, ledger and shielded-state representations. The
  boundary also fails closed on JSON-looking strings larger than its recursive
  inspection budget, preventing a transport-sized serialized action from
  bypassing the scan. Evidence is the complete-flow artifact above plus
  `reports/open-source-p0-privacy-browser-20260716T231154Z/ACCEPTANCE.json`
  (`sha256:e07bacd757b646022a13bdf28a9730a30a158920141eaf73677ee92f1597df01`).
- [x] Test wrong-recipient, malformed ciphertext, replay, downgrade, and mixed-v1/v2 attacks.
  Wrong recipients and authenticated-ciphertext mutation reveal nothing;
  malformed/plaintext envelopes reject; accepted v2 batches cannot replay; v1
  and mixed v1/v2 batches fail live admission and execution without mutation.
- [x] Prove v2 works end to end from two fresh wallets on the immutable candidate.
  Candidate commit `d1e68ee8` funds two distinct new accounts, establishes
  independent trustlines and issued holdings, creates separate encrypted-v2
  notes, verifies a real K15 cross-wallet private swap, recovers an output from
  chain ciphertext, and privately egresses it with accepted receipt code,
  exact public delta, nullification, conservation and zero leaks across 13
  public artifacts. Evidence:
  `reports/open-source-p0-privacy-two-wallet-20260717T052157Z/ACCEPTANCE.json`
  (SHA-256
  `ffcd818b23f60cc81eaa660d2b0f01bb0fddc9e6a593e9758bce842fa2686978`).

### P0-PUBLIC-EVIDENCE-01 — private note openings in public evidence

**Problem:** tracked and historical evidence contained complete private note openings.

- [x] Extend the scanner to detect note-opening and spend-authority fields without echoing values.
- [x] Archive raw evidence into a deterministic restricted archive and remove it
  from the candidate tree.
- [x] Pass the current tracked-tree scan.
- [x] Pass a local one-commit sanitized-history rehearsal.
- [x] Freeze the final reviewed source tree.
- [x] Construct a final clean, non-shallow public staging repository containing
  only intended refs and the exact reviewed tree.
- [x] Pass current-tree and complete reachable-history scans with zero findings.
- [x] Record staging commit, tree, ref set, file count, scanner versions, and hashes.
- [x] Independently reproduce the exact staging clone from the recorded procedure.

### P0-WALLET-02 — unsafe wallet development server/dependencies

**Problem:** production-like wallet serving exposed development-server and
dependency vulnerabilities.

- [x] Replace the production path with hardened same-origin static serving.
- [x] Enforce loopback/CSP behavior and update vulnerable dependencies.
- [x] Pass wallet tests, production build, and zero-vulnerability audit locally.
- [x] Prove the actual demo/public service cannot enable Vite development serving
  or unsafe filesystem access through configuration drift.
- [x] Run browser CSP, origin, navigation, cache, and source-map disclosure tests.
- [x] Rerun build/tests/audit against the immutable candidate artifact.

### P0-WALLET-BRIDGE-DEST-01 — unsafe/hardcoded bridge destination

**Problem:** the wallet could target a retired/drained vault without an exact
governed address and runtime-code-hash binding. Disabling deposits is not closure.

**Architecture alignment:** the bridge keeps one stable accounting and lifecycle
core while governance ratchets the accepted evidence tier from independently
observed facts to receipt and finality proofs. The wallet and proxy must therefore
consume a complete, typed route profile from replicated chain state; a digest-only
amendment whose manifest preimage is supplied by the client is not authenticated
route discovery. The route profile and API must remain verifier-neutral so stronger
tiers do not require another wallet or accounting rewrite. The production target is
proof-verified entry and exit; any earlier tier remains explicit in state and UX.

- [x] Remove the unsafe default and fail closed on missing/mismatched code hash.
- [x] Add browser/backend/scanner regressions for the retired address and absent binding.
- [x] Define a versioned `VaultBridgeRouteProfile` containing the route and asset IDs,
  source chain, vault and token addresses and runtime code hashes, route epoch,
  activation and expiry, verifier kind and evidence tier, and the referenced
  confirmation/challenge/deadline policy.
- [x] Commit the complete canonical profile through signed governance into replicated
  state, with a deterministic active-profile pointer per asset. Do not require a
  caller, proxy, environment variable, or bundled file to supply the authoritative
  manifest preimage.
- [x] Commit every profile field and active pointer to the versioned state root and
  preserve byte-identical pre-activation replay, snapshot migration, and rollback.
- [x] Implement authenticated route discovery that accepts only an asset or route ID
  and returns the complete active profile from chain state, including its governance
  authorization, activation/freshness status, and exact verification tier.
- [x] Keep the proxy an untrusted transport: it may cache immutable release/topology
  data but may not select, substitute, or override any route/profile field.
- [x] Verify the connected source network, deployed vault/token runtime code hashes,
  route activation, expiry, and profile freshness before approval, quote, or signing.
- [x] Bind the exact route-profile hash and epoch into the wallet confirmation and
  signed ingress transaction. Bind egress to the same pinned source route directly
  or through an immutable bucket/withdrawal-packet reference that validators verify.
- [x] Define rotation semantics explicitly: new ingress uses only the latest active
  profile, while in-flight deposits and redemptions finish against their pinned
  profile unless governance executes a separately modeled emergency transition.
- [x] Return and display the active evidence tier and its concrete technical trust
  dependencies in the wallet; never infer a stronger tier from relayer output.
- [x] Expose and verify the full bridge conservation state `V = S + D + B - R`,
  distinguishing uncredited deposits, burned-unreleased redemptions, and
  released-unsettled redemptions. Any unexplained mismatch is fail-closed.
- [x] Prove route rotation, stale cache, wrong network, proxy substitution, and
  runtime-code change rejection, plus downgrade and false-tier rejection.
- [x] Complete one real deposit and withdrawal through the governed route with
  accepted receipt codes, pinned-profile evidence, and lifecycle conservation.
- [x] Prove the same wallet/API contract can select a stronger verifier profile without
  changing route authority, transaction accounting, or user-facing money semantics.

### P0-PROXY-AUTH-01 — unauthenticated public mutation/custody proxy

**Problem:** a public-bind/all-origins proxy exposed mutation and custody-dispatch routes.

- [x] Make safe loopback/origin behavior the default.
- [x] Remove native custody signer dispatch from the public proxy.
- [x] Add real HTTP/WebSocket boundary tests and exhaustive mutation-route classification.
- [x] Pass proxy tests and zero-vulnerability audit locally.
- [x] Implement and test the supported authenticated TLS/reverse-proxy deployment profile.
- [x] Prove authentication, authorization, CSRF/origin, replay, body/concurrency/rate
  limits, and WebSocket upgrade handling at the deployed edge.
- [x] Verify no unclassified mutation route exists on the immutable candidate.

### P0-SECRET-01 — credential reachable in repository history

**Problem:** publishing existing history could disclose a historical provider credential.

- [x] Add fail-closed exact-ref/tree and reachable-history publication gates.
- [x] Pass tracked-tree and local sanitized-history rehearsal scans.
- [x] Provider owner revokes/decommissions the captured credential and privately
  records provider-side evidence without committing the secret.
- [x] Search every candidate ref and tag; retain only the reviewed sanitized history.
- [x] Pass the final staging current/history scan with zero credential findings.
- [x] Record owner, timestamp, provider action reference, scanner result, exact refs,
  commit, and tree in the private release evidence.
- [x] Block publication automatically unless all preceding evidence is present.

### P0-ASSET-01 — legacy wrap could mint mislabeled issued assets

**Problem:** `wrap_owned(asset="pfUSDC")` could debit native PFT while labeling the
owned object as another asset.

- [x] Add execution and real RPC no-mutation regressions.
- [x] Restrict the legacy wrapper to native PFT semantics.
- [x] Preserve signed issued-asset deposit/custody paths separately.
- [x] Inventory every owned-object creation call site and prove asset/value source binding.
- [x] Fuzz wrong-label, unknown-asset, zero/overflow, replay, and concurrent-wrap cases.
- [x] Rerun execution/node/FastPay integration suites on the immutable candidate.

### P0-RPC-01 — arbitrary debit/funding RPC and incomplete safe replacement

**Problem:** an RPC could debit arbitrary accounts; removing it was necessary, but
FastPay funding is not complete until the signed replacement is available in the wallet.

- [x] Remove the arbitrary-debit remote RPC and its wallet/proxy exposure.
- [x] Add real-store no-mutation and remote-method rejection tests.
- [x] Define the signed, domain-separated account-to-owned/FastLane deposit transaction.
- [x] Bind sender, asset, amount, fee, sequence, destination owner key, chain/genesis,
  expiry, and nonce; verify before any debit or lock.
- [x] Commit the deposit through normal consensus with an accepted receipt code.
- [x] Implement wallet local signing, display, submission, finality, and balance refresh.
- [x] Prove fresh-wallet funding, duplicate/replay/wrong-owner/wrong-asset rejection,
  and exact account-plus-owned conservation on six nodes.
- [x] Restore complete FastPay funding UX only through the signed path.
  Browser and Python/WAN callers now both use the signed consensus deposit;
  neither public client exposes `wrap_owned` or `unwrap_owned`. Python migration
  evidence:
  `reports/open-source-p0-python-fastpay-signed-deposit-20260717T034903Z/ACCEPTANCE.json`
  (SHA-256
  `ac370c7dc094b49b5c6c9606b81fc4f6cf065acde5f0d3ffd14bd47320bf91d1`).

### P0-BRIDGE-01 — unverified external bridge transitions and refund race

**Problem:** external consume/import/refund paths trusted assertions and allowed a
late-consume versus refund safety race. Historical-only containment is not closure.

- [x] Preserve pre-fix asserted-event and refund-race reproductions.
- [x] Implement governed Ethereum finalized checkpoints binding chain, block/hash,
  authority epoch, bridge/token addresses, and runtime code hashes.
- [x] Verify Ethereum header/receipt/log inclusion and bind topic, emitter, token,
  amount, sender, recipient, route, nonce, transaction index, and log index.
- [x] Implement the PFTL finality/state/receipt proof consumed by Ethereum.
- [x] Define a single versioned packet state machine with mutually exclusive
  destination consumption and source cancellation/refund.
- [x] Require cryptographic cancellation evidence; elapsed local height alone is insufficient.
- [x] Persist consume/refund/replay state atomically across crashes and relayers.
- [x] Integrate every transition into the public/private/FastLane/external supply oracle.
- [x] Test reorg, wrong-chain/address/code/topic/token/amount/recipient/nonce,
  malformed proof, replay, delayed consume, concurrent refund, partition, and restart.
- [x] Complete deposit, consume, return, and refund end to end with accepted receipts
  and exact conservation.

**Primary code:** `crates/bridge/`, bridge execution/node workflows,
`crates/ethereum-contracts/`, and wallet/proxy bridge surfaces.

### P0-SUPPLY-01 — unverified settlement-backed mint release

**Problem:** Ethereum mint release could rely on caller-selected settlement claims.
An interface plus deployment block is not the final verifier.

- [x] Require a nonzero one-time `IMintSettlementVerifier` and exact proof/escrow binding.
- [x] Add forged amount/beneficiary/escrow/replay/replacement tests.
- [x] Select and specify the real PFTL-to-Ethereum finality verification mechanism
  and its explicit trust model.
- [x] Implement the production `IMintSettlementVerifier`; prohibit mock,
  owner-assertion, and placeholder deployments.
- [x] Bind chain/genesis, controller, route, pending/escrow ID, beneficiary, token,
  amount, nonce, finalized height/root, and proof digest.
- [x] Make verifier/code-hash governance timelocked and safe with unresolved escrows.
- [x] Add Foundry unit, fuzz, invariant, replay, replacement, and pinned-fork tests.
- [x] Deploy to a test environment and prove one release exactly matches finalized backing.
  An isolated PFTL ledger commits an accepted 110-atom backing receipt at a
  real state root. Isolated Anvil then deploys the production `MintController`
  and production 3-of-4 `ThresholdMintSettlementVerifier`; the certificate
  binds that height/root/receipt, releases the exact 110-atom escrow once, and
  leaves certified backing, released supply and beneficiary balance equal with
  zero escrow/unresolved obligations. Uncertified release and certificate/
  release replay reject. Evidence:
  `reports/open-source-p0-mint-settlement-anvil-20260716T235900Z/ACCEPTANCE.json`
  (`sha256:b9cf666416126a81c3ecf18bd9686e485e84c7db055fce48c834f7a0311f66fa`).
- [x] Prove aggregate PFTL plus Ethereum supply conservation across mint, return, and failure.
  One continuous governed-route test uses the same issued asset, isolated PFTL
  state and live Anvil vault through wrong-amount failure, valid claim/mint,
  return burn, source release and terminal settlement. The exact
  `V = S + D + B - R` oracle is green at all five checkpoints with zero
  unexplained delta; all 11 PFTL money receipts are accepted and all eight EVM
  receipts have status `0x1`. Evidence:
  `reports/open-source-p0-governed-bridge-aggregate-20260717T000800Z/ACCEPTANCE.json`
  (`sha256:0a18b1a9ab808fe74c2f70df0c4de1e2866a70990758af0a94b41096e400f8e9`).

### P0-NATIVE-SUPPLY-01 — genesis and fee-burn supply not fully bound

**Problem:** native genesis supply could be rewritten locally and some fee burns
were not reported in receipts/checkpoints.

- [x] Bind native supply into canonical genesis and constrain historical replay base.
- [x] Implement the live-custody plus explicit-fee-burn replay oracle.
- [x] Correct FastLane deposit/checkpoint fee reporting.
- [x] Implement checkpoint v2 cumulative-burn commitment and refuse unverifiable v1.
- [x] Complete an exhaustive inventory of every native custody/mint/burn/fee path.
- [x] Fuzz maximum arithmetic, duplicate custody, and unknown-lane classification;
  prove prune, tampered-archive no-mutation, and checkpoint restore at the real
  storage boundary.
- [x] Rebuild a legacy checkpoint from archive and prove the v1 refusal/recovery procedure.
- [x] Run genesis-to-tip, pruned-history, snapshot, and post-restore conservation on
  the immutable candidate.

### P0-ISSUED-SUPPLY-02 — issued supply omitted private/FastLane custody

**Problem:** moving issued assets into private or FastLane custody created false mint headroom.

- [x] Include FastLane reserves and Asset-Orchard live totals in aggregate supply.
- [x] Reject over-cap state roots and asset transactions before mutation.
- [x] Add exact-cap, over-cap, FastLane, and Asset-Orchard round-trip regressions.
- [x] Complete compiler-enforced custody/transition inventory coverage for every new lane.
- [x] Fuzz aggregate overflow and duplicate/unknown/unsupported custody; prove
  mint, burn, clawback, escrow, offer, and external-route transitions at their
  production execution boundaries.
- [x] Prove concurrent private egress admits exactly one spend, then verify exact
  issued supply through snapshot restore and block replay.
- [x] Prove transparent + private + FastLane + external totals stay within cap through
  a complete customer flow on the immutable candidate. The composed regression
  holds 30/20/25/25 atoms across those four lanes at a 100-atom cap, rejects a
  signed one-atom false-headroom mint before canonical mutation, and remains
  exact through every supply-neutral lane shift. The real release FastLane,
  BFT-checkpoint external, and two-fresh-wallet K15 private transition tests
  pass alongside it; acceptance SHA-256
  `993d961ada1d113a9bf91f74a17e430c87b8795439cf561ef310a23820b88d6a`.

### P0-COMMIT-ATOMICITY-01 — ordered commits admitted concurrent double application

**Problem:** the durable ordered-commit lock covered only journal persistence,
not the preceding read/execute phase. Concurrent callers could read the same
parent and unspent state and each report the same spend as accepted.

- [x] Preserve the real eight-worker same-nullifier reproduction: pre-fix, all
  eight disclosed-egress calls returned `accepted`.
- [x] Acquire the cross-process ordered-commit lock before journal recovery and
  every consensus-state read, then hold it through execution and persistence.
- [x] Require a typed `StorageMutationLock` witness at the locked journal writer
  and recovery helpers so commit callers cannot silently omit the guard.
- [x] Apply the same boundary to transparent, shielded, bridge, and governance batches.
- [x] Prove one concurrent egress accepts, seven fail at ordered-batch
  idempotency, supply remains exact, snapshot restore matches, and replay passes.
- [x] Rerun all batch-kind, crash-recovery, and full workspace gates on the
  immutable candidate. The complete node/workspace run covers transparent,
  shielded, bridge, governance, activation and FastPay ordered-commit paths,
  including 0..N persist-prefix recovery matrices, the eight-worker
  same-nullifier race, replay, snapshot and supply checks; all passed.

### P0-STATE-01 — state root omitted FastLane/FastSwap state

**Problem:** economically relevant FastLane/FastSwap fields did not affect the
replicated state root. The commitment and controlled-devnet rollout are now
complete.

- [x] Preserve the real equal-root counterexample.
- [x] Commit all ten identified FastLane/FastSwap fields with canonical encoding.
- [x] Add field-by-field sensitivity, ordering, activation, bootstrap, replay, and
  catch-up regressions.
- [x] Freeze the full replicated-state field inventory with a compile-time exhaustive boundary.
- [x] Specify state-root version activation, old-root replay, snapshot migration,
  rollback-before-activation, and mixed-version refusal.
- [x] Prove crash at every activation/persist boundary.
- [x] Run six-node shadow replay from the current devnet snapshot and compare every root.
  The old h1220 over-cap snapshot remains rejection evidence. The authorized
  clean reset established the byte-reproducible cap-valid candidate, crossed
  consensus-v2 at h1 with an accepted exact-six block, and its signed v6
  snapshot passes six independent exact-tip/root `verify-state` and
  `verify-blocks` replays. Acceptance SHA-256:
  `2269e611a93a2715c2746859c63e91eb577d79a8fbad5bedec029a6eb7083d73`.
- [x] Prove the compatible rolling deployment in an isolated six-node clone; on
  the shared devnet, either rolling-deploy or perform an authorized planned clean
  reset, whichever minimizes time and operational risk.
- [x] Activate once at a scheduled height; verify exact six-node roots and replay.
- [x] Prove rollback from the pre-activation snapshot and forward recovery in a drill environment.

Isolated drill evidence:
`reports/open-source-p0-state-six-node-20260716T184400Z/README.md`.

## 4. Critical P1 coupled to core restoration

### P1-FASTPAY-01 — abandoned lock cancellation and late certificates

This P1 is in the critical path because FastPay is a core customer-visible capability.
It may not be closed by default-disabling the lane.

- [x] Add a regression proving normal server startup exposes signed FastPay operations.
- [x] Restore FastPay default availability in the audit tree; retain an explicit
  emergency-disable option rather than an enable-only experimental flag.
- [x] Bind owner authorizations and validator votes to the exact chain, genesis,
  protocol version, and validator-registry identity; reject foreign domains before mutation.
- [x] Preserve distinct-validator certificate counting, signed admission,
  live-state checks, and durable lock-before-sign protections.
- [x] Keep the unsafe unlock operation fail-closed while the real protocol is implemented.
- [x] Define and model versioned object/lock IDs, bounded validity, decision
  certificates, certificate retrieval, and a consume-or-cancel fence. The
  initial `q-f` partial-vote recovery rule failed for `n=4` and was discarded;
  the selected model permits confirmation only from a complete `n-f`
  certificate, requires `n-f` durable apply acknowledgements for product
  finality, and otherwise orders cancel plus version advance. Model, spec and
  evidence: `docs/specs/fastpay-payment-recovery-v1.md` and
  `reports/open-source-p1-fastpay-recovery-model-20260717T001735Z/ACCEPTANCE.json`.
- [x] Prove a delayed certificate can never apply after cancellation/version advance.
- [x] Prove abandoned partial locks recover within a bounded period.
- [x] Add Byzantine, partition, withheld-broker, delayed-vote/certificate, expiry-race,
  restart, reconfiguration, replay, and crash-atomicity tests.
- [x] Implement safe unlock/cancel and restore complete wallet funding/send/unwrap UX.
  The replacement is the ordered v3 recovery decision, not deletion of the
  legacy lock file: no-certificate decisions atomically advance every input
  version, and delayed applies/reveals reject. A sub-quorum direct apply is
  speculative and rolls back on a quorum-certified omission while its full
  certificate remains available for recovery. Quorum-applied effects are bound
  into the next block and offline validators reconstruct them from certified
  evidence. The six-validator catch-up/minority-rollback regression, snapshot
  restore, 12 model tests, seven execution recovery tests, 14 node FastPay tests,
  wallet-web 240/240 and wallet-proxy 23/23 are green; strict affected-crate
  Clippy is green. Production recovery evidence:
  `reports/open-source-p1-fastpay-production-recovery-20260717T031617Z/ACCEPTANCE.json`
  (SHA-256
  `50bffc346fa91c728d140de7031e9d7a2de138110ca1a6d6a0a07fbce4e58195`).
  Governed rotation/old-epoch drain evidence:
  `reports/open-source-p1-fastpay-committee-rotation-20260717T033036Z/ACCEPTANCE.json`
  (SHA-256
  `93f7b7bbabfc62119891a3b04b060678ff133b31cc2968ee37a2a2fbef965ff5`).
  Persistence-boundary crash and complete adversarial-matrix evidence:
  `reports/open-source-p1-fastpay-crash-matrix-20260717T033709Z/ACCEPTANCE.json`
  (SHA-256
  `0602315444ae44fa0516be3b65ad9be978766b17f2207127248971a8bfc671e8`).
- [x] Preserve the established warm FastPay latency envelope in real six-node tests.
  The WAN Python client is no longer a legacy-v2 blocker: funding uses the
  signed consensus deposit, while send and unwrap bind the fresh governed v3
  recovery capability and require a cryptographically authenticated durable-
  apply quorum. Local migration evidence:
  `reports/open-source-p1-fastpay-python-v3-wan-client-20260717T040732Z/ACCEPTANCE.json`
  (SHA-256
  `2ee2a07b9c2c2a65d58ec169827440e4ed545932a09b8eef218f676acd8ddb80`).
  The safety-correct WAN candidate at `1e9352c6` completed five of five hot
  owned transfers with five distinct signed durable apply acknowledgements per
  payment: p50 2,489.978 ms and p95 3,724.984 ms. The earlier approximately
  1.1-second result returned after only one acknowledgement and is explicitly
  excluded. After the accepted battery, all six validators held the identical
  height-33 tip/root, empty mempools and the same 22 one-atom destination
  objects. Evidence:
  `reports/open-source-p1-fastpay-wan-20260717T-quorum-ack-1e9352c6-five-payment/`.
- [x] Make a completed FastPay response replayable after the client loses the
  terminal response. Commit `77e4a3c7` upgrades the existing outbox in place to
  retain a versioned completed record bounded by 1,024 entries, seven days and
  16 MiB. It binds the exact method/certificate operation digest, terminal
  digest and signed apply acknowledgements; persists the terminal result before
  returning it; replays it without another validator call; migrates v1 pending
  records; and fails closed on conflicting, malformed, oversized or tampered
  state. Unit coverage proves pre-terminal crash refusal, restart, count/TTL
  compaction and conflict/tamper rejection. The real six-validator proxy test
  proves exact replay after exact-six replication with zero additional apply
  attempts. The complete clean-checkout proxy suite passes `24/24` after
  `90c3836a` removed its legacy dependency on a separately running validator.

## 5. Batching and network-preservation guideline

### 5.1 Reset authority and budget

- The founder explicitly authorizes shared-devnet resets for this remediation
  program, reconfirmed 2026-07-16. No additional approval is required before a
  justified reset.
- Shared-devnet resets are an accepted engineering tool when they materially
  shorten delivery time, remove incompatible experimental state, or reduce the
  risk of a complex in-place transition.
- Minimize reset count by batching compatible protocol changes into one candidate
  and one planned reset wherever practical. There is no artificial hard cap, but
  every reset must have a recorded reason and must advance a named checklist gate.
- Before each reset, capture the current tip/root, release identity, validator
  roster, state/config manifests, relevant balances/supply totals, and a restorable
  snapshot or deterministic genesis/bootstrap input. Hash and archive the record.
- A reset must use a deterministic, reviewed genesis/bootstrap manifest and must
  end with all six validators converged, supply invariants proven, mempools clean,
  and customer test wallets either reproducibly recreated or explicitly retired.
- Do not reflexively reset merely to erase a red result. Preserve and diagnose the
  failure first; then reset if the evidence shows reset/rebootstrap is the fastest
  safe route forward.
- Devnet reset authority does not remove the production requirement to prove
  versioned migration, rolling upgrade, replay, snapshot recovery, and rollback in
  isolated six-node environments.
- Local ephemeral clusters may be recreated freely for destructive fault injection
  and repeated activation rehearsals.

### 5.2 Build many separable commits, deploy one compatible validator binary

To minimize restarts without hiding regressions:

1. Implement each P0 as separable commits with targeted tests.
2. Merge the completed commits into one candidate binary containing versioned,
   inactive protocol upgrades for consensus, governance, FastPay, bridge types,
   and state-root commitments.
3. Replay the current devnet snapshot offline and in an isolated six-node clone.
4. Rehearse all activations repeatedly on ephemeral clones, including abort and rollback.
5. After the isolated rolling-upgrade drill passes, choose the fastest safe shared-
   devnet route: rolling-deploy the candidate once, or perform one planned clean
   reset directly onto the candidate and reviewed bootstrap manifest.
6. For a rolling deploy, wait after each node for rejoin, matching tip/root, empty
   unexpected mempool, healthy receipt reads, and stable observation. For a reset,
   verify the archived pre-reset manifest and complete 6/6 deterministic bootstrap
   before accepting transactions.
7. Do not activate a new protocol while the fleet contains mixed binaries.
8. After 6/6 binary convergence, activate features in dependency order at distinct,
   predeclared heights without restarting validators:
   - state-root version and migration;
   - consensus v2;
   - signed governance/registry rotation;
   - FastPay v2 cancellation/object versioning;
   - bridge/checkpoint/settlement protocol.
9. Require a stable observation window and exact replay evidence after each activation
   before scheduling the next. Activation failure uses the tested pre-activation
   rollback path or, on the shared devnet, an evidence-preserving clean reset when
   that is the faster safe recovery.

### 5.3 Separate deployments that do not require validator resets

- Wallet-web and wallet-proxy fixes deploy independently after browser/proxy gates;
  they do not restart validators.
- Ethereum contracts deploy independently after Foundry and fork gates; their governed
  route remains inactive until code hashes and verifier state are committed.
- Publication-history sanitation never touches the running fleet.
- Provider credential revocation never touches consensus or chain state.

### 5.4 Test batching for speed without sacrificing coverage

For every commit:

- run the smallest real-boundary regression first;
- run its owning crate/package suite;
- run formatting, workspace check, and strict Clippy when Rust changes;
- run wallet/proxy or Foundry gates when those surfaces change.

At the end of each protocol batch:

- run model/property/fault suites and affected cross-crate integration tests;
- run replay/migration against the frozen devnet snapshot;
- build one candidate artifact and compare its hash across two clean builds.

Run the expensive complete workspace/Orchard/replay/Foundry/browser battery only
after a batch is internally green and again on the final immutable candidate.
This avoids repeatedly paying multi-hour proof costs for changes that have not
passed targeted boundaries, while retaining complete final coverage.

### 5.5 Proposed batches

**Batch A — offline safety core, no fleet mutation**

- consensus model/types/durable state/view change;
- signed governance and registry rotation;
- state-root activation/migration;
- FastPay cancellation model and implementation;
- native/issued supply inventories;
- all local simulations, replay, and crash tests.

**Batch B — customer surfaces, no validator reset**

- browser custody and report redaction closure;
- proxy authentication and wallet serving;
- signed FastPay deposit/funding UX;
- current privacy-v2-only flow and leakage captures.

**Batch C — external settlement, no shared-chain reset**

- Ethereum checkpoint/inclusion verification;
- PFTL settlement verifier contract;
- consume/refund state machine;
- governed bridge route discovery and wallet integration;
- Foundry, fork, replay, and conservation tests.

**Batch D — one shared-devnet rollout/reset and sequential activations**

- one rolling binary deployment or one planned clean reset onto the candidate;
- sequential versioned activations with observation gates;
- end-to-end customer flow and fault drills;
- additional resets are permitted when evidence shows they are the fastest safe
  recovery, but each is archived and tied to a failed/advanced gate.

**Batch E — immutable public candidate**

- provider revocation evidence;
- sanitized exact-ref history;
- complete integrated battery;
- reproducible artifacts/SBOM;
- zero-open-P0 closure table.

## 6. Immediate execution queue

- [x] Correct the audit classification: containment is not `FIXED` for consensus,
  governance, bridge, wallet bridge, settlement verifier, or FastPay.
- [x] Restore FastPay default availability with a failing-then-green real RPC regression.
- [x] Freeze the current devnet snapshot and generate the offline replay fixture
  before the first authorized shared-devnet mutation or reset.
- [x] Begin `P0-CONSENSUS-01` with the canonical artifact types and executable safety model.
- [x] Implement durable consensus safety state and view-change validation.
- [x] Run the first `n=4` and `n=6` quorum-intersection, failed-proposer,
  typed-QC, lock, and restart model batteries.
- [x] Continue directly through signed governance, FastPay cancellation, and bridge
  implementation according to the batches above.
