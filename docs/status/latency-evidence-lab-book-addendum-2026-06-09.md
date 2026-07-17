# Latency Evidence Lab Book Addendum

Date: 2026-06-09 UTC
Status: addendum, not article copy
Related article: `$POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md`
Related plan: `docs/status/latency-claims-evidence-upgrade-plan-2026-06-08.md`

## Purpose

This addendum records the evidence work behind the Post Fiat L1 v2 private XRPL
latency article in lab-book form. It is intentionally more mechanical than the
public article: what test was run, what question it answered, where the output
lives, which scripts produced it, what hashes bind the artifacts, and what the
result does not prove.

This is not a public-mainnet claim. The claim-critical packets are local,
single-host, six-validator, sequential native-transfer benchmarks.

## Short Conclusion

The evidence supports this narrow statement:

```text
In a local one-host, six-validator native-transfer benchmark, Post Fiat L1 v2
full-vote certified finality completed signed transfers at 88.083 ms p50 and
104.705 ms p95. Stock private rippled validated transfers at 3000.565 ms p50.
The matrix-selected reduced-timing private rippled profile, close_750ms, was
classified strained and validated transfers at 883.507 ms p50. The aggressive
close_250ms stress lane reached 573.367 ms p50 but had 15345.814 ms p95 and
19918.846 ms p99.
```

The strongest XRPL timing conclusion is:

```text
In this local private topology, reducing rippled ledger timing improved median
latency, but aggressive compression produced large tails. No reduced-timing
profile met the frozen stable criteria in the timing matrix.
```

The strongest Post Fiat safety conclusion is:

```text
The measured fast path passed the local adversarial finality gate included in
the selected matched packet: 9/9 cases passed, residual_work == [].
```

The evidence does not prove public-mainnet latency, WAN behavior, throughput
under concurrent load, optimized XRPL behavior, or superiority over modern
object/certificate-first systems.

## Evidence Inventory

| Evidence item | Location | Role |
|---|---|---|
| XRPL timing stability matrix | `$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z/` | Maps stock and reduced-timing private `rippled` profiles. Selects the comparison profile and classifies stress lanes. |
| Selected matched latency packet | `$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/` | Reruns Post Fiat against stock private `rippled`, selected `close_750ms`, and aggressive `close_250ms`. |
| Safety gate report | `$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/safety/` | Shows local adversarial finality gate passed. |
| Peer calibration packet | `$POSTFIATORG_REPO/static/benchmarks/peer-l1-latency-avax-sui-20260607T135723Z/` | Context only. Shows Post Fiat is not claiming the lowest local p50 among peer systems. |
| Article source | `$POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md` | Public article draft updated with current packet-backed evidence. |

## Hash Manifest Verification

The two claim-critical packets were verified with their public hash manifests:

```bash
cd $POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z
sha256sum -c SHA256SUMS.txt

cd $POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z
sha256sum -c SHA256SUMS.txt
```

Both commands verified all packet entries.

Important file hashes:

| File | SHA256 |
|---|---|
| Timing matrix `SHA256SUMS.txt` | `15576ce7ccfee1fd11008fac91ab2a8fb1a42d36b3ddc78ac52f7bcf1c3f8969` |
| Timing matrix `aggregate.json` | `04e17ac8b436c0c32a6727c99e7606702af47d5f10c6f992c9b140901b557ef4` |
| Timing matrix `manifest.json` | `91a46f98843fd2dd56103e93a3f42381e154bd56dcd22ae3eca91582606ec8fb` |
| Selected matched packet `SHA256SUMS.txt` | `8463e3ceefc15ee2eea03ae02a194783d54dd383f2eb89cb8e70ec7579458a7e` |
| Selected matched packet `aggregate.json` | `2a50c731ae8c240d2b6413667f055a627a963aa97c9615beb6c9100431131b65` |
| Selected matched packet `manifest.json` | `75344173cf9b0968f045e09d554444540be146c70dbf38ef6c55d3037e7e953c` |
| Safety gate report | `48a1f1af561c07d730e7ef55487cfa5decef09588df658e4cd246f4dd6201527` |
| Peer calibration `SHA256SUMS.txt` | `6c2e0bda5ee3110bb5168f6369cb7930b4c0f5ec072711609f43106d44c888a0` |
| Peer calibration `aggregate.json` | `e9ea032726c6208d175bd00e4491731b68422ab34beaf20cfd09c63d461ff87a` |
| Article source scored by TIH | `4fa4b0b546ffbb2cfcd8e5073110481c2936d564127d88a8c4c2aa62be18cd8d` |

