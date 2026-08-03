# A666 Public Reserve Product: Eight-Hour Emergency Execution Spec

- **Date:** 2026-08-03
- **Status:** EMERGENCY EXECUTION
- **Target:** Complete the valid A666 public-reserve product within eight hours
**Authoritative parent plan:**
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md`

## 1. Executive requirement

This is not a demo-hardening plan and it does not authorize a legacy or
operator-trusted substitute.

The required product is the existing A666 NAVCoin, migrated in place from a
StakeHub-dependent reserve-publication lineage to the public PostFiat reserve
proof system. A clean public checkout must be able to reproduce the reserve
proof, PFTL must enforce the proof and resulting NAV, and a user must be able
to complete the real product lifecycle:

```text
Ethereum USDC
  -> pfUSDC on PFTL
  -> A666 issuance at verified NAV
  -> private or transparent PFTL ownership
  -> proof-bound wA666 export to Ethereum
  -> wA666 return to native A666
  -> A666 redemption to pfUSDC
```

The accepted terminal state is **complete and operating**. There is no
fallback demo, no attestation substitute, no new replacement NAVCoin, and no
permission to label partial work as shipped. A failed execution step enters
the repair-and-retry loop in section 13 and execution continues until the
acceptance contract is satisfied.

## 2. Product acceptance contract

The product is complete only when every item below is evidenced.

- [ ] The existing A666 asset ID remains
  `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c`.
- [ ] Existing A666 supply, balances, export entitlements, receipts, and
  Ethereum wA666 identity remain continuous through migration.
- [ ] Aave, EVM spot, Hyperliquid, staked NEAR, staked Solana, and XMR reserve
  quantities are verified by public source-specific code.
- [ ] All six valuation claims are cryptographic and policy-bound.
- [ ] The proof contains zero attested-value and zero controlled-value source
  claims.
- [ ] The finalized pfUSDC reserve overlay is included without double counting.
- [ ] A fresh aggregate proof verifies against the pinned successor ELF and
  verification key.
- [ ] PFTL validators deterministically verify the public profile, proof,
  reserve packet, overlay, issued supply, precision, and conservative NAV.
- [ ] The live A666 profile and route are governed to the public successor.
- [ ] Transparent issue and redeem pass against finalized live state.
- [ ] Private issue and redeem pass against finalized live state.
- [ ] Ethereum export and return pass against finalized live state.
- [ ] Replay, stale-proof, wrong-profile, wrong-overlay, wrong-supply,
  wrong-NAV, wrong-packet, and duplicate-operation attempts fail closed.
- [ ] All six validators converge after restart on identical height, tip, and
  state root.
- [ ] Reserve, supply, wallet-balance, vault, and bridge conservation checks
  pass.
- [ ] The browser wallet can perform and recover the complete lifecycle
  without an internal StakeHub API, path, token, or process.
- [ ] A clean public checkout reproduces the complete proof and lifecycle
  evidence.
- [ ] Only after all prior items pass, machine-readable readiness records all
  six sources as production-qualified and sets `stakehub_deprecated=true`.

## 3. Meaning of StakeHub deprecation

StakeHub may remain an operator-side custody and unlock tool. It may use local
private keys to sign exact, domain-separated ownership or source statements
requested by the public proof workflow.

StakeHub must not:

- define balances, liabilities, positions, prices, haircuts, or NAV;
- choose which assets or liabilities are omitted;
- emit an aggregate reserve attestation accepted as proof;
- decide whether a source is fresh or final;
- substitute a controlled or attested value for a cryptographic source;
- be required by the wallet, proxy, validator, proof verifier, packet builder,
  route, or clean-checkout reproduction.

All semantic meaning must be implemented and auditable in the public tree.
Every exact signature supplied by a custody tool must be independently
verified by public code and bound to the source, owner, checkpoint, profile,
epoch, policy, and challenge.

## 4. Fixed public successor identity

The execution must use the already-pinned successor identity. It must not
silently rebuild a different proving identity.

```text
successor profile:
f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91

source manifest:
8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb567268ca5942669ff6977ef32dd3a41

valuation policy:
350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c

source commit:
5b8f0317375af6fb46d586d9d9152b511457b802

guest ELF SHA-256:
2b41e4e8095b1dacdc519b2f0a2b4831ebc57cc8003a4d3686f6d9e4687e81df

