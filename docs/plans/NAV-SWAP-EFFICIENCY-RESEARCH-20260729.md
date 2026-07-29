# NAV Swap Efficiency Research: Radical Latency Reduction

**Date:** 2026-07-29
**Scope:** transparent and private A666/pfUSDC NAV swap round trips
(Ethereum USDC -> PFTL -> Ethereum), measured against the 25-minute
per-leg acceptance SLO.
**Inputs:** Phase 8 evidence timeline reconstruction
(`docs/evidence/a666-acceptance-20260728/phase-8-private-redeem-verify/`),
`docs/plans/private-egress-consensus-performance-plan.md`,
`docs/status/zk-prover-*.md`, `crates/privacy_orchard/`, orchestration
scripts, and current Ethereum consensus-layer status.

## Executive summary

The Phase 8 run took `1,848s` (issue) and `2,688s` (redemption) against
`1,500s` targets. Decomposing the run from evidence-file timestamps and
block timestamps shows the misses are **not dominated by Ethereum
finality**. They are dominated by:

1. **Cold Halo2 proving-key builds in one-shot CLI processes**
   (~`341s` measured `pk_build` cost, paid at least twice) — about
   **18 minutes** of the redemption leg;
2. **Serial six-validator checkpoint vote collection over ssh** — about
   **3.2 minutes per checkpoint round**, at least two rounds;
3. **SP1 Groth16 wrap latency** — about **3 minutes per proof**, twice;
4. **Fully serialized orchestration** — nothing overlaps the ~13-minute
   Ethereum finality wait; and
5. defect repair time (now fixed with regression guards).

The repo has already solved the *verifier-side* cold-start problem
(pinned VK artifacts, resident validators, `426ms` hot certified
rounds). The same treatment has **not** been applied to the
*prover-side* action creation path, and that is now the single largest
cost in the private leg.

**Conclusion: both legs can meet the existing 25-minute SLO without any
consensus or circuit redesign.** Tier 0 + Tier 1 below are projected to
bring issue to ~22 min (finality-bound) and redemption to ~13-15 min.
Tier 2 options go further and address the finality floor itself.

## Measured latency decomposition (Phase 8)

Timestamps are UTC from evidence file mtimes; anchors from Ethereum
block timestamps in `timing-summary.json`.

### Issue leg — deposit `17:29:35` to wA666 mint `18:00:23` (`1,848s`)

| Stage | Window | ~Cost | Class |
|---|---|---:|---|
| Ethereum 2-epoch finality wait + SP1 ingress witness/proof | 17:29 -> 17:46:28 | `1,010s` | chain floor + prover |
| pfUSDC claim relay + PFTL rounds (incl. policy-pin defect retry) | 17:46 -> 17:52:40 | `370s` | orchestration + defect |
| A666 primary issue + export | 17:52 -> 17:55:28 | `170s` | PFTL ops |
| Export SP1 Groth16 (CUDA) | 17:55:44 -> 17:58:54 | `190s` | prover |
| Ethereum preflight + mint tx | 17:59 -> 18:00:29 | `90s` | Ethereum tx |

### Redemption leg — burn `18:02:37` to USDC release `18:47:23` (`2,688s`)

| Stage | Window | ~Cost | Class |
|---|---|---:|---|
| Burn -> checkpoint capture | 18:02:37 -> 18:06:04 | `210s` | wait |
| Serial 6-validator checkpoint votes + certificate (return) | 18:06 -> 18:09:20 | `200s` | orchestration |
| Destination-consume repair (intervention; second serial vote round) | 18:09:44 -> 18:14:19 | `275s` | defect (fixed) |
| Return import round (h405) | 18:14:38 -> 18:15:47 | `70s` | consensus |
| Orchard ingress + round (h406) | 18:16:36 -> 18:17:45 | `70s` | consensus |
| **Private-primary-redeem action creation** | 18:17:45 -> 18:27:07 | **`560s`** | **cold prover** |
| Redeem round (h407) | 18:27:19 -> 18:28:10 | `50s` | consensus |
| **Private egress action creation** | 18:28:10 -> 18:36:44 | **`510s`** | **cold prover** |
| Egress round (h408) | 18:37:02 -> 18:38:10 | `70s` | consensus |
| Transparent burn-to-redeem + round (h409) | 18:38:35 -> 18:40:45 | `130s` | PFTL ops |
| Egress witness build | 18:42:20 -> 18:43:01 | `100s` | prover |
| SP1 Groth16 egress proof (CUDA) | 18:43:32 -> 18:46:27 | `210s` | prover |
| Ethereum withdrawal tx | -> 18:47:28 | `60s` | Ethereum tx |
| Settle round (h410, post-release) | 18:48 -> 18:49:22 | `85s` | consensus |

