# PostFiat L1 Current State

Date: 2026-05-22
Status: canonical current-state reference

This document is a grounded status snapshot of the PostFiat L1 v2 codebase.
It is not a marketing summary. It separates implemented code, exercised
evidence, and remaining production gaps. Use this file with
`docs/status/controlled-testnet-burndown.md` before relying on older planning or
research markdown.

Older raw research prompts and agent responses are archived under
`work_archive/2026-05-13-superseded-markdown/`. The live synthesis of that work
is `docs/status/research-response-synthesis.md`.

## DATED CURRENT STATE

As of 2026-05-22:

- The review-remediation follow-up tightened controlled-testnet runtime
  boundaries without changing the production-claims boundary. Genesis hashing
  and replicated state-root hashing now use explicit length-delimited
  canonical encodings with golden vectors. Node file writes route through the
  shared storage atomic writer, and governance replay publication verifies
  through the same checked atomic publish path. `rpc-serve` now shares the
  controlled transport public-bind guard and spools child requests through
  private non-predictable temp directories. Debug proof construction has
  release-mode fail-closed gate tests. Cobalt example reports use a sandboxed
  helper for `REPORT` output. Raw Ambient P0/P1 rows are dispositioned in
  `docs/status/ambient-finding-disposition-ledger.json`; script and generated
  evidence rows remain separate burndown milestones, not fixed rows. The
  ledger root is now a manifest with row shards under
  `docs/status/ambient-finding-disposition-ledger/`.

- The Orchard/Halo2 privacy fast path now has SDK-facing ordered batch
  envelopes for the local request-file path. `postfiat-rpc-sdk` builds and
  validates both `shield_batch_orchard` and `shield_batch_orchard_withdraw`
  requests/responses, `postfiat-node rpc --request-file` can create Orchard
  shielded and withdraw batches from those envelopes, and
  `scripts/testnet-orchard-wallet-finality-smoke` proves SDK-built output,
  spend, and withdraw batches apply and finalize. Latest redacted local
  evidence:
  `reports/testnet-orchard-wallet-finality-smoke/sdk-withdraw-v0-20260515T082626Z/testnet-orchard-wallet-finality-smoke.json`.
  The evidence records finality for mint, migrate, Orchard output, Orchard
  spend, and Orchard withdraw at heights 1-5; `orchard_withdraw_total=1`;
  redacted Orchard action JSON byte lengths and SHA3-384 hashes; replay/tamper
  rejections; snapshot export/import into a fresh data dir; imported shielded
  state verification; and imported Orchard change-note scanning. This is still
  privacy alpha work, not a production privacy claim.

- RPC/SDK privacy guardrails now treat Orchard raw private witness/key field
  names (`master_seed_hex`, `spending_key_hex`, `full_viewing_key_hex`, and
  `rseed`) as key material in public-shaped request/response/event/error
  validation. This is a defensive blocker against accidentally exposing local
  scan/spend witness material through future RPC surfaces.

- The Orchard/Halo2 path now also has local peer-certified multi-validator
  evidence. `scripts/testnet-orchard-peer-certified-smoke` booted four local
  validators, ran shielded mint, migration into `orchard-v1`, Orchard output,
  and Orchard spend as peer-certified shielded batches, then verified all
  validators converged at height 4 with the same state root and block tip.
  Latest redacted evidence:
  `reports/testnet-orchard-peer-certified-smoke/20260515T065718Z/testnet-orchard-peer-certified-smoke.json`.
  The report records `peer_certified_orchard_ok=true`, finality for
  mint/migrate/output/spend at heights 1-4, `verified_all=true` for shielded
  state, recipient/change-note scanning, 4 Orchard outputs, 4 nullifiers, and a
  2-unit Orchard fee burn.

- The Orchard action format now has an optional 48-byte
  `external_binding_hash` inside the Orchard authorization signature domain.
  Existing action JSON remains backward compatible through serde defaults, but
  withdraw envelopes now bind transparent recipient, amount, fee, policy id,
  and disclosure hash into the signed Orchard action; disclosure-only envelopes
  can reuse the same mechanism. Mutation tests prove changing or removing the
  external binding hash makes verification fail with
  `binding_signature_invalid`.

- Orchard withdraw v0 is now implemented locally. `orchard-withdraw-create`
  builds a real one-note Orchard/Halo2 withdraw action and signs it over the
  external transparent envelope hash. `shield-batch-orchard-withdraw` wraps the
  action as `orchard_withdraw_v1`; ordered shielded apply verifies the hash,
  rejects mismatched recipient/amount/fee envelopes, burns the signed fee,
  records `withdraw_total`, updates Orchard root/nullifier state, and credits
  the transparent ledger in the same committed block. The same batch builder is
  now exposed through `postfiat-rpc-sdk` as
  `shield_batch_orchard_withdraw_request` and through `postfiat-node rpc
  --request-file` as `shield_batch_orchard_withdraw`. Focused evidence is the
  node integration test
  `orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers`, which
  passed on 2026-05-15 together with `cargo test -p postfiat-privacy-orchard`
  and `cargo check --workspace`. The local wallet finality smoke now also
  proves SDK-created withdraw batch generation,
  mint/migrate/output/spend/withdraw `tx` finality through height 5,
  post-withdraw
  `orchard_withdraw_total=1`, transparent ledger credit, snapshot import, and
  external-binding mismatch rejection:
  `reports/testnet-orchard-wallet-finality-smoke/sdk-withdraw-v0-20260515T082626Z/testnet-orchard-wallet-finality-smoke.json`.
  The local peer-certified smoke now also proves withdraw across four
  validators and five certified shielded rounds:
  `reports/testnet-orchard-peer-certified-smoke/withdraw-v0-20260515T080523Z/testnet-orchard-peer-certified-smoke.json`.
  This is still privacy alpha, not a production privacy claim.

- Orchard selective disclosure v0 is implemented as a local CLI path.
  `orchard-disclose` writes a redacted
  `postfiat-orchard-disclosure-packet-v1` for a decrypted Orchard output. The
  packet carries chain id, genesis hash, protocol version, note commitment,
  nullifier, value, memo, retained-root metadata, auditor instructions, and
  ordered-batch block/batch/receipt finality evidence when available. It does
  not include spend keys, viewing keys, note `rseed`, or Merkle auth paths.
  Focused evidence is the node Orchard integration test, which now covers both
  direct local disclosure without finality and ordered-batch disclosure with
  finality.
- `orchard-disclosure-verify` validates packet schema/hash, local
  chain/genesis context, archive commitment inclusion, and ordered-batch
  block/finality fields when present. Tampering the packet value without
  recomputing the disclosure hash fails closed in the focused integration test.
  The wallet finality smoke now also generates a change-note disclosure packet,
  verifies it against the live data dir, verifies the same packet after
  snapshot import, and records both verifier reports:
  `reports/testnet-orchard-wallet-finality-smoke/perf-malformed-v0-20260515T103617Z/testnet-orchard-wallet-finality-smoke.json`.
  The same report now records local performance metrics for the real Orchard
  output/spend/withdraw path: 7,264-byte proofs, roughly 19 KB action files,
  about 39-40s local action construction per shielded action on this host,
  about 11.5-12.0s ordered apply/verify per shielded batch, sub-20ms disclosure
  verification, a 1,048,577-byte oversized proof rejected fail-closed with
  `oversized_hex` in about 63ms, a 4,097-byte encrypted-output ciphertext
  rejected fail-closed with `oversized_hex` in about 3ms, a 7,264-byte
  malformed proof rejected with `proof_verification_failed` in about 11.7s,
  and a 580-byte malformed encrypted-output ciphertext rejected with
  `binding_signature_invalid` in about 11.4s.

- A P0 code review hardening slice has landed, was committed as `2cae621`
  (`Harden validator transport and ordered commits`), pushed to `origin/main`,
  and redeployed to the live controlled network. The code now requires
  ML-DSA-authenticated validator transport envelopes, requires signed proposals
  by default on validator vote services, disables uncertified transport
  service-apply, adds bounded concurrent RPC serving with a 64 KiB request-line
  read cap, and journals ordered commits for crash recovery before multi-file
  state writes. Full local readiness passed:
  `reports/testnet-readiness-gate/testnet-readiness-gate-20260514T211920Z.json`
  (`readiness_ok=true`, 4 validators, 3 rounds, final height 3). Final checks:
  `bash -n` on touched testnet scripts, `cargo fmt --check`,
  `cargo test --workspace --all-targets`, and `cargo clippy --workspace
  --all-targets -- -D warnings`; the non-gate certified-batch retry smoke also
  passed.
- Live-green evidence for `2cae621` now exists. Fresh operator-private package:
  `reports/testnet-live-operator-artifact-20260514T-p0-hardening-2cae621/packages/testnet-release-package-20260515T000031Z`.
  Prep check passed:
  `reports/testnet-live-operator-artifact-20260514T-p0-hardening-2cae621/launch-prep-check.json`.
  Exact remote join passed:
  `reports/testnet-live-operator-artifact-20260514T-p0-hardening-2cae621/remote-join-dry-run/testnet-release-remote-join-dry-run.json`.
  Live redeploy passed:
  `reports/testnet-live-launch-20260515T-p0-hardening-2cae621/testnet-release-live-launch.json`.
  Post-redeploy proof gates passed for SDK wallet finality, continuity,
  restart, single-validator partial outage, below-quorum no-advance/recovery,
  observability, RPC read-load, and RPC oversized-request edge rejection:
  `reports/testnet-live-wallet-finality/p0-hardening-2cae621-20260515T0000/testnet-live-wallet-finality.json`,
  `reports/testnet-live-continuity-soak/p0-hardening-2cae621-5round-20260515T0000/testnet-live-continuity-soak.json`,
  `reports/testnet-live-restart-drill/p0-hardening-2cae621-20260515T0000/testnet-remote-restart-drill.json`,
  `reports/testnet-live-partial-outage-drill/p0-hardening-2cae621-20260515T0000/testnet-remote-partial-outage-drill-20260515T001140Z.json`,
  `reports/testnet-live-below-quorum-outage-drill/p0-hardening-2cae621-amount10-20260515T0000/testnet-live-below-quorum-outage-drill-20260515T001720Z.json`,
  `reports/testnet-remote-observability/p0-hardening-2cae621-20260515T0000/testnet-remote-observability.json`,
  `reports/testnet-remote-rpc-read-load/p0-hardening-2cae621-300-20260515T0000/testnet-remote-rpc-read-load.json`,
  and
  `reports/testnet-remote-rpc-edge-load/p0-hardening-2cae621-20260515T0000-rerun/testnet-remote-rpc-edge-load.json`.
