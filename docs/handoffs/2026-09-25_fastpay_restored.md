# FastPay restored on the six-validator fleet

FastPay is restored on `postfiat-wan-devnet-2`. All six validators run signed
release `fastpay-committee-20260925-r3` from source `943c4ca7b6de28d840d4ca5075a4cbff55a74eee`.
Both validator and RPC process hashes were checked on every host.

The retained **0.001 PFT** payment now appears as accepted in the existing Python
CLI and wallet UI. Its original five votes and five acknowledgements were verified.
Final recovery resumed the existing operation without another wrap or FastPay transfer.
An ordinary transfer returned unused test funds and anchored the FastPay effect at
**height 1034**. All six nodes expose the exact recipient output, confirm input
consumption, and agree on the finalized root. This is a separate test wallet; the original
external client's one-PFT operation was not accessed or claimed recovered.

## Changes

- Authorize each local signer against its current registry key and pinned FastPay
  committee key. The rotated sixth signer remains ineligible; quorum stays five.
- Reconstruct pending signed FastPay effects from the durable journal over the
  finalized transactional database. Wallet reads and block execution see the same
  effects; canonical checkpoint state changes only through ordered certification.
  Restart, dependency ordering, idempotence and certified omission are tested.
- Preserve finalized-checkpoint verification when restoring active storage.
  Ordinary full-history import still enforces historical replay.
- Derive exact output objects in the Rust SDK after certificate/ack verification,
  and expose those verified fields to the existing Python wallet interface.

The intermediate authority-only rollout exposed the database/read-path defect.
It was rolled back on the five eligible signers without changing ledger state;
FastPay certificate formation remained blocked until the final repair was qualified.

## Validation and recovery

Passed: 22 main and 21 release FastPay node tests; 20 release execution tests;
Rust SDK cryptographic verification; 2 checkpoint and 14 storage tests; 67 Python
wallet tests; four FastPay proxy scripts. Replacing the new read view with the old
canonical-only read reproduces the missing-output failure. Frozen output-ID vectors
preserve compatibility. Forged votes, duplicate votes and missing dependencies reject.

The exact retained live certificate passed unsigned and signed checkpoint restores
at height 1033. Repeated fresh-process queries expose the same output without a
second debit, and corrupted journal votes reject. A separate signed fleet backup
passed before canary deployment and sequential all-six convergence checks.
Verification basis is **consensus-v2 finalized checkpoint**, not full-history replay;
the known block-1011 replay anomaly is unchanged.

Node SHA-256: `a82684e2462b43fd91e6806c322b8f3af2636a7f5fea0662b447c564292746aa`.
Final state root: `509b0a0db83ebe5ba53b39d7b7052604df30d3c0753bbbae637d1bb1d35e4632c6ce8ecdcf68b6d3046a1220f25c572e`.
[Machine-readable evidence](2026-09-25_fastpay_repair_evidence.json) records source,
binary, backup, payment, UI and convergence evidence without private wallet material.
The implementation is in core source commit `d8f65885`; the release branch preserves
the exact deployed source archive plus the bounded repairs.

Use the [FastPay recovery guide](../runbooks/fastpay-committee-recovery.md) to update
the Python/SDK pair and resume an existing operation on its original wallet host.
**Five eligible signers satisfy quorum five: one more signer outage stops new
FastPay certificates.** A governed committee replacement remains separate work.
Task Node was unavailable; no generated task, evidence submission or reward is claimed.
