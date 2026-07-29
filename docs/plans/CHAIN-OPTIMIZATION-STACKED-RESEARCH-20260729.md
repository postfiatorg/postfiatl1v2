# Stacked Chain Optimization Report: Latency, Swaps, ZK, PQ

**Date:** 2026-07-29
**Scope:** whole-stack performance program for PFTL: NAV swap round trips,
Halo2 privacy proving, SP1 ingress/egress proving, consensus latency,
Ethereum-facing finality, and the post-quantum signature/proof layer.
**Relationship to prior work:** extends
`NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md` (Tier 0-2, orchestration-focused)
with externally researched, deeper-stack options. Tier numbers there are
unchanged; this report introduces stacked layers `S1-S6` that compose with
them.

## Where we stand (measured internal baselines)

| Metric | Value | Source |
|---|---:|---|
| A666 issue leg (deposit -> spendable wA666) | 1,848s | Phase 8 evidence |
| A666 redemption leg (burn -> USDC release) | 2,688s | Phase 8 evidence |
| Halo2 AssetOrchard cold `pk_build` (K=15) | ~330s | zk-prover-optimization-results |
| Halo2 hot prove (K=15, cached PK) | 5.78s | zk-prover-optimization-results |
| SP1 Groth16 wrap (CUDA) | ~190-210s each | Phase 8 evidence |
| Warm PFTL consensus round | 50-70s e2e; 426ms hot certified | private-egress perf plan |
| Controlled-testnet wallet finality | p50 290ms / p95 375ms | subsecond-finality milestone |
| Serial 6-validator checkpoint vote round | ~3.2 min | Phase 8 evidence |
| Ethereum 2-epoch finality | ~780s | chain floor |

The consensus core is already fast when warm. The stack's latency is
dominated by (a) cold prover state, (b) serialized orchestration, (c) the
Ethereum finality floor, and (d) per-statement Groth16 wraps.

## Implemented optimization run: pass 3

The first immutable, hands-off optimization lineage
(`a666-opt-pass3-20260729`) ran from a frozen preflight at PFTL height 453.
It completed the private issue path through Ethereum mint and PFTL
destination-consume without a manual resume or state repair. The strict
latency gate then stopped the runner before redemption because issuance
missed the 1,500-second SLO.

| Pass-3 result | Measured | Gate | Verdict |
|---|---:|---:|---|
| Private issue, Ethereum deposit block -> spendable wA666 mint block | 1,824s | <=1,500s | FAIL |
| Private redemption recovery, wA666 burn block -> USDC withdrawal block | 852s | <=1,500s | PASS |
| Minted/redeemed NAV | 1,000,000 atoms | exact | PASS |
| Deposit / withdrawal | 905,538 / 900,581 USDC atoms | governed NAV arithmetic | PASS |
| Final PFTL supply and Ethereum wrapper conservation | baseline restored | exact | PASS |
| Withdrawal replay rejection | rejected | required | PASS |

The issue leg was functionally correct:

- deposit `0x7552c0aba0ff317240fd8bb965ca57acdddaf1f7266f0e456d3bf2430b634c07`
  was included at Ethereum block 25,635,693;
- full-finality ingress proof verification, pfUSDC relay, private primary
  issue, private A666 egress, and native export finalized at PFTL heights
  454-459;
- Ethereum acceptance and mint completed at blocks 25,635,844-25,635,845;
- the wallet increased by exactly 1,000,000 wA666 and PFTL recorded
  destination consumption at height 460.

The measured issue critical path was:

| Boundary | UTC | Increment |
|---|---:|---:|
| Deposit confirmed | 03:06:50 | - |
| Full-finality ingress witness available | 03:22:29 | 939s |
| Ingress Groth16 proof | 03:24:55 | 146s |
| pfUSDC relay complete | 03:26:31 | 96s |
| Private pfUSDC ingress complete | 03:27:46 | 75s |
| Private primary issue complete | 03:30:26 | 160s |
| Private A666 egress complete | 03:32:12 | 106s |
| Native export complete | 03:33:22 | 70s |
| Export Groth16 proof | 03:36:41 | 199s |
| Ethereum mint state recorded | 03:37:17 | 36s |

This isolates the miss: Ethereum finality consumed about 15m39s and the
post-finality pipeline consumed about 14m48s. The latter, not correctness or
liquidity, is the remaining controllable issue bottleneck. The clean runner
stopped as designed after destination consumption. A separately labeled
recovery then privately redeemed the reconciled test position in 852
seconds, returned 900,581 USDC, rejected withdrawal replay, settled at PFTL
height 466, and restored authorized supply, outstanding claims,
Ethereum-spendable supply, wallet wA666, and wrapper total supply to their
pre-pass values. The only retained economic delta was the expected 4,957
atom governed spread.

