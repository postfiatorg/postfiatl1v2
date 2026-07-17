# Fast Finality Experiment

Status: controlled-testnet fast-finality evidence packet closed
Date: 2026-06-05
Scope: PostFiat L1 v2 transparent PQ settlement

## 2026-06-06 Closure

The fast-finality milestone is closed for the controlled-testnet transparent
PFT path by the 2026-06-06 evidence packet:

`reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-evidence-packet/README.md`

Done means the packet includes:

- a 25-round live controlled-fleet wallet-finality benchmark through the
  persistent finality RPC edge;
- a 100-round local persistent RPC regression;
- a slow-peer quorum-early smoke after the live run;
- a post-run live validator doctor;
- script hashes, dirty worktree disclosure, packet hashes, and a value-oriented
  redaction scan;
- a claim boundary that keeps this to controlled testnet, not public write
  admission or mainnet-grade WAN evidence.

Closure results:

| Surface | Result | Evidence |
|---|---:|---|
| Live wallet finality | 25/25 passed | `reports/testnet-live-wallet-finality-benchmark/fast-finality-exp-20260606-live25-wallet-finality2/testnet-live-wallet-finality-benchmark.json` |
| Live `submit_finality_total` | p50 `290.35356ms`, p95 `375.418875ms`, p99 `380.18433ms` | same |
| Live certified round | p50 `277.490913ms`, p95 `349.296915ms`, p99 `353.31662ms` | same |
| Live write edge | non-public and single bounded request per round | same |
| Local persistent RPC | 100/100 passed | `reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-local100-persistent-rpc2/testnet-tx-finality-latency-benchmark.json` |
| Local `submit_to_finality` | p50 `1108.065983ms`, p95 `1419.845026ms`, p99 `1474.393055ms` | same |
| Local final height | `101` | same |
| Slow-peer quorum-early | passed; quorum `4/5`, slow node `validator-4` | `reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-quorum-early/testnet-transport-peer-certified-quorum-early.json` |
| Post-run live doctor | passed at height `60` | `reports/testnet-live-validator-doctor/fast-finality-exp-20260606-post-live25-doctor/testnet-live-validator-doctor.json` |

The live doctor reported the deployed binary hash
`c597dd4b0d0dee8cdfc2f7ca4ea86418489616850fa2349fabf01b5627280142`
across the six-validator fleet after the run.

## Question

Can we materially reduce user-visible transaction finality by treating quorum
certification as the first client-visible finality event, while keeping
validator convergence and replay evidence intact?

## Runs

### Local 25-Round Baseline

Command:

```bash
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/local25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/local25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/local25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/local25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/local25/logs/local-harness.json \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/local25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_finality` | `1847ms` | `2056ms` | `2155ms` |
| `submit_to_certified` | `1379ms` | `1521ms` | `1608ms` |
| `certified_round` | `1188ms` | `1317ms` | `1387ms` |
| `tx_finality_rpc` | `68ms` | `75ms` | `76ms` |
| `submit_rpc` | `71ms` | `78ms` | `78ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `113ms` |
| local vote | `120ms` |
| vote requests | `260ms` |
| certificate | `104ms` |
| certified sends | `331ms` |
| local apply | `185ms` |
| post-apply status | `7ms` |

Read: the current path is correct but not subsecond. The biggest avoidable
client-visible component is certified-batch broadcast before returning to the
client. If the node returns after quorum certificate plus local apply, and
continues certified broadcast to lagging peers asynchronously, the existing
numbers imply an immediate several-hundred-millisecond reduction without
weakening the certificate.

### Local 25-Round Fast-Apply Experiment

Change:

- added `--local-apply-before-certified-send` to peer-certified round and loop
  mode;
- wired `LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1` through
  `scripts/node-run-peer-certified`;
- kept the old binary default off;
- changed the benchmark default to opt in for this experiment;
- added `client_visible_finality_ms` to the peer-certified timing report.

The invariant is unchanged: the node does not apply locally until after a
quorum block certificate has been aggregated. The only scheduling change is
that the local validator applies the certified block before waiting for
certified-batch sends to the other peers.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-fastapply25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/fastapply25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/fastapply25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/fastapply25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/fastapply25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/fastapply25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/fastapply25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `client_visible_finality_round` | `847ms` | `938ms` | `960ms` |
| `submit_to_client_visible_finality_estimated` | `978ms` | `1081ms` | `1103ms` |
| `submit_to_finality` | `1902ms` | `2092ms` | `2140ms` |
| `certified_round` reported total | `1194ms` | `1326ms` | `1366ms` |
| `certified_sends` stage | `340ms` | `429ms` | `470ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `122ms` |
| local vote | `129ms` |
| vote requests | `270ms` |
| certificate | `125ms` |
| local apply | `178ms` |
| post-apply status | `7ms` |
| certified sends | `340ms` |

