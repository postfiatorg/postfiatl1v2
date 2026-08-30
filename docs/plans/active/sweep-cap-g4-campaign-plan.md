# Sweep-Cap G4 Campaign: Executable Plan

**Status:** Authorized — one run, continuing the operator's approved option 1
**Date:** 2026-08-30
**Predecessor:** [Deferred-maintenance G4 plan](deferred-maintenance-g4-campaign-plan.md) (closed, failed on the ≤5-compactions-per-resume telemetry contract; fixed candidate-side)

## Change since the predecessor

Candidate `d0ae79f3` bounds each post-finality maintenance pass to five
compactions (oldest first); a first pass after restore no longer sweeps
leftovers plus fresh deliveries in one report. Steady state remains five per
round with a one-round lag; the frozen runner's certified-send work gate
(validated == compacted + pruned, ≤5 each per resume) holds exactly. New
fixture `maintenance_sweeps_at_most_five_jobs_per_pass`; 19/39/17/2 suites,
rollback, and tamper/crash all PASS at the freeze.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `d0ae79f3342fc78cbf907dbf231a60de8bc40606` (`main`) |
| Candidate binary | SHA-256 `9e82d9286d79307b6246a773882e744ade1abad6b10498c3ed2d9c9e6b78c80c`; 52,066,024 bytes |
| G1 candidate manifest | SHA-256 `8df8f7a65471ec9ebd43975ace581c7a32fb0ccb79e9e6bc7321b6c4a0fe7dff` |
| G2 safety manifest | SHA-256 `689a96dcfaa99887a147df05d7dc3d9658e6e048048333b380359eaef72609bc` |
| Runner/verifier | `2e49abb2` (unchanged); helper `8eaa3188…4b4d` (unchanged) |
| Prepared-input manifest | SHA-256 `a94786c864a1b9a201ac61b777c944ff05aebf574423b294d301be6e066a5bb4` |
| Output directory | `~/repos/postfiat-storage-g4-measurement-2e49abb2-d0ae79f3-v1` (private) |

All other rules, gates, thresholds, hygiene controls, and the one-run/no-retry
discipline are inherited unchanged. **Fail:** one diagnosis, closure docs,
stop; further attempts need new operator input.
