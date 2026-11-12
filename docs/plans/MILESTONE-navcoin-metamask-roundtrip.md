# Milestone: NAVCoin MetaMask round trip — Ethereum half

Task: `task_2e3d8ac70136562a3efaf961c2bbc567` (accepted 2026-11-11)
Locked spec: `post-fiat-emergency-execution-plan/execution/SPEC-navcoin-metamask-roundtrip.md`
(TIH 86.00, locked per `AGENTS.md`; not to be rewritten or re-scored)

Deadline: Friday 13 Nov 2026.

**Standing constraint: wA666 and wBMNRC stay paused for the whole of this work.**
Nothing here un-pauses them and nothing depends on them.

## Defect map

| # | Defect | Code | Status |
|---|---|---|---|
| D1 | Committee taken from the prover's own input, no ancestry | `crates/pfusdc_proofs/src/lib.rs:293-326` @ `7a91bb30`; verifier only checks nonzero at `PFTLReceiptFinalityVerifierV1.sol:302-303` | **closed in contract**: `PFTLReceiptFinalityVerifierV2.sol` derives the committee root only from the proven segment |
| D2 | Any resulting checkpoint recorded as accepted even when it does not advance the tip | `PFTLReceiptFinalityVerifierV1.sol:320-328` (`_recordCheckpoint`) combined with the lenient prior check at `:182` | **closed in contract**: `_advanceTipIfStrictlyGreater` records canonical only on advance |
| D3 | Export multipliers hard-rejected unless exactly 10050/9995, blocking the 20 bps checkout | `crates/pftl_uniswap_proofs/src/lib.rs:271`; constants `crates/types/src/market_nav_asset_types.rs:2325-2326` | open; decision taken: **parameterise** |

## Tasks

### 1. Ancestry-enforcing verifier (D1)

- [x] `PFTLReceiptFinalityVerifierV2.sol`: pin vkey and genesis ancestry root immutably
- [x] verify the SP1 proof before reading any committed output
- [x] derive the committee root only from the proven ancestry segment
- [x] reject a caller-supplied committee root not identical to the proven root
- [x] reject zero/absent committee commitment
- [x] no owner path over vkey, trust anchor or ancestry rules
- [x] tests: made-up committee rejected; root supplied outside the proven segment rejected;
      proof rooted anywhere but the pinned genesis rejected; genuine-prior + made-up
      checkpoint rejected for want of ancestry

### 2. Checkpoint ordering, kept separate from ancestry (D2)

- [x] explicit stored pinned tip; tip-advance requires a checkpoint strictly above it
- [x] reject at-or-below for tip advance; do not conflate ordering with ancestry or replay
- [x] tests: above-with-valid-ancestry advances; equal cannot; below cannot;
      higher-with-invalid-ancestry rejected; a rejection leaves the stored tip unchanged

### 3. Receipt replay prevention, kept separate again (D1/D2)

- [x] canonical route-scoped receipt identifier, independent of the checkpoint proved against
- [x] identifier binds route, destination chain, controller, wrapped token, recipient, amount
- [ ] consume-check, verify, mark-consumed atomically with mint authorization; revert whole op
      if minting fails
- [ ] tests: immediate resubmission rejected; same receipt re-proved against a later valid
      checkpoint rejected; changed recipient/amount/token/controller/route/chain invalidates the
      proof; reverted mint leaves no inconsistent state; planted accepted-receipt replay rejected

### 4. Guest program build and fee parameterisation (D3)

- [ ] parameterise the multipliers, bound into the route config digest the proof already binds
- [ ] build the ancestry-proving guest program reproducibly, record the vkey
- [ ] pin that vkey in V2
- [ ] test: a 20 bps route is accepted; a route whose multipliers disagree with the committed
      digest is rejected

### 5. Proof through a CLI and an interface (mandate)

- [ ] Python CLI a human can run end to end against local/fork infrastructure
- [ ] existing user-facing interface shows the result
- [ ] both wrapped tokens verified still paused in the evidence

### 6. Close out (mandate)

- [ ] update existing `postfiatl1v2` docs; refresh what is obsolete, no bloat
- [ ] retire this milestone doc into completed plans
- [ ] evidence: diffs, focused test output, CLI output, interface screenshot, paused-state proof

## Not claimed by this milestone

The three economic release gates are **not** satisfied and this work does not satisfy them:
the four unreported Schwab liability categories, withdrawal-authority restriction, and
redemption beyond the posted reserve. No end-to-end mainnet withdrawal is claimed.

## Journal

- 2026-11-11 — Task accepted. Defect map above verified line by line against the worktree:
  `pftl_uniswap_proofs/src/lib.rs:271` is the multiplier rejection,
  `market_nav_asset_types.rs:2325` is `10_050`, `pfusdc_proofs/src/lib.rs:47` is
  `&witness.finality_ancestry`, `PFTLReceiptFinalityVerifierV1.sol:302-303` is the
  nonzero-only committee check. Multiplier decision: parameterise.

