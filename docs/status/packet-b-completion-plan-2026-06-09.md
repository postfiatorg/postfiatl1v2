# Packet B Completion Plan - Multi-Host Post Fiat / Private XRPL Latency

Date: 2026-06-09 UTC
Status: execution plan from current repo state
Related:

- `docs/status/packet-b-multihost-postfiat-xrpl-latency-plan-2026-06-09.md`
- `docs/status/packet-b-multihost-lab-2026-06-09.md`
- `content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md` in `postfiatorg.github.io`

## Goal

Build Packet B: a publishable, reproducible multi-host latency evidence packet
that runs the same six-validator native-transfer workload across three
project-controlled remote machines for Post Fiat and private `rippled`.

The packet exists to answer one criticism:

```text
The current latency article is mainly a single-host/loopback packet. Show the
same Post Fiat path and private XRPL controls across separated remote hosts.
```

The packet must not claim public-mainnet performance, independent operator
diversity, or a perfect same-finality-surface comparison. The claim boundary is:

```text
controlled three-machine topology, six validators, sequential native-transfer
workload, Post Fiat certified applied-batch receipt surface versus private XRPL
validated-ledger inclusion.
```

## Current State

Already done:

- remote credentials and six-validator topology preflight passed;
- Post Fiat remote smoke passed with reset state;
- Post Fiat full-vote and quorum-fast smoke passed;
- private stock `rippled` remote smoke passed;
- private `rippled close_750ms` remote smoke passed;
- `scripts/xrpl-private-control-multihost-benchmark` exists and works;
- `scripts/testnet-remote-ssh-smoke` supports clean state reset and quorum-fast
  for `normal-run`;
- `scripts/testnet-remote-ssh-smoke` and `scripts/testnet-provision-bundle`
  now support quorum-fast for the batched remote loop path;
- optimized short-matrix session 01 passed for all four required latency lanes;
- optimized short-matrix session 02 passed for all four required latency lanes;
- session 02 hashes are recorded in
  `reports/packet-b-multihost-latency/SHORT_MATRIX_OPTIMIZED_SHA256SUMS.txt`.

In progress / next:

- optimized short-matrix session 03 is the next execution step;
- after session 03, validate the full 3x200 short matrix, update hashes and lab
  notes, run the secret scan, and then run the safety gate;
- only after that should the 5x1000 publication matrix start.

Important execution findings:

- `normal-run` creates one SSH/sudo command per height and then polls status.
  The initial 200-round compatibility lane passed, but wall-clock runtime was
  dominated by orchestration.
- `REMOTE_TRANSPARENT_DRIVER=loop` is not the correct publication driver for
  proposer-routed evidence. A single-node loop fails proposer-locality checks
  when deterministic proposer rotation expects another validator. This is a
  useful safety finding: the binary rejects the wrong local proposer.
- Scaled Post Fiat lanes should use proposer-routed `normal-run` with
  `REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0`. That keeps signed local proposer
  execution for each height and retains final verify-state, while removing six
  remote status SSH calls after every round.
- The XRPL multihost harness spends meaningful wall time collecting `rippled`
  debug logs after the measured loop completes. This is artifact overhead, not
  latency. The harness now supports `--log-policy full|stdio|none`; use
  `--log-policy stdio` for 5x1000 so summary reports and bounded stdout/stderr
  logs are retained without fetching full debug logs.
- The first 5x1000 Post Fiat full-vote attempt reached the final verification
  phase but failed because `verify-state` exceeded the generic SSH timeout.
  The remote smoke harness now exposes `REMOTE_VERIFY_STATE_TIMEOUT_SECONDS`;
  use `REMOTE_VERIFY_STATE_TIMEOUT_SECONDS=900` for full-matrix Post Fiat
  lanes.
- Full-matrix session 01 Post Fiat full-vote passed after the timeout fix. The
  subsequent quorum-fast deploy hit a transient SCP close on validator-2 before
  measurement. Resume from session 01 quorum-fast with
  `REMOTE_OPERATOR_RETRIES=6` and `REMOTE_OPERATOR_RETRY_BACKOFF_SECONDS=5`.

## Required Lanes

