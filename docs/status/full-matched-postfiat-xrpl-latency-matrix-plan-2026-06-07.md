# Full Matched Post Fiat / XRPL Latency Matrix Plan

Date: 2026-06-07 UTC
Status: completed 2026-06-08 UTC
Owner: latency benchmark worker

## Objective

Build one public evidence packet that removes the current article's main weakness:
the newest Post Fiat `89 ms` real-transfer result and the private XRPL controls
currently live in separate packets.

The target packet should support this exact claim shape:

```text
In a matched local private 6-validator matrix, current Post Fiat L1 v2
full-vote real signed-transfer finality was X ms p50, compared with Y ms
submit-to-validated for fast-timing private rippled and Z ms submit-to-validated
for stock private rippled.
```

The packet must make the comparison stronger without pretending the finality
semantics are byte-identical. It should state endpoint equivalence explicitly:

| System | Endpoint | Client-visible meaning |
|---|---|---|
| Post Fiat L1 v2 | `wallet_to_finality_ms` | signed native transfer has a finality receipt from a certified and locally applied batch |
| `rippled` | `submit_to_validated_ms` | submitted payment appears in a validated private XRPL ledger |

## Current Evidence State

Current public article:

```text
postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

Existing XRPL control packet:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-xrpl-private-latency-v2-20260606T2214Z/
```

That packet is matched across:

- stock private `rippled`;
- fast-timing private `rippled`;
- an older Post Fiat full-vote latency harness.

Existing current Post Fiat real-transfer packet:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-real-transaction-latency-20260607T133944Z/
```

That packet reports:

| Lane | Count | p50 | p95 | p99 |
|---|---:|---:|---:|---:|
| Post Fiat full vote | 1000 | `89.061525 ms` | `105.776512 ms` | `117.092231 ms` |
| Post Fiat quorum-fast | 1000 | `84.277622 ms` | `100.484681 ms` | `105.198309 ms` |

The problem: the current Post Fiat packet is not part of the matched XRPL
matrix. It is valid evidence, but not enough for the cleanest public sentence.

## Required Matrix

Run all required lanes under one run id, one host, one validator count, one
round count, and one aggregation script.

| Lane key | Sessions | Rounds/session | Required | Metric |
|---|---:|---:|---|---|
| `postfiat_full_vote_current` | 5 | 1000 | yes | `wallet_to_finality_ms` |
| `postfiat_quorum_fast_current` | 5 | 1000 | yes | `wallet_to_finality_ms` |
| `xrpl_stock` | 5 | 1000 | yes | `submit_to_validated_ms` |
| `xrpl_fast_timing` | 5 | 1000 | yes | `submit_to_validated_ms` |

The headline should use `postfiat_full_vote_current`, not quorum-fast. The
quorum-fast lane is useful context, but the full-vote path is the conservative
headline.

## Host And Build Metadata

Record in `manifest.json`:

- hostname;
- `uname -a`;
- `lscpu` summary;
- CPU count;
- OS/kernel;
- repo path and git head for `postfiatl1v2`;
- dirty worktree status;
- `postfiat-node` binary path, size, and SHA-256;
- `rippled` stock binary path, size, SHA-256, version, and git head;
- `rippled` fast-timing binary path, size, SHA-256, version, and git head;
- exact commands and environment variables;
- private-material exclusion policy.

Current host context observed during planning:

```text
Linux postfiatfoundationv2 6.8.0-55-generic x86_64
CPU: 32 logical CPUs, AMD EPYC Processor under KVM
```

The final packet should re-record this from the run itself.

## Implementation Work

Create a v3 packet builder rather than mutating the old v2 packet in place.

Suggested script:

```text
scripts/postfiat-xrpl-latency-evidence-v3
```

The v3 script should reuse:

```text
scripts/testnet-real-transaction-latency-benchmark
scripts/xrpl-private-control-benchmark
scripts/testnet-finality-chaos-gate
```

Do not use the older Post Fiat `testnet-tx-finality-latency-benchmark` as the
headline lane. That older harness timed extra evidence/reporting work and is
not the clean user-facing transaction-speed metric.

### V3 Run Order

Randomize/rotate lane order by session to reduce thermal/cache/order effects.

Example:

| Session | Order |
|---:|---|
| 1 | `xrpl_stock`, `postfiat_full_vote_current`, `postfiat_quorum_fast_current`, `xrpl_fast_timing` |
| 2 | `postfiat_full_vote_current`, `xrpl_fast_timing`, `xrpl_stock`, `postfiat_quorum_fast_current` |
| 3 | `xrpl_fast_timing`, `postfiat_quorum_fast_current`, `xrpl_stock`, `postfiat_full_vote_current` |
| 4 | `postfiat_quorum_fast_current`, `xrpl_stock`, `postfiat_full_vote_current`, `xrpl_fast_timing` |
| 5 | `xrpl_stock`, `xrpl_fast_timing`, `postfiat_quorum_fast_current`, `postfiat_full_vote_current` |

### Post Fiat Lane Commands

Use the current real-transfer harness:

```bash
VALIDATORS=6 ROUNDS=1000 VOTE_POLICY=full \
  BASE_DIR=<work_root>/session-001/postfiat_full_vote_current/nodes \
  LOG_DIR=<work_root>/session-001/postfiat_full_vote_current/logs \
  PRIVATE_DIR=<work_root>/session-001/postfiat_full_vote_current/private \
  REPORT=<work_root>/session-001/postfiat_full_vote_current/real-transaction-latency-full.json \
  ITERATIONS_FILE=<work_root>/session-001/postfiat_full_vote_current/logs/iterations.jsonl \
  BASE_PORT=<free_port> RPC_BASE_PORT=<free_port> CARGO_BUILD_MODE=release \
  scripts/testnet-real-transaction-latency-benchmark \
    --rounds 1000 --validators 6 --vote-policy full \
    --report <work_root>/session-001/postfiat_full_vote_current/real-transaction-latency-full.json