- 2026-11-11 — `crates/ethereum-contracts/src/PFTLReceiptFinalityVerifierV2.sol` written and
  compiling; `test/PFTLReceiptFinalityVerifierV2.t.sol` passes **14/14** under
  `forge test --match-contract PFTLReceiptFinalityVerifierV2Test`.

  The three concerns are enforced separately and tested separately:
  - ancestry: `_requireAncestry` rejects a wrong root, an absent segment, and any
    caller-supplied committee root that is not identical to the proven one. A nonzero
    committee root is no longer sufficient -- that was the whole of V1's check.
  - ordering: `_advanceTipIfStrictlyGreater` writes `canonicalCheckpointHeight` only when a
    checkpoint advances the tip, so a below-tip claim can never become a usable `prior`. V1
    recorded every resulting checkpoint unconditionally.
  - replay: a route-scoped `receiptIdentifier`, independent of the checkpoint, is consumed
    once. The same export re-proved against a later valid checkpoint with a fresh nullifier is
    still rejected.

  Named P0-01 forgeries covered: `testRejectsMadeUpCommittee`,
  `testRejectsGenuinePriorWithMadeUpCheckpointBelowTip`,
  `testRejectsSameReceiptReProvedAgainstLaterCheckpoint`. Ordering-is-not-ancestry is covered
  by `testRejectsHigherCheckpointWithInvalidAncestry`.

  Still open: the guest program build and vkey (step 4), the multiplier parameterisation in
  `crates/pftl_uniswap_proofs`, the CLI and interface (step 5), and close-out (step 6). The
  contract currently requires a nonzero `multiplierPolicyCommitment` so the fee policy is bound
  into the proof, but the guest side that computes it is not yet written.

- 2026-11-12 (early hours) — R2-1 fix DEPLOYED to all six validators.
  Release `r2-1-beacon-anchor-20261112`, binary sha256 `973734620ee6a15b...`,
  signed with the trusted publisher `pfc531e00ffefecb8fabe40c6f47c69148df31e49c`.
  Rolled canary-first (validator-1), one node at a time, convergence verified
  between each. No divergence at any point; height 1194, identical state roots.
  A post-deploy transaction moved 1 pfUSDC -> 1 B23 with reserve, supply and
  invariant all correct, so the chain writes correctly under the new code.

  Corrections recorded:
  - **The live vault was never paused.** Earlier status reports (including mine)
    cited `0x8583409d...`. The route the chain actually uses is
    `ethereum-mainnet-usdc-v1`, vault `0xe7a432a28b20621a70e845c7f202004cf98e7a2c`,
    `paused() = false`, holding 76.062020 USDC. Its verifier_kind is
    `sp1-groth16`, which is the branch R2-1 lives in
    (`nav_vault_asset_execution.rs:7502`), so that balance was exposed until this
    deploy landed.
  - **validator-5's RPC was left on the pre-fix binary** by an earlier hand-written
    restart script of mine (`/usr/local/bin/rpc5-start.sh`, now removed). Its
    systemd unit was in `failed` because the stray process held port 27655. Since
    rpc-serve performs mempool admission, that node could have admitted deposits
    under pre-fix rules. Found via `build_git_revision: ae70d66d` and fixed; all
    six now report the new release.

  Standing exposure after the deploy, for the record: fake pfUSDC $0 (lane
  fail-closed, no anchor pinned); vault drain bounded at 76.06 USDC and only by
  burning genuine pfUSDC; wrapped-token forgery $0 while both pauses hold;
  stale-NAV buying up to ~$140 of A666 principal plus held pfUSDC; private pool
  $0 (`admission_switch: off`, survived the restarts, re-arms on host reboot).

- [ ] **Pin the first beacon anchor — STAGED, NOT SUBMITTED.**
  Anchor built from live finality and the live route: slot 15634944, root
  `e54a9a9ccfd16c722b1df16e0eee1b3890adc298b57a01469dc7d99cd4c22844`, route epoch
  11, bound to the real vault and route_binding
  `35ec9274bdad13f7c53edac615b160ca668a10cc22d7dcef1f6f79d20a3dd8c4`.
  Files on validator-0: `/tmp/beacon-anchor.json`, `/tmp/gov-batch.json`,
  `/tmp/gov-amendment.json`. All four values verified present in the signed batch.
  Blocked on: the activation requires one `SignedGovernanceAuthorizationV2` per
  support vote (`consensus_artifacts.rs:2858`), i.e. a signature ceremony across
  the validators via `vault-bridge-route-profile-governance-assemble`
  (`--signed-amendment-file`, `--proposal-slot`). Not attempted further at 01:00;
  this is a consensus governance action and should be done with fresh attention,
  not by trial and error.
  **Until it is pinned the ingress admits nothing, so USDC -> pfUSDC remains
  unavailable. That is the fail-closed posture, not a regression.**
