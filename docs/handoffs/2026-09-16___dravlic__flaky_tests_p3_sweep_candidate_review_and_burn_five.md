# Flaky tests, P3 sweep, candidate review, and burn five

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-16 UTC

## BLUF

The [other lane's handoff](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/handoffs/2026-09-16___codex__combined_release_check_kickoff_and_private_funding.md)
combined PR #37, PR #39 and seven queued consensus fixes into draft PR #41,
qualified on saved copies, and requested a fifth review, a minor-bug sweep and
deterministic timing tests. Today on `main`, the transport fixture was made
deterministic, [twelve of thirteen P3 findings were repaired](../review/qa-campaign-20260915.md#p3-sweep),
and a [read-only candidate review](../review/pr41-release-candidate-review-20260916.md)
found two P1s and one P2 while confirming the qualification packet. With time
left, [burn five](../review/qa-campaign-20260916.md) covered five previously
unreviewed areas outside the candidate's changes: 1 P1, 14 P2, 7 P3; the P1 and
thirteen P2s were repaired, with SMG-07 blocked by candidate overlap. The
126-row inventory passed at 86.33/100. Neither the fleet nor candidate branch
was changed.

## Current state

- **Repository:** `main` at `c52e7c44` before this handoff; initial
  `git pull --rebase origin main` was already up to date. Today's commits are
  on main only, outside PR #41. The candidate remains
  `release/combined-devnet-20260915` at `15126ac3`; the review's temporary
  worktree was removed.

- **Timing tests — [8a2b0824](https://github.com/postfiatorg/postfiatl1v2/commit/8a2b0824):**
  The `transport_batch_payload` fixture released its chosen ports and treated
  `AddrInUse` as readiness, allowing port reuse and false readiness. A panic
  then poisoned a shared mutex and cascaded into other transport tests. The
  fixture now retains transport and RPC listeners for the whole scenario;
  `rpc_serve` moved from
  [rpc_cli.rs](https://github.com/postfiatorg/postfiatl1v2/blob/8a2b0824/crates/node/src/rpc_cli.rs)
  to [rpc_serve_runtime.rs](https://github.com/postfiatorg/postfiatl1v2/blob/8a2b0824/crates/node/src/rpc_serve_runtime.rs),
  with test-only transport listener hooks in
  [transport_runtime_tests.rs](https://github.com/postfiatorg/postfiatl1v2/blob/8a2b0824/crates/node/src/transport_runtime_tests.rs).
  No assertion or timeout was weakened. Under CPU load, before: 1/20
  whole-target runs failed; after: 90/90 executions of the three named tests
  passed and 29/30 whole-target runs passed. The remaining failure was
  `fastswap_service::tests::replacement_relayer_recovers_expired_partial_prepare_via_rpc_evidence`.
  **The strict 30/30 whole-target gate was not met within the time box.**
  The stress script and evidence notes remain untracked under `.tih/` on the
  server.

- **P3 sweep — [7095b393](https://github.com/postfiatorg/postfiatl1v2/commit/7095b393),
  [d8932b57](https://github.com/postfiatorg/postfiatl1v2/commit/d8932b57),
  [3e56203a](https://github.com/postfiatorg/postfiatl1v2/commit/3e56203a):**
  Each repair has a regression: storage STO-03; Cobalt COB-03 (signed-beacon
  authentication), COB-04; network NET-04 (legacy validator-set false quorum);
  operational CLIs OPS-09/10/11; finality FIN-03; types TYP-03; node serving
  SRV-03; shadow/swap SHD-03; governance CLI-03. STO-04 was skipped: both
  revisions already write receipt state once, so it is not a defect at HEAD.
  COB-03, NET-04, TYP-03 and SHD-03 are conservatively consensus-affecting,
  source-only. At sweep close, the inventory had 70 fixed and one recorded P3
  remaining. [The campaign section](../review/qa-campaign-20260915.md#p3-sweep)
  records Rust 72 and Python 63 passes; the operator's reruns at `3e56203a`
  passed Python 63, consensus_cobalt 75, types 138, ordering_fast 35 and
  decision oracle 4.

- **Candidate review — [b5ef5bd6](https://github.com/postfiatorg/postfiatl1v2/commit/b5ef5bd6):**
  The [review](../review/pr41-release-candidate-review-20260916.md) identifies
  file/line at `15126ac3`, condition, observed/expected behavior and minimal
  suggested change for each finding:

  | Severity | Finding |
  | --- | --- |
  | P1 | Arc's initial validator set and checkpoint lie outside signed route authorization; activation accepts different bootstrap trust states under one authorized profile. |
  | P1 | FastPay accepts certificates the V2 commitment cannot encode: verification skips unknown voters and lacks certificate count/shape bounds. |
  | P2 | Arc finality cursor advancement rejects independent deposits at the same or an earlier height. |

  All 79 listed checksums matched; the raw test totals were 1,433 passed,
  0 failed, 39 ignored; all six history reports agreed through block 1021;
  source references matched `1c435f4f`. These are saved-copy qualification
  checks, not live authority. The time box did not reach an exhaustive core
  review, complete Arc guest/host, pfETH ingress, Ethereum contracts, Arc
  conformance, remaining prover code, generated evidence diff, standalone
  egress-identity tests, independent proof reproduction or a saved-chain
  replay. Nothing on the candidate was changed.

- **Burn five:** [Brief](../review/qa-campaign-20260916-burn5-brief.md)
  ([c4303717](https://github.com/postfiatorg/postfiatl1v2/commit/c4303717));
  [campaign index](../review/qa-campaign-20260916.md). Each allowed surface
  was read in full; line counts and coverage limits are in the log. Reviews
  contain file, line, condition and suggested change. Repairs have regression
  tests; classifications below are source-only.

  | Surface/review | P1 / P2 / P3 | Findings → repair | Result and consensus impact |
  | --- | --- | --- | --- |
  | [A1 mempool proposals](../review/mempool-proposals-review-20260916.md) | 0 / 1 / 1 | [6f2332cf](https://github.com/postfiatorg/postfiatl1v2/commit/6f2332cf) → [1c9f44f1](https://github.com/postfiatorg/postfiatl1v2/commit/1c9f44f1) | Fixed pending offers bypassing the sender pending limit; not consensus-affecting. |
  | [A2 vote locks and view recovery](../review/vote-locks-review-20260916.md) | 1 / 1 / 0 | [1511ea09](https://github.com/postfiatorg/postfiatl1v2/commit/1511ea09) → [090bd17e](https://github.com/postfiatorg/postfiatl1v2/commit/090bd17e) | Fixed missing directory durability barrier at lock publication and migration ignoring non-regular JSON lock entries; both conservatively consensus-affecting. |
  | [A3 Cobalt handoff and authority](../review/cobalt-handoff-review-20260916.md) | 0 / 4 / 2 | [34611d66](https://github.com/postfiatorg/postfiatl1v2/commit/34611d66) → [21cf30f8](https://github.com/postfiatorg/postfiatl1v2/commit/21cf30f8) | Fixed older full-knowledge checkpoints accepted for newer decisions and unchecked compact-transcript expansion (consensus-affecting); failed response writes terminating the shadow listener and rehearsal panic on the Foundation route (not consensus-affecting). |
  | [A4 storage migration, activation, certified-send index](../review/storage-migration-review-20260916.md) | 0 / 5 / 2 | [307214ef](https://github.com/postfiatorg/postfiatl1v2/commit/307214ef), [514f7e15](https://github.com/postfiatorg/postfiatl1v2/commit/514f7e15) → [401fa055](https://github.com/postfiatorg/postfiatl1v2/commit/401fa055) | Fixed activation artifact overwrite race, append recovery missing directory durability barriers, intent recovery hiding unrelated divergence, and failed backend validation leaving the candidate selected; none consensus-affecting. SMG-07 (interrupted prune rejecting retention payload paths) remains unfixed: repair needs a candidate-changed file. |
  | [A5 swap and recovery services](../review/swap-recovery-services-review-20260916.md) | 0 / 3 / 2 | [696eeffa](https://github.com/postfiatorg/postfiatl1v2/commit/696eeffa) → [d679f8e8](https://github.com/postfiatorg/postfiatl1v2/commit/d679f8e8) | Fixed FastPay rollback comparing different input orderings (consensus-affecting); forward journal transitions replacing batch identity and persistence exceeding its own 32 MiB reload limit (not consensus-affecting). |

- **Inventory and gate:** [Inventory](../review/defect-inventory-20260910.md)
  at [17640b29](https://github.com/postfiatorg/postfiatl1v2/commit/17640b29):
  126 rows — 26 P1, 68 P2, 32 P3; 84 fixed, 16 dispositioned, 5 retained,
  8 recorded unfixed, 1 scope-blocked, 6 needing live evidence, 6 needing an
  operator decision. First full gate **86.33/100**: `openai/gpt-6-astra-pro`
  85.60, `anthropic/claude-fable-5.1` 83.80, `z-ai/glm-5.3` 89.60. Run group
  `qa-defect-inventory-burn5-20260916`; scored-file SHA-256 from
  [Scores](../review/qa-campaign-20260916.md#scores):
  `f81ff48dd6f1cd9afe49f57ffe0b8548cc2af14a13ed2b964641ca2970a24c35`.
  No rewrite or rescore; campaign closed in
  [c52e7c44](https://github.com/postfiatorg/postfiatl1v2/commit/c52e7c44).
  The last three burn scores fell 88.87 → 88.00 → 86.33. The next extension
  risks falling below 86 unless wording is tightened or the document split;
  this is a wording task, not a change to findings.

- **Undeployed changes:** The earlier seven sets `e95efbdf`, `c9a61fcd`,
  `6ec35092`, `0a1216c3`, `33c8ce34`, `bbb291ce`, `83488d91` are already
  ancestors of the candidate. Today's four conservatively consensus-affecting
  P3 repairs and burn-five repairs `090bd17e`, `21cf30f8`, `d679f8e8` are on
  main only. These changes remain undeployed; shadow and rehearsal evidence
  is not live authority.

- **Fleet:** No live probe today. The [Current State record](../status/chain-state-current.md)
  remains the September 14 capture, `2026-09-14T11:30:24Z–11:32:29Z`: six
  validators agreed at height 1020 with empty mempools; all 12 validator/RPC
  processes ran release `a666-source-route-20260907`, binary
  `57b0f4d1…634eec83`. That recorded deployment lineage is separate from
  today's main and candidate source. No devnet change, fleet mutation,
  deployment or restart occurred. **No Task Node action by this lane.**

- **Full Rust CI:** One `gh run list --workflow rust-ci.yml --limit 12`
  query (JSON fields added for commit/status matching), at approximately
  `2026-09-16T13:12:39Z`. Only three requested commits appeared. The other
  seven were outside the returned window: their verdicts were **not observed**,
  so no green, failed or in-progress status can be asserted for them. No
  additional CI lookup or full Rust suite was run for this handoff.

  | Commit | rust-ci at observation |
  | --- | --- |
  | `8a2b0824` | Not observed — outside the 12-run window |
  | `7095b393` | Not observed — outside the 12-run window |
  | `d8932b57` | Not observed — outside the 12-run window |
  | `3e56203a` | Not observed — outside the 12-run window |
  | `b5ef5bd6` | Not observed — outside the 12-run window |
  | `1c9f44f1` | Not observed — outside the 12-run window |
  | `090bd17e` | Not observed — outside the 12-run window |
  | `21cf30f8` | [In progress](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35094235056) |
  | `401fa055` | [In progress](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35096337712) |
  | `d679f8e8` | [In progress](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35098269621) |

## Next decision or action

1. PR #41 remains the other lane's. Its handoff's release decisions are
   unchanged: seven reachable-history scan findings, unavailable pinned
   testnet archive state, and live activation/rollback. Resolve or explicitly
   disposition the two candidate-only P1s (Arc route activation; FastPay
   certificate acceptance) before live activation. This lane proposed
   minimal changes in the review and changed nothing on the branch.
2. The candidate owner decides whether today's timing fix and twelve P3
   repairs join this release or the next. The four consensus-affecting P3
   repairs would change signed or hashed bytes if merged. Burn-five repairs
   also remain main-only and undeployed, with consensus impact listed above.
3. Rerun the whole `postfiat-node` binary target 30 times under load to close
   the gate; investigate
   `fastswap_service::tests::replacement_relayer_recovers_expired_partial_prepare_via_rpc_evidence`
   with the same determinism approach as the transport tests.
4. Campaign candidates: burn-five P3s; SMG-07 after the candidate merges;
   a sixth burn of remaining node files, including candidate-excluded
   `rpc_dispatch.rs`, `transport_protocol.rs`, `block_replay_wallet.rs`
   after merge; bridge, Orchard and program crates once the other lane's
   rework lands. Tighten or split inventory wording before its next extension.
5. Unchanged: StakeHub #8 findings, the whitepaper abstract rule, and inventory
   rows needing an operator decision or a named live environment.

## References

- [Other lane's September 16 handoff, candidate branch at 15126ac3](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/handoffs/2026-09-16___codex__combined_release_check_kickoff_and_private_funding.md),
  [candidate review](../review/pr41-release-candidate-review-20260916.md),
  [previous handoff](2026-09-15___dravlic__burn_four_closed_and_rpc_health_probe.md).
- [P3 sweep and burn-four log](../review/qa-campaign-20260915.md),
  [burn-five brief](../review/qa-campaign-20260916-burn5-brief.md),
  [burn-five log and review index](../review/qa-campaign-20260916.md),
  [defect inventory](../review/defect-inventory-20260910.md).
- Burn-five surface closeouts:
  [6888b4e1](https://github.com/postfiatorg/postfiatl1v2/commit/6888b4e1),
  [204fd33f](https://github.com/postfiatorg/postfiatl1v2/commit/204fd33f),
  [41bd85ff](https://github.com/postfiatorg/postfiatl1v2/commit/41bd85ff),
  [3bfcbfed](https://github.com/postfiatorg/postfiatl1v2/commit/3bfcbfed),
  [330d38be](https://github.com/postfiatorg/postfiatl1v2/commit/330d38be).
  Findings and repair commits are linked beside each surface above.