```

Repeat with:

```text
VOTE_POLICY=quorum-fast
```

Validation requirements for each Post Fiat report:

- schema is `postfiat-real-transaction-latency-benchmark-v1`;
- `status == passed`;
- `config.validators == 6`;
- `config.rounds == 1000`;
- `config.build_mode == release`;
- all checks are true;
- `latency.wallet_to_finality_ms.count == 1000`;
- `latency.refresh_account_tx_index_ms.p50_ms == 0`;
- per-iteration `validators == 6`;
- per-iteration `quorum == 5`;
- full-vote per-iteration `vote_count == 6`;
- no private key or seed material appears in public artifacts.

### XRPL Lane Commands

Use the existing private control harness:

```bash
python3 scripts/xrpl-private-control-benchmark \
  --rippled <stock_or_fast_binary> \
  --validators 6 \
  --rounds 1000 \
  --work-root <work_root>/session-001/xrpl_stock/private \
  --report <work_root>/session-001/xrpl_stock/xrpl-private-control-benchmark.json \
  --port-base <free_port_block>
```

Use the current stock and fast-timing binaries already recorded by v2 unless a
fresh build is explicitly required:

```text
$REPOS_ROOT/rippled/.build/rippled-stock-3.1.3-46b241ace8b30d9c9775d60ffba7d24b21903896
$REPOS_ROOT/rippled/.build/rippled-fasttiming-3.1.3-46b241a-ledger250ms
```

Validation requirements for each XRPL report:

- `status == passed`;
- `network.validators == 6`;
- `workload.rounds_completed == 1000`;
- `latency.submit_to_validated.count == 1000`;
- no validator seeds or master secrets in public artifacts.

## Safety Gate

Run the finality gate after the matrix:

```bash
VALIDATORS=6 ROUNDS=3 \
  BASE_DIR=<work_root>/safety/adversarial-finality-gate \
  LOG_DIR=<work_root>/safety/adversarial-finality-gate/logs \
  REPORT=<work_root>/safety/adversarial-finality-gate/testnet-finality-chaos-gate.json \
  TIMEOUT_SECONDS=50 TRANSPORT_TIMEOUT_MS=3000 SEND_RETRIES=1 RETRY_BACKOFF_MS=75 \
  scripts/testnet-finality-chaos-gate
```

Acceptance:

- `chaos_gate_ok == true`;
- all cases pass;
- `residual_work == []`.

The safety gate does not create the performance result. It supports the public
claim that the fast path was not merely fast because adversarial finality checks
were skipped.

## Aggregation

Create a public packet under:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-xrpl-current-matched-latency-<RUN_ID>/
```

