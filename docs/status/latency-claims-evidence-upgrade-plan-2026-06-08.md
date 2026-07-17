# Latency Claims Evidence Upgrade Plan

Date: 2026-06-08 UTC
Status: L1/L2 evidence sprint complete; L3-L5 follow-on
Owner: latency benchmark worker / tmux injector
Related article: `$POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md`

## Execution State

Implementation started 2026-06-08 UTC. The L1/L2 evidence sprint completed on
2026-06-09 UTC.

New tooling added:

- `scripts/build-rippled-timing-profiles`
  - patches `rippled/src/xrpld/consensus/ConsensusParms.h`;
  - builds one binary per timing profile;
  - writes source diffs, profile constants, binary hashes, and a manifest;
  - restores the stock source and rebuilds `.build/rippled` after profile
    builds.
- `scripts/xrpl-timing-stability-matrix`
  - builds or consumes timing-profile binaries;
  - runs the stock and reduced-timing private `rippled` matrix;
  - publishes raw reports, aggregate stats, methodology, classifications, and
    `SHA256SUMS.txt`.
- `scripts/xrpl-private-control-benchmark`
  - extended to `xrpl-private-control-benchmark-v2`;
  - records per-round submit-node server snapshots, ledger sequence before and
    after validation, ledgers crossed while waiting, polling attempts/errors,
    and parsed consensus-log telemetry.

Smoke checks completed:

- `scripts/build-rippled-timing-profiles` successfully built `close_1000ms`,
  wrote a manifest, restored source, and rebuilt stock.
- `scripts/xrpl-timing-stability-matrix` completed a 1-session, 3-round smoke
  matrix for `stock` and `close_1000ms` and verified `SHA256SUMS.txt`.

Completed timing-matrix run:

```text
RUN_ID=xrpl-timing-stability-20260608T152301Z
SESSIONS=5
ROUNDS=1000
VALIDATORS=6
PROFILES=stock,close_1500ms,close_1000ms,close_750ms,close_500ms,close_250ms
BUILD_PROFILES=1
```

Public packet target:

```text
$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z/
```

Hash verification:

```text
cd $POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z
sha256sum -c SHA256SUMS.txt
```

Result: all packet entries verified.

Private work root:

```text
$POSTFIAT_REPO/reports/xrpl-private-timing-stability-matrix/xrpl-timing-stability-20260608T152301Z/
```

Completed selected rerun:

```text
RUN_ID=postfiat-selected-xrpl-v4-20260609T052252Z
PUBLIC_PACKET=$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/
LANES=postfiat_full_vote_current,postfiat_quorum_fast_current,xrpl_stock,xrpl_tuned_selected,xrpl_aggressive_250ms
SESSIONS=5
ROUNDS=1000
VALIDATORS=6
```

Hash verification:

```text
cd $POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z
sha256sum -c SHA256SUMS.txt
```

Result: all packet entries verified.

Final article score:

```text
round=round-20260609T140603Z
document_sha256=4fa4b0b546ffbb2cfcd8e5073110481c2936d564127d88a8c4c2aa62be18cd8d
openai chat-latest: avg=88.00, runs=5, range=87-89
opus: avg=87.20, runs=5, range=87-88
deepseek: avg=94.00, runs=5, range=92-95
overall_avg=89.73
game_state=needs-new-evidence
recommended_modality=execution-sprint
```

The scoring harness recommendation is consistent with this document's claim
ladder: L1/L2 are complete, while more interesting public claims require L3-L5
evidence packets.

## Objective

Upgrade the current Post Fiat L1 v2 latency article from a narrow local
benchmark result into a stronger, more interesting performance-engineering
claim without overstating what the evidence supports.

The immediate issue this sprint addressed was the private `rippled` control.
The earlier matched packet was useful, but it only had two XRPL operating
points:

| Lane | What it tells us | What it does not tell us |
|---|---|---|
| Stock private `rippled` | default XRPL-style timing is stable and lands near the normal ledger-close cadence | whether XRPL can be tuned into a stable subsecond private network |
| Fast-timing private `rippled` (`ledger250ms`) | aggressive timer compression can reduce p50 but produced severe p95/p99 instability in our run | where the stable low-latency boundary actually is |

