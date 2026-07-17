# Controlled Testnet Burndown

Status: canonical CTO execution list
Date: 2026-05-14
Scope: PostFiat L1 v2 controlled testnet

This is the unified burndown list for moving the current MVP through controlled
testnet. `roadmap.md` remains the milestone roadmap; this file is the working
checklist.

Privacy-specific execution is tracked in
`docs/status/privacy-production-burndown.md`. That file defines the separate
semi-production and production-grade privacy gates; this controlled-testnet
burndown only tracks the transparent PQ settlement launch boundary.

Short overnight hardening/tooling work is tracked in
`docs/status/overnight-launch-hardening-burndown.md`. Use that file for
bounded overnight runs focused on evidence refresh, RPC doctor tooling,
validator UX, monitors, and Python RPC client work.

XRPL-style feature parity work is tracked in
`docs/status/xrpl-feature-parity-burndown.md`. That file scopes payment memos,
issued assets/trustlines, escrow/atomic settlement, NFTs, and the later DEX
decision. The controlled-testnet launch boundary remains the transparent PQ PFT
settlement path unless that burndown is explicitly promoted into the active
launch scope.

Update rule: every implementation slice that materially changes readiness must
update this file or the referenced evidence report before handoff.

## Current Read

The optimized controlled-testnet release candidate has been cut and recut
against the fixed finality path. Controlled launch execution has now passed on
the live operator surface. The next blocker is not release packaging or initial
launch; it is post-launch hardening: sustained live soak, restart/load drills,
persistent write-edge installation/exposure evidence, public endpoint load
evidence, and independent operator onboarding/replay packets.

- Active target: controlled testnet over the transparent PQ settlement path.
- Current P0 code-review hardening status: live green for commit `2cae621`
  (`Harden validator transport and ordered commits`). The patch clears the P0
  review items found in validator transport, proposal signing policy,
  service-applied batch certification, RPC slowloris/resource bounds, and
  crash-atomic ordered commits. It adds ML-DSA-authenticated transport
  envelopes, signed proposal fail-closed defaults, certificate-required
  transport service apply, bounded concurrent RPC serving with a 64 KiB read
  cap, and an ordered commit recovery journal. Full local readiness passed:
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260514T211920Z.json`
  (`readiness_ok=true`, 4 validators, 3 rounds, final height 3). Final checks
  passed: `bash -n` on touched testnet scripts, `cargo fmt --check`,
  `cargo test --workspace --all-targets`, and `cargo clippy --workspace
  --all-targets -- -D warnings`; the non-gate certified-batch retry smoke also
  passed. Fresh live evidence is now cut for the same hardening slice: package
  `reports/testnet-live-operator-artifact-20260514T-p0-hardening-2cae621/packages/testnet-release-package-20260515T000031Z`,
  live redeploy
  `reports/testnet-live-launch-20260515T-p0-hardening-2cae621/testnet-release-live-launch.json`,
  wallet finality
  `reports/testnet-live-wallet-finality/p0-hardening-2cae621-20260515T0000/testnet-live-wallet-finality.json`,
  continuity/restart/outage/read-load/edge-load reports under the matching
  `p0-hardening-2cae621` report directories, ending at height 17 after the
  post-below-quorum continuity and RPC checks.
- Current completion estimate: local and remote optimized latency evidence are
  inside the controlled-launch targets, final candidate/operator/release
  status/controlled-launch evidence have been recut for the optimized path, and
  live install/start/convergence/certified-round/wallet-finality evidence now
  exists for the controlled operator deployment.
- Prior P0 blocker: local 5-validator submit-to-finality latency was too high
  and grew with height. Prior measured local path:
  `submit_to_finality` p50 `10687.461021ms`, p95 `18038.730873ms`, p99
  `18697.75935ms`; `certified_round` p50 `8987.836195ms`, p95
  `15407.30261ms`; `tx_finality_rpc` p50 `1138.107827ms`, p95
  `2023.420813ms`.
- Latency evidence:
  `reports/testnet-tx-finality-latency-benchmark/current-20260514T124520Z/testnet-tx-finality-latency-benchmark.json`.
- Stage-timing smoke evidence:
  `reports/testnet-tx-finality-latency-benchmark/timings-20260514T141353Z/testnet-tx-finality-latency-benchmark.json`.
  The one-round local 5-validator smoke reported `submit_to_finality`
  `3.497s`, peer-certified round `2.670s`, peer-certified internal total
  `2.644s`, serial vote requests `1.105s`, and serial certified sends
  `0.872s`.
- Parallel fanout evidence:
  `reports/testnet-tx-finality-latency-benchmark/parallel-fanout-5round-20260514T141813Z/testnet-tx-finality-latency-benchmark.json`.
  After parallel block-vote collection and parallel certified-batch broadcast,
  the local 5-validator 5-round smoke reported `submit_to_finality` p50
  `2.890s`, p95 `3.772s`; `certified_round` p50 `1.870s`, p95 `2.525s`;
  `tx_finality_rpc` p50 `0.477s`, p95 `0.670s`. Height growth remains visible:
  height 2 finalized in `2.119s`; height 6 finalized in `3.772s`.
- Optimized local gate evidence:
  `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`.
  After parallel vote collection, parallel certified-batch broadcast, hot-path
  vote/certificate validation without per-round full block replay, and
  `tx` finality hot-path lookup with explicit audit mode, the local
  5-validator 20-round benchmark reports `submit_to_finality` p50 `1.563s`,
  p95 `1.709s`, p99 `1.753s`; `certified_round` p50 `0.921s`, p95 `1.043s`;
  and `tx_finality_rpc` p50 `0.062s`, p95 `0.068s`.
- Optimized remote gate evidence:
  `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`.
  A 5-validator, 20-round proposer-routed normal-run remote smoke converged at
  height `20` and reports peer-certified round total p50 `1.032s`, p95
  `1.116s`, p99 `1.139s`; vote-request p50 `0.255s`, p95 `0.271s`; and
  certified-send p50 `0.339s`, p95 `0.371s`.
- Quorum-early allowed-peer-failure evidence:
  `reports/testnet-transport-peer-certified-quorum-early/current-20260514T165536Z/testnet-transport-peer-certified-quorum-early.json`.
  A 5-validator local TCP peer-certified round held one peer connection open
  without a vote response, returned after the 4-of-5 quorum formed, recorded
  the slow validator as an unresolved vote target and skipped certified-send
  target, certified four votes, converged the online quorum, and kept
  vote-request timing at `0.196s` against a `4s` peer timeout.
- Latency diagnosis: the old local bottleneck was harness-shaped: serial vote
  collection, serial certified-batch broadcast, and repeated O(height) full
  block replay on the hot path. The optimized local run clears the local gate,
  and allowed-peer-failure vote collection now returns after quorum instead of
  waiting for a slow non-quorum peer timeout. Remaining launch work is
  post-launch evidence and longer-run storage/runtime hardening.
- Latency target before controlled launch: local 5-validator p50 under 2s and
  p95 under 4s, with no linear height-growth. Initial remote 5-validator target:
  p50 2-5s and p95 5-10s. Tuned remote target after persistent runtime/storage:
  p50 1-3s.
- Architecture direction: first fix the current peer-certified path so it can
  be measured honestly; then decide whether to evolve ordering into a
  HotStuff-2/Jolteon-style two-chain path. Cobalt remains the governance /
  validator-set / registry-root layer, not the immediate latency bottleneck.
- Controlled-testnet candidate revision:
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5` (`Record optimized finality
  launch status`).
