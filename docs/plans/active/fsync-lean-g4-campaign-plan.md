# Fsync-Lean G4 Campaign: Executable Plan

**Status:** Authorized — one run under the operator's standing 2026-08-29 directive to unblock the testnet path
**Date:** 2026-08-29
**Predecessors:** [Hygiene-corrected G4 plan](hygiene-corrected-g4-campaign-plan.md) (closed, failed on ratios 1.1149/1.1009 with every gate passing), [Batched-index G4 plan](batched-index-g4-campaign-plan.md) (closed, failed on one 8.4 ms residual)

## BLUF

The hygiene-corrected run completed the full 5+5+5 matrix for the first time
and passed every gate except the two ratio thresholds, missing by 1.5% and
0.09%. Its data localizes the entire remaining height cost to validator-0's
at-cap certified-send resume (~26 → ~67 ms p95), which under concurrent
six-validator I/O pays ~14 fsyncs per batch — five redundant per-job retention
disposal syncs and one duplicated completed-directory sync — plus
pretty-printed index encoding.

Candidate `66f30f13` removes exactly that: one retention-disposal sync per
batch, deduplicated directory syncs (14 → 9 fsyncs), and compact index
encoding (~687 → ~603 KB). Crash-ordering semantics are unchanged: intent
durable before moves, moves durable before the covering index write, index
durable before intent clear, and a lost disposal is re-cleaned by the next
resume's retention cleanup exactly as before. All 17/37/17/2 focused suites
and both release spot checks pass at the freeze.

Same matrix, same gates, same runner and inputs, one run, no retry.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `66f30f13f4b22033cb369031d05f5c246239c9ef` (`main`) |
| Candidate binary | SHA-256 `b357442939eb475d76ba45c3dc67a62c71d55578703853de3e4bbcb5cfeaea40`; 52,064,104 bytes; zero-warning release build |
| G1 candidate manifest | SHA-256 `9f1f80d6ddd195404d581f594a5fc493174c37056cab752de50f29addf4879f6` |
| G2 safety manifest | SHA-256 `ef410442aef58408e43fb4da2ba270e8294284cb1f78dce5988f5868742c1e0a`; rollback PASS (ancestor `0ac4e190`), tamper/crash PASS |
| Runner/verifier | `a3c7bea9` (unchanged); helper `cd43dd98…047a` (unchanged) |
| Prepared-input manifest | SHA-256 `0d22cc070420f927181426d6b6e550a8676d4d1da7005580098f7cb075a69466`; derived from the unchanged `ae658441` fleets |
| Output directory | `~/repos/postfiat-storage-g4-measurement-a3c7bea9-66f30f13-v1` (private) |

## Hygiene controls (inherited)

≥55 GB free disk before initialization; zero operator host activity between
campaign start and process exit.

## Outcome handling

**Pass:** milestone G4 PASS; proceed to G3 height-915 replay against
`b3574429…ea40`; assemble locally available G5 material; height-924 remains
the recorded external blocker. **Fail:** one diagnosis, closure docs, stop —
further attempts need new explicit operator input.