Checks:

- 25 iterations passed;
- all iterations confirmed;
- all validators converged;
- state verification passed on every validator;
- final height was `26`.

Read: this creates a real subsecond in-round finality point for the local
quorum-certified path. It does not yet make the external harness wall-clock
`submit_to_finality` subsecond, because the script still waits for all
certified sends to finish and then performs a separate finality RPC. The next
runtime step is to expose this early point as the long-running service response
instead of only reporting it inside the loop artifact.

### Local 25-Round Deferred Fast-Return Experiment

Change:

- added `--defer-certified-sends` to peer-certified round and loop mode;
- copied the batch into the deferred-send artifact directory before loop-mode
  archival, so background send workers do not race the processed-batch move;
- launched file-backed deferred send workers after quorum certificate plus
  local apply;
- returned the source validator process before waiting for peer fanout;
- extended the benchmark to validate every deferred send report after peer
  services exit.

The invariant is unchanged: the source validator still requires a quorum block
certificate and a verified local apply before returning. The deferred work is
only the propagation of that certified batch to peers that already voted.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-deferred25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/deferred25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/deferred25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/deferred25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/deferred25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/deferred25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/deferred25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_fast_return` | `1148ms` | `1266ms` | `1278ms` |
| `client_visible_finality_round` | `901ms` | `997ms` | `1011ms` |
| `certified_round` wall clock | `941ms` | `1044ms` | `1061ms` |
| `submit_to_client_visible_finality_estimated` | `1032ms` | `1139ms` | `1153ms` |
| `submit_to_finality` | `2016ms` | `2193ms` | `2274ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `119ms` |
| local vote | `125ms` |
| vote requests | `318ms` |
| certificate | `129ms` |
| local apply | `178ms` |
| deferred send launch | `2ms` |
| post-apply status | `7ms` |

Checks:

- 25 iterations passed;
- every round launched 4 deferred certified-send workers;
- every deferred send report verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- final height was `26`.

Read: this is the first actual wall-clock fast-return result. It removes
certified-send fanout from the user-visible source-process return and improves
the old `submit_to_certified` p50 from `1379ms` to `1148ms`. The source round
itself is around one second and p50 below the old certified path, but full
submit-to-return is still above one second because the benchmark still pays
separate submit-RPC and mempool-batch process costs before consensus starts.

### Local 25-Round Combined Mempool Fast-Return Experiment

Change:

- added `transport-peer-certified-mempool-round`, which creates the transparent
  mempool batch and runs the peer-certified round in one node process;
- kept quorum certificate plus verified local apply as the fast-return floor;
- kept deferred certified sends enabled and validated every deferred peer ack
  after the source process returned;
- preserved per-round mempool batching, peer-certified timings, and deferred
  send reports in the benchmark evidence packet.

The invariant is unchanged: the source validator still does not return until
it has formed a quorum block certificate and locally applied the certified
block. The optimization removes a process boundary between mempool batching and
certification.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-combined25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/combined25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/combined25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/combined25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/combined25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/combined25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/combined25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_fast_return` | `961ms` | `1109ms` | `1111ms` |
| `client_visible_finality_round` | `768ms` | `867ms` | `908ms` |
| `certified_round` wall clock | `834ms` | `945ms` | `978ms` |
| `submit_to_client_visible_finality_estimated` | `886ms` | `998ms` | `1034ms` |
| `submit_to_finality` | `1804ms` | `2016ms` | `2145ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `95ms` |
| local vote | `105ms` |
| vote requests | `257ms` |
| certificate | `106ms` |
| local apply | `170ms` |
| deferred send launch | `2ms` |
| post-apply status | `7ms` |

Checks:

- 25 iterations passed;
- every round used the combined mempool/certify command;
- every round launched 4 deferred certified-send workers;
- every deferred send report was preserved under the evidence logs path and
  verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- generated node/private wallet material was removed after the run;