- The active controlled-testnet candidate is the transparent PQ settlement path.
  The exact candidate revision is
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5` (`Record optimized finality
  launch status`).
- The latest exact-head remote P0 control point is
  `c18d590` (`Add SDK TCP wallet flow example`), which passed with 5 validators,
  the placement manifest, normal-run transparent ordering, restart,
  partial-outage, RPC catch-up, validator-registry mutation, emergency
  key-rotation rehearsal, topology capture, history retention, history role
  policy, remote RPC edge load, and SDK signer mode in the wallet finality path:
  `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`.
- The latest clean-head controlled-launch evidence pack was generated at
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`:
  `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`.
  It passed with `git.dirty=false`, binds the selected remote P0 report, checks
  SDK signer mode, and verifies that the benchmark, Cobalt lifecycle, and
  registry-root binding subpacks consumed that same P0 report.
- The 8-hour remote soak `remote-soak-current-bucket-8h-v2` succeeded for
  29,237 seconds, 91 iterations, final height 92, and zero observed height lag
  on the runtime candidate `0410884`.
- The current exact-head final candidate gate passed on `56db87a`, reusing that
  completed soak checkpoint and requiring a fresh exact remote-join rehearsal
  against the generated operator artifact:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/testnet-release-final-candidate-20260514T-current-head-56db87a-optimized-latency.json`.
- The release package excluded private material:
  `reports/testnet-release-packages/testnet-release-package-20260514T145919Z`.
- Exact artifact remote-join rehearsal for the current package passed with 5
  validators across 2 machines:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/candidate/release-gate/release-gate/logs/exact-remote-join-dry-run.json`.
- Live launch prep now has an explicit fail-closed checker:
  `scripts/testnet-controlled-launch-prep-check`. The current package prep
  report,
  `reports/testnet-controlled-launch-prep-check/current-package-prep-check-20260514.json`,
  shows the release package verifies but is not directly live-installable
  without an operator-private handoff because the package topology still uses
  placeholder hosts and the matching local private material was deliberately
  removed after rehearsal.
- A credential-bound operator-private launch artifact was generated, retained
  its per-validator private material locally, and passed fail-closed launch
  prep:
  `reports/testnet-live-operator-artifact-20260514T-prep/launch-prep-check.json`.
  The exact artifact also passed remote fake-root join rehearsal:
  `reports/testnet-live-operator-artifact-20260514T-prep/remote-join-dry-run/testnet-release-remote-join-dry-run.json`.
- Live controlled launch passed on the operator surface:
  `reports/testnet-live-launch-20260514T151903Z-head-9e4fb20-rerun5/testnet-release-live-launch.json`.
  The report records five validator/RPC services active across three machines,
  initial convergence at height 0, a certified transparent round, and
  post-round convergence at height 1. The live launch package revision is
  `9e4fb20fb1dd1cde0b2b00d595351a0075120b7f`; launcher fixes were committed
  through `fe00519`.
- Live SDK wallet finality passed:
  `reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json`.
  It funded a fresh SDK wallet at height 5, quoted through live read RPC,
  signed locally with the SDK, submitted through a temporary SSH-local bounded
  write edge, ordered the mempool transaction at height 6, verified `tx`
  finality through live read RPC, and converged all five validators at height
  6.
- Live post-launch continuity soak has passed multiple windows:
  `reports/testnet-live-continuity-soak/current-rerun-20260514T162207Z/testnet-live-continuity-soak.json`.
  The first positive soak ran five proposer-routed certified transparent rounds
  from height 11 to height 16, with one accepted transfer per block and
  convergence after every round. Later continuity evidence includes 20 rounds
  to height 38 and 100 rounds to height 141. This is continuity/hardening
  evidence, not a throughput claim.
- Live restart and public RPC edge-load hardening have started:
  `reports/testnet-live-restart-drill/current-20260514T162808Z/testnet-remote-restart-drill-20260514T162808Z.json`
  restarted all five validator/RPC service pairs and verified service/state/RPC
  convergence at height 16. Post-restart ordering then passed two certified
  transfer rounds to height 18:
  `reports/testnet-live-continuity-soak/post-restart-20260514T162853Z/testnet-live-continuity-soak.json`.
  Live RPC edge-load evidence also passed:
  `reports/testnet-live-rpc-edge-load/current-20260514T163012Z/testnet-remote-rpc-edge-load.json`.
  Each validator rejected three oversized RPC requests and answered a valid
  status request afterward, converged at height 18.
- Live RPC read-load evidence passed:
  `reports/testnet-remote-rpc-read-load/current-20260514T170449Z/testnet-remote-rpc-read-load.json`.
  The script sent 300 SDK-built and SDK-validated read requests across the five
  live RPC endpoints for `status`, `server_info`, `ledger`, `fee`,
  `validators`, and `manifests`; all responses validated, overall latency was
  p50 `0.344s`, p95 `0.672s`, p99 `0.772s`, and the network remained
  converged at height 41 after the run.
- Broad live RPC read-load evidence passed:
  `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json`.
  The script sent 1,200 SDK-built and SDK-validated read requests across the five
  live RPC endpoints for 12 read methods, including ledger/block list-shaped
  responses; all responses validated, overall latency was p50 `0.359s`, p95
  `0.701s`, p99 `0.797s`, and the network remained converged at height 164
  after the run.
- Controlled write-edge policy/audit evidence passed:
  `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`.
  The audit verifies that the current release package's five validator RPC
  systemd units remain read-only by default, the live SDK wallet finality write
  path was bounded and SSH-local, the local write-edge pressure smoke passed,
  and `docs/runbooks/controlled-write-edge-policy.md` defines the allowed
  controlled write service shape. A persistent externally exposed write edge is
  still not installed.
- Longer live continuity soak passed:
  `reports/testnet-live-continuity-soak/longer-20round-20260514T163232Z/testnet-live-continuity-soak.json`.
  It ran 20 proposer-routed certified transparent rounds from height 18 to
  height 38, one accepted transfer per block, convergence after every round,
  and round totals between `0.283s` and `0.341s` in the peer-certified round
  report timings. This is still continuity evidence, not a TPS benchmark.
- Extended live continuity soak passed:
  `reports/testnet-live-continuity-soak/longer-100round-20260514T171905Z/testnet-live-continuity-soak.json`.
  It ran 100 proposer-routed certified transparent rounds, one accepted
  transfer per block, convergence after every round, and final five-validator
  convergence at height 141. This is continuity evidence, not a TPS benchmark.
- Live single-validator partial-outage drill passed:
  `reports/testnet-live-partial-outage-drill/current-20260514T164149Z/testnet-remote-partial-outage-drill-20260514T164149Z.json`.
  Validator 3 was stopped while validator 4 proposed height 39; the network
  formed a 4-of-5 quorum certificate, recorded one failed vote request and one
  failed certified-send target for the offline validator, kept the four online
  validators converged, restarted the offline validator, replayed the missed
  certified batch on the first attempt, and reconverged all five validators at
  height 39. Post-outage continuity then passed two more certified rounds to
  height 41:
  `reports/testnet-live-continuity-soak/post-partial-outage-20260514T164318Z/testnet-live-continuity-soak.json`.
- Live below-quorum outage drill passed:
  `reports/testnet-live-below-quorum-outage-drill/live-below-quorum-rerun-20260514T181916Z/testnet-live-below-quorum-outage-drill-20260514T181916Z.json`.
  Validators 4 and 3 were stopped while validator 2 attempted height 142; with
  only three validators online against a four-vote quorum, the round failed
  with `insufficient block votes: got 3, need 4`, no validator advanced beyond
  height 141, restart also caused no state advance, and the same height then
  recovered with a five-vote certificate and full convergence. Post-recovery
  continuity then passed two more certified rounds to height 144:
  `reports/testnet-live-continuity-soak/post-below-quorum-continuity-20260514T182141Z/testnet-live-continuity-soak.json`.
- Live mixed read/write load passed:
  `reports/testnet-live-mixed-read-write-load/live-mixed-read-write-10x-20260514T185000Z/testnet-live-mixed-read-write-load.json`.
  It ran 600 SDK-built and SDK-validated reads across 12 methods while ten
  certified transparent rounds advanced from height 154 to height 164,
  recorded workload overlap, and verified post-mixed convergence at height
  164.
- Fresh live observability passed:
  `reports/testnet-remote-observability/live-observability-post-mixed-10x-20260514T185456Z/testnet-remote-observability.json`.
  It verified all five validator/RPC service pairs, read-RPC health,
  convergence at height 164 with zero height lag, and current data/log/event
  counters after the mixed read/write run.
- Live host-group outage drill passed:
  `reports/testnet-live-host-group-outage-drill/live-host-group-outage-20260514T191629Z/testnet-live-host-group-outage-drill.json`.
  The drill profiled the live topology, selected a two-validator operator-host
  group that can block quorum but cannot form quorum, stopped that group,
  proved no state advance while below quorum, restarted it, and recovered with
  full convergence at height 165. This is capture-threshold evidence for the
  controlled topology, not a broad decentralization claim.
- Live hardening evidence pack passed:
  `reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json`.
  The pack validates 17 live evidence entries, requires a 100-round continuity
  window, includes the below-quorum outage, host-group outage, mixed read/write
  load, and fresh observability checks, and summarizes max final height 165.
