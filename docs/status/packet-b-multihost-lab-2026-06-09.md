# Packet B Multi-Host Lab Status - 2026-06-09

## Status

Packet B is runnable but not complete as a publication packet.

Completed:

- remote credential and six-validator topology preflight;
- clean Post Fiat multi-host smoke with state reset;
- Post Fiat full-vote multi-host smoke;
- Post Fiat quorum-fast multi-host smoke;
- remote private stock `rippled` multi-host smoke;
- remote private `rippled close_750ms` multi-host smoke;
- remote XRPL harness implementation.

Not yet complete:

- five-session, 1000-round public packet;
- public Packet B `manifest.json`, `README.md`, and `SHA256SUMS.txt`;
- blog update based on Packet B.

New short-matrix status:

- one optimized 200-round session now passes for all four required latency
  lanes;
- the optimized Post Fiat lanes use proposer-routed `normal-run` with
  per-round status polling disabled and final verify-state retained;
- the single-node loop driver is not suitable for proposer-routed evidence,
  because the binary correctly rejects a local validator proposing for another
  validator's deterministic proposer slot.

## Topology

The smoke matrix used six validator slots on three project-controlled remote
machines. This is controlled multi-host evidence, not a decentralization claim.

Validator placement:

| Validator | Machine slot |
|---|---:|
| validator-0 | 0 |
| validator-1 | 1 |
| validator-2 | 2 |
| validator-3 | 0 |
| validator-4 | 1 |
| validator-5 | 2 |

## Harness Changes

`scripts/testnet-remote-ssh-smoke`

- Added `REMOTE_SMOKE_RESET_STATE=1`, which stops the validator services and
  removes only `/var/lib/postfiat/validator-N` for the slot being deployed.
- Added `POSTFIAT_QUORUM_FAST=1` support for the remote `normal-run`
  transparent driver.
- Added `quorum_fast_requested` to reports.

`scripts/xrpl-private-control-multihost-benchmark`

- New remote private `rippled` control harness.
- Builds validator keys locally, deploys a static private validator list,
  uploads a content-addressed stripped `rippled` binary once per machine,
  starts six remote validators, runs the payment driver on validator 0 against
  loopback RPC, fetches logs, and writes a credential-free report.
- Reports private validator public keys but not validation seeds.

## Smoke Results

| Lane | Rounds | Status | p50 ms | p95 ms | p99 ms | Notes |
|---|---:|---|---:|---:|---:|---|
| Post Fiat clean one-shot remote | 50 | passed | 1086.700 | 1168.975 | 1206.293 | Fresh remote state, `REMOTE_SMOKE_RESET_STATE=1`. |
| Post Fiat full-vote remote | 20 | passed | 1093.289 | 1160.302 | 1201.595 | `normal-run`; all flags false for quorum-fast. |
| Post Fiat quorum-fast remote | 20 | passed | 1045.439 | 1089.638 | 1094.480 | `normal-run`; `quorum_early_full_propagation=true`. |
| XRPL stock remote | 10 | passed | 3001.421 | 3004.909 | 3004.909 | Private static-UNL `rippled`; validated-ledger inclusion. |
| XRPL `close_750ms` remote | 10 | passed | 1808.323 | 1916.337 | 1916.337 | Private timing-profile `rippled`; validated-ledger inclusion. |

## Optimized Short Matrix Results

Status: 3x200 complete and validated.

Per-session results:

| Lane | Rounds | Status | p50 ms | p95 ms | p99 ms | Notes |
|---|---:|---|---:|---:|---:|---|
| Post Fiat full-vote remote, session 01 | 200 | passed | 1056.884 | 1120.863 | 1163.775 | Proposer-routed `normal-run`; `REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0`; final verify-state passed. |
| Post Fiat full-vote remote, session 02 | 200 | passed | 1062.541 | 1129.606 | 1152.240 | Same driver and clean-state setup. |
| Post Fiat full-vote remote, session 03 | 200 | passed | 1045.827 | 1108.648 | 1139.866 | Same driver and clean-state setup. |
| Post Fiat quorum-fast remote, session 01 | 200 | passed | 987.825 | 1042.955 | 1087.792 | Proposer-routed `normal-run`; `POSTFIAT_QUORUM_FAST=1`; final verify-state passed. |
| Post Fiat quorum-fast remote, session 02 | 200 | passed | 1003.942 | 1064.847 | 1079.822 | Same driver and clean-state setup. |
| Post Fiat quorum-fast remote, session 03 | 200 | passed | 1023.662 | 1114.018 | 1143.831 | Same driver and clean-state setup. |
| XRPL stock remote, session 01 | 200 | passed | 2997.459 | 3006.231 | 3046.984 | Private static-UNL `rippled`; validated-ledger inclusion; 200/200 payments validated. |
| XRPL stock remote, session 02 | 200 | passed | 2998.885 | 3049.302 | 3055.967 | Same private control. |
| XRPL stock remote, session 03 | 200 | passed | 2996.770 | 3040.655 | 3047.173 | Same private control. |
| XRPL `close_750ms` remote, session 01 | 200 | passed | 929.031 | 1708.653 | 1858.670 | Private timing-profile `rippled`; validated-ledger inclusion; 200/200 payments validated. |
| XRPL `close_750ms` remote, session 02 | 200 | passed | 882.786 | 1859.309 | 1912.572 | Same private control. |
| XRPL `close_750ms` remote, session 03 | 200 | passed | 885.392 | 1860.845 | 1913.714 | Same private control. |