- Exact final evidence:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/testnet-release-final-candidate-20260514T-current-head-56db87a-optimized-latency.json`.
- Current release package:
  `reports/testnet-release-packages/testnet-release-package-20260514T145919Z`.
- Completed 5-validator soak:
  `reports/testnet-remote-soak-current-bucket-8h-v2/testnet-remote-soak-20260513T-current-bucket-8h-v2.json`.
  This soak ran on runtime candidate `0410884`; `56db87a` is a release-gate
  evidence recut that reused the completed soak checkpoint.
- Fresh exact-head 5-validator remote P0 control point:
  `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`
  for `c18d590` (`Add SDK TCP wallet flow example`). This proves the current
  gate set with the placement manifest, normal-run ordering, remote RPC edge
  load, restart, partial outage, RPC catch-up, validator-registry mutation,
  emergency key-rotation rehearsal, topology capture, history retention, history
  role policy, and SDK signer mode in the wallet finality path.
- Current clean-head controlled-launch evidence pack:
  `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`
  for `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`. It reports
  `git.dirty=false`, `status=passed`, requires the selected remote P0 report to
  use SDK signer mode, and proves the benchmark, Cobalt lifecycle, and
  registry-root binding subpacks used that same P0 report.
- Exact release join evidence for the current package:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/candidate/release-gate/release-gate/logs/exact-remote-join-dry-run.json`.
- Current release status:
  `reports/testnet-release-status-current-56db87a-optimized-latency/testnet-release-status-current-56db87a-optimized-latency.json`
  reports `status=ready` and `ready_for_controlled_launch=true` from the final
  candidate report plus operator launch packet.
- Live launch prep is now fail-closed through
  `scripts/testnet-controlled-launch-prep-check`. Current prep-check evidence:
  `reports/testnet-controlled-launch-prep-check/current-package-prep-check-20260514.json`.
  It proves the optimized public release package verifies, but also makes the
  live-install blocker explicit: the package topology uses placeholder
  `validator-N.testnet.local` hosts and the matching validator private material
  was intentionally removed after the exact-join rehearsal. The live operator
  artifact must therefore be a fresh credential-bound package with retained
  operator-private material, or an explicit host-alias plus matching private
  material handoff.
- Live launch execution now has a redacted executor:
  `scripts/testnet-release-live-launch`. It refuses to mutate remote services
  unless `POSTFIAT_CONFIRM_LIVE_LAUNCH=1` is set, reruns the prep check, uploads
  the exact release artifact and split private material, installs and starts one
  validator/RPC pair at a time, removes staged private material, verifies
  service activity and state, records convergence, and can submit one certified
  transparent round after launch.
- Operator-private launch prep passed:
  `reports/testnet-live-operator-artifact-20260514T-prep/launch-prep-check.json`.
  Exact remote fake-root join rehearsal for that artifact also passed:
  `reports/testnet-live-operator-artifact-20260514T-prep/remote-join-dry-run/testnet-release-remote-join-dry-run.json`.
- Live launch passed:
  `reports/testnet-live-launch-20260514T151903Z-head-9e4fb20-rerun5/testnet-release-live-launch.json`.
  It records five validator/RPC service pairs active across three machines,
  initial convergence at height 0, one certified transparent round, and
  post-round convergence at height 1.
- Live SDK wallet finality passed:
  `reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json`.
  It records a fresh SDK wallet funded at height 5, a live read-RPC fee quote,
  SDK signing, bounded SSH-local write-edge submission, peer-certified mempool
  ordering at height 6, `tx` finality through live read RPC, and five-validator
  convergence at height 6.
- First live post-launch continuity soak passed:
  `reports/testnet-live-continuity-soak/current-rerun-20260514T162207Z/testnet-live-continuity-soak.json`.
  It records five proposer-routed certified transparent rounds from height 11
  to height 16, one accepted transfer per block, and convergence after every
  round. This starts post-launch hardening but is not a long soak or TPS claim.
- Live restart drill passed:
  `reports/testnet-live-restart-drill/current-20260514T162808Z/testnet-remote-restart-drill-20260514T162808Z.json`.
  It restarted all five validator/RPC service pairs, verified services and
  state, and verified read RPC convergence at height 16.
- Post-restart ordering passed:
  `reports/testnet-live-continuity-soak/post-restart-20260514T162853Z/testnet-live-continuity-soak.json`.
  It records two certified transfer rounds after restart, advancing the network
  from height 16 to height 18 with convergence after every round.
- Live RPC edge-load passed:
  `reports/testnet-live-rpc-edge-load/current-20260514T163012Z/testnet-remote-rpc-edge-load.json`.
  It records three oversized request rejections per validator and a valid
  status response after pressure, with convergence at height 18.
- Live RPC read-load passed:
  `reports/testnet-remote-rpc-read-load/current-20260514T170449Z/testnet-remote-rpc-read-load.json`.
  It records 300 SDK-validated read requests across the five live RPC endpoints
  for `status`, `server_info`, `ledger`, `fee`, `validators`, and `manifests`.
  Overall read latency was p50 `0.344s`, p95 `0.672s`, and p99 `0.772s`;
  every response validated through `postfiat-rpc-sdk`, and all validators
  remained converged at height 41 after the run.
- Broad live RPC read-load passed:
  `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json`.
  It records 1,200 SDK-validated read requests across five live RPC endpoints
  for 12 read methods: `status`, `server_info`, `metrics`, `ledger`, `fee`,
  `validators`, `manifests`, `blocks`, `receipts`, `mempool_status`,
  `bridge_status`, and `shield_turnstile`. Overall latency was p50 `0.359s`,
  p95 `0.701s`, p99 `0.797s`; all validators remained converged at height
  164 after the run.
- Controlled write-edge policy audit passed:
  `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`.
  It verifies that the current release package's five validator RPC systemd
  units are read-only by default, the live SDK wallet finality write path was a
  bounded SSH-local temporary edge, the local write-edge pressure smoke passed,
  and `docs/runbooks/controlled-write-edge-policy.md` defines the service
  boundary. A persistent externally exposed write edge is still not installed.
- Longer live continuity soak passed:
  `reports/testnet-live-continuity-soak/longer-20round-20260514T163232Z/testnet-live-continuity-soak.json`.
  It records 20 proposer-routed certified transparent rounds from height 18 to
  height 38, one accepted transfer per block, convergence after every round,
  and peer-certified round totals between `0.283s` and `0.341s`. This is
  continuity evidence, not throughput evidence.
- Extended live continuity soak passed:
  `reports/testnet-live-continuity-soak/longer-100round-20260514T171905Z/testnet-live-continuity-soak.json`.
  It records 100 proposer-routed certified transparent rounds, one accepted
  transfer per block, convergence after every round, and final five-validator
  convergence at height 141. This extends continuity evidence; it is still not
  a TPS claim.
- Live single-validator partial-outage drill passed:
  `reports/testnet-live-partial-outage-drill/current-20260514T164149Z/testnet-remote-partial-outage-drill-20260514T164149Z.json`.
  It stopped validator 3, formed a 4-of-5 quorum certificate at height 39 from
  validator 4, recorded one failed vote request and one failed certified send
  for the offline validator, kept the online validators converged, restarted
  the offline validator, replayed the missed certified batch on the first
  attempt, and reconverged all five validators. Post-outage ordering then
  passed:
  `reports/testnet-live-continuity-soak/post-partial-outage-20260514T164318Z/testnet-live-continuity-soak.json`.