To make more interesting claims, we needed to map the stability envelope, not
just compare Post Fiat against stock and one aggressive detuned XRPL binary.
That L1/L2 work is now complete. The next unresolved claim levels are load,
multi-host shape, and broader peer positioning.

## Pre-Sprint Claim Ceiling

Before the timing matrix and selected rerun, the evidence supported this:

```text
In a matched local private six-validator benchmark, Post Fiat L1 v2 finalized
real signed native transfers materially faster at p50 than stock private
`rippled` and a fast-timing private `rippled` control. The fast-timing `rippled`
lane improved median latency but showed severe tail instability in this packet.
```

That earlier evidence did not support these claims:

- Post Fiat is faster than optimized XRPL.
- XRPL cannot run stably below a specific ledger cadence.
- XRPL's public network has 15-20 second tails.
- Post Fiat's p95/p99 ratios against fast-timing `rippled` are clean headline
  multipliers.
- Post Fiat mainnet will have the same latency as the local controlled packet.

## Claim Ladder

Each stronger public claim requires a specific evidence packet.

| Claim level | Public claim unlocked | Required evidence |
|---|---|---|
| L0: current | Post Fiat has sub-100ms local controlled finality at p50; stock XRPL-style timing is much slower locally | existing matched local packet |
| L1: XRPL tuning envelope | Aggressively reducing XRPL ledger timing creates a median/tail tradeoff; stable subsecond XRPL requires a narrower timing envelope if it exists | XRPL timing matrix with logs and stability telemetry |
| L2: best-effort tuned XRPL comparison | Post Fiat beats the best stable private `rippled` profile we found under this host/topology/workload | timing matrix plus selected-best stable profile rerun |
| L3: topology robustness | Post Fiat remains lower-latency than tuned XRPL across local, same-region multi-host, and degraded-validator conditions | multi-host matrix plus validator lag/failure runs |
| L4: application throughput | Post Fiat keeps transaction completion inside an application budget under realistic load, not just sequential sends | concurrency/load matrix with p50/p95/p99 and failure rates |
| L5: architectural positioning | Post Fiat is faster than XRPL-style ledger close, slower than Sui's best local object fast path, and positioned as an XRPL-adjacent low-latency account chain | peer packet with XRPL, Sui, Avalanche, and clearly separated semantics |

This sprint completed L1 and L2. L3-L5 are follow-on work.

## Required Evidence Packet 1: XRPL Timing Stability Matrix

Create a public packet under:

```text
postfiatorg.github.io/static/benchmarks/xrpl-private-timing-stability-matrix-<RUN_ID>/
```

Private working directory:

```text
postfiatl1v2/reports/xrpl-private-timing-stability-matrix/<RUN_ID>/
```

### Matrix

Run private six-validator `rippled` with the same workload and topology across
multiple timing profiles.

| Profile | Purpose | Sessions | Rounds/session |
|---|---|---:|---:|
| `stock` | baseline stable XRPL-style ledger timing | 5 | 1000 |
| `close_1500ms` | conservative compression | 5 | 1000 |
| `close_1000ms` | likely subsecond boundary candidate | 5 | 1000 |
| `close_750ms` | aggressive but not extreme | 5 | 1000 |
| `close_500ms` | very aggressive | 5 | 1000 |
| `close_250ms` | current fast-timing stress point | 5 | 1000 |

The exact constants must be recorded from the patched `rippled` binary or build
diff, not inferred from filename. If the build only changes one constant, record
that. If it changes multiple consensus/ledger timers, record every changed
constant and the source diff hash.

### Required Per-Round Metrics

Each XRPL round must record:

- `submit_to_validated_ms`;
- transaction hash;
- validated ledger index;
- ledger index before submit;
- ledger index after validation;
- number of ledgers crossed while waiting;
- RPC peer count before and after;
- server state before and after;
- validated ledger age before and after;
- error/retry count;
- whether `tx` returned `validated: true`;
- UTC start and stop timestamps.

### Required Log Telemetry

For each `rippled` node, parse logs into a per-session summary:

- count of consensus pauses;
- count of `No close time consensus`;
- count of max-consensus or near-max-consensus rounds;
- count of laggard/offline validator messages;
- count of ancestor/acquisition failures;
- final validated ledger sequence per validator;
- final peer count per validator;
- largest validator ledger-sequence gap;
- any validator that ends behind the majority;
- first round where instability appears.