- redaction scan over the preserved evidence directories found no private key,
  seed, mnemonic, SSH credential, or password markers.

Read: this was the best measured local path before local-vote overlap. The source-process fast
return is now subsecond at p50, and the in-round client-visible point is well
under one second at p50 and p95. The p95 full source return is still just above
one second, and full `submit_to_finality` still includes peer convergence plus
a separate finality RPC. The next bottleneck is not certified fanout; it is the
remaining submit/RPC/process boundary and the vote-request/local-apply floor
inside the round.

### Local 25-Round Overlapped Local-Vote Experiment

Change:

- after writing the block proposal, the source validator now starts its own
  local block vote on a worker thread;
- remote vote requests begin immediately instead of waiting for the local vote
  to finish;
- certificate aggregation still waits for the local vote file and the remote
  vote evidence;
- quorum certificate plus verified local apply remains the fast-return floor;
- deferred certified sends and peer-ack evidence remain enabled.

This is a scheduling change, not a consensus-rule change. The optimization
removes local vote construction from the critical path when remote vote fanout
is slower. Stage timings should therefore not be summed linearly: local vote
and remote vote requests overlap.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-overlap25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/overlap25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/overlap25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/overlap25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/overlap25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/overlap25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/overlap25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_fast_return` | `904ms` | `991ms` | `1055ms` |
| `client_visible_finality_round` | `700ms` | `765ms` | `828ms` |
| `certified_round` reported total | `702ms` | `766ms` | `833ms` |
| `submit_to_client_visible_finality_estimated` | `820ms` | `894ms` | `960ms` |
| `submit_to_finality` | `1756ms` | `1927ms` | `1995ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `112ms` |
| local vote, overlapped | `155ms` |
| vote requests | `268ms` |
| certificate | `126ms` |
| local apply | `178ms` |
| deferred send launch | `2ms` |
| post-apply status | `7ms` |

Checks:

- 25 iterations passed;
- every round used the combined mempool/certify command;
- every round launched 4 deferred certified-send workers;
- every deferred send report was preserved under the evidence logs path and
  verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- generated node/private wallet material was removed after the run;
- redaction scan over the preserved evidence directories found no private key,
  seed, mnemonic, SSH credential, or password markers.

Read: this was the best measured local path before hot finality receipt emission. Compared with the combined
run, p50 source-process fast return improved from `961ms` to `904ms`, and p95
fast return improved from `1109ms` to `991ms`. The estimated submit-to-client
visible path is now subsecond through p99. The remaining in-round floor is
remote vote fanout plus certificate aggregation plus local apply. The remaining
full-finality floor is still benchmark structure: peer convergence and a
separate finality RPC after the fast-return point.

### Local 25-Round Hot Finality Receipt Experiment

Change:

- the peer-certified round report now includes `local_hot_finality`, a
  `postfiat-tx-finality-v1` report built from the quorum certificate, the
  locally applied receipt, and the local post-apply block tip;
- the benchmark can set `HOT_FINALITY_RECEIPT=1` and consume that finality
  object immediately after local apply instead of waiting for peer convergence
  and issuing a separate `tx` RPC;
- deferred peer-send reports, peer service reports, convergence checks, and
  state verification still run before the benchmark report is accepted.

