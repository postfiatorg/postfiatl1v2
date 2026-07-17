# Subsecond Controlled-Testnet Finality Milestone

Status: closed for controlled-testnet transparent PFT path
Date: 2026-06-05
Scope: PostFiat L1 v2 transparent PQ settlement on the controlled testnet

This milestone turns the current controlled-testnet transaction path from
"fast enough for launch evidence" into an always-on, reproducibly measured
subsecond user-finality path. It does not broaden public write exposure, claim
mainnet decentralization, or include privacy proving performance.

## 2026-06-06 Acceptance Result

The controlled-testnet milestone is closed by:

`reports/testnet-fast-finality-milestone/fast-finality-exp-20260606-evidence-packet/README.md`

Acceptance readout:

| Requirement | Result |
|---|---|
| Live controlled fleet wallet-finality p50 <= `1500ms`, p95 <= `2500ms` | passed: p50 `290.35356ms`, p95 `375.418875ms`, p99 `380.18433ms` over 25 rounds |
| Live write edge remains controlled, not public | passed: every round used a non-public, single-request bounded write edge |
| Local persistent RPC 100-round regression | passed: 100/100 rounds, final height `101` |
| No linear height-growth across 25 live rounds | passed: live final heights strictly increased |
| No linear height-growth across 100 local rounds | passed: local final height `101` after 100 confirmed rounds |
| Slow-peer quorum-early still works | passed: quorum `4/5`, slow node `validator-4`, no wait on laggard |
| Post-run validator services and state remain healthy | passed: six-validator doctor green at height `60` |
| Evidence packet is reproducible and sanitized | passed: packet includes `SHA256SUMS.txt`, script hashes, dirty-state disclosure, and redaction scan |

The closure packet supports controlled-testnet engineering claims only. It does
not prove public write admission, public WAN throughput, mainnet
decentralization, adversarial public RPC load, privacy proving performance, or
same-ledger continuity from the 2026-05-14 launch window.

## Current Read

The controlled validator fleet is live at the service/state layer after the
successful persistent-finality canary on 2026-06-05:

- Report:
  `reports/testnet-live-validator-doctor/fast-finality-exp-20260605-post-finality4-doctor/testnet-live-validator-doctor.json`.
- Six validators passed.
- Validator and RPC services were active.
- State verification, history readiness, local key checks, account transaction
  indexes, private-file permissions, binary hash convergence, chain id, block
  height, tip hash, and state root all converged.
- Current height was `8` on `postfiat-testnet-candidate`.
- The live binary hash is
  `c597dd4b0d0dee8cdfc2f7ca4ea86418489616850fa2349fabf01b5627280142`.
- Genesis hash was
  `f28f182e4c5a84dfeff653213cc9341797ca4002e813705d143d3cf0934a171144e9ae99ce5a8b3ab56575e611369344`.
- State root was
  `4cf914abf74ed87d607f13ef350db8cd2f166b8dd4411dba780e3d65b25301bf43d87bc67cffc83a7068d88fde5a4be6`.

The live canary exercised the explicitly gated persistent finality write edge:

- Report:
  `reports/testnet-live-wallet-finality/fast-finality-exp-20260605-live-wallet-finality4/testnet-live-wallet-finality.json`.
- Final transaction:
  `996626848f07aab9ff38f76cbee11c3fb1b5d01e5e78c587a57f088e0fac814d442c0f7386d087f0c24cdf680987caee`.
- Initial height `6`, final height `8`.
- `submit_finality.total_ms`: `218.560222`.
- `submit_finality.certified_round_ms`: `204.938832`.
- `submit_finality.mempool_submit_ms`: `6.353909`.
- `submit_finality.mempool_batch_ms`: `6.773345`.
- The finality response was accepted and confirmed, and a read-side
  `tx_finality` lookup confirmed the same proof and certificate.
- The write edge was temporary, SSH-local, and bounded to `max_requests=1`.

The public validator-list and scoring surfaces are also reachable:

- `https://postfiat.org/testnet_vl.json` returned HTTP 200, schema version 2,
  with VL sequence `6` and 20 validators in the signed blob.
- `https://scoring-testnet.postfiat.org/api/scoring/config` returned HTTP 200
  with cadence `168h`, score cutoff `40`, max UNL size `20`, and min gap `5`.
- `https://scoring-testnet.postfiat.org/api/scoring/rounds?limit=10` returned
  HTTP 200 with seven published rows; latest completed scoring row was round
  `8`, VL sequence `6`, completed on 2026-06-02T17:33:05Z.

