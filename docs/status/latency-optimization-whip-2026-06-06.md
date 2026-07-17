# Latency Optimization Overnight Whip

Status: active 10-hour overnight execution mandate
Date: 2026-06-06
Scope: PostFiat L1 v2 transparent PFT finality latency

This is the whip reference for improving PostFiat controlled-testnet latency.
Treat this document as the priority order for the overnight run. The objective
is direct: make the network faster by any technically defensible means, while
preserving deterministic consensus, validator convergence, evidence quality,
and the controlled-write boundary.

The overnight agent should not try to win a marketing comparison against XRPL.
The job is to understand and improve PostFiat's own latency stack first. A
private XRPL lab comparison becomes useful after PostFiat has a stage-by-stage
latency driver report.

## Injector Prompt

Use this one-line prompt for a tmux/WHIP injector:

```text
Read docs/status/latency-optimization-whip-2026-06-06.md, run the 10-hour latency optimization burn down in order, keep the lab book updated, preserve consensus safety, run evidence gates, and stop only for real blockers or the 10-hour cutoff.
```

## Run Window

This mandate is designed for one bounded 10-hour WHIP run.

- Start time: the moment the `l1` WHIP cron block is installed.
- Maximum wall-clock duration: 10 hours.
- At hour 9, stop starting new high-risk or long-running code changes.
- At hour 9, do not start a live 25-round benchmark unless the local evidence
  already justifies it and the run has enough remaining time to complete doctor
  and packet packaging.
- At hour 9:30, switch to wrap-up: run gates, update the lab book, package
  evidence, and record blockers.
- At hour 10, uninstall the `l1` WHIP cron block even if useful work remains.
  Leave the next step in the lab book rather than continuing indefinitely.

WHIP does not currently expose a native max-duration flag. When enabling this
mandate, schedule an explicit one-shot auto-stop with `at`:

```bash
mkdir -p reports/testnet-latency-whip
cat >/tmp/postfiat-latency-whip-autostop.at <<'EOF'
cd $POSTFIAT_REPO
$CODEX_WHIP_HOME/venv/bin/codex-whip uninstall-cron --profile l1
date -u +"latency whip auto-stop at %Y-%m-%dT%H:%M:%SZ" >> reports/testnet-latency-whip/latency-whip-autostop.log
EOF
at -M -t YYYYMMDDHHMM.SS </tmp/postfiat-latency-whip-autostop.at
```

Before enabling cron, verify no old `l1` block is present:

```bash
crontab -l 2>/dev/null | sed -n '/BEGIN codex-whip profile l1/,/END codex-whip profile l1/p'
```

After enabling cron, record the start and cutoff timestamps in the first lab
book entry of the run.

## Current Baseline

Latest closed packet:

```text
reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-evidence-packet/README.md
```

Baseline results from that packet:

| Surface | Current result |
|---|---:|
| Live controlled fleet wallet finality | 25/25 passed |
| Live `submit_finality_total` | p50 `290.35356ms`, p95 `375.418875ms`, p99 `380.18433ms` |
| Live certified round | p50 `277.490913ms`, p95 `349.296915ms`, p99 `353.31662ms` |
| Local persistent RPC | 100/100 passed |
| Local `submit_to_finality` | p50 `1108.065983ms`, p95 `1419.845026ms`, p99 `1474.393055ms` |
| Slow-peer quorum-early | passed, quorum `4/5`, slow node `validator-4` |
| Post-run live doctor | passed at height `60` |

Do not discard this baseline. Every optimization must compare against it or a
fresh baseline generated during the whip run.

## Operating Rules

- Make technical calls without asking preference questions.
- Prefer measurement before changing code.
- Preserve deterministic state transition behavior and canonical signing,
  hashing, and serialization.
- Do not weaken quorum certification, local apply verification, state
  convergence checks, replay evidence, or redaction checks.
- Do not expose a persistent public write edge. The finality write edge must
  remain explicitly gated and controlled.
- Do not spam live writes. Use local benchmarks for iteration; run live
  25-round evidence only after a meaningful change or to establish a fresh
  baseline.
- Do not treat harness-only wins as protocol wins. Separate user-visible
  protocol latency from script startup, SSH, artifact extraction, and read-back
  audit latency.
- Every completed item must leave either a JSON report under `reports/`, a
  code/doc change, or a blocker entry in the lab book.
- Do not revert unrelated dirty files. Work with existing local changes if they
  touch the latency slice; otherwise ignore them.
- If a change touches consensus, transport, RPC write admission, key material,
  storage, or replay behavior, run the narrow safety tests before declaring the
  slice complete.
- Do not start unrelated privacy, whitepaper, governance, feature-parity, or
  cleanup work from this mandate.
- Respect the 10-hour cutoff. Do not continue the run just because there is more
  useful latency work available.

## Success Criteria

Primary overnight success is one of:

- a measured latency improvement with passing local and live evidence;
- a stage-by-stage latency driver report that identifies the next bottleneck
  with enough precision to implement the next slice;
- a real blocker report proving why neither of the above can be achieved.

Stretch targets:

| Metric | Current | Stretch |
|---|---:|---:|
| Live `submit_finality_total` p50 | `290ms` | `<=250ms` |
| Live `submit_finality_total` p95 | `375ms` | `<=350ms` |
| Live `submit_finality_total` p99 | `380ms` | `<=500ms` |
| Local 100-round `submit_to_finality` p50 | `1108ms` | `<=900ms` |
| Local 100-round `submit_to_finality` p95 | `1420ms` | `<=1300ms` |
| Local 100-round `client_visible_finality_round` p50 | `950ms` | `<=750ms` |

The stretch targets are not permission to cut safety. A slower safe result with
a precise bottleneck explanation is better than a fast ambiguous result.

## Work Loop

For each slice:

1. State the hypothesis in the lab book.
2. Capture or reuse a baseline.
3. Make the smallest coherent change or measurement addition.
4. Run the narrowest useful local gate.
5. Compare p50/p95/p99 and stage-level deltas.
6. Keep, revert, or refine the change based on evidence.
7. Update the lab book and burn down.
8. Run live evidence only after local evidence is meaningful.

Never stack multiple unmeasured optimizations before a benchmark unless the
first task is explicitly instrumentation-only.

## Lab Book

Append entries here during the run. Each entry must use this form:

```text
### YYYY-MM-DD HH:MM UTC - SHORT NAME

Hypothesis:
Change:
Commands:
Reports:
Result:
Decision:
Next:
```

### 2026-06-06 Initial State

Hypothesis: the next large gain will come from identifying whether live
`~290-380ms` finality is dominated by quorum RTT, local apply, RPC admission,
finality proof emission, read-back audit, or harness overhead.

Change: none. Starting from the closed 2026-06-06 fast-finality evidence
packet.

Commands: none in this entry.

Reports:

- `reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-evidence-packet/README.md`
- `reports/testnet-live-wallet-finality-benchmark/fast-finality-exp-20260606-live25-wallet-finality2/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-local100-persistent-rpc2/testnet-tx-finality-latency-benchmark.json`

Result: current live controlled fleet p50/p95/p99 is
`290.35356ms` / `375.418875ms` / `380.18433ms`; local 100-round persistent RPC
p50/p95/p99 is `1108.065983ms` / `1419.845026ms` / `1474.393055ms`.

Decision: start with measurement decomposition and local iteration. Do not run
private XRPL until PostFiat's own stage drivers are understood.

Next: `LAT-001`.

### 2026-06-06 02:09 UTC - WHIP ENABLED

Hypothesis: a bounded 10-hour WHIP run can improve latency or produce a precise
latency-driver report without weakening consensus safety or exposing a public
write edge.

Change: enabled `codex-whip` profile `l1` against `codex_l1session:0.0` with
the latency mandate prompt and scheduled an auto-stop.

Commands:

```bash
at -M -t 202606061209.43 </tmp/postfiat-latency-whip-autostop.at
$CODEX_WHIP_HOME/venv/bin/codex-whip install-cron --profile l1 --target codex_l1session:0.0 --repo $POSTFIAT_REPO --start-codex --stable-seconds 60 --cooldown-seconds 120
```

Reports:

- `reports/testnet-latency-whip/latency-whip-run-window.env`
- `reports/testnet-latency-whip/latency-whip-autostop-at.out`
- `reports/testnet-latency-whip/latency-whip-atq.txt`
- WHIP log: `$CODEX_WHIP_STATE/l1.log`

Result: run window recorded as start `2026-06-06T02:09:43Z`, cutoff
`2026-06-06T12:09:43Z`; auto-stop scheduled as `at` job `1`.

Decision: leave WHIP enabled for the bounded 10-hour latency sprint.

Next: `LAT-001`.

### 2026-06-06 02:16 UTC - LAT-001 DRIVER REPORT

Hypothesis: existing local and live finality reports already contain enough
stage timing data to identify the first bottleneck without touching consensus
code.

Change: added `scripts/testnet-latency-driver-report`, a read-only report
builder over the closed local 100-round and live 25-round evidence.

Commands:

```bash
chmod +x scripts/testnet-latency-driver-report
RUN_ID=latency-whip-20260606T0216-driver scripts/testnet-latency-driver-report
python3 -m py_compile scripts/testnet-latency-driver-report
git diff --check -- scripts/testnet-latency-driver-report docs/status/latency-optimization-whip-2026-06-06.md whip.md
jq -e '.schema == "postfiat-testnet-latency-driver-report-v1" and .status == "passed" and (.local.ranked_p95_drivers|length) > 0 and (.live.ranked_p95_drivers|length) > 0 and .claim_boundary.not_a_code_change == true' reports/testnet-latency-whip/latency-whip-20260606T0216-driver/latency-driver-report.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0216-driver/latency-driver-report.json`

Result: report passed. Local p95 top drivers were `local_apply`
(`501.335289ms`, `35.3%` of submit-finality p95), `vote_requests`
(`399.872163ms`, `28.2%`), and `local_vote` (`253.826074ms`, `17.9%`).
Live spend-path p95 top drivers were `local_apply` (`161.765293ms`, `43.1%`),
`vote_requests` (`141.896672ms`, `37.8%`), and `local_vote`
(`45.081477ms`, `12.0%`). Stage shares are non-additive because some timing
boundaries overlap.

Decision: first useful focus is instrumentation completion, then choose between
`local_apply` and `vote_requests` as the first optimization target.

Next: `LAT-002`.

### 2026-06-06 02:26 UTC - LAT-002/LAT-003 LOCAL CONTRACTED BASELINE

Hypothesis: adding explicit timing fields and a metric contract will make the
next optimization comparable without changing consensus, quorum, local apply,
or replay behavior; a short local 25-round run is enough to choose the next
bottleneck.

Change: instrumented the live wallet-finality canary and distribution wrapper
with wrapper wall-clock fields, added a local latency metric contract, emitted
`read_lookup_ms`, `finality_receipt_emission_proxy_ms`, and
`submit_to_finality_unattributed_ms`, and updated the driver report to consume
the new fields. No consensus flags, quorum predicates, local-apply checks, or
write-edge admission checks were weakened.

Commands:

```bash
python3 -m py_compile scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark scripts/testnet-latency-driver-report
bash -n scripts/testnet-tx-finality-latency-benchmark
git diff --check -- scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark scripts/testnet-tx-finality-latency-benchmark scripts/testnet-latency-driver-report docs/status/latency-optimization-whip-2026-06-06.md whip.md
VALIDATORS=5 ROUNDS=25 BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/nodes LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/logs PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/private-wallet-material REPORT=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/logs/local-harness.json TIMEOUT_SECONDS=45 RPC_TIMEOUT_MS=10000 LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 DEFER_CERTIFIED_SENDS=1 COMBINE_MEMPOOL_CERTIFY=1 HOT_FINALITY_RECEIPT=1 SUBMIT_IN_CERTIFY=1 PERSISTENT_FINALITY_RPC=1 scripts/testnet-tx-finality-latency-benchmark --rounds 25
jq -e '.schema == "postfiat-testnet-tx-finality-latency-benchmark-v1" and .status == "passed" and .latency_benchmark_ok == true and .metric_contract.schema == "postfiat-latency-metric-contract-v1"' reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json
RUN_ID=latency-whip-20260606T022426-local25-driver LOCAL_REPORT=reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json scripts/testnet-latency-driver-report
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T022426-local25-driver/latency-driver-report.json`

Result: local 25-round benchmark passed with p50/p95/p99
`submit_to_finality` of `851.823197ms` / `928.359818ms` /
`955.050526ms`; p50/p95/p99 `client_visible_finality_round` of
`727.238108ms` / `786.01203ms` / `819.309657ms`. Fresh local p95 drivers
were `vote_requests` (`300.998189ms`), `local_apply` (`226.321493ms`), and
`local_vote` (`184.073292ms`). The live driver report from the latest closed
live 25-round evidence still ranks `local_apply`, `vote_requests`, and
`local_vote` as the top three.

