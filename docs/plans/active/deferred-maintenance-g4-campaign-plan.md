# Deferred-Maintenance G4 Campaign: Executable Plan

**Status:** Authorized — one run under the operator's approved option 1 (2026-08-30 "yes"): remove the at-cap retention cost from the client-visible span at its source
**Date:** 2026-08-30
**Predecessor:** [Stall-tolerant G4 plan](stall-tolerant-g4-campaign-plan.md) (closed, failed on ratios alone by 0.3%/0.6% with every gate passing)

## The change, stated plainly

Certified-send completed-index maintenance — one-time migration, intent
reconciliation, compaction, and pruning — now runs **after client-visible
finality** inside the same measured round, instead of before proposal. The
pre-proposal step still scans the outbox and refuses to propose with pending
jobs (safety unchanged). Rationale: four campaigns proved the median round is
height-flat (p50 ratio 1.029) and the entire p95 gap is validator-0's at-cap
retention bookkeeping (~56 ms of fsyncs and index I/O), which does not gate
proposal or finality of the round it runs in. Deferring it is the same work,
same crash-safety (same lock, intents, and recovery), same telemetry shape —
executed where it belongs, off the finality-critical path. This follows the
existing structure: `post_apply_status_ms` already runs after the
client-visible span.

Gate integrity:

- `outbox_resume_ms` (a coverage stage) now sums the pending scan and the
  post-finality maintenance, so the work remains fully timed and
  `total_ms` coverage still accounts for it — nothing hides.
- All certified-send work counters and the migration-position rule report per
  round exactly as before; the runner is unchanged from `2e49abb2`.
- `consensus_round_ms` / `wallet_to_finality_ms` remain the runner's
  client-visible finality measurement; the 1.10 thresholds are untouched.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `d364430a6bf0d1199eaa55880d386aed337a9a05` (`main`) |
| Candidate binary | SHA-256 `b75dfaa932f37e2dec0b0b9e25223563c18b8527831d9a5c7a60e70d312c805c`; 52,065,832 bytes; zero-warning release build |
| G1 candidate manifest | SHA-256 `9c7112ce1adbd3d54f7a7ca4069ab4223cff11dc8780f1794c786f225888c570` |
| G2 safety manifest | SHA-256 `76c32271f369b7b3e8558c06a9b6b090556c5133e49b9083fc6e4a4d0732a491`; rollback PASS (six-validator end-to-end rounds on this binary), tamper/crash PASS |
| Runner/verifier | `2e49abb2` (unchanged); helper `8eaa3188…4b4d` (unchanged) |
| Prepared-input manifest | SHA-256 `44360af121ac286a309652990d7f9ceea6a70e530da8ce512dc3ef38ba6cc83a` |
| Output directory | `~/repos/postfiat-storage-g4-measurement-2e49abb2-d364430a-v1` (private) |

Hygiene controls inherited (92 GB free at start; zero operator host activity
during measurement). Matrix, gates, and one-run/no-retry rules unchanged.

**Fail:** one diagnosis, closure docs, stop; further attempts need new
operator input.
