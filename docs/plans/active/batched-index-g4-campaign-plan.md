# Batched-Index G4 Qualification Campaign: Executable Plan

**Status:** Authorized — one run; operator directed "get the testnet unblocked" on 2026-08-29 and this plan records that as the single run authorization
**Date:** 2026-08-29
**Baseline:** `main` at `86929450` (frozen candidate); predecessor failure closed at `db595bf2`
**Predecessors:** [Remediated G4 plan](remediated-g4-qualification-campaign-plan.md) (closed, failed),
[Vote-lock marker and batched index fixes handoff](../../handoffs/2026-08-29___postfiatchad__vote_lock_marker_and_batched_index_fixes.md),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)

## BLUF

Fourth G4 attempt, first with all four defect classes fixed and fixture-covered:
`redb` bounded work, vote-lock index scan, certified-send tombstone resume, and
now the vote-lock empty-directory marker plus the batched completed-index
mutations that caused the 1.40 height ratios. Unchanged 5+5+5 matrix, unchanged
gates, unchanged runner logic. One run, no retry, diagnose-once on failure.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `86929450992a64d1be1fb98cfde6aa46143c4568` (`main`; includes `ff2b3532`, `48a94425`) |
| Candidate binary | SHA-256 `d27fc062e1248275e39b922a0da175f0c8e4cb7218023a2e9885ffcb9911112c`; 52,064,552 bytes; zero-warning release build |
| G1 candidate manifest | SHA-256 `5913d29650318fa467c35184ea340ce0888b40ba90ca00bca51533689d65c2ca` |
| G2 safety manifest | SHA-256 `76a750832bab4fce68b057a31330715c275ff97283504731d85d649bf45be2bf`; rollback PASS (ancestor `0ac4e190`), tamper/crash PASS (37 owner tests, coverage complete) |
| Runner/verifier | `a3c7bea9285ab02871fd2111038764c6174b905b` on `postfiatchad/corrected-g4-vote-lock-gate`; gate logic parent `15d059d1`; clean |
| Batch-builder helper | SHA-256 `cd43dd98549cfb1651ee2295b8ba6d8364dcaea4ff10e40c7203acf8a53e047a`; rebuilt clean at `a3c7bea9` |
| Prepared-input manifest | SHA-256 `78206feaa7adb2dd3cc235545de748c0de2194b915470f9a71ecf5ee7b665cad`; derived from the unchanged `ae658441` build with `prepared_by` provenance preserved |
| Output directory | `~/repos/postfiat-storage-g4-measurement-a3c7bea9-86929450-v1` (private; never commit or publish) |

## Hard rules (inherited, unchanged)

1. Preflight and verification outside the four-hour measurement clock.
2. Unchanged matrix: 5 selected-redb windows at height 50, 5 at height 5,000,
   5 legacy-JSONL controls at height 50; 50 rounds per window; six validators.
3. One run. Diagnose once on failure from the campaign's own artifacts; no
   retry, no matrix change, no relabeling; partial output is never a packet.
4. Offline only: no devnet, deployment, height-924 copy, or live-fleet claims.
5. Round success = valid certificate + literal receipts + six-validator
   convergence; never elapsed time alone.
6. Both migration allowances fire per-validator on first use, now including
   zero-work empty-state migrations on both the vote-lock and certified-send
   surfaces.

## Gate table (thresholds unchanged)

| Gate | Threshold |
| --- | --- |
| Height-5,000/height-50 `consensus_round_ms` p95 ratio | ≤ 1.10 |
| Height-5,000/height-50 `wallet_to_finality_ms` p95 ratio | ≤ 1.10 |
| Named synchronous-stage height model | No material positive relationship |
| Round-coverage residual | < 100 ms every measured round |
| Selected/legacy height-50 comparison | All five legacy windows complete |
| Correctness, convergence, bounded `redb` work, zero full-history reads | Every round |
| Vote-lock and certified-send bounded-work and migration-position gates | Every round |
| Four-hour measurement budget | Not exceeded |

## Outcome handling

**Pass:** update the milestone (G4 PASS with identities), proceed to G3
height-915 replay against binary `d27fc06…112c`, then G5 packet assembly of
locally available material. Height-924 replay remains blocked on a named
custodian and read-only-copy authorization — record it as the remaining
external blocker. No deployment claim.

**Fail:** one named-stage diagnosis, handoff, milestone update, stop.