Decision: keep the instrumentation. Treat this as a contracted fresh local
baseline, not a durable code-speedup claim until a 100-round regression
repeats it.

Next: `LAT-004`; choose a single optimization target from the common top-two
drivers, with live `local_apply` favored if the code review shows a safe,
narrow improvement.

### 2026-06-06 03:26 UTC - LAT-004/LAT-010 REJECTED OPTIMIZATION AND LIVE RESTORE

Hypothesis: the common top-two latency drivers were `vote_requests` and
`local_apply`; a narrow reduction in repeated vote-request status/topology work
could reduce local and live p95 without changing quorum, signing, hashing,
local apply, or replay behavior.

Change: tried two transport-status slices. First, remote vote response reused
the already-computed local status instead of recomputing status after signing
the vote. Second, outbound vote requests reused preloaded round
topology/source status instead of recomputing source status per outbound
request. Both Rust changes were reverted. The accepted changes from this slice
are instrumentation, driver reporting, and evidence packaging only.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
cargo build --release -p postfiat-node
scripts/testnet-transport-peer-certified-quorum-early-smoke
scripts/testnet-tx-finality-latency-benchmark --rounds 25
scripts/testnet-tx-finality-latency-benchmark --rounds 100
scripts/testnet-live-binary-compatibility-check
scripts/testnet-live-orchard-binary-upgrade
scripts/testnet-live-wallet-finality-benchmark
scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T022911Z-quorum-early/testnet-transport-peer-certified-quorum-early-smoke.json`
- `reports/testnet-latency-whip/latency-whip-20260606T022929Z-local25-vote-status-reuse/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T022929-vote-status-reuse-driver/latency-driver-report.json`
- `reports/testnet-latency-whip/latency-whip-20260606T023424Z-quorum-early-preloaded-status/testnet-transport-peer-certified-quorum-early-smoke.json`
- `reports/testnet-latency-whip/latency-whip-20260606T023444Z-local25-preloaded-status/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T023444-preloaded-status-driver/latency-driver-report.json`
- `reports/testnet-latency-whip/latency-whip-20260606T023646Z-local100-preloaded-status/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T023646-local100-preloaded-status-driver/latency-driver-report.json`
- `reports/testnet-latency-whip/latency-whip-20260606T024435Z-live-binary-compat/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T024512Z-live-binary-upgrade/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T024600Z-live25-preloaded-status/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T032057Z-live-binary-compat-reverted/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T032138Z-live-binary-upgrade-reverted/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T032245Z-live-doctor-reverted/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0326-evidence-packet/README.md`

Result: first status-reuse local 25 was worse than the fresh local contracted
baseline: p50/p95/p99 `submit_to_finality` moved from `851.823197ms` /
`928.359818ms` / `955.050526ms` to `899.640481ms` / `999.698803ms` /
`1013.155822ms`. The second preloaded-status candidate had a better local
100-round p50/p95/p99 than the older closed local 100 baseline
(`1056.287914ms` / `1343.760446ms` / `1399.044798ms` versus
`1108.065983ms` / `1419.845026ms` / `1474.393055ms`), but live controlled
fleet p50/p95/p99 regressed to `378.075303ms` / `454.776148ms` /
`494.844098ms` versus the prior live closed baseline of `290.35356ms` /
`375.418875ms` / `380.18433ms`. Reverted live binary compatibility, reverted
upgrade, and post-revert validator doctor all passed. Post-revert doctor
reported matching binary hash
`c597dd4b0d0dee8cdfc2f7ca4ea86418489616850fa2349fabf01b5627280142`,
height `110`, converged state, active services, and state verification.

Decision: reject the vote-request status preload lane. Do not claim a retained
live latency improvement from this slice. Keep the measurement/reporting
changes and the packet; live is restored to the reverted release binary. The
new evidence says `local_apply`, not source-status recomputation, is the next
high-value target.

Next: decompose `local_apply` internals before touching consensus-adjacent
state application code. Any next optimization must preserve local verified
apply and run local safety gates before another live benchmark.

### 2026-06-06 03:48 UTC - LAT-011 VALIDATOR COUNT MATRIX

Hypothesis: validator count will identify whether the next tail-latency slope
is dominated by quorum fanout alone or by local apply/certification work that
also widens as topology grows.

Change: no code change. Ran the local fast-finality transaction shape at
supported validator counts. `VALIDATORS=3` is blocked by the existing harness
guard: `VALIDATORS must be at least 4 for quorum latency benchmark`.

Commands:

```bash
VALIDATORS=4 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
VALIDATORS=5 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
VALIDATORS=6 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
VALIDATORS=10 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/README.md`
- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/validator-count-matrix-summary.md`
- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/v4/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/v5/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/v6/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/v10/testnet-tx-finality-latency-benchmark.json`

Result:

| Validators | submit p50 | submit p95 | submit p99 | vote_requests p95 | local_apply p95 | certificate p95 |
|---:|---:|---:|---:|---:|---:|---:|
| 4 | `783.26386ms` | `870.631915ms` | `891.30288ms` | `315.610591ms` | `196.729385ms` | `122.924227ms` |
| 5 | `792.27934ms` | `902.228615ms` | `936.379756ms` | `300.531994ms` | `224.658585ms` | `137.042339ms` |
| 6 | `841.732074ms` | `934.100018ms` | `981.985105ms` | `310.541634ms` | `243.604635ms` | `146.438206ms` |
| 10 | `1048.876885ms` | `1221.094202ms` | `1227.672476ms` | `337.010406ms` | `343.54619ms` | `201.887803ms` |

Decision: mark LAT-011 done with the `3`-validator blocker recorded. The
matrix says topology growth is not explained by vote-request fanout alone:
`local_apply`, `certificate`, and `local_vote` all widen at 10 validators.

Next: LAT-012 batch-size matrix if runtime allows; in parallel planning terms,
the next code slice should instrument `local_apply` and certificate formation
more deeply before attempting another live binary.

### 2026-06-06 03:53 UTC - LAT-012 BATCH-SIZE MATRIX BLOCKER

Hypothesis: the current local finality harness may already be able to vary
batch size with node `--max-transactions` controls.

Change: no code change. Read `scripts/testnet-tx-finality-latency-benchmark`,
node command usage, and existing batch smoke scripts.

Commands:

```bash
rg -n "BATCH|batch|transfers|TRANSFER|rounds|ROUNDS" scripts/testnet-tx-finality-latency-benchmark scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark
rg --files scripts | rg 'batch|throughput|latency|mempool|transfer'
rg -n "max-transactions|MAX_TRANSACTIONS|BATCH_SIZE|batch-size|throughput|signed-transfer-file" scripts crates/node/src docs/status docs
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0353-batch-size-matrix/README.md`

Result: the node supports `mempool-batch --max-transactions N`, and existing
smokes can create multi-transfer batches. The finality latency harness cannot
currently produce a true batch-size matrix because it constructs one signed
transfer per measured round and the persistent finality RPC path accepts a
single `signed_transfer_json`. Non-persistent measured paths also hard-code
`--max-transactions 1`. The maximum supported measured batch size in the
current finality harness is therefore `1`.

Decision: mark LAT-012 blocked on a dedicated finality harness extension. Do
not publish or infer batch-size throughput/latency from direct-apply smoke
coverage.

Next: LAT-013 topology matrix if host placement evidence is available;
otherwise record the topology blocker and move to LAT-014 design.

### 2026-06-06 04:01 UTC - LAT-013 TOPOLOGY MATRIX BLOCKER

Hypothesis: current live artifacts may already contain enough redacted host or
region placement metadata to classify same-host, same-region, and cross-region
latency.

Change: no code change. Checked live doctor, live wallet-finality reports,
remote deploy plans, and existing topology-capture reports.

Commands:

```bash
find reports -name '*remote-deploy-plan*.json'
jq '{run_id, validators, machine_count, checks: .checks}' reports/testnet-latency-whip/latency-whip-20260606T032245Z-live-doctor-reverted/testnet-live-validator-doctor.json
jq '{run_id, validators, rounds_completed, chain_ids, latency: .latency.submit_finality_total}' reports/testnet-latency-whip/latency-whip-20260606T024600Z-live25-preloaded-status/testnet-live-wallet-finality-benchmark.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0401-topology-matrix/README.md`

Result: no current six-validator deploy plan was found in the recorded
`*remote-deploy-plan*.json` artifacts. The live doctor reports six active
validators, but does not expose machine count, host grouping, region grouping,
or RTT. An older topology capture exists from 2026-05-14, but it was a
five-validator 2/2/1 host/operator grouping and is stale relative to the
current six-validator live fleet.

Decision: mark LAT-013 blocked on current topology metadata. Do not label the
live benchmark as same-region or cross-region without a fresh redacted topology
and RTT capture.

Next: LAT-014 private XRPL comparison design.

### 2026-06-06 04:06 UTC - LAT-014 PRIVATE XRPL COMPARISON DESIGN

Hypothesis: an XRPL comparison can be useful only after the PostFiat latency
driver is decomposed and only if the XRPL side uses a private P2P validator
network with a validated-ledger finality definition.

Change: wrote a design artifact. No code change and no XRPL benchmark claim.

Commands: no benchmark commands; design-only slice.

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0406-private-xrpl-comparison-design/README.md`

Result: design requires matching validator count, host placement, client
placement, persistent client path, transaction shape, warm-up exclusion, and
finality semantics. XRPL standalone mode is explicitly rejected because it does
not exercise validator consensus. The XRPL metric should be
`submit_to_validated_ledger_ms`; the PostFiat side should use the current
metric contract's `submit_to_finality` and `client_visible_finality_round`.

Decision: mark LAT-014 done as a design artifact. Do not run XRPL until a
private XRPL harness and current topology capture exist.

Next: return to the primary latency target: instrument `local_apply` and
certificate formation in the local finality path.

## Burn Down

| ID | Priority | Status | Task | Exit Evidence |
|---|---|---|---|---|
| LAT-001 | P0 | Done | Build a latency-driver report from existing local/live benchmark JSON. It must extract p50/p95/p99 for wallet signing, RPC admission, mempool submit, mempool batch, proposal, local vote, vote requests, certificate, local apply, finality receipt emission, read lookup, certified sends, and artifact/harness overhead where available. | `reports/testnet-latency-whip/latency-whip-20260606T0216-driver/latency-driver-report.json`. |
| LAT-002 | P0 | Done | Add missing stage timings to the local benchmark and live wallet-finality wrapper without changing consensus behavior. Missing fields should be explicit `null` or `not_measured`, not silently omitted. | Script syntax checks plus `reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json`. |
| LAT-003 | P0 | Done | Run a quick local persistent-RPC matrix to identify local bottlenecks: 5 validators, 25 rounds, current fast path, no code changes except instrumentation. | `reports/testnet-latency-whip/latency-whip-20260606T022426Z-local25/testnet-tx-finality-latency-benchmark.json`. |
| LAT-004 | P0 | Done | Identify whether local p95 is dominated by vote requests, local apply, batch creation, or RPC admission. Choose exactly one bottleneck for the first optimization. | Lab-book chose `vote_requests` first from fresh local 25; live result redirects next work to `local_apply`. |
| LAT-005 | P1 | Done | Optimize the chosen bottleneck. Candidate levers: remove unnecessary read-after-write from the measured user-finality path, cache reusable RPC/topology/material parsing, reduce process/shell crossings in local harness, avoid repeated full state reconstruction, tighten vote-request parallelism, or move audit-only replay out of the hot path. | Two vote-request/status optimizations tested, rejected, and reverted; see `reports/testnet-latency-whip/latency-whip-20260606T0326-evidence-packet/README.md`. |
| LAT-006 | P1 | Done | Run local 100-round regression after the first accepted optimization. Require no linear height growth, all iterations confirmed, convergence, and state verification. | Candidate local 100 passed at `reports/testnet-latency-whip/latency-whip-20260606T023646Z-local100-preloaded-status/testnet-tx-finality-latency-benchmark.json`; candidate later rejected by live gate. |
| LAT-007 | P1 | Done | Re-run slow-peer quorum-early after any transport or quorum-path change. | `reports/testnet-latency-whip/latency-whip-20260606T022911Z-quorum-early/testnet-transport-peer-certified-quorum-early-smoke.json` and `reports/testnet-latency-whip/latency-whip-20260606T023424Z-quorum-early-preloaded-status/testnet-transport-peer-certified-quorum-early-smoke.json`. |
| LAT-008 | P1 | Done | Run one live 25-round controlled-fleet wallet-finality benchmark only after local evidence is improved or instrumentation materially changed. | Candidate live 25 passed functionally but regressed latency: `reports/testnet-latency-whip/latency-whip-20260606T024600Z-live25-preloaded-status/testnet-live-wallet-finality-benchmark.json`; change reverted. |
| LAT-009 | P1 | Done | Run post-live validator doctor. | Post-revert doctor passed: `reports/testnet-latency-whip/latency-whip-20260606T032245Z-live-doctor-reverted/testnet-live-validator-doctor.json`. |
| LAT-010 | P1 | Done | Package a new latency evidence packet with reports, script hashes, dirty-state report, redaction scan, and claim boundary. | `reports/testnet-latency-whip/latency-whip-20260606T0326-evidence-packet/README.md`. |
| LAT-011 | P2 | Done | Run validator-count matrix locally: `3/4/5/6/10` validators if runtime allows. Keep transaction shape fixed. | `reports/testnet-latency-whip/latency-whip-20260606T033036Z-validator-matrix/README.md`; `3` validators blocked by harness minimum. |
| LAT-012 | P2 | Blocked | Run batch-size matrix locally: `1/10/100/1000` transfers or maximum supported by current harness. Keep topology fixed. | Current finality harness supports measured batch size `1` only; see `reports/testnet-latency-whip/latency-whip-20260606T0353-batch-size-matrix/README.md`. |
| LAT-013 | P2 | Blocked | Run topology matrix if machine access is available: same host, same region, cross-region. | Current live artifacts lack a six-validator redacted deploy plan/RTT capture; see `reports/testnet-latency-whip/latency-whip-20260606T0401-topology-matrix/README.md`. |
| LAT-014 | P2 | Done | Design private XRPL comparison only after LAT-001 through LAT-010 are complete. Use P2P private network, not standalone mode; match validator count, host placement, client distance, and validated-ledger definition. | `reports/testnet-latency-whip/latency-whip-20260606T0406-private-xrpl-comparison-design/README.md`; no PostFiat claim change without measured data. |