Canonical evidence:
`docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass3/`.
The concise independent recovery record is
`pass3-recovery-summary.json`. Private notes and spending material remained
on validator-2 and are not present in the evidence tree.

## S1 — Halo2 privacy-prover layer

### S1.1 Proving-key serialization (removes the 330s cold build class)

Upstream `zcash/halo2` only supports read/write for the *verifying* key;
stable `ProvingKey` serialization is an open request (issues #443/#449), with
upstream's stance being that the PK is recomputable from circuit code and
disk-loading is "just" a performance optimization. That optimization is
exactly what we need: our measured recompute cost is ~330-341s and it is the
single largest line item in the redemption leg.

Two implementation routes, in preference order:

1. **Port the PSE-fork PK serde surface** (`ProvingKey::write/read` with
   `SerdeFormat::RawBytesUnchecked` for trusted local artifacts) onto our
   pinned `halo2_proofs 0.3.2`, the same way the pinned-VK compatibility
   patch was done. Load fail-closed with a fingerprint check against the
   pinned VK. Expected load time: seconds (bounded by disk/mmap of a
   few-hundred-MB artifact), replacing ~341s of keygen.
2. If patching is deemed too risky for the frozen release: **resident prover
   daemon only** (Tier 0.1), accepting that any daemon restart re-pays the
   build once.

Route 1 hardens route 2 (daemon restarts become cheap) and also fixes the
one-shot CLI paths we cannot daemonize. Ship the PK artifact per release
with the same regression-gate treatment as
`scripts/private-egress-pinned-vk-regression-gate`.

### S1.2 GPU Halo2 proving (targets the remaining 5.8s hot prove)

The ecosystem now has near-fully-GPU Halo2 provers: ICICLE-Halo2 offloads
MSM, NTT, and gate/permutation/lookup evaluation with reported up-to-25x
prover gains; Snarkify's cuSnark and Kroma's Tachyon are drop-in Halo2 GPU
backends. Our A100 host already exists for SP1 CUDA. Porting the
AssetOrchard prover onto a GPU backend is the credible path below the missed
`<5s` CPU target — realistic outcome is sub-second to low-single-second hot
proves. Caveats: our circuit is on Pasta curves (IPA, not KZG); ICICLE
supports non-BN254 fields but integration must be validated against
`halo2_proofs 0.3.2` semantics, and proofs must remain byte-compatible with
the pinned VK (backend change must not change the transcript).

### S1.3 Batched action proving

`halo2_proofs` supports proving multiple circuit instances in one proof
run (`batch` feature is default). Where a flow needs N actions (ingress +
redeem + egress), creating them in one resident-prover session amortizes
witness setup and keeps all data hot. Pairs with S6.1 (single consensus
batch).

## S2 — SP1 / SNARK-wrap layer

### S2.1 Track the zkVM frontier (order-of-magnitude headroom)

Our SP1 proofs (ingress witness, export, egress) plus ~3-minute Groth16
wraps are ~8-10 min per round trip. The zkVM field has moved dramatically:
SP1 Hypercube (multilinear-polynomial proof system) claims up to 5x better
latency/cost than SP1 Turbo, proves >93% of Ethereum mainnet blocks in
under 12s, and by late 2025 reached 99.7% of blocks under 12s on 16 RTX
5090s; it is now live on mainnet infrastructure. Competing zkVMs (Brevis
Pico Prism) report 6.9s average block-proving latency on 64 RTX 5090s. Our
guest programs (Helios finality verification, receipt/witness checks) are
orders of magnitude smaller than a full EVM block, so a Hypercube-class
prover should take our per-proof latency from minutes to seconds.

Action: benchmark our four guest programs on the newest SP1 release train
on the existing A100 (and price a 5090 host); adopt behind the standard
pinned-vkey governance flow (new `program_vkey` per release, fail-closed).

### S2.2 Aggregate statements, pay the wrap once

Deployed systems wrap fast STARK/FRI proofs in Groth16/fflonk for cheap
on-chain verification; recursion lets many statements compress into one
wrapped proof. We currently pay the ~3-min wrap per statement (export,
destination-consume, egress). Aggregating them into one guest (research
report Tier 2.4) halves-to-thirds the wrap cost per leg and simplifies the
Ethereum verifier surface. With S2.1 the wrap itself also shrinks.

### S2.3 Overlap and residency

Keep the CUDA prover warm and start wraps concurrently with Ethereum
preflight (Tier 1.2). With S2.1+S2.2 the proving pipeline stops being a
critical-path stage at all: everything fits inside the finality window.

## S3 — Consensus / FastPay layer

### S3.1 Certified fast path for payments (FastPay lineage)

We already ship `fastpay_speculative_effects_v1` state. The reference
points: FastPay demonstrates sub-100ms intra-continental confirmation for
consensus-less payments; Sui's Mysticeti (uncertified DAG) achieves ~0.5s
WAN consensus commit at 50k-200k TPS, and its FPC variant weaves
FastPay-style consensus-less commits into the DAG to cut fast-path latency
further. Our warm certified round is already 426ms with six validators, so
the near-term win is not a consensus redesign but (a) extending the
fast-path/speculative-effects coverage to more transaction classes (swap
reserve/subscribe legs, pfUSDC claims), and (b) keeping validators
resident + multiplexed (Tier 0.4) so every round is a hot round.

### S3.2 Parallel vote fan-out (reiterated, now with precedent)

Serial ssh vote collection is ~15-25s per validator. All modern BFT
implementations broadcast concurrently. Concurrent fan-out is a pure
orchestration fix worth ~3 min per checkpoint round (x2 per round trip).

### S3.3 If/when the validator set grows

For a larger, adversarial WAN set, the literature warns that uncertified
DAGs suffer under even mildly faulty/slow peers (data fetching lands on the
critical path; Shoal++/Autobahn report large latency degradation with a few
lossy nodes). If PFTL scales validator count, prefer certified-DAG or
Autobahn-style designs over copying Mysticeti wholesale. Not a 2026
priority at N=6.

## S4 — Ethereum-facing finality layer

### S4.1 The floor is real but is being re-priced

Ethereum L1 finality remains ~2 epochs (~13-15 min); 3SF/SSF remain
research-track (3SF spreads BFT phases across three slots; still faces
million-BLS-signature aggregation constraints and validator-set downsizing
questions). Do not plan on a protocol change landing inside our acceptance
horizon.

What has changed in 2026: the **Fast Confirmation Rule (FCR)** gives a
formal single-slot confirmation rule (no hard fork required) that closes
the "finality-assumption gap" bridges have historically hand-waved with
confirmation-count heuristics, and is analyzed as immune to rational
finality-stall attacks. This is the right technical substrate for the
research report's Tier 2.2 (risk-priced early acceptance):

- **Small swaps (below a governed cap):** accept an FCR-confirmed or
  1-epoch-justified checkpoint, with the 0.5% issue spread explicitly
  priced as reorg insurance, attested by the existing six-validator
  checkpoint vote quorum. Floor drops from ~13 min to ~1-6 min.
- **Large swaps:** keep full 2-epoch finality.
- Model note: pre-finality acceptance risks include rational finality
  stalls and bridge-targeted reorg MEV; the cap plus spread must price
  both, and the policy must be governed and fail-closed.

### S4.2 Intent/relayer fast lane (product-level, zero protocol change)

The fast bridges (Across-style) hit seconds-level UX by having a relayer
pre-fund the destination and absorb finality risk for a fee, while
canonical burn/mint flows (e.g. CCTP) wait ~13 min for Ethereum finality.
PFTL can offer the same as a product tier: an operator- or third-party-
funded fast lane that fronts pfUSDC/wA666 immediately and reconciles
against the trustless proof flow. The trustless path remains canonical and
unchanged; the fast lane is pure UX on top.

### S4.3 Overlap everything with the wait

Unchanged from Tier 0.3/1.2-1.3: continuous verifier checkpointing,
prewarmed provers, witness scaffolding, and preflights all belong inside
the finality window. With S2.1, proving disappears into it entirely.

## S5 — Post-quantum signature/proof layer

Current state: ML-DSA transactions with an SP1-guest ML-DSA verification
precompile (mldsa-precompile-20260718 lane), and "compressed quantum
proofs" via STARK->Groth16 wrapping for Ethereum.

### S5.1 Aggregate ML-DSA under a PQ-transparent proof

Precedent now exists at scale: BNB Chain announced migration of
transaction and consensus signatures to ML-DSA-44 with **pqSTARK
aggregation** of the signatures. For PFTL, a STARK proof that "N ML-DSA
signatures over these messages verify" collapses per-transaction 2,420-byte
signatures and per-vote verification cost into one succinct, hash-based
(quantum-safe) artifact — directly shrinking blocks, checkpoint
certificates, and light-client material. This composes with our existing
SP1 ML-DSA guest acceleration.

### S5.2 Know what the Groth16 wrap costs us

The standard STARK-in-Groth16 wrap "sacrifices quantum resistance and
requires a trusted setup" — acceptable today because the Ethereum verifier
contract side is classical anyway, but it means our *bridge* proofs are not
PQ end-to-end even though the *chain* is. Track hash-based wrap targets
(STARK-verifier precompiles / cheap FRI verification on Ethereum) so the
wrap can be swapped out later; keep the wrap isolated behind the existing
pinned-vkey abstraction so this is a release-train change, not a redesign.

### S5.3 Falcon/FN-DSA for size-critical paths (watch, don't jump)

FN-DSA signatures are ~3.6x smaller than ML-DSA at comparable security
with faster verification, now NIST-standardized as FIPS 206 — attractive
for high-volume payment traffic and light clients. But signing requires
constant-time floating-point Gaussian sampling with a history of
side-channel pitfalls, and lattice threshold/multisig tooling is not
production-grade for either scheme. Position: keep ML-DSA as the canonical
transaction scheme; consider FN-DSA only for validator-vote or
light-client-artifact compression after S5.1, which may make the point
moot (aggregation already removes the size pressure).

## S6 — Architectural (carried and extended)

- **S6.1 Single-batch private redemption** (Tier 2.1): orchard ingress +
  private redeem + private egress as one shielded batch/round; with S1
  residency the inter-round prover gaps vanish anyway, but one round is
  still strictly better for resume semantics.
- **S6.2 PFTL-only private NAV path** (Tier 2.5): private redemption of
  PFTL-held A666 without the Ethereum excursion; removes the finality
  floor for the product's most common case.
- **S6.3 SLO split by clock** (Tier 2.3): report `chain_seconds` vs
  `system_seconds`; gate releases on what we control.

## Stacked projection

Redemption leg (today 2,688s):

| Stack | Projected | Mechanism |
|---|---:|---|
| Tier 0 only | ~800-1,000s | resident prover, parallel votes, overlap |
| + S1.1/S1.2 | ~500-700s | PK artifact + GPU Halo2 (hot prove <2s) |
| + S2.1/S2.2 | ~250-400s | seconds-class SP1 + single wrap |
| + S6.1 | ~150-250s | one consensus round, no gaps |

Issue leg (today 1,848s): floor-bound at ~800s (finality) + ~60-120s of
system time once S2 lands; with S4.1 small-swap early acceptance,
~120-400s total for capped amounts.

## Sequencing

1. **Now (frozen-release-safe):** Tier 0.1-0.4 + Tier 1.4 timestamps.
   Unchanged top priority.
2. **Next release train:** S1.1 pinned PK artifact; S2.2 statement
   aggregation; Tier 1.2/1.3 overlap and continuous checkpointing.
3. **Following train:** S2.1 zkVM upgrade benchmark + adoption; S1.2 GPU
   Halo2 spike (transcript-compatibility gate).
4. **Governance decision:** S4.1 FCR/justified-checkpoint early acceptance
   with governed cap and priced spread (answers the "change the gate or
   the architecture" question), and/or S6.3 SLO split.
5. **Program work:** S6.1 single-batch redemption, S5.1 pqSTARK ML-DSA
   aggregation, S4.2 fast-lane product, S6.2 PFTL-only private path.

## Safety constraints (unchanged, binding on every layer)

- Independent validator re-execution before votes; no proposer-trust
  shortcuts.
- All new artifacts (PK, vkeys, GPU-backend proofs) pinned + fingerprinted,
  fail-closed, with regression gates mirroring the pinned-VK precedent.
- Pre-mutation rejection and replay protection semantics unchanged.
- Private material stays on validator-2, mode 0600, out of evidence;
  resident daemons hold seeds/spending keys with one-shot discipline.
- Any early-acceptance policy (S4.1/S4.2) must be governed, capped, and
  priced; the trustless full-finality path remains canonical.

## External references

- Succinct, "SP1 Hypercube" + 16-GPU real-time proving updates
  (blog.succinct.xyz).
- Brevis Pico Prism real-time proving results (Oct 2025).
- Ingonyama ICICLE-Halo2 v2; Snarkify cuSnark; Kroma Tachyon (GPU Halo2).
- zcash/halo2 issues #443/#449 (ProvingKey serialization).
- Mysticeti (arXiv 2310.14821); FastPay; Shoal++ (arXiv 2405.20488).
- 3SF protocol (D'Amato et al.); SoK: Speedy Secure Finality
  (arXiv 2512.20715); Ethereum Fast Confirmation Rule analyses (2026).
- Alpen Labs, "Current state of SNARKs" (wrap tradeoffs).
- FIPS 204/205/206 landscape; BNB Chain ML-DSA + pqSTARK aggregation
  announcements; Falcon deployment analyses (2026).
