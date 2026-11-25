# Solana payout service install bundle for the second exit release

- Source revision: `af1baf87` on `release/sol-exit-r2` (full hash in the bundle's `revision` file)
- File: `/home/postfiat/var/sol-exit-r1/sol-exit-install-af1baf87.tar`
- Size: `90224640` bytes
- SHA-256: `048e3dd22b2f8e13ec96215fb3690e70f35c4a0ca16b521858a9a5190fca87b5`
- Built 2026-11-24. Pins the upgraded verifier (`0x3406becc…`, the 123,152-byte build zero-filled to the extended ProgramData). Install only after the Solana upgrade and migration; stop the withdrawal unit first.

## Speed-up bundle

- Source revision: `c033278b` on `ops/sol-exit-r2-speed`
- File: `/home/postfiat/var/sol-exit-r1/sol-exit-install-c033278b.tar`
- Size: `90224640` bytes
- SHA-256: `9fac1ea70a29be5aabca92be7d2f7f4cccbf32ad33254eece0d4a47dca1d7613`
- Differs from `af1baf87` only in `exit_payout.py` and `exit_proof.py`: each block is read once per payout, the ancestry walk stops at the prior checkpoint, and the proof inputs upload while the GPU proves. On the first second-release payout the reads took 22 s instead of 42 s and produced a byte-identical witness.