## Commands

Use unique `RUN_ID`s. Prefer `date -u +%Y%m%dT%H%M%SZ` suffixes.

### Local 25-Round Fast Path

```bash
RUN_ID=latency-whip-$(date -u +%Y%m%dT%H%M%SZ)-local25
ROOT=reports/testnet-latency-whip/$RUN_ID
VALIDATORS=5 ROUNDS=25 \
BASE_DIR=$ROOT/nodes \
LOG_DIR=$ROOT/logs \
PRIVATE_DIR=$ROOT/private-wallet-material \
REPORT=$ROOT/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=$ROOT/logs/local-harness.json \
TIMEOUT_SECONDS=45 RPC_TIMEOUT_MS=10000 \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 HOT_FINALITY_RECEIPT=1 SUBMIT_IN_CERTIFY=1 \
PERSISTENT_FINALITY_RPC=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

### Local 100-Round Regression

```bash
RUN_ID=latency-whip-$(date -u +%Y%m%dT%H%M%SZ)-local100
ROOT=reports/testnet-latency-whip/$RUN_ID
VALIDATORS=5 ROUNDS=100 \
BASE_DIR=$ROOT/nodes \
LOG_DIR=$ROOT/logs \
PRIVATE_DIR=$ROOT/private-wallet-material \
REPORT=$ROOT/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=$ROOT/logs/local-harness.json \
TIMEOUT_SECONDS=45 RPC_TIMEOUT_MS=10000 \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 HOT_FINALITY_RECEIPT=1 SUBMIT_IN_CERTIFY=1 \
PERSISTENT_FINALITY_RPC=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 100
```

### Slow-Peer Quorum-Early

```bash
RUN_ID=latency-whip-$(date -u +%Y%m%dT%H%M%SZ)-quorum-early
ROOT=reports/testnet-latency-whip/$RUN_ID
VALIDATORS=5 \
BASE_DIR=$ROOT/nodes \
LOG_DIR=$ROOT/logs \
REPORT=$ROOT/testnet-transport-peer-certified-quorum-early.json \
HARNESS_REPORT=$ROOT/logs/local-harness.json \
scripts/testnet-transport-peer-certified-quorum-early-smoke
```

### Live 25-Round Controlled Fleet

Run this only after local evidence justifies a live run:

```bash
RUN_ID=latency-whip-$(date -u +%Y%m%dT%H%M%SZ)-live25
ROOT=reports/testnet-latency-whip/$RUN_ID
VALIDATORS=6 ROUNDS=25 \
ROOT_DIR=$ROOT \
LOG_DIR=$ROOT/logs \
REPORT=$ROOT/testnet-live-wallet-finality-benchmark.json \
TIMEOUT_SECONDS=300 RPC_TIMEOUT_MS=30000 \
scripts/testnet-live-wallet-finality-benchmark
```

### Live Validator Doctor

```bash
RUN_ID=latency-whip-$(date -u +%Y%m%dT%H%M%SZ)-doctor
ROOT=reports/testnet-live-validator-doctor/$RUN_ID
VALIDATORS=6 \
ROOT_DIR=$ROOT \
LOG_DIR=$ROOT/logs \
REPORT=$ROOT/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 \
scripts/testnet-live-validator-doctor
```

## Candidate Optimization Areas

Investigate in measurement order, not preference order.

### RPC And Harness

- Separate protocol finality latency from wrapper wall-clock latency.
- Check whether the live wrapper pays unnecessary read-back latency after the
  finality receipt is already returned.
- Keep audit read-back as a separate measured stage, not part of the
  user-finality metric unless the claim requires it.
- Avoid repeated SDK builds, topology parsing, or process startup in benchmark
  loops.

### Mempool And Batch Creation

- Verify whether batch construction is still doing avoidable disk or JSON work.
- Keep batch ids and signing preimages canonical.
- Preserve replay and invalid-signature rejection tests.

### Vote And Certificate Path

- Inspect `vote_requests_ms`, per-peer RTT, quorum wait, and slow/unresolved
  targets.
- Preserve quorum threshold and deterministic certificate contents.
- Do not return before local verified apply.
- If connection reuse is added, bound queues and timeouts.

### Local Apply And Storage

- Determine whether local apply cost is CPU, disk, JSON serialization, or
  state verification.
- Do not skip durable state writes in any path that claims validator finality.
- If a fast receipt uses hot in-memory state, retain an audit/replay path.

### Live Topology

- Measure whether proposer choice changes latency.
- Compare validators by host/region if placement metadata is available.
- Do not move or restart live services merely to chase a small p50 improvement
  unless the lab book names the expected gain and rollback plan.

## Required Gates Before Calling A Code Slice Done

At minimum:

```bash
python3 -m py_compile scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark
bash -n scripts/testnet-tx-finality-latency-benchmark
git diff --check -- <files changed in this slice>
```

If Rust code changed:

```bash
cargo test -p postfiat-node
```

If transport/quorum code changed:

```bash
scripts/testnet-transport-peer-certified-quorum-early-smoke
scripts/testnet-transport-peer-certified-partial-outage-smoke
```

If RPC write admission changed:

```bash
scripts/testnet-rpc-method-inventory
```

If live evidence ran:

```bash
scripts/testnet-live-validator-doctor
```

## Evidence Packet Requirements

For any accepted overnight improvement, create:

```text
reports/testnet-latency-whip/<run_id>/evidence-packet/
```

Include:

- local benchmark report;
- live benchmark report if live was run;
- slow-peer report if transport/quorum code changed;
- post-run validator doctor if live was run;
- script hashes;
- current git head;
- `git status --short`;
- value-oriented credential scan;
- `SHA256SUMS.txt`;
- short `README.md` with what changed, measured lift, and claim boundary.

Do not include raw node directories, private wallet material, validator key
directories, SSH logs with credentials, or private IP/secret material.

## Claim Boundary

Allowed after this whip, if supported by evidence:

- "controlled-testnet transparent PFT finality improved from X to Y";
- "the dominant measured latency driver is Z";
- "under this controlled topology, p50/p95/p99 are X/Y/Z";
- "the write edge remained controlled and non-public".

Not allowed:

- public write throughput claims;
- mainnet readiness claims;
- public decentralization claims;
- "faster than XRPL mainnet" claims from controlled PostFiat data;
- private XRPL comparison claims unless a private P2P XRPL lab was actually run
  under matched conditions.

## Stop Conditions

Stop and record a blocker if:

- the 10-hour run window expires;
- validator doctor fails after a live run;
- state roots diverge;
- a report contains private key or credential-shaped material;
- a latency improvement requires weakening quorum, local apply, or replay;
- live write access is unavailable or unsafe;
- the same failure repeats three times with no new information.

If blocked on live access, continue local instrumentation and local benchmark
work. Do not sit idle.

### 2026-06-06 03:54 UTC - LAT-015 LOCAL APPLY WRITE BREAKDOWN

Hypothesis: the `local_apply` tail is dominated by either ordered-commit
persistence, account-index refresh, or full-log JSON rewrites; splitting
`write_commit` will identify the safest next target.

Change: added measurement-only nested timing for the transparent batch
ordered-commit writer. Existing commit callers still use the same journaled
write path; the new report is surfaced only through the local latency harness.

Commands:

```bash
bash -n scripts/testnet-tx-finality-latency-benchmark
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/nodes \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/logs \
PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/private-wallet-material \
HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/logs/local-harness.json \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/testnet-tx-finality-latency-benchmark.json \
VALIDATORS=5 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25 --validators 5 --report reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/testnet-tx-finality-latency-benchmark.json
jq -e '.schema == "postfiat-testnet-tx-finality-latency-benchmark-v1" and .status == "passed" and .latency_benchmark_ok == true and (.latency.peer_certified_stage.local_apply_breakdown.write_commit_breakdown.write_ledger.p95_ms | type == "number") and (.iterations | all(.[]; .peer_certified_timings.local_apply_breakdown.schema == "postfiat-apply-batch-timings-v1" and .peer_certified_timings.local_apply_breakdown.write_commit_breakdown.schema == "postfiat-ordered-commit-write-timings-v1"))' reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/testnet-tx-finality-latency-benchmark.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T035456Z-local25-write-breakdown/testnet-tx-finality-latency-benchmark.json`

Result: local 25-round p50/p95/p99 `submit_to_finality` was
`809.058645ms` / `946.415494ms` / `961.95247ms`. `local_apply` p95 was
`221.031212ms`; `prepare_commit` p95 was `73.652372ms`; `write_commit` p95
was `124.340115ms`. Within `write_commit`, p95 drivers were `write_journal`
`47.696276ms`, `write_blocks` `34.586859ms`, `refresh_account_tx_index`
`20.892156ms`, and `write_batch_archive` `12.863406ms`.

Decision: keep the instrumentation. Do not optimize persistence yet; first
split `prepare_commit`, because it is still a large opaque p95 component and
may distinguish state-root/certificate CPU from log cloning.

Next: add prepare-commit substage timing, then choose the least risky
optimization target.

### 2026-06-06 04:03 UTC - LAT-016 CERTIFICATE VERIFY BREAKDOWN

Hypothesis: `prepare_commit` is dominated by repeated block-certificate
verification inside local apply after the same peer-certified round has already
aggregated and verified the votes.

Change: added measurement-only certificate-verifier substage timing under
`prepare_commit_breakdown`.

Commands:

```bash
bash -n scripts/testnet-tx-finality-latency-benchmark
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/nodes \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/logs \
PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/private-wallet-material \
HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/logs/local-harness.json \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/testnet-tx-finality-latency-benchmark.json \
VALIDATORS=5 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25 --validators 5 --report reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/testnet-tx-finality-latency-benchmark.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T040315Z-local25-certificate-breakdown/testnet-tx-finality-latency-benchmark.json`

Result: local 25-round p50/p95/p99 `submit_to_finality` was
`879.672346ms` / `933.522367ms` / `1026.372752ms`. `prepare_commit` p95 was
`70.511657ms`; inside it, certificate p95 was `62.158768ms`, split into
`certificate_vote_signature` `40.953252ms`, `certificate_id` `11.417677ms`,
and `certificate_registry_root` `10.811557ms`.

Decision: implement a narrow same-process verified-certificate apply path for
the source validator only. Generic certificate-file apply and remote peer apply
must keep full signature verification.

Next: `LAT-017`.

### 2026-06-06 04:09 UTC - LAT-017 VERIFIED CERTIFICATE LOCAL APPLY

Hypothesis: the source validator can safely avoid duplicate vote-signature
verification during immediate local apply if the certificate was just produced
by the in-process aggregate verifier, the on-disk artifact matches the verified
object exactly, and apply still checks the certificate against current commit
evidence and validator registry root.

Change: added `VerifiedBlockCertificateFile`, `aggregate_verified_block_certificate`,
and `apply_batch_with_verified_certificate_with_timings`. The peer-certified
source validator now uses the verified wrapper for transparent local apply.
Remote peer service and generic file-based apply still use full verification.

Commands:

```bash
bash -n scripts/testnet-tx-finality-latency-benchmark
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/nodes \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/logs \
PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/private-wallet-material \
HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/logs/local-harness.json \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/testnet-tx-finality-latency-benchmark.json \
VALIDATORS=5 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25 --validators 5 --report reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/testnet-tx-finality-latency-benchmark.json
cargo test -p postfiat-node
scripts/testnet-transport-peer-certified-partial-outage-smoke
scripts/testnet-transport-peer-certified-quorum-early-smoke
cargo test -p postfiat-node proposal_certificate_accepts_three_of_four_bft_quorum
cargo test -p postfiat-node split_block_votes_reconstruct_certificate
cargo test -p postfiat-node external_proposal_certificates_apply_non_transparent_batches
git diff --check -- crates/node/src/lib_parts/part_02.rs crates/node/src/lib_parts/part_03.rs crates/node/src/transport_cli.rs crates/node/src/main_parts/cli_dispatch.rs crates/node/src/governance.rs crates/node/src/privacy.rs scripts/testnet-tx-finality-latency-benchmark docs/status/latency-optimization-whip-2026-06-06.md
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T040922Z-local25-verified-cert/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-transport-peer-certified-partial-outage/testnet-transport-peer-certified-partial-outage-20260606T041739Z.json`
- `reports/testnet-transport-peer-certified-quorum-early/testnet-transport-peer-certified-quorum-early-20260606T041757Z.json`

Result: local 25-round p50/p95/p99 `submit_to_finality` improved to
`782.537383ms` / `896.857191ms` / `909.781064ms`. `local_apply` p50/p95 fell
to `124.239456ms` / `181.370854ms`; `prepare_commit` p50/p95 fell to
`19.208929ms` / `19.929572ms`; source local apply
`certificate_vote_signature` p50/p95 is now `0.0ms` / `0.0ms`, while
`certificate_registry_root` remains checked at `10.506027ms` /
`10.741571ms`.

Safety gates: `cargo check`, `bash -n`, `cargo fmt --check`, `git diff
--check`, both transport smokes, and three focused block-certificate tests
passed. Full `cargo test -p postfiat-node` completed with 88 passed and 8
failed in pre-existing-looking fixture/vector areas:
governance-agent hash fixtures, governance-agent lineage audit, replicated
state-root vector, and wallet test vector. No block-certificate or transport
test failed in the focused reruns.

Decision: keep the verified-certificate fast path for longer local evidence.
Do not deploy live until a longer local run and live upgrade/rollback plan are
recorded.

Next: run local 100-round evidence with the accepted fast path.

### 2026-06-06 04:19 UTC - LAT-018 LOCAL 100 VERIFIED CERTIFICATE

Hypothesis: the verified-certificate fast path should hold over a longer local
run, but remaining p95 should move toward storage write growth rather than
certificate verification.

Change: no new code. Ran the accepted verified-certificate fast path for 100
local rounds.

Commands:

```bash
BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/nodes \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/logs \
PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/private-wallet-material \
HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/logs/local-harness.json \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/testnet-tx-finality-latency-benchmark.json \
VALIDATORS=5 ROUNDS=100 scripts/testnet-tx-finality-latency-benchmark --rounds 100 --validators 5 --report reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/testnet-tx-finality-latency-benchmark.json
jq -e '.schema == "postfiat-testnet-tx-finality-latency-benchmark-v1" and .status == "passed" and .latency_benchmark_ok == true and .rounds == 100 and (.latency.submit_to_finality.p95_ms | type == "number") and (.latency.peer_certified_stage.local_apply_breakdown.prepare_commit_breakdown.certificate_vote_signature.p95_ms == 0)' reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/testnet-tx-finality-latency-benchmark.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/testnet-tx-finality-latency-benchmark.json`

Result: local 100-round p50/p95/p99 `submit_to_finality` was
`1048.2952ms` / `1374.721412ms` / `1400.337563ms`, compared with the closed
local 100 baseline `1108.065983ms` / `1419.845026ms` / `1474.393055ms`.
`prepare_commit` p95 stayed low at `23.436873ms`, and
`certificate_vote_signature` remained `0.0ms`. The new long-run local apply
p95 is storage-driven: `write_journal` `173.887381ms`, `write_blocks`
`127.082108ms`, `write_batch_archive` `42.515688ms`, and account-index
refresh `27.134502ms`.

Decision: local fast-path improvement is real but p95 is now storage-bound.
Proceed to controlled live evidence before attempting a larger journal/storage
rewrite.

Next: build release binary, run live binary compatibility/upgrade/doctor, then
run live 25-round wallet-finality evidence if doctor passes.

### 2026-06-06 04:27 UTC - LAT-019 LIVE VERIFIED CERTIFICATE DEPLOYMENT

Hypothesis: the verified-certificate source-local-apply fast path is safe to
exercise on the controlled live fleet after local evidence, but live latency may
now be dominated by block-log persistence because the chain height has grown.

Change: built release binary `60864fd5d417ce45183c18cccc6b67b7545230d9284d7255951d7b6fa16efe83`,
ran live binary compatibility, upgraded the controlled fleet, ran validator
doctor, ran 25 live wallet-finality rounds, then ran validator doctor again.

Commands:

```bash
cargo build --release -p postfiat-node
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T042709Z-live-binary-compat \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T042709Z-live-binary-compat/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T042709Z-live-binary-compat/testnet-live-binary-compatibility.json \
scripts/testnet-live-binary-compatibility-check
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T042748Z-live-binary-upgrade \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T042748Z-live-binary-upgrade/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T042748Z-live-binary-upgrade/testnet-live-orchard-binary-upgrade.json \
POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T042831Z-live-doctor-post-upgrade \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T042831Z-live-doctor-post-upgrade/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T042831Z-live-doctor-post-upgrade/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T042856Z-live25-verified-cert \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T042856Z-live25-verified-cert/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T042856Z-live25-verified-cert/testnet-live-wallet-finality-benchmark.json \
VALIDATORS=6 ROUNDS=25 scripts/testnet-live-wallet-finality-benchmark
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T050333Z-live-doctor-post-benchmark \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T050333Z-live-doctor-post-benchmark/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T050333Z-live-doctor-post-benchmark/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T042709Z-live-binary-compat/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T042748Z-live-binary-upgrade/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T042831Z-live-doctor-post-upgrade/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T042856Z-live25-verified-cert/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T050333Z-live-doctor-post-benchmark/testnet-live-validator-doctor.json`

Result: compatibility, upgrade, pre-benchmark doctor, live benchmark, and
post-benchmark doctor all passed. Live benchmark completed 25/25 with controlled
write edge checks true. The live run advanced from initial height `110` to
spend height `160`. Live `submit_finality_total` p50/p95/p99 was
`553.729569ms` / `617.552241ms` / `651.456701ms`; `certified_round`
p50/p95/p99 was `504.711739ms` / `556.594408ms` / `597.622979ms`.

Decision: live is healthy but this is not a live latency win against the stale
height-60 closed baseline (`290.35356ms` / `375.418875ms` /
`380.18433ms`). Do not claim live improvement. Treat the current live result as
evidence that chain-height/log-write growth has become the dominant live issue.
The previously deployed binary artifact was not available locally for an exact
height-160 rollback A/B; rolling back blindly is not justified because the
verified-certificate path removes duplicate work and generic/remote full
verification remains intact.

Next: target storage/journal growth: stop rewriting full ordered-commit journal
payloads and full block logs on every commit, while preserving crash recovery
and replay.

### 2026-06-06 05:20 UTC - LAT-020 COMPACT ORDERED-COMMIT JOURNAL

Hypothesis: local p95 is inflated by writing a full duplicate of receipts,
ordered batches, archive, and block log into `ordered_commit_journal.json`
before writing the same post-commit state to the final files. A delta journal
should preserve crash recovery while removing most of `write_journal` from the
critical path.

Change: replaced the hot-path ordered-commit journal with
`postfiat-ordered-commit-delta-journal-v1`, containing only optional updated
state files plus the commit delta: receipt delta, ordered batch id, archive
entry, block record, and optional validator registry. Kept legacy full
`postfiat-ordered-commit-journal-v1` recovery support. Added idempotent delta
merge checks for recovery after partial final-file writes and a direct recovery
test for the new delta journal.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
cargo test -p postfiat-node status_recovers_pending_ordered_commit
cargo test -p postfiat-node proposal_certificate_accepts_three_of_four_bft_quorum
cargo test -p postfiat-node split_block_votes_reconstruct_certificate
cargo test -p postfiat-node external_proposal_certificates_apply_non_transparent_batches
RUN_ID=latency-whip-20260606T0520-local25-delta-journal \
BASE_DIR=reports/testnet-latency-whip/${RUN_ID}/nodes \
LOG_DIR=reports/testnet-latency-whip/${RUN_ID}/logs \
PRIVATE_DIR=reports/testnet-latency-whip/${RUN_ID}/private-wallet-material \
REPORT=reports/testnet-latency-whip/${RUN_ID}/testnet-tx-finality-latency-benchmark.json \
scripts/testnet-tx-finality-latency-benchmark --rounds 25 --validators 5
git diff --check -- crates/node/src/lib_parts/part_03.rs crates/node/src/lib_test_parts/consensus_block_history_snapshot_tests.rs
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0520-local25-delta-journal/testnet-tx-finality-latency-benchmark.json`