- Live below-quorum outage drill passed:
  `reports/testnet-live-below-quorum-outage-drill/live-below-quorum-rerun-20260514T181916Z/testnet-live-below-quorum-outage-drill-20260514T181916Z.json`.
  It stopped validators 4 and 3, left only three validators online against a
  four-vote quorum, attempted height 142, failed to form a certificate with
  `insufficient block votes: got 3, need 4`, proved no state advance at height
  141 while below quorum, restarted the stopped validators without advancing
  state, then certified height 142 with all five votes and final convergence.
  Post-recovery continuity then passed two more certified rounds to height 144:
  `reports/testnet-live-continuity-soak/post-below-quorum-continuity-20260514T182141Z/testnet-live-continuity-soak.json`.
- Live mixed read/write load passed:
  `reports/testnet-live-mixed-read-write-load/live-mixed-read-write-10x-20260514T185000Z/testnet-live-mixed-read-write-load.json`.
  It ran 600 SDK-validated read requests across 12 methods while ten
  certified transparent rounds advanced from height 154 to height 164, proved
  workload overlap, accepted the expected non-converged standalone read-load
  final samples during advancing writes, and then proved post-mixed
  five-validator convergence at height 164.
- Fresh live observability passed:
  `reports/testnet-remote-observability/live-observability-post-mixed-10x-20260514T185456Z/testnet-remote-observability.json`.
  It verified all five validator/RPC service pairs, read-RPC health,
  convergence at height 164 with zero height lag, and current data/log/event
  counters after the mixed read/write run.
- Live host-group outage drill passed:
  `reports/testnet-live-host-group-outage-drill/live-host-group-outage-20260514T191629Z/testnet-live-host-group-outage-drill.json`.
  It profiled the live topology, selected a two-validator operator-host group
  that can block quorum but cannot form quorum, stopped that group, proved the
  below-quorum attempt did not advance state, restarted it, and recovered with
  full convergence at height 165. This is honest capture-threshold evidence,
  not a decentralization claim.
- Live hardening evidence pack passed:
  `reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json`.
  It validates 17 live evidence entries, requires a 100-round continuity
  window, includes the below-quorum outage, host-group outage, mixed read/write
  load, and fresh observability checks, and summarizes max final height 165.
- Benchmark evidence pack v0 now has a reproducible generator:
  `scripts/testnet-benchmark-evidence-pack`. It verifies and aggregates the
  final candidate, release status, P0 gate, 8-hour soak checkpoint, local
  ML-DSA/certificate-size benchmark suite, wallet tx-finality evidence, RPC
  write-edge pressure, and observability/disk evidence. It is not a TPS claim;
  it now auto-discovers fresh remote-head P0 evidence and fails closed unless
  the wallet finality path uses SDK signer mode. WAN public endpoint load,
  target hardware memory profile, and end-to-end WAN bandwidth remain open.
- Cobalt lifecycle audit v0 now has a reproducible verifier:
  `scripts/testnet-cobalt-lifecycle-audit`. It aggregates current P0 governance
  evidence for admit, remove, suspend, reactivate, planned rotate-key,
  emergency rotate-key, live admission, live suspension, stale/tamper rejection,
  and remote post-change drills. It covers canonical-UNL lifecycle governance,
  not full non-uniform Cobalt trust views.
- Registry-root binding audit v0 now has a reproducible verifier:
  `scripts/testnet-registry-root-binding-audit`. It verifies current P0
  certificate artifacts and wallet tx-finality evidence bind compact votes to a
  validator registry root without embedding repeated public keys.
- Amendment lifecycle v0 now has a runnable smoke:
  `scripts/testnet-cobalt-amendment-lifecycle-smoke` and
  `docs/governance/cobalt-amendment-lifecycle.md`. The current path proves certified
  validator-set, crypto-policy, and bridge-witness-epoch amendments through
  governance batches, activation-height metadata, veto-window rejection,
  paused-amendment rejection, separate activation-record artifacts, automatic
  same-kind supersession records, rollback records, tampered-vote rejection,
  insufficient-support rejection, and ordered multi-record amendment replay
  packaging via `scripts/testnet-cobalt-amendment-replay-bundle`. The replay
  bundle is now checked by the node-owned
  `postfiat-node governance-amendment-replay-verify` verifier, including
  tamper rejection, and the controlled-launch evidence pack requires those
  node-verifier checks.
- Controlled launch evidence pack v0 now has a reproducible generator:
  `scripts/testnet-controlled-launch-evidence-pack`. It consumes the final
  candidate, operator launch packet, release status, selected remote P0 gate,
  benchmark pack, Cobalt lifecycle audit, registry-root binding audit, amendment
  lifecycle smoke, public claims checklist, and canonical docs into one redacted
  launch-evidence manifest. It requires the selected remote P0 report to use SDK
  signer mode and requires the benchmark, Cobalt lifecycle, and registry-root
  binding subpacks to use the same P0 report. It is the first place to check
  before reviewer/operator handoff.
- External reviewer command sheet:
  `docs/review/controlled-testnet-review-packet.md`. It names the exact
  evidence-pack head, reproduction commands, evidence map, review questions,
  and excluded-scope boundaries for consensus, crypto, RPC, governance, and
  operator review.
- The older exact final watcher for `0410884` remains useful long-soak/watch
  evidence, but `56db87a` is now the current release-gate/package control point.

## Priority Order

1. Run broader partition/load evidence against the launched controlled network.
   The 100-round live continuity soak, single-validator outage, below-quorum
   outage, host-group capture-threshold outage, initial live read-load, and a
   bounded mixed read/write load now exist; the remaining load work is
   external-WAN, broader partition/capture coverage, and longer mixed workload
   windows.
2. Install and evidence a persistent controlled write edge only when external
   write exposure is needed. The policy/audit now exists, the current wallet
   finality proof uses a temporary SSH-local bounded write edge, and public
   validator RPC remains read-only by default.
3. Publish and operationalize the transparent-chain candidate.
4. Package canonical Cobalt governance so validator lifecycle is auditable.
5. Harden public RPC and wallet surfaces enough for external participants.
6. Publish and iterate the quantum, finality, RPC, and validator-topology
   evidence package.
7. Advance Confidential Settlement v1 in parallel behind the existing privacy
   adapter boundary.

## Status Terms

- `Done`: implemented, checked, and has committed evidence.
- `In progress`: implementation or evidence exists but the exit condition is
  not fully met.
- `Open`: not yet implemented or not yet evidenced.
- `Later`: not required for the first controlled testnet cut.

## P0 Finality Latency Gate

Goal: move the transparent settlement path from correctness/evidence harness
latency to credible controlled-testnet latency without weakening deterministic
finality, ML-DSA validator authentication, or auditable certificate evidence.

Prior measured state:

- Local 5-validator submit-to-finality: p50 `10.687s`, p95 `18.039s`, p99
  `18.698s`.
- Local 5-validator certified round: p50 `8.988s`, p95 `15.407s`, p99
  `15.909s`.
- Local `tx` finality RPC: p50 `1.138s`, p95 `2.023s`, p99 `2.192s`.
- The height curve is unacceptable: iteration 1 / height 2 submitted to
  finality in `3.510s`; iteration 20 / height 21 submitted to finality in
  `18.698s`.

Current optimized local measured state:

- Local 5-validator submit-to-finality: p50 `1.563s`, p95 `1.709s`, p99
  `1.753s` across 20 rounds.
- Local 5-validator certified round: p50 `0.921s`, p95 `1.043s`, p99
  `1.060s`.
- Local `tx` finality RPC: p50 `0.062s`, p95 `0.068s`, p99 `0.069s`.
- Evidence:
  `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`.

Current optimized remote measured state:

- Remote 5-validator peer-certified round total: p50 `1.032s`, p95 `1.116s`,
  p99 `1.139s` across 20 proposer-routed normal-run rounds.
- Remote vote requests: p50 `0.255s`, p95 `0.271s`.
- Remote certified sends: p50 `0.339s`, p95 `0.371s`.
- Evidence:
  `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`.

