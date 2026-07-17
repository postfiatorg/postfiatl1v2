# Interesting Latency Claims: Requirements Spec

Date: 2026-06-09 UTC
Status: current requirements
Related draft: `$PASTEDOCS_ROOT/perf_write.md`
Related evidence plan: `docs/status/latency-claims-evidence-upgrade-plan-2026-06-08.md`

## Goal

Make the latency article about an architecture result, not a one-off stopwatch
result.

The public claim should become:

```text
Post Fiat L1 v2 is an XRPL-adjacent account chain whose local certified-finality
path is materially faster than private XRPL-style ledger-close controls under
matched native-transfer workloads, while preserving an explicit local safety
gate. The next question is not whether one local run was fast, but where the
XRPL timing envelope breaks, where Post Fiat's current account lane sits among
modern BFT systems, and what structural changes would be required for still
lower latency.
```

The article should only claim what the packets prove. The job now is to move
from a narrow local XRPL comparison to more interesting architecture claims:
load behavior, multi-host behavior, peer positioning, and what structural work
would be required for materially lower latency.

## Current Claim Ceiling

The current evidence supports:

```text
In a local six-validator private benchmark, Post Fiat L1 v2 completed real
signed native transfers faster at p50 than stock private `rippled` and faster
than the matrix-selected reduced-timing private `rippled` profile. The
aggressive 250ms `rippled` stress lane lowered median latency but showed severe
tail instability in this topology.
```

Current packet-backed numbers:

| Lane | p50 ms | p95 ms | p99 ms | Mean ms | Status |
|---|---:|---:|---:|---:|---|
| Post Fiat full-vote current | 88.083 | 104.705 | 110.593 | 87.937 | local certified finality |
| Post Fiat quorum-fast current | 83.936 | 100.362 | 106.561 | 83.188 | local certified finality |
| Private `rippled` stock | 3000.565 | 3054.779 | 6010.940 | 3121.950 | stock control |
| Private `rippled` `close_750ms` | 883.507 | 984.493 | 1818.378 | 941.539 | matrix-selected strained control |
| Private `rippled` `close_250ms` | 573.367 | 15345.814 | 19918.846 | 1573.954 | aggressive stress lane |

The current evidence does not support:

- Post Fiat is faster than optimized XRPL.
- Public XRPL mainnet has the same tail behavior as the private stress lane.
- Post Fiat mainnet will have the same latency as the local benchmark.
- Post Fiat is faster than Sui, FastPay, HotStuff, Tendermint, or other BFT
  peers.
- The aggressive `rippled` timing profile is a fair optimized-XRPL baseline.
- A throughput claim under concurrent user load.
- A WAN or public-testnet latency claim.

## Target Claims To Earn

These are the claims that would make the article more interesting. They are
ordered from nearest-term to structurally ambitious.

| Claim | Why it matters | Status / evidence needed before publication |
|---|---|---|
| Post Fiat beats stock private `rippled` on real native-transfer completion latency. | Establishes the basic result in terms a reader understands: signed transfers finalized faster. | Earned by the v4 selected matched packet. |
| Post Fiat beats the fastest private `rippled` profile selected by the timing matrix, with the profile honestly labeled. | Moves the comparison from a weak stock baseline to a fairer local XRPL envelope. | Earned as a local result against `close_750ms`, which must be labeled `strained`, not stable or optimized. |
| The aggressive 250ms `rippled` lane improves median latency but creates unacceptable tail behavior in this topology. | Turns the old "tail crash" from a confusing artifact into a real engineering finding about timer compression. | Earned as a stress-lane result, not as a claim about public XRPL or optimized XRPL. |
| Post Fiat's fast path still passes local adversarial finality gates. | Separates "fast because unsafe" from "fast with a tested certified-finality boundary." | Partly earned by the local adversarial finality gate. Needs broader fuzzing/multi-host evidence for production-strength claims. |
| The current Post Fiat account lane is not globally fastest; it is faster than XRPL-style ledger close but slower than object/certificate-first local fast paths. | Makes the article credible by placing the result against the right architectural peers. | Peer calibration packet for Sui/Avalanche/FastPay-style references, with semantic labels. |
| The next major latency gain requires a structural fast path rather than another timer tweak. | Creates a compelling roadmap instead of pretending constant tuning is the endgame. | Load matrix, profile data, and a design packet for an owned-value or certificate-first transfer lane. |
| Post Fiat can hit an application-facing latency budget under concurrency. | Moves from benchmark storytelling to product relevance. | Load matrix with concurrency, finalized tx/s, failures, p95/p99, queue depth, and validator resource metrics. |
| The result survives multi-host deployment. | Converts local evidence into controlled-testnet evidence. | Same-region and WAN-shaped runs with validator lag, restart, and outage cases. |

## Claim Ladder

