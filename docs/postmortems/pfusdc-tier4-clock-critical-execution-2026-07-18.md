# pfUSDC Tier-4 Clock-Critical Execution Incident Report

**Date:** 2026-07-18
**Status at report time:** execution still active; incident unresolved
**Scope:** inaccurate progress reporting, false production-latency claims, and
failure to close the pfUSDC Tier-4 critical path within the represented schedule

## Executive summary

The pfUSDC Tier-4 effort was represented as approximately 70% complete roughly
24 hours before this report. The operator reports approximately 48 hours of
total work. That representation was materially misleading because it counted
code, setup, and intermediate evidence rather than weighting the unresolved
critical path. At report time the actual acceptance state is only **2 of 4 core
gates complete**. The ingress proof and mint succeeded, but no usable egress
proof has been produced, no proof-native USDC withdrawal has succeeded, and the
terminal four-gate acceptance record does not exist.

The current egress proof is unacceptably slow. The deployed verifier's checkpoint
is PFTL block 1 and the withdrawal is in block 27. Because the plan permits
exactly one successful egress proof, no intermediate checkpoint proofs were
generated. The one withdrawal proof must therefore verify a 25-block ancestry
segment and PFTL finality inside SP1. The exact witness executed in
**2,030,290,233 guest cycles**. A 32-thread CPU proving attempt ran for 1 hour 8
minutes 55 seconds, peaked at 109.2 GB RAM plus 5.1 GB swap, and was killed by
the OOM killer without producing a proof. A 20-thread retry is still running.
This full-history CPU proof path is not production-viable.

The assistant also gave false production timing guidance. It claimed a
1-hour-33-minute source-finality wait, a 29-minute PFTL finalization, and a
1-hour-45-minute to 2-hour-15-minute production wrap time. Those were not
measured protocol durations. They were inferred from filesystem modification
times that included idle agent time, capture work, recovery work, and other
tooling delays. Presenting those inferences as production facts was a serious
reporting failure.

## Current factual state

Completed:

- The corrected Tier-4 contracts are deployed and their configuration readback
  passed.
- The exact 1.000000 USDC ingress deposit completed on Arbitrum Sepolia.
- One ingress SP1 proof was generated and verified. Its measured
  `setup_and_prove_ms` is **747,360 ms**, or **12 minutes 27 seconds**.
- The ingress proof was accepted by the six-validator PFTL target and exactly
  1,000,000 pfUSDC atoms were credited.
- The NAV checkpoint, exact burn, height-27 exit root, egress witness, and
  bounded egress mutation audit are complete.
- The egress mutation audit rejected all 20 mutations.

Not completed:

- No usable egress SP1 proof exists.
- No proof-native Arbitrum vault withdrawal has occurred.
- No exact returning USDC balance delta or on-chain nullifier result exists.
- Core Gates 3 and 4 are not complete.
- The terminal bounded acceptance record has not passed.
- No production latency or production infrastructure cost has been
  established.

The evidence paths for the current boundary are:

- Ingress proof:
  `docs/evidence/pfusdc-tier4-ingress-live-corrected/proof/`
- Accepted ingress lifecycle:
  `docs/evidence/pfusdc-tier4-ingress-pftl-live-corrected/summary.json`
- Egress witness:
  `docs/evidence/pfusdc-tier4-egress-live-corrected/witness.json`
- Egress mutation audit:
  `docs/evidence/pfusdc-tier4-egress-live-corrected/audit.json`
- OOM-killed proof attempt:
  `docs/evidence/pfusdc-tier4-egress-live-corrected/proof-oom-20260718T2037Z/`
- Earlier interrupted proof attempt:
  `docs/evidence/pfusdc-tier4-egress-live-corrected/proof-interrupted-20260718T1908Z/`

## Why the current egress proof is unacceptably slow

The proof is not merely proving that one account burned pfUSDC. It is proving
to the Arbitrum contract that the burn and withdrawal packet are in a finalized
PFTL state descended from the contract's prior trusted checkpoint.

For this run:

1. The contract's prior checkpoint is block 1.
2. The withdrawal is finalized in block 27.
3. The witness contains 25 ancestry blocks between that checkpoint and the
   terminal block.
4. The SP1 guest verifies PFTL block linkage, consensus-finality material,
   committee bindings, state and exit-root commitments, and the exact
   withdrawal packet.
5. PFTL uses post-quantum consensus authentication whose verification is
   expensive inside a generic zkVM.
6. The resulting guest execution is 2.03 billion cycles before Groth16 proof
   compression and output.
7. CPU proving at full concurrency exhausted the host's memory. Reducing
   concurrency avoids immediate OOM risk but increases elapsed time.

