# Packet B: Multi-Host Post Fiat / Private XRPL Latency Plan

Date: 2026-06-09 UTC
Status: execution plan
Owner: latency benchmark worker / tmux injector
Related article: `$POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md`
Current article score ceiling: local packet only; next evidence should address multi-host and matched-control objections.

## Objective

Build a public evidence packet that tests Post Fiat and private `rippled` across
the same remote machine set, under the same validator count and sequential
native-transfer workload.

This packet is meant to address the article's strongest current criticism:

```text
The present result is a one-host loopback benchmark. Show the same Post Fiat
certified-receipt path and the private XRPL controls under real host separation.
```

The packet must not claim public-mainnet performance, WAN performance, or
independent validator operation. These are project-controlled machines and are
acceptable for controlled-testnet topology evidence.

## Claim Shape To Earn

If successful, the article may add a sentence shaped like this:

```text
In a follow-on controlled multi-host packet, the same six-validator signed
transfer workload was run across three remote machines. Post Fiat measured X ms
p50 / Y ms p95 at the certified applied-batch receipt surface. Stock private
rippled measured A ms p50 / B ms p95 at validated-ledger inclusion, and the
matrix-selected close_750ms private rippled control measured C ms p50 / D ms p95.
```

Required caveat:

```text
These remain different finality surfaces: Post Fiat certified applied-batch
receipt versus private XRPL validated-ledger inclusion.
```

## Existing Assets

### Credential Sources

Do not print or publish passwords.

Current default remote smoke credential bucket:

```text
$SSH_CREDENTIAL_FILE
```

The remote smoke harness prefers this file over `$SSH_CREDENTIAL_FILE`
when `SSH_CRED_FILE` is not set.

Current machine targets:

| Machine | SSH target | Role |
|---|---|---|
| machine 1 | `azazoth@198.51.100.12` | remote validator host |
| machine 2 | `azrael@198.51.100.10` | remote validator host |
| machine 3 | `postfiat@198.51.100.11` | remote validator host |

Before running:

```bash
chmod 600 $SSH_CREDENTIAL_FILE
```

Older credential source:

```text
$SSH_CREDENTIAL_FILE
```

Older targets:

| Machine | SSH target |
|---|---|
| old machine 1 | `goodalexander@198.51.100.14` |
| old machine 2 | `pfrpc@198.51.100.13` |

Use the current `machinemucket.txt` targets first. The older file is fallback
only.

### Existing Post Fiat Remote Harness

Primary script:

```text
scripts/testnet-remote-ssh-smoke
```

Known supporting scripts:

```text
scripts/testnet-remote-deploy-plan
scripts/testnet-config-bundle
scripts/testnet-provision-bundle
scripts/testnet-finality-chaos-gate
```

Relevant prior reports:

```text
reports/testnet-remote-deploy-plans-current-bucket/testnet-remote-deploy-plan-20260513T054436Z.json
reports/testnet-remote-deploy-plans-cobalt-realigned/testnet-remote-deploy-plan-20260518T214913Z.json
reports/testnet-cobalt-remote-bootstrap-smoke/bootstrap-cobalt-realigned-20260518T214913Z-pinned-creds.json
```

Those plans reused three physical machines across multiple validator slots.

### Existing Local Matched Evidence Tooling

Local packet builder:

```text
scripts/postfiat-xrpl-latency-evidence-v4
```

Local XRPL timing matrix:

```text
scripts/xrpl-timing-stability-matrix
scripts/build-rippled-timing-profiles
scripts/xrpl-private-control-benchmark
scripts/select-xrpl-tuned-profile
```

Current public local packets:

```text
postfiatorg.github.io/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z/
postfiatorg.github.io/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/
```

## Packet B Topology

Use six validators across three remote machines.

Initial host mapping:

| Validator | Host |
|---|---|
| validator-0 | `198.51.100.12` |
| validator-1 | `198.51.100.10` |
| validator-2 | `198.51.100.11` |
| validator-3 | `198.51.100.12` |
| validator-4 | `198.51.100.10` |
| validator-5 | `198.51.100.11` |

Why this mapping:

- It keeps the validator count equal to the local article packet: `6`.
- It uses the three available remote hosts without requiring new procurement.
- It tests host separation while preserving the controlled-testnet scope.

Public language must say "three project-controlled remote machines", not
"decentralized validator set".

## Required Lanes

Minimum useful Packet B lanes:

| Lane | Required | Metric | Notes |
|---|---|---|---|
| `postfiat_full_vote_multihost` | yes | `wallet_to_finality_ms` | conservative headline Post Fiat lane |
| `postfiat_quorum_fast_multihost` | yes | `wallet_to_finality_ms` | engineering context |
| `xrpl_stock_multihost` | yes | `submit_to_validated_ms` | stock private `rippled` validated-ledger control |
| `xrpl_close_750ms_multihost` | yes | `submit_to_validated_ms` | matrix-selected local tuned control; label as `strained` if reused |
| `postfiat_multihost_safety_gate` | yes | pass/fail | remote or local-adapted adversarial finality gate |
| `xrpl_close_250ms_multihost` | optional | `submit_to_validated_ms` | stress lane only; do not block Packet B on it |

Do not include `close_250ms` in the first must-pass run. It is interesting but
not necessary to answer the matched-control criticism.

## Work Plan

### Phase 0: Credential And Host Preflight

Goal:

```text
Confirm the three remote machines are reachable and have sudo/systemd/network
conditions required for the run.
```

Commands:

```bash
cd $POSTFIAT_REPO

chmod 600 $SSH_CREDENTIAL_FILE

SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
REMOTE_SMOKE_REUSE_MACHINES=1 \
REMOTE_SMOKE_PREFLIGHT_ONLY=1 \
REMOTE_SMOKE_DEPLOY=0 \
REMOTE_SMOKE_BUILD=0 \
REPORT=reports/packet-b-multihost-latency/preflight/testnet-remote-ssh-preflight.json \
scripts/testnet-remote-ssh-smoke
```

Pass criteria:

- all three machine credentials parse;
- all six validator slots map to a machine;
- SSH login succeeds;
- sudo/systemd checks are either pass or documented;
- no password appears in report output.

If the three-machine bucket fails, retry with `$SSH_CREDENTIAL_FILE` only
as a fallback and record that the topology changed.

### Phase 1: Post Fiat Multi-Host Smoke

Goal:

```text
Prove the existing Post Fiat remote harness can run the same six-validator
certified path on these machines before spending time on XRPL.
```

Command shape:

```bash
cd $POSTFIAT_REPO

SSH_CRED_FILE=$SSH_CREDENTIAL_FILE \
VALIDATORS=6 \
ROUNDS=50 \
REMOTE_SMOKE_REUSE_MACHINES=1 \
REMOTE_SMOKE_DEPLOY=1 \
REMOTE_SMOKE_BUILD=1 \
REMOTE_ROUND_KIND=transparent \
REMOTE_TRANSPARENT_DRIVER=round \
REPORT=reports/packet-b-multihost-latency/postfiat-smoke/testnet-remote-ssh-smoke.json \
scripts/testnet-remote-ssh-smoke
```

Pass criteria:

- report status is passed;
- final state converged across six validators;
- finality/certified-round latency fields exist;
- no private keys, wallet seeds, or SSH passwords are published;
- services are left in known-good state or stopped deliberately.

If smoke fails, do not proceed to XRPL. Fix remote deployment first.

### Phase 2: Private `rippled` Multi-Host Harness

Goal:

```text
Create or adapt a remote private rippled harness that can deploy six rippled
validators across the same three machines and measure submit-to-validated
latency from the controller.
```

Script target:

```text
scripts/xrpl-private-control-multihost-benchmark
```

Accept these inputs:

```text
--rippled <binary>
--validators 6
--rounds <N>
--host-map <json or csv>
--ssh-cred-file $SSH_CREDENTIAL_FILE
--work-root <work_root>
--report <report.json>
--profile-name stock|close_750ms
--port-base <base>
```

Required behavior:

- copy the selected `rippled` binary to every remote host;
- generate per-validator private config under the private work root;
- start six `rippled` validators across the host map;
- expose one submit/query RPC endpoint for the controller;
- submit sequential native XRP/PFT-equivalent payments;
- poll until `validated: true`;
- record `submit_to_validated_ms`;
- collect per-node server info and logs;
- stop remote `rippled` services/processes at the end;
- redact seeds, master secrets, SSH passwords, and generated validator material.

Existing local script to reuse:

```text
scripts/xrpl-private-control-benchmark
```

Do not rewrite the local script destructively. Add a multi-host script or a
clearly separated mode so the local Packet A evidence remains reproducible.

### Phase 3: XRPL Multi-Host Smoke