| Level | Public claim unlocked | Status |
|---|---|---|
| L0: current local result | Post Fiat has low local certified-finality latency and beats stock private `rippled` in the existing matched packet. | Earned. |
| L1: XRPL timing envelope | Private `rippled` can be pushed faster at the median, but below some timing envelope the tails and validator alignment degrade. | Earned locally by the timing matrix, with scope limited to this topology. |
| L2: selected XRPL comparison | Post Fiat beats the matrix-selected private `rippled` control under the same host/topology/workload, while stating that the selected control was `strained`. | Earned locally by the selected v4 rerun. |
| L3: load behavior | Post Fiat stays inside an application-facing latency budget under concurrent transfers, not just sequential sends. | Load matrix with concurrency, finalized tx/s, failed tx/s, p50/p95/p99, RPC admission latency, and validator resource metrics. |
| L4: network shape | The result survives beyond one local host under same-region or WAN-like latency. | Multi-host matrix with controlled latency injection and validator outage/degradation cases. |
| L5: peer positioning | Post Fiat is faster than XRPL-style ledger close in these packets, slower than object/owned-value fast paths, and currently sits as an XRPL-adjacent certified-finality account chain. | Peer packet against Sui/Avalanche/Canton/FastPay-style references with semantics separated. |
| L6: next architecture claim | The biggest further latency improvement likely requires a structural fast path, not another constant tweak. | Design and prototype evidence for owned-value or certificate-first transfer lane, plus adversarial safety tests. |

The current article can target L0-L2. The next evidence sprint should target
L3-L5. L6 is the next architecture sprint.

## Completed Evidence

| Packet | Location | What it proves |
|---|---|---|
| XRPL timing stability matrix | `$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z` | In this local six-validator setup, no reduced `rippled` timing profile was cleanly stable. `close_750ms` was the selected strained control; `close_250ms` was the aggressive unstable stress lane. |
| Selected matched v4 rerun | `$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z` | Post Fiat full-vote and quorum-fast lanes beat stock private `rippled`, selected `close_750ms`, and aggressive `close_250ms` on p50/p95/p99/mean in the five-session local packet. |
| Local adversarial finality gate | `$POSTFIAT_REPO/docs/status/adversarial-finality-gate-2026-06-06.md` | The fast path has passed a local adversarial finality gate, which supports "not fast because safety was simply removed." It does not by itself prove production Byzantine completeness. |

All public claims must link the public copies of these artifacts and preserve
their scope: local, private, six-validator, native-transfer workload.

## Evidence Packet 1: XRPL Timing Stability Matrix

Status: complete for the current local article. Keep this section as the rerun
requirement if the timing matrix is expanded or repeated on another topology.

Purpose:

```text
Find the fastest private `rippled` timing profile that remains stable under the
same local six-validator native-transfer workload.
```

Required profiles:

| Profile | Role |
|---|---|
| `stock` | baseline XRPL-style private ledger timing |
| `close_1500ms` | conservative compression |
| `close_1000ms` | likely boundary candidate |
| `close_750ms` | aggressive compression |
| `close_500ms` | very aggressive compression |
| `close_250ms` | stress lane matching the earlier aggressive control |

Minimum run:

```text
validators: 6
sessions: 5
rounds/session: 1000
workload: real signed native transfers
```

Required outputs:

- raw per-session JSON;
- aggregate p50, p95, p99, max, mean;
- complete binary hashes for every `rippled` profile;
- source diff and changed constants for every patched binary;
- validator log telemetry;
- final validated-ledger sequence per validator;
- classification for each profile: `stable`, `strained`, or `unstable`;
- `SHA256SUMS.txt`;
- one-page `README.md` explaining what the packet proves.

Stable profile criteria must be frozen before interpreting results. The packet
must expose the selection mechanically, for example:

```text
fastest_stable_profile: none
selected_profile: close_750ms
selected_profile_classification: strained
```

If no reduced-timing profile is stable, the article must say that directly.

## Evidence Packet 2: Selected XRPL Rerun

Status: complete for the current local article. Keep this section as the rerun
requirement if the selected profile changes or the benchmark moves to another
topology.

Purpose:

```text
Compare Post Fiat against the best private `rippled` control selected by the
matrix instead of against a hand-picked aggressive stress lane.
```

Required lanes:

| Lane | Sessions | Rounds/session |
|---|---:|---:|
| Post Fiat L1 v2 full-vote current | 5 | 1000 |
| Post Fiat L1 v2 quorum-fast current | 5 | 1000 |
| Stock private `rippled` | 5 | 1000 |
| Matrix-selected tuned private `rippled` | 5 | 1000 |
| Aggressive `close_250ms` private `rippled` stress lane | 5 | 1000 |

Required article treatment:

- headline comparison uses Post Fiat vs stock and Post Fiat vs matrix-selected
  tuned `rippled`;
- `close_250ms` appears only as an aggressive stress lane;
- if the selected tuned profile is `strained`, the article says `strained` in
  the table label;
- p95/p99 ratios are not used as headline ratios unless the baseline is stable.

## Evidence Packet 3: Safety Gate

Purpose:

```text
Show the fast path is not merely fast because safety checks were skipped.
```

Required evidence:

- local adversarial finality test document linked from the article;
- explicit coverage table for:
  - equivocation;
  - duplicate votes;
  - wrong-height votes;
  - stale votes;
  - missing validators;
  - insufficient quorum;
  - conflicting certificates;
  - restart behavior;
  - validator lag;
  - malformed certificate input.

The article can say:

```text
The current fast path has passed a local adversarial finality gate.
```

The article should not say:

```text
The current fast path is production-Byzantine-complete.
```

unless broader simulation, fuzzing, restart, and multi-host evidence exists.

## Evidence Packet 4: Load Matrix

Purpose:

```text
Move from single-user completion latency to application-facing throughput and
tail behavior.
```

Required matrix:

| Lane | Concurrency | Transactions/session | Sessions |
|---|---:|---:|---:|
| Post Fiat full-vote | 1 | 1000 | 3 |
| Post Fiat full-vote | 4 | 4000 | 3 |
| Post Fiat full-vote | 16 | 8000 | 3 |
| Post Fiat full-vote | 64 | 16000 | 3 |
| Post Fiat quorum-fast | 1 | 1000 | 3 |
| Post Fiat quorum-fast | 4 | 4000 | 3 |
| Post Fiat quorum-fast | 16 | 8000 | 3 |
| Post Fiat quorum-fast | 64 | 16000 | 3 |

Required metrics:

- finalized tx/s;
- accepted tx/s;
- failed tx/s;
- p50/p95/p99 completion latency;
- RPC admission latency;
- mempool or admission queue depth if available;
- validator CPU and memory;
- state size before and after;
- storage write latency if available.

This unlocks claims about application performance. Without it, the public
article should stay focused on sequential completion latency.

## Evidence Packet 5: Multi-Host And WAN Shape

Purpose:

```text
Separate local protocol hot-path speed from real network behavior.
```

Required environments:

- single host loopback;
- same-region multi-VM;
- injected RTT matrix, for example 5ms, 25ms, 75ms, 150ms;
- one-validator-lag case;
- one-validator-restart case;
- one-validator-offline case.

Required outputs:

- p50/p95/p99 by environment;
- finality failures;
- retry count;
- validator alignment;
- certificate size;
- bandwidth per finalized transfer;
- clear statement of which path remains valid under each failure mode.

This is required before making mainnet-style latency claims.

## Evidence Packet 6: Peer Calibration

Purpose:

```text
Prevent the article from sounding like Post Fiat is globally fastest when the
real claim is narrower and more interesting.
```

Required comparisons:

- Sui owned-object or local equivalent, labeled as an object fast path;
- Avalanche local/private chain, labeled by its exact consensus/finality path;
- XRPL/rippled private ledger close, labeled as account-ledger-close;
- Post Fiat account certified finality.

Required rule:

```text
No peer number appears in a headline table unless the article says what was
actually measured and why it is or is not semantically comparable.
```

Expected article conclusion:

```text
Post Fiat's current account lane is meaningfully faster than XRPL-style private
ledger close in these packets, but object/certificate-first systems define a
lower-latency design target. That points to the next architecture sprint.
```

## Article Requirements After L1/L2 Evidence

The article should be rewritten around outcomes readers understand:

1. What was measured.
2. What Post Fiat achieved.
3. What the XRPL timing matrix found.
4. Which `rippled` profile is the fairest tuned local control.
5. What happened to the aggressive 250ms stress lane.
6. Why the safety gate matters.
7. What the result does and does not say about mainnet.
8. What further work would make the claims stronger.

Avoid internal process language. Public readers do not care how many attempts
were made unless the attempt history changes the evidence.

## Forbidden Public Claims Until The Packet Exists

Do not publish:

```text
Post Fiat is faster than optimized XRPL.
```

Allowed only after a timing matrix selects a stable tuned `rippled` profile and
the selected rerun confirms it.

Do not publish:

```text
XRPL has unstable tails.
```

Allowed only as:

```text
The aggressive reduced-timing private `rippled` profile in this local packet
showed unstable tails.
```

Do not publish:

```text
Post Fiat is faster than Sui.
```

The current peer evidence points the other direction for local object-style
fast paths.

Do not publish:

```text
Post Fiat mainnet finality is 85ms.
```

Allowed only as:

```text
The local six-validator packet measured roughly sub-100ms p50 completion on
this controlled setup.
```

## Done Definition

The article is ready for stronger claims when all of these are true:

- XRPL timing stability matrix packet exists and passes `sha256sum -c`.
- The fastest stable or least-bad strained `rippled` profile is selected from
  the matrix, not manually chosen.
- The selected XRPL rerun packet exists and passes `sha256sum -c`.
- The article links the exact public packets.
- Every headline number can be traced to a public JSON artifact.
- The article labels stress lanes, tuned lanes, and peer calibration lanes
  correctly.
- The safety gate is linked and its coverage is not overstated.
- The exact final article is scored with the dedicated TIH scoring harness.
- No lower-scoring rewrite is promoted over a higher-scoring live article
  without explicit user approval.