Working targets:

- Immediate triage target: local 5-validator p50 `2-4s`.
- Controlled-launch target: local 5-validator p50 `<2s`, p95 `<4s`, with no
  linear height growth.
- Initial remote controlled target: 5-validator p50 `2-5s`, p95 `5-10s`.
- Tuned remote target after persistent runtime/storage: 5-validator p50
  `1-3s`.

Research inputs:

- Request:
  `docs/research-requests/consensus-finality-latency-research-request.md`.
- Responses:
  `docs/research-requests/claude_response.md` and
  `docs/research-requests/gemini_response.md`.
- CTO synthesis: use the Claude response as the stronger base, with a
  conservative implementation order. Do not start with a full protocol rewrite.
  First make the current peer-certified path production-shaped enough to
  measure honestly.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-LATENCY-001 | Stage-timing instrumentation | Done | Peer-certified round reports setup, proposal build, target selection, local vote, aggregate vote-request timing, per-peer vote request timings, certificate aggregation, aggregate certified-send timing, per-peer certified-batch send timings, local apply, post-apply status, and verification timings in machine-readable reports. Benchmark report also aggregates these into `latency.peer_certified_stage`. Evidence: `reports/testnet-tx-finality-latency-benchmark/timings-20260514T141353Z/testnet-tx-finality-latency-benchmark.json`. |
| CT-LATENCY-002 | Fix benchmark control surface | Done | Benchmark accepts `--rounds`, `--validators`, and `--report`, fails closed on unknown flags, and records config sources in the report. Evidence: `reports/testnet-tx-finality-latency-benchmark/cli-control-20260514T140900Z/testnet-tx-finality-latency-benchmark.json` shows `rounds=1`, `config.rounds_source="cli"`, and passed convergence/finality checks. |
| CT-LATENCY-003 | Parallel vote collection | Done | Current sync transport sends block-vote requests concurrently for the full-vote controlled path, validates each response, records per-peer timing/result evidence, preserves deterministic certificate ordering through validator-ordered certificate aggregation, and returns after quorum when `--allow-peer-failures` is enabled. Evidence: `reports/testnet-tx-finality-latency-benchmark/parallel-fanout-5round-20260514T141813Z/testnet-tx-finality-latency-benchmark.json` shows vote-request p50 `0.554s`, p95 `0.746s`, down from the prior one-round serial `1.105s`. Quorum-early evidence: `reports/testnet-transport-peer-certified-quorum-early/current-20260514T165536Z/testnet-transport-peer-certified-quorum-early.json` records one unresolved slow peer, 4-of-5 certification, online quorum convergence, and `0.196s` vote-request timing against a `4s` peer timeout. |
| CT-LATENCY-004 | Parallel certified-batch broadcast | Done | Current sync transport broadcasts certified batches concurrently, records per-peer ack/failure timing evidence, and removes serial peer latency from the certified round. Evidence: `reports/testnet-tx-finality-latency-benchmark/parallel-fanout-5round-20260514T141813Z/testnet-tx-finality-latency-benchmark.json` shows certified-send p50 `0.231s`, p95 `0.244s`, down from the prior one-round serial `0.872s`. |
| CT-LATENCY-005 | Indexed finality query | Done for controlled launch hot path | `tx` finality no longer calls full `verify_blocks` on the default user-facing hot path. Responses now carry `verification_mode="selected-block-hot-path"` and `block_log_verified=false`; callers can request `--audit-block-log` for full replay mode. SDK validation accepts both modes and still validates receipt/block/certificate shape. Evidence: `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json` shows `tx_finality_rpc` p50 `0.062s`, p95 `0.068s`. Persisted tx indexes remain part of CT-LATENCY-007 storage hardening. |
| CT-LATENCY-006 | Long-running validator service mode | Open | Validators can run once for multiple rounds with hot local state and without per-round service startup; benchmark proves same-height convergence and stable latency across at least 20 rounds. |
| CT-LATENCY-007 | Append/index storage path | Open | Commit path avoids rewriting aggregate history files on the hot path; blocks, receipts, certificates, and tx index are written atomically or append-only with crash-recovery checks. |
| CT-LATENCY-008 | Persistent network/runtime design | Open | Decide and implement the next runtime step after CT-LATENCY-001 through 007: persistent TCP first or Tokio/QUIC actor runtime; bounded queues and backpressure are mandatory. |
| CT-LATENCY-009 | Ordering protocol decision | Open | After runtime/storage measurements, decide whether current peer-certified ordering is sufficient for controlled testnet or whether to implement HotStuff-2/Jolteon-style two-chain ordering. |
| CT-LATENCY-010 | New release latency gate | Done for local and remote controlled latency | Local 5-validator gate is cleared by `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`: submit-to-finality p50 `1.563s`, p95 `1.709s`, p99 `1.753s` across 20 rounds. Remote 5-validator peer-certified latency is cleared by `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`: peer-certified total p50 `1.032s`, p95 `1.116s`, p99 `1.139s` across 20 normal-run rounds. |

## P0 Release Candidate

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-RC-001 | Complete tx-finality wallet/P0 evidence wiring | Done | Wallet sign-transfer smoke proves submitted spend through public `tx` finality RPC; P0 report includes proof id, certified block hash, certificate id, and read-only RPC evidence. |
| CT-RC-002 | Commit current evidence slice | Done | Commit includes the tx-finality evidence wiring and SDK validation fix; `git diff --check` and relevant checks pass. |
| CT-RC-003 | Rotate release evidence after commit | Done for candidate `56db87a` | Release gate, release-candidate gate, final candidate gate, exact artifact remote-join dry run, operator launch packet, release status, and controlled-launch evidence pack all point at revision `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`. Reopen after behavior-changing commits. |
| CT-RC-004 | Complete 8-hour remote soak | Done | Soak job succeeded for 29,237 seconds, 91 iterations, final height 92, zero lag, and all continuity/observability/tamper/restart/snapshot/RPC checks passing. |
| CT-RC-005 | Cut final controlled-testnet candidate | Done for candidate `56db87a` | Exact final candidate gate passed with completed soak checkpoint, SDK-signer P0 evidence, exact remote-join evidence, candidate-gate evidence, redaction checks, release package manifest, operator launch packet, release-status `ready_for_controlled_launch=true`, and controlled-launch evidence pack. |
| CT-RC-006 | Record CTO progress report | Done | This document and `docs/status/chain-state-current.md` summarize commit, checks, release evidence, soak result, and residual risks. |
| CT-RC-007 | Recut release candidate after latency gate | Done | Final candidate, release status, operator launch packet, exact remote-join rehearsal, and controlled-launch evidence pack were recut for optimized candidate `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`. Evidence: `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`. |

## P0 Cobalt Governance