Key observations:

- The two private action creation gaps (`560s` + `510s` = **17.8
  minutes, ~40% of the whole `4,668s` round trip**) match the measured
  cold `pk_build_ms 341,879` baseline in
  `docs/status/zk-prover-optimization-results.md`, plus prove and note
  construction. The hot prove path is measured at **`5.8s`**. The cost
  is process-local: `ASSET_ORCHARD_*_PROVING_KEY` are `OnceLock` caches
  in `crates/privacy_orchard/src/asset_orchard_circuit.rs`, and the
  acceptance flow invokes `asset-orchard-*-create` as **one-shot CLI
  processes over ssh**, so every invocation starts cold.
- Warm consensus rounds are already fast (`50-70s` end to end,
  `426ms` hot certified round per the private-egress performance
  plan). The prior pinned-VK work paid off; consensus is not the
  bottleneck.
- Checkpoint vote collection is serial: validator votes land at
  `18:07:30, 18:07:52, 18:08:03, 18:08:22, 18:08:43, 18:08:54` —
  15-25s apart, sequential ssh round trips.
- The issue leg's `1,010s` head segment is mostly the unavoidable
  ~`780s` two-epoch Ethereum finality wait, but ~`230s` of witness and
  proof work is serialized *after* it instead of overlapped.

## External state of the art (checked 2026-07-29)

- Ethereum L1 finality remains two epochs (~13 minutes). 3SF /
  fast-finality designs remain research (leanConsensus prototypes with
  small validator sets) and are not deployed on mainnet. **Planning
  assumption: the ~13-minute finality floor per Ethereum-touching
  direction holds for the foreseeable acceptance window.**
- Consequence: a 25-minute leg containing one full finality wait has a
  ~12-minute budget for everything else. That is achievable with the
  fixes below; a leg containing *two* finality waits is not achievable
  and must not be designed in.

## Recommendations

### Tier 0 — orchestration only, no protocol change (days)

| # | Change | Est. saving |
|---|---|---:|
| 0.1 | **Resident prover on validator-2.** Extend the Phase 3 resident-coordinator pattern (`transport-peer-certified-private-egress-loop`) to *action creation*: a long-lived process that prewarms swap, private-egress, and private-primary-redeem proving keys before funding, then accepts create requests. Fallback: a no-op warm prove before deposit. | **~17 min** (redeem leg) |
| 0.2 | **Parallel checkpoint vote fan-out.** Collect the six governed checkpoint votes concurrently instead of serially. | ~3 min per checkpoint round (x2) |
| 0.3 | **Overlap the finality wait.** During the ~13-minute deposit/burn finality windows: prewarm SP1 CUDA provers, build witness scaffolding, pre-stage orchard ingress materials, run all preflights and policy/manifest hash validations. | ~3-5 min |
| 0.4 | **Persistent ssh multiplexing** (`ControlMaster`) and resident remote agents; eliminate per-stage process spawn and scp round trips. | ~1-2 min |
| 0.5 | **Back-to-back PFTL rounds.** h406/h407/h408 are individually fast but separated by prover gaps; with 0.1 they can run within ~3 minutes total. | (enabled by 0.1) |

### Tier 1 — prover engineering (1-2 weeks)

