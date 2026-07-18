# pfUSDC Tier-4 initial consensus-v2 checkpoint

This record proves the prerequisite controlled-target checkpoint. It is not a
claim that Core Gates 2-4 are complete.

- Six validators started from the same fresh genesis with consensus-v2 active
  at height 1.
- Deterministic proposer `validator-1` certified a signed faucet self-transfer.
- The receipt was literally `code=accepted` on every validator.
- Every validator finalized height 1 at the same block ID and state root.
- The block contains a 5-of-6 prepare QC and a 6-of-6 precommit QC.
- `verify-state` and `verify-blocks` passed independently on every validator.
- The validators were stopped after verification; no service was left running.
- No SP1 proof, EVM deployment, deposit, burn, or live-fund transaction occurred.

The full block/certificate artifacts remain in the controlled target at the
paths recorded in `hashes.json`. This repository evidence intentionally records
only public commitments and sanitized results; it does not copy validator or
faucet secret-key material.
