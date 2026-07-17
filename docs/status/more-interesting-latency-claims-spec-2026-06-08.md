# More Interesting Latency Claims: Evidence Spec

Date: 2026-06-08 UTC
Status: execution spec
Related draft: `$PASTEDOCS_ROOT/perf_write.md`
Related plan: `docs/status/latency-claims-evidence-upgrade-plan-2026-06-08.md`

## Purpose

The current latency article is only valuable if the public claim is stronger
than "we ran one local benchmark and got a low p50."

The article should become a claims-backed performance note about where Post
Fiat L1 v2 sits in the design space:

```text
Post Fiat L1 v2 is an XRPL-adjacent account chain whose current local
certified-finality path is materially faster than private XRPL-style ledger
close under matched native-transfer workloads, while still preserving an
explicit safety gate and leaving open the harder question of object-style
fast paths.
```

That claim is interesting because it connects performance to architecture. It
is not just a stopwatch result.

## Current Claim Ceiling

The existing public evidence supports this narrow statement:

```text
In a matched local private six-validator benchmark, Post Fiat L1 v2 finalized
real signed native transfers materially faster at p50 than stock private
`rippled` and faster than an aggressively reduced-timing private `rippled`
control, while the aggressive `rippled` lane showed severe tail instability in
that packet.
```

The evidence does not yet support:

- Post Fiat is faster than optimized XRPL.
- XRPL cannot be tuned into a stable subsecond private network.
- Post Fiat is faster than modern BFT/object chains generally.
- Post Fiat mainnet will have the same latency as the local packet.
- The aggressive `rippled` tail ratios are a clean optimized-XRPL comparison.

## Interesting Claims To Unlock

### Claim 1: XRPL Timing Envelope

Public wording after evidence:

```text
Private `rippled` can be made much faster at the median by compressing ledger
timers, but below a certain timing envelope the tails and validator alignment
degrade sharply in our six-validator local benchmark.
```

Evidence required:

- Run stock and reduced-timing private `rippled` profiles:
  - `stock`
  - `close_1500ms`
  - `close_1000ms`
  - `close_750ms`
  - `close_500ms`
  - `close_250ms`
- Use at least `5` sessions and `1000` real signed transfers per session.
- Publish raw JSON, logs, exact binary hashes, timing diffs, aggregate stats,
  classification rules, and `SHA256SUMS.txt`.
- Classify each profile as `stable`, `strained`, or `unstable` before using it
  in the article.

Status:

```text
In progress. Active run:
xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z
```

Tooling:

```text
scripts/xrpl-timing-stability-matrix
```

The matrix aggregate must expose `fastest_stable_profile`. The follow-on
selected comparison should consume that value directly rather than choosing a
profile by narrative preference.

### Claim 2: Best Stable XRPL Comparison

Public wording after evidence:

```text
Against the fastest stable private `rippled` profile we found under this
topology and workload, Post Fiat L1 v2 still finalized native transfers faster
at p50, with a tighter client-visible completion path.
```

Evidence required:

- Select the fastest `stable` profile from the XRPL timing matrix.
- If no reduced-timing profile is `stable`, say that explicitly and select the
  least-bad `strained` profile only as a stress/control lane.
- Rerun the matched comparison:
  - Post Fiat L1 v2 full-vote current
  - Post Fiat L1 v2 quorum-fast current
  - stock private `rippled`
  - fastest stable private `rippled`, or clearly labeled least-bad strained
    profile
  - `close_250ms` as an instability stress lane, not as optimized XRPL
- Publish the packet with raw reports, scripts, commands, hashes, and a
  one-page methodology.

This is the minimum packet needed for a more interesting XRPL-facing article.

Tooling:

```text
scripts/postfiat-xrpl-latency-evidence-v4
```

Required invocation inputs:

```text
TUNED_BIN=<binary from timing matrix manifest>
TUNED_PROFILE=<fastest_stable_profile or explicit least-bad strained profile>
TUNED_CLASSIFICATION=<stable|strained>
TUNED_SOURCE_MATRIX=<public timing matrix packet path or URL>
AGGRESSIVE_BIN=<close_250ms binary>
AGGRESSIVE_PROFILE=close_250ms
```

Generate those inputs from the timing matrix packet with:

```text
scripts/select-xrpl-tuned-profile <timing-matrix-packet-dir>
```

The v4 packet builder separates `xrpl_tuned_selected` from
`xrpl_aggressive_250ms`, so the article can compare against the selected tuned
profile while retaining `close_250ms` only as a stress lane.

### Claim 3: Performance Is Not Coming From Skipping Safety