SP1 verification key:
0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf
```

Any mismatch stops that operation, preserves live state, repairs the source of
the mismatch, and repeats verification. It does not create a new profile by
accident.

## 5. Current baseline

At the start of this emergency execution:

- `G0`, `G1`, and `G2` pass;
- `G3` through `G7` remain open;
- all six public adapters exist but are recorded as partial and
  `production_qualified=false`;
- two fresh six-source successor epochs exist with cryptographic quantity and
  valuation claims and zero attested or controlled value;
- epoch 7 proves net assets of `2,835,791,218,669` USD-e8 atoms;
- epoch 8 proves net assets of `2,839,694,289,118` USD-e8 atoms;
- retained ten-target fuzz qualification completed with 326,405,841
  executions and no crash, timeout, or OOM;
- the exact height-776 archive replay passes with the historical tip and state
  root;
- the current pushed tip is `dafeb27c5a800fbe876664ec51a410bafe852d4c`;
- CI runs `30778161599` and `30778196570` are green across all jobs;
- the epoch-7 bounded aggregate proof retry is active;
- no successor release has been applied to a validator;
- rollout state records `applied: []`;
- all six live validator and RPC services are active;
- A666 remains governed by the legacy profile;
- the local browser wallet serves successfully but its configured RPC fleet
  uses ports `38650` through `38655`, while the current live tunnels use
  `39650` through `39655`.

This baseline must be captured again at execution start because proofs,
services, branch tips, heights, and freshness windows can change.

## 6. Non-negotiable safety and truth constraints

- No live A666 mutation before the two aggregate proofs independently verify
  and the exact six-validator controlled rehearsal passes.
- No manual NAV value or manually selected reserve packet hash.
- No aggregate operator attestation.
- No relabeling historical attested Solana or XMR material as cryptographic.
- No direct-issued pfUSDC fixture as evidence for the live lifecycle.
- No new A666-like asset; migrate the existing A666 in place.
- No deletion or bulk cleanup of the dirty worktree.
- Never use `git add .`; stage only named, reviewed files.
- Never print, copy into evidence, or commit private keys, seeds, API tokens, or
  wallet backups.
- Never deploy the obsolete staged `e949f48` release.
- Use only the signed safe-rollout workflow with a verified preflight and
  signed height/state-root recovery snapshot.
- Keep the live route paused during profile, packet, and policy mutation.
- After any validator divergence, stop mutation, preserve evidence, restore
  convergence, and resume from the last verified state.
- Do not stop the legacy StakeHub publication services until the public live
  lifecycle and clean reproduction both pass.

## 7. Eight-hour critical path

The lanes below are concurrent only where they do not contend for the same
memory, validator state, files, or signing authority. The aggregate proofs
must not run concurrently on the current 122 GiB host because a single proof
has approached 60 GiB RSS. Epoch 8 may run concurrently only on a physically
separate, qualified prover with the exact pinned ELF and witness.

### T+00:00–00:20 — freeze, fingerprint, and resource decision

- [ ] Disable all automated prompt injection for this task.
- [ ] Record branch, exact tip, tracked diff, untracked boundaries, CI results,
  proof-service state, and current proof output inventory.
- [ ] Verify the six validators and RPC processes are active.
- [ ] Read height, tip, state root, profile, route, supply, vault, overlay,
  reservations, entitlements, and mempool independently from all six nodes.
- [ ] Confirm that no validator received the obsolete staged release.
- [ ] Confirm the signed height-776 recovery snapshot and its state root.
- [ ] Decide immediately whether epoch 8 can be proved on a separate qualified
  host. If it can, copy only content-addressed public inputs and the pinned ELF,
  run the proof there, and verify returned artifacts locally. If it cannot,
  preserve the sequential local proof path and compress later operational
  steps without deleting acceptance gates.
- [ ] Write the execution clock and evidence root into the run manifest.

### T+00:20–01:15 — finalize exact-tip tooling while proofs run

- [ ] Review the current named tracked changes and the new profile-rotation
  test; discard accidental edits and retain only required migration hardening.
- [ ] Require route activation to consume canonical `nav_per_unit`, the exact
  A666 asset, the pinned packet hash, an explicit issuer-key path, and a paused
  route.
- [ ] Require profile rotation to derive and independently match the pinned
  successor profile and reject controlled sources.
- [ ] Run focused Python tests, formatting, `git diff --check`, proof-input
  inventory, crypto-callsite policy, and readiness gates.
- [ ] Commit only named files and push the exact tip.
- [ ] Start CI for that exact tip; all jobs must pass before release staging.

### T+00:00–03:30 — aggregate proof completion and verification

- [ ] Preserve the active epoch-7 proof job; do not duplicate it locally.
- [ ] On completion, require `proof.bin`, `proof-calldata.bin`,
  `public-values.bin`, and `proof-report.json`.
- [ ] Independently verify epoch 7 with the pinned ELF and verification key.
- [ ] Byte-compare epoch-7 public values to the committed 584-byte pin and
  require SHA-256
  `a215726624267dc5c5a60ac2829b24a149855a3edcfc798c965826e17bca7e68`.
- [ ] Hash and retain all four proof artifacts and the supervised service
  result.
- [ ] Complete epoch 8 on the isolated prover or sequential bounded local
  prover.
- [ ] Independently verify epoch 8 with the same pinned identity.
- [ ] Byte-compare epoch-8 public values to the committed pin and require
  SHA-256
  `1bc443108e0f2b78d92037d986378cd6df51bd3fc069e64594a521f83a36b9dd`.
- [ ] Hash and retain all four epoch-8 proof artifacts.
- [ ] Confirm both proofs contain six cryptographic quantities, six
  cryptographic valuations, zero attested value, and zero controlled value.

### T+03:30–04:30 — exact controlled six-validator lifecycle

- [ ] Run the exact ignored release-mode test
  `a666_public_successor_proof_migrates_and_survives_six_validator_restart`.
- [ ] Use the real pfUSDC vault propose/attest/finalize/claim state machine.
- [ ] Register and rebind the exact public successor to the same A666 asset.
- [ ] Finalize epoch 7, perform issuance, derive the nonzero pfUSDC overlay,
  and finalize epoch 8 against the composite reserve root.
- [ ] Pass transparent issue and redeem.
- [ ] Pass private issue and redeem.
- [ ] Pass Ethereum export and return state transitions.
- [ ] Pass partial-validator outage and recovery.
- [ ] Pass restart and signed snapshot restoration.
- [ ] Reject replay, stale proof, wrong profile, wrong source root, wrong
  valuation root, wrong overlay, wrong supply, wrong NAV, and wrong packet.
- [ ] Require identical state roots across all six controlled validators.
- [ ] Require reserve, supply, wallet, vault, and bridge conservation.
- [ ] Record G3, G4, and G5 evidence source by source; do not mark a source
  qualified merely because the aggregate proof verifies.

### T+04:30–05:15 — exact release build, replay, and recovery package

- [ ] Require exact-tip CI success.
- [ ] Build the release from a clean checkout with locked dependencies.
- [ ] Record source revision, build inputs, binary SHA-256, proof identity,
  topology, and config hashes in the signed deployment manifest.
- [ ] Replay the full copied signed height-776 archive with the exact release
  binary.
- [ ] Require 776 blocks, tip
  `3be5a881e124e71c6b2704fffbeb95b874205b8bd85d26ad29b60307596727f967aebc2bfd496e7d87b0835bdb6766c9`,
  and state root
  `10dfb17b640a69749ca4b00d66b9e0141fa33644df6bcd8f7008f85f4501a42424681edaf25a3fd0111cc55492b256f9`.
- [ ] Create a new release ID; never reuse `a666-public-reserve-e949f48`.
- [ ] Run fresh fleet preflight against all six validators.
- [ ] Capture and independently verify a new signed finalized-checkpoint
  backup immediately before rollout.
- [ ] Rehearse the exact rollback command and verify its target hashes without
  applying it.

### T+05:15–06:15 — rolling validator release

- [ ] Apply the release to one validator at a time through
  `scripts/postfiat-safe-rollout`.
- [ ] After each validator, require service readiness, correct binary hash,
  committee identity, archive replay/restart success, and convergence with the
  unchanged validators.
- [ ] Never allow more than one validator to be unavailable at once.
- [ ] After validator 6, require all six nodes to report the same height, tip,
  state root, profile, route, supply, vault, overlay, reservations,
  entitlements, and empty mempool.

### T+06:15–06:50 — governed live A666 migration

- [ ] Recollect fresh source checkpoints if the proof freshness window no
  longer permits the prepared proof; rebuild the public proof rather than
  weakening freshness.
- [ ] Advance PFTL proof height only with the final release binary and the
  approved value-carrying height-advance procedure.
- [ ] Pause the A666 route and verify zero active reservations and zero unsafe
  export entitlements.
- [ ] Register the exact successor profile.
- [ ] Rebind the existing A666 asset to that profile.
- [ ] Build the overlay-aware NAV packet from verified public values, live
  issued supply, asset precision, and finalized pfUSDC overlay.
- [ ] Submit and finalize the packet.
- [ ] Advance the route epoch and policy to the finalized profile, proof epoch,
  packet hash, and conservative NAV.
- [ ] Verify every mutation independently on all six validators before the
  next mutation.

### T+06:50–07:35 — live product lifecycle

- [ ] Perform a small transparent pfUSDC-to-A666 issuance.
- [ ] Verify that A666 supply increases by the issued amount and the pfUSDC
  reserve/overlay accounting changes exactly once.
- [ ] Perform a transparent A666-to-pfUSDC redemption and verify retirement and
  reserve release.
- [ ] Perform a private pfUSDC-to-A666 issuance.
- [ ] Perform a private A666-to-pfUSDC redemption.
- [ ] Export A666 to wA666 on Ethereum and verify the intended MetaMask
  recipient balance and PFTL entitlement state.
- [ ] Return wA666 from Ethereum, restore native A666 on PFTL, and reject burn
  replay.
- [ ] Repeat final reserve, supply, wallet, vault, bridge, nullifier, and
  receipt conservation checks.
- [ ] Unpause the route only after all lifecycle checks pass.

### T+07:35–08:00 — wallet, public reproduction, and final truth update

- [ ] Repair the wallet RPC fleet mismatch so its configured endpoints and
  persistent tunnel endpoints are identical and restart-safe.
- [ ] Verify the browser wallet loads, reads finalized balances, discovers the
  governed route, submits signed operations, displays durable bridge/export
  status, survives navigation, and recovers pending jobs.
- [ ] Run one browser-driven transparent lifecycle and one browser-driven
  private lifecycle against the migrated route.
- [ ] From a clean public checkout, reproduce the successor identity, both
  public-values blobs, proof verification, packet derivation, and final live
  state without any StakeHub code or filesystem path.
- [ ] Record exact artifact hashes, transaction IDs, Ethereum transaction
  hashes, heights, tips, state roots, balances, supply deltas, and screenshots
  in the evidence manifest.
- [ ] Update the six source qualification records only from collected evidence.
- [ ] Set `stakehub_deprecated=true` only after every acceptance item passes.
- [ ] Stop obsolete StakeHub reserve-publication services only after the public
  replacement remains healthy and the recovery package is verified.
- [ ] Commit and push the final evidence/status update and require final CI.

## 8. Source-by-source qualification matrix

Every row must be complete for two distinct fresh epochs.

| Source | Public quantity proof | Public valuation proof | Ownership / completeness | Finality | Liabilities | Two-epoch reconciliation | Adversarial/fuzz | Production qualified |
|---|---|---|---|---|---|---|---|---|
| Aave on Arbitrum | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| EVM spot | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| Hyperliquid | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| Staked NEAR | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| Staked Solana | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| XMR reserve | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |
| Finalized pfUSDC overlay | [ ] | N/A | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] |

For each source, evidence must name the deployed public reader or proof
identity, governed owner and position policy, exact checkpoint, finality
basis, quantity result, valuation input and result, liability result, bounds,
failure vectors, proof commitment, and independent clean-checkout
reproduction.

## 9. Live mutation sequence and invariants

The only permitted live sequence is:

```text
fresh all-six fleet read
  -> signed recovery snapshot
  -> rolling exact release
  -> all-six convergence
  -> route pause
  -> successor profile registration
  -> existing A666 profile rebind
  -> overlay-aware proof packet submit/finalize
  -> route epoch/policy advance
  -> all-six convergence
  -> transparent lifecycle
  -> private lifecycle
  -> Ethereum export/return
  -> conservation and replay checks
  -> route unpause
  -> clean public reproduction
  -> StakeHub deprecation flag
