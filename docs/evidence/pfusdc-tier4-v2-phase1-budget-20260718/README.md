# pfUSDC Tier-4 V2 Phase-1 execution budget

**Recorded:** 2026-07-18 22:07 UTC
**Host RAM:** 122 GiB
**Founder local-proving limit:** 61 GiB projected peak RSS (50%)
**Status:** local representative proof hit the mandatory cap; escalated to a
systemd-capable rented high-memory VM; no V2 proof generated yet

## Candidate identity

- Patched-only ELF SHA-256:
  `b404f71b82e062bba4e228f11f69404701e9030c9305bc152f5954040dd8f67f`
- Patched-only vkey:
  `0x00fdae512ddc8cee8d184f82ae155e1517dc6c8757faa0741dd26eed65d9981a`
- Optimized ELF SHA-256:
  `27e0d07d5563b982ef549a4ad793c0b49d9ef69534aff33ac2c6a224cc9f0099`
- Optimized vkey:
  `0x0079327e8673d2d415897390d9ad7c050bb0f74d694e498b62436b568bc0116e`

These are benchmark candidate identities, not frozen deployment artifacts.

## Root-cause delta

The abandoned V1 exact witness executed in 2,030,290,233 cycles. Adding the
SP1 `sha2 0.10.9` and `sha3 0.10.8` patches reduced the same witness to
1,425,953,847 cycles, a 29.8% reduction. This remained outside the local memory
limit, so the directive's conditional ancestry work was required.

The optimized guest verifies only the valid non-nil precommit commit QC for
ancestry blocks while retaining full proposal, prepare-QC, and precommit-QC
verification for the terminal block. It also avoids re-verifying QCs already
admitted into `ConsensusV2QcGraph` and reuses unchanged committee roots. The
exact witness then fell to 520,023,827 cycles, a 74.4% reduction from V1.

The optimized public values are byte-identical to the V1 public values for the
archived witness.

## Measured execute table

| Segment | SP1 cycles | Execute time | Projected peak RSS | Projected time to prior OOM phase |
| ---: | ---: | ---: | ---: | ---: |
| 1 block | 50,777,571 | 1.922 s | 2.78 GiB | 1.19 min |
| 2 blocks | 66,428,387 | 2.232 s | 3.63 GiB | 1.55 min |
| 8 blocks | 159,095,645 | 4.690 s | 8.70 GiB | 3.72 min |
| 26 blocks (exact archived witness) | 520,023,827 | 13.116 s | 28.45 GiB | 12.16 min |
| 64 blocks | 1,023,917,161 | 15.341 s | 56.01 GiB | 23.94 min |

Peak RSS in this table was a pre-proof projection using the measured V1 ratio:
119,250,374,656 peak bytes / 2,030,290,233 cycles = 58.7356 projected bytes
per cycle. The time column scales the second attempt's 47-minute-27.810-second
wall time to its OOM phase. It is not a complete-proof latency claim.

The 26-block proof disproved that linear RSS projection. The local systemd
unit reached its exact `MemoryMax=60G` ceiling (64,424,509,440 bytes), used
4,602,269,696 bytes of swap, and was killed by the cgroup OOM controller at
2026-07-18 22:08:51 UTC. No receipt was produced. There will be no local retry.

## Phase-2 representative selection and proof-purpose record

The 64-block witness is only 5 GiB below the local ceiling and its projected
time to the prior failure phase is already 23.94 minutes, before proof
completion. It therefore does not credibly meet the directive's approximately
30-minute deadline envelope.

The exact 26-block archived witness is the largest measured segment that is
both comfortably below the memory ceiling and projected to fit the timebox. It
is selected for the one permitted Phase-2 representative benchmark proof.

**Proof purpose, recorded before start:** establish setup/core/recursion/
compression/verification wall time and peak RSS for the optimized V2 egress
guest on the exact 26-block witness. This proof is benchmark evidence only. It
will not be submitted to a contract and is not the final V2 egress proof.

The proof must run as a persistent systemd user service with `MemoryMax=60G`.
If that hard limit is exceeded, the local benchmark fails and further proving
must move to a rented high-memory host.

That condition occurred. The failed partial output is archived at
`docs/evidence/pfusdc-tier4-v2-representative-proof-26block-local-cap-20260718T220851Z/`.
The identical bounded proof is being moved to a rented VM with systemd and a
new explicit `MemoryMax`.

## Evidence

- Patched-only exact execute:
  `docs/evidence/pfusdc-tier4-v2-benchmark-patched-26block-20260718/`
- Optimized exact execute:
  `docs/evidence/pfusdc-tier4-v2-benchmark-optimized-26block-20260718/`
- Synthetic witnesses:
  `docs/evidence/pfusdc-tier4-v2-segment-benchmark-witnesses-20260718/`
- Synthetic execute reports:
  `docs/evidence/pfusdc-tier4-v2-benchmark-1block-20260718/`,
  `docs/evidence/pfusdc-tier4-v2-benchmark-2block-20260718/`,
  `docs/evidence/pfusdc-tier4-v2-benchmark-8block-20260718/`, and
  `docs/evidence/pfusdc-tier4-v2-benchmark-64block-20260718/`.
