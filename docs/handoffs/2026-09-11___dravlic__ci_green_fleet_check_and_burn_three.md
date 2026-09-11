# CI green, fleet check, burn three closed early

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-11 UTC

## BLUF

No handoff or message arrived from the other lane on September 11, so work
continued on the QA campaign line. Three results landed: CI on `main` was
restored in `6a858064`; a [fresh read-only fleet observation](../status/chain-state-current.md)
landed in `1ee3383b`; and the [third QA burn](../review/qa-campaign-20260911.md)
reviewed storage, execution, Cobalt ratification, and network/mempool admission.
The original burn stopped during the network repair when the model provider
refused to continue. The closeout completed that repair in `f2dea308` and
closed the campaign early in `283853fd`. Operational Python CLIs (A5) and the
defect-inventory extension with its Text Improvement Harness gate (B) are the
resume point.

## Current state

### CI

- `docs-build` and `product-security-ci` are green from `6a858064` onward.
  The campaign had broken both: the link repair corrected 12 anchors in the
  [defect inventory](../review/defect-inventory-20260910.md), with the
  post-gate byte change and replacement SHA-256 recorded in the
  [burn-2 gate section](../review/qa-campaign-20260910.md); the secret repair
  replaced a realistic passphrase literal in the
  [wallet security regression](https://github.com/postfiatorg/postfiatl1v2/blob/6a858064cfaaf080f4815a83e9cd0509d6a2f038/wallet-extension/lib/security-regression.test.mjs)
  with a minimum-length placeholder.
- `rust-ci` is green on `40452f09`, which includes the execution repairs.
  At handoff time, GitHub runs for later repair commits were still in progress;
  storage, Cobalt, and network repairs therefore retain a **full Rust suite
  verdict pending** note until those runs are green.

### Fleet observation

The read-only observation ran from `2026-09-11T10:33:54Z` through `10:34:57Z`.
All six validators agreed at height 1020, tip `9d02b8ee…b1768feb`, and state
root `587c6526…d39bead6`, with empty mempools. All 12 validator and RPC
processes ran binary `57b0f4d1…634eec83`; nothing deployed changed, and RPC
repairs `83488d91` and `15af691d` remain undeployed.

Validator-0 RPC was answering. Its current process started at
`2026-09-11T06:16:08Z`, from a replacement outside this lane whose cause is
unknown. It had accepted 1,025 of its 10,000 lifetime connections, about 238 per
hour, implying straight-line exhaustion near `2026-09-13T00:21Z` if traffic
and process state remain unchanged.

### Burn three

The [brief](../review/qa-campaign-20260911-burn3-brief.md) and
[campaign log](../review/qa-campaign-20260911.md) are the governing index.
Completed surfaces found **5 P1, 7 P2, and 5 P3** issues:

- **A1 — Storage and snapshots:** 1 P1 / 3 P2 / 2 P3.
  [Review](../review/storage-snapshots-review-20260911.md); repair `69e1f1ce`
  handles torn FastSwap WAL suffixes, prevents failed snapshot imports from
  publishing partial destination state, and bounds WAL reads and growth. It is
  not consensus-affecting.
- **A2 — Execution:** 1 P1 / 2 P2.
  [Review](../review/execution-review-20260911.md); repair `e95efbdf` rejects
  duplicate receipt IDs that were ratifiable but uncommittable, applies the
  owned-object limit to ordered `OwnedDeposit`, and prevents validators from
  certifying replicated state the storage layer refuses. All three changes are
  consensus-affecting, source-only, and not activated.
- **A3 — Cobalt ratification:** 1 P1 / 1 P2 / 2 P3.
  [Review](../review/cobalt-ratification-review-20260911.md); repair
  `c9a61fcd` checks quorum overlap rather than raw subset overlap in transition
  witnesses and binds DABC full-knowledge activation to the pending candidate
  identity. Both changes are consensus-affecting, source-only, and not
  activated.
- **A4 — Network and mempool admission:** 2 P1 / 1 P2 / 1 P3.
  [Review](../review/network-mempool-review-20260911.md); findings `13a23d91`
  covered one worker per pre-authentication connection, unbounded memory and log
  growth from one unauthenticated persistent connection, and unlimited retained
  batch-service rejections. Repair `f2dea308` adds in-flight,
  per-connection-request, retained-summary, and rejection limits with a
  regression test. `cargo check` and all five focused validator transport
  tests pass. The repair is not consensus-affecting.

A5 and B were not started. Bridge, Orchard, proof, and program surfaces were
excluded by the brief because the other lane's
`recovery/private-funding-20260910` work reworks them outside this repository.
The devnet was not mutated, deployed, or restarted; the only fleet action was
the read-only observation. No Task Node action occurred.

## Next decision or action

1. Resume A5 under the [burn-3 brief](../review/qa-campaign-20260911-burn3-brief.md),
   then extend the defect inventory and run its Text Improvement Harness gate.
   A4 needs no further source work beyond its pending `rust-ci` verdict.
2. The consensus-affecting burn repairs `f9f13ead`, `e95efbdf`, and
   `c9a61fcd` remain on `main`, undeployed beside signing fix `bbb291ce`.
   The [PR #37/#39 lineage decision and deployment sheet](../governance/signing-fix-deploy-decision-20260909.md)
   remain the rollout gate; the fleet continues on the pre-fix binary.
3. If the validator-0 replacement at 06:16Z was not operator-initiated, its
   cause remains unknown. Its accept budget is projected to exhaust again near
   September 13 at 00:21Z unless the RPC repair is deployed or wallet readiness
   traffic stops.
4. The [September 10 handoff](2026-09-10___dravlic__qa_campaign_burn_one_done_burn_two_running.md)
   remains current for StakeHub PR #8 findings, the whitepaper abstract-rule
   decision, and the six inventory rows requiring an operator decision.

## References

- [Burn-3 brief](../review/qa-campaign-20260911-burn3-brief.md) and
  [campaign log](../review/qa-campaign-20260911.md)
- [Storage](../review/storage-snapshots-review-20260911.md),
  [execution](../review/execution-review-20260911.md),
  [Cobalt](../review/cobalt-ratification-review-20260911.md), and
  [network/mempool](../review/network-mempool-review-20260911.md) reviews
- [Canonical current fleet state](../status/chain-state-current.md)
- [Signing-fix deployment decision](../governance/signing-fix-deploy-decision-20260909.md)
  and [September 9 qualification handoff](2026-09-09___dravlic__signing_fix_qualification_hold.md)
- [Previous QA handoff](2026-09-10___dravlic__qa_campaign_burn_one_done_burn_two_running.md)
  and [other lane's last handoff](2026-09-09___codex__whitepaper_and_consensus_to_dravlic.md)
- CI and fleet commits: `6a858064`, `1ee3383b`
- Burn-3 commits: `c25b3389`, `8533f5d7`, `0d015227`, `69e1f1ce`,
  `0449de41`, `e95efbdf`, `40452f09`, `b5c16c2c`, `c9a61fcd`,
  `e1aafb3b`, `13a23d91`, `f2dea308`, `283853fd`
