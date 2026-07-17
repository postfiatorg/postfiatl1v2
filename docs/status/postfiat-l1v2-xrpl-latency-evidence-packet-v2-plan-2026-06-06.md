# Post Fiat L1 v2 / XRPL Latency Evidence Packet v2 Plan

Date: 2026-06-06 UTC
Status: ready for execution
Owner: benchmark worker / tmux injector
Scope: matched private latency evidence for Post Fiat L1 v2 vs stock and fast-timing `rippled`

## Objective

Build a second-generation public evidence packet that can support a stronger version of the current latency article.

The target claim is narrow:

```text
In matched private XRPL-style controls, Post Fiat L1 v2 release full-vote
finality has lower client-visible local finality latency than stock and
fast-timing private rippled, and the result survives repeated sessions,
release binaries, hash-bound artifacts, and local adversarial finality gates.
```

This is not a public-mainnet claim. It is a controlled private benchmark claim.

## Why This Is Needed

The current article scores well, but the scoring harness repeatedly identifies the same evidence gap:

- only one 100-round session per configuration;
- no repeated independent sessions;
- no interleaved run order;
- no cross-session variance or confidence bounds;
- no aggregate distribution/CDF artifact;
- safety evidence is adjacent rather than packaged as part of the same benchmark artifact.

Further copy editing is not the binding constraint. The next score lift requires more benchmark evidence.

## Output Packet

