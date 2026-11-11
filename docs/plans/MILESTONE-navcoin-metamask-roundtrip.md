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
