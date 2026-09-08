# Safety landed and UNL V2 milestone complete

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-08 UTC

## BLUF

This session landed the previously reviewed
[safety closeout](2026-09-07___codex__safety_closeout.md), restored a fully
green CI baseline on `main`, and implemented all five sections of the
[UNL Amendment V2 milestone](../plans/completed/tasknode-unl-amendment-v2-milestone.md)
in one day. The evidence contract, admission policy, paired adversarial and
liveness gate, explicit human CLI, and user report are complete. V2 remains
`SHADOW_ONLY`: this is implementation and offline evidence, not promotion or
live authority.

## Current state

- **Safety and CI:**
  [`80f2232b`](https://github.com/postfiatorg/postfiatl1v2/commit/80f2232b9ca92f32253fedcd016348d65c2904fc)
  landed the 15 files mapped by the safety-closeout threads without additional
  content edits. The linked [proof-input review](../review/proof-input-review-20260907.md)
  records the reproduced public-tree hygiene sequence, 125 reserve tests with
  three existing ignored SP1 cases, 20 A666 tests, all ten fuzz targets, and
  strict documentation build. Its
  [product-security-ci run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/34208486770)
  was the first full pass for that workflow. Rust CI then passed on
  [`519b1bad`](https://github.com/postfiatorg/postfiatl1v2/commit/519b1bad2df92afd0f1b5c5f6841c4dca18f358e)
  ([run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/34209858520)),
  establishing the fully green three-workflow baseline. That commit changes
  only the [genesis-registry golden-vector test](https://github.com/postfiatorg/postfiatl1v2/blob/519b1bad2df92afd0f1b5c5f6841c4dca18f358e/crates/types/src/genesis_registry_tests.rs):
  the failure introduced at
  [`fa7e67ff`](https://github.com/postfiatorg/postfiatl1v2/commit/fa7e67ff7289488d32dda0537578a6d792d18efe)
  required a source archive already documented as fetched out-of-tree at
  [`a1161ff9`](https://github.com/postfiatorg/postfiatl1v2/commit/a1161ff9c54704f7acb022f8c320801eafa01fb2).
  It does not change the
  vectors or serialization; the Rust and Python implementations retain
  byte-for-byte agreement.

- **UNL Amendment V2:** The locked
  [amendment](../governance/tasknode-unl-amendment-v2-20260907.md) still hashes
  to `a74bdf603c70ebc3432499b555b470d87da93e13c54424b3687f834bf02f892f`.

  | Section | Commit and implemented surface |
  | --- | --- |
  | A — evidence contract | [`a710f96c`](https://github.com/postfiatorg/postfiatl1v2/commit/a710f96c2f58fe840d831744ceb0e971a230fbfd): closed versioned [schemas](https://github.com/postfiatorg/postfiatl1v2/blob/a710f96c2f58fe840d831744ceb0e971a230fbfd/python/postfiat_rpc/tasknode_unl_v2_schema.py) and [evidence verification](https://github.com/postfiatorg/postfiatl1v2/blob/a710f96c2f58fe840d831744ceb0e971a230fbfd/python/postfiat_rpc/tasknode_unl_v2_evidence.py), including control epochs, the 180-day fresh window, separated consent classes, no sale-detection field, the explicit unchanged-key limit, and malformed-record isolation. |
  | B — graph and admission | [`1306ac26`](https://github.com/postfiatorg/postfiatl1v2/commit/1306ac2679c97fd9ed7729a8d9edcac1059c8ab6): frozen-window [policy](https://github.com/postfiatorg/postfiatl1v2/blob/1306ac2679c97fd9ed7729a8d9edcac1059c8ab6/python/postfiat_rpc/tasknode_unl_v2_policy.py), exact `max(2,N/10)` caps, `CLEAR` / `SATURATED` / `EXISTING_BREACH`, preserved incumbent breaches, and no funding-derived positive mass. |
  | C — paired gate | [`41f6ae1a`](https://github.com/postfiatorg/postfiatl1v2/commit/41f6ae1a59a94d07f37b2df6095f643382bb1b95): preregistered [offline gate](https://github.com/postfiatorg/postfiatl1v2/blob/41f6ae1a59a94d07f37b2df6095f643382bb1b95/benchmarks/ai-governance/tasknode-unl-v2-gate-20260908/README.md), outputs, and committed trial history. |
  | D/E — CLI and report | [`74c9e732`](https://github.com/postfiatorg/postfiatl1v2/commit/74c9e732557f2cea69859c2524def53af42c53d5): explicit `python -m postfiat_rpc.tasknode_unl_v2` `derive`/`render` interface and [human report documentation](../governance/tasknode-unl-v2-shadow-report.md); milestone retired to completed plans. |

- **Paired gate:** The core and extension preregistrations were locked before
  their trials at `94bc0979…` and `180698bd…`. The frozen V1 baseline was
  byte-verified and not rewritten. The verdict is `PASS_SHADOW_ONLY`, with
  14/14 consent-complete honest controls admissible; disconnected vouch rings
  and parallel identities gained zero seats, while unsolicited dust and larger
  inflows were audit-only and did not change target mass, cluster, group, or
  result. Hidden unchanged-key control changes and undeclared common funding
  remain unresolved public-input limits, and accepted seed-connected relations
  can admit attacker-controlled accounts within the tested budget. These are
  design facts exposed by the [gate results](https://github.com/postfiatorg/postfiatl1v2/blob/41f6ae1a59a94d07f37b2df6095f643382bb1b95/benchmarks/ai-governance/tasknode-unl-v2-gate-20260908/outputs/results.json),
  not live-validator evidence.

- **Verification and protected boundaries:** The combined V1/V2 selection
  passed 167 tests and 43 subtests; the actual CLI separately passed 10 tests.
  The V1 modules, frozen V1 simulation artifacts, amendment and lock records,
  and whitepapers are byte-unchanged across the four V2 implementation commits.

- **Task Node:** Sections A and B used tasks
  `task_aece75f855b2633598b89dffd266a024` and
  `task_91a089bd14dedf6b6fb681fae7e49561`; initial evidence was submitted and
  both await verification. Section C generation requests `req_cf56f893…` and
  `req_6948d197…` failed with `taskgen_provider_output_invalid` and
  `needsAttention=true`. The operator then ended Task Node lifecycle use for
  sections C–E; the milestone journal is the retained record.

- **Operational boundary:** No devnet, validator fleet, deployment, registry,
  or economic action occurred, and no live probe was performed. The only
  writes outside Git were the two authorized standard Task Node lifecycles for
  A/B; other network access was read-only CI inspection. The last documented
  fleet observation remains height 924 on 2026-08-26 in
  [Current State](../status/chain-state-current.md). Source milestone head
  `74c9e732` is not a deployed-binary claim.

- **Other lanes:** The whitepaper was a read-only reference; its separate
  [whitepaper handoff](2026-09-08___codex__whitepaper_to_dravlic.md) remains
  unchanged. StakeHub was untouched; `fix/pr8-safety-20260907` and its reviewed
  working tree remain local and unpushed. PR #8 publication or disposition is
  still an operator decision.

## Next decision or action

1. Read the [gate README](https://github.com/postfiatorg/postfiatl1v2/blob/41f6ae1a59a94d07f37b2df6095f643382bb1b95/benchmarks/ai-governance/tasknode-unl-v2-gate-20260908/README.md)
   and [shadow report](../governance/tasknode-unl-v2-shadow-report.md) before
   considering promotion. The unresolved control/funding limits and accepted
   seed-bridge admissions are policy facts, not missing code.
2. Verify the Section A/B Task Node evidence when convenient, then decide
   whether to backfill lifecycle records for C–E or retain the failure note as
   the final ledger record.
3. Decide whether to push `fix/pr8-safety-20260907` and update StakeHub PR #8,
   or dispose of the local patch another way.
4. The [2026-09-03 direction adoption](2026-09-03___dravlic__agent_direction_decided_z3_planned.md)
   and [Z3 G1](../plans/active/z3-navcoin-roundtrip-plan.md) remain open. The
   [PR #38 decision](../governance/yolo-deploy-decision-20260907.md) is recorded
   as option A and awaits the next separately qualified release; activation
   remains unscheduled.

## References

- [Safety closeout](2026-09-07___codex__safety_closeout.md)
- [Completed UNL Amendment V2 milestone](../plans/completed/tasknode-unl-amendment-v2-milestone.md)
- [Locked UNL Amendment V2](../governance/tasknode-unl-amendment-v2-20260907.md)
- [V2 shadow CLI and report](../governance/tasknode-unl-v2-shadow-report.md)
- [V2 paired gate](https://github.com/postfiatorg/postfiatl1v2/tree/41f6ae1a59a94d07f37b2df6095f643382bb1b95/benchmarks/ai-governance/tasknode-unl-v2-gate-20260908)
- [Separate whitepaper handoff](2026-09-08___codex__whitepaper_to_dravlic.md)
- [Current State](../status/chain-state-current.md)
