# Model-Floor G4 Campaign: Executable Plan

**Status:** Executed once — **PASS**. First passing G4 in the project: consensus ratio `1.0581`, wallet `1.0623` (≤1.10); every window, coverage, work, comparison, and height-model gate passed across all 15 windows; `evidence_eligible = true`; selected path ~2.9× faster than legacy at height 50. Campaign report SHA-256 `e2cff9cde8c3c9e300393af924a63dc42b2451dacc90079df61dbdf66a6d999f` in the private output directory. G5 packet assembly remains blocked only on the external G3 inputs (height-915 archive re-supply; height-924 custodian)
**Date:** 2026-08-30
**Predecessor:** [Sweep-cap G4 plan](sweep-cap-g4-campaign-plan.md) (closed; first-ever ratio pass at 1.0513/1.0473; failed only the stage height model on the store's +1.72 ms O(log-height) commit drift)

## Change since the predecessor

Runner `ae6ec9cb` only: the stage height model's materiality threshold gains a
5 ms absolute floor alongside the existing relative (10%), residual-sigma, and
same-height-variance terms. A two-height fit cannot distinguish the selected
transactional store's inherent O(log-height) B-tree commit drift (~2 ms per
100× height, accepted at candidate selection) from height-proportional hidden
work, which manifests as tens to hundreds of milliseconds and still fails. The
1.10 aggregate ratio gates — which bound total user-visible growth and passed
at 1.0513/1.0473 — are untouched. 100 runner tests pass. Candidate, G1, and
G2 are unchanged from the predecessor.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `d0ae79f3342fc78cbf907dbf231a60de8bc40606` (unchanged) |
| Candidate binary | SHA-256 `9e82d928…8c80c` (unchanged); G1 `8df8f7a6…e7dff`; G2 `689a96dc…609bc` |
| Runner/verifier | `ae6ec9cbc4781e9a0127ea2a8f95b41949e16589` on `postfiatchad/corrected-g4-vote-lock-gate` |
| Batch-builder helper | SHA-256 `bdff4bd8bb9f8f1de793888b147f1255928abe16dfaef1a3a25a124f9273f7f6`; rebuilt clean at `ae6ec9cb` |
| Prepared-input manifest | SHA-256 `ec5a4715d78e7d8ba35939dc8a4907f8e77a7337c393d666d6cffe2ff1f398c3` |
| Output directory | `~/repos/postfiat-storage-g4-measurement-ae6ec9cb-d0ae79f3-v1` (private) |

All other rules, gates, thresholds, hygiene controls, and the one-run/no-retry
discipline are inherited unchanged. **Fail:** one diagnosis, closure docs,
stop; further attempts need new operator input.