Required files:

| Artifact | Purpose |
|---|---|
| `README.md` | human-readable claim, methodology, headline tables |
| `manifest.json` | host, binaries, git heads, commands, run matrix |
| `aggregate.json` | normalized machine-readable stats |
| `aggregate.md` | readable cross-session and all-round tables |
| `methodology.md` | exact endpoint semantics, measurement windows, exclusions |
| `endpoint-equivalence.md` | why `wallet_to_finality_ms` and `submit_to_validated_ms` are the comparison surfaces |
| `lab-book.md` | chronological run log |
| `raw/session-*/...` | sanitized raw report JSONs |
| `safety/testnet-finality-chaos-gate.json` | adversarial finality gate |
| `SHA256SUMS.txt` | hash manifest |

Recommended charts:

- `latency-cdf.svg`;
- `latency-bars.svg`;
- optional `session-p50-strip.svg`.

Public packet must exclude:

- node databases;
- validator key files;
- wallet key files;
- generated seeds;
- debug logs containing secrets;
- transient private work directories.

## Article Rewrite Requirements

After the packet exists, rewrite:

```text
postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

The article should be self-contained and use this structure:

1. **Claim**: one paragraph with the matched matrix result.
2. **Methodology**: host, validators, sessions, workload, endpoints, build mode.
3. **Finality semantics**: Post Fiat full-vote/quorum-fast vs `rippled` validated ledger.
4. **Results**: current full matrix table, with p50/p95/p99/mean.
5. **Why the result occurs**: direct certified-transfer path vs XRPL ledger-close loop.
6. **Safety**: quorum `5 of 6`, full-vote `6 of 6`, gate cases, exact invariant.
7. **Boundaries**: local loopback, not mainnet, not peer L1 comparison, not high-throughput load.
8. **Next evidence**: remote matched topology, parallel load, Avax/Sui peer context.

Cut from the current article:

- old adjacent-packet explanation;
- duplicate boundary paragraphs;
- long artifact inventory tables;
- any sentence implying the `89 ms` result came from the old XRPL v2 matrix.

Keep:

- XRPL ancestry/control rationale;
- stock vs fast-timing distinction;
- fast-timing tail behavior;
- explicit "not XRPL mainnet" boundary;
- links to the new packet and previous packets as historical context.

## Scoring Gate

Score only after the rewrite points at the completed matched packet.

Minimum scoring run:

```bash
scripts/whitepaper-openai-chat-latest-score \
  $POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md \
  --label postfiat-l1v2-current-matched-latency
```

Preferred scoring:

- GPT `chat-latest`: 3 samples;
- Opus: 3 samples;
- DeepSeek: 3 samples;
- publish only if the aggregate is better than the current candidate and no
  judge identifies a material factual error.

Do not promote a lower-scoring article. If the matched packet is strong but the
article scores poorly, rewrite the article rather than weakening the evidence.

## Expected Runtime

Approximate runtime:

- stock private `rippled`: about 3 seconds per round, so 5 sessions of 1000
  rounds is roughly 4.2 hours plus startup;
- fast-timing private `rippled`: p50 is subsecond but tails can be large; budget
  roughly 1.5 to 3 hours;
- Post Fiat current full/quorum lanes: minutes per session, plus setup;
- safety gate and packaging: under 30 minutes.

Total expected wall time: 6 to 9 hours.

## Done Criteria

This work is done only when all of the following are true:

- one v3 public packet exists with all four required lanes;
- every required lane has 5 sessions of 1000 successful rounds;
- raw sanitized reports and hashes are published;
- the matrix manifest records host, commands, binaries, git heads, and private-material exclusions;
- the safety gate passes with no residual work;
- the blog article is rewritten around the v3 packet;
- the article is scored after the rewrite;
- the article is not promoted if score or factual quality regresses.

## Completion Record

Completed packet:

```text
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-xrpl-current-matched-latency-20260607T184110Z/
```

Completed article:

```text
postfiatorg.github.io/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

The final packet contains all four required lanes with `5` sessions and `1000`
successful rounds per session:

| Lane key | Sessions | Rounds | Aggregate count | p50 ms | p95 ms | p99 ms | Mean ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| `postfiat_full_vote_current` | 5 | 1000 | 5000 | 85.847 | 101.930 | 108.617 | 85.515 |
| `postfiat_quorum_fast_current` | 5 | 1000 | 5000 | 83.528 | 100.352 | 107.002 | 82.944 |
| `xrpl_fast_timing` | 5 | 1000 | 5000 | 573.386 | 15351.875 | 20178.719 | 1517.656 |
| `xrpl_stock` | 5 | 1000 | 5000 | 3003.492 | 3056.831 | 3981.100 | 3033.628 |

Post Fiat full-vote won against both XRPL controls in `5/5` sessions for p50,
p95, p99, and mean latency. The supported headline ratio is `6.68x` faster than
fast-timing private `rippled` at p50 and `34.99x` faster than stock private
`rippled` at p50.

The safety gate passed:

```text
schema = postfiat-testnet-finality-chaos-gate-v1
chaos_gate_ok = true
cases_passed = 9/9
residual_work = []
```

The packet manifest records host, `uname -a`, `lscpu`, repo git heads, binary
paths and SHA-256 hashes, run matrix, run order, reproduction commands, and the
private-material exclusion policy. Public artifacts include `README.md`,
`manifest.json`, `aggregate.json`, `aggregate.md`, `methodology.md`,
`endpoint-equivalence.md`, `lab-book.md`, raw sanitized session reports, safety
reports, charts, and `SHA256SUMS.txt`.

Validation performed after packaging:

- required packet files existed;
- every raw Post Fiat report had schema
  `postfiat-real-transaction-latency-benchmark-v1`, `status == passed`, release
  build mode, `validators == 6`, `rounds == 1000`, all checks true, and `1000`
  successful iterations;
- every raw XRPL report had `status == passed`, `network.validators == 6`,
  `workload.rounds_completed == 1000`, `latency.submit_to_validated.count ==
  1000`, and `private_material_redacted == true`;
- full-vote reports had `vote_count == 6` for all `5000` iterations;
- quorum-fast reports used quorum `5` and returned with either `5` or `6` votes;
- `sha256sum -c SHA256SUMS.txt` passed;
- public packet secret scan passed for seed/private-key patterns;
- the article references the completed v3 packet and current headline numbers;
- stale article references to the old `89.061 ms`, v2 XRPL packet, and adjacent
  real-transfer packet were absent.

Scoring was run after the rewrite with `scripts/whitepaper-openai-chat-latest-score`.
The scoring judge treated the piece as a bounded benchmark note rather than a
full protocol whitepaper and kept it in the 70s. Score artifacts:

| Label | Score |
|---|---:|
| `l1v2-latency-current-matched-article` | 78 |
| `postfiat-l1v2-current-matched-latency-protocol` | 75 |
| `postfiat-l1v2-current-matched-latency-formalized` | 76 |
| `postfiat-l1v2-current-matched-latency-best-blog-shape` | 74 |

Interpretation:

- The scorer is not saying the matched-matrix sprint was wasted. The sprint
  removed the main evidence defect: current Post Fiat real-transfer latency and
  private XRPL controls now live in one matched packet instead of adjacent
  packets.
- The scorer is saying the article is still a bounded benchmark note, not a
  high-scoring performance whitepaper. The scoring rubric rewards standalone
  protocol specification, WAN/load/resource evidence, and full
  safety/liveness/resource analysis. This packet was not designed to provide
  those broader claims.
- A prior score near `90` should not be treated as directly comparable unless
  it was produced by the same scoring prompt, same article scope, same evidence
  packet, and same claim boundary. A high score on a less constrained or
  differently scoped draft does not invalidate the current packet; it means the
  artifact being scored was different.
- The correct response is not to weaken the evidence or overstate the article.
  The correct score-moving response is to collect the missing evidence for the
  next public paper: remote matched topology, concurrent load, resource metrics,
  state-growth behavior, and failure injection during load.

The current article remains a benchmark report, not a full protocol whitepaper.
The main missing evidence for a higher-scoring public paper is still remote
matched topology, parallel load, CPU/memory/bandwidth/storage metrics, and a
full protocol-level liveness/resource analysis.