This is a client-response-path change. It does not change the block proposal,
vote, certificate, local apply, or peer fanout invariants. It moves the
client-visible finality receipt to the point where the source validator already
has the accepted receipt under a quorum-certified block.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-hotfinality25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/hotfinality25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/hotfinality25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/hotfinality25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/hotfinality25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/hotfinality25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 \
HOT_FINALITY_RECEIPT=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/hotfinality25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_finality` | `889ms` | `1015ms` | `1033ms` |
| `submit_to_fast_return` | `864ms` | `990ms` | `1005ms` |
| `client_visible_finality_round` | `661ms` | `748ms` | `773ms` |
| `submit_to_client_visible_finality_estimated` | `778ms` | `880ms` | `907ms` |
| hot finality extraction | `10ms` | `13ms` | `14ms` |

Consensus-stage p50 timings:

| Stage | p50 |
|---|---:|
| proposal | `113ms` |
| local vote, overlapped | `137ms` |
| vote requests | `246ms` |
| certificate | `128ms` |
| local apply | `170ms` |
| deferred send launch | `2ms` |
| post-apply status | `7ms` |

Checks:

- 25 iterations passed;
- every round used the combined mempool/certify command;
- every round emitted a hot finality object for the submitted transaction;
- the hot finality response used schema `postfiat-tx-finality-v1`,
  `verification_mode: selected-block-hot-path`, an accepted receipt, and a
  quorum certificate;
- every round launched 4 deferred certified-send workers;
- every deferred send report was preserved under the evidence logs path and
  verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- generated node/private wallet material was removed after the run;
- redaction scan over the preserved evidence directories found no private key,
  seed, mnemonic, SSH credential, or password markers.

Read: this is the current best measured local path. Compared with the overlap
run, measured full `submit_to_finality` p50 improved from `1756ms` to `889ms`,
because the client-visible finality receipt no longer waits for post-return
peer convergence and a separate `tx` RPC. The p95 full path is still just over
one second (`1015ms`). The next tail cut is to remove the remaining submit RPC
and per-round process overhead by turning submit, certification, and finality
receipt emission into one long-running write path.

### Local 25-Round One-Process Submit/Certify Hot-Finality Experiment

Change:

- `transport-peer-certified-mempool-round` accepts `--signed-transfer-file`;
- the source validator admits the signed transfer into its local mempool inside
  the same command that batches, certifies, applies, and emits the hot finality
  receipt;
- the benchmark defaults to `SUBMIT_IN_CERTIFY=1`, so it no longer launches a
  separate submit RPC server for the measured path;
- the report records `mempool_submit_ms`, `submit_rpc_ms`, and
  `submit_admission_ms` separately, so the removed RPC cost is not confused with
  free admission;
- deferred peer-send reports, peer service reports, convergence checks, and
  state verification still run before the benchmark report is accepted.

This is still a local controlled-testnet benchmark. It does not change quorum
rules, block validation, or peer fanout invariants. It removes one process/RPC
boundary from the measured write path.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-oneprocess25 \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/oneprocess25/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/oneprocess25/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/oneprocess25/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/oneprocess25/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/oneprocess25/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 \
HOT_FINALITY_RECEIPT=1 \
SUBMIT_IN_CERTIFY=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/oneprocess25/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_finality` | `876ms` | `969ms` | `987ms` |
| `submit_to_fast_return` | `831ms` | `930ms` | `945ms` |
| `submit_to_client_visible_finality_estimated` | `799ms` | `893ms` | `909ms` |
| `client_visible_finality_round` | `722ms` | `807ms` | `829ms` |
| `submit_admission` / `mempool_submit` | `48ms` | `54ms` | `54ms` |
| `mempool_batch` | `27ms` | `35ms` | `36ms` |
| hot finality extraction | `10ms` | `13ms` | `13ms` |

Checks:

- 25 iterations passed;
- every round admitted the signed transfer inside the combined
  mempool/certify command;
- every round emitted a hot finality object for the admitted transaction;
- the artifact gate required `submit_to_finality.p95_ms < 1000`,
  `submit_to_finality.p99_ms < 1000`, and
  `submit_to_client_visible_finality_estimated.p99_ms < 1000`;
- every round launched 4 deferred certified-send workers;
- every deferred send report verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- generated node/private wallet material was removed after the run;
- redaction scan over the preserved evidence directories found no private key
  or seed markers.

Read: this is the current best measured local path. The local 5-validator
controlled harness now has subsecond full `submit_to_finality` through p99
while preserving quorum certification, local apply, deferred peer fanout
evidence, convergence, and state verification. The remaining product work is
not a new quorum rule; it is to move this one-process path into the persistent
write service and then re-run it on the live controlled fleet.

### Local 25-Round Persistent Finality RPC Experiment

Change:

- added an explicitly gated RPC method,
  `mempool_submit_signed_transfer_finality`;
- the method is disabled unless `rpc-serve` is started with
  `--allow-mempool-submit-finality`;
- unlike normal read RPCs, this controlled write method runs in process instead
  of going through the child-per-request isolation path;
- the method admits the signed transfer, runs the same peer-certified
  mempool/certify round, and returns the `postfiat-tx-finality-v1` receipt in
  the RPC response;