Result: compile, formatting, diff check, legacy and delta journal recovery
tests, and focused block-certificate tests passed. The local 25-round benchmark
passed 25/25. `submit_to_finality` p50/p95/p99 was `756.937974ms` /
`825.962688ms` / `842.46558ms`. `write_journal` p95 dropped to `5.353915ms`,
and total `write_commit` p95 was `87.901858ms`. Remaining p95 storage costs
were `write_blocks` `41.869762ms`, `write_batch_archive` `14.534368ms`, and
account-index refresh `20.126392ms`.

Decision: keep the delta-journal change for longer local validation. It removes
the duplicated full-history journal write while preserving legacy journal
recovery and idempotent crash replay semantics.

Next: run a 100-round local benchmark with the delta journal and compare
against LAT-018.

### 2026-06-06 05:30 UTC - LAT-021 LOCAL 100 DELTA JOURNAL

Hypothesis: the compact ordered-commit delta journal should hold over a
100-round local run and reduce long-run p95 by removing the duplicated full
journal write.

Change: no new code after LAT-020. Ran a 100-round local benchmark with the
delta journal and existing verified-certificate fast path.

Commands:

```bash
RUN_ID=latency-whip-20260606T0530-local100-delta-journal \
BASE_DIR=reports/testnet-latency-whip/${RUN_ID}/nodes \
LOG_DIR=reports/testnet-latency-whip/${RUN_ID}/logs \
PRIVATE_DIR=reports/testnet-latency-whip/${RUN_ID}/private-wallet-material \
REPORT=reports/testnet-latency-whip/${RUN_ID}/testnet-tx-finality-latency-benchmark.json \
scripts/testnet-tx-finality-latency-benchmark --rounds 100 --validators 5
jq -e '.schema == "postfiat-testnet-tx-finality-latency-benchmark-v1" and .status == "passed" and .latency_benchmark_ok == true and .rounds == 100 and (.latency.submit_to_finality.p95_ms | type == "number") and (.latency.peer_certified_stage.local_apply_breakdown.write_commit_breakdown.write_journal.p95_ms < 10)' reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/testnet-tx-finality-latency-benchmark.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/testnet-tx-finality-latency-benchmark.json`

Result: local 100-round benchmark passed 100/100. `submit_to_finality`
p50/p95/p99 improved from LAT-018 `1048.2952ms` / `1374.721412ms` /
`1400.337563ms` to `1009.133175ms` / `1236.075285ms` / `1254.700457ms`.
`write_journal` p50/p95/p99 improved from `92.333839ms` / `173.887381ms` /
`179.575778ms` to `4.872379ms` / `5.86ms` / `7.901207ms`. Total
`write_commit` p95 improved by `134.906264ms`; total local apply p95 improved
by `136.082607ms`. Remaining storage p95 is now `write_blocks`
`151.488653ms`, `write_batch_archive` `49.320329ms`, and account-index refresh
`26.491596ms`.

Decision: keep the compact delta journal. The next storage target is full
`blocks.json` and `batch_archive.json` rewrite growth, but that is a larger
format/replay change than the journal delta.

Next: run additional persistence/transport smokes, then consider a controlled
live binary upgrade and live 25-round evidence.

### 2026-06-06 05:25 UTC - LAT-022 LIVE DELTA JOURNAL DEPLOYMENT

Hypothesis: the compact delta journal is safe to deploy to the controlled live
fleet after local 100-round evidence, persistence verification, and transport
smokes. Live latency may improve modestly, but the current live p95 is still
likely dominated by full `blocks.json` / `batch_archive.json` rewrites at
higher chain height.