```

Required invariants after every arrow:

- all finalized validators agree on the same deterministic state;
- A666 asset identity never changes;
- balances and issued supply do not change except through an accepted,
  evidenced state transition;
- reserve value is neither omitted nor double counted;
- profile, proof, packet, route, and NAV identities remain mutually bound;
- no replayable operation or orphaned reservation/entitlement remains;
- recovery remains possible from the last signed finalized checkpoint.

## 10. Wallet product requirements

The wallet is part of the acceptance contract, not an optional presentation
layer.

- It must use the governed Ethereum-mainnet USDC-to-pfUSDC route; Arbitrum is
  not offered for new deposits.
- It must discover NAVCoins and routes generically; A666-specific presentation
  may exist as metadata but not as protocol logic.
- It must show deposit, relay, issuance, private transfer, export, return, and
  redemption as durable recoverable jobs.
- Navigating away and returning must recover current status.
- “From PFTL” and “From MetaMask” redemption sources must reflect actual
  balances and available return operations.
- It must never show a successful action until finalized source and
  destination state support that claim.
- It must display actionable errors for RPC reachability, authorization,
  finality, proof generation, relay, insufficient balance, stale NAV, paused
  route, and recovery.
- It must not expose an internal StakeHub concept to the user.
- Its persistent RPC tunnel configuration must match the endpoints consumed by
  the proxy after machine or service restart.

## 11. Required evidence bundle

The final evidence root must contain:

- execution manifest with exact start/end timestamps and source revisions;
- clean and working-tree status inventories;
- CI run URLs and conclusions for the release and final evidence tips;
- successor identity and reproducible-build hashes;
- epoch-7 and epoch-8 witness, public-value, proof, calldata, report, and hash
  records;
- per-source qualification records for both epochs;
- controlled six-validator lifecycle report;
- signed pre-rollout snapshot and independent verification report;
- release manifest, public deployment manifest, topology, binary hash, and
  per-validator rollout records;
- before/after all-six profile, route, packet, supply, reserve, overlay,
  balance, mempool, height, tip, and state-root snapshots;
- transparent/private issue and redeem receipts;
- Ethereum export/return transaction and finality evidence;
- negative/replay test results;
- conservation report;
- browser lifecycle report and screenshots;
- clean public reproduction report;
- final machine-readable readiness record.

Secrets are never part of the evidence bundle.

## 12. Completion gates

| Gate | Requirement | Complete |
|---|---|---|
| G0 | Wallet/runtime has no StakeHub dependency | [x] |
| G1 | Public bounded proof standard and immutable identities | [x] |
| G2 | Generic public proof framework reproduces cleanly | [x] |
| G3 | All six A666 source families production-qualified | [ ] |
| G4 | Two fresh source-equivalent public aggregate proofs verify | [ ] |
| G5 | Exact controlled six-validator lifecycle and rollback pass | [ ] |
| G6 | Existing live A666 migrates and passes the full live lifecycle | [ ] |
| G7 | Clean public checkout reproduces the complete final product | [ ] |
| UX | Browser wallet completes and recovers the full lifecycle | [ ] |

The product is complete only when every row is checked.

## 13. Repair-and-retry loop

There is no legacy-demo fallback and no permission to waive a failed gate.
When a step fails:

1. Stop only the affected operation; preserve logs and exact inputs.
2. If live state was touched, keep the route paused and verify fleet
   convergence. Roll back the affected release or governed mutation to the
   last signed finalized checkpoint when convergence or invariants cannot be
   restored in place.
3. Classify the failure as proof identity, source evidence, freshness,
   deterministic execution, archive compatibility, validator rollout,
   governance ordering, bridge/finality, wallet connectivity, or evidence
   integrity.
4. Apply the narrowest deterministic fix and add a regression test that fails
   on the observed defect.
5. Rebuild from the exact source tip and repeat every invalidated downstream
   gate. Never reuse an artifact whose inputs or verifier changed.
6. Continue execution until the product acceptance contract passes.

At the eight-hour mark, the status is either **complete** or **still
executing toward the same acceptance contract**. The timebox does not convert
an incomplete state into a shipped state and does not terminate the repair
loop.

## 14. Final completion statement

The only valid completion statement is:

> The existing A666 NAVCoin now uses the public, auditable PostFiat reserve
> proof system for Aave, EVM spot, Hyperliquid, staked NEAR, staked Solana,
> XMR, and the finalized pfUSDC overlay. The same A666 asset and Ethereum wA666
> representation pass transparent and private issuance/redemption plus
> export/return against verified NAV. All six validators converge, public
> reproduction passes, the browser lifecycle passes, and StakeHub is not a
> proof authority or runtime dependency.

That statement must not be issued until every gate in section 12 is complete.