- finality writes are serialized by a server-side lock so concurrent request
  workers do not race the same validator state height;
- the benchmark still validates the full round report, deferred peer-send
  reports, peer service reports, convergence, and state verification after the
  user-visible response.

This is the first local benchmark of the intended persistent write edge shape:
the source RPC process is already running before the timer starts, and the
measured path is the client request/response that carries finality.

Command:

```bash
RUN_ID=fast-finality-exp-20260605-persistent-rpc25b \
ROUNDS=25 \
BASE_DIR=reports/testnet-fast-finality-experiment/persistent-rpc25b/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/persistent-rpc25b/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/persistent-rpc25b/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/persistent-rpc25b/testnet-tx-finality-latency-benchmark.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/persistent-rpc25b/logs/local-harness.json \
LOCAL_APPLY_BEFORE_CERTIFIED_SEND=1 \
DEFER_CERTIFIED_SENDS=1 \
COMBINE_MEMPOOL_CERTIFY=1 \
HOT_FINALITY_RECEIPT=1 \
SUBMIT_IN_CERTIFY=1 \
PERSISTENT_FINALITY_RPC=1 \
scripts/testnet-tx-finality-latency-benchmark --rounds 25
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/persistent-rpc25b/testnet-tx-finality-latency-benchmark.json`

Key timings:

| Metric | p50 | p95 | p99 |
|---|---:|---:|---:|
| `submit_to_finality` | `786ms` | `864ms` | `905ms` |
| persistent finality RPC | `786ms` | `864ms` | `905ms` |
| `submit_to_fast_return` | `786ms` | `864ms` | `905ms` |
| `submit_to_client_visible_finality_estimated` | `747ms` | `824ms` | `863ms` |
| `client_visible_finality_round` | `695ms` | `735ms` | `760ms` |
| peer-certified reported total | `708ms` | `748ms` | `773ms` |
| submit admission | `47ms` | `58ms` | `60ms` |

Checks:

- 25 iterations passed;
- every round used the persistent finality RPC method;
- the finality RPC response carried schema
  `postfiat-rpc-mempool-submit-signed-transfer-finality-v1` and embedded a
  confirmed `postfiat-tx-finality-v1` receipt;
- the artifact gate required `submit_to_finality.p50_ms < 800`,
  `submit_to_finality.p95_ms < 900`, `submit_to_finality.p99_ms < 950`, and
  `persistent_finality_rpc.p99_ms < 950`;
- every round validated the underlying peer-certified mempool-round report;
- every deferred send report verified the peer ack;
- every peer validator service accepted one vote and one certified batch;
- all validators converged;
- state verification passed on every validator;
- generated node/private wallet material was removed after the run;
- redaction scan over the preserved evidence directories found no private key
  or seed markers.

Read: this is the current best measured local path. Compared with the previous
one-process CLI benchmark, p50 `submit_to_finality` improved from `876ms` to
`786ms`, p95 from `969ms` to `864ms`, and p99 from `987ms` to `905ms`. The
remaining local floor is now mostly the peer-certified round itself: remote
vote requests, certificate aggregation, and local apply.

### Slow-Peer Quorum-Early Smoke

Command:

```bash
BASE_DIR=reports/testnet-fast-finality-experiment/quorum-early/nodes \
LOG_DIR=reports/testnet-fast-finality-experiment/quorum-early/logs \
REPORT=reports/testnet-fast-finality-experiment/quorum-early/testnet-transport-peer-certified-quorum-early.json \
HARNESS_REPORT=reports/testnet-fast-finality-experiment/quorum-early/logs/local-harness.json \
scripts/testnet-transport-peer-certified-quorum-early-smoke
```

Result: passed.

Report:
`reports/testnet-fast-finality-experiment/quorum-early/testnet-transport-peer-certified-quorum-early.json`

Key facts:

- 5 validators.
- Source node: `validator-1`.
- Slow node: `validator-4`.
- Quorum: `4`.
- Vote count: `4`.
- Remote vote count: `3`.
- Slow peer recorded as unresolved.
- Certified send to slow peer was skipped.
- Online quorum converged.
- Slow peer remained unchanged.
- Vote requests completed in `273ms`.
- Certified sends completed in `280ms`.

Read: quorum-early behavior already exists and works under one intentionally
slow non-quorum peer. The fast-finality experiment should reuse this property
for normal user finality instead of waiting for all peer broadcast work before
returning.