3x200 means:

| Lane | Sessions | Rounds each | p50 mean ms | p95 mean ms | p99 mean ms |
|---|---:|---:|---:|---:|---:|
| Post Fiat full-vote remote | 3 | 200 | 1055.084 | 1119.706 | 1151.961 |
| Post Fiat quorum-fast remote | 3 | 200 | 1005.143 | 1073.940 | 1103.815 |
| XRPL stock remote | 3 | 200 | 2997.705 | 3032.063 | 3050.042 |
| XRPL `close_750ms` remote | 3 | 200 | 899.070 | 1809.602 | 1894.985 |

Additional diagnostic lane:

| Lane | Rounds | Status | p50 ms | p95 ms | p99 ms | Notes |
|---|---:|---|---:|---:|---:|---|
| Post Fiat full-vote remote compatibility | 200 | passed | 1111.344 | 1182.555 | 1218.501 | Earlier per-round-status `normal-run`; useful compatibility check but too slow as the scaled driver. |

Failed driver attempt:

- `REMOTE_TRANSPARENT_DRIVER=loop` with only validator-0 proposal keys failed
  immediately with missing key for `validator-1`.
- `REMOTE_TRANSPARENT_DRIVER=loop` with combined proposal keys still failed
  because the binary rejected validator-0 as the local proposer for height 1
  when deterministic proposer rotation expected validator-1.
- This is a harness-design finding, not a consensus failure. It confirms the
  publication Post Fiat lane should use proposer-routed `normal-run`.

Optimized short-matrix hashes are recorded in:

```text
reports/packet-b-multihost-latency/SHORT_MATRIX_OPTIMIZED_SHA256SUMS.txt
```

## Artifacts

| Artifact | SHA-256 |
|---|---|
| `reports/packet-b-multihost-latency/preflight/testnet-remote-ssh-preflight.json` | `e10d2206e6b6e1b52ebf221c68a7c4ae8ee52f00c440481986d329d5d20951ef` |
| `reports/packet-b-multihost-latency/postfiat-smoke/testnet-remote-ssh-smoke-clean.json` | `17231ac8c9f4d4b1f7856816dae3f1d770d3035afe0b39dc5114f98f1cbd1a50` |
| `reports/packet-b-multihost-latency/postfiat-full-normal-smoke/testnet-remote-ssh-smoke.json` | `ead61917c273a2133c4b6a9341bc8ee774b9c75247d95d79a0f372dcffae3555` |
| `reports/packet-b-multihost-latency/postfiat-quorum-normal-smoke-v2/testnet-remote-ssh-smoke.json` | `0d11e34c0696f05e0ee9d9cb7a45d6be434800846701bffcb28b303f3cb4a97e` |
| `reports/packet-b-multihost-latency/xrpl-stock-smoke/xrpl-private-control-multihost-benchmark.json` | `cdfe7359dd2531158cf2411285920679a5f7492d920a08447c0203ef8383156f` |
| `reports/packet-b-multihost-latency/xrpl-close750-smoke/xrpl-private-control-multihost-benchmark.json` | `43f9a8967daba3e6bfb1334a64022412c9abb14bb452f57c14fa9fed45645814` |
| `scripts/testnet-remote-ssh-smoke` | `c549196f6b0b262c0ce2477edb6d318b61405c4dceadf6ec9ffb9a83132d5267` |
| `scripts/testnet-provision-bundle` | `66b21930ccd9481c311e449a744ecd9e89eb3a544f71e6bbb30a18883155cdab` |
| `scripts/xrpl-private-control-multihost-benchmark` | `8016eff9073762a6a0a1d95b76034b0ce73a3aae83a37635282a855ad8ada357` |
| `reports/packet-b-multihost-latency/bin/rippled-stock-3.1.3-46b241a-stripped` | `2db568990d69bd274ec7657854e0e1e0b574525e1848f4a43549dc477aafd003` |
| `reports/packet-b-multihost-latency/bin/rippled-close_750ms-46b241a-stripped` | `2128a879b6fec177031dec82cc9d5c560d46ff952c149e655e0268ef46b40098` |

## Important Caveats

- These are smoke and short-matrix runs, not the final Packet B publication
  packet.
- The optimized 3x200 matrix is a passed short matrix, not the final 5x1000
  publication matrix.
- Post Fiat optimized lanes reset state for each lane and retained final
  verify-state.
- XRPL direct peer counts were `3,3,3,1,1,1` at the end of both remote XRPL
  smokes because two validators share each machine/IP. The nodes were all in
  `proposing` state and the submitted transactions validated, but the final
  packet should describe this peer surface plainly.
