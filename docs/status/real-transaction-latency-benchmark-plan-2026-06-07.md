# Real Transaction Latency Benchmark Plan

Date: 2026-06-07 UTC
Status: implemented; local evidence passed
Owner: latency worker
Scope: build a clean transaction-speed benchmark that measures real signed transfers without timing evidence-packet overhead

## Result Update: 2026-06-07

The plan has now been implemented for local controlled-testnet evidence.

Implemented artifacts:

- `postfiat-node tx-latency-benchmark`: in-binary benchmark runner for real signed native transfers.
- `scripts/testnet-real-transaction-latency-benchmark`: provisioning wrapper that starts validator transport services once, then runs the measured loop in the node binary.
- synchronous account-history index refresh removed from the finality commit path; `account-tx-index-build` is the explicit catch-up path.
- persistent validator services refresh local status per connection so multi-round service reuse does not report stale local state.

The measured loop uses:

- real wallet key material generated for the run;
- real signed native transfers;
- normal mempool admission;
- normal proposal/vote/certificate/apply path;
- local hot finality receipts;
- post-run state verification across all validators.

The measured metrics exclude shell setup, `jq`, report construction, hash manifests, and evidence export. Fee quote time is reported per iteration but is not included in `wallet_to_finality_ms`; `wallet_to_finality_ms` starts at wallet signing.

## Evidence Summary

All runs below used 6 local validators, 1000 sequential signed native transfers, release `postfiat-node`, local loopback transport, persistent validator services, and post-run state verification.

| Policy | `wallet_to_finality_ms` p50 / p95 / p99 | `admitted_to_finality_ms` p50 / p95 / p99 | `consensus_round_ms` p50 / p95 / p99 | Sync account-index refresh |
|---|---:|---:|---:|---:|
| `full` | `89.061525` / `105.776512` / `117.092231` | `80.584858` / `96.626791` / `106.712084` | `74.278318` / `89.898910` / `99.947237` | `0.0 ms` p50/p95/p99 |
| `quorum-fast` | `84.277622` / `100.484681` / `105.198309` | `75.785991` / `91.735668` / `97.235542` | `69.676769` / `85.336663` / `89.847728` | `0.0 ms` p50/p95/p99 |

Passed checks in both reports:

- `iteration_count_matches_rounds`;
- `all_transactions_final`;
- `all_receipts_accepted`;
- `no_duplicate_receipts`;
- `final_height_matches_rounds`;
- `state_verified_after_run`;
- `all_rounds_ok`;
- `all_vote_policies_match`;
- `account_history_index_not_in_synchronous_finality`;
- `converged`.

Evidence files:

| Artifact | SHA-256 |
|---|---|
| `reports/testnet-real-transaction-latency-benchmark/full-1000/real-transaction-latency-full-1000.json` | `6d1e8233ffaefdfc7d3ab82ce4d7f769ca31733fb76cacfda33e79ee6a07dfe2` |
| `reports/testnet-real-transaction-latency-benchmark/full-1000/logs/iterations.jsonl` | `fd9ca422652cf7f9948ac4e41943ecb28c52d2194b2a89180b490c530419130d` |
| `reports/testnet-real-transaction-latency-benchmark/full-1000/logs/local-harness.json` | `3c43bc3a72e760b7e2a8df89a6741d6f41f2f123f5f286c0a96547817c0c025a` |
| `reports/testnet-real-transaction-latency-benchmark/quorum-1000/real-transaction-latency-quorum-fast-1000.json` | `27f925a0b93ff6bf181adaa36033153d6dab7c0ff8dee402a301edcc7534b7a0` |
| `reports/testnet-real-transaction-latency-benchmark/quorum-1000/logs/iterations.jsonl` | `9d435367159a3d78ae2cf91bc9df3aee80c29ad6830b19cb9bce6c312cde7627` |
| `reports/testnet-real-transaction-latency-benchmark/quorum-1000/logs/local-harness.json` | `563927e95b3332fffa1786363e4e7c0e21490cfac5b968cb5fd385e68c996442` |
| `reports/testnet-finality-chaos-gate/real-tx-latency-20260607/testnet-finality-chaos-gate.json` | `01d2d530cde60a779f8193082b1a7bcc5879aa941f77526c53265dbb4cb9882c` |
| `reports/testnet-real-transaction-latency-benchmark/evidence-packet-20260607/manifest.json` | `d72ad8a6b4a4ddc4d0643ec8b5bf9124514082aa2a408041615c274cf0cc5fe5` |