The current fast-timing packet showed this category of failure. Future packets
must quantify it rather than leaving it as a narrative read of logs.

### Stability Classification

Classify each profile after the run:

| Classification | Criteria |
|---|---|
| `stable` | all sessions complete; p99 < 2x p50 or p99 < 2s; no validator ends materially behind; low/no consensus pause counts |
| `strained` | all sessions complete; p50 useful; p95/p99 show repeated stalls; logs show non-trivial consensus disagreement |
| `unstable` | repeated >10s tails, validator lag, max-consensus behavior, or incomplete sessions |

The exact threshold can be adjusted before execution, but it must be frozen in
the packet before results are interpreted.

## Required Evidence Packet 2: Best Stable XRPL Rerun

After the timing matrix, select the fastest `stable` or least-bad `strained`
profile and rerun it as a clean control against Post Fiat.

Create:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-best-stable-xrpl-latency-<RUN_ID>/
```

Matrix:

| Lane | Sessions | Rounds/session |
|---|---:|---:|
| Post Fiat L1 v2 full-vote current | 5 | 1000 |
| Post Fiat L1 v2 quorum-fast current | 5 | 1000 |
| Stock private `rippled` | 5 | 1000 |
| Best stable tuned private `rippled` | 5 | 1000 |
| Current `ledger250ms` fast-timing `rippled` | 5 | 1000 |

The article headline should compare Post Fiat full-vote to stock and best stable
tuned `rippled`. The `ledger250ms` lane should be shown as an instability
stress case, not as the optimized XRPL control.

## Required Evidence Packet 3: Load And Concurrency Matrix

Sequential transactions answer "how fast can one user observe completion?"
They do not answer "what happens under load?"

Create:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-load-latency-matrix-<RUN_ID>/
```

Matrix:

| Lane | Concurrency | Transactions | Sessions |
|---|---:|---:|---:|
| Post Fiat full-vote | 1 | 1000 | 3 |
| Post Fiat full-vote | 4 | 4000 | 3 |
| Post Fiat full-vote | 16 | 8000 | 3 |
| Post Fiat full-vote | 64 | 16000 | 3 |
| Post Fiat quorum-fast | 1 | 1000 | 3 |
| Post Fiat quorum-fast | 4 | 4000 | 3 |
| Post Fiat quorum-fast | 16 | 8000 | 3 |
| Post Fiat quorum-fast | 64 | 16000 | 3 |

Report:

- client-visible finality p50/p95/p99;
- throughput accepted/finalized per second;
- failure rate;
- RPC admission latency;
- mempool/admission queue depth if available;
- batch size distribution;
- validator CPU and memory if available.

This packet unlocks application-facing claims. Without it, the article should
stay focused on sequential local finality.

## Required Evidence Packet 4: Multi-Host / WAN Shape

Local one-host results are useful for protocol hot-path work, but public readers
will ask what survives network delay.