Public wording after evidence:

```text
The fast Post Fiat path has passed local adversarial finality checks, so the
result is not merely "fast because safety was removed."
```

Evidence required:

- Link the adversarial finality gate document.
- Confirm it covers equivocation, stale/duplicate votes, wrong-height votes,
  missing validators, insufficient quorum, conflicting certificates, and
  restart behavior.
- If any gate is missing, state that as follow-up and do not imply complete
  Byzantine coverage.

Existing anchor:

```text
postfiatl1v2/docs/status/adversarial-finality-gate-2026-06-06.md
```

### Claim 4: Application-Facing Latency Budget

Public wording after evidence:

```text
Post Fiat's local chain-critical path fits comfortably inside a sub-2-second
application budget, leaving room for RPC, UI, network jitter, indexing, and
business logic.
```

Evidence required:

- Keep the HCI/application-budget explanation in the article.
- Separate local chain-critical-path time from end-user wall-clock time.
- Do not imply mainnet users will see local-lab latency.
- Add a load/concurrency matrix before making throughput claims:
  - concurrency `1`, `4`, `16`, `64`
  - p50/p95/p99 latency
  - finalized tx/s
  - failed tx/s
  - RPC admission latency
  - validator CPU/memory if available

### Claim 5: Peer Positioning

Public wording after evidence:

```text
Post Fiat is not the fastest local system in the comparison set: Sui's local
effects path is much lower at p50. The more precise claim is that Post Fiat is
much faster than the private XRPL-style controls in this packet family while
remaining an XRPL-adjacent account/balance chain.
```

Evidence required:

- Keep Sui and Avalanche lanes clearly labeled as peer calibration, not direct
  equivalence.
- Link the peer benchmark appendix.
- Avoid using Sui's `4 ms` local p50 as a negative or positive absolute claim
  unless the packet clearly states what was measured.
- Use peer results to sharpen architecture discussion:
  - Sui/object path: much lower local p50, different model.
  - XRPL/account ledger close: more conservative cadence, familiar lineage.
  - Post Fiat/account certified finality: current middle position.

## Article Structure After Evidence

The public article should follow this order:

1. State the result in one paragraph.
2. Say exactly what was measured.
3. Give the headline table.
4. Explain the XRPL timing envelope and show why `close_250ms` is a stress lane,
   not "optimized XRPL."
5. Compare against the fastest stable tuned `rippled` profile if one exists.
6. Explain why this matters for application latency budgets.
7. Calibrate against Sui/Avalanche without pretending those systems are the same
   kind of chain.
8. Explain the safety gate.
9. End with what evidence is still missing for mainnet-style claims.

The article should not narrate internal benchmark process. It should describe
results, interpretation, and limits.

## Forbidden Claims Until Proven

Do not publish these:

```text
Post Fiat is faster than optimized XRPL.
```

Allowed only after a stable tuned profile has been selected and rerun.

```text
XRPL has unstable tails.
```

Allowed only as:

```text
The aggressive reduced-timing private `rippled` profile in this local packet
showed unstable tails.
```

```text
Post Fiat is faster than Sui.
```

Not supported by the current peer packet.

```text
Post Fiat mainnet finality will be 85 ms.
```

Not supported. Local protocol latency is only one term in public deployment
latency.

```text
The benchmark proves public-network superiority.
```

Not supported. This is a controlled local/private benchmark.

## Evidence Packet Checklist

Every packet used by the article must contain:

- `README.md` with the exact claim supported;
- `methodology.md`;
- raw JSON reports;
- aggregate JSON and Markdown;
- command log or `commands.sh`;
- source commit hashes;
- binary hashes where binaries are benchmarked;
- environment summary;
- `SHA256SUMS.txt`;
- successful `sha256sum -c SHA256SUMS.txt` verification;
- a one-line caveat about what the packet does not prove.

## Scoring Gate

Before promotion:

- Score the exact article file with the dedicated text-improvement harness.
- Record GPT, Opus, DeepSeek, and overall averages.
- Do not promote a scored regression unless explicitly instructed.
- If a revision improves factual rigor but lowers score, keep it as a candidate
  and iterate rather than replacing the high-scoring article.

## Done Definition

This sprint is done when:

- the XRPL timing matrix finishes and is hash-verified;
- the fastest stable or least-bad strained XRPL profile is identified;
- the matched Post Fiat vs selected XRPL rerun is complete and hash-verified;
- the article has been rewritten around supported public claims;
- the exact final article has been scored;
- the article links to the public evidence packets;
- no live claim depends on an unrun benchmark or an unstated assumption.
