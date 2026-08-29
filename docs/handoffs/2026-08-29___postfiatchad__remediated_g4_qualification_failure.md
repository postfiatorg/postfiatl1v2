# Remediated G4 qualification failure

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

The single campaign authorized by the
[remediated G4 plan](../plans/active/remediated-g4-qualification-campaign-plan.md)
ran once and **failed**. All ten selected-redb windows completed and passed
their per-window correctness and bounded-work gates, but their bound reports
miss both 1.10 height-ratio limits at `1.402629` and `1.401582`. The first
legacy control then failed the vote-lock migration-position gate because four
validators migrated on their second observed reservation. No final report or
packet exists, the packager was not invoked, and no retry is authorized. The
[storage milestone](../plans/active/storage-scaling-milestone.md) is updated with
the complete result.

## Current state

### Exact campaign result

| Boundary | Recorded fact |
| --- | --- |
| Candidate source | `a92bb085ceb6a9f405e916608e6b7bb6010fcc9b` |
| Candidate binary | SHA-256 `902773e00e5226dab9e027ebce2b932b2cf26509dba08424f6ebe46db985e182` |
| G1 / G2 | `ed66a6375234f64d5aab863bccb6415b07c77fc5a3a028c5a6c2f01f41af0190` / `dd300bcb8130f91ab54e26f969fe7dca37335d99cc5bf4ca78a939a79584d170` |
| Runner | `a3c7bea9285ab02871fd2111038764c6174b905b` on `postfiatchad/corrected-g4-vote-lock-gate` |
| Helper | SHA-256 `ad70ca685cfaf1d0a67eb80f4805438c0e4363c8957598d1d884abd03690014a` |
| Prepared input / verification | `c9fb32e7c3cebcf2ef16a90843c63dd96b7ed0ebc3c20ce94d2fd21707e7da42` / `6848d49d2488cd0730efd14863c5fe446a1f31827cec98346583beee8b9cbb58` |
| Measurement clock | `2026-08-29T14:44:45Z` to `2026-08-29T15:18:44Z`; 2,038.594669 / 14,400 seconds |
| Matrix reached | Ten selected windows complete: 5 × height 50 and 5 × height 5,000, 50 rounds each; first legacy height-50 window ran 50 raw rounds and then failed its gate |
| Failed unit | `legacy-jsonl/height-50-window-1` |
| Reason | `VOTE_LOCK_MIGRATION_AFTER_FIRST_VALIDATOR_RESERVATION` |
| Final report / packet | None / none; packager not invoked |
| Campaign rule | One run consumed; no resume, retry, relabeling, or packaging |

The private output is
`~/repos/postfiat-storage-g4-measurement-a3c7bea9-a92bb085-v1`. Preserve it;
do not commit, publish, delete, or treat it as redaction-safe packet material.

| Private failure artifact | SHA-256 |
| --- | --- |
| `campaign-checkpoint.json` | `e33dfdb628563f38d486ace5a3ebc13be280ecea5cb862a8da51627b1c6028a3` |
| Failed legacy raw report | `379a7b2630925b529ef55f727f92f38d32cfa49f3466f286e9fef12ab4815790` |
| Failed legacy vote-lock receipt | `4f3ad65296946d28bebd9a1ae88eb472ba92141deda5bb1b1bcacddb18cb4327` |
| One diagnosis | `e2134a4ea8988ced89e95f601b0cdc0aeaeffe9acd46676976f54adadb60c164` |

### Gate table

| Gate | Result |
| --- | --- |
| Consensus p95 height ratio | **FAIL** — 405.759 ms at height 50; 569.129 ms at height 5,000; ratio `1.402629` |
| Wallet-to-finality p95 height ratio | **FAIL** — 418.215 ms at height 50; 586.163 ms at height 5,000; ratio `1.401582` |
| Named synchronous-stage height model | PASS for the ten selected windows; no stage reports a material positive linear relationship |
| Round coverage | All 500 selected rounds pass, max residual 79.619 ms; failed legacy report independently recomputes to 50/50 pass, max 66.968 ms, but no receipt was emitted before the vote-lock stop |
| Selected/legacy height-50 comparison | **Unavailable / fail** — only one legacy window ran and it failed before becoming a completed unit |
| Selected correctness and storage work | PASS for all ten selected windows: literal receipts, six-validator convergence, bounded `redb` work, zero full-history reads |
| Vote-lock work | **FAIL** in first legacy window |
| Certified-send work | All ten selected windows pass; failed legacy report independently recomputes to pass, but no receipt was emitted before the vote-lock stop |
| Time budget | PASS — not exceeded |
| Overall | **FAIL** |