Change: built release binary
`d8d7ea16e1c71df64234f825dd42d0418a96e6d4665a9485ec2e4ab97d02f5b2`, ran live
compatibility, upgraded controlled validators, ran doctor, attempted a 25-round
live benchmark, added a hard per-round canary timeout to the live benchmark
wrapper after an SCP/SSH staging hang, ran a clean timeout-wrapped 4-round live
benchmark, and ran doctor again.

Commands:

```bash
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0
scripts/testnet-transport-peer-certified-partial-outage-smoke
scripts/testnet-transport-peer-certified-quorum-early-smoke
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
git diff --check -- crates/node/src/lib_parts/part_03.rs crates/node/src/lib_test_parts/consensus_block_history_snapshot_tests.rs docs/status/latency-optimization-whip-2026-06-06.md
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0525-live-binary-compat-delta-journal \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0525-live-binary-compat-delta-journal/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0525-live-binary-compat-delta-journal/testnet-live-binary-compatibility.json \
scripts/testnet-live-binary-compatibility-check
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0526-live-binary-upgrade-delta-journal \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0526-live-binary-upgrade-delta-journal/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0526-live-binary-upgrade-delta-journal/testnet-live-orchard-binary-upgrade.json \
POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0527-live-doctor-post-delta-upgrade \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0527-live-doctor-post-delta-upgrade/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0527-live-doctor-post-delta-upgrade/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/testnet-live-wallet-finality-benchmark.json \
VALIDATORS=6 ROUNDS=25 scripts/testnet-live-wallet-finality-benchmark
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0533-live-doctor-after-stalled-live25 \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0533-live-doctor-after-stalled-live25/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0533-live-doctor-after-stalled-live25/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
python3 -m py_compile scripts/testnet-live-wallet-finality-benchmark
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0535-live4-delta-journal-timeout-wrapper \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0535-live4-delta-journal-timeout-wrapper/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0535-live4-delta-journal-timeout-wrapper/testnet-live-wallet-finality-benchmark.json \
VALIDATORS=6 ROUNDS=4 CANARY_TIMEOUT_SECONDS=240 scripts/testnet-live-wallet-finality-benchmark
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0540-live-doctor-post-live4-delta \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0540-live-doctor-post-live4-delta/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0540-live-doctor-post-live4-delta/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0525-live-binary-compat-delta-journal/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0526-live-binary-upgrade-delta-journal/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0527-live-doctor-post-delta-upgrade/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/rounds/round-01/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/rounds/round-02/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/rounds/round-03/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0528-live25-delta-journal/rounds/round-04/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0533-live-doctor-after-stalled-live25/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0535-live4-delta-journal-timeout-wrapper/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0540-live-doctor-post-live4-delta/testnet-live-validator-doctor.json`

Result: local `verify-state`, `verify-blocks`, transport smokes, formatting,
`cargo check`, diff check, live compatibility, controlled binary upgrade, and
all live doctors passed. The attempted 25-round live benchmark produced four
passing round reports, advancing live height from `160` to `168`, but the
aggregate run was stopped because optional SSH/SCP staging hung before the
25-round summary was produced. The four successful rounds had
`submit_finality.total_ms` values `620.819957ms`, `612.740425ms`,
`644.292374ms`, and `577.741286ms`. After adding the timeout wrapper, a clean
4/4 aggregate passed with live `submit_finality_total` p50/p95/p99
`625.466046ms` / `655.981982ms` / `655.981982ms`; `certified_round` p50/p95/p99
was `582.713611ms` / `584.370603ms` / `584.370603ms`. Post-run doctor passed
with all validators OK, converged, state verified, history ready, account
indexes usable/current, binary hashes matched, and controlled write-edge checks
intact.

Decision: keep the live delta-journal deployment. Do not claim a full live
25-round distribution for this slice; claim local 100-round improvement and a
clean live 4-round controlled-fleet smoke/aggregate. The remaining live
bottleneck is still full block/archive persistence plus remote evidence
collection reliability.

Next: target the larger persistence format issue only after a scoped design for
append/snapshot block and archive storage. In parallel, keep the live benchmark
timeout wrapper so future SCP/SSH stalls fail closed.

### 2026-06-06 05:45 UTC - LAT-023 HISTORY PRUNE SIZE PROBE

Hypothesis: the remaining block/archive persistence cost is structurally tied
to retained-history file size. Existing partial-history tooling should show the
same direction as the latency measurements: retaining a shorter active window
shrinks hot files and preserves replay through a checkpoint.

Change: wrote `docs/status/latency-append-storage-slice-2026-06-06.md` as the
next-slice append/snapshot storage spec. Ran `history-status` and
`history-prune-plan` on LAT-021 evidence. Then copied the local 100-round
validator state, created an archive handoff proof for heights `1..80`, pruned
the copy to retain blocks `81..101`, and verified the pruned copy. No live state
and no original benchmark evidence were mutated.

Commands:

```bash
target/debug/postfiat-node history-status --data-dir reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0
target/debug/postfiat-node history-prune-plan --data-dir reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0
target/debug/postfiat-node history-prune-plan --data-dir reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0 --up-to-height 80 --retain-recent-blocks 20 --minimum-replay-window-blocks 20 --archive-handoff-not-required
RUN_DIR=reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe
SRC=reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/nodes/validator-0
DST=$RUN_DIR/validator-0-pruned
cp -a "$SRC" "$DST"
target/debug/postfiat-node history-archive-handoff-create --data-dir "$DST" --from-height 1 --to-height 80 --archive-uri local-probe --output "$RUN_DIR/archive-handoff-1-80.json" --overwrite
target/debug/postfiat-node history-archive-handoff-verify --data-dir "$DST" --proof-file "$RUN_DIR/archive-handoff-1-80.json"
target/debug/postfiat-node history-prune --data-dir "$DST" --up-to-height 80 --archive-handoff-file "$RUN_DIR/archive-handoff-1-80.json" --retain-recent-blocks 20 --minimum-replay-window-blocks 20
target/debug/postfiat-node verify-blocks --data-dir "$DST"
target/debug/postfiat-node history-status --data-dir "$DST" --retain-recent-blocks 20 --minimum-replay-window-blocks 20
```

Reports:

- `docs/status/latency-append-storage-slice-2026-06-06.md`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/archive-handoff-create.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/archive-handoff-verify.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/history-prune.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/verify-blocks-after-prune.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/history-status-after-prune.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/file-sizes-after-prune.txt`

Result: default `history-prune-plan` correctly refused pruning at height `101`
because the chain is inside the default `50,000` block retention window and no
archive handoff proof was present. A local policy probe with 20-block retention
reported `80` eligible blocks and `prune_allowed: true`. The copied-state prune
passed archive handoff create/verify, prune, and post-prune `verify-blocks`.
The pruned copy retained `21` blocks and preserved tip hash
`d5adb2559fda3990251a5e109e0576a9ea109628302d637bf0faa258f09873c404906c19c2bd08086739823c08657fd9`
and state root
`e47655e0ea5e2f347a52f3b08237dcbbd460c2cab5166e1a8547e24b0fab204b5826cb1a9cdaa8ac9ba63cad562de4e6`.
File sizes dropped from `blocks.json` `3,694,707` bytes to `768,245` bytes,
`batch_archive.json` `1,164,252` bytes to `242,090` bytes, and
`receipts.json` `34,244` bytes to `7,122` bytes, each about `79.2%`.

Decision: do not run live pruning from this WHIP. The evidence supports either
explicit partial-history policy activation or append/snapshot active storage as
the next major latency slice, but live pruning changes operator history posture
and needs its own release gate.

Next: keep current deployed delta-journal improvement; if the run continues,
limit further work to append-storage design/tests or evidence packaging unless
a low-risk local-only measurement slice appears.

### 2026-06-06 05:50 UTC - LAT-024 EVIDENCE PACKET

Hypothesis: the accepted latency claims should be packaged with hashes and
claim boundaries so the run remains auditable without rereading every report.

Change: added a compact evidence packet README and `SHA256SUMS.txt`.

Commands:

```bash
sha256sum reports/testnet-latency-whip/latency-whip-20260606T0530-local100-delta-journal/testnet-tx-finality-latency-benchmark.json reports/testnet-latency-whip/latency-whip-20260606T041922Z-local100-verified-cert/testnet-tx-finality-latency-benchmark.json reports/testnet-latency-whip/latency-whip-20260606T0535-live4-delta-journal-timeout-wrapper/testnet-live-wallet-finality-benchmark.json reports/testnet-latency-whip/latency-whip-20260606T0540-live-doctor-post-live4-delta/testnet-live-validator-doctor.json reports/testnet-latency-whip/latency-whip-20260606T0545-local-prune-size-probe/verify-blocks-after-prune.json docs/status/latency-optimization-whip-2026-06-06.md docs/status/latency-append-storage-slice-2026-06-06.md target/release/postfiat-node
sha256sum -c reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/SHA256SUMS.txt
git diff --check -- reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/README.md reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/SHA256SUMS.txt docs/status/latency-optimization-whip-2026-06-06.md docs/status/latency-append-storage-slice-2026-06-06.md
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/README.md`
- `reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/SHA256SUMS.txt`

Result: packet written and `sha256sum -c` passed for all listed artifacts.
Diff check passed for the packet and docs.

Decision: treat this as the current accepted packet for the sprint so far. It
states the live 25-round limitation explicitly and preserves the local100 and
live4 claim boundaries.

Next: continue only with low-risk append-storage design/tests or additional
evidence checks; do not start an unsafe live pruning or active storage format
change from this slice.

### 2026-06-06 06:04 UTC - LAT-025 CHECKPOINT MERGE FIX

Hypothesis: the compact delta journal must append correctly after a validator
has pruned history and is verifying from `history_checkpoint.json`; otherwise
partial-history validators would reject the next block after prune.

Change: fixed `merge_block_delta` to account for `history_base_height(store)?`
when deciding whether a delta block extends retained history or matches an
already-applied retained block. Rebuilt and redeployed the fixed release binary
`681d424a52e0a5866e3cd2c40be686291136219fd1dbef5956e198c3ec2c4779`.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
cargo test -p postfiat-node history_prune_writes_checkpoint_and_allows_post_prune_block
cargo test -p postfiat-node status_recovers_pending_ordered_commit
cargo test -p postfiat-node
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0602-live-binary-compat-checkpoint-fix \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0602-live-binary-compat-checkpoint-fix/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0602-live-binary-compat-checkpoint-fix/testnet-live-binary-compatibility.json \
scripts/testnet-live-binary-compatibility-check
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0603-live-binary-upgrade-checkpoint-fix \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0603-live-binary-upgrade-checkpoint-fix/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0603-live-binary-upgrade-checkpoint-fix/testnet-live-orchard-binary-upgrade.json \
POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0604-live-doctor-checkpoint-fix \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0604-live-doctor-checkpoint-fix/logs \
REPORT=reports/testnet-latency-whip/latency-whip-20260606T0604-live-doctor-checkpoint-fix/testnet-live-validator-doctor.json \
COMMAND_TIMEOUT_SECONDS=180 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0602-live-binary-compat-checkpoint-fix/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0603-live-binary-upgrade-checkpoint-fix/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0604-live-doctor-checkpoint-fix/testnet-live-validator-doctor.json`

Result: focused history-prune regression test passed, and both ordered-commit
journal recovery tests passed. Full `cargo test -p postfiat-node` returned to
the known fixture/vector failure set: `89` passed and `8` failed
(`governance_agent_*` fixture hashes/lineage audit, replicated state-root
vector, and wallet test vector). The newly introduced history-prune failure was
gone. Live compatibility passed, controlled binary upgrade passed, and
post-upgrade doctor passed with all validators OK, converged, state verified,
history ready, account indexes usable/current, and binary hashes matched.

Decision: keep the checkpoint-aware delta journal fix and the fixed live binary
hash `681d424a52e0a5866e3cd2c40be686291136219fd1dbef5956e198c3ec2c4779`.

Next: refresh the evidence packet hash list, then continue only with bounded
docs/evidence work or low-risk local tests.

### 2026-06-06 06:17 UTC - LAT-026 CURRENT-CODE LOCAL100 GATE

Hypothesis: after the checkpoint-aware merge fix, the compact delta journal
should still deliver the storage-stage improvement on the exact current code
that was redeployed to live, and the resulting local state should pass saved
state/block verification gates.

Change: reran the 100-round local wallet-finality benchmark with the
checkpoint-fixed code, then saved `verify-blocks` and `verify-state` reports
for the same validator state. No live state was changed by this local gate.

Commands:

```bash
RUN_ID=latency-whip-20260606T0608-local100-checkpoint-fix \
ROOT_DIR=reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix \
LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/logs \
scripts/testnet-tx-finality-latency-benchmark
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/nodes/validator-0 > reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/verify-blocks.json
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/nodes/validator-0 > reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/verify-state.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0608-local100-checkpoint-fix/verify-state.json`