Goal: move from partially complete Cobalt-derived canonical-UNL governance to
complete, auditable canonical Cobalt governance for controlled testnet.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-COBALT-001 | Canonical Cobalt status/spec | Done | `docs/governance/cobalt-canonical-mode.md` is the source of truth for current claims and safe marketing language. |
| CT-COBALT-002 | Controlled-testnet Cobalt execution plan | Done | `docs/governance/cobalt-controlled-testnet-plan.md` covers lifecycle semantics, artifacts, commands, evidence, and release gates. |
| CT-COBALT-003 | Signed operator manifest format | Done | `operator-manifest-create` and `operator-manifest-verify` bind validator id, master key, hot key, operator metadata, domain/contact, infrastructure labels, rotation state, signature, and manifest hash without writing private material to the public manifest. |
| CT-COBALT-004 | Genesis validator governance bundle | Done | `governance-genesis-bundle` and `governance-genesis-verify` bind the canonical initial validator set, manifests, registry root, quorum config, chain id, genesis hash, and protocol version; `scripts/testnet-governance-genesis-bundle-smoke` proves create/verify/tamper rejection/redaction. |
| CT-COBALT-005 | Lifecycle runbook and CLI audit | Done for canonical lifecycle v0 | `scripts/testnet-cobalt-lifecycle-audit` verifies current P0 evidence for admit, remove, suspend, reactivate, planned rotate-key, emergency rotate-key, live admission, live suspension, stale/tamper rejection, and remote post-change drills. `scripts/testnet-validator-registry-lifecycle-replay-bundle` packages the ordered admit/remove/suspend/reactivate lifecycle as a replay bundle, and `postfiat-node validator-registry-lifecycle-replay-verify` replays the bundle through node-owned registry root checks with out-of-order tamper rejection. Remaining expansion is independent-operator replay publication. |
| CT-COBALT-006 | Amendment lifecycle | Done for canonical lifecycle v0 | `docs/governance/cobalt-amendment-lifecycle.md`, `scripts/testnet-cobalt-amendment-lifecycle-smoke`, `scripts/testnet-cobalt-amendment-replay-bundle`, `postfiat-node governance-amendment-replay-verify`, `postfiat-node governance-replay-build --amendment-replay-bundle-file`, and `scripts/testnet-controlled-launch-evidence-pack` define, exercise, package, node-verify, replay-package-bind, and gate validator-set, crypto-policy, and bridge-witness-epoch amendments, activation-height metadata, veto-window rejection, paused-amendment rejection, separate activation-record artifacts, automatic same-kind supersession records, rollback records, tampered-vote rejection, insufficient-support rejection, ordered multi-record replay verification, and node-side tamper rejection. |
| CT-COBALT-007 | Registry-root binding audit | Done for current P0 evidence | `scripts/testnet-registry-root-binding-audit` verifies the current P0 certificate artifact metrics, each recorded certificate artifact, compact registry-root-bound votes, and wallet tx-finality RPC evidence. Rerun for each release candidate; broader future work is full trust-view root binding. |
| CT-COBALT-008 | Governance replay bundle v0 | Done for v0 | `governance-replay-build` creates and self-verifies canonical replay packages that bind the genesis governance bundle, signed operator manifests, registry update, optional amendment replay bundle, governance batch, post-change block/certificate, and tamper-rejection evidence. Ordered multi-record registry lifecycle bundles are now covered under CT-COBALT-005. |
| CT-COBALT-009 | Remote mutation drills | In progress | Remote evidence covers live suspension, key rotation or emergency rotation, stale-vote rejection, failed-leader recovery, partition safety, and catch-up after governance change. |
| CT-COBALT-010 | Release gate requires governance replay | Done for candidate `56db87a` | Readiness, P0, release, release-candidate, and final gates require governance replay plus genesis bundle evidence, and the final cut passed with exact release evidence. |

## P0 Transparent PQ Settlement

Goal: make the transparent chain externally credible as a quantum-oriented,
XRP-like settlement substrate.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-PQ-001 | ML-DSA transparent auth path | Done for MVP | Accounts and validators use ML-DSA-style signatures with deterministic wallet vectors and signed transfer evidence. |
| CT-PQ-002 | External signed transfer path | Done for MVP | Fee quote, offline signing, public RPC submit, mempool admission, batch apply, receipt, and account-state verification are evidenced. |
| CT-PQ-003 | Tx finality artifact | Done for MVP | Public RPC returns confirmed receipt plus certified block evidence and SDK validates it; wallet/P0 gate records proof id, certified block hash, certificate id, and read-only RPC evidence. |
| CT-PQ-004 | Fee/reserve/burn evidence | Done | Receipts, metrics, wallet/P0 reports, and fee quote reports expose minimum fee, charged fee, burned fee, state-expansion fee, reserve checks, burn totals, and the fact that the historical fee collector is not funded. Fresh evidence: `reports/testnet-fee-reserve-policy-refresh/fee-reserve-20260513T193228Z/testnet-fee-reserve-policy-smoke.json` and `reports/testnet-transfer-fee-quote-refresh/transfer-fee-quote-20260513T193236Z/testnet-transfer-fee-quote-smoke.json`; readiness/P0 gates require both checks. |
| CT-PQ-005 | Canonical transaction envelope audit | Done for transparent v1 | `docs/specs/transparent-transaction-envelope.md` freezes signing bytes, network/genesis/protocol binding, sequence, fee, operation fields, and replay behavior. `wallet_test_vector_is_deterministic_and_redacted` locks exact fixture signing bytes, signing hash, and tx id. The v1 envelope intentionally has no expiry field; any expiry must be a versioned envelope change. |
| CT-PQ-006 | Partial-history validator mode | In progress | Validators keep current state, recent blocks, recent certificates, recent receipts, checkpoints, and replay windows without needing full archival history. |
| CT-PQ-007 | Archive/indexer history path | In progress | `docs/status/controlled-testnet-history-roles.json` declares the controlled-testnet split between partial-history validators and at least one full-history archive role. Archive-window export/verify/import/backfill and policy smoke evidence prove the first operator model; account-history indexing and independent archive onboarding remain. |
| CT-PQ-008 | ML-DSA performance report | In progress | `postfiat-bench` now reports ML-DSA public-key bytes, private-key bytes, signature bytes, separate sign/verify timings, and registry-backed certificate-size rows for 4/5/7/10/21/33/100 validators. Smoke evidence: `reports/testnet-ml-dsa-performance/ml-dsa-verify-20260513T193753Z/testnet-ml-dsa-performance-20260513T193753Z.json`. Remaining work is release-grade CPU/memory/WAN/RPC-payload benchmarking on target hardware. |

## P0 RPC Surface

Goal: give external participants a predictable public read/write interface
without opening unbounded attack surfaces.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-RPC-001 | Read-only service discipline | Done for controlled live broad read-load | Public read methods are bounded, rate-limited, redaction-safe, and covered by local/remote smoke plus live read-load evidence. `scripts/testnet-remote-rpc-read-load` exercises SDK-built and SDK-validated read requests against the live controlled endpoints, including list-shaped read responses. Evidence: `reports/testnet-remote-rpc-read-load/current-20260514T170449Z/testnet-remote-rpc-read-load.json` records 300 validated requests across six methods, and `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json` records 1,200 validated requests across 12 methods with post-run convergence at height 164. Mixed read/write evidence now records 600 validated reads while certified writes advanced from height 154 to height 164: `reports/testnet-live-mixed-read-write-load/live-mixed-read-write-10x-20260514T185000Z/testnet-live-mixed-read-write-load.json`. Broader external-WAN and longer mixed workload load remain under CT-AUDIT-002. |
| CT-RPC-002 | Write edge hardening | Done for controlled policy; persistent install open | Signed-transfer submit has per-peer/global caps, invalid-signature metrics, oversized-envelope rejection, valid-read-after-pressure evidence, and a documented controlled write-edge boundary. Evidence: `docs/runbooks/controlled-write-edge-policy.md` and `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`. The audit proves packaged validator RPC units are read-only and the live wallet finality write edge was bounded/SSH-local. Remaining work is persistent external write-edge installation plus external-WAN load/auth evidence; do not claim unrestricted public write RPC. |
| CT-RPC-003 | XRP-like read aliases | Done for local/public RPC surface | `server_info`, `ledger`, `fee`, `validators`, and `manifests` are implemented as bounded read-only aliases; existing `tx`, `account`, and `transfer_fee_quote` remain stable. SDK request/response validation and read-only `rpc-serve` smoke evidence: `reports/testnet-rpc-read-alias-smoke/read-aliases-20260513T192121Z/testnet-rpc-read-alias-smoke.json`; readiness/P0 gates now require the smoke. |
| CT-RPC-004 | Finality proof validation | Done for compact certificate v0 | SDK rejects malformed finality responses, missing or mismatched registry-root-backed compact certificates, non-compact certificate public-key material, receipt mismatches, and private-key leaks. Local wallet/RPC smoke passed with the stricter SDK at `reports/testnet-wallet-sign-transfer-smoke/compact-certificate-check/testnet-wallet-sign-transfer-smoke.json`. Remaining expansion is end-to-end WAN public endpoint evidence. |
| CT-RPC-005 | Public RPC operator policy | Done for controlled testnet | `docs/runbooks/public-rpc-operator-policy.md` defines ports, timeouts, max requests, read/write defaults, logs, retention, firewall posture, verification commands, and safe remote write opt-in. First live RPC edge-load evidence passed in `reports/testnet-live-rpc-edge-load/current-20260514T163012Z/testnet-remote-rpc-edge-load.json`; live broad read-load evidence passed in `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json`. Broader external-WAN and mixed workload load remain under CT-AUDIT-002. |