### Live Canaries And Upgrade Boundary

Attempted:

```bash
RUN_ID=fast-finality-exp-20260605-live-wallet \
ROOT_DIR=reports/testnet-fast-finality-experiment/live-wallet \
LOG_DIR=reports/testnet-fast-finality-experiment/live-wallet/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/live-wallet/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/live-wallet/testnet-live-wallet-finality.json \
scripts/testnet-live-wallet-finality
```

and after adding the explicit direct-state opt-in to the harness transfer
round:

```bash
RUN_ID=fast-finality-exp-20260605-live-wallet-v2 \
ROOT_DIR=reports/testnet-fast-finality-experiment/live-wallet-v2 \
LOG_DIR=reports/testnet-fast-finality-experiment/live-wallet-v2/logs \
PRIVATE_DIR=reports/testnet-fast-finality-experiment/live-wallet-v2/private-wallet-material \
REPORT=reports/testnet-fast-finality-experiment/live-wallet-v2/testnet-live-wallet-finality.json \
scripts/testnet-live-wallet-finality
```

Result: blocked before producing a live wallet-finality report.

The failure happened in `run_transfer_round` while invoking the deployed
remote `/usr/local/bin/postfiat-node batch-transfer` for the initial funding
round. The command emitted the CLI usage/direct-state guard footer and exited
before a certified round was produced. A follow-up validator doctor passed:

`reports/testnet-fast-finality-experiment/postfailed-live-doctor/testnet-live-validator-doctor.json`

The live fleet remained converged at height `4`; no successful live write
finality evidence was produced by this experiment.

Follow-up harness work replaced that stale direct-state funding step with the
normal mempool path and taught the live wallet-finality script to call the
new gated write edge:

- initial funding uses `mempool-submit-transfer` plus `mempool-batch`, then a
  peer-certified batch round;
- the spend leg sends a local-only RPC request to
  `mempool_submit_signed_transfer_finality`;
- the write server must report `mempool_submit_signed_transfer_finality_count`
  equal to `1` and `mempool_submit_finality_enabled=true`;
- the response must carry
  `postfiat-rpc-mempool-submit-signed-transfer-finality-v1` with an embedded
  confirmed `postfiat-tx-finality-v1` receipt.

The live fleet is currently six validators across three machines, not the
older five-validator harness default. With `VALIDATORS=6`, the live canary
advanced farther but still did not produce a live finality benchmark:

- the funding round selected `validator-5` as proposer for height `5`;
- the funding round failed during peer-certified batch transport with repeated
  empty-frame vote-response reads;
- a follow-up validator doctor passed and showed the live fleet still healthy:
  `reports/testnet-fast-finality-experiment/post-canary-doctor/testnet-live-validator-doctor.json`.

The deployed live binary also lacks the new
`--allow-mempool-submit-finality` surface. I built the current release binary
and attempted a controlled six-validator binary rollout:

```bash
VALIDATORS=6 \
POSTFIAT_CONFIRM_LIVE_PRIVACY_BINARY_UPGRADE=1 \
RUN_ID=fast-finality-exp-20260605-binary-upgrade \
ROOT_DIR=reports/testnet-fast-finality-experiment/binary-upgrade \
LOG_DIR=reports/testnet-fast-finality-experiment/binary-upgrade/logs \
REPORT=reports/testnet-fast-finality-experiment/binary-upgrade/testnet-live-orchard-binary-upgrade.json \
scripts/testnet-live-orchard-binary-upgrade
```

Result: failed, then rolled back.

Evidence:

- failed upgrade report:
  `reports/testnet-fast-finality-experiment/binary-upgrade/testnet-live-orchard-binary-upgrade.json`;
- post-upgrade doctor:
  `reports/testnet-fast-finality-experiment/post-upgrade-doctor/testnet-live-validator-doctor.json`;
- rollback report:
  `reports/testnet-fast-finality-experiment/binary-rollback/testnet-live-binary-rollback.json`;
- post-rollback doctor:
  `reports/testnet-fast-finality-experiment/post-rollback-doctor/testnet-live-validator-doctor.json`.