- Allowed-peer-failure quorum-early vote collection is now implemented and
  evidenced locally:
  `reports/testnet-transport-peer-certified-quorum-early/current-20260514T165536Z/testnet-transport-peer-certified-quorum-early.json`.
  In a 5-validator TCP peer-certified smoke, one slow peer held the vote
  connection open, the proposer certified after the 4-of-5 quorum without
  waiting for the `4s` peer timeout, recorded the slow peer as unresolved and
  skipped for certified send, and kept the online quorum converged.
- The older exact final watcher for `0410884` remains valid long-soak/watch
  evidence for the runtime behavior, while `56db87a` is now the current
  release-gate/package control point.
- Operator launch packet generation is now explicit:
  `scripts/testnet-operator-launch-packet` validates the final candidate,
  release package, release gate, exact remote join, soak checkpoint, P0 gate,
  host preflight, and private-material exclusion before writing a redacted
  operator handoff packet.
- `scripts/testnet-release-status` supports explicit final-candidate mode. The
  latest generated status report is `ready` with
  `ready_for_controlled_launch=true`:
  `reports/testnet-release-status-current-56db87a-optimized-latency/testnet-release-status-current-56db87a-optimized-latency.json`.
- Prior P0 launch blocker: local 5-validator submit-to-finality latency was too
  high and grew with height. Prior benchmark:
  `reports/testnet-tx-finality-latency-benchmark/current-20260514T124520Z/testnet-tx-finality-latency-benchmark.json`.
  It reports `submit_to_finality` p50 `10.687s`, p95 `18.039s`, p99
  `18.698s`; `certified_round` p50 `8.988s`, p95 `15.407s`; and `tx`
  finality RPC p50 `1.138s`, p95 `2.023s`.
- Current optimized local latency evidence:
  `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`.
  It reports local 5-validator `submit_to_finality` p50 `1.563s`, p95
  `1.709s`, p99 `1.753s`; `certified_round` p50 `0.921s`, p95 `1.043s`; and
  `tx` finality RPC p50 `0.062s`, p95 `0.068s` across 20 rounds.
- Current optimized remote latency evidence:
  `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`.
  It reports 5-validator remote peer-certified round total p50 `1.032s`, p95
  `1.116s`, p99 `1.139s` across 20 proposer-routed normal-run rounds.
- `scripts/testnet-benchmark-evidence-pack` now generates a v0 benchmark
  evidence packet by verifying and aggregating the final candidate, release
  status, P0 gate, 8-hour soak checkpoint, local ML-DSA/certificate-size
  benchmark suite, wallet tx-finality evidence, RPC write-edge pressure, and
  observability/disk evidence. It deliberately does not claim TPS; WAN public
  endpoint load, target hardware memory profile, end-to-end bandwidth, and
  privacy proof costs remain outside the v0 packet.
- `scripts/testnet-cobalt-lifecycle-audit` now generates a v0 canonical Cobalt
  lifecycle audit by verifying current P0 evidence for admit, remove, suspend,
  reactivate, planned rotate-key, emergency rotate-key, live admission, live
  suspension, stale/tamper rejection, local post-change fault drills, and remote
  post-change registry drills.
- `scripts/testnet-registry-root-binding-audit` now generates a v0 audit that
  verifies current P0 certificate artifacts and wallet tx-finality evidence bind
  compact votes to a validator registry root without repeating validator public
  keys inside each vote.
- `scripts/testnet-cobalt-amendment-lifecycle-smoke` now exercises the current
  v0 Cobalt amendment path for validator-set, crypto-policy, and
  bridge-witness-epoch amendments through governance batches. It proves
  activation-height metadata, veto-window rejection, paused-amendment rejection,
  separate activation-record artifacts, automatic same-kind supersession
  records, rollback records, tampered-vote rejection, and insufficient-support
  rejection. `scripts/testnet-cobalt-amendment-replay-bundle` now packages the
  ordered amendment/activation/supersession/rollback record set and verifies it
  through `postfiat-node governance-amendment-replay-verify`, including
  node-side tamper rejection. The controlled-launch evidence pack requires the
  node-verifier pass and node-tamper-rejection checks.
- `scripts/testnet-controlled-launch-evidence-pack` now generates a v0
  controlled-launch evidence manifest. It verifies or runs the benchmark pack,
  Cobalt lifecycle audit, registry-root binding audit, and amendment lifecycle
  smoke, then binds them to the selected remote P0 gate, final candidate,
  operator launch packet, release status, public claims checklist, whitepaper,
  and canonical status docs. It fails closed unless the selected remote P0
  report uses SDK signer mode and the benchmark/Cobalt/registry subpacks all
  point at that same P0 report. This is the handoff artifact to use before
  external reviewer or operator review.
- `docs/review/controlled-testnet-review-packet.md` is the v0 external reviewer
  command sheet. It records the exact evidence-pack head, reproduction commands,
  evidence map, review questions, and launch-claim exclusions.
- `docs/status/public-claims-checklist.md` is the current source of truth for
  allowed launch claims and overclaim boundaries.
- `docs/status/privacy-production-burndown.md` is the current source of truth
  for moving privacy from debug semantics to semi-production and production
  confidential settlement.
- The working execution checklist is
  `docs/status/controlled-testnet-burndown.md`.
- The bounded overnight hardening/tooling checklist is
  `docs/status/overnight-launch-hardening-burndown.md`; it prioritizes fresh
  evidence, RPC doctor tooling, validator UX, monitor snapshots, and a Python
  RPC client v0 before any additional cleanup.

## CONTROLLED-TESTNET COMPLETION SNAPSHOT

Current CTO estimate: **controlled launch execution has passed for the
transparent PQ settlement path**. The remaining work before broader external
exposure is post-launch hardening: sustained live soak, restart/load drills on
the live deployment, persistent write-edge installation/exposure evidence,
public endpoint load evidence, independent operator onboarding packets, and
continued storage/runtime hardening.

This percentage is not a mainnet or public-testnet readiness claim. It measures
the narrow controlled-testnet boundary: transparent PQ settlement, known
validators, public RPC/wallet minimum, canonical governance evidence, release
gate evidence, and an honest exclusion of production privacy/bridge custody.
Correctness, governance, wallet, RPC, and packaging evidence remain materially
advanced. The local transparent settlement path and remote peer-certified path
are now inside the controlled launch latency targets, and the optimized release
evidence pack is cut.

| Pillar | Current % | 100% Controlled-Testnet Meaning | Main To-Dos |
| --- | ---: | --- | --- |
| Transparent PQ chain | 97-99% | Transparent transfers, ML-DSA auth, fees/reserves/burn, mempool admission, receipts, tx-finality artifacts, canonical transaction-envelope fixture, replayable block/certificate evidence, release-bound metrics, optimized local 5-validator submit-to-finality, optimized remote peer-certified round latency, allowed-peer-failure quorum-early vote collection, live launch, live SDK wallet finality, and 100-round live continuity soak are implemented/evidenced. | Keep persisted tx indexes / append-only storage, broader partition/load drills, and external-WAN endpoint evidence as launch-hardening follow-up. |
| 5-validator controlled network | Live controlled launch passed; post-launch hardening started | Five validators are installed and running from the operator-private package; validator/RPC services are active; convergence passed; a certified transparent round passed; live wallet finality passed through a temporary controlled write edge; 100-round continuity soak passed to height 141; live all-validator restart drill passed; post-restart ordering passed; single-validator partial-outage quorum and recovery passed; below-quorum outage failed without state advance and recovered at height 142; post-recovery continuity passed to height 144; mixed read/write load passed while advancing from height 154 to height 164; fresh live observability passed at height 164 with zero height lag; host-group outage proved one two-validator operator-host group can block quorum without advancing state and recovered at height 165; RPC oversized edge-load passed; write-edge policy audit passed; live hardening evidence pack passed; reports are redaction-scanned. | Run broader partition/load evidence, external-WAN endpoint load, install/evidence a persistent write edge only when external write access is needed, and package independent-operator onboarding/replay artifacts. |
| RPC and wallet minimum | 96-98% | User can derive/restore wallet through node CLI or SDK binary/library, quote fee through live read RPC, sign offline through node CLI or SDK binary/library, submit through a controlled write edge, query account/receipt/tx finality, query XRP-like `server_info`/`ledger`/`fee`/`validators`/`manifests` read aliases, validate finality/read responses through SDK validation, and consume typed SDK summaries for account, receipts, quote, submit, and finality with registry-root-backed compact certificate checks. Live evidence now proves SDK quote/sign/submit/`tx` finality, SDK-validated broad read-load across 12 read methods, and the controlled write-edge policy audit proves validator RPC units remain read-only by default. | Install/evidence a persistent external write edge if public write access is needed, gather external-WAN/mixed-workload endpoint load evidence, and add an HTTP/gateway wrapper example if public RPC is exposed behind HTTP infrastructure; keep account-history indexing, account key-rotation transaction, and hardware/external signer work scoped as post-controlled-testnet production work. |
| Cobalt canonical governance | 93-95% | Validator lifecycle is canonical-mode Cobalt governance: manifests, genesis validator bundle, registry-root-bound certificates, admit/suspend/reactivate/remove/rotate evidence, governance replay bundle, registry-root-bound finality/certificate audit, amendment lifecycle metadata with activation-height/veto/pause rejection, separate activation-record artifacts, automatic supersession records, rollback records, ordered amendment replay packaging, ordered validator-registry lifecycle replay packaging, node-owned amendment and registry lifecycle replay verification, governance replay package amendment-bundle binding, node-side tamper rejection, controlled-launch evidence gating, and readiness gate requirements. | Package manifests into operator onboarding/release artifacts and publish independent-operator replay packets. |
| Quantum posture | 68-72% | Transparent auth is PQ by construction, with stable domain separation, public vectors, local ML-DSA performance/certificate-size evidence, dependency inventory, and crypto-audit package. | Freeze domain labels; gather KATs and dependency versions; document key lifecycle and validator/account rotation posture; extend the benchmark evidence pack to release-grade CPU/memory/WAN/RPC-payload runs; leave ML-KEM for privacy track. |
| Validator storage/history | 84-88% | Validators run partial-history mode with explicit retention windows, checkpointed prune execution, pending-prune recovery, prune journal, archive-node handoff proof, deterministic archive-window export/verify/import, bounded read-only RPC/SDK archive-window validation, source-driven archive-window backfill from a full-history RPC source, post-prune backfill/append verification, RPC-visible history posture, explicit archive/indexer role policy, and readiness/P0/release gate requirements. | Fold the fresh `c18d590` P0 history evidence into the next release package; broaden archive role from controlled operator to independent archive/indexer onboarding. |
| Production privacy | 65-75% | Confidential Settlement v1 has Orchard/Halo2 verified shielded actions in the local ordered and local 4-validator peer-certified apply paths, production pool state with root history and duplicate-nullifier replay safety, wallet scan/decrypt/spend/withdraw/disclose flow, transparent/shielded turnstile accounting, fee burn, SDK request-file envelopes for shielded action and withdraw batches, opt-in remote Orchard batch creation from bounded `action_json` through child-timeout RPC serving, per-peer/global rate-limit evidence, concurrent malformed RPC edge-load evidence with parent+child RSS sampling, external envelope binding in the Orchard authorization domain, local redacted disclosure packets with ordered-batch finality evidence, local disclosure verification, redacted evidence reports, and an operator-policy report that publishes protocol caps plus current verifier posture. | Add deposit/outer-envelope fee policy, SDK-library packaging beyond request files, regulated disclosure policy, target-hardware/repeated privacy benchmarks, live privacy-alpha evidence, and audit packet. |

