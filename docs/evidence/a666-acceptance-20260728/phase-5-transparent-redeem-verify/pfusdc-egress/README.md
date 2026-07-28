# Phase 5 pfUSDC egress incident and fail-closed recovery

## Result

The transparent A666 primary redemption completed on PFTL, producing
`999500` pfUSDC atoms for Joe. The subsequent pfUSDC burn finalized at PFTL
height `384`, but the Ethereum withdrawal was stopped before proving or
broadcast because the deployed verifier's immutable SP1 guest cannot verify
blocks carrying the newer PFTL-Uniswap receipt-root consensus commitment.

No proof bypass, operator inventory, validator fork, receipt replacement, or
ledger edit was used.

## Finalized PFTL burn

- burn transaction:
  `fdc1c8cfc3963a9552eb671abbc537866efcceed4bbc76cce0e29ab826807e698498ac52a5cec36ba143d35587085f68`
- height: `384`
- amount: `999500`
- destination: `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`
- redemption:
  `1683e1d9ef472b55d9a822d507fa7eb4b197221982b05250ec5bf8e782d2a1d657904eafa98f723b5af3f2c89f18c032`
- state: pending and fully backed

## Root cause

The deployed `PFTLFinalityVerifierV1` pins egress program vkey
`0x007a22f1b8a47814a027ee0af8086a6f5f6ae4af0530dc7ffb2acac2da617834`.
That frozen program predates
`pftl_uniswap_receipt_root` in `ConsensusV2BlockRef`. The height-384 block
correctly uses the new consensus block-ID and signing encoding, so the old
guest cannot verify it. Both the verifier vkey and the legacy vault's verifier
reference are immutable in the deployed contracts.

The exact witness fails the old guest before proof generation. A rebuilt guest
using the current consensus implementation validates and executes the same
witness:

- current ELF SHA-256:
  `ea0d3ef37ade9e2413646c8051b58f8e8123516e75da0937a8d47d4d9586f2fe`
- current program vkey:
  `0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b`
- execution cycles: `744041727`
- canonical public values: `1486` bytes

## Fail-closed actions

The legacy Ethereum vault was paused at mainnet transaction
`9ffc64aec79481094ff2087f72467d041167553d8a45476688792749c3b5d340`,
block `25631940`. The vault's `104520000` USDC atoms remain in custody.

pfUSDC was halted on PFTL at height `385`:

- transaction:
  `3b9489c1415b4f7f57e3b6489205b5d72debc7c0109506ab432490d05b3a14d4cae3c08c6883139735ff164a4721a0e1`
- certificate:
  `dd47aebef1ac23b8ab9e7bebf409e242fc9460cd1e97b287db23f32cae822da29c7096485de1c27976c45e959ada8a93`
- votes: `5` of `6`

These controls prevent new deposits and new unprovable burns while the
replacement verifier/vault lane is deployed and accepted.

## Acceptance status

Phase 5 produced a valid defect and a fail-closed recovery state. It is not a
withdrawal PASS. Phase 6 must deploy and register a verifier/vault lane pinned
to the current guest, execute a fresh transparent redemption through it, and
prove replay rejection before either side is unpaused.