- XRPL final validated sequence reads are serial observations from live nodes,
  so they can differ while ledgers keep closing. Per-transaction observations
  show the ledger sequence before submit, after validation, and ledgers crossed.
- The `close_750ms` profile validated each measured transaction in one or two
  ledgers; the p50 was about 1.8s, not 750ms.
- The first Post Fiat smoke failure was a dirty-state failure:
  `verify-state failed: appended block height 133 does not extend materialized
  tip 321`. That was fixed by adding `REMOTE_SMOKE_RESET_STATE=1`.
- The first attempted quorum-fast Post Fiat run was invalid because the flag
  was not preserved through `sudo -u postfiat`. The valid quorum-fast report is
  `postfiat-quorum-normal-smoke-v2`.
- Secret scan of the optimized short matrix found XRPL validation seeds and
  private keys inside `private/` harness directories and keygen debug logs.
  The summary JSON reports set `private_material_redacted=true`, but the
  `private/` subtrees are not public artifacts and must be excluded or redacted
  before building the public Packet B packet.
- Path-only secret scan at
  `reports/packet-b-multihost-latency/SECRET_SCAN_SUMMARY_20260610T0120Z.md`
  found 64 matching paths, all under `private/`, and zero matching paths outside
  `private/`. The raw tree remains non-publishable; a public packet must be
  assembled from a redacted subset.
- The XRPL multihost harness now supports `--log-policy full|stdio|none`.
  Earlier short-matrix runs used full debug-log collection. The 5x1000 matrix
  should use `--log-policy stdio` so the latency report remains intact while
  avoiding multi-megabyte debug-log transfer after every lane.
- First 5x1000 full-vote attempt completed the 1000-round loop but failed
  during final `verify-state` because the generic SSH timeout was too low for a
  1000-block replay. The failed report is preserved at
  `reports/packet-b-multihost-latency/full-matrix/session-01/postfiat-full/testnet-remote-ssh-smoke.failed-verify-timeout.json`.
  `scripts/testnet-remote-ssh-smoke` now has
  `REMOTE_VERIFY_STATE_TIMEOUT_SECONDS`, reported under
  `operator_retry_policy.verify_state_timeout_seconds`.
- Full-matrix session 01 Post Fiat full-vote then passed with
  `REMOTE_VERIFY_STATE_TIMEOUT_SECONDS=900`: p50 1220.708 ms, p95 1341.589 ms,
  p99 1384.331 ms. The next quorum-fast deploy attempt failed before
  measurement because SCP to validator-2 closed the SSH connection. The failed
  deploy report is preserved at
  `reports/packet-b-multihost-latency/full-matrix/session-01/postfiat-quorum/testnet-remote-ssh-smoke.failed-scp-deploy.json`.
  Resume Post Fiat lanes with a larger SSH/SCP retry budget.

## Go/No-Go

Go for 5x1000 full matrix.

The 3x200 short matrix passed and the safety gate passed.

Safety gate:

| Gate | Status | Cases | Report | SHA-256 |
|---|---|---:|---|---|
| `scripts/testnet-finality-chaos-gate` | passed | 9/9 | `reports/packet-b-multihost-latency/safety-gate-20260610T0125Z/testnet-finality-chaos-gate.json` | `2f22db63f00d66def27f19a4c388df7850bd3528a5ca97233ebb349b34eba7bc` |

Next required work:

- run the 5x1000 full matrix with `--log-policy stdio` on XRPL lanes.

## Next Commands

Completed safety gate:

```bash
cd $POSTFIAT_REPO
BASE_DIR=reports/packet-b-multihost-latency/safety-gate-20260610T0125Z \
LOG_DIR=reports/packet-b-multihost-latency/safety-gate-20260610T0125Z/logs \
REPORT=reports/packet-b-multihost-latency/safety-gate-20260610T0125Z/testnet-finality-chaos-gate.json \
VALIDATORS=6 \
ROUNDS=3 \
scripts/testnet-finality-chaos-gate
```

Run 5x1000 with the same four publication lanes used for the 3x200 short
matrix:

- Post Fiat full-vote remote: proposer-routed `normal-run`,
  `REMOTE_NORMAL_RUN_STATUS_EVERY_ROUND=0`.
- Post Fiat quorum-fast remote: same driver plus `POSTFIAT_QUORUM_FAST=1`.
- XRPL stock remote: private static-UNL `rippled`.
- XRPL `close_750ms` remote: selected timing-profile `rippled`.

Use `--log-policy stdio` on XRPL lanes. Do not run lanes concurrently on the
same machines.

## Publication Rule

Do not update the public latency article from Packet B until:

1. the 3x200 matrix passes;
2. the safety gate passes;
3. the 5x1000 matrix passes;
4. the final packet has `README.md`, `manifest.json`, `SHA256SUMS.txt`, and
   secret scan;
5. the article states the finality surfaces separately:
   `peer_certified_total` for Post Fiat and `submit_to_validated` for XRPL;
6. the article states this is controlled three-machine evidence, not public
   WAN decentralization evidence.
