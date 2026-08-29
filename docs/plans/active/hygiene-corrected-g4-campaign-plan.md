# Hygiene-Corrected G4 Campaign: Executable Plan

**Status:** Authorized — one run under the operator's standing 2026-08-29 directive to unblock the testnet path
**Date:** 2026-08-29
**Predecessor:** [Batched-index G4 plan](batched-index-g4-campaign-plan.md) (closed, failed on a single 8.4 ms round-coverage residual overshoot; see the [residual failure handoff](../../handoffs/2026-08-29___postfiatchad__batched_index_g4_residual_failure.md))

## BLUF

Identical campaign to the predecessor — same frozen candidate `86929450`,
binary `d27fc062…112c`, runner `a3c7bea9`, helper `cd43dd98…047a`, prepared
input `78206fea…5cad`, same matrix, same gates, one run, no retry — with two
added operator hygiene controls that the predecessor's diagnosis named as the
failure's environmental factors:

1. **Disk headroom:** at least 55 GB free before initialization, so occupancy
   stays below ~90% through the run.
2. **Quiet host:** zero operator commands on the measurement host between
   campaign start and exit; progress is observed only by waiting on the
   campaign process itself.

No candidate, runner, gate, threshold, or input identity changes. The
predecessor's diagnostic ratios (1.0926 / 1.0869) predict a pass with ~1%
margin; residual jitter is the known risk and is exactly what the hygiene
controls target. On failure: one diagnosis, handoff, milestone update, stop —
and any further attempt requires new explicit operator input.

## Outcome handling

**Pass:** milestone G4 PASS with identities; proceed to G3 height-915 replay
against binary `d27fc062…112c`; then assemble locally available G5 material.
Height-924 replay remains blocked on a named custodian and read-only-copy
authorization. No deployment claim.

**Fail:** one named diagnosis, closure docs, stop.