Goal:

```text
Run a cheap private rippled remote smoke before the full 1000-round packet.
```

Command shape:

```bash
cd $POSTFIAT_REPO

scripts/xrpl-private-control-multihost-benchmark \
  --rippled $REPOS_ROOT/rippled/.build/rippled-stock-3.1.3-46b241ace8b30d9c9775d60ffba7d24b21903896 \
  --validators 6 \
  --rounds 25 \
  --host-map reports/packet-b-multihost-latency/host-map.json \
  --ssh-cred-file $SSH_CREDENTIAL_FILE \
  --work-root reports/packet-b-multihost-latency/xrpl-stock-smoke/private \
  --report reports/packet-b-multihost-latency/xrpl-stock-smoke/xrpl-private-control-multihost.json \
  --profile-name stock \
  --port-base 39000
```

Pass criteria:

- six validators start;
- the private network validates ledgers;
- all 25 payments reach `validated: true`;
- p50/p95 are finite;
- no secrets in report;
- remote processes stop cleanly.

If stock smoke fails, fix the remote XRPL harness before running `close_750ms`.

### Phase 4: Packet B Short Matrix

Goal:

```text
Run enough rounds to identify obvious multi-host breakage without paying the
full stock-rippled time cost.
```

Initial short matrix:

| Lane | Sessions | Rounds/session |
|---|---:|---:|
| Post Fiat full-vote remote | 3 | 200 |
| Post Fiat quorum-fast remote | 3 | 200 |
| Stock private `rippled` remote | 3 | 200 |
| `close_750ms` private `rippled` remote | 3 | 200 |

Expected runtime:

- Post Fiat lanes should be minutes if healthy.
- Stock `rippled` is expected to dominate time: roughly `200 * 3s * 3 = 30min`
  plus setup and variance.
- `close_750ms` should be materially shorter but may tail.

Pass criteria:

- all lanes complete;
- no lane has unexplained failed/expired transactions;
- Post Fiat final state converges;
- `rippled` payments are validated;
- p50/p95/p99 exist for all lanes;
- host and script hashes are recorded.

Decision after short matrix:

| Result | Next action |
|---|---|
| clean Post Fiat, clean XRPL | run full matrix |
| clean Post Fiat, XRPL harness unstable | publish Post Fiat survival packet separately and keep Packet B blocked |
| Post Fiat unstable | stop and fix Post Fiat remote finality before article update |
| credentials/hosts flaky | stop and do host reliability cleanup |

### Phase 5: Packet B Full Matrix

Goal:

```text
Produce the public matched multi-host packet.
```

Full matrix:

| Lane | Sessions | Rounds/session | Required |
|---|---:|---:|---|
| `postfiat_full_vote_multihost` | 5 | 1000 | yes |
| `postfiat_quorum_fast_multihost` | 5 | 1000 | yes |
| `xrpl_stock_multihost` | 5 | 1000 | yes |
| `xrpl_close_750ms_multihost` | 5 | 1000 | yes |

Optional after required lanes:

| Lane | Sessions | Rounds/session | Role |
|---|---:|---:|---|
| `xrpl_close_250ms_multihost` | 3 | 500 | stress only |

Expected runtime:

```text
Stock rippled: about 4-5 hours for 5000 sequential validated payments.
close_750ms: about 1.5-3 hours depending on tails.
Post Fiat lanes: likely less than 1 hour total if remote path is healthy.
Setup/retries: 1-3 hours.
```

Target total:

```text
6-10 hours if remote credentials and host state are good.
```

Stop rule:

```text
Do not let this turn into a 22-hour run. If a lane has not made measurable
progress for 45 minutes, stop that lane, preserve logs, and mark Packet B
blocked on the specific failure.
```

### Phase 6: Safety Gate

Goal:

```text
Show the remote Post Fiat path is not just fast because safety checks were
removed.
```

Minimum:

```bash
cd $POSTFIAT_REPO

VALIDATORS=6 \
BASE_DIR=reports/packet-b-multihost-latency/safety/local-or-remote/nodes \
LOG_DIR=reports/packet-b-multihost-latency/safety/local-or-remote/logs \
REPORT=reports/packet-b-multihost-latency/safety/testnet-finality-chaos-gate.json \
TIMEOUT_SECONDS=60 \
TRANSPORT_TIMEOUT_MS=3000 \
SEND_RETRIES=1 \
RETRY_BACKOFF_MS=75 \
scripts/testnet-finality-chaos-gate
```