The current release binary installed with hash
`f246d0345bd7481720bf40ecf27e176d7dcedd891d3174a174c52dab0d730b53`, but the
post-upgrade doctor failed: services were stuck `activating`,
`all_services_active=false`, and `all_state_verified=false`. The same live
data was interpreted under genesis hash
`cff4d9a909ff19345ec115e972c5f29ab2c4fa2256a80b4075508f98a15db42b802677dc32f9952b2aaf51d8dccad083`
instead of the running chain's
`f28f182e4c5a84dfeff653213cc9341797ca4002e813705d143d3cf0934a171144e9ae99ce5a8b3ab56575e611369344`.
That is a state-compatibility blocker, not a latency result.

The rollback restored binary hash
`922c0ab630b0eced22c0c0a5c475ac4a93c272e1460c923986d5f856f98b98ac`. The
post-rollback doctor passed with all six validators active, all state verified,
block height `4`, chain id `postfiat-testnet-candidate`, genesis hash
`f28f182e4c5a84dfeff653213cc9341797ca4002e813705d143d3cf0934a171144e9ae99ce5a8b3ab56575e611369344`,
and state root
`a1553742fc4e5a197dca07c8a09c21c81f9377eff6b0dd492e84c0fe0290a9f9ba510d112160915a69c075f2e31d6558`.

### Live Controlled-Fleet Persistent Finality Canary

Follow-up compatibility work made the current binary replay the existing live
ledger instead of forcing a re-genesis:

- restored the live genesis hash domain to the historical May-era encoding;
- accepted historical governance-registry replay under the old update domain
  while keeping current writes on the current domain;
- added a legacy JSON replay-root fallback for old block headers, with new
  blocks continuing to use the current canonical replay root;
- selected transport peers from the active governance validator set instead of
  the stale seven-peer topology, so the six-validator fleet no longer dials the
  inactive `validator-6`;
- installed the explicit controlled-testnet public-bind override
  `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1` for validator and RPC units.

Compatibility and rollout gates:

| Gate | Report | Result |
|---|---|---|
| copied-live binary compatibility | `reports/testnet-live-binary-compatibility/fast-finality-exp-20260605-compat-copy6/testnet-live-binary-compatibility.json` | passed |
| live binary rollout | `reports/testnet-live-orchard-binary-upgrade/fast-finality-exp-20260605-live-upgrade2/testnet-live-orchard-binary-upgrade.json` | passed |
| post-upgrade validator doctor | `reports/testnet-live-validator-doctor/fast-finality-exp-20260605-post-upgrade2-doctor/testnet-live-validator-doctor.json` | passed |
| live wallet-finality canary | `reports/testnet-live-wallet-finality/fast-finality-exp-20260605-live-wallet-finality4/testnet-live-wallet-finality.json` | passed |
| post-write validator doctor | `reports/testnet-live-validator-doctor/fast-finality-exp-20260605-post-finality4-doctor/testnet-live-validator-doctor.json` | passed |

The live fleet is now running binary hash
`c597dd4b0d0dee8cdfc2f7ca4ea86418489616850fa2349fabf01b5627280142`.

Live canary facts:

- six controlled validators;
- initial height `6`, final height `8`;
- final transaction
  `996626848f07aab9ff38f76cbee11c3fb1b5d01e5e78c587a57f088e0fac814d442c0f7386d087f0c24cdf680987caee`;
- `submit_finality.total_ms`: `218.560222`;
- `submit_finality.certified_round_ms`: `204.938832`;
- `submit_finality.mempool_submit_ms`: `6.353909`;
- `submit_finality.mempool_batch_ms`: `6.773345`;
- finality response was accepted and confirmed under proof id
  `4ea4c6344cc991d23b3eeb44a052372d2f503414ff39d71265d4780256b219f38961417268ab6e6c7a3fd8d4f57a5a95`;
- read-side `tx_finality` lookup confirmed the same proof and certificate;
- the temporary write edge was SSH-local and bounded to `max_requests=1`, with
  `ok_count=1`, `error_count=0`, and
  `mempool_submit_finality_enabled=true`;
- the post-write doctor passed with all validator/RPC services active, all
  state verified, all validators converged at height `8`, state root
  `4cf914abf74ed87d607f13ef350db8cd2f166b8dd4411dba780e3d65b25301bf43d87bc67cffc83a7068d88fde5a4be6`,
  and tip hash
  `dd50feaa878d3389bc61caca1efd48ad0f584d94f5defc4cfe3f77ac5d23dc3cc0bef34b5f51a2c2516306e3c9570554`.