Hash manifests:

```text
reports/testnet-real-transaction-latency-benchmark/full-1000/real-transaction-latency-full-1000.json.SHA256SUMS.txt
reports/testnet-real-transaction-latency-benchmark/quorum-1000/real-transaction-latency-quorum-fast-1000.json.SHA256SUMS.txt
reports/testnet-real-transaction-latency-benchmark/evidence-packet-20260607/SHA256SUMS.txt
```

Safety gate:

```text
VALIDATORS=6 ROUNDS=3 BASE_DIR=reports/testnet-finality-chaos-gate/real-tx-latency-20260607 \
  LOG_DIR=reports/testnet-finality-chaos-gate/real-tx-latency-20260607/logs \
  REPORT=reports/testnet-finality-chaos-gate/real-tx-latency-20260607/testnet-finality-chaos-gate.json \
  TIMEOUT_SECONDS=50 TRANSPORT_TIMEOUT_MS=3000 SEND_RETRIES=1 RETRY_BACKOFF_MS=75 \
  scripts/testnet-finality-chaos-gate
```

Result: `testnet_finality_chaos_gate=ok`, 9/9 cases passed, `residual_work=[]`.

## Commands Used

Full-vote benchmark:

```bash
VALIDATORS=6 ROUNDS=1000 VOTE_POLICY=full \
  BASE_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000/nodes \
  LOG_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000/logs \
  PRIVATE_DIR=reports/testnet-real-transaction-latency-benchmark/full-1000/private \
  REPORT=reports/testnet-real-transaction-latency-benchmark/full-1000/real-transaction-latency-full-1000.json \
  ITERATIONS_FILE=reports/testnet-real-transaction-latency-benchmark/full-1000/logs/iterations.jsonl \
  BASE_PORT=27650 RPC_BASE_PORT=28650 CARGO_BUILD_MODE=release \
  scripts/testnet-real-transaction-latency-benchmark --rounds 1000 --validators 6 --vote-policy full \
  --report reports/testnet-real-transaction-latency-benchmark/full-1000/real-transaction-latency-full-1000.json
```

Quorum-fast benchmark:

```bash
VALIDATORS=6 ROUNDS=1000 VOTE_POLICY=quorum-fast \
  BASE_DIR=reports/testnet-real-transaction-latency-benchmark/quorum-1000/nodes \
  LOG_DIR=reports/testnet-real-transaction-latency-benchmark/quorum-1000/logs \
  PRIVATE_DIR=reports/testnet-real-transaction-latency-benchmark/quorum-1000/private \
  REPORT=reports/testnet-real-transaction-latency-benchmark/quorum-1000/real-transaction-latency-quorum-fast-1000.json \
  ITERATIONS_FILE=reports/testnet-real-transaction-latency-benchmark/quorum-1000/logs/iterations.jsonl \
  BASE_PORT=29650 RPC_BASE_PORT=30650 CARGO_BUILD_MODE=release \
  scripts/testnet-real-transaction-latency-benchmark --rounds 1000 --validators 6 --vote-policy quorum-fast \
  --report reports/testnet-real-transaction-latency-benchmark/quorum-1000/real-transaction-latency-quorum-fast-1000.json
```

Focused Rust checks:

```bash
cargo check -p postfiat-node
cargo test -p postfiat-node account_tx_index_explicit_build_catches_up_after_archive_prune -- --nocapture
cargo test -p postfiat-node init_then_run_once -- --nocapture
```

## Current Limitations

- This is local loopback controlled-testnet evidence, not public WAN latency.
- The wrapper provisions a fresh local network per run; it does not measure long-lived public RPC load.
- `quorum-fast` is reported separately. In this sequential benchmark, certified propagation to all validators still happens before the next round, so p50 improvement is modest.
- Fee quote time is measured per iteration but not included in `wallet_to_finality_ms`; the user-facing metric begins at wallet signing.
- The report includes per-round JSONL and hash manifests, but not a separate rendered public packet README yet.

## Objective

Produce a benchmark packet that answers one user-facing question:

```text
How long does a real signed native transfer take to become final on Post Fiat L1 v2?
```

The current 1000-transfer report is useful, but it is an evidence harness. It times wallet signing, mempool admission, certified finality, local apply, finality lookup, shell orchestration, JSON parsing, file artifact handling, and report extraction together. That is acceptable as an end-to-end evidence packet, but it is not a clean transaction-speed benchmark.

The target benchmark must still use real transactions:

- real wallet keys;
- real signed native transfers;
- normal mempool admission;
- normal proposal, vote, certificate, and apply path;
- real finality receipt or equivalent finality proof;
- post-run verification that every transaction mutated state correctly.

The benchmark must not time unrelated evidence generation, shell parsing, report construction, or audit export as if those were user transaction latency.

## Measurement Boundaries

Report three metrics, with names that cannot be confused.

| Metric | Starts | Stops | What It Means |
|---|---|---|---|
| `wallet_to_finality_ms` | wallet begins signing or client submits a pre-signed transfer, depending on mode | client receives verified finality for that transfer | user-facing transaction latency |
| `admitted_to_finality_ms` | node accepts an already-signed transfer into mempool | certified block is locally applied and finality receipt is available | protocol transaction latency |
| `consensus_round_ms` | proposal construction begins for an admitted batch | quorum/full certificate is formed and applied | consensus/apply latency, not full UX |

Each metric is valid, but they answer different questions. Public claims must name the metric used.

## Current Diagnosis

Current report:

```text
reports/testnet-tx-finality-latency-benchmark/testnet-tx-finality-benchmark-hotpath-final-1000-20260607T114241Z.json
```

Relevant p50 values:

| Stage | p50 ms |
|---|---:|
| `submit_to_finality` | 182.951 |
| `submit_to_certified` | 121.852 |
| `client_visible_finality_round` | 97.370 |
| `harness_or_unattributed_overhead` | 61.885 |
| `vote_requests` | 28.354 |
| `local_apply` | 44.147 |
| `write_commit` | 39.819 |
| `refresh_account_tx_index` | 24.763 |

Interpretation:

- the headline number is not a pure consensus number;
- roughly one third of the headline p50 is harness or report-extraction overhead;
- `refresh_account_tx_index` is a read/query cache refresh and should not block transaction finality;
- the benchmark was run with full vote collection, not quorum-fast completion;
- local apply still performs synchronous persistence before client-visible finality.

## Required Harness

Create a persistent benchmark runner. It can be Rust or Python, but Rust is preferred if it reuses the RPC SDK and avoids subprocess-per-round overhead.

Suggested command:

```text
postfiat-node tx-latency-benchmark
  --topology <path>
  --source-validator <node-id>
  --wallet-seed-file <path>
  --recipient <address>
  --rounds 1000
  --mode wallet-to-finality|admitted-to-finality|consensus-round
  --vote-policy full|quorum-fast
  --evidence-mode minimal|full
  --report <path>
```

If implemented as a script first, it must still keep connections/processes persistent:

- start validators once;
- keep RPC services alive;
- keep the client process alive;
- do not spawn `jq`, `nc`, node CLI commands, or SDK CLI commands inside the timed loop;
- write per-round raw records after timing, or buffer them and flush periodically;
- export evidence and hash manifests after the measured loop.

## Timed Path Rules

The timed path may include:

- wallet signing, if measuring `wallet_to_finality_ms`;
- signed transaction serialization;
- RPC submit or direct local submit, depending on declared mode;
- mempool admission;
- batch formation;
- proposal construction;
- validator vote requests;
- certificate aggregation;
- local block apply;
- finality receipt availability;
- finality lookup or callback delivery.

The timed path must not include:

- shell orchestration;
- `jq` parsing;
- report generation;
- evidence packet assembly;
- hash manifest generation;
- final full-history replay;
- graph rendering;
- README generation;
- repeated process startup;
- unrelated account-history query cache rebuilds unless explicitly measuring query-readiness latency.

## Code Changes

### Phase 1: Defer account history index refresh

Move account transaction index refresh out of the synchronous finality commit path.

Current offender:

```text
refresh_account_tx_index_ms p50 ~= 24.763 ms
```

Required behavior:

- committing a block records enough data for finality and later indexing;
- account history index refresh runs after finality, in a background worker, explicit maintenance command, or post-commit queue;
- `account_tx` remains correct by falling back to scan/rebuild when the index is stale;
- status exposes index freshness.

Safety boundary:

- do not alter block hash, state root, receipts, certificate, parent link, or transaction execution semantics;
- if the async/index worker fails, finality remains valid and `account_tx` reports stale index or falls back safely.

Acceptance:

- focused tests cover stale index after commit;
- focused tests cover index catch-up after multiple commits;
- benchmark report proves `refresh_account_tx_index_ms` no longer appears in the synchronous finality stage.

### Phase 2: Add persistent real-transfer benchmark

