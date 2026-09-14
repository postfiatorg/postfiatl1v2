# Fleet restart record and burn three closed

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-14 UTC

## BLUF

No handoff or message arrived from the other lane, and no other-lane change
landed on `main` after the September 11 handoff. The QA campaign continued.
A [read-only fleet observation](../status/chain-state-current.md) (`b573b6d8`)
established that the September 11 validator-0 RPC replacement was part of an
unattributed, fleet-wide service restart. The [third burn](../review/qa-campaign-20260911.md)
is closed: A5 operational Python CLIs were reviewed and repaired, and unit B
extended the defect inventory to 89 rows and passed its first full Text
Improvement Harness gate at 88.87/100. At handoff preparation, this repository
was on `main` at `292570b9`; source repairs have not been deployed.

## Current state

- **Fleet:** The latest read-only capture, `2026-09-14T11:30:24Z–11:32:29Z`,
  found six-of-six agreement at height 1020, tip `9d02b8ee…b1768feb`,
  state root `587c6526…d39bead6`, and empty mempools. Validator-2
  answered on retry after one timeout. All 12 validator/RPC processes run
  release `a666-source-route-20260907`, binary `57b0f4d1…634eec83`.
  RPC repairs `83488d91` and `15af691d` remain undeployed. See
  [Current State](../status/chain-state-current.md) for the capture.
- **September 11 restart:** Systemd entries show stop/start of the validator,
  RPC, Cobalt shadow, and NAVCoin archive RPC services on all six hosts:
  validator-2 `06:01:47Z`, validator-0 `06:16:06–08Z`, validator-4
  `06:25:15Z`, validator-5 `06:26:15Z`, validator-1 `06:26:48Z`,
  validator-3 `06:46:42Z`. Host uptimes date from July/August; no accepted
  SSH login appeared in the `05:45–07:00Z` journals of validators 0, 2, or 3,
  and this lane's server took no action in that window. The actor is unknown.
  This was neither a host reboot nor a deployment; the binary is unchanged.
  The [fleet record](../status/chain-state-current.md) supersedes the
  validator-0-only account in the previous handoff.
- **Validator-0 RPC:** Answering as PID 3611291 since
  `2026-09-11T06:16:08Z`; completed request index 5,655/10,000 at
  `11:31:36Z`. Its whole-process mean is 73.2 accepts/hour, down from the
  September 11 estimate of 238/hour. At that mean, exhaustion projects to
  approximately `2026-09-16T22:53Z`; the earlier September 13 prediction
  did not occur. Event logs lack timestamps, so a last-24-hour rate is
  unavailable.
- **A5 — operational Python CLIs:** All nine files in the
  [review](../review/operational-clis-review-20260911.md) were examined
  (`b25e3bec`). One P1 allowed transfer finality without a matching accepted
  receipt. Seven P2s involved incomplete account history, an implicit faucet
  data directory, noncanonical NAV asset identity, missing venue fields,
  unbounded venue response bytes, remote shadow catch-up without a separate
  interlock, and mixed registry receipt deadlines. Repair `c2724977`
  changed 16 Python files (+391/−81) with a regression per P1/P2; it does not
  affect consensus. The operator's rerun at `9f098df6` reported 183 passed,
  3 skipped (archived round fixtures outside the repository), and 44 subtests
  passed. Three P3s remain recorded without repair.
- **B — inventory and gate:** The [inventory](../review/defect-inventory-20260910.md)
  gained 28 rows (STO 6, EXE 3, COB 4, NET 4, OPS 11) in `2eec1ece`:
  89 total, comprising 25 P1, 44 P2, 20 P3. Dispositions are 48 fixed,
  16 dispositioned, 5 previously retained, 8 burn-3 P3 recorded without
  repair, 6 requiring a named live environment, and 6 requiring an operator
  decision. Run group `qa-defect-inventory-burn3-20260914` scored 88.87/100
  on its first full gate (five reviews each: `openai/gpt-6-astra-pro` 89.80,
  `anthropic/claude-fable-5.1` 86.80, `z-ai/glm-5.3` 90.00).
  The scored-file SHA-256 in the [campaign Scores section](../review/qa-campaign-20260911.md#scores)
  is `2e4adbc6f78686af8316291dca516827a2d7eea988848cb5705bf5ee9cf7311f`.
  The [campaign](../review/qa-campaign-20260911.md) closed in `292570b9`:
  A1–A5 found 6 P1, 14 P2, 8 P3; every P1/P2 was repaired.
- **CI:** `rust-ci`, `docs-build`, and `product-security-ci` are green on
  the September 11 commits including `f2dea308`, `283853fd`, and
  `736ba8f2`, closing the previous pending full-Rust-suite note.
  The single `gh run list --limit 9` check for today's commits showed:

  | Commit | `rust-ci` | `docs-build` | `product-security-ci` |
  | --- | --- | --- | --- |
  | `9f098df6` | **failed** | green | green |
  | `2eec1ece` | in progress | green | green |
  | `292570b9` | in progress | green | green |

No live probe was performed in this handoff session. The devnet was untouched
beyond the earlier read-only observation: no fleet mutation, deployment, or
restart by this lane. No Task Node action was taken.

## Next decision or action

1. The [PR #37/#39 decision and deployment sheet](../governance/signing-fix-deploy-decision-20260909.md)
   remain the rollout gate. The fleet still runs the pre-fix binary; signing
   fix `bbb291ce` and consensus-affecting source repairs `f9f13ead`,
   `e95efbdf`, `c9a61fcd` are merged on `main` but undeployed. Today's
   CLI repairs add no consensus-affecting change.
2. Establish the origin of the September 11 `06:01–06:47Z` fleet-wide service
   restart before rollout if the other lane did not initiate it. It did not
   change the deployed binary.
3. Address validator-0's accept budget before the conditional
   `2026-09-16T22:53Z` exhaustion estimate, through deployment of the RPC
   repair or stopping wallet readiness traffic.
4. Possible next campaign work: a fourth burn covering unreviewed
   `crates/types` encoding/hashing, the `crates/node` startup, deployment
   manifest and RPC serve loop actually run by the fleet, and consensus-v2
   signatures in `crates/ordering_fast` and `crates/crypto_provider`;
   alternatively, the open RPC serve-loop read-timeout and round-trip probe
   P2 in the [single-writer deployment plan](../plans/active/devnet-storage-single-writer-deployment-plan.md).
5. [September 10's handoff](2026-09-10___dravlic__qa_campaign_burn_one_done_burn_two_running.md)
   still governs StakeHub #8 branch findings and the whitepaper abstract-rule
   decision; six inventory rows need an operator decision and six a named live
   environment.

## References

- [Current fleet state](../status/chain-state-current.md) and
  [burn-3 brief](../review/qa-campaign-20260911-burn3-brief.md)
- [Campaign and gate](../review/qa-campaign-20260911.md),
  [A5 review](../review/operational-clis-review-20260911.md), and
  [defect inventory](../review/defect-inventory-20260910.md)
- [Signing-fix deployment decision](../governance/signing-fix-deploy-decision-20260909.md)
  and [single-writer deployment plan](../plans/active/devnet-storage-single-writer-deployment-plan.md)
- [Previous handoff](2026-09-11___dravlic__ci_green_fleet_check_and_burn_three.md)
  and [other lane's last handoff](2026-09-09___codex__whitepaper_and_consensus_to_dravlic.md)