Read: the live controlled fleet now has a measured persistent finality edge.
The result is a canary, not a latency distribution: it proves that the
optimized path can be deployed onto the existing six-validator live ledger,
submit a real wallet transfer through a bounded write edge, return a
quorum-certified finality object in about `219ms`, and leave the fleet healthy
afterward. It does not claim public write exposure, public decentralization,
TPS, or adversarial WAN performance.

## Redaction

Generated private wallet material and local validator node data directories
were removed from the experiment tree after the run. A scan for obvious secret
fields under the local experiment report tree and live finality report paths
returned clean for:

- `private_key_hex`
- `master_seed_hex`
- `mnemonic`
- PEM private-key markers
- `ssh_cred`
- `password`

## Conclusion

The experiment supports the fast-finality direction and establishes both a
subsecond in-round certified-local finality point under the local 5-validator
harness and a successful live controlled-fleet persistent finality canary. It
does not complete the full public-performance milestone yet.

What is already true:

- local finality benchmark passes;
- slow-peer quorum-early behavior passes;
- fast-apply peer-certified mode passes 25 local rounds;
- deferred fast-return mode passes 25 local rounds;
- one-process submit/certify hot-finality mode passes 25 local rounds;
- persistent finality RPC mode passes 25 local rounds;
- local one-process `submit_to_finality` is subsecond through p99
  (`876ms` p50, `969ms` p95, `987ms` p99);
- local persistent-RPC `submit_to_finality` is subsecond through p99
  (`786ms` p50, `864ms` p95, `905ms` p99);
- `client_visible_finality_round` is subsecond at p50, p95, and p99 in the
  fast-apply run, and p50/p95 subsecond in the preserved-artifact deferred run;
- one-process `submit_to_fast_return` is `831ms` p50, `930ms` p95, `945ms`
  p99;
- persistent-RPC `submit_to_fast_return` is `786ms` p50, `864ms` p95, `905ms`
  p99;
- read finality lookup is already cheap, around `75ms` p95;
- validators converge after the local benchmark;
- the current binary is compatible with the existing live ledger;
- the persistent finality RPC write edge was deployed in a bounded
  SSH-local canary on the six-validator live fleet;
- the live canary returned confirmed finality for a wallet transfer in
  `218.560222ms`;
- the live post-write validator doctor passed with all services active and all
  validators converged at height `8`.

What is not yet true:

- there is not yet a 25-round live controlled-fleet latency distribution;
- the local 100-round regression gate has not yet been run;
- the write edge remains explicitly gated and was exercised only through a
  bounded controlled canary, not exposed as a public write endpoint;
- the evidence packet is not yet packaged as a publication-ready artifact.

## Next Code Experiment

Expand the live controlled-fleet canary into a distribution and package it:

1. Keep the fleet on binary
   `c597dd4b0d0dee8cdfc2f7ca4ea86418489616850fa2349fabf01b5627280142` unless a
   newer binary first passes copied-live compatibility.
2. Run a 25-round live controlled-fleet wallet-finality benchmark through the
   same bounded persistent finality RPC edge.
3. Keep the write edge SSH-local or otherwise explicitly controlled, not public
   read-only RPC.
4. Keep deferred send reports and post-run convergence checks mandatory.
5. Run local 100-round persistent RPC as the regression gate.
6. Run the slow-peer quorum-early smoke after the live benchmark.
7. Package the reports, binary hash, dirty-state report, redaction scan, and
   claim-boundary note as the milestone evidence packet.

Expected effect from current timings: the local persistent RPC harness shows
`submit_to_finality` p99 `905ms`, and the first live canary returned in
`218.560222ms`. The live controlled fleet should still be evaluated against a
distribution with a wider p95 allowance for remote machine and network jitter.

Acceptance for the next experiment:

- 25-round live controlled-fleet wallet-finality benchmark passes through the
  persistent finality RPC edge;
- live `submit_to_finality` p50 <= `1500ms`, p95 <= `2500ms`;
- local 25-round persistent RPC benchmark continues to pass as a regression
  gate;
- slow-peer quorum-early still passes;
- no validator divergence after background certified sends complete;
- live wallet-finality harness either passes or records a precise deployed CLI
  mismatch with first-line stderr.