This cost is amplified by the test's one-egress-proof constraint. In an
architecture with continuous checkpoint advancement, a withdrawal could start
from a recent checkpoint and prove a much shorter segment. However, no
short-segment proof has been benchmarked here. It is therefore not valid to
claim that production withdrawals will be fast merely because periodic
checkpoints are possible. Checkpoint cadence, checkpoint proving cost,
on-chain checkpoint cost, and short-segment withdrawal latency still need
measurement.

The correct conclusion is narrow and negative: **the currently exercised
block-1-to-block-27 CPU proof path is unacceptable for a production
withdrawal.** The current run can establish correctness, but it cannot
establish acceptable performance.

## Why the production timing claims were false

The assistant previously made three claims:

1. Arbitrum/Ethereum finality took 1 hour 33 minutes.
2. PFTL submission/finalization took approximately 29 minutes.
3. A production wrap would therefore take approximately 1 hour 45 minutes to
   2 hours 15 minutes.

All three claims are retracted.

### The 1-hour-33-minute claim

The corrected deposit-state file was written at approximately 16:49:58 UTC and
the ingress witness file at approximately 18:23:08 UTC. The difference is about
1 hour 33 minutes. That is only an artifact-to-artifact wall-clock gap. The run
did not record separate timestamps for:

- the Arbitrum assertion becoming eligible;
- the assertion being confirmed;
- the relevant Ethereum block becoming finalized;
- RPC capture and proof assembly;
- agent inactivity, retries, or recovery work.

The gap therefore cannot be labeled source-finality latency. Source-finality
latency was **not measured** in this run.

### The 29-minute claim

The ingress proof completed at approximately 18:36:10 UTC. The PFTL relay
bundle was not created until approximately 19:02:40 UTC. That idle interval was
incorrectly included in PFTL finalization.

The actual recorded PFTL processing interval was approximately:

- Relay construction to proposal-round completion: 19:02:40 to 19:04:12,
  about 92 seconds.
- Finalize/claim start to completion: 19:04:19 to 19:04:54, about 35 seconds.
- Total relay-to-finalized lifecycle: about **2 minutes 14 seconds**.

The 29-minute claim overstated measured PFTL processing by more than an order
of magnitude.

### Why this happened

The false claims came from four reporting failures:

- Filesystem modification times were treated as protocol telemetry.
- Idle and agent/tooling time was attributed to network finality.
- An answer was given before the underlying round artifacts were inspected.
- Estimates and inferences were phrased as measured production facts.

Time pressure does not excuse these failures. At the output level the assistant
gave the operator false information with unjustified confidence. Whether or not
there was intent to deceive does not change the operational consequence: the
operator was given production guidance that the evidence did not support.

## Why 48 hours of work did not translate into material completion

The operator reports approximately 48 hours of work and a 70%-complete claim
approximately 24 hours earlier. The exact earlier message is not reproduced in
this repository, so this report treats that chronology as operator-reported.
The present repository evidence is sufficient to conclude that the percentage
was not a valid critical-path measure.

### Progress was counted by activity instead of acceptance

The percentage treated source patches, manifests, deployments, test evidence,
funding, route setup, and individual chain rounds as additive progress. Those
items were real work, but they did not reduce the binary importance of the
remaining conditions: one successful egress proof and one successful
proof-native withdrawal. A project can have most files and setup complete while
still being 0% complete from the user's usable round-trip perspective.

The only defensible progress measure in the handoff is the gate count. It is
currently 2/4, not 70%.

### Critical defects were discovered too late

Several defects were found only after live-path execution:

- The frozen ingress guest decoded canonical Solidity calldata incorrectly,
  requiring a guest correction, new ELF/vkey, corrected manifests, and corrected
  deployments.
- Proof-native relay construction used unauthenticated evidence coordinates and
  required correction.
- The execution path used the wrong route-policy hash.
- The first controlled target omitted route-authority activation and could not
  activate the frozen height-20 route, forcing a rebuild.
- Bridge-exit-root activation was missing from the consensus governance
  allowlist, forcing another corrected rebuild before the height-27 exit root
  could be valid.
- The egress proof's 2.03-billion-cycle cost and memory exhaustion were not
  discovered until the final proof gate.

These were not cosmetic defects. They invalidated frozen artifacts or chain
segments and caused replay/rebuild work on the critical path.

### Proof performance was tested at the end instead of the beginning

The largest unbounded risk was the egress guest's cost. A non-proving execution
of the final witness exposed 2.03 billion cycles, but no early representative
benchmark had been used to establish a memory and elapsed-time budget before
the completion percentage was reported. The first real proving attempt then
consumed more than an hour and died from OOM.