## Test 0: Tooling And Smoke Checks

Question:

```text
Can we build custom private rippled timing profiles and run the matrix harness
before spending a full benchmark run?
```

What was done:

- `scripts/build-rippled-timing-profiles` was smoke-run against `close_1000ms`.
- `scripts/xrpl-timing-stability-matrix` was smoke-run with one session and
  three rounds for `stock` and `close_1000ms`.
- The smoke matrix wrote a hash manifest and verified `SHA256SUMS.txt`.

Evidence:

- Recorded in `docs/status/latency-claims-evidence-upgrade-plan-2026-06-08.md`
  under `Execution State`.

Result:

```text
Smoke checks passed. Full evidence run was allowed to proceed.
```

Limit:

```text
The smoke runs were harness-readiness checks only. They are not public
performance evidence.
```

## Test 1: XRPL Private Timing Stability Matrix

Question:

```text
If private rippled ledger timing is compressed, is there a stable low-latency
profile under our local six-validator native-transfer workload?
```

Command:

```bash
cd $POSTFIAT_REPO

RUN_ID=xrpl-timing-stability-20260608T152301Z \
SESSIONS=5 \
ROUNDS=1000 \
VALIDATORS=6 \
PROFILES=stock,close_1500ms,close_1000ms,close_750ms,close_500ms,close_250ms \
BUILD_PROFILES=1 \
FORCE=1 \
scripts/xrpl-timing-stability-matrix
```

Scripts:

| Script | Role | SHA256 |
|---|---|---|
| `scripts/build-rippled-timing-profiles` | Patches/builds one `rippled` binary per timing profile. | `812ed9b10ab3f140f98d27c0cb2c676c35df2dec524138ec5945a92928b23485` |
| `scripts/xrpl-timing-stability-matrix` | Runs the profile matrix and writes the public packet. | `cd75e2c957b7b943444405a3ab4c0efa69858ed69b87d52358b4c42bc77c964a` |
| `scripts/xrpl-private-control-benchmark` | Runs each six-validator private `rippled` lane. | `42326ab887cf834ee068e99ec75bba780119da03acde471b6b12f870bf24e065` |
| `scripts/select-xrpl-tuned-profile` | Selects the profile used by the follow-on matched rerun. | `84b529c3af78d5e8b63da0b0e58ba092252a2134714b1af213342bc0fadfdedf` |

Profile binaries:

| Profile | Binary SHA256 | Source diff SHA256 |
|---|---|---|
| `stock` | `c67d10b48bd6a2e62cc33a0771d4428f6786e0b05a71e9febb180ac1af438bf3` | stock source |
| `close_1500ms` | `9e2dc1268319f8a7f635fb0e65cc994f6e4866fade3ac7d7d6b2f89aa069f857` | `5c1e48222ced6db535de6f90d5c47b9072095967407841843a7a08d491ec2c37` |
| `close_1000ms` | `3efb43a81d697bdeb1b0dc4fc97e312ee03b4155aa7674a0863989abaf8c57d8` | `4d109ee40537e25bcb1ef9a5284a30c52a760e47f48e90ff0acc6d4f81c9270c` |
| `close_750ms` | `10dc9c03fb0c5901f6ab06d8e26513d09c743a99d0caf84aaf69e1c30011e5b9` | `7a713fa01cec4c0f50ea78c66b597d05684f0192df29c4d12baab6243de65532` |
| `close_500ms` | `f4f32cd576a2f3ef22bf1efd19a200e81f1dafbd93b141bd1424922d958ff577` | `6cb8ff2a06c8d8a4ea8209f237fada3f5a629b521f9866c17364301bca09bfa5` |
| `close_250ms` | `175541ffb4acbb6c87fb0de255b613d98c0c4c07893997e24712b381a385d2f9` | `f23ceb87e5cb1c1a5c7efb8337405a98407895fd64d9bfec6fde8b1f41fc0136` |

Method:

- One local host.
- Six private `rippled` validators.
- Five sessions per profile.
- 1000 sequential payments per session.
- JSON-RPC `submit`, then poll `tx` until `validated: true`.
- Stability classes frozen in `methodology.md`.

Result:

| Profile | Class | Count | p50 ms | p95 ms | p99 ms | Mean ms | Max ms | >=10s tails |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| `stock` | strained | 5000 | 3001.717 | 3054.519 | 6003.322 | 3078.111 | 18008.493 | 4 |
| `close_1500ms` | strained | 5000 | 1811.109 | 1868.058 | 3628.837 | 1880.540 | 14426.371 | 1 |
| `close_1000ms` | strained | 5000 | 1193.009 | 1296.206 | 2435.711 | 1258.695 | 6048.540 | 0 |
| `close_750ms` | strained | 5000 | 884.057 | 1760.959 | 2689.190 | 970.852 | 5425.243 | 0 |
| `close_500ms` | unstable | 5000 | 623.259 | 1295.996 | 15860.846 | 1114.227 | 44978.919 | 81 |
| `close_250ms` | unstable | 5000 | 572.713 | 15291.490 | 20311.839 | 1535.799 | 61819.698 | 274 |

Lab-book interpretation:

```text
No reduced-timing private rippled profile classified stable. close_750ms was
the fastest non-unstable profile and therefore became the selected strained
comparison control. close_250ms was retained only as an aggressive stress lane.
```

Claim boundary:

```text
This proves a local private timing-envelope result. It does not prove public
XRPL behavior, optimized XRPL behavior, or that stock rippled is unsafe.
```

## Test 2: Selected Post Fiat / XRPL Matched Rerun

Question:

```text
After selecting the least-bad reduced private rippled control from the timing
matrix, how does Post Fiat L1 v2 compare under the same local six-validator
sequential native-transfer benchmark?
```

Fresh reproduction command:

```bash
cd $POSTFIAT_REPO

RUN_ID=postfiat-selected-xrpl-v4-20260609T052252Z \
SESSIONS=5 \
ROUNDS=1000 \
VALIDATORS=6 \
TUNED_BIN=$REPOS_ROOT/rippled/.build/rippled-timing-46b241ace8b3-close_750ms \
TUNED_PROFILE=close_750ms \
TUNED_CLASSIFICATION=strained \
TUNED_SOURCE_MATRIX=$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z \
AGGRESSIVE_BIN=$REPOS_ROOT/rippled/.build/rippled-timing-46b241ace8b3-close_250ms \
AGGRESSIVE_PROFILE=close_250ms \
RUN_SAFETY_GATE=1 \
BUILD_RELEASE=1 \
FORCE=1 \
scripts/postfiat-xrpl-latency-evidence-v4
```

Final repackaging command:

```bash
cd $POSTFIAT_REPO

RUN_ID=postfiat-selected-xrpl-v4-20260609T052252Z \
SESSIONS=5 \
ROUNDS=1000 \
VALIDATORS=6 \
TUNED_BIN=$REPOS_ROOT/rippled/.build/rippled-timing-46b241ace8b3-close_750ms \
TUNED_PROFILE=close_750ms \
TUNED_CLASSIFICATION=strained \
TUNED_SOURCE_MATRIX=$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z \
AGGRESSIVE_BIN=$REPOS_ROOT/rippled/.build/rippled-timing-46b241ace8b3-close_250ms \
AGGRESSIVE_PROFILE=close_250ms \
RUN_SAFETY_GATE=1 \
BUILD_RELEASE=0 \
RESUME=1 \
scripts/postfiat-xrpl-latency-evidence-v4
```

Scripts:

| Script | Role | SHA256 |
|---|---|---|
| `scripts/postfiat-xrpl-latency-evidence-v4` | Orchestrates the selected matched rerun and public packet. | `ad10609effb4e3fe7a0bff2c237860562bdf613c56e04bf2820d488f1321a0a0` |
| `scripts/testnet-real-transaction-latency-benchmark` | Measures Post Fiat signed-transfer wallet-to-finality latency. | `d04fa82fcb606b4abc103d6dc5105e2d0da663358e09585142744e996fc6e755` |
| `scripts/xrpl-private-control-benchmark` | Measures private `rippled` submit-to-validated latency. | `42326ab887cf834ee068e99ec75bba780119da03acde471b6b12f870bf24e065` |
| `scripts/testnet-finality-chaos-gate` | Runs local adversarial finality gate included in this packet. | `d4cd49e7feae9e37d8e0a93cb869cd5a629af922ad855264f1fc6f3b5fb21f12` |

Method:

- One local host.
- Six validators.
- Five sessions per lane.
- 1000 sequential signed native transfers per session.
- Post Fiat surface: `wallet_to_finality_ms`.
- XRPL surface: `submit_to_validated_ms`.
- Lane order rotated by session.
- Safety gate run as part of the packet.

Result:

| Lane | Count | p50 ms | p95 ms | p99 ms | Mean ms |
|---|---:|---:|---:|---:|---:|
| Post Fiat full-vote current | 5000 | 88.083 | 104.705 | 110.593 | 87.937 |
| Post Fiat quorum-fast current | 5000 | 83.936 | 100.362 | 106.561 | 83.188 |
| Selected `close_750ms` private `rippled` | 5000 | 883.507 | 984.493 | 1818.378 | 941.539 |
| Aggressive `close_250ms` private `rippled` stress lane | 5000 | 573.367 | 15345.814 | 19918.846 | 1573.954 |
| Stock private `rippled` | 5000 | 3000.565 | 3054.779 | 6010.940 | 3121.950 |

Win-count result:

```text
Post Fiat full-vote was faster than stock private rippled, selected close_750ms,
and aggressive close_250ms on p50, p95, p99, and mean in all 5/5 sessions.
```

Lab-book interpretation:

```text
The selected matched rerun supports a local transaction-finality result:
Post Fiat's current certified-finality path was materially faster than both
stock private rippled and the matrix-selected strained private rippled profile.
```

Claim boundary:

```text
wallet_to_finality_ms and submit_to_validated_ms are not byte-identical protocol
events. They are client-visible completion surfaces for the same local private
question: after submitting a native transfer, when can the client observe it as
final or validated?
```

## Test 3: Local Adversarial Finality Gate

Question:

```text
Did the measured fast Post Fiat path skip the obvious finality safety checks?
```

Command path:

```text
The gate was invoked by scripts/postfiat-xrpl-latency-evidence-v4 with
RUN_SAFETY_GATE=1 and emitted under the selected matched packet's safety/
directory.
```

Result:

```text
status=passed
validators=6
cases_passed=9
cases_total=9
residual_work=[]
report_sha256=48a1f1af561c07d730e7ef55487cfa5decef09588df658e4cd246f4dd6201527
```

Required case coverage:

| Required case | Covered by | Result |
|---|---|---|
| duplicate/conflicting proposal-vote refusal | `focused_finality_tests`, `proposal_vote_lock_restart` | passed |
| stale vote rejection | `focused_finality_tests` | passed |
| stale certificate rejection | `focused_finality_tests` | passed |
| parent/state-root tamper rejection | `focused_finality_tests` | passed |
| under-quorum partition rejection | `focused_finality_tests`, `finality_partition_matrix` | passed |
| process restart persistence | `proposal_vote_lock_restart`, `node_run_peer_certified_restart` | passed |
| one-validator outage | `peer_certified_partial_outage` | passed |
| delayed vote retry | `finality_delayed_vote_retry` | passed |
| Byzantine disjoint proposer | `byzantine_proposer_disjoint` | passed |
| malformed transport/certified-batch rejection | `transport_batch_tamper`, `transport_certified_batch_tamper` | passed |

Lab-book interpretation:

```text
The safety gate does not create the performance result. It supports the narrower
claim that the measured fast path still passed local adversarial finality checks
for stale/conflicting/under-quorum/tampered/restart/outage cases.
```

Claim boundary:

```text
This is a local gate. It is not a proof of production Byzantine completeness,
WAN safety, public-operator safety, or full protocol formal verification.
```

## Test 4: Peer Calibration Context

Question:

```text
Is Post Fiat claiming the lowest local p50 among modern L1-style systems?
```

Packet:

```text
$POSTFIATORG_REPO/static/benchmarks/peer-l1-latency-avax-sui-20260607T135723Z/
```

Scripts:

| Script | Role | SHA256 |
|---|---|---|
| `scripts/peer-latency-avax-local` | Local Avalanche C-Chain transfer receipt lane. | `c9e0f998c05c82fa6591c05f6b8493ed213957bf072d6b636214d0a7d2217216` |
| `scripts/peer-latency-sui-local` | Local Sui owned/shared object lanes. | `041da7a9dfe1ca61ddccef1357f1983194cce9742cfb87e87b50f385a4b3f26c` |

Result:

| Lane | Sessions | Transactions | p50 ms | p95 ms | p99 ms |
|---|---:|---:|---:|---:|---:|
| `postfiat_full_local` | 1 | 1000 | 89.062 | 105.777 | 117.092 |
| `postfiat_quorum_fast_local` | 1 | 1000 | 84.278 | 100.485 | 105.198 |
| `avax_local_c_chain_transfer` | 3 | 3000 | 1957.219 | 1978.009 | 1988.044 |
| `sui_local_owned_transfer` | 3 | 3000 | 3.968 | 230.196 | 311.582 |
| `sui_local_shared_object_tx` | 3 | 3000 | 3.928 | 227.653 | 349.605 |

Lab-book interpretation:

```text
This packet is context, not the XRPL comparison. It prevents overclaiming:
Post Fiat's local account/certified-finality lane is faster than the private
XRPL controls measured in the claim-critical packets, but Sui local effects
lanes have much lower p50 in this peer packet.
```

Claim boundary:

```text
The peer packet is local/devnet context. It should not be used as a direct
semantic equivalence claim or as public-mainnet evidence.
```

## Test 5: Article Scoring

Question:

```text
Does the exact article file, after integrating the current evidence, pass the
dedicated text-improvement harness at a publishable score?
```

Command:

```bash
cd $POSTFIATORG_REPO

PYTHONPATH=$REPOS_ROOT/text-improvement-harness-codex-plugin \
python3 -m text_improvement_harness round \
  $POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md \
  --project postfiat-blog \
  --runs 5 \
  --concurrency 15 \
  --openai-key-file $REPOS_ROOT/openai.txt \
  --openrouter-key-file $REPOS_ROOT/openx.txt
```

Result:

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

Lab-book interpretation:

```text
The current article is high-scoring enough for the narrow local evidence claim.
The scorer's recommended next action is not more prose. It is new evidence:
load/concurrency, multi-host/WAN shape, and better peer calibration.
```

## Test 6: Site Build And Publication Sanity

Question:

```text
Does the Hugo site build with the current article and public benchmark packets?
```

Command:

```bash
cd $POSTFIATORG_REPO
./.tih/bin/hugo --cleanDestinationDir
```

Result:

```text
Build completed successfully.
Pages: 22
Static files: 9539
Warnings: existing layout lookup warnings for some page/section kinds.
Public benchmark directories present under public/benchmarks/.
```

Lab-book interpretation:

```text
The article and benchmark packet paths are buildable in the static site. The
warnings are existing Hugo layout warnings, not failures for this benchmark
article or packet assets.
```

## What This Addendum Says We Still Need

The evidence above closes the L1/L2 article update:

- L1: XRPL local timing envelope mapped.
- L2: Post Fiat rerun against the selected private `rippled` control.

It does not close the next stronger claims:

| Missing claim | Evidence required |
|---|---|
| Application-facing throughput | Concurrent load matrix with finalized tx/s, failure rate, p50/p95/p99, RPC admission latency, queue depth, CPU, memory, and state size. |
| Multi-host/network shape | Same-region and WAN-shaped validator runs, plus validator delay/outage/restart cases under load. |
| Public-testnet latency | Public RPC clients, external network paths, longer-duration tests, and validator disturbance during load. |
| Broader peer positioning | Peer packets whose semantics are explicitly separated: XRPL ledger close, Post Fiat account certified finality, Sui owned/shared object paths, Avalanche local/private chains. |
| Lower p50 architecture | Prototype owned-value or certificate-first transfer lane with adversarial safety tests. |

## Reproduction Shortcuts

Verify claim-critical packet hashes:

```bash
cd $POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z
sha256sum -c SHA256SUMS.txt

cd $POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z
sha256sum -c SHA256SUMS.txt
```

Read packet-local lab books:

```text
$POSTFIATORG_REPO/static/benchmarks/xrpl-private-timing-stability-matrix-xrpl-timing-stability-20260608T152301Z/lab-book.md
$POSTFIATORG_REPO/static/benchmarks/postfiat-l1v2-selected-xrpl-matched-latency-postfiat-selected-xrpl-v4-20260609T052252Z/lab-book.md
$POSTFIATORG_REPO/static/benchmarks/peer-l1-latency-avax-sui-20260607T135723Z/lab-book.md
```

Read the article:

```text
$POSTFIATORG_REPO/content/blog/postfiat-l1v2-private-xrpl-latency-benchmark.md
```

Read the execution plan and completion audit:

```text
$POSTFIAT_REPO/docs/status/latency-claims-evidence-upgrade-plan-2026-06-08.md
```
