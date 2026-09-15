# Burn four closed and RPC health probe

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-15 UTC

## BLUF

At the start of the day, no other-lane handoff or message had arrived and no new
change had landed on `main` since the [September 14 handoff](2026-09-14___dravlic__fleet_restart_record_and_burn_three_closed.md).
The [fourth QA burn](../review/qa-campaign-20260915.md) closed five previously
unreviewed surfaces: 0 P1, 10 P2 repaired with regressions, and 5 P3 recorded
without repair. The [inventory](../review/defect-inventory-20260910.md) reached
104 rows and passed its first compliant full gate at 88.00/100. The
[single-writer plan's RPC read-timeout and health-probe P2](../plans/active/devnet-storage-single-writer-deployment-plan.md)
is closed in source (`b1b782d8`, `5ac323f9`, `035c99c6`); nothing was
deployed. At handoff preparation, the repository was on `main` at
`6c265697`.

## Current state

- **Burn 4:** The [brief](../review/qa-campaign-20260915-burn4-brief.md) is
  `d4037d14`; the [campaign log](../review/qa-campaign-20260915.md) indexes
  all five reviews, their focus-path limits, failures, repairs, and focused
  verification. Each surface found 0 P1 / 2 P2 / 1 P3:

  | Surface and review | Findings → repair | P2 repaired |
  | --- | --- | --- |
  | [A1 finality, artifacts, signing](../review/finality-consensus-review-20260915.md) | `abb42827` → `6ec35092` | Maximum-view timeout successor overflow; conflicting proposer signatures before durable proposal-lock reservation. Consensus-affecting. |
  | [A2 types and state commitment](../review/types-state-commitment-review-20260915.md) | `f5c8e88a` → `0a1216c3` | FastPay committee root omitted both new-order admission heights; recovery-reveal commitments omitted retained certificate bytes. Consensus-affecting: identities and state-root bytes change. |
  | [A3 node startup, release, RPC](../review/node-serving-review-20260915.md) | `910030ce` → `bccd5b5f` | Keep-alive omitted earlier request events (now logged per request, limited to 64 per connection); RPC/transport ready files preceded serving preflights. Not consensus-affecting. |
  | [A4 Cobalt shadow, FastSwap services](../review/shadow-swap-services-review-20260915.md) | `b2df0ae1` → `33c8ce34` | Round ordering lowered a shadow peer's replay watermark; issuer asset-control prepare could sign a stale round-zero vote. Consensus-affecting per the brief; shadow evidence is not live authority. |
  | [A5 node CLI, governance agent](../review/node-cli-governance-review-20260915.md) | `face08c2` → `eb4c2afd` | Repeated shadow flags selected an earlier live target; FastSwap bootstrap overwrote an existing governance payload. Not consensus-affecting. |

  The five P3s remain recorded, not fixed. Large files were reviewed at the
  specified focus paths, not line by line. Each surface's focused tests passed
  in the [campaign Verification section](../review/qa-campaign-20260915.md#verification);
  the operator's reruns at `5c558d65` passed ordering_fast 33, postfiat-types
  135, and 20 focused node tests. Full Rust-suite verdicts are CI's, below.

- **Inventory:** Fifteen rows (three each of FIN-, TYP-, SRV-, SHD-, CLI-) were
  added in `ec9cc1a0`; closeout `66d24dcd`. Totals: 104 rows, 25 P1,
  54 P2, 25 P3. Run group `qa-defect-inventory-burn4-20260915` averaged
  **88.00/100** on the first compliant full score (five reviews per model:
  `openai/gpt-6-astra-pro` 88.40, `anthropic/claude-fable-5.1` 86.60,
  `z-ai/glm-5.3` 89.00). Scored-file SHA-256, from the
  [Scores section](../review/qa-campaign-20260915.md#scores):
  `795542e3964aef09681bd69dc8f31317726a66997105f0156e5781f8496d9298`.
  No rewrite or rescore.

- **RPC P2:** The existing serve loop bounds first-byte, whole-frame, and idle
  keep-alive reads by a socket timeout and fixed 30-second deadline;
  `b1b782d8` changes no serve-loop behavior and tests that an idle socket
  closes while accepting continues. `5ac323f9` adds no-retry, one-connection,
  read-only status probes in
  [Rust](https://github.com/postfiatorg/postfiatl1v2/blob/6c2656976aad2ab9de18c8cc528a31242a43b0d9/crates/node/src/rpc_probe.rs) and
  [Python](https://github.com/postfiatorg/postfiatl1v2/blob/6c2656976aad2ab9de18c8cc528a31242a43b0d9/python/postfiat_rpc/rpc_probe.py):
  `postfiat-node rpc-probe --port PORT [--host HOST] [--timeout-ms MS]`
  (127.0.0.1, 5000 ms by default), or
  `python3 -m postfiat_rpc.rpc_probe --endpoint HOST:PORT [--timeout-seconds S]`
  (5 seconds by default). Exit 0 reports height, tip prefix and round-trip
  milliseconds; exit 1 gives a one-line refusal, timeout, no-response,
  malformed-response or `ok:false` reason. See the
  [operator policy](../runbooks/public-rpc-operator-policy.md). The operator's
  `035c99c6` reruns passed Rust probe 2, idle keep-alive 1, adjacent
  keep-alive/stalled-client 3, and Python probe 3. This is not
  consensus-affecting and is not deployed.

- **Rust coverage:** `6c265697` adds bounded property and fuzz cases around
  every Rust burn-3/4 repair, including 5,265 object-cap combinations, 120
  duplicate receipt positions, 128 quorum-overlap combinations, 5,137
  retained-summary attempts and 128 reveal-signature mutations. The
  [coverage section](../review/qa-campaign-20260915.md#burn-3-and-4-fuzz-and-property-coverage)
  names tests and case counts: of the FastPay committee window corpus's 260
  cases, **one parsed and reached the invariant check; 259 were rejected at
  parse**, with zero invariant failures. The operator reran consensus_cobalt,
  types, ordering_fast, execution and the fuzz corpus. Tests only; no behavior
  change. Burn-3 Python repair `c2724977` was outside this Rust-only unit.

- **Fleet and lineage:** The [Current State](../status/chain-state-current.md)
  retains the latest recorded fleet capture, `2026-09-14T11:30:24Z–11:32:29Z`.
  A separate operator read-only identity/event-log check at
  `2026-09-15T07:57Z` found six hosts on the same processes and binary since
  the September 11 restarts; all six RPC event logs were last written at
  `2026-09-14T14:28Z`. Validator-0 stood at request 5,658/10,000;
  wallet-readiness traffic had stopped, so accept-budget exhaustion was no
  longer approaching at the previous rate. No new observation was entered in
  Current State today. All validator/RPC processes still run release
  `a666-source-route-20260907`, binary `57b0f4d1…634eec83`.
  Signing fix `bbb291ce`, burn-1–3 consensus repairs `f9f13ead`,
  `e95efbdf`, `c9a61fcd`, and burn-4 consensus repairs `6ec35092`,
  `0a1216c3`, `33c8ce34` are seven change sets merged on `main`,
  source-only and undeployed. The fleet binary also lacks the RPC probe and
  earlier RPC repairs. No live probe was performed in this handoff session;
  no devnet mutation, deployment, restart or Task Node action occurred.

- **Full Rust CI:** Single `gh run list --workflow rust-ci.yml --limit 25`
  snapshot at approximately `2026-09-15T12:39Z`. Each status applies to
  that commit's run at handoff time; in-progress runs have no verdict.

  | Commit | rust-ci | Commit | rust-ci |
  | --- | --- | --- | --- |
  | `d4037d14` | green | `abb42827` | green |
  | `6ec35092` | green | `6c50c0ea` | green |
  | `f5c8e88a` | green | `0a1216c3` | green |
  | `46cec6d7` | in progress | `910030ce` | failed |
  | `bccd5b5f` | green | `35629ca9` | in progress |
  | `b2df0ae1` | in progress | `33c8ce34` | in progress |
  | `0daae811` | green | `face08c2` | in progress |
  | `eb4c2afd` | in progress | `5c558d65` | in progress |
  | `ec9cc1a0` | in progress | `66d24dcd` | in progress |
  | `b1b782d8` | in progress | `5ac323f9` | in progress |
  | `035c99c6` | in progress | `6c265697` | in progress |

  Documentation-only `910030ce` failed 3 timing-sensitive
  [transport batch tests](https://github.com/postfiatorg/postfiatl1v2/blob/6c2656976aad2ab9de18c8cc528a31242a43b0d9/crates/node/src/main_parts/tests/transport_batch_payload_tests.rs)
  (149 passed, 3 failed); the same family failed on documentation-only
  `9f098df6` on September 14 while neighboring code commits passed.
  Treat it as a flaky CI family pending determinism; it is not evidence against
  a repair. [Failed CI run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/34961493785).

## Next decision or action

1. The [PR #37/#39 rollout decision](../governance/signing-fix-deploy-decision-20260909.md)
   remains the gate for the seven undeployed consensus-affecting change sets.
   Identify the owner of the September 11 fleet-wide service restart
   (`06:01–06:47Z`, all six hosts) before rollout.
2. Next campaign choices: enumerate unreviewed `crates/node` modules against
   the eleven review documents before burn five; sweep the thirteen
   recorded-not-fixed P3 findings from burns 3 and 4; or, after the #37/#39
   lineage decision, qualify a deployment-exact candidate containing all seven
   change sets by the September 9 clone-gate procedure.
3. Make the `transport_batch_payload` timing tests deterministic or
   quarantine them so main's CI verdict does not depend on scheduling.
4. [September 10's handoff](2026-09-10___dravlic__qa_campaign_burn_one_done_burn_two_running.md)
   still governs StakeHub #8 branch findings, the whitepaper abstract-rule
   call, and inventory rows requiring an operator decision or named live
   environment.

## References

- [Burn-4 brief](../review/qa-campaign-20260915-burn4-brief.md),
  [campaign log](../review/qa-campaign-20260915.md), and
  [defect inventory](../review/defect-inventory-20260910.md).
- [Current State](../status/chain-state-current.md),
  [deployment decision](../governance/signing-fix-deploy-decision-20260909.md),
  [single-writer plan](../plans/active/devnet-storage-single-writer-deployment-plan.md),
  and [RPC operator policy](../runbooks/public-rpc-operator-policy.md).
- [Previous handoff](2026-09-14___dravlic__fleet_restart_record_and_burn_three_closed.md)
  and [other lane's last handoff](2026-09-09___codex__whitepaper_and_consensus_to_dravlic.md).