| # | Change | Est. saving |
|---|---|---:|
| 1.1 | **Pinned proving-key artifact.** Mirror the pinned-VK approach: serialize the PK assembly once per release (the `halo2_proofs 0.3.2` compatibility patch already exists for VK loading), embed or ship with the release, load fail-closed with fingerprint validation. Removes the cold build even for one-shot processes and de-risks the 0.1 daemon. | hardens 0.1 |
| 1.2 | **SP1 pipeline overlap.** Start the Groth16 wrap concurrently with Ethereum preflight; keep the CUDA prover resident and warm across the run (two wraps per round trip, ~3 min each, currently fully serialized). | ~2-3 min |
| 1.3 | **Continuous verifier checkpointing.** A background daemon keeps the Ethereum verifier's finalized-checkpoint view fresh so per-swap runs never select or prove a stale checkpoint at swap time (also eliminates the defect class behind A666-P8-002). | ~1-2 min + defect class |
| 1.4 | **Automated machine timestamps** at every stage boundary so future optimization is measured, not reconstructed from file mtimes. | measurement |

### Tier 2 — architectural (weeks+; only if sub-15-minute legs are required)

| # | Change | Effect |
|---|---|---|
| 2.1 | **Single-batch private redemption.** Orchard ingress, private-primary redeem, and private egress run as three consensus rounds with prover gaps between them. Combining them into one shielded batch (they already execute in the same peer-certified path) collapses three rounds and two gaps into one round. | -2 rounds, simpler resume semantics |
| 2.2 | **Risk-priced early acceptance of Ethereum deposits.** Accept justified (1-epoch, ~6.4 min) or safe-head checkpoints for amounts below a governed cap, with the issue spread explicitly priced as reorg insurance; require full 2-epoch finality above the cap. The governed six-validator checkpoint vote quorum already exists as the attestation layer to carry this policy. | finality floor 13 min -> ~7 min for small swaps |
| 2.3 | **Split the SLO by clock.** Report `chain_seconds` (Ethereum finality waits) and `system_seconds` (everything the stack controls) separately; gate the release on `system_seconds` plus a documented chain floor. | gate realism |
| 2.4 | **SP1 proof aggregation.** Aggregate export + destination-consume (and later egress) statements into one guest so the ~3-minute Groth16 wrap is paid once per leg instead of per statement. | ~3 min per leg |
| 2.5 | **Fewer Ethereum touches for pure NAV swaps.** A user who only wants private NAV redemption of PFTL-held A666 does not need the export/mint/burn/return excursion; offer the PFTL-only private path directly. The full excursion remains as the bridge acceptance flow. | product-level |

## Projected budgets after Tier 0 + Tier 1

| Leg | Today | Projected | SLO `1,500s` |
|---|---:|---:|---|
| Issue (deposit -> spendable wA666) | `1,848s` | ~`1,300-1,400s` | PASS, ~2 min margin |
| Redemption (burn -> USDC release) | `2,688s` | ~`800-1,000s` | PASS, ~8 min margin |

Issue-leg margin is thin because it is finality-bound; Tier 2.2 or 2.3
is the only way to create real headroom there.

## Suggested sequencing

1. Land Tier 0.1-0.4 and Tier 1.4 (instrumentation) before resuming the
   Phase 9 deposit lineage; these are orchestration-only and preserve
   the frozen-release requirement.
2. Land Tier 1.1 (pinned PK artifact) in the next release train with
   the same regression-gate treatment as the pinned VK
   (`scripts/private-egress-pinned-vk-regression-gate` precedent).
3. Decide Tier 2.2 vs 2.3 explicitly as the answer to the handoff's
   "change the gate or the architecture" question.
4. Treat Tier 2.1/2.4/2.5 as the follow-on program once the formal A8
   gate has passed.

## Safety constraints carried over

All optimizations must preserve the rules established in the
private-egress performance plan and the acceptance spec:

- independent validator re-execution before votes (no "proposer said
  it's valid" shortcuts);
- fail-closed pinned-artifact fingerprint validation;
- pre-mutation rejection semantics and replay protection unchanged;
- private material stays on validator-2 at mode `0600`, out of
  evidence; a resident prover daemon must hold note seeds and spending
  keys with the same discipline as the current one-shot flow.
