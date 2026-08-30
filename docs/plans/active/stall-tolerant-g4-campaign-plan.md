# Stall-Tolerant G4 Campaign: Executable Plan

**Status:** Executed once — **FAIL on ratios alone, by 0.3%/0.6%** (consensus `1.1028`, wallet `1.1061` vs ≤1.10). All 15 windows completed; every window, coverage (amended), work, comparison, and height-model gate passed. Diagnosis: the round p50 ratio is height-flat (1.029); p95 deterministically lands on validator-0's at-cap resume (~56 ms vs ~24 ms at height 50), whose cost is the certified-send index file's own fsyncs on this virtio disk. No further run without new explicit operator input; see the closure section of the [environment decision handoff](../../handoffs/2026-08-29___postfiatchad__g4_measurement_environment_decision.md)
**Date:** 2026-08-30
**Predecessors:** [Fsync-lean G4 plan](fsync-lean-g4-campaign-plan.md) (closed, failed on one isolated 103.6 ms host stall), [environment decision handoff](../../handoffs/2026-08-29___postfiatchad__g4_measurement_environment_decision.md)

## What changed and why it is legitimate

1. **Candidate `1423770e`:** one further fsync removed — the intent-clear
   directory sync is skipped for fully-applied batch intents, because
   replaying a fully-applied batch is idempotent (appends see indexed
   destinations, prunes see absent sources). At-cap batch: ~8 fsyncs.
   Retention-disposal deferral was evaluated and rejected: the retention
   cleanup bound (1 entry) makes it a net-zero fsync trade. All 17/37/17/2
   focused suites pass; G2 rollback and tamper/crash refreshed and PASS.
2. **Runner `2e49abb2`** (gate-logic change, reviewed and operator-approved):
   the round-coverage gate now tolerates **at most one** residual round per
   window in (100, 250] ms; a second stall round, any round over 250 ms, or
   any systematic pattern still fails, and tolerated rounds are listed in the
   receipt. Rationale: two of three campaigns died on single isolated 3.6 ms
   and 8.4 ms overshoots from KVM scheduler steal on different proposers with
   normal stages, while one campaign ran 750/750 rounds clean — proving the
   candidate hides no systematic stage. A stalled round's full latency still
   feeds the untouched 1.10 p95 ratio gates. 99 runner tests pass, including
   three new stall-adjudication fixtures.
3. **Ratio thresholds are unchanged.** If the ratios fail on the merits, the
   result is a final FAIL and further attempts need new operator input.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `1423770edd017f08ad4eeda51a35801bc285417d` (`main`) |
| Candidate binary | SHA-256 `15ceeb9d17702d26727a7914acf70ea7da2a104a40c8b16508d9bd166bf1c96e`; 52,064,680 bytes; zero-warning release build |
| G1 candidate manifest | SHA-256 `cf55a45fbf20ccffc2506149b87570986f614886a801142bf696b9b8b4952dba` |
| G2 safety manifest | SHA-256 `2412bb702e429e65d20e8a46958a45fc0e8889c06a413403e5917e237a95514a`; rollback PASS (ancestor `0ac4e190`), tamper/crash PASS |
| Runner/verifier | `2e49abb21c915505b418702222ac86f31e5d0f54` on `postfiatchad/corrected-g4-vote-lock-gate`; logic parent `a3c7bea9` |
| Batch-builder helper | SHA-256 `8eaa318848c9ed0c4fb591280a184e6b40eaae44a46cc11ed03094ce953d4b4d`; rebuilt clean at `2e49abb2` |
| Prepared-input manifest | SHA-256 `b985e6a93f604d0513897944a429fb1d637d1522f0ba1a82b836bddb9e32505f`; derived from the unchanged `ae658441` fleets |
| Output directory | `~/repos/postfiat-storage-g4-measurement-2e49abb2-1423770e-v1` (private) |

Hygiene controls inherited: ≥55 GB free (97 GB at start), zero operator host
activity during measurement. All other hard rules, the 5+5+5 matrix, and the
gate table are unchanged from the predecessor plans.

## Outcome handling

**Pass:** milestone G4 PASS; G3 height-915 replay remains blocked on
re-supplying its quarantine-archive input (not on this host) and height-924 on
a named custodian; G5 assembles what is locally available. **Fail:** one
diagnosis, closure docs, stop.