Result: the benchmark passed `100/100`. `submit_to_finality` p50/p95/p99 was
`1042.853454ms` / `1305.767585ms` / `1349.507537ms`, compared with the
LAT-018 pre-delta baseline of `1048.2952ms` / `1374.721412ms` /
`1400.337563ms`. The current-code p95 improvement is therefore
`68.953827ms`. The storage substage moved as intended: `write_journal` p95 fell
from `173.887381ms` to `5.976688ms`, `write_commit` p95 fell to
`245.277763ms`, and `local_apply` p95 fell to `317.085261ms`. The remaining
hot write stages are now `write_blocks` p95 `152.9276ms` and
`write_batch_archive` p95 `49.745765ms`.

Saved `verify-blocks` passed with `101` blocks, tip hash
`64b12afa9b4a86e7f40c7f965c2e4d81cc9fff42f06e21e6cb8578e6c28e1f2cef2b73e2a83b9185b7237b89ab5c16e7`,
and state root
`3ec6f02f2c8644551428bc06418c2c11b8bf57e5a0d3b20fad19fe1336ec244dbbd2c4e470d215b880c114f647b20a96`.
Saved `verify-state` passed against the same tip/state root.

Decision: use this checkpoint-fixed `0608` run as the primary local benchmark
in the evidence packet. Keep the earlier `0530` result only as intermediate
lab-book evidence. The accepted headline should be the conservative
current-code p95 improvement of `68.953827ms`, while the larger and more stable
storage-stage claim is the journal p95 drop from `173.887381ms` to
`5.976688ms`.

Next: refresh the evidence packet README and hashes against LAT-026, then
continue with only bounded evidence/doc checks unless a low-risk local-only
append-storage test is identified.

### 2026-06-06 08:38 UTC - LAT-027 APPEND HISTORY HOT PATH

Hypothesis: after compact ordered-commit journals removed the full pending
commit rewrite, the next durable local bottleneck was rewriting retained
`blocks.json` and `batch_archive.json` on every accepted block. Append-backed
active history should preserve the existing JSON compatibility surface while
moving the hot path to one canonical append per new block/archive entry.

Change: added append-log compatibility for `blocks.append.jsonl` and
`batch_archive.append.jsonl`. Reads merge the legacy compact JSON files plus
append logs and fail closed on conflicting duplicate heights or archive ids.
Full writes still compact by writing the legacy JSON file and removing the
append log. Snapshot export materializes merged block/archive state so snapshot
import does not restore from stale compact JSON. `history-status` reports the
append files. The ordered-commit write path now appends new block and archive
entries after verifying they extend the merged in-memory state.

Commands:

```bash
cargo fmt -p postfiat-node -p postfiat-storage --check
python3 -m py_compile scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark scripts/testnet-live-validator-doctor scripts/testnet-live-orchard-binary-upgrade
git diff --check -- crates/node/src/lib_parts/part_03.rs scripts/testnet-live-wallet-finality scripts/testnet-live-validator-doctor
cargo check -p postfiat-node
cargo test -p postfiat-storage
cargo test -p postfiat-node init_then_run_once
cargo test -p postfiat-node status_recovers_pending_ordered_commit_delta_journal
ROUNDS=100 BASE_DIR=reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/nodes LOG_DIR=reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/logs PRIVATE_DIR=reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/private-wallet-material REPORT=reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/testnet-tx-finality-latency-benchmark.json HARNESS_REPORT=reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/logs/local-harness.json scripts/testnet-tx-finality-latency-benchmark
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/nodes/validator-0 > reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/verify-blocks.json
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/nodes/validator-0 > reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/verify-state.json
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0802-local100-append-hotpath-current/verify-state.json`

Result: focused formatting, Python compile, diff, storage tests, node check,
snapshot import, and journal recovery gates passed. The current-code append
local100 benchmark passed `100/100`. `submit_to_finality` p50/p95/p99 was
`1035.660217ms` / `1224.453067ms` / `1251.125052ms`, compared with the
checkpoint-fixed full-history-write run at `1042.853454ms` / `1305.767585ms` /
`1349.507537ms`. The append hot path therefore improved current-code local p95
by `81.314518ms` over LAT-026 and by `150.268345ms` over the LAT-018
verified-cert baseline.

The target write stages moved as intended: `write_blocks` p95 dropped from
`152.9276ms` to `27.438267ms`, and `write_batch_archive` p95 dropped from
`49.745765ms` to `9.06305ms`. `write_commit` p95 dropped from `245.277763ms`
to `80.08058ms`; local apply p95 dropped from `317.085261ms` to
`153.586098ms`. The resulting append files were `blocks.append.jsonl`
`3,602,473` bytes and `batch_archive.append.jsonl` `1,159,988` bytes, with
compact legacy files remaining as empty JSON arrays until the next full
compaction.

`verify-blocks` passed with `101` blocks, tip hash
`a7a3dd2fb4c7d123ff8dc1585cc1d6621c845a9130ee4361c0b3e368db3d4559a21e3ad89937ccad21025acab8ff3259`,
and state root
`fa3d4ac9613c4c6e5160e7b25dd86dcb484cca6383beba48ac4359f80be88af70420dae33291e5e5d162703cefafd3f0`.
`verify-state` passed against the same block log.

Decision: promote append-backed active history to live only after a corrected
six-validator live gate. Keep read compatibility and compaction behavior as
part of the storage contract. The next local storage bottlenecks are vote and
proposal construction/network request time, not block/archive full rewrites.

### 2026-06-06 08:39 UTC - LAT-028 LIVE ACTIVE-SET COVERAGE FIX

Hypothesis: the failed earlier append live run might have been a harness/live
coverage issue rather than a consensus-storage issue, because the error named
`validator-5` while the upgrade and doctor loops had been using the old
five-validator default.

Change: diagnosed the live topology and found the controlled testnet topology
contains validators `0..6`, while active node status reports
`validator_count: 6`. `validator-6` is present in topology but inactive and
behind; `validator-5` is active and was omitted by the old default live doctor
and binary-upgrade loops. Updated `scripts/testnet-live-wallet-finality` to
default to six validators, and added
`requested_validator_count_matches_active_state` to
`scripts/testnet-live-validator-doctor`.

Commands:

```bash
VALIDATORS=7 scripts/testnet-live-validator-doctor
VALIDATORS=6 scripts/testnet-live-binary-compatibility-check
VALIDATORS=6 POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=6 scripts/testnet-live-wallet-finality
VALIDATORS=6 ROUNDS=4 scripts/testnet-live-wallet-finality-benchmark
scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0720-live-transport-diagnostics/live-transport-diagnostics.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0722-live-round-error-heads/live-round-error-heads.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0725-live-doctor-7-preupgrade/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0734-live-doctor-6-validator-restart/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0736-live1-direct-6-validator-restart/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0740-live4-6-validator-restart/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0752-live-doctor-default-6-harness-guard/testnet-live-validator-doctor.json`

Result: the seven-validator doctor failed because `validator-6` was inactive
and at height `4`, while validators `0..5` were converged at height `179`.
The failed live round error was exactly `transport block vote request to
validator-5 failed after 17 attempts`; it was not an append-storage validation
error. A six-validator compatibility check passed, the controlled six-validator
restart passed, six-validator doctor passed, and the direct live canary passed
from height `179` to `181`. The six-validator rollback/full-write live4 passed
`4/4` with `submit_to_finality` p50/p95/p99 `704.79038ms` / `751.342891ms` /
`751.342891ms`. The default doctor then passed with the new active-count guard.

Decision: all future live latency gates in this sprint use the six active
validators. The older five-validator live doctors are not valid evidence for
whole-active-set health.

### 2026-06-06 08:40 UTC - LAT-029 APPEND HOT PATH LIVE PROMOTION

Hypothesis: with the six-validator active set correctly covered, the append
hot path should deploy cleanly and improve live submit-to-finality latency.

Change: built and deployed release binary
`11d5e332806fe8acb42d755a06fffbc669623498ba6758d26fc5f2299a37d18f`
to validators `0..5`.

Commands:

```bash
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node
VALIDATORS=6 scripts/testnet-live-binary-compatibility-check
VALIDATORS=6 POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
scripts/testnet-live-validator-doctor
scripts/testnet-live-wallet-finality
ROUNDS=4 scripts/testnet-live-wallet-finality-benchmark
scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0812-live-binary-compat-append-hotpath-6/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0814-live-binary-upgrade-append-hotpath-6/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0816-live-doctor-append-hotpath-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0818-live1-append-hotpath-6/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0822-live4-append-hotpath-6/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0832-live-doctor-post-live4-append-hotpath-6/testnet-live-validator-doctor.json`

Result: live compatibility passed, controlled six-validator upgrade passed,
pre-write doctor passed, direct canary passed from height `189` to `191`, and
post-live4 doctor passed with validators `0..5` converged at height `199`.
The final doctor checks were all true, including services active, state
verified, history ready, account indexes usable/current, binary hashes matched,
and `requested_validator_count_matches_active_state`.

The append-hotpath live4 passed `4/4`. `submit_to_finality` p50/p95/p99 was
`637.311699ms` / `683.596389ms` / `683.596389ms`. `certified_round`
p50/p95/p99 was `572.343049ms` / `615.567785ms` / `615.567785ms`.
Spend-side client-visible finality p50/p95/p99 was `570.004588ms` /
`612.52162ms` / `612.52162ms`.

Decision: keep the append-hotpath binary live. The accepted live headline is a
six-active-validator `4/4` aggregate, not a full 25-round live distribution.
The accepted local headline is current-code local100 p95 `1224.453067ms`.

### 2026-06-06 08:04 UTC - LAT-030 FULL NODE SUITE AUDIT

Hypothesis: the append-backed history hot path should not introduce new
storage, history, snapshot, ordered-commit, or Orchard verifier regressions
beyond the repo's pre-existing governance/test-vector fixture failures.

Change: no code change. Re-ran the full `postfiat-node` test suite into a
hashable log after live append-hotpath promotion.

Commands:

```bash
mkdir -p reports/testnet-latency-whip/latency-whip-20260606T0804-full-node-suite-audit
cargo test -p postfiat-node 2>&1 | tee reports/testnet-latency-whip/latency-whip-20260606T0804-full-node-suite-audit/cargo-test-postfiat-node.log
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0804-full-node-suite-audit/cargo-test-postfiat-node.log`

Result: the full suite completed with the known residual result:
`89` passed and `8` failed. The failures were the governance-agent
fixture/lineage tests plus two deterministic vector tests:
`governance_agent_gate_3_5_accepts_identical_ruleset_outputs`,
`governance_agent_gate_10_1_measures_verifier_cost_on_postfiat_artifacts`,
`governance_agent_gate_10_5_records_compact_receipt_and_verifier_outcomes`,
`governance_agent_gate_14_keeps_tp_greater_than_one_out_of_admission`,
`governance_agent_gate_15_rejects_adversarial_governance_escalation`,
`governance_agent_evidence_lineage_audit_rejects_report_drift`,
`replicated_state_root_commits_to_chain_domain`, and
`wallet_test_vector_is_deterministic_and_redacted`.

The latency-adjacent gates passed inside the full suite, including
`status_recovers_pending_ordered_commit_delta_journal`,
`status_recovers_pending_ordered_commit_journal`,
`history_prune_recover_completes_pending_prune_after_checkpoint_write`,
`history_prune_writes_checkpoint_and_allows_post_prune_block`,
`snapshot_import_rejects_bad_manifest_file_set`,
`verify_blocks_rejects_tampered_genesis_faucet_account`,
`verify_blocks_replays_historical_registry_after_live_key_rotation`,
`init_then_run_once`, `orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note`,
and `orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers`.

Decision: accept the full-suite audit as residual-risk evidence for this slice:
there is no new observed storage/history/snapshot/ordered-commit regression,
but the repo still has pre-existing governance/test-vector fixture drift that
keeps the full node suite red.

Next: shift the next latency slice away from block/archive persistence and
toward the post-append p95 drivers: vote/proposal transport and local vote
construction.

### 2026-06-06 08:21 UTC - LAT-031 LOCAL VOTE FAST PATH

Hypothesis: the source validator's local vote in
`transport-peer-certified-batch-round` was rebuilding and revalidating the
same proposal that the source had just constructed and signed. Reusing the
in-memory, already-built proposal for the local vote should reduce local vote
p95 without weakening remote validator checks, certificate aggregation, local
apply, or replay.