Preferred:

- adapt the adversarial gate to the remote host map, or
- run remote outage/restart drills using `scripts/testnet-remote-ssh-smoke`
  plus targeted service stop/restart.

Minimum article language if only the local gate is rerun:

```text
The multi-host latency packet is paired with the existing local adversarial
finality gate; a remote adversarial gate remains next evidence.
```

Preferred article language if remote gate passes:

```text
The same remote packet also passed a controlled remote outage/restart finality
gate.
```

## Public Packet Shape

Publish under:

```text
$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-multihost-xrpl-matched-latency-<RUN_ID>/
```

Required files:

| File | Purpose |
|---|---|
| `README.md` | one-page packet summary and claim boundary |
| `methodology.md` | host map, validator count, workload, finality surfaces, exclusions |
| `endpoint-equivalence.md` | Post Fiat receipt surface vs XRPL validated-ledger surface |
| `commands.sh` | exact reproduction commands, with passwords excluded |
| `host-map.json` | validator-to-host mapping, no passwords |
| `manifest.json` | repo heads, binary hashes, OS/kernel, command log, script hashes |
| `aggregate.json` | machine-readable aggregate stats |
| `aggregate.md` | human-readable tables |
| `session-summary.csv` | one row per lane/session |
| `latency-bars.svg` | p50/p95/p99 visual |
| `latency-cdf.svg` | distribution visual if available |
| `raw/` | redacted per-session reports |
| `safety/` | safety gate report(s) |
| `SHA256SUMS.txt` | hash manifest for public packet |
| `lab-book.md` | UTC start/finish/event log |

Required private exclusions:

- SSH passwords;
- generated XRPL seeds/master secrets;
- generated Post Fiat validator private keys;
- wallet seed material;
- node databases unless deliberately published;
- unredacted logs containing credentials.

## Article Update Rules

Only update the article if Packet B has at least:

- Post Fiat full-vote multi-host lane;
- Post Fiat quorum-fast multi-host lane;
- stock private `rippled` multi-host lane;
- `close_750ms` private `rippled` multi-host lane;
- packet hash manifest;
- command log;
- host map;
- no-secret scan;
- at least one safety gate reference.

Add only one new article section:

```text
## Multi-Host Follow-On Packet
```

Do not rewrite the whole article first. Add a compact table:

| Lane | Count | p50 ms | p95 ms | p99 ms | Mean ms |
|---|---:|---:|---:|---:|---:|
| Post Fiat full-vote multi-host | ... | ... | ... | ... | ... |
| Post Fiat quorum-fast multi-host | ... | ... | ... | ... | ... |
| Stock private rippled multi-host | ... | ... | ... | ... | ... |
| close_750ms private rippled multi-host | ... | ... | ... | ... | ... |

Add one sentence:

```text
This does not remove the finality-surface caveat; it removes the one-host
loopback caveat for this controlled topology.
```

Then rescore with TIH. Do not promote if the score drops.

## Done Definition

Packet B is done when:

- `sha256sum -c SHA256SUMS.txt` passes in the public packet;
- aggregate tables include all required lanes;
- public raw reports are redacted;
- `commands.sh` can be read without exposing credentials;
- `manifest.json` records host map, script hashes, binary hashes, and git heads;
- the article can cite the packet without using internal-only paths;
- the packet supports a stronger claim than the current local article.

Packet B is blocked, not done, if:

- the XRPL remote harness cannot validate payments across hosts;
- Post Fiat remote finality fails to converge;
- credential instability prevents repeatable runs;
- raw reports contain secrets;
- only Post Fiat is run and the private `rippled` lanes are missing.

## Why This Should Not Take 22 Hours

Do not rerun the full local XRPL timing matrix. Reuse its selected control:
`close_750ms`.

Do not run the optional `close_250ms` stress lane until the required lanes are
complete.

Use staged gates:

1. SSH preflight.
2. Post Fiat 50-round smoke.
3. XRPL stock 25-round smoke.
4. 3x200 short matrix.
5. 5x1000 full matrix only if the short matrix is clean.

The expensive part is stock private `rippled`, because its expected p50 is
around three seconds by design. That cost is bounded and predictable:

```text
5000 stock payments * about 3 seconds = about 4.2 hours
```

The whole Packet B run should be treated as a 6-10 hour execution sprint, not
an open-ended investigation.