## P0 Wallet

Goal: minimum credible wallet path for controlled testnet participants.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-WALLET-001 | Deterministic backup/restore | Done for MVP | Wallet seed backup/restore and public vectors are readiness-gated and private material is redacted. |
| CT-WALLET-002 | Offline quote/sign/submit | Done for MVP | Wallet signs quoted transfers and submits through public RPC with receipt verification. |
| CT-WALLET-003 | Tx finality in wallet flow | Done for MVP | Wallet smoke records the finality artifact for the spend it submits, and P0 evidence captures the proof id, certified block hash, certificate id, and read-only RPC evidence. |
| CT-WALLET-004 | Key rotation UX | Boundary documented for controlled testnet | Validator key rotation is implemented through governance and documented in `docs/runbooks/validator-emergency-key-rotation.md`. Account key rotation is explicitly not implemented for controlled testnet; `docs/specs/account-key-rotation-boundary.md` defines first-spend key binding, sweep-if-compromised UX, lost-key limits, and future versioned transaction requirements. |
| CT-WALLET-005 | SDK wrapper | Done for transport-free SDK v0 | RPC SDK now supports deterministic wallet backup creation from a master seed, wallet public-identity restore, quote-bound ML-DSA transfer signing without node CLI shelling, validated typed summaries for `account`, `receipts`, `transfer_fee_quote`, mempool submit, and `tx` finality, and a tx-finality poll request helper from submitted tx id. `postfiat-rpc-sdk wallet-backup`, `wallet-identity`, and `wallet-sign-quote` expose the flow from the SDK binary; `scripts/testnet-sdk-wallet-cli-smoke` proves backup/identity/sign/submit-request construction, `SIGNER_MODE=sdk scripts/testnet-wallet-sign-transfer-smoke` proves SDK signing against the real quote -> public RPC submit -> apply -> `tx` finality path, and readiness/P0 now default to SDK signer mode with `P0_WALLET_SIGNER_MODE=sdk`. `crates/rpc_sdk/examples/tcp_wallet_flow.rs` is the packaged SDK transport example for the current newline-delimited TCP RPC surface. Latest local P0 evidence: `reports/testnet-p0-network-gate-local-sdk-signer-readiness/testnet-p0-network-gate-20260514T014214Z.json`. Remaining expansion is an HTTP/gateway wrapper example if public RPC is exposed behind HTTP infrastructure. |
| CT-WALLET-006 | Exchange/custody deposit model | Done for model v0 | `docs/specs/wallet-exchange-custody-model.md` specifies ML-DSA account semantics, no-BIP32/xpub constraints, unique deposit-address assignment, address-balance/finality attribution, withdrawal flow through SDK quote-bound signing, watch-only limits, and recovery/key-loss boundaries. Remaining production work is account-history indexing, hardware/external signer integration, formal backup encryption, and account key-rotation UX. |

## P0 Validators And Topology

Goal: controlled validator cohort is operationally real and honestly described.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-VAL-001 | Five-validator remote candidate | Done for exact-head final candidate | `56db87a` final candidate gate passed with the completed 8-hour soak checkpoint, SDK-signer P0 evidence, fresh exact artifact remote-join rehearsal, release package, operator launch packet, release status, and controlled-launch evidence pack. Fresh remote P0 evidence at `c18d590` covers 5 validators, placement manifest, topology capture, remote readiness, restart, partial outage, RPC catch-up, validator registry, emergency key rotation, history retention, history role policy, and SDK signer mode. |
| CT-VAL-002 | Placement/capture evidence | Done for controlled P0 and live capture threshold; public expansion still open | Fresh P0 includes manifest-backed placement capacity and topology capture evidence for 5 complete controlled targets. Live host-group outage evidence now proves the current controlled topology has a two-validator operator-host group that can block quorum but cannot form quorum, and that state does not advance while that group is offline. Independent public expansion diversity remains P1. |
| CT-VAL-003 | Restart/catch-up drills | Done for exact-head P0; started on live launch | Fresh P0 proves remote restart and RPC catch-up convergence across the 5-validator controlled cohort. Live post-launch restart evidence now also passes: `reports/testnet-live-restart-drill/current-20260514T162808Z/testnet-remote-restart-drill-20260514T162808Z.json`, followed by post-restart certified ordering in `reports/testnet-live-continuity-soak/post-restart-20260514T162853Z/testnet-live-continuity-soak.json`. |
| CT-VAL-004 | Fault drills | In progress | Fresh P0 proves one-validator outage, validator-registry suspension, post-suspend convergence, fault-tolerance evidence, and emergency key-rotation rehearsal. Live post-launch one-validator outage now also passes: `reports/testnet-live-partial-outage-drill/current-20260514T164149Z/testnet-remote-partial-outage-drill-20260514T164149Z.json`, followed by post-outage ordering in `reports/testnet-live-continuity-soak/post-partial-outage-20260514T164318Z/testnet-live-continuity-soak.json`. Extended post-launch continuity passes 100 rounds to height 141 in `reports/testnet-live-continuity-soak/longer-100round-20260514T171905Z/testnet-live-continuity-soak.json`. Live below-quorum outage now also passes: `reports/testnet-live-below-quorum-outage-drill/live-below-quorum-rerun-20260514T181916Z/testnet-live-below-quorum-outage-drill-20260514T181916Z.json`, followed by post-recovery ordering to height 144 in `reports/testnet-live-continuity-soak/post-below-quorum-continuity-20260514T182141Z/testnet-live-continuity-soak.json`. Live host-group outage passed at height 165 with no state advance while a blocking operator-host group was offline. Mixed read/write load advanced the network to height 164 while reads validated. Remaining launch hardening is broader partition/load evidence under longer mixed workload conditions. |
| CT-VAL-005 | Operator onboarding package | Done for controlled launch | Release package contains binary, public config, provision scripts, systemd units, package verifier, and package-local runbook. `docs/runbooks/controlled-testnet-operator-launch.md` plus `scripts/testnet-operator-launch-packet` validate final candidate evidence and generate a redacted operator launch packet. Independent public-operator expansion remains CT-VAL-007. |
| CT-VAL-006 | Validator pruning policy | Done for exact-head final candidate; independent archive onboarding remains | `history-status`, `history-prune-plan`, archive-handoff create/verify, archive-window export/verify/import, read-only RPC/SDK `archive_window`, source-driven `archive-window-backfill`, destructive `history-prune`, and `history-prune-recover` now report partial-history posture, fail closed unless a deterministic handoff proof covers the prune boundary, write `history_checkpoint.json`/`history_prune_pending.json`/`history_prune_journal.json`, store imported windows under `history_archive_windows/`, prove interrupted recovery plus post-prune source backfill/append/verification, and are wired into readiness/P0/release gates. `docs/status/controlled-testnet-history-roles.json` plus `scripts/testnet-history-role-policy-smoke` define and verify archive/indexer operating responsibility. Fresh remote P0 evidence is `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`. Runbook is `docs/runbooks/validator-history-retention.md`. |
| CT-VAL-007 | Independent-operator expansion | Later | Move from controlled self-operated cohort to independent operators across providers, jurisdictions, legal domains, and funding sources. |
| CT-VAL-008 | Live launch prep gate | In progress | `scripts/testnet-controlled-launch-prep-check` now verifies package manifest, topology host binding, operator-private validator material, fake-root install/key validation, credential parsing, and redaction before live machine mutation. Current evidence: `reports/testnet-controlled-launch-prep-check/current-package-prep-check-20260514.json` fails the existing public package for placeholder topology and missing retained private material. Exit condition: the exact artifact selected for live launch passes this check. |
| CT-VAL-009 | Live launch executor | In progress | `scripts/testnet-release-live-launch` performs the controlled mutable launch from an exact release package after prep-check pass and explicit `POSTFIAT_CONFIRM_LIVE_LAUNCH=1`. Exit condition: live launch report passes, services are active, validator state converges, one certified transparent round is recorded, and post-launch docs point at the evidence. |