This supports the narrow statement: the controlled live fleet, public
validator-list/scoring surfaces, and bounded persistent-finality write path are
up after the fast-finality live canary. It does not, by itself, prove public
write throughput, public write exposure, or adversarial WAN performance.

It also does not prove same-ledger continuity from the 2026-05-14 launch
window. The 2026-05-17 validator-doctor evidence was height `134` with genesis
hash `bbf70950a1d288dbf4177d9b48474ad2d72f8eede839e289acd692b11a2843b2d133235c62692f6fc14aca19505bc7b8`.
The 2026-06-05 live validator-doctor evidence is now height `8` with genesis hash
`f28f182e4c5a84dfeff653213cc9341797ca4002e813705d143d3cf0934a171144e9ae99ce5a8b3ab56575e611369344`.
Those are different ledger instances under the same controlled-testnet chain
id. Remote service and data-file timestamps place the current instance's
install window between 2026-05-18 and 2026-05-21 UTC.

First fast-finality experiment readout:
`docs/status/fast-finality-experiment-2026-06-05.md`.

The first fast-apply, deferred fast-return, combined mempool/certify,
overlapped local-vote, hot-finality-receipt, one-process submit/certify, and
persistent finality RPC implementation now has local evidence:

- `--local-apply-before-certified-send` added to peer-certified round and loop
  mode;
- `--defer-certified-sends` added to return after quorum certificate plus
  local apply while file-backed send workers continue certified peer fanout;
- `transport-peer-certified-mempool-round` added to create the mempool batch
  and run the peer-certified round inside one node process;
- local vote construction now overlaps remote vote requests after the proposal
  is written;
- the peer-certified round emits a hot `postfiat-tx-finality-v1` object after
  quorum certificate plus verified local apply;
- the combined mempool/certify command can now admit a signed transfer and
  certify it in one process with `--signed-transfer-file`;
- `rpc-serve` has an explicitly gated `mempool_submit_signed_transfer_finality`
  method for the controlled write edge;
- local 25-round fast-apply benchmark passed;
- local 25-round deferred fast-return benchmark passed;
- local 25-round combined mempool/certify fast-return benchmark passed;
- local 25-round overlapped local-vote benchmark passed;
- local 25-round hot-finality-receipt benchmark passed;
- local 25-round one-process submit/certify hot-finality benchmark passed;
- local 25-round persistent finality RPC benchmark passed;
- `client_visible_finality_round`: p50 `847ms`, p95 `938ms`, p99 `960ms`;
- `submit_to_client_visible_finality_estimated`: p50 `978ms`, p95 `1081ms`,
  p99 `1103ms`;
- deferred fast-return `client_visible_finality_round`: p50 `901ms`, p95
  `997ms`, p99 `1011ms`;
- deferred fast-return `submit_to_fast_return`: p50 `1148ms`, p95 `1266ms`,
  p99 `1278ms`;
- combined fast-return `client_visible_finality_round`: p50 `768ms`, p95
  `867ms`, p99 `908ms`;
- combined fast-return `submit_to_fast_return`: p50 `961ms`, p95 `1109ms`,
  p99 `1111ms`;
- combined `submit_to_client_visible_finality_estimated`: p50 `886ms`, p95
  `998ms`, p99 `1034ms`;
- overlapped fast-return `client_visible_finality_round`: p50 `700ms`, p95
  `765ms`, p99 `828ms`;
- overlapped fast-return `submit_to_fast_return`: p50 `904ms`, p95 `991ms`,
  p99 `1055ms`;
- overlapped `submit_to_client_visible_finality_estimated`: p50 `820ms`, p95
  `894ms`, p99 `960ms`;
- full overlapped `submit_to_finality` remains p50 `1756ms` because the
  harness still waits for peer convergence and performs a separate finality
  RPC.
- hot finality `submit_to_finality`: p50 `889ms`, p95 `1015ms`, p99 `1033ms`;
- hot finality `submit_to_client_visible_finality_estimated`: p50 `778ms`, p95
  `880ms`, p99 `907ms`;
- hot finality `client_visible_finality_round`: p50 `661ms`, p95 `748ms`, p99
  `773ms`.
- one-process `submit_to_finality`: p50 `876ms`, p95 `969ms`, p99 `987ms`;
- one-process `submit_to_fast_return`: p50 `831ms`, p95 `930ms`, p99 `945ms`;
- one-process `submit_to_client_visible_finality_estimated`: p50 `799ms`, p95
  `893ms`, p99 `909ms`;