What gets us to 100% for the first controlled testnet launch:

1. Execute the recut operator runbook in live controlled launch.
2. First pass `scripts/testnet-controlled-launch-prep-check` with the exact
   package, credential-bound topology, and per-validator private material that
   will be installed.
3. Capture post-launch convergence, RPC, certified-round, SDK wallet finality,
   service-log, and redaction evidence.
4. Keep release evidence exact to the candidate revision after behavior-changing
   commits; docs-only commits are post-cut bookkeeping unless explicitly
   re-cut.
5. Make canonical Cobalt governance artifacts explicit enough for external
   replay: manifests, genesis bundle, lifecycle/runbook, replay gate.
6. Complete validator partial-history retention so validators are not mandatory
   archive nodes.
7. Keep the benchmark/evidence packet current and extend it from v0 evidence
   aggregation to release-grade WAN endpoint load, CPU/memory, end-to-end
   bandwidth, and repeated finality measurements.
8. Generate the controlled-launch evidence pack before each external reviewer
   or operator handoff so final-candidate, governance, benchmark, and claims
   evidence stay in one auditable manifest.
9. Keep privacy and bridge claims scoped: privacy semantics exist, production
   privacy is the parallel Confidential Settlement v1 workstream, and bridge
   custody is out of controlled-testnet scope.
10. Use the public claims checklist before changing public-facing language.

## STATE OF THE CHAIN

PostFiat L1 v2 is currently a Rust MVP for an XRP-like, post-quantum-oriented,
federated validator chain with Cobalt-style governance evidence and
Orchard-inspired shielded semantics. The transparent settlement path is the
most mature part of the system. Cobalt/governance is materially exercised but
not yet packaged as the complete canonical controlled-testnet governance
process. Privacy has real note/nullifier/turnstile semantics and RPC paths, but
the debug privacy path remains controlled-testnet-only behind explicit
debug-proof gating, and production privacy still lacks zkVM/STARK proofs and
ML-KEM note encryption.

Approximate current maturity:

| Pillar | Current State | Maturity |
| --- | --- | --- |
| PQ XRP-like transparent settlement | Working local chain, ML-DSA-65 signing, canonical envelope fixture, fees/reserves/burn evidence, mempool, receipts, RPC quote/sign/submit, wallet vectors, public `tx` finality RPC implementation, and wallet/P0 finality evidence | 88-92% |
| Cobalt-style validator governance | Governance and validator-registry evidence exists; rotation/suspension/stale-key/post-change drills are implemented and exercised; canonical-mode claim boundary, signed operator manifests, genesis governance bundle, genesis-linked replay package v0, readiness-gated genesis smoke evidence, amendment activation/supersession/rollback records, ordered amendment replay bundle, ordered validator-registry lifecycle replay bundle, node-owned replay verifiers, and controlled-testnet execution plan are implemented/documented | 86-90% |
| Production private settlement | Note/nullifier/turnstile semantics exist; proof/encryption backend is not production | 15-25% |
| Controlled remote testnet | 5-validator 8-hour soak passed for runtime candidate `04108846e91e5a4a126480ab70bde9c46520f650`; exact final candidate gate, release package, operator launch packet, release status, and controlled-launch evidence pack passed on current head `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`; live operator-private launch passed on package revision `9e4fb20`; live SDK wallet finality passed at height 6 | Controlled launch executed; post-launch soak/restart/load and public endpoint hardening remain |

The immediate strategic target is now:

1. Harden the running controlled network with longer-duration soak, broader
   partition/load drills, public endpoint policy, and independent-operator
   onboarding packets.
2. Package Cobalt-style governance as the canonical validator lifecycle and
   amendment path for controlled testnet.
3. Ship regulated confidential settlement v1 by replacing the debug privacy
   proof/encryption backend behind the existing semantics.

## What Is Implemented

### Workspace And Chain Shape

- Rust workspace with separate crates for crypto, types, execution, Cobalt,
  ordering, privacy, proofs, node, storage, RPC SDK, network, bridge, bench,
  and fuzz.
  Code: `Cargo.toml`, `crates/*/Cargo.toml`.
- Core persistent state includes ledger, mempool, blocks, receipts, governance,
  validator registry, bridge, and shielded state.
  Code: `crates/storage/src/lib.rs:49`, `crates/storage/src/lib.rs:92`,
  `crates/storage/src/lib.rs:96`.
- Node commands expose init, transfer, mempool, batch, RPC, wallet, governance,
  shielded, bridge, and validator operations.
  Code: `crates/node/src/main.rs:641`, `crates/node/src/main.rs:841`,
  `crates/node/src/main.rs:869`, `crates/node/src/main.rs:1412`,
  `crates/node/src/main.rs:1956`, `crates/node/src/main.rs:2002`.

### Post-Quantum Signing

- The crypto provider uses ML-DSA-65 from the `fips204` crate.
  Code: `crates/crypto_provider/src/lib.rs:1`,
  `crates/crypto_provider/src/lib.rs:6`,
  `crates/crypto_provider/src/lib.rs:7`,
  `crates/crypto_provider/src/lib.rs:8`.
- Key generation supports random keygen and deterministic seed keygen.
  Code: `crates/crypto_provider/src/lib.rs:40`,
  `crates/crypto_provider/src/lib.rs:49`.
- Domain-separated hashing exists in the crypto provider.
  Code: `crates/crypto_provider/src/lib.rs:123`.
- Transparent transfers carry algorithm id, public key, and signature.
  Code: `crates/types/src/lib.rs:852`, `crates/types/src/lib.rs:904`.
- The benchmark harness publishes ML-DSA-65 byte constants and local timings:
  public key 1,952 bytes, private key 4,032 bytes, signature 3,309 bytes,
  separate sign/verify elapsed time, and certificate-size rows for controlled
  testnet and larger validator counts.
  Code: `crates/bench_harness/src/main.rs:87`,
  `crates/bench_harness/src/main.rs:137`,
  `scripts/testnet-ml-dsa-performance-smoke`.
  Evidence:
  `reports/testnet-ml-dsa-performance/ml-dsa-verify-20260513T193753Z/testnet-ml-dsa-performance-20260513T193753Z.json`.

Current gap: ML-DSA is integrated and locally benchmarked, but the production
crypto posture still needs external audit prep, stable KAT packaging,
key-rotation UX, wallet SDKs, custodian flows, and release-grade hardware/WAN
performance reports.

### Transparent XRP-Like Settlement

Implemented capabilities:

- Account transfers with signed transaction envelopes.
  Code: `crates/types/src/lib.rs:852`, `crates/types/src/lib.rs:904`,
  `crates/node/src/lib.rs:1545`, `crates/node/src/lib.rs:1554`.
- Mempool admission for locally signed and externally signed transfers.
  Code: `crates/node/src/lib.rs:1582`, `crates/node/src/lib.rs:1607`,
  `crates/node/src/lib.rs:1614`, `crates/node/src/lib.rs:1628`.
- Mempool admission dry-runs pending transfers before accepting new signed
  transfers.
  Code: `crates/node/src/lib.rs:1667`, `crates/node/src/lib.rs:1678`.
- Canonical transparent transfer envelope v1 is documented and pinned to a
  deterministic wallet fixture covering exact signing bytes, signing hash,
  transaction id, network/genesis/protocol binding, sequence, fee, operation
  fields, and replay behavior.
  Spec: `docs/specs/transparent-transaction-envelope.md`.
  Code: `crates/types/src/lib.rs:852`, `crates/execution/src/lib.rs:321`,
  `crates/node/src/lib.rs:12308`.
- Batch apply writes receipts.
  Code: `crates/node/src/lib.rs:2234`, `crates/types/src/lib.rs:987`.
- Fee/reserve/burn policy is exposed in receipts, metrics, quote reports, and
  readiness/P0 aggregation. Current evidence proves minimum fee, charged fee,
  burned fee, state-expansion fee, reserve checks, burn total, and no funding
  of the historical fee collector address.
  Evidence:
  `reports/testnet-fee-reserve-policy-refresh/fee-reserve-20260513T193228Z/testnet-fee-reserve-policy-smoke.json`,
  `reports/testnet-transfer-fee-quote-refresh/transfer-fee-quote-20260513T193236Z/testnet-transfer-fee-quote-smoke.json`.
- Fee quote RPC and CLI path exists.
  Code: `crates/node/src/lib.rs:1428`, `crates/node/src/main.rs:1412`,
  `crates/rpc_sdk/src/lib.rs:177`.
- SDK transport example exists for the current newline-delimited TCP public RPC
  surface.
  Code: `crates/rpc_sdk/examples/tcp_wallet_flow.rs:1`.