Create:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-multihost-latency-<RUN_ID>/
```

Minimum matrix:

| Topology | Validators | Sessions | Rounds/session |
|---|---:|---:|---:|
| one host / loopback | 6 | 3 | 1000 |
| same-region multi-VM | 6 | 3 | 1000 |
| two-region split | 6 | 3 | 1000 |
| one validator delayed | 6 | 3 | 1000 |
| one validator down | 6 | 3 | 1000 |

Report p50/p95/p99 plus finality safety checks. This unlocks statements about
deployment sensitivity, not mainnet guarantees.

## Required Evidence Packet 5: Peer Architecture Calibration

The current peer packet with Avalanche and Sui is useful because it prevents
XRPL from doing all the rhetorical work. Expand it only after L1/L2 above.

Required lanes:

| Lane | Required distinction |
|---|---|
| Sui owned-object transfer | local returned-effects fast path, not equivalent to full shared-state consensus |
| Sui shared-object call | closer to consensus path, still different semantics |
| Avalanche local C-Chain transfer | EVM-style local completion surface |
| Post Fiat full-vote | account/balance certified finality |
| Best stable tuned private `rippled` | XRPL-style private validated ledger |

Allowed claim:

```text
Post Fiat is not the fastest possible local chain architecture. Sui's local
object path is much lower at p50 in our peer packet. Post Fiat's claim is an
XRPL-adjacent account/balance finality path that is materially faster than the
XRPL-style controls we measured while retaining a simpler account-chain shape.
```

## Article Changes After L1/L2

Once the XRPL timing matrix and best-stable rerun exist, rewrite the core result
section around these claims:

1. Stock XRPL-style private timing is stable but intentionally slow.
2. Timer compression is not free: it improves median but can create consensus
   tail instability.
3. The fastest stable tuned `rippled` profile we found is the fair XRPL control.
4. Post Fiat full-vote is compared against that fair control.
5. The `ledger250ms` lane is treated as a stress case, not a headline win.

Do not headline fast-timing p95/p99 ratios unless the selected tuned profile is
classified stable by the predeclared criteria.

## No-Go Claims

Do not put these in the blog unless later evidence directly supports them:

- "Post Fiat beats XRPL mainnet."
- "XRPL is slow because it is poorly written."
- "XRPL cannot be optimized."
- "The `ledger250ms` p99 blowout is inherent to all XRPL configurations."
- "Local loopback p50 predicts public mainnet p50."
- "Post Fiat beats Sui."
- "Post Fiat has solved high-throughput performance" before a load matrix.

## Execution Checklist

1. Add or extend a `rippled` timing-profile build script.
2. Emit source diffs and binary hashes for every timing profile.
3. Extend `scripts/xrpl-private-control-benchmark` to capture per-round
   server-info snapshots and richer ledger-gap data.
4. Add a log parser for consensus pauses, laggards, close-time disagreement,
   ancestor/acquisition failures, and validator ledger drift.
5. Run the XRPL timing stability matrix.
6. Freeze stability classifications before selecting a best stable control.
7. Run the best-stable Post Fiat/XRPL rerun.
8. Hash and publish sanitized packets.
9. Update the article only with claims unlocked by the packets.
10. Score the updated article with the dedicated text-improvement harness.

## Done Criteria

This work is done only when:

- the XRPL timing matrix exists as a public hash-bound packet;
- the best stable tuned `rippled` profile is identified or the absence of one is
  explicitly documented;
- Post Fiat is rerun against that selected control in a matched packet;
- the article no longer treats the `ledger250ms` tail blowout as a clean
  optimized-XRPL comparison;
- the public claim text maps directly to packet evidence;
- the scoring harness score is recorded against the exact article file that
  would be promoted.

## Completion Audit: 2026-06-09 UTC

| Done criterion | Current evidence | Status |
|---|---|---|
| XRPL timing matrix exists as a public hash-bound packet. | `$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z/`; `sha256sum -c SHA256SUMS.txt` verified all entries. | complete |
| Best stable tuned `rippled` profile is identified or absence is documented. | Matrix `aggregate.md` states `Fastest stable profile: none`; `close_750ms` is the selected non-unstable strained control; `close_250ms` is unstable stress evidence. | complete |
| Post Fiat is rerun against the selected control in a matched packet. | `$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/`; lanes include Post Fiat full-vote, Post Fiat quorum-fast, stock `rippled`, selected `close_750ms`, and aggressive `close_250ms`; `sha256sum -c SHA256SUMS.txt` verified all entries. | complete |
| Article no longer treats the `ledger250ms` tail blowout as a clean optimized-XRPL comparison. | Article labels `close_250ms` as an aggressive stress lane and says it is not the optimized XRPL control. The selected comparison is `close_750ms`, labeled `strained`. | complete |
| Public claim text maps directly to packet evidence. | Article numbers match selected rerun `aggregate.md` and matrix `aggregate.md`; article links both packets, safety summary, methodology, manifest, raw reports, and hash manifest. | complete |
| Scoring harness score is recorded against the exact article file that would be promoted. | TIH round `round-20260609T140603Z`; document hash `4fa4b0b546ffbb2cfcd8e5073110481c2936d564127d88a8c4c2aa62be18cd8d`; overall average `89.73`; model averages recorded above. | complete |

Residual work is intentionally outside this sprint's done criteria:

- load/concurrency matrix for throughput claims;
- multi-host/WAN-shaped matrix for deployment-shape claims;
- peer calibration expansion for broader BFT/object-system positioning;
- structural owned-value or certificate-first fast-path prototype for materially
  lower latency targets.