## P0 Quantum Posture

Goal: make the quantum-resistance claim precise and evidenced.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-Q-001 | ML-DSA account and validator signatures | Done for MVP | Transparent auth path uses ML-DSA-style signatures and rejects algorithm/key/signature tampering. |
| CT-Q-002 | Domain separation freeze | In progress | Chain id, genesis hash, protocol version, tx ids, certificates, receipts, Cobalt evidence, and wallet vectors use documented labels. |
| CT-Q-003 | Known-answer and recovery vectors | In progress | Public deterministic wallet/signing vectors are stable and covered by readiness. |
| CT-Q-004 | External crypto audit prep | Open | Crypto inventory, KATs, domain labels, dependency versions, key lifecycle, and failure modes are packaged for review. |
| CT-Q-005 | ML-KEM note envelopes | Open | Confidential Settlement v1 uses ML-KEM KEM/DEM note encryption with recipient, outgoing, auditor, and recovery envelopes. |

## Critical Parallel Privacy Track

Goal: ship Confidential Settlement v1 without pretending the current debug proof
adapter is production privacy.

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-PRIV-001 | Privacy claim boundary | Done | Docs state shielded semantics exist, debug proofs are not production privacy, and privacy remains a first-class product pillar. |
| CT-PRIV-002 | Public action/journal types | Partial | `OrchardShieldedAction` binds pool id, proof system, circuit id, flags, anchor, nullifiers, randomized verification keys, value commitments, output commitments, encrypted outputs, value balance, fee, optional 48-byte `external_binding_hash`, proof, spend signatures, and binding signature. `orchard_deposit_v1` adds a signed transparent funding envelope carrying funding transfer, amount, fee, policy id, and disclosure hash; `orchard_withdraw_v1` adds a transparent exit envelope carrying recipient, amount, fee, policy id, and disclosure hash. Asset-Orchard adds `asset_orchard_ingress_v1`, `shielded_swap_v1` carrying `AssetOrchardSwapAction`, and disclosed `asset_orchard_egress_v1`; internal swaps hide asset/value/party fields, while current egress reveals the exited note facts and is not private egress. `postfiat-orchard-disclosure-packet-v1` now records redacted note/finality evidence for locally decrypted outputs and has a local verifier report. Remaining: final multi-action transaction wrapper, private egress, and regulated disclosure policy. |
| CT-PRIV-003 | Orchard/Halo2 verification adapter | Done for first gated path | `crates/privacy_orchard` reconstructs an upstream Orchard bundle from PostFiat JSON, derives the PostFiat chain-bound authorizing sighash, verifies the real Halo2 proof, and verifies binding/spend signatures. |
| CT-PRIV-004 | Certified Orchard apply path | Done for local deposit/spend/withdraw v0 | Node has a local `orchard-action` verify/apply gate, CLI ordered shielded batch paths via `shield-batch-orchard`, `shield-batch-orchard-deposit`, and `shield-batch-orchard-withdraw` plus `apply-shield-batch`, local RPC request-file methods `shield_batch_orchard`, `shield_batch_orchard_deposit`, and `shield_batch_orchard_withdraw` for SDK-built Orchard batch envelopes, and a local 4-validator peer-certified shielded batch smoke at `scripts/testnet-orchard-peer-certified-smoke`. These paths persist verified nullifiers, output commitments, encrypted outputs, accepted anchors, recomputed Orchard root history, duplicate replay rejection, unretained-anchor rejection, direct transparent-to-Orchard deposits with signed funding burn, migrated-budget compatibility, signed fee burn, and ordered withdraw ledger crediting with external-envelope binding. Focused direct-deposit evidence: `cargo test -p postfiat-node orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note -- --nocapture`. Latest peer-certified output/spend/withdraw evidence: `reports/testnet-orchard-peer-certified-smoke/withdraw-v0-20260515T080523Z/testnet-orchard-peer-certified-smoke.json`. Remote Orchard batch creation from inline JSON now works through `rpc-serve --allow-orchard-batch-create` with child timeout, rate limits, and server-controlled spooling; file-path requests and direct `apply_shield_batch` are rejected. Latest RPC action evidence from `scripts/testnet-orchard-rpc-batch-create-smoke`: `reports/testnet-orchard-rpc-batch-create/orchard-rpc-batch-create-v0-20260515T111516Z/testnet-orchard-rpc-batch-create.json`. Remaining: fold direct deposit into peer-certified/live launch gates. |
| CT-PRIV-005 | Wallet scanning and encrypted notes | Partial | Node has deterministic Orchard wallet keygen, private wallet-key writes, full-viewing-key export, receive-only scanning with retained-root Merkle witness material, local `orchard-disclose` redacted packets and `orchard-disclosure-verify` reports for decrypted outputs, `orchard-deposit-create --amount N` for signed transparent-to-Orchard funding, `orchard-output-create` for wallet-created zero-value or migrated-budget nonzero Orchard output actions, `orchard-spend-create --amount N --fee N` for one-note private transfers with default or explicit change and signed minimum-enforced fee burn, `orchard-withdraw-create --to ADDRESS --amount N --fee N` for one-note shielded-to-transparent withdrawals, SDK request/response validation for `shield_batch_orchard`, `shield_batch_orchard_deposit`, and `shield_batch_orchard_withdraw`, `postfiat-node rpc --request-file` creation of Orchard action/deposit/withdraw batches, `scripts/testnet-orchard-wallet-finality-smoke` for local ordered wallet finality evidence through mint/migrate/output/spend/withdraw at heights 1-5 with redacted action JSON hashes, disclosure packet verification, snapshot-import disclosure verification, local prover/verifier timing, action/proof byte metrics, oversized and exact-size malformed proof/ciphertext fail-closed rejection, spend-batch replay rejection, batch-id tamper rejection, withdraw-envelope mismatch rejection, snapshot export/import, imported shielded-state verification, and imported change-note scanning, plus `scripts/testnet-orchard-peer-certified-smoke` for 4-validator peer-certified mint/migrate/output/spend/withdraw convergence and finality evidence. Latest local performance/disclosure/malformed-bound evidence: `reports/testnet-orchard-wallet-finality-smoke/perf-malformed-v0-20260515T103617Z/testnet-orchard-wallet-finality-smoke.json`. Latest peer-certified withdraw evidence: `reports/testnet-orchard-peer-certified-smoke/withdraw-v0-20260515T080523Z/testnet-orchard-peer-certified-smoke.json`. Remaining: fold direct deposit into smokes, scan/spend/disclose SDK helpers, and production envelope/key-management policy. |
| CT-PRIV-006 | Pricing and benchmarks | Partial | Positive value-balance Orchard fee burns and withdraws now have deterministic minimum-fee enforcement and receipts report charged/burned/minimum fees; withdraws include transparent account-creation state expansion fees when needed. The wallet finality smoke now records local proof construction time, ordered apply/verify time, proof bytes, ciphertext bytes, action bytes, disclosure verifier time, oversized proof/ciphertext rejection probes, and exact-size malformed proof/ciphertext rejection probes. Latest local performance evidence: `reports/testnet-orchard-wallet-finality-smoke/perf-malformed-v0-20260515T103617Z/testnet-orchard-wallet-finality-smoke.json` shows oversized proof/ciphertext fail closed in about 63ms/3ms and exact-size malformed proof/ciphertext fail closed in about 11.7s/11.4s before the valid ordered flow continues. `postfiat-node orchard-operator-policy` now publishes verifier posture, remote batch-create controls, and protocol caps; latest evidence: `reports/testnet-orchard-operator-policy/operator-policy-v1-20260515T110835Z/orchard-operator-policy.json`. Concurrent malformed-request edge-load evidence now exists at `reports/testnet-orchard-rpc-malformed-edge-load/orchard-rpc-malformed-edge-load-v0-20260515T113620Z/testnet-orchard-rpc-malformed-edge-load.json`: three exact-size malformed proof requests fail closed in about 13.5-13.8s, no child timeout fires, post-load `status` succeeds, and sampled parent+child RSS peaks at about 78.5 MB. Rate-limit evidence now exists at `reports/testnet-orchard-rpc-rate-limit/orchard-rpc-rate-limit-v0-20260515T114343Z/testnet-orchard-rpc-rate-limit.json`: per-peer and global Orchard batch-create caps reject excess requests without child timeout. Threshold gate evidence now exists at `reports/testnet-orchard-rpc-threshold-gate/orchard-rpc-threshold-gate-v0-20260515T115102Z/testnet-orchard-rpc-threshold-gate.json`: max malformed latency `30000ms`, max sampled RSS `524288KB`, and zero child timeouts; `scripts/testnet-benchmark-evidence-pack` now requires this gate before passing. Remaining: repeated p50/p95/p99 runs, target-hardware memory/RSS, RPC payload size, direct deposit/outer-envelope fee policy, and benchmark publication. |
| CT-PRIV-007 | Regulated disclosure flow | Partial | Local selective disclosure artifact exists: `orchard-disclose` writes a redacted `postfiat-orchard-disclosure-packet-v1` with note commitment/nullifier/value/memo, retained-root metadata, auditor instructions, and ordered-batch finality evidence when available. `orchard-disclosure-verify` validates packet hash/schema, chain/genesis context, archive commitment inclusion, and block/finality evidence; tampered packet content fails closed. Remaining: approved auditor/view-key policy, recovery flow, custodian workflow, and sanctions/travel-rule posture before production privacy claims. |