Change: factored block-vote signing behind `create_block_vote_for_target` and
added `create_block_vote_for_verified_proposal` for the internal source
validator path. The ordinary `block-vote` CLI path and all remote
`transport-block-vote-request` handlers still call `create_block_vote`, which
reconstructs the expected proposal from local batch and state before signing.
The new path validates proposal domain/signature/height and active validator
membership, then signs the vote target for the proposal the same process just
built.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
cargo test -p postfiat-node split_block_votes_reconstruct_certificate
cargo test -p postfiat-node proposal_certificate_accepts_three_of_four_bft_quorum
cargo test -p postfiat-node signed_block_proposals_verify_before_votes
cargo test -p postfiat-node init_then_run_once
ROUNDS=25 REPORT=reports/testnet-latency-whip/latency-whip-20260606T0810-local25-local-vote-fastpath/testnet-tx-finality-latency-benchmark.json scripts/testnet-tx-finality-latency-benchmark --rounds 25
ROUNDS=100 REPORT=reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/testnet-tx-finality-latency-benchmark.json scripts/testnet-tx-finality-latency-benchmark --rounds 100
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/nodes/validator-0
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/nodes/validator-0
scripts/testnet-transport-peer-certified-quorum-early-smoke
git diff --check -- crates/node/src/node_types.rs crates/node/src/block_finality.rs crates/node/src/main_parts/cli_dispatch.rs crates/node/src/transport_cli.rs
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0810-local25-local-vote-fastpath/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0815-local100-local-vote-fastpath/verify-state.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0821-quorum-early-local-vote-fastpath/testnet-transport-peer-certified-quorum-early.json`

Result: focused formatting, check, proposal-vote/certificate tests, and
`init_then_run_once` passed. The local25 benchmark passed `25/25` with
`submit_to_finality` p50/p95/p99 `739.397019ms` / `795.179249ms` /
`883.872521ms`; local25 convergence and state verification were true.

The local100 benchmark passed `100/100` with `submit_to_finality` p50/p95/p99
`830.47189ms` / `968.243657ms` / `999.667316ms`, improving p95 by
`256.20941ms` versus LAT-027 append-hotpath p95 `1224.453067ms` and by
`406.477755ms` versus LAT-018 verified-cert p95 `1374.721412ms`.
`local_vote` p95 dropped from `312.208144ms` to `148.856946ms`, and
`vote_requests` p95 dropped from `452.635845ms` to `332.430555ms`. Saved
`verify-blocks` and `verify-state` both passed for validator-0 at `101`
blocks, tip hash
`36995a1ca9209c9f1ea19f053e58e3e6fa638586f333917aa60b9fb595b05db3f74885cbc86840fa9c546959a706c05f`,
and state root
`c87bfb4e3634bef0e0b801033d18e1b8579727c2b020d92d5ec1b51dffa4e07487150483e04718c8b04eb0c0f2bdd7b0`.

The slow-peer quorum-early smoke passed with quorum `4`, vote count `4`, slow
node `validator-4` unresolved/skipped, online nodes converged, services
verified, and all state verified.

Decision: accept the local-vote fast path for live promotion. The claim boundary
is narrow: this optimizes only the source validator's own vote after it has
created the proposal. Remote validators still independently reconstruct and
verify proposal-vs-batch/state before voting.

Next: build release, deploy to the six-active-validator live fleet, then run
compatibility, doctor, direct canary, and live distribution gates.

### 2026-06-06 09:22 UTC - LAT-032 INCREMENTAL ACCOUNT-TX CACHE

Hypothesis: after the local-vote fast path, live certified-round latency was
dominated by local apply, specifically the rebuildable `account_tx` operator
cache. The disk cache writer was rewriting every account shard on every block.
An incremental shard update should reduce live write-commit latency while
preserving the monolithic index for existing doctor compatibility and falling
back to the old full rewrite whenever the cache extension cannot be proven.

Change: extended account-tx index refresh to track accounts touched by newly
committed rows. The hot path still writes the monolithic `account_tx_index.json`
so existing status/doctor checks stay current, but the disk-shard writer now
rewrites only touched account shards and updates the disk-index metadata. Disk
metadata now stores a per-account shard tip hash, so an untouched shard remains
valid only when the current disk-index metadata explicitly points at that older
shard version. Missing, incompatible, or non-extension disk state falls back to
the original full disk-shard rewrite.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
cargo test -p postfiat-node account_tx_index_auto_refresh_catches_up_after_archive_prune
cargo test -p postfiat-node payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx
cargo test -p postfiat-node asset_transactions_apply_from_batch_replay_and_account_tx
cargo test -p postfiat-node init_then_run_once
ROUNDS=25 REPORT=reports/testnet-latency-whip/latency-whip-20260606T0918-local25-account-tx-incremental/testnet-tx-finality-latency-benchmark.json scripts/testnet-tx-finality-latency-benchmark --rounds 25
ROUNDS=100 REPORT=reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/testnet-tx-finality-latency-benchmark.json scripts/testnet-tx-finality-latency-benchmark --rounds 100
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/nodes/validator-0
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/nodes/validator-0
target/debug/postfiat-node account-tx-index-status --data-dir reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/nodes/validator-0
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0918-local25-account-tx-incremental/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0921-local100-account-tx-incremental/verify-state.json`

Result: focused account-tx, payment/account-tx, asset/account-tx, and
`init_then_run_once` tests passed. Local25 passed `25/25` with
`submit_to_finality` p50/p95/p99 `682.706429ms` / `742.604398ms` /
`749.726668ms`, and account-tx refresh p95 `25.287088ms`.

Local100 passed `100/100` with `submit_to_finality` p50/p95/p99
`794.831981ms` / `972.646164ms` / `978.907793ms`. This was flat on headline
p95 versus LAT-031 (`968.243657ms`) because vote-request RTT dominated the
tail, but it reduced the intended cache stage: `account_tx_refresh` p95 was
`23.884156ms`. Saved `verify-blocks` and `verify-state` passed for
validator-0 at `101` blocks, tip hash
`0a5dc99c16780add1040695bd16da9a6bec9a3e1ec5853631a7db5f936942baa1a44e8a09030ba4aa7a7fe8698ba6d87`,
and state root
`2fb49d304d9f17d8653b1f8d1da34488a9e8d9a1ac3ac449e4288a2107816e804259f3c01a60e2382818d787dd5cd4b9`.
`account-tx-index-status` reported both the monolithic and disk index usable
and current at the same tip.

Decision: accept for live testing because the live LAT-031 distribution showed
`account_tx_refresh` p95 `329.20825ms`, much larger than local. Do not claim a
local headline p95 improvement from this slice alone.

Next: deploy to live only through compatibility, controlled upgrade, doctor,
canary, and distribution gates.

### 2026-06-06 10:02 UTC - LAT-033 LIVE ACCOUNT-TX CACHE PROMOTION

Hypothesis: incremental account-tx disk-shard refresh should remove the live
local-apply bottleneck observed after the local-vote fast path and materially
improve six-validator live distribution latency.

Change: built and deployed release binary
`48a793d3c1512fc6ec1e1bb3b00e933e3267648ee09a3e07be1ce24c10d11e26`
to validators `0..5`.

Commands:

```bash
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node
VALIDATORS=6 scripts/testnet-live-binary-compatibility-check
VALIDATORS=6 POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=6 scripts/testnet-live-wallet-finality
VALIDATORS=6 ROUNDS=4 scripts/testnet-live-wallet-finality-benchmark
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=6 ROUNDS=25 scripts/testnet-live-wallet-finality-benchmark
VALIDATORS=6 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T0927-live-binary-compat-account-tx-incremental-6/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0930-live-binary-upgrade-account-tx-incremental-6/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0932-live-doctor-account-tx-incremental-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0934-live1-account-tx-incremental-6/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0937-live4-account-tx-incremental-6/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0942-live-doctor-post-live4-account-tx-incremental-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T0944-live25-account-tx-incremental-6/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T1001-live-doctor-post-live25-account-tx-incremental-6/testnet-live-validator-doctor.json`

Result: compatibility passed, controlled upgrade passed, pre-write doctor
passed, and direct canary passed from height `251` to `253`. The first live4
aggregate passed `4/4` with `submit_to_finality` p50/p95/p99
`400.341649ms` / `458.650553ms` / `458.650553ms`, improving materially over
the LAT-029 append-hotpath live4 p50/p95/p99 `637.311699ms` / `683.596389ms`
/ `683.596389ms`. In that live4, `account_tx_refresh` p95 was `14.107503ms`
and local apply p95 was `85.776251ms`.

The full live25 confirmation passed `25/25` with `submit_to_finality`
p50/p95/p99 `431.75899ms` / `471.237623ms` / `516.717459ms`. Certified-round
p50/p95/p99 was `368.497512ms` / `410.177368ms` / `440.37643ms`, and
spend-side client-visible finality p50/p95/p99 was `366.244989ms` /
`408.17694ms` / `438.169648ms`.

The live25 stage breakdown shows `account_tx_refresh` p50/p95/p99
`12.47547ms` / `15.470176ms` / `23.2221ms`, down from the LAT-031 live25
post-local-vote-fastpath p95 `329.20825ms`. The remaining largest live stage
is vote-request RTT: p50/p95/p99 `213.051045ms` / `237.294271ms` /
`250.735652ms`.

Final post-live25 doctor passed with services active, binary hashes matched,
state verified, history ready, account-tx monolithic and disk indexes usable
and current, validators converged, and
`requested_validator_count_matches_active_state` true.

Decision: keep binary
`48a793d3c1512fc6ec1e1bb3b00e933e3267648ee09a3e07be1ce24c10d11e26` live.
The accepted live distribution claim is now the full six-validator live25
p50/p95/p99 `431.75899ms` / `471.237623ms` / `516.717459ms`. The next
bottleneck is vote-request RTT/fanout, not local write persistence.

### 2026-06-06 10:35 UTC - LAT-034 QUORUM-EARLY FULL PROPAGATION

Hypothesis: the remaining vote-request RTT/fanout tail could be reduced by
forming a BFT quorum certificate after the source validator receives the
required remote votes, while still sending the certified block to every active
peer. This should preserve convergence better than the existing
`--allow-peer-failures` quorum-early path, which skips unresolved certified
send targets.

Change: added default-off `--quorum-early-full-propagation` for peer-certified
batch, mempool, and batch-loop rounds, plus
`--finality-quorum-early-full-propagation` for the controlled RPC finality
write edge. In this mode vote collection may stop at quorum, but
`certified_send_targets` remains the full active peer set and round success
requires full certified-send coverage. Existing `--allow-peer-failures`
semantics are unchanged.

Commands:

```bash
cargo fmt -p postfiat-node --check
cargo check -p postfiat-node
bash -n scripts/testnet-tx-finality-latency-benchmark scripts/node-run-peer-certified
python3 -m py_compile scripts/testnet-live-wallet-finality scripts/testnet-live-wallet-finality-benchmark
cargo test -p postfiat-node split_block_votes_reconstruct_certificate
cargo test -p postfiat-node proposal_certificate_accepts_three_of_four_bft_quorum
cargo test -p postfiat-node signed_block_proposals_verify_before_votes
cargo test -p postfiat-node init_then_run_once
VALIDATORS=6 QUORUM_EARLY_FULL_PROPAGATION=1 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
VALIDATORS=6 QUORUM_EARLY_FULL_PROPAGATION=0 ROUNDS=25 scripts/testnet-tx-finality-latency-benchmark --rounds 25
VALIDATORS=6 QUORUM_EARLY_FULL_PROPAGATION=1 ROUNDS=100 scripts/testnet-tx-finality-latency-benchmark --rounds 100
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/nodes/validator-0
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/nodes/validator-0
target/debug/postfiat-node account-tx-index-status --data-dir reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/nodes/validator-0
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node
VALIDATORS=6 scripts/testnet-live-binary-compatibility-check
VALIDATORS=6 POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 scripts/testnet-live-orchard-binary-upgrade
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=6 QUORUM_EARLY_FULL_PROPAGATION=1 scripts/testnet-live-wallet-finality
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=6 QUORUM_EARLY_FULL_PROPAGATION=1 ROUNDS=4 scripts/testnet-live-wallet-finality-benchmark
VALIDATORS=6 scripts/testnet-live-validator-doctor
VALIDATORS=5 scripts/testnet-transport-peer-certified-quorum-early-smoke
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T101056Z-local25-quorum-early-full-prop/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T101318Z-local25-quorum-control/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/verify-state.json`
- `reports/testnet-latency-whip/latency-whip-20260606T101502Z-local100-quorum-early-full-prop/account-tx-index-status.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102415Z-live-binary-compat-quorum-early-full-prop-6/testnet-live-binary-compatibility.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102502Z-live-binary-upgrade-quorum-early-full-prop-6/testnet-live-orchard-binary-upgrade.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102551Z-live-doctor-quorum-early-full-prop-precanary-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102631Z-live1-quorum-early-full-prop-6/testnet-live-wallet-finality.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102823Z-live-doctor-post-live1-quorum-early-full-prop-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T102905Z-live4-quorum-early-full-prop-6/testnet-live-wallet-finality-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T103441Z-live-doctor-post-live4-quorum-early-full-prop-6/testnet-live-validator-doctor.json`
- `reports/testnet-latency-whip/latency-whip-20260606T104014Z-quorum-early-regression-after-full-prop/testnet-transport-peer-certified-quorum-early.json`