| Lane | Driver | Rounds | Sessions | Publication role |
|---|---|---:|---:|---|
| `postfiat_full_vote_multihost` | proposer-routed `normal-run`, final verify-state | 1000 | 5 | headline Post Fiat lane |
| `postfiat_quorum_fast_multihost` | proposer-routed `normal-run` with `POSTFIAT_QUORUM_FAST=1`, final verify-state | 1000 | 5 | engineering/context lane |
| `xrpl_stock_multihost` | private `rippled` harness | 1000 | 5 | stock XRPL control |
| `xrpl_close_750ms_multihost` | private `rippled` harness | 1000 | 5 | selected tuned XRPL control |
| `postfiat_multihost_safety_gate` | adversarial finality gate | pass/fail | 1+ | safety qualifier |

Optional only after required lanes:

- `xrpl_close_250ms_multihost` as a stress lane. It must not block Packet B.

## Phase 1 - Completed Diagnostic Compatibility Lane

Status: complete.

The earlier `short-matrix/session-01/postfiat-full` compatibility lane used the
slower per-round-status `normal-run` driver and passed. It remains useful as a
diagnostic, but it is not the scaled publication driver because wall-clock
runtime was dominated by per-round status polling. Validation gate:

```bash
jq -e '
  .status=="passed"
  and .converged==true
  and .rounds_requested==200
  and ([.round_reports[].quorum_early_full_propagation] | all(. == false))
' reports/packet-b-multihost-latency/short-matrix/session-01/postfiat-full/testnet-remote-ssh-smoke.json
```

Record:

- `latency.peer_certified_total`;
- final heights;
- `quorum_fast_requested`;
- unique `quorum_early_full_propagation` flags;
- SHA-256 of the report.

## Phase 2 - Completed 2x200 Optimized Short Matrix

Status: sessions 01 and 02 complete.

Each completed session ran one clean 200-round pass for each required latency
lane using the publication drivers:

```bash
# Post Fiat full-vote proposer-routed normal-run
SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
ROUNDS=200 \
REMOTE_SMOKE_REUSE_MACHINES=1 \
REMOTE_SMOKE_DEPLOY=1 \
REMOTE_SMOKE_RESET_STATE=1 \
REMOTE_SMOKE_BUILD=1 \
REMOTE_ROUND_KIND=transparent \
REMOTE_TRANSPARENT_DRIVER=normal-run \
REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0 \
REPORT=reports/packet-b-multihost-latency/short-matrix-optimized/session-01/postfiat-full/testnet-remote-ssh-smoke.json \
scripts/testnet-remote-ssh-smoke

# Post Fiat quorum-fast proposer-routed normal-run
POSTFIAT_QUORUM_FAST=1 \
SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
ROUNDS=200 \
REMOTE_SMOKE_REUSE_MACHINES=1 \
REMOTE_SMOKE_DEPLOY=1 \
REMOTE_SMOKE_RESET_STATE=1 \
REMOTE_SMOKE_BUILD=1 \
REMOTE_ROUND_KIND=transparent \
REMOTE_TRANSPARENT_DRIVER=normal-run \
REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0 \
REPORT=reports/packet-b-multihost-latency/short-matrix-optimized/session-01/postfiat-quorum/testnet-remote-ssh-smoke.json \
scripts/testnet-remote-ssh-smoke

# XRPL stock private control
SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
ROUNDS=200 \
scripts/xrpl-private-control-multihost-benchmark \
  --rippled reports/packet-b-multihost-latency/bin/rippled-stock-3.1.3-46b241a-stripped \
  --root-dir reports/packet-b-multihost-latency/short-matrix-optimized/session-01/xrpl-stock \
  --run-id packet-b-short-optimized-s01-xrpl-stock

# XRPL close_750ms private control
SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
ROUNDS=200 \
scripts/xrpl-private-control-multihost-benchmark \
  --rippled reports/packet-b-multihost-latency/bin/rippled-close_750ms-46b241a-stripped \
  --root-dir reports/packet-b-multihost-latency/short-matrix-optimized/session-01/xrpl-close750 \
  --run-id packet-b-short-optimized-s01-xrpl-close750
```

Pass criteria:

- all reports have `status == "passed"`;
- Post Fiat reports have `converged == true`;
- full-vote Post Fiat has no quorum-fast flags;
- quorum-fast Post Fiat has quorum-fast requested and observed;
- Post Fiat optimized reports set `normal_run_status_every_round == false`
  and final verify-state passes;
- XRPL reports have all submitted payments validated;
- no report contains credential material;
- hash every report and modified harness script.

## Phase 3 - Finish 3x200 Optimized Short Matrix

Run `short-matrix-optimized/session-03` with the exact same four lanes and
drivers used for sessions 01 and 02:

1. Post Fiat full-vote, proposer-routed `normal-run`,
   `REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0`.
2. Post Fiat quorum-fast, same driver plus `POSTFIAT_QUORUM_FAST=1`.
3. Private stock `rippled`, validated-ledger inclusion.
4. Private `rippled close_750ms`, validated-ledger inclusion.

Pass criteria:

- all four session-03 reports pass the same `jq` gates used for session 02;
- all three optimized short-matrix sessions have 200/200 successful rounds per
  lane;
- Post Fiat final verify-state passes on every Post Fiat lane;
- XRPL reports have `private_material_redacted=true`;
- report hashes are appended to
  `reports/packet-b-multihost-latency/SHORT_MATRIX_OPTIMIZED_SHA256SUMS.txt`.

Then update `docs/status/packet-b-multihost-lab-2026-06-09.md` with a compact
3x200 result table and the observed artifact overhead.

## Phase 4 - Safety Gate

Run the existing adversarial finality gate after the 3x200 short matrix:

```bash
scripts/testnet-finality-chaos-gate
```

If the command requires remote adaptation, write the adapted command and output
path into the lab doc before publishing. The packet is not publication-ready
without a safety gate result.

Pass criteria:

- every listed adversarial/finality scenario passes;
- output is hash-bound;
- the article only says the fast path passed the local/controlled safety gate,
  not that the system is proven safe under all Byzantine/WAN conditions.

## Phase 5 - Run Full 5x1000 Matrix

Only run this if the 3x200 short matrix and safety gate pass.

Use the same four lanes and drivers as Phase 2, with:

- directories under `reports/packet-b-multihost-latency/full-matrix/session-0N/`;
- `ROUNDS=1000`;
- sessions `01` through `05`;
- clean Post Fiat deploy/reset per Post Fiat lane;
- clean private XRPL deployment per XRPL lane.

Expected practical runtime:

- Post Fiat loop lanes should be dominated by measured consensus/application
  path rather than SSH setup;
- XRPL lanes remain bounded by ledger timing and payment validation polling;
  use `--log-policy stdio` to bound artifact collection;
- do not run lanes concurrently on the same machines.

## Phase 6 - Public Packet

Create:

```text
reports/packet-b-multihost-latency/PACKET_B_README.md
reports/packet-b-multihost-latency/manifest.json
reports/packet-b-multihost-latency/SHA256SUMS.txt
```

The manifest must include:

- topology: six validators on three project-controlled machines;
- lane names, driver names, round count, session count;
- machine role mapping without passwords;
- script hashes;
- binary hashes for both `rippled` profiles;
- report hashes;
- pass/fail gates;
- finality-surface definitions;
- known caveats.

Required caveats:

- Post Fiat metric is certified applied-batch receipt timing;
- XRPL metric is validated-ledger inclusion timing;
- this is controlled multi-host evidence, not public WAN or independent
  operator evidence;
- two validators share each physical host/IP in this topology;
- XRPL peer counts must be explained if they remain asymmetric because of
  shared-machine/IP placement.

Run a secret scan before copying anything public:

```bash
rg -n "everythingIsRigged|theMachineRises|validation_seed|secret|password|passwd" \
  reports/packet-b-multihost-latency
```

Any match must be deleted or moved into a non-public private folder before
publication.

## Phase 7 - Blog Update

Only update the public article after Phases 3-6 pass.

The article update should be one evidence addendum, not a broad rewrite:

```text
Packet B extends the local packet to a controlled three-machine topology. In
that packet, the same six-validator sequential native-transfer workload measured
Post Fiat at X/Y/Z and private XRPL controls at A/B/C and D/E/F. The finality
surfaces remain different, so the packet supports a narrower claim: the Post
Fiat certified-receipt path retained its latency advantage under controlled host
separation.
```

Do not publish:

- "public WAN";
- "decentralized validator set";
- "apples-to-apples finality";
- "XRPL is poorly designed";
- "production mainnet performance".

## Done Definition

Packet B is done when:

1. 3x200 publication-driver short matrix passes;
2. safety gate passes;
3. 5x1000 full matrix passes, if the safety gate and short matrix remain clean;
4. `manifest.json`, `PACKET_B_README.md`, and `SHA256SUMS.txt` exist;
5. secret scan is clean for the publishable artifact tree;
6. the article is updated only with the earned Packet B claim;
7. all changed scripts/docs are listed with hashes.