- Signed transfer submission over public RPC exists behind explicit opt-in.
  Code: `crates/node/src/main.rs:665`, `crates/node/src/main.rs:690`,
  `crates/node/src/main.rs:6409`, `crates/rpc_sdk/src/lib.rs:251`,
  `crates/rpc_sdk/src/lib.rs:259`.
- RPC write edge includes invalid-signature counters and per-peer/global
  submission limits.
  Code: `crates/node/src/main.rs:5268`, `crates/node/src/main.rs:5271`,
  `crates/node/src/main.rs:5274`, `crates/node/src/main.rs:5474`,
  `crates/node/src/main.rs:5484`, `crates/node/src/main.rs:5493`,
  `crates/node/src/main.rs:6074`.

Evidence:

- Latest targeted wallet quote/sign/submit smoke with SDK signer mode and `tx`
  finality evidence:
  `reports/testnet-wallet-sign-transfer-smoke/sdk-signer-rpc-flow/testnet-wallet-sign-transfer-smoke.json`.
- Latest local P0 gate requiring SDK signer mode:
  `reports/testnet-p0-network-gate-local-sdk-signer-readiness/testnet-p0-network-gate-20260514T014214Z.json`.
- Compile check for packaged SDK TCP example:
  `cargo check -p postfiat-rpc-sdk --examples`.
- Prior node-wallet targeted wallet quote/sign/submit smoke with `tx` finality
  evidence:
  `reports/testnet-wallet-sign-transfer-smoke/rpc-quote-submit/testnet-wallet-sign-transfer-smoke.json`.
- Prior P0 aggregation for wallet signed-submit path:
  `reports/testnet-p0-network-gate-local-wallet-rpc-submit/testnet-p0-network-gate-20260512T221256Z.json`.
- RPC write-edge load smoke script:
  `scripts/testnet-rpc-write-edge-load-smoke:128`,
  `scripts/testnet-rpc-write-edge-load-smoke:202`,
  `scripts/testnet-rpc-write-edge-load-smoke:282`,
  `scripts/testnet-rpc-write-edge-load-smoke:369`.
- Controlled write-edge policy audit:
  `scripts/testnet-controlled-write-edge-policy-audit:1`,
  `docs/runbooks/controlled-write-edge-policy.md:1`,
  `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`.

Current gaps to 100% PQ XRP-like chain:

- The active 5-validator remote controlled-testnet candidate has completed its
  8-hour runtime soak and an exact-head final release gate for revision
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`.
- Default long-running node mode needs sustained soak under restarts, failed
  leaders, partitions, lag, and load.
- Public RPC needs external-WAN load evidence and continued rate-limit/load
  evidence on the public read edge. Persistent external write-edge installation
  remains open; the current policy/audit evidence proves the safe service
  boundary and read-only validator RPC default.
- Validators have the initial explicit partial-history mode: current state,
  recent block/certificate/receipt windows, checkpointed destructive prune,
  pending-prune recovery, prune journal, archive-handoff proof, deterministic
  archive-window export/verify/import, bounded read-only RPC/SDK
  `archive_window` validation, and source-driven archive-window backfill from a
  full-history RPC source. Readiness, P0, and release gates now require the
  smoke. `docs/status/controlled-testnet-history-roles.json` now defines the
  controlled-testnet split between partial-history validators and a full-history
  archive role, and `scripts/testnet-history-role-policy-smoke` verifies that
  policy against the retention evidence. Remaining work is fresh candidate
  evidence after the next release cut and independent archive/indexer
  onboarding. Runbook:
  `docs/runbooks/validator-history-retention.md`.
- `history-status`, `history-prune-plan`, archive-handoff create/verify, and
  archive-window export/verify/import/backfill, and destructive
  `history-prune`/`history-prune-recover` now expose the partial-history
  operator surface. The plan/prune path fails closed when current height is
  inside retention or when archive handoff proof is missing, invalid, or does
  not cover the prune boundary. `history-prune` writes
  `history_prune_pending.json`, `history_checkpoint.json`, prunes covered
  block/archive/receipt rows, verifies the retained suffix from the checkpoint,
  appends `history_prune_journal.json`, and has regression tests/smoke coverage
  for archive bundle verification, post-prune archive RPC backfill, pending
  recovery, and appending block 2 after pruning block 1.
  Code: `crates/node/src/lib.rs:94`, `crates/node/src/lib.rs:1934`,
  `crates/node/src/lib.rs:2037`, `crates/node/src/lib.rs:2087`,
  `crates/node/src/lib.rs:2142`, `crates/node/src/lib.rs:2410`,
  `crates/node/src/lib.rs:2492`, `crates/node/src/lib.rs:17650`,
  `crates/node/src/main.rs:823`, `crates/node/src/main.rs:832`,
  `crates/node/src/main.rs:841`, `crates/node/src/main.rs:887`,
  `crates/node/src/main.rs:906`, `crates/node/src/main.rs:917`,
  `crates/node/src/main.rs:932`, `crates/node/src/main.rs:6151`,
  `crates/node/src/main.rs:6188`, `crates/node/src/main.rs:6922`,
  `crates/rpc_sdk/src/lib.rs:278`, `crates/rpc_sdk/src/lib.rs:1312`,
  `crates/rpc_sdk/src/lib.rs:2630`, `crates/rpc_sdk/src/main.rs:108`,
  `docs/status/controlled-testnet-history-roles.json:1`,
  `scripts/testnet-history-role-policy-smoke:1`,
  `scripts/testnet-readiness-gate:275`, `scripts/testnet-p0-network-gate:732`,
  `scripts/testnet-release-gate:703`.
  Evidence:
  `reports/testnet-history-retention-smoke/archive-backfill-sdk-20260513T203911Z/testnet-history-retention-smoke.json`,
  `reports/testnet-readiness-gate/history-retention-gate-20260513T205424Z/testnet-readiness-gate-20260513T205424Z.json`,
  `reports/testnet-p0-network-gate/history-retention-local-20260513T210044Z/testnet-p0-network-gate-20260513T210044Z.json`,
  `reports/testnet-readiness-gate/history-role-policy-gate-20260513T211102Z/testnet-readiness-gate-20260513T211102Z.json`,
  and
  `reports/testnet-p0-network-gate/history-role-policy-local-20260513T211641Z/testnet-p0-network-gate-20260513T211641Z.json`.
- Wallet now has typed RPC SDK summaries for account, receipts, fee quote,
  mempool submit, and tx finality, plus SDK-native deterministic backup
  creation, public identity restore, quote-bound ML-DSA transfer signing, and
  SDK binary commands for backup/identity/signing. Remaining gaps are packaged
  HTTP examples, account-history indexing,
  hardware/external signer support, and account key rotation.
- First local ML-DSA and certificate-size benchmark evidence exists; remaining
  benchmarks must cover release hardware finality latency, bandwidth, CPU,
  memory, disk, and RPC rejection behavior.

### Wallet

Implemented capabilities:

- Deterministic wallet key generation from a 32-byte seed.
  Code: `crates/node/src/lib.rs:6350`.
- Wallet restore from backup.
  Code: `crates/node/src/lib.rs:6388`.
- Offline/local wallet signing against explicit quote fields or a quote file.
- SDK-native wallet backup creation, public identity restore, and quoted
  transfer signing without invoking `postfiat-node`.
  Code: `crates/rpc_sdk/src/lib.rs`.
- SDK binary commands expose backup creation, identity restore, quoted signing,
  and submit-request construction; smoke evidence proves the transport-free
  wallet CLI path.
  Code: `crates/rpc_sdk/src/main.rs`, `scripts/testnet-sdk-wallet-cli-smoke`.
  Evidence: `reports/testnet-sdk-wallet-cli-smoke/testnet-sdk-wallet-cli-smoke.json`.
- Exchange/custody deposit model specifies unique deposit addresses,
  no-xpub/BIP32 constraints, watch-only limits, finality-backed attribution,
  withdrawal flow, and recovery boundaries.
  Spec: `docs/specs/wallet-exchange-custody-model.md`.
- Account key-rotation boundary documents that controlled testnet uses
  first-spend public-key binding plus backup/restore or sweep-if-compromised;
  no account key-rotation transaction exists yet.
  Spec: `docs/specs/account-key-rotation-boundary.md`.
  Code: `crates/node/src/lib.rs:6402`, `crates/node/src/main.rs:869`,
  `crates/node/src/main.rs:885`, `crates/node/src/main.rs:928`.
- Wallet signing self-test verifies submit-ready redacted transfer and applies
  it.
  Code: `crates/node/src/lib.rs:10070`, `crates/node/src/lib.rs:10124`.
- Public test vectors exist for deterministic recovery/signing.
  Evidence: `reports/testnet-wallet-test-vectors-smoke/manual/wallet-test-vectors.public.json`.

Evidence:

- `reports/testnet-wallet-minimum-smoke/manual/testnet-wallet-minimum-smoke.json`.
- `reports/testnet-wallet-test-vectors-smoke/manual/testnet-wallet-test-vectors-smoke.json`.
- `reports/testnet-wallet-test-vectors-smoke/state-expansion/testnet-wallet-test-vectors-smoke.json`.

Current gaps:

- No hardware-wallet story.
- No production account-history indexer for exchange deposit scanning.
- No account key-rotation transaction path; the non-claim boundary is
  documented for controlled testnet.
- No production address/account recovery standard beyond MVP fixtures.

### HotStuff-Family Ordering

Implemented capabilities:

- Deterministic proposer selection by height/view.
  Code: `crates/node/src/lib.rs:1328`, `crates/node/src/lib.rs:1342`,
  `crates/ordering_fast/src/lib.rs:981`.
- Block proposal files include view/proposer/signature material.
  Code: `crates/node/src/lib.rs:542`, `crates/node/src/lib.rs:558`,
  `crates/node/src/lib.rs:580`, `crates/node/src/lib.rs:1910`.
- Block votes and block certificates exist.
  Code: `crates/node/src/lib.rs:411`, `crates/node/src/lib.rs:4026`,
  `crates/node/src/lib.rs:4126`, `crates/node/src/lib.rs:4171`.
- Timeout votes and timeout certificates exist.
  Code: `crates/node/src/lib.rs:4300`, `crates/node/src/lib.rs:4405`,
  `crates/ordering_fast/src/lib.rs:1337`.
- Proposal/vote equivocation detection exists.
  Code: `crates/ordering_fast/src/lib.rs:1016`,
  `crates/ordering_fast/src/lib.rs:1024`,
  `crates/ordering_fast/src/lib.rs:1408`.
- Adversarial ordering simulation covers failed leaders, dropped votes,
  partitions, equivocation, and stale votes.
  Code: `crates/ordering_fast/src/lib.rs:1810`,
  `crates/ordering_fast/src/lib.rs:1814`,
  `crates/ordering_fast/src/lib.rs:1827`,
  `crates/ordering_fast/src/lib.rs:1832`.

Current gaps:

- HotStuff-family loop is not yet the fully proven default long-running
  production node path under sustained faults and load.
- Payload availability decision is still open: stay chained HotStuff for
  controlled testnet or introduce Narwhal/Bullshark-style availability before
  public testnet.
- Need longer remote soak and published fault/load reports.

### Cobalt-Style Governance And Validator Registry

Implemented capabilities:

- Governance amendments and validator registry records are typed state.
  Code: `crates/types/src/lib.rs:710`, `crates/types/src/lib.rs:758`,
  `crates/types/src/lib.rs:766`, `crates/types/src/lib.rs:791`,
  `crates/types/src/lib.rs:821`.
- Cobalt crate supports governance amendment ratification and verification.
  Code: `crates/consensus_cobalt/src/lib.rs:100`,
  `crates/consensus_cobalt/src/lib.rs:115`,
  `crates/consensus_cobalt/src/lib.rs:175`,
  `crates/consensus_cobalt/src/lib.rs:658`.
- Cobalt crate supports validator registry updates including admit, suspend,
  reactivate, and rotate-key operations.
  Code: `crates/consensus_cobalt/src/lib.rs:76`,
  `crates/consensus_cobalt/src/lib.rs:79`,
  `crates/consensus_cobalt/src/lib.rs:80`,
  `crates/consensus_cobalt/src/lib.rs:81`,
  `crates/consensus_cobalt/src/lib.rs:240`,
  `crates/consensus_cobalt/src/lib.rs:320`.
- Cobalt evidence uses domain-separated ids for amendment, registry proposal,
  vote, certificate, and update ids.
  Code: `crates/consensus_cobalt/src/lib.rs:541`,
  `crates/consensus_cobalt/src/lib.rs:560`,
  `crates/consensus_cobalt/src/lib.rs:580`,
  `crates/consensus_cobalt/src/lib.rs:595`,
  `crates/consensus_cobalt/src/lib.rs:612`,
  `crates/consensus_cobalt/src/lib.rs:635`.
- Node applies governance batches and verifies Cobalt evidence before accepting
  governance payloads.
  Code: `crates/node/src/lib.rs:3372`, `crates/node/src/lib.rs:3443`,
  `crates/node/src/lib.rs:10305`.
- Node applies validator-registry updates to live registry state.
  Code: `crates/node/src/lib.rs:3368`, `crates/node/src/lib.rs:6859`,
  `crates/node/src/lib.rs:8254`.
- Governance replay packages can now be built through first-class node code and
  CLI, and the builder self-verifies the package before publishing it. The v0
  package can bind the genesis governance bundle and signed operator manifests
  before replaying a registry update, optional amendment replay bundle, and
  post-change block certificate.
  Code: `crates/node/src/lib.rs:892`, `crates/node/src/lib.rs:3084`,
  `crates/node/src/main.rs:1381`.
- Ordered validator-registry lifecycle replay bundles can be verified through
  node code and CLI. The verifier walks the initial registry through ordered
  admit, remove, suspend, and reactivate updates, recomputes every registry
  root, checks domain/update ids, rejects duplicate or out-of-order updates, and
  is packaged by `scripts/testnet-validator-registry-lifecycle-replay-bundle`.
  Code: `crates/node/src/lib.rs:1261`, `crates/node/src/lib.rs:5150`,
  `crates/node/src/main.rs:1559`.
- Tests cover key rotation, admit, suspend, non-contiguous active sets,
  tampered vote/certificate rejection, and stale-key surfaces.
  Code: `crates/consensus_cobalt/src/lib.rs:1357`,
  `crates/consensus_cobalt/src/lib.rs:1399`,
  `crates/consensus_cobalt/src/lib.rs:1490`,
  `crates/node/src/lib.rs:11626`,
  `crates/node/src/lib.rs:11783`,
  `crates/node/src/lib.rs:11935`,
  `crates/node/src/lib.rs:12987`.

Evidence:

- Local validator-registry update smoke:
  `reports/testnet-validator-registry-update/stale-vote-20260512T155815Z/testnet-validator-registry-update-smoke.json`.
- Ordered validator-registry lifecycle replay bundle smoke-check:
  `reports/testnet-validator-registry-lifecycle-replay-bundle/smoke-check/testnet-validator-registry-lifecycle-replay-bundle.json`.
- Wallet sign/submit/finality smoke using SDK compact-certificate validation:
  `reports/testnet-wallet-sign-transfer-smoke/compact-certificate-check/testnet-wallet-sign-transfer-smoke.json`.
- Remote validator registry drill evidence:
  `reports/testnet-remote-validator-registry-drill/testnet-remote-validator-registry-drill-20260512T120221Z.json`.
- P0 reports include validator-registry/fault-tolerance evidence in
  `reports/testnet-p0-network-gate/testnet-p0-network-gate-20260512T111446Z.json`
  and related log artifacts.
- Governance genesis bundle smoke creates operator manifests, verifies them,
  builds and verifies the genesis governance bundle, proves tampered manifest
  rejection, removes generated private key material, redaction-scans public
  artifacts, and is wired into readiness, P0, release, and release-candidate
  gates.
  Code: `scripts/testnet-governance-genesis-bundle-smoke`,
  `scripts/testnet-readiness-gate`, `scripts/testnet-p0-network-gate`,
  `scripts/testnet-release-gate`, `scripts/testnet-release-candidate-gate`.
- Governance replay package smoke uses `governance-replay-build`, verifies the
  replay package offline, checks genesis bundle/operator manifest linkage,
  checks optional amendment replay bundle binding, checks post-change block
  certificate binding to the new registry root, and proves tampered
  expected-update rejection.
  Evidence:
  `reports/testnet-governance-replay-package-smoke/governance-replay-genesis-link-20260513T185251Z/testnet-governance-replay-package-smoke.json`.

Current gaps to 100% Cobalt governance:

- Operator manifest create/verify and genesis governance bundle/verify are now
  implemented.
  Code: `crates/node/src/lib.rs:999`, `crates/node/src/lib.rs:3037`,
  `crates/node/src/lib.rs:3044`, `crates/node/src/lib.rs:3095`,
  `crates/node/src/lib.rs:3161`, `crates/node/src/main.rs:1380`,
  `crates/node/src/main.rs:1431`, `crates/node/src/main.rs:1443`,
  `crates/node/src/main.rs:1472`.
- Fresh exact-HEAD remote P0 evidence exists for `c18d590`:
  `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`.
- Clean-head controlled-launch evidence exists for `56db87a`:
  `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`.
  It binds the selected remote P0 report and verifies benchmark/Cobalt/registry
  subpack P0 inputs.
- The current final candidate package is:
  `reports/testnet-release-packages/testnet-release-package-20260514T145919Z`.
- Remaining lifecycle work is now mostly publication and operator expansion:
  `scripts/testnet-cobalt-lifecycle-audit` aggregates admit, suspend,
  reactivate, rotate, remove, and emergency rotate evidence; amendment replay
  has a standalone bundle with node-owned verification, tamper rejection,
  launch-evidence gating, and optional governance replay package binding; and
  validator-registry lifecycle replay packages ordered multi-record
  admit/remove/suspend/reactivate evidence with node-owned verification and
  out-of-order tamper rejection.
- Need full manifest package publication for independent validator onboarding
  and release artifacts.
- Need Cobalt trust-view/essential-subset analysis beyond a single canonical
  validator set.
- Live host-group capture-threshold evidence now exists for the controlled
  topology. Broader remote partition/DDoS drills remain after live governance
  changes.
- Need external consensus review.
- Need public release replay package publication that lets a third party verify
  the candidate governance history without private machines.

### Privacy And Shielded Value

Implemented capabilities:

- Shielded notes are typed state with commitments, nullifiers, owners, assets,
  values, pools, and metadata.
  Code: `crates/types/src/lib.rs:359`, `crates/types/src/lib.rs:372`.
- Shielded action types exist for mint, spend, migrate, and action batches.
  Code: `crates/types/src/lib.rs:418`, `crates/types/src/lib.rs:426`,
  `crates/types/src/lib.rs:434`, `crates/types/src/lib.rs:442`,
  `crates/types/src/lib.rs:450`, `crates/types/src/lib.rs:460`.
- Debug shielded mint/spend/migrate semantics exist.
  Code: `crates/privacy/src/lib.rs:73`, `crates/privacy/src/lib.rs:83`,
  `crates/privacy/src/lib.rs:139`, `crates/privacy/src/lib.rs:225`.
- Shielded scan, disclose, turnstile summary, and note root exist.
  Code: `crates/privacy/src/lib.rs:275`, `crates/privacy/src/lib.rs:306`,
  `crates/privacy/src/lib.rs:390`.
- Proof adapter boundary exists as `ProofSystem`.
  Code: `crates/proofs/src/lib.rs:32`, `crates/proofs/src/lib.rs:90`,
  `crates/proofs/src/lib.rs:118`, `crates/proofs/src/lib.rs:128`.
- Current debug proof backend is controlled-testnet-only and fail-closed in
  release-mode gate tests unless the explicit debug override is set.
  Code: `crates/proofs/src/lib.rs:6`, `crates/proofs/src/lib.rs:131`,
  `crates/proofs/src/lib.rs:173`.
- Privacy crate currently calls the debug proof system internally.
  Code: `crates/privacy/src/lib.rs:112`, `crates/privacy/src/lib.rs:179`,
  `crates/privacy/src/lib.rs:508`.
- Orchard/Halo2 production-privacy adapter exists with a PostFiat-owned
  serialized action shape and real proof/signature verification. The
  authorizing sighash now binds PostFiat `action.fee`, so fee mutation after
  signing invalidates verification before any nonzero fee accounting is
  enabled.
  Code: `crates/privacy_orchard/src/types.rs:663`,
  `crates/privacy_orchard/src/verify.rs:132`,
  `crates/privacy_orchard/src/verify.rs:172`,
  `crates/privacy_orchard/src/verify.rs:439`.
- Node can verify a serialized Orchard action and, when explicitly applied,
  persist verified nullifiers, output commitments, encrypted outputs, accepted
  anchors, and retained Orchard roots in separate Orchard pool state. Nonzero
  Orchard value balances fail closed until transparent/shielded turnstile
  accounting exists.
  Code: `crates/types/src/lib.rs:406`, `crates/types/src/lib.rs:434`,
  `crates/node/src/lib.rs:6541`, `crates/node/src/lib.rs:6730`,
  `crates/node/src/lib.rs:7542`.
- Ordered shielded batches can carry `orchard_action_v1` payloads and execute
  them through `apply-shield-batch`.
  Code: `crates/types/src/lib.rs:492`, `crates/types/src/lib.rs:498`,
  `crates/node/src/lib.rs:6698`, `crates/node/src/lib.rs:11719`.
- `verify-shielded` / RPC `verify_shielded` reports Orchard pool id,
  nullifier count, output count, accepted-anchor count, retained-root count,
  and latest retained root.
  Code: `crates/node/src/lib.rs:1125`, `crates/node/src/lib.rs:7065`.
- Orchard root history is recomputed from persisted output commitments with the
  upstream Orchard note-commitment tree hash; state verification rejects
  duplicate roots, non-monotonic output counts, mismatched retained roots, and
  accepted anchors that are not retained.
  Code: `crates/privacy_orchard/src/verify.rs:580`,
  `crates/privacy_orchard/src/verify.rs:597`,
  `crates/node/src/lib.rs:7187`, `crates/node/src/lib.rs:8302`.
- Wallet-created Orchard output actions exist for the zero-value path: the
  adapter builds a real Orchard/Halo2 output bundle against a caller-supplied
  retained anchor and raw Orchard recipient address, and the node
  `orchard-output-create` command writes an action file anchored to the latest
  retained pool root using exactly one recipient source: raw address, wallet
  key file, or view-key file.
  Code: `crates/privacy_orchard/src/verify.rs:448`,
  `crates/node/src/lib.rs:6651`, `crates/node/src/lib.rs:7391`,
  `crates/node/src/lib.rs:7411`, `crates/node/src/main.rs:2390`.
- Deposit-side nonzero Orchard value is now accounted: `orchard-output-create
  --value N` can build nonzero output actions, apply accepts negative Orchard
  value balances only when prior `pool_migration` turnstile events into
  `orchard-v1` cover the deposit amount, and Orchard pool state tracks consumed
  deposit budget plus cumulative value balance. Migration nullifies the source
  debug note, so migrated budget cannot also be spent in the debug pool.
  Positive value-balance withdrawals still fail closed.
  Code: `crates/privacy/src/lib.rs:225`, `crates/privacy/src/lib.rs:273`,
  `crates/types/src/lib.rs:417`, `crates/node/src/lib.rs:7022`,
  `crates/node/src/lib.rs:7078`, `crates/node/src/lib.rs:7781`,
  `crates/node/src/lib.rs:8275`.
- Orchard wallet scan v0 exists: the adapter derives full viewing keys from
  Orchard spending keys, derives default raw Orchard addresses from either
  spending keys or full viewing keys, and trial-decrypts persisted Orchard
  encrypted outputs with a full viewing key. The node writes deterministic
  private `postfiat-orchard-wallet-v1` key files, exports receive-only private
  `postfiat-orchard-view-key-v1` scan files without spending-key material, and
  can scan through `orchard-scan --view-key-file`, `--key-file`, or
  `--spending-key-hex`. Scan reports now include the latest retained root,
  output count, note position, witness anchor, 32-node auth path, and note
  `rseed` for each decrypted note; these reports are private wallet artifacts.
  Code: `crates/privacy_orchard/src/verify.rs:597`,
  `crates/privacy_orchard/src/verify.rs:766`,
  `crates/privacy_orchard/src/verify.rs:774`,
  `crates/privacy_orchard/src/verify.rs:802`,
  `crates/node/src/lib.rs:6604`, `crates/node/src/lib.rs:6628`,
  `crates/node/src/lib.rs:6826`, `crates/node/src/lib.rs:7262`,
  `crates/node/src/main.rs:2484`,
  `crates/node/src/main.rs:2500`.
- Orchard wallet spend actions v0 exist: the adapter builds a real
  spend-plus-output Orchard/Halo2 bundle from a decrypted note, spend authority,
  and retained Merkle witness; the node `orchard-spend-create` rescans with a
  spending key or wallet key file, selects `--input-output-index`, writes a
  verified one-note private transfer action with signed fee burn, and optionally
  uses `--amount N` to create a recipient output plus spender-default change
  output. The apply path nullifies the spent note while adding replacement
  outputs worth `input_value - fee`. Orchard pool state tracks cumulative fee
  burn, accepted receipts report `fee_charged` / `fee_burned` /
  `minimum_fee`, and underpriced positive value-balance fee burns fail closed.
  This is a local-wallet v0 transfer path, not yet
  withdraw/general transaction support.
  Code: `crates/privacy_orchard/src/verify.rs:239`,
  `crates/privacy_orchard/src/verify.rs:497`,
  `crates/privacy_orchard/src/verify.rs:541`,
  `crates/types/src/lib.rs:419`, `crates/node/src/lib.rs:6696`,
  `crates/node/src/lib.rs:7018`, `crates/node/src/lib.rs:7147`,
  `crates/node/src/main.rs:2425`.
- Local CLI gates exist for direct Orchard actions, Orchard output creation,
  Orchard spend creation, Orchard withdraw creation, Orchard
  keygen/view-key/scan/disclose/disclosure-verify reports, and ordered Orchard
  shielded batches.
  Code: `crates/node/src/main.rs:2372`, `crates/node/src/main.rs:2390`,
  `crates/node/src/main.rs:2425`, `crates/node/src/main.rs:2464`,
  `crates/node/src/main.rs:2484`, `crates/node/src/main.rs:2500`,
  `crates/node/src/main.rs:2630`.
- Asset-Orchard v1 exists for private NAV OTC movement inside PFTL. The
  consensus action enum includes `asset_orchard_ingress_v1`,
  `shielded_swap_v1`, and `asset_orchard_egress_v1`. Public ingress burns
  transparent issued-asset balances into typed private note commitments.
  Internal swaps use a fixed two-input/two-output Halo2 proof that hides raw
  asset ids, values, owners, recipients, and price while exposing nullifiers,
  output commitments, encrypted outputs, proof bytes, and signatures. Current
  egress is disclosed egress: it validates a revealed note opening and credits
  the public issued-asset ledger. It is functional exit, not private egress.
  Code: `crates/types/src/lib_parts/shielded_bridge_governance.rs`,
  `crates/privacy_orchard/src/asset_orchard.rs`,
  `crates/privacy_orchard/src/asset_orchard_circuit.rs`,
  `crates/node/src/privacy.rs`,
  `crates/node/src/lib_parts/part_02.rs`.
- Local Orchard wallet finality smoke exists:
  `scripts/testnet-orchard-wallet-finality-smoke` runs ordered shielded mint,
  migration into `orchard-v1`, Orchard output creation, wallet scan,
  one-note spend with an explicit change view key, one-note withdraw to a
  transparent account, post-spend recipient/change scans, shielded
  verification, and `tx` finality lookup for mint/migrate/output/spend/withdraw
  receipts. It now also creates and verifies a redacted change-note disclosure
  packet, re-verifies that packet after snapshot import, and records local
  proof/action byte and timing metrics.
  Latest evidence:
  `reports/testnet-orchard-wallet-finality-smoke/perf-malformed-v0-20260515T103617Z/testnet-orchard-wallet-finality-smoke.json`.
- Local peer-certified Orchard privacy smoke exists:
  `scripts/testnet-orchard-peer-certified-smoke` runs four local validators
  through peer-certified shielded mint, migration into `orchard-v1`, Orchard
  output, Orchard spend, and Orchard withdraw rounds, then verifies finality,
  convergence, shielded state, ledger credit, withdraw accounting, and
  recipient/change scans.
  Latest evidence:
  `reports/testnet-orchard-peer-certified-smoke/withdraw-v0-20260515T080523Z/testnet-orchard-peer-certified-smoke.json`.
- Node and RPC SDK expose shielded action batch, apply, scan, disclose, and
  turnstile paths.
  Code: `crates/node/src/lib.rs:2674`, `crates/node/src/lib.rs:2695`,
  `crates/node/src/lib.rs:2731`, `crates/node/src/lib.rs:2747`,
  `crates/node/src/lib.rs:2762`, `crates/node/src/lib.rs:2844`,
  `crates/node/src/lib.rs:2850`, `crates/rpc_sdk/src/lib.rs:329`,
  `crates/rpc_sdk/src/lib.rs:358`.

Evidence:

- SDK shielded RPC smoke script:
  `scripts/devnet-sdk-shielded-rpc-smoke:58`,
  `scripts/devnet-sdk-shielded-rpc-smoke:108`,
  `scripts/devnet-sdk-shielded-rpc-smoke:160`,
  `scripts/devnet-sdk-shielded-rpc-smoke:249`,
  `scripts/devnet-sdk-shielded-rpc-smoke:386`.
- Remote soak supports shielded round kinds:
  `scripts/testnet-remote-soak:88`,
  `scripts/testnet-remote-soak-checkpoint-smoke:26`.

Current gaps to production privacy:

- Current Orchard/Halo2 private transactions must not be claimed as end-to-end
  post-quantum private value.
- Local disclosure packets and verifier reports exist, but there is no approved
  auditor/view-key registry, recovery policy, sanctions/travel-rule posture, or
  custodian workflow.
- Wallet files, receive-only view-key export, wallet-created spends/withdraws,
  explicit change, scan witness generation, transparent/shielded turnstile
  accounting, and ordered disclosure evidence exist locally, but the public SDK
  ergonomics are still request-file/CLI-oriented.
- Root retention, archive/indexer policy, and stale-root behavior need broader
  release-grade testing.
- True private Asset-Orchard egress is not implemented. Current disclosed
  egress reveals the exited asset/value and note-opening material.
- `postfiat-node orchard-operator-policy` now reports privacy enablement,
  protocol proof/ciphertext/action caps, root retention, indexing role,
  verifier timeout/concurrency settings, and remote batch-create controls.
  `rpc-serve --allow-orchard-batch-create` can accept bounded Orchard
  `action_json` under child timeout/rate limits and write only to
  server-controlled spool paths. Direct public shielded apply remains
  disallowed.
- `scripts/testnet-orchard-rpc-malformed-edge-load-smoke` now proves the
  remote Orchard batch-create edge fails closed under three concurrent
  exact-size malformed proof actions, keeps child timeout count at zero, answers
  a post-load `status` request, and records parent+child RSS samples.
- `scripts/testnet-orchard-rpc-rate-limit-smoke` now proves per-peer and global
  Orchard batch-create caps reject excess requests at the RPC edge without
  child timeout.
- `scripts/testnet-orchard-rpc-threshold-gate` now turns the current
  malformed-edge and rate-limit reports into a pass/fail threshold gate with
  max malformed latency, max sampled RSS, and child-timeout limits.
- No private wallet/prover service flow.
- Proof/ciphertext byte pricing and shielded fee model are only partial.
- No audit of note format, proof program, encryption format, disclosure packet,
  or wallet scanning model.

### RPC

Implemented capabilities:

- RPC SDK builds and validates request/response envelopes for status, metrics,
  state, account, fee quote, mempool, shielded, bridge, governance-adjacent
  batch APIs, and XRP-like read aliases.
  Code: `crates/rpc_sdk/src/lib.rs:177`, `crates/rpc_sdk/src/lib.rs:251`,
  `crates/rpc_sdk/src/lib.rs:329`, `crates/rpc_sdk/src/lib.rs:1062`,
  `crates/rpc_sdk/src/lib.rs:1726`.
- `server_info`, `ledger`, `fee`, `validators`, and `manifests` are exposed as
  read-only RPC aliases; `tx`, `account`, and `transfer_fee_quote` continue to
  serve the existing XRP-like transaction/account/fee surfaces.
  Code: `crates/node/src/main.rs:6033`, `crates/node/src/main.rs:6429`,
  `crates/node/src/main.rs:7313`, `crates/rpc_sdk/src/lib.rs:14`,
  `crates/rpc_sdk/src/lib.rs:171`.
- Node `rpc-serve` is read-only by default and only enables signed mempool
  submission when `--allow-mempool-submit` is present.
  Code: `crates/node/src/main.rs:641`, `crates/node/src/main.rs:665`,
  `crates/node/src/main.rs:5719`, `crates/node/src/main.rs:5739`.
- RPC server rejects oversized request lines before parse.
  Code: `crates/node/src/main.rs:7476`.
- RPC server emits structured reports/events with request counts, ok counts,
  invalid signature counts, and rate-limit counts.
  Code: `crates/node/src/main.rs:5268`, `crates/node/src/main.rs:5358`,
  `crates/node/src/main.rs:5373`, `crates/node/src/main.rs:6027`.

Evidence:

- Wallet quote/sign/submit RPC smoke and readiness/P0 SDK signer default:
  `scripts/testnet-wallet-sign-transfer-smoke:19`,
  `scripts/testnet-wallet-sign-transfer-smoke:197`,
  `scripts/testnet-wallet-sign-transfer-smoke:259`,
  `scripts/testnet-wallet-sign-transfer-smoke:266`,
  `scripts/testnet-wallet-sign-transfer-smoke:290`,
  `scripts/testnet-wallet-sign-transfer-smoke:358`,
  `scripts/testnet-wallet-sign-transfer-smoke:454`,
  `scripts/testnet-readiness-gate:13`,
  `scripts/testnet-readiness-gate:231`,
  `scripts/testnet-p0-network-gate:19`,
  `scripts/testnet-p0-network-gate:322`,
  `scripts/testnet-p0-network-gate:762`.
- RPC write-edge load smoke:
  `scripts/testnet-rpc-write-edge-load-smoke:143`,
  `scripts/testnet-rpc-write-edge-load-smoke:248`,
  `scripts/testnet-rpc-write-edge-load-smoke:311`.
- RPC read-alias smoke:
  `scripts/testnet-rpc-read-alias-smoke:1`,
  `reports/testnet-rpc-read-alias-smoke/read-aliases-20260513T192121Z/testnet-rpc-read-alias-smoke.json`.
- Readiness/P0 gates require the read-alias smoke before reporting pass.
  Code: `scripts/testnet-readiness-gate:249`,
  `scripts/testnet-p0-network-gate:321`.
- Public RPC controlled-testnet operator policy is documented.
  Code: `docs/runbooks/public-rpc-operator-policy.md:1`.
- Live controlled RPC read-load script and evidence:
  `scripts/testnet-remote-rpc-read-load:1`,
  `reports/testnet-remote-rpc-read-load/current-20260514T170449Z/testnet-remote-rpc-read-load.json`,
  `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json`.
- Controlled write-edge policy audit and evidence:
  `scripts/testnet-controlled-write-edge-policy-audit:1`,
  `docs/runbooks/controlled-write-edge-policy.md:1`,
  `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`.

Current gaps:

- Need external-WAN load, archive/catch-up policy, public endpoint metadata,
  persistent write-edge installation if public write access is needed, and
  operational monitoring.

### Bridge

Implemented capabilities:

- Bridge domains, transfers, witness attestations, pause/resume actions, and
  bridge action batches exist as state-machine types.
  Code: `crates/types/src/lib.rs:486`, `crates/types/src/lib.rs:574`,
  `crates/types/src/lib.rs:589`, `crates/types/src/lib.rs:613`,
  `crates/types/src/lib.rs:650`, `crates/types/src/lib.rs:660`.

Current gap:

- Bridge is simulation only. It is not production external custody and should
  not be represented as such.

### Controlled Testnet And Machines

Implemented/exercised capabilities:

- Local readiness and P0 gates aggregate smokes into machine-readable reports.
  Code: `scripts/testnet-p0-network-gate:616`.
- Remote placement capacity profile fails closed before deployment when
  credential inventory/topology cannot satisfy strict requirements.
  Code: `scripts/testnet-remote-placement-capacity-profile:377`,
  `scripts/testnet-remote-placement-capacity-profile:449`,
  `scripts/testnet-remote-placement-capacity-profile:669`.
- Placement manifest support exists and redacts host/operator/provider labels.
  Code: `scripts/testnet-remote-placement-capacity-profile:111`,
  `scripts/testnet-remote-placement-capacity-profile:424`.
- Current strict 5-validator/quorum-4 placement capacity now passes when the
  three newly supplied machines are combined with the prior two.
  Evidence:
  `reports/testnet-remote-placement-capacity-profile/current-manifest/testnet-remote-placement-capacity-profile-20260513T043654Z.json`.

Current gaps:

- Post-launch soak/restart/load evidence now includes launch, wallet finality,
  100-round continuity, restart, partial outage, below-quorum outage, mixed
  read/write load, host-group outage, fresh observability, RPC edge-load, RPC
  read-load, write-edge policy, and the live hardening evidence pack. Remaining
  work is broader partition/capture, external-WAN/mixed-workload load, and
  keeping future release evidence exact after behavior-changing commits.
- Need broaden source-driven archive backfill and independent archive/indexer
  onboarding so PQ signature/certificate growth does not force every validator
  to become a full archival server.
- Need broaden topology diversity after controlled testnet before making public
  decentralization claims.

## Most Important Open Risks

1. **Privacy backend**: debug proof adapter must be replaced with zkVM/STARK
   backend and ML-KEM note encryption.
2. **Remote execution**: the first 5-validator controlled-testnet soak and
   exact-head final candidate gate passed, but post-cut operation still needs
   continued soak/restart/load evidence before broader public claims.
3. **Default node loop**: HotStuff-family ordering evidence exists, but default
   long-running production loop still needs sustained proof.
4. **Cobalt completeness**: validator lifecycle is strong for MVP, but needs
   canonical specs, manifests, replay package, essential-subset/trust-view
   analysis, and external review.
5. **Wallet/custody**: MVP CLI exists, but exchange/custodian-grade wallet,
   key rotation, address discovery, and SDK support remain incomplete.
6. **Validator history growth**: PQ signatures and certificates make full
   archival storage expensive; controlled testnet needs a partial-history
   validator policy plus archive/indexer separation.
7. **Benchmarks**: ML-DSA bandwidth/verification, certificate sizes, finality,
   RPC load, and privacy proof/ciphertext costs need published measurements.

## Current CTO Priority Order

1. Publish and operationalize the active 5-validator remote controlled-testnet
   candidate.
2. Package transparent PQ XRP-like chain as a reproducible release boundary:
   wallet, RPC, fees, mempool, receipts, validator cohort, runbooks, benchmarks.
3. Keep live hardening moving: broader partition/load drills, external-WAN RPC
   load, and persistent write-edge installation only when external write
   exposure is needed.
4. Define partial-history validator mode and archive/indexer responsibility.
5. Package Cobalt governance lifecycle as the canonical validator operation
   process.
6. Implement Confidential Settlement v1:
   zkVM proof backend, ML-KEM note envelopes, disclosure envelopes,
   wallet/prover/RPC path, fee pricing, and local/remote P0 evidence.
7. Bring in external consensus and cryptography review once the above evidence
   exists.