The ratio and named-stage values are exact recomputations from the ten
checkpoint-bound normalized reports using the frozen runner's aggregation
functions. They are diagnostics, not a fabricated final campaign report.

### What failed and why

The immediate stop is node-owned in
`crates/node/src/vote_locks.rs:193-260`. When the vote-lock directory contains
no JSON locks, `migrate_block_proposal_vote_locks` returns without writing the
durable index marker. The first reservation then creates a lock. A later
reservation sees that lock and performs migration.

In the failed legacy window, validators 0, 1, 2, and 5 each migrated in
finalized round 2 on their second observed reservation, examining four files
and 866 bytes. Validator 3 migrated in round 2 on its first observed
reservation and passed; validator 4 had no observed migration. The runner at
`benchmarks/storage-scaling/run_campaign.py:962-1107` correctly rejected the
four late migrations.

The preflight claim was too strong. Its fixture modeled first-use and late-use
telemetry, but did not execute the candidate's exact empty-directory → first
lock → second reservation sequence against a portable legacy restore. The
plan now records that retrospective preflight failure.

There is also an independent selected-path problem: even though every selected
window passed its local gates, the aggregate p95 height ratios are about 1.40.
Therefore an eager vote-lock marker fix alone cannot make this frozen result
pass.

### Repository and operational boundaries

- The repository was on `main` at pushed `c84083fb` when the run began. The
  closure documents are a docs-only successor; runtime source remains the
  frozen `a92bb085` candidate.
- The main worktree's two unrelated untracked auditor inventories under
  `docs/security/` were preserved untouched and unstaged.
- The runner and candidate source worktrees remain clean at their frozen
  revisions.
- No Task Node, agents, devnet, fleet, service, deployment, validator-copy, or
  height-924 action occurred.
- No live probe was performed. This handoff makes no new claim about the
  controlled devnet; see [Current State](../status/chain-state-current.md).
- Storage remains **selected, not offline qualified**. G3, G4, G5, deployment,
  and public testnet remain blocked.

## Next decision or action

Do not restart this campaign.

If Post Fiat chooses to continue storage qualification, the next bounded work
must be a new reviewed remediation, not another run. It must:

1. lock the intended empty-directory vote-lock marker contract and add both a
   node-owner fixture and the exact portable-restore end-to-end fixture;
2. explain and remediate the selected-window p95 tail that produces the two
   ~1.40 height ratios;
3. refresh every affected source, binary, G1/G2, runner, helper, and prepared-
   input binding; and
4. obtain separate authorization for exactly one new campaign.

G3 exact replay can remain a separate track, but height 924 still needs a named
custodian and separate read-only copy authorization. Do not wait idle for it.
No G5 packaging, G6 rehearsal, deployment, or public-testnet claim is allowed
until a future G4 passes and all other gates close.

## References

- [Remediated G4 qualification plan](../plans/active/remediated-g4-qualification-campaign-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Certified-send remediation handoff](2026-08-29___postfiatchad__certified_send_eager_index_remediation_complete.md)
- [Vote-lock owner source](https://github.com/postfiatorg/postfiatl1v2/blob/a92bb085ceb6a9f405e916608e6b7bb6010fcc9b/crates/node/src/vote_locks.rs)
- [Runner gate source](https://github.com/postfiatorg/postfiatl1v2/blob/a3c7bea9285ab02871fd2111038764c6174b905b/benchmarks/storage-scaling/run_campaign.py)
- [Current State](../status/chain-state-current.md)