Create a public packet under:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-xrpl-private-latency-v2-YYYYMMDDTHHMMSSZ/
```

The packet must contain:

| Artifact | Purpose |
|---|---|
| `README.md` | human summary, claim boundary, and headline table |
| `manifest.json` | run matrix, commits, binaries, host specs, configs, run order |
| `commands.sh` | exact commands used to run the packet |
| `raw/` | unedited raw benchmark reports for every session |
| `safety/` | adversarial finality gate reports and hashes |
| `aggregate.json` | machine-readable aggregate statistics |
| `aggregate.md` | readable aggregate tables |
| `session-summary.csv` | per-session p50/p95/p99/mean/min/max/stddev |
| `latency-cdf.svg` | CDF or distribution chart across all sessions |
| `latency-bars.svg` | grouped p50/p95/p99 chart |
| `endpoint-equivalence.md` | precise endpoint comparison |
| `SHA256SUMS.txt` | hash manifest for all public artifacts |

Do not include generated validator private keys, wallet seeds, node databases, or transient private material.

## Benchmark Matrix

Run three configurations:

| Config | Description | Rounds/session |
|---|---|---:|
| `xrpl_stock` | private 6-validator `rippled` using stock timing | 1000 |
| `xrpl_fast_timing` | same `rippled` commit rebuilt with reduced local ledger timers | 1000 |
| `postfiat_l1v2_full_vote` | release Post Fiat L1 v2, 6 validators, full-vote finality, quorum-early disabled | 1000 |

Minimum sessions:

```text
5 independent sessions per configuration
```

Preferred sessions:

```text
10 independent sessions per configuration
```

If time is limited, 5 sessions x 1000 rounds is the minimum article-grade packet.

## Run Order

Interleave configurations so thermal effects, scheduler effects, and background host noise do not all favor one implementation.

Use a rotating order:

```text
session 1: xrpl_stock -> postfiat_l1v2_full_vote -> xrpl_fast_timing
session 2: postfiat_l1v2_full_vote -> xrpl_fast_timing -> xrpl_stock
session 3: xrpl_fast_timing -> xrpl_stock -> postfiat_l1v2_full_vote
repeat until session target is met
```

Each session must use fresh run directories. Record start/end UTC for every config run.

## Required Build Metadata

Record in `manifest.json`:

- Post Fiat repo path;
- Post Fiat git head;
- Post Fiat git status short;
- `cargo --version`;
- `rustc --version`;
- `postfiat-node` SHA256;
- `postfiat-rpc-sdk` SHA256;
- `rippled` repo path;
- `rippled` upstream commit;
- stock `rippled` binary SHA256;
- fast-timing `rippled` binary SHA256;
- host `uname`;
- CPU count;
- run order;
- environment variables that change benchmark behavior;
- excluded private-material policy.

## Endpoint Equivalence

Create `endpoint-equivalence.md` with this table:

| System | Endpoint | Client-visible meaning |
|---|---|---|
| `rippled` | submit-to-validated | payment appears in a validated ledger on the private network |
| Post Fiat L1 v2 | submit-to-finality | transfer appears in a certified and locally applied batch with a transaction-specific finality receipt |

State the boundary plainly:

```text
These are not byte-identical protocol concepts. They are the closest
client-visible local-finality surfaces in the two systems. A stricter future
comparison should also report post-finality read/query cost for each application.
```

## Statistics

For every raw run, compute:

- count;
- min;
- max;
- mean;
- standard deviation;
- p50;
- p95;
- p99.

For each configuration across sessions, compute:

- mean of session p50;
- stddev of session p50;
- confidence interval for session p50;
- mean of session p95;
- stddev of session p95;
- confidence interval for session p95;
- mean of session p99;
- stddev of session p99;
- confidence interval for session p99;
- aggregate all-round p50/p95/p99;
- ratio distribution vs Post Fiat for stock `rippled`;
- ratio distribution vs Post Fiat for fast-timing `rippled`.

Report win counts:

```text
Post Fiat p50 faster than fast-timing rippled: X/Y sessions
Post Fiat p95 faster than fast-timing rippled: X/Y sessions
Post Fiat p99 faster than fast-timing rippled: X/Y sessions
Post Fiat p50 faster than stock rippled: X/Y sessions
Post Fiat p95 faster than stock rippled: X/Y sessions
Post Fiat p99 faster than stock rippled: X/Y sessions
```

Do not present within-run confidence intervals as cross-session confidence.

## Safety Gate

Rerun the adversarial finality gate against the exact code and binaries represented by the benchmark packet.

Include:

```text
safety/adversarial-finality-gate.json
safety/adversarial-finality-gate.md
```

Required cases:

| Case | Required result |
|---|---|
| duplicate/conflicting proposal-vote refusal | passed |
| stale vote rejection | passed |
| stale certificate rejection | passed |
| parent/state-root tamper rejection | passed |
| under-quorum partition rejection | passed |
| process restart persistence | passed |
| one-validator outage | passed |
| delayed vote retry | passed |
| Byzantine disjoint proposer | passed |
| malformed transport/certified-batch rejection | passed |

Record:

- focused tests passed/total;
- chaos gate passed/total;
- command used;
- report SHA256;
- residual work.

## Acceptance Criteria

The packet is article-ready only if all are true:

- 5+ independent sessions per configuration completed;
- each session has 1000 measured sequential transfers, or the packet clearly marks the lower count as preliminary;
- run order is interleaved;
- all raw reports are included and hash-bound;
- Post Fiat beats fast-timing `rippled` on p50 and p95 in most sessions;
- Post Fiat beats stock `rippled` on p50 and p95 in all or near-all sessions;
- aggregate p50/p95 ratios remain materially favorable;
- safety gate passes on the same benchmarked code;
- `SHA256SUMS.txt` validates;
- public packet contains no private keys, wallet seeds, generated node databases, or secrets.

If any acceptance criterion fails, do not update the article as if the packet is final. Publish it only as a preliminary packet or rerun.

## Execution Steps

1. Inspect existing benchmark scripts:

```bash
rg -n "xrpl-private|finality-latency|chaos-gate|release" scripts docs/status
```

2. Build or extend a single orchestration script:

```text
scripts/postfiat-xrpl-latency-evidence-v2
```

3. Script responsibilities:

- build/reuse release Post Fiat binaries;
- build/reuse stock `rippled`;
- build/reuse fast-timing `rippled`;
- create a timestamped output directory;
- run the interleaved matrix;
- copy raw reports into `raw/`;
- run the safety gate;
- write `manifest.json`;
- compute aggregate statistics;
- generate markdown and chart artifacts;
- write `SHA256SUMS.txt`;
- validate hashes;
- print final packet path.

4. Run a smoke packet first:

```bash
SESSIONS=1 ROUNDS=20 scripts/postfiat-xrpl-latency-evidence-v2
```

5. If the smoke packet validates, run the article-grade packet:

```bash
SESSIONS=5 ROUNDS=1000 scripts/postfiat-xrpl-latency-evidence-v2
```

6. If time permits, run the stronger packet:

```bash
SESSIONS=10 ROUNDS=1000 scripts/postfiat-xrpl-latency-evidence-v2
```

## Optional Remote Follow-On

The local v2 packet is the immediate article-grade evidence target. The next credibility jump is a matched remote topology:

```text
same host class
same regions
same validator count
same client location
same transaction count
private rippled cluster vs Post Fiat L1 v2 cluster
```

Remote packet additions:

- region manifest;
- per-validator host metadata;
- measured RTT matrix;
- public RPC load profile;
- one-validator outage under load;
- delayed-vote and partition injection under load.

Do not block the local v2 packet on the remote packet.

## Blog Update Rule

Only update the blog after the v2 packet exists and passes acceptance criteria.

The article update should replace the current single-session table with:

- aggregate cross-session table;
- win-count table;
- CDF/chart;
- safety-gate summary;
- endpoint-equivalence note;
- one clear boundary sentence that this remains a controlled private benchmark, not a public-mainnet latency number.

## Tmux Injector Prompt

Use this if handing to an overnight worker:

```text
Read $POSTFIAT_REPO/docs/status/postfiat-l1v2-xrpl-latency-evidence-packet-v2-plan-2026-06-06.md.
Build and run the Post Fiat L1 v2 / private XRPL latency evidence packet v2.
Preserve consensus safety. Do not include secrets or private validator material in the public packet.
First run SESSIONS=1 ROUNDS=20 as a smoke test. If it validates, run SESSIONS=5 ROUNDS=1000.
Interleave stock rippled, fast-timing rippled, and Post Fiat full-vote sessions.
Rerun the adversarial finality gate against the same code represented by the packet.
Emit README.md, manifest.json, raw reports, safety reports, aggregate.json, aggregate.md,
session-summary.csv, latency charts, endpoint-equivalence.md, commands.sh, and SHA256SUMS.txt.
Stop only for a real blocker or if an acceptance criterion fails in a way that requires design input.
Keep a short lab book inside the output directory.
```
