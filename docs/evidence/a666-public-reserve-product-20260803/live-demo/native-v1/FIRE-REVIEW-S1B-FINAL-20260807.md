# FIRE REVIEW S1B FINAL 2026-08-07

## 1. VERDICT
**S1b EXPIRED/UNSAFE for execution — binding VALID, window CLOSED. Do not fire from S1b. Awaiting S1c refresh. GO phrase "GO FIRE-20L-EXEC-3" remains the ONLY authorization and now requires an S1c binding.**

## 2. S1B BINDING SUMMARY
Stage S1b covers 10 active packets: leg0, leg1, leg2a, leg2b, leg3a, leg3b0, leg3b, leg3c, leg3d, and leg3e. Legs 3f/3g/3h are S2-deferred; S3/S4 are out of scope.

| Fact | S1b value |
|---|---|
| Quorum | Block 25704345, hash 0x43642b3e...8139, base fee 159,284,909 wei, publicnode+dRPC; Llama HTTP 521 substituted |
| Leg3e | Deadline 1786124483; fill 8,057,858 atoms (8.057858 USDC), zero drift; min-out 7,816,122 atoms (7.816122 USDC) |
| Fork receipts | 3c, 3d, 3e, 3f, 3g, 3h all status 0x1 |
| Leg3h advisory | 11,013,374 atoms (11.013374 wA666), min-out 10,682,972 atoms (10.682972 wA666) |
| Gas | 0.480635 USDC; projected cumulative 501.505480 USDC <= 530.000000 USDC |
| Fleet | h778, root b287451679a9d4d9, unchanged |
| Deposit expiry | h1776, 998 blocks remaining |
| Replay / nonce | Reservation replay absent; EVM nonce 304 |

## 3. EXECUTION-WINDOW GATE
Leg3e deadline is **17:41:23 UTC**. Conservative measured time-to-leg3e-mined is 25 minutes: EVM legs take 1-2 minutes each at 12-second blocks; PFTL phases require 20-60 seconds for build, signing, and finality plus approximately 30 seconds rsync per refresh, anchored to /tmp/krimp-exec-fire20260806/leg-1/ execution timestamps. Nine stages precede leg3e. With a 10-minute margin, 35 minutes are required. Minimum safe GO-by was **17:06:23 UTC**, already past at publication near 17:20 UTC with approximately 21 minutes available. **GATE FAIL: EXPIRED/UNSAFE.**

## 4. BUILD-PROVENANCE DISCLOSURES
- R1 base-first overlay: D1 67-leaf leg1 carry and FIRE-10 13-leaf leg3a carry; resolver is pointer-set-only.
- R2 staged-exemption carry: 83 active entries, with fresh-resolved drops and vestigial drops.
- R3 vestigial pre-D1 metadata drops: 6 per array, covering leg1 prover-hash/deposit-report pointers into the pre-amendment command layout, extending D1.
- S1 remains 16/16 immutable; values-S1.json is byte-unchanged.

## 5. S1C RECOMMENDATIONS
- The pipeline is rehearsed and refresh takes approximately 20 minutes. Cut S1c only when Sauron is ready to fire.
- Lengthen the leg3e deadline convention. Fork timestamp plus 3600 seconds gave a one-hour window that build latency consumes. Recommend fork timestamp plus four hours; this requires packet-convention sign-off.
- Add a pre-fire liveness gate: chain has remained idle at h778 since the fire review, so confirm block production before GO.

Evidence: /tmp/snaga-s1b/NUMBERS.json, /tmp/snaga-s1b/gas.txt, /tmp/krimp-s1b/fleet-status.txt, /tmp/ghash-lr/s2-determination.txt.