Result: local25 six-validator opt-in passed with p50/p95/p99
`683.292494ms` / `740.153193ms` / `767.021173ms`, improving over the
same-code no-flag six-validator control p50/p95/p99 `751.38928ms` /
`817.297523ms` / `837.667012ms`. The opt-in local25 used 5-of-6 quorum
certificates in all 25 rounds, marked `vote_request_quorum_early=true`, left
zero skipped certified-send targets, and launched five deferred certified-send
jobs per round.

Local100 opt-in passed with p50/p95/p99 `839.862601ms` / `986.67944ms` /
`1003.791389ms`. All 100 rounds used 5-of-6 quorum certificates, had one
unresolved vote target, skipped zero certified-send targets, and launched five
deferred certified-send jobs. Saved `verify-blocks` and `verify-state` passed
for validator-0 at `101` blocks; account-tx status reported monolithic and
disk indexes usable with `tip_hash == current_tip_hash`.

Live compatibility, controlled upgrade, pre-canary doctor, opt-in direct
canary, post-canary doctor, opt-in live4, and post-live4 doctor all passed on
the six-active-validator fleet. The deployed binary hash is
`6ab2be57071b1ffff3db1afc003e71c927dba2ca07238a65827e9eb50fc2c1ef`.
The live opt-in canary formed 5-of-6 certificates with zero skipped certified
send targets. The spend/finality round launched five deferred certified-send
jobs and returned `submit_finality.total_ms` `412.22129ms`.

The live4 opt-in distribution did not beat the accepted live baseline:
p50/p95/p99 was `445.021374ms` / `519.838354ms` / `519.838354ms`, worse than
the LAT-033 live4 account-tx-incremental p95 `458.650553ms` and the accepted
LAT-033 live25 p95 `471.237623ms`. Stage data showed the mode worked
mechanically in all live4 rounds, but one round still had higher vote RTT and
local apply (`vote_requests_ms` `233.391053ms`, `local_apply_ms`
`123.464681ms`).

The existing slow-peer `--allow-peer-failures` quorum-early smoke also passed
after the new mode was added, preserving the old skipped-target semantics for
that explicitly allowed-failure path.

Decision: keep the binary live because the new mode is default-off and all
doctors passed, but do not promote `--quorum-early-full-propagation` as the
accepted live latency path and do not run a full live25 opt-in benchmark. The
accepted live distribution remains LAT-033 p50/p95/p99 `431.75899ms` /
`471.237623ms` / `516.717459ms`. The new flag is retained as an experimental
transport mode with local evidence and a live canary, pending a better live
distribution or a more precise RTT/fanout design.

Next: do not spend more live writes on this mode tonight. Package the evidence,
rerun checks, and leave the next live optimization as targeted vote-request
transport RTT reduction rather than quorum threshold alone.

### 2026-06-06 10:38 UTC - LAT-035 VOTE REQUEST TARGET RTT READOUT

Hypothesis: the remaining live vote-request tail may be explained by one
specific validator target or by a general per-round request path.

Change: no code change. Read the accepted LAT-033 default live25 spend/finality
rounds and the LAT-034 quorum-early live4 spend/finality rounds, grouping
per-target vote-request timings and unresolved targets.

Commands:

```bash
python3 - <<'PY'
# Read accepted live25 and opt-in live4 canary reports; group spend-round
# vote_request_targets by validator and unresolved_vote_targets by count.
PY
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T1038-vote-target-rtt/README.md`

Result: in the accepted default live25, all `25` spend/finality rounds
collected `6/6` votes. Per-target p95 request times were all in the
`217.796955ms` to `243.703175ms` band; `validator-5` was the slowest observed
target but not by enough to explain the whole tail by itself. In the
quorum-early full-propagation live4, all four spend/finality rounds formed
`5/6` certificates, but unresolved targets rotated across `validator-2`,
`validator-4`, and `validator-5`.

Decision: do not treat a single-validator restart or exclusion as the next
latency fix. The next credible live optimization is reducing per-round request
setup/RTT cost itself: connection reuse, persistent vote listeners, smaller
request framing, or a push/gossip vote path that retains remote proposal
reconstruction and full certified propagation.

Next: after hour 9, avoid new high-risk transport code. Finish packet
verification and leave the persistent vote-request transport design as the next
work item.

### 2026-06-06 10:42 UTC - LAT-036 PERSISTENT VOTE TRANSPORT DESIGN

Hypothesis: quorum threshold alone is not enough; the next live win requires
reducing the per-round request setup cost while preserving remote vote
reconstruction and full certified propagation.

Change: no code change. Wrote the next-slice design for a default-off,
round-scoped persistent vote transport. The design keeps one-shot transport as
the fallback, keeps certificate aggregation unchanged, and requires the remote
validator to continue reconstructing proposal-vs-batch/state before signing.

Commands:

```bash
sed -n '1560,1748p' crates/node/src/transport_cli.rs
sed -n '1160,1285p' crates/node/src/transport_cli.rs
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T1042-persistent-vote-transport-design/README.md`

Result: design artifact created. The current hot path opens a fresh TCP
connection per target per round; the proposed slice is a round-scoped
persistent vote service/connection pool that keeps existing proposal
verification and response validation intact.

Decision: do not implement this after hour 9 in the current sprint. This is the
next coherent transport optimization target for a fresh implementation window.

Next: wrap-up checks unless a low-risk measurement remains.

### 2026-06-06 10:49 UTC - LAT-037 POST-TRANSPORT FULL SUITE AUDIT

Hypothesis: adding default-off quorum-early full propagation should not create
new storage, history, Orchard, certificate, proposal, or wallet-flow failures
beyond the repo's known governance/test-vector fixture drift.

Change: no code change. Re-ran the full `postfiat-node` test suite after the
LAT-034 transport changes and saved the log.

Commands:

```bash
mkdir -p reports/testnet-latency-whip/latency-whip-20260606T1043-full-node-suite-after-quorum-early
cargo test -p postfiat-node 2>&1 | tee reports/testnet-latency-whip/latency-whip-20260606T1043-full-node-suite-after-quorum-early/cargo-test-postfiat-node.log
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T1043-full-node-suite-after-quorum-early/cargo-test-postfiat-node.log`

Result: suite completed with the same known residual result as LAT-030:
`89` passed and `8` failed. The failures were
`governance_agent_gate_3_5_accepts_identical_ruleset_outputs`,
`governance_agent_gate_10_1_measures_verifier_cost_on_postfiat_artifacts`,
`governance_agent_gate_10_5_records_compact_receipt_and_verifier_outcomes`,
`governance_agent_gate_14_keeps_tp_greater_than_one_out_of_admission`,
`governance_agent_gate_15_rejects_adversarial_governance_escalation`,
`governance_agent_evidence_lineage_audit_rejects_report_drift`,
`replicated_state_root_commits_to_chain_domain`, and
`wallet_test_vector_is_deterministic_and_redacted`.

Latency-adjacent gates passed, including proposal/vote/certificate tests,
account-tx refresh, payment/asset account-tx flows, ordered-commit journal
recovery, history prune/recovery, snapshot tamper rejection,
`init_then_run_once`, and both Orchard tests.

Decision: accept as a residual-risk audit. The full suite remains red for the
known fixture set, but the transport slice did not add a new observed
latency-adjacent test failure.

Next: no more high-risk implementation before hour 9. Continue with packaging
and final doctor/checks.

### 2026-06-06 10:51 UTC - LAT-038 FINAL LIVE DOCTOR

Hypothesis: after the default-off transport flag deployment, opt-in live
canary/live4, and post-transport test audit, the six-active-validator live
fleet should still be healthy and converged.

Change: no code change. Ran final validator doctor.

Commands:

```bash
VALIDATORS=6 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T105040Z-live-doctor-final-wrap-6/testnet-live-validator-doctor.json`

Result: final doctor passed.

Decision: live fleet remains healthy on binary
`6ab2be57071b1ffff3db1afc003e71c927dba2ca07238a65827e9eb50fc2c1ef`.
The quorum-early full-propagation mode remains default-off and not promoted as
the accepted live latency path.

Next: final packet verification and stop starting implementation work.

### 2026-06-06 11:02 UTC - LAT-039 CURRENT DEFAULT LOCAL100

Hypothesis: after adding the default-off quorum-early full-propagation mode,
the default path should still pass local 100-round regression and preserve the
same driver shape: vote-request RTT/fanout as the dominant p95 stage.

Change: no code change. Ran current-code local100 with
`QUORUM_EARLY_FULL_PROPAGATION=0`.

Commands:

```bash
VALIDATORS=6 ROUNDS=100 QUORUM_EARLY_FULL_PROPAGATION=0 scripts/testnet-tx-finality-latency-benchmark --rounds 100
target/debug/postfiat-node verify-blocks --data-dir reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/nodes/validator-0
target/debug/postfiat-node verify-state --data-dir reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/nodes/validator-0
target/debug/postfiat-node account-tx-index-status --data-dir reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/nodes/validator-0
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/verify-blocks.json`
- `reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/verify-state.json`
- `reports/testnet-latency-whip/latency-whip-20260606T105539Z-local100-default-after-quorum-early/account-tx-index-status.json`

Result: current-code default local100 passed `100/100` with
`submit_to_finality` p50/p95/p99 `909.77186ms` / `1092.556454ms` /
`1114.396247ms`. Verification gates passed. Stage p95s were
`vote_requests` `381.881881ms`, `local_vote` `174.645601ms`,
`certificate` `168.355875ms`, and `local_apply` `157.388626ms`.

Decision: accept as a default-path regression/driver measurement, not as an
improvement claim. It reinforces LAT-035/LAT-036: the next implementation
target is persistent/round-scoped vote transport, not further local write-path
work.

Next: hour 9 has passed; wrap-up only.

### 2026-06-06 11:05 UTC - FINAL PACKET VERIFICATION

Hypothesis: the evidence packet should be hash-verifiable after the final
default-path measurement and should not contain whitespace/diff hygiene issues
in the touched source, script, lab-book, or report files.

Change: no code change. Ran final manifest and diff hygiene checks.

Commands:

```bash
sha256sum -c reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/SHA256SUMS.txt
git diff --check -- crates/node/src/transport_cli.rs crates/node/src/main_parts/cli_dispatch.rs crates/node/src/main_parts/runtime_helpers.rs crates/node/src/rpc_cli.rs scripts/node-run-peer-certified scripts/testnet-tx-finality-latency-benchmark scripts/testnet-live-wallet-finality docs/status/latency-optimization-whip-2026-06-06.md reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/README.md reports/testnet-latency-whip/latency-whip-20260606T0550-evidence-packet/SHA256SUMS.txt reports/testnet-latency-whip/latency-whip-20260606T1038-vote-target-rtt/README.md reports/testnet-latency-whip/latency-whip-20260606T1042-persistent-vote-transport-design/README.md
```

Result: manifest verification passed for every packet artifact and
`git diff --check` reported no issues.

Decision: accept packet packaging. No further implementation work should start
inside the final hour.

### 2026-06-06 11:38 UTC - CUTOFF-WINDOW LIVE DOCTOR

Hypothesis: the live fleet should remain healthy during the cutoff window after
the final accepted latency deployment and the default-off quorum transport
binary deployment.

Change: no code change. Ran one more six-validator live doctor.

Commands:

```bash
VALIDATORS=6 scripts/testnet-live-validator-doctor
```

Reports:

- `reports/testnet-latency-whip/latency-whip-20260606T113713Z-live-doctor-cutoff-watch-6/testnet-live-validator-doctor.json`

Result: doctor passed. The report shows all validator checks passing,
convergence at block height `321`, and a matching live binary hash
`6ab2be57071b1ffff3db1afc003e71c927dba2ca07238a65827e9eb50fc2c1ef`.

Decision: accept as the final pre-cutoff live health readout. Continue to wait
for the scheduled `12:09 UTC` cutoff and then verify the stop state.

### 2026-06-06 12:11 UTC - WHIP STOP VERIFIED

Hypothesis: the 10-hour WHIP should stop at the scheduled cutoff and leave no
queued `at` job or active WHIP cron block behind.

Change: no code change. Verified scheduler state after the cutoff.

Commands:

```bash
date -u +%Y-%m-%dT%H:%M:%SZ
atq
crontab -l
tail -n 80 $CODEX_WHIP_STATE/l1.log
```

Result: time was `2026-06-06T12:11:33Z`; `atq` was empty; `crontab -l` was
empty; the WHIP log showed minute-by-minute no-action monitoring through
`2026-06-06T12:08:01Z` while the Codex pane was active.

Decision: accept the 10-hour cutoff as verified. The sprint is closed; do not
restart WHIP or open additional latency work in this run.