Build the runner described above.

Minimum output schema:

```json
{
  "schema": "postfiat-real-transaction-latency-benchmark-v1",
  "generated_utc": "...",
  "config": {
    "validators": 6,
    "rounds": 1000,
    "mode": "wallet-to-finality",
    "vote_policy": "full",
    "transport": "local-loopback-persistent",
    "build_mode": "release"
  },
  "latency": {
    "wallet_to_finality_ms": {"count": 1000, "p50_ms": 0, "p95_ms": 0, "p99_ms": 0},
    "admitted_to_finality_ms": {"count": 1000, "p50_ms": 0, "p95_ms": 0, "p99_ms": 0},
    "consensus_round_ms": {"count": 1000, "p50_ms": 0, "p95_ms": 0, "p99_ms": 0}
  },
  "checks": {
    "all_transactions_final": true,
    "all_receipts_accepted": true,
    "state_verified_after_run": true,
    "no_duplicate_receipts": true,
    "final_height_matches_rounds": true
  },
  "not_measured": [
    "public WAN latency",
    "public RPC load",
    "evidence packet assembly",
    "full history replay inside timed path"
  ]
}
```

### Phase 3: Run two honest policies

Run both policies, because they answer different production questions.

| Policy | Meaning | Expected Effect |
|---|---|---|
| `full` | wait for every validator vote in the configured validator set | conservative, higher latency, better audit completeness |
| `quorum-fast` | finalize once quorum is reached, late votes can arrive after finality | lower latency, closer to BFT fast path |

Do not hide the policy. A public claim must say which one was measured.

### Phase 4: Evidence export after timing

After the timed loop completes, export:

- raw per-round latency JSONL;
- final benchmark summary JSON;
- final state verification;
- receipt inclusion verification;
- block/certificate consistency checks;
- hash manifest;
- command manifest;
- git/binary metadata.

Evidence export must be outside the timed path and clearly labeled as such.

## Required Runs

Minimum:

```text
6 validators
1000 sequential real native transfers
release build
local loopback persistent harness
full vote policy
quorum-fast policy
```

Preferred:

```text
5 sessions x 1000 transfers for each policy
```

Optional comparison:

```text
same persistent harness shape against private rippled where possible
```

Do not block the Post Fiat transaction-speed result on optimizing XRPL. XRPL controls are separate comparison evidence, not a prerequisite for measuring Post Fiat.

## Article-Grade Claims

Allowed after Phase 2 and one clean 1000-round run:

```text
In a local 6-validator persistent real-transfer benchmark, Post Fiat L1 v2
finalized 1000 sequential signed native transfers with p50 X ms, p95 Y ms,
and p99 Z ms under [full/quorum-fast] vote policy.
```

Allowed after Phase 3:

```text
Full-vote finality measured X/Y/Z ms. Quorum-fast finality measured A/B/C ms.
The two policies have different safety/liveness tradeoffs and are reported
separately.
```

Not allowed:

```text
Post Fiat is faster than public XRPL mainnet.
Post Fiat is state-of-the-art BFT.
Post Fiat has production WAN latency of X ms.
Evidence packet generation is part of transaction latency.
```

## Acceptance Criteria

- persistent harness exists and is documented;
- timed path does not spawn per-round shell commands;
- timed path does not run evidence export;
- real signed transfers are used;
- every transaction has an accepted receipt;
- final state is verified after the run;
- report splits `wallet_to_finality_ms`, `admitted_to_finality_ms`, and `consensus_round_ms`;
- report records vote policy;
- full-vote and quorum-fast are not mixed in one headline;
- account-history indexing is not in the synchronous finality path unless explicitly measuring query-readiness latency;
- public packet includes raw report, manifest, commands, verification output, and `SHA256SUMS.txt`.

## Initial Burndown

1. `[x]` Refactor account transaction index refresh out of synchronous commit.
2. `[x]` Add focused tests for stale index fallback and catch-up.
3. `[x]` Add persistent benchmark runner with real signed transfers.
4. `[x]` Add report schema and validation checks.
5. `[x]` Run local 6-validator 1000-transfer full-vote benchmark.
6. `[x]` Run local 6-validator 1000-transfer quorum-fast benchmark.
7. `[x]` Run adversarial finality gate on the same code.
8. `[x]` Package evidence as reports, raw JSONL, harness reports, and `SHA256SUMS.txt`.
9. `[ ]` Update blog only after deciding this packet should replace or extend the current artifact.