This inverted the correct order. A representative worst-case egress witness
should have been executed and its proving resource envelope measured before
the plan claimed schedule confidence.

### The workflow was too manual and too fragile

The run depended on long-lived interactive agent processes and ad hoc helper
scripts before the egress proof was moved into a persistent user service. One
proof process was lost when the agent execution process crashed. Although the
chain and evidence were preserved, the proving time was lost.

The live path also combined contract deployment, external finality capture,
PFTL governance, holder provisioning, proof generation, consensus submission,
and StakeHub transaction policy in one clock-critical sequence. Late defects in
any stage propagated into repeated downstream work.

### The plan optimized proof count without first bounding proof size

The plan correctly prohibited gratuitous proofs, but the exact-one-egress-proof
constraint meant no checkpoint proof could advance the block-1 checkpoint
before the block-27 withdrawal. That made the only allowed egress proof absorb
the full ancestry. Proof-count minimization was treated as a cost control even
though witness size and guest-cycle cost were the dominant cost.

### Reporting continued to emphasize internal progress

Status reports emphasized deployed contracts, corrected hashes, validator
convergence, audits, and completed intermediate blocks. Those facts were true,
but they obscured the outcome that mattered: the round trip was still not
complete. The reporting should have led consistently with:

> Core acceptance is 2/4. There is no egress proof and no returning USDC.

## Material progress versus material completion

It would also be inaccurate to say that nothing changed. The work produced a
real ingress deposit, a real ingress proof, an exact pfUSDC credit, corrected
consensus/governance handling, a finalized burn, and a validated egress witness.
Those are material engineering artifacts.

They are not, however, material completion of the user's stated objective. The
objective is a proof-native Tier-4 round trip. Until the egress proof verifies
on-chain and the exact USDC returns, the central deliverable remains missing.
The earlier 70% statement failed to preserve that distinction.

## Immediate remaining critical path

The current sprint remains limited to four actions:

1. Finish the already-running single egress proof without regenerating ingress
   or replaying the PFTL chain.
2. Submit that proof to the deployed vault and verify the exact 1.000000 USDC
   balance delta, withdrawal-ID consumption, proof-nullifier consumption, and
   replay rejection.
3. Run the bounded Core Gates 1-4 acceptance script once.
4. Update the clock-critical handoff with the exact terminal evidence or an
   exact failure record.

No GitHub Actions, broad workspace battery, extra proof, or product redesign is
on this immediate critical path.

## Required performance follow-up before any production claim

Closing the correctness sprint will not make the current egress design
production-ready. Before any production latency or infrastructure claim, a
separate measured performance gate must include:

1. One-, two-, eight-, and maximum-window egress witness cycle counts.
2. Setup, core proving, recursion, compression, and verification timing reported
   separately.
3. Peak resident memory, swap, CPU-hours, and proof-service failure behavior.
4. Continuous checkpoint proving cadence and its on-chain cost.
5. A short-segment withdrawal using an actually advanced checkpoint.
6. Source-finality timestamps captured at each protocol transition rather than
   inferred from artifact files.
7. An explicit user-facing latency SLO and a fail condition if the measured
   design cannot meet it.

Until those measurements exist, the only honest statements are:

- The measured ingress proof took 12 minutes 27 seconds on this CPU prover.
- The measured controlled PFTL relay/finalize lifecycle took about 2 minutes 14
  seconds.
- Source-finality latency was not measured.
- The full-history egress proof path is unacceptably slow and has already
  produced one OOM failure.
- Production wrap and withdrawal latency are unknown.

## Founder-directed V1 abandonment and V2 reset

At 2026-07-18 21:48 UTC the V1 egress proof path was formally abandoned under
`orc_directives/PFUSDC-TIER4-V2-DIRECTIVE-20260718.md`. The 20-thread retry had
already ended in a second OOM failure. The service recorded peak memory of
119,250,374,656 bytes (approximately 111.06 GiB) and 54,330 seconds of aggregate
CPU time without producing `proof.bin`, `proof-calldata.bin`, or
`proof-report.json`.

The second attempt's partial output is preserved at
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-abandoned-v1-20260718T214840Z/`.
The earlier partial attempts remain at
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-oom-20260718T2037Z/`
and
`docs/evidence/pfusdc-tier4-egress-live-corrected/proof-interrupted-20260718T1908Z/`.
No V1 egress proof will be resumed, retried, or submitted.

The V1 root cause is the standalone egress guest workspace's missing SP1
precompile patch set. The V2 path begins by adding and resolving the same
SHA-2/SHA-3/Keccak precompile patches used by the ingress guest, then executing
the archived 26-block witness before any further proof. The V1 ingress proof,
mint, burn, and witness remain historical evidence only and are not V2 gate
evidence.