- persistent finality RPC `submit_to_finality`: p50 `786ms`, p95 `864ms`, p99
  `905ms`;
- persistent finality RPC `submit_to_client_visible_finality_estimated`: p50
  `747ms`, p95 `824ms`, p99 `863ms`;

The first live deployment attempt for the persistent finality RPC found a
release boundary rather than a latency number:

- the live fleet is six validators across three machines;
- the deployed binary does not expose `--allow-mempool-submit-finality`;
- the current release binary with hash
  `f246d0345bd7481720bf40ecf27e176d7dcedd891d3174a174c52dab0d730b53` failed
  the live doctor after rollout, with services stuck `activating` and state
  verification failing;
- the post-upgrade doctor interpreted the same live data under genesis hash
  `cff4d9a909ff19345ec115e972c5f29ab2c4fa2256a80b4075508f98a15db42b802677dc32f9952b2aaf51d8dccad083`,
  while the recovered live chain uses
  `f28f182e4c5a84dfeff653213cc9341797ca4002e813705d143d3cf0934a171144e9ae99ce5a8b3ab56575e611369344`;
- rollback restored the previous binary and the post-rollback doctor passed.

Evidence:

- failed upgrade:
  `reports/testnet-fast-finality-experiment/binary-upgrade/testnet-live-orchard-binary-upgrade.json`;
- post-upgrade doctor:
  `reports/testnet-fast-finality-experiment/post-upgrade-doctor/testnet-live-validator-doctor.json`;
- rollback:
  `reports/testnet-fast-finality-experiment/binary-rollback/testnet-live-binary-rollback.json`;
- post-rollback doctor:
  `reports/testnet-fast-finality-experiment/post-rollback-doctor/testnet-live-validator-doctor.json`.

Follow-up compatibility work fixed that boundary. The current binary now
replays the existing live ledger, the six-validator rollout passed, and the
bounded live wallet-finality canary returned confirmed finality in
`218.560222ms`.

## Existing Latency Baseline

The current docs identify the old bottleneck as harness and hot-path structure:
serial vote collection, serial certified-batch broadcast, and repeated full
block replay on the transaction-finality path.

Known optimized evidence:

- Local 5-validator 20-round finality benchmark:
  `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`.
- Local `submit_to_finality`: p50 `1563ms`, p95 `1709ms`, p99 `1753ms`.
- Local `certified_round`: p50 `921ms`, p95 `1043ms`, p99 `1060ms`.
- Local `tx_finality_rpc`: p50 `62ms`, p95 `68ms`, p99 `69ms`.
- Remote 5-validator normal-run peer-certified round:
  `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`.
- Remote peer-certified round: p50 `1032ms`, p95 `1116ms`, p99 `1139ms`.

The chain already cleared the original controlled-launch target. The next
useful target is a stricter UX milestone: client-visible finality under one
second on the controlled path, backed by fresh live evidence.

## Milestone Objective

Ship and document a controlled-testnet fast-finality path where a transparent
PFT transfer can be submitted, certified by quorum, observed by read RPC, and
replayed from evidence with these gates:

- Local controlled 5-validator `submit_to_finality`: p50 <= `900ms`, p95 <=
  `1500ms`, p99 <= `2000ms`.
- Local controlled 5-validator `submit_to_certified`: p50 <= `650ms`, p95 <=
  `1000ms`.
- Local `tx_finality_rpc`: p95 <= `75ms`, with full replay retained as an audit
  mode outside the hot path.
- Live controlled fleet wallet-finality run: p50 <= `1500ms`, p95 <= `2500ms`
  for submit-to-finality over the current six-validator deployment.
- No linear height-growth across at least 100 local rounds and 25 live rounds.
- Validator services, RPC services, state verification, account indexes,
  history readiness, and convergence remain green after the benchmark.

These thresholds are intentionally controlled-testnet thresholds. Public WAN
latency, public load balancers, persistent public write admission, and
mainnet-grade adversarial conditions remain separate launch surfaces.

## Work Plan

### M0: Truth Source Refresh

Create a current evidence bundle before touching the hot path:

- `scripts/testnet-live-validator-doctor`
- public VL/scoring HTTP checks
- latest read-only RPC load check against the live endpoints, if public RPC
  endpoints are advertised for the current deployment
- current wallet-finality run through the approved controlled write path

Exit criterion: one report names the exact code revision, live report paths,
height, validator count, public endpoint status, and whether write finality was
freshly exercised.

### M1: Metric Contract

Freeze the milestone metrics in a small machine-readable gate:

- `quote_rpc`
- `sign`
- `submit_rpc`
- `mempool_batch`
- `certified_round`
- `submit_to_certified`
- `tx_finality_rpc`
- `submit_to_finality`
- per-stage peer-certified timing: proposal, local vote, vote requests,
  certificate, local apply, certified sends, post-apply status

Exit criterion: the benchmark fails closed if any required field is missing,
if round count is too small, if state does not converge, or if height-growth is
visible.

### M2: Hot Submit Path

Reduce client-visible latency before changing consensus shape:

- keep fee quote and signing local/SDK-native;
- avoid unnecessary process startup or shell round trips in submit/finality
  scripts;
- keep the transaction-finality lookup indexed and hot;
- keep full block replay out of the user-finality path and behind explicit
  audit mode;
- make the write edge return the finality receipt or a stable finality lookup
  key immediately after quorum certification.

Exit criterion: local `submit_rpc + tx_finality_rpc` remains under `150ms` p95
while preserving invalid-signature and rate-limit counters.

### M3: Quorum-Certified Early Return

Treat quorum certification as the first client-visible finality event:

- return when canonical quorum has certified and the local validator has
  applied the certified batch;
- continue best-effort certified-batch broadcast to lagging peers after the
  client-visible certificate exists;
- record unresolved or slow peer targets in evidence;
- keep convergence checks mandatory after each run.

Exit criterion: one intentionally slow non-quorum peer does not force
client-visible finality to wait for the full peer timeout, and the final report
shows which peer was slow.

Current status: locally implemented as a controlled persistent RPC write path
and exercised once on the live six-validator controlled fleet.
The peer-certified round can now apply locally before certified-send fanout
after a quorum certificate exists, defer certified peer sends to file-backed
workers, create the mempool batch inside the same node command that runs
certification, overlap local vote construction with remote vote requests, emit
a hot finality receipt after local apply, admit the signed transfer inside that
same combined command, and return the finality receipt from an explicitly
gated in-process RPC method. The local persistent-RPC 25-round report shows
`786ms` p50, `864ms` p95, and `905ms` p99 full submit-to-finality. The
first live canary returned confirmed finality in `218.560222ms`. The remaining
work is to expand the live canary into a 25-round distribution and package the
evidence packet.

### M4: Persistent Runtime Path

Remove harness-only latency from the measured path:

- use long-running validator/RPC services for benchmark traffic;
- avoid per-round node startup;
- avoid per-round full state reconstruction;
- avoid reopening all peer channels when a connection pool or persistent
  transport is available;
- keep crash-recovery and ordered-commit journal tests in the gate.

Exit criterion: 100 local rounds and 25 live rounds show no linear height
growth, and restart/recovery evidence still passes.

### M5: Evidence Packet

Publish a single milestone evidence packet:

- code revision and dirty-state report;
- validator-doctor report;
- local benchmark report;
- live wallet-finality report;
- live RPC read-load report;
- rate-limit/write-edge policy report if write admission is exercised;
- redaction check proving no credentials, IPs, private keys, or passwords are
  in the packet;
- short claim-boundary file stating exactly what the packet proves and what it
  does not prove.

Exit criterion: the evidence packet is reproducible from scripts and can be
linked from `docs/status/controlled-testnet-burndown.md`,
`docs/status/chain-state-current.md`, and `docs/whitepaper.md`.

## Non-Goals

- No claim that the public Internet write edge is open.
- No TPS claim.
- No mainnet readiness claim.
- No privacy transaction/proving latency claim.
- No public decentralization claim.
- No validator-set governance claim beyond the current controlled cohort.

## Acceptance Checklist

- [x] Fresh live validator doctor passes.
- [ ] Fresh public VL/scoring checks pass.
- [ ] Local 100-round benchmark passes the milestone latency gate.
- [ ] Live 25-round wallet-finality benchmark passes the milestone latency
      gate or records the exact blocker.
- [ ] Slow-peer/quorum-early test passes.
- [ ] Restart after benchmark passes.
- [ ] RPC read-load after benchmark passes.
- [ ] Write-edge policy remains read-only by default, with explicit opt-in if
      exercised.
- [ ] Evidence packet is redacted and hashable.
- [ ] Canonical docs are updated with the new evidence and claim boundaries.

## Residual Risks

The likely remaining bottlenecks for a live distribution are certified-send
fanout, local apply cost, ML-DSA signature/certificate bandwidth, WAN jitter,
write-edge serialization, and harness overhead. The live `218.560222ms` canary
is promising but not a p95 claim; the next decision should be based on a
25-round live run plus a 100-round local regression gate.