## P1 Benchmarks, Audit, And Docs

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CT-AUDIT-001 | Release package redaction | In progress | Release packages and reports scan clean for private keys, SSH material, passwords, and machine credential leaks. |
| CT-AUDIT-002 | Reproducible benchmark bundle | In progress | `scripts/testnet-benchmark-evidence-pack` produces v0 controlled-launch evidence for ML-DSA byte constants/timings, certificate-size model and artifacts, wallet tx finality, RPC write-edge pressure, Orchard RPC malformed-proof latency/RSS/rate-limit thresholds, observability/disk, 8-hour soak, final-candidate readiness, release-status linkage, selected remote P0 binding, and SDK signer mode. Live controlled RPC read-load now has separate evidence. Remaining exit work is release-grade external-WAN endpoint load, target-hardware CPU/memory, end-to-end bandwidth, and repeated run windows with exact clean build hashes. |
| CT-AUDIT-003 | External review packet | Done for v0 packet | `docs/review/controlled-testnet-review-packet.md` plus `scripts/testnet-controlled-launch-evidence-pack` provide the reviewer-facing narrative, command sheet, evidence manifest, claim boundaries, and verifier commands across final candidate, operator launch packet, release status, selected remote P0 gate, benchmark, Cobalt lifecycle, registry-root binding, amendment lifecycle, public claims, and canonical docs. Current clean-head pack: `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`. Reopen after behavior-changing release cuts or when assigning named external reviewers. |
| CT-AUDIT-004 | Public claims checklist | Done for controlled launch | `docs/status/public-claims-checklist.md` maps allowed and disallowed public language to committed code/docs/evidence, including explicit boundaries for full Cobalt, production privacy, decentralization, bridge custody, and TPS/latency claims. `scripts/testnet-public-claims-check` validates required claim ids and boundaries. |
| CT-AUDIT-005 | Live hardening evidence pack | Done for current live hardening window | `scripts/testnet-live-hardening-evidence-pack` validates live launch, live SDK wallet finality, continuity windows, restart, fresh live observability, RPC edge-load, RPC read-load, partial outage, below-quorum outage, host-group outage, mixed read/write load, and controlled write-edge policy evidence into one redacted manifest. Current pack: `reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json`, with 17 evidence entries, max continuity rounds 100, min read requests 1200, min mixed read requests 600, and max final height 165. |

## Formal Document Map

- `roadmap.md`: milestone roadmap and broad burn-down.
- `docs/status/controlled-testnet-burndown.md`: this unified working list.
- `docs/status/controlled-launch-execution-milestone.md`: exact checklist for
  executing the current controlled launch and recording post-launch evidence.
- `docs/status/chain-state-current.md`: grounded capability snapshot.
- `docs/status/research-response-synthesis.md`: CTO synthesis of research-agent
  reports.
- `docs/status/public-claims-checklist.md`: evidence-backed public claims and
  overclaim boundaries for controlled launch.
- `docs/specs/transparent-transaction-envelope.md`: canonical transparent
  transfer v1 signing envelope and fixture.
- `docs/specs/wallet-exchange-custody-model.md`: controlled-testnet
  exchange/custody deposit model and ML-DSA watch-only constraints.
- `docs/specs/account-key-rotation-boundary.md`: account key-rotation
  non-claim boundary and future transaction requirements.
- `docs/governance/cobalt-canonical-mode.md`: canonical Cobalt claim boundary.
- `docs/governance/cobalt-controlled-testnet-plan.md`: canonical P0 Cobalt
  governance execution plan for controlled testnet.
- `docs/governance/cobalt-amendment-lifecycle.md`: current v0 amendment
  lifecycle scope and remaining delayed-activation/veto gaps.
- `docs/runbooks/controlled-testnet-operator-launch.md`: launch captain and
  validator/RPC operator checklist for the current controlled-testnet package.
- `scripts/testnet-controlled-launch-prep-check`: fail-closed local/operator
  gate that proves the selected launch package has matching topology,
  per-validator private material, fake-root install/key validation, and
  redacted reports before live service mutation.
- `scripts/testnet-release-live-launch`: explicit-confirmation remote executor
  for the mutable controlled launch from a release package after prep check.
- `docs/runbooks/sdk-wallet-flow.md`: non-operator SDK wallet flow for quote,
  sign, submit, and finality verification.
- `docs/whitepaper.md`: public-facing technical thesis.
- `work_archive/2026-05-13-superseded-markdown/`: archived raw research and
  request markdown; historical input only.
