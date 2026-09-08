# UNL amendment V2 paired gate

This directory is the Section C, offline-only shadow experiment. It consumes the
Section A evidence types and roots and the Section B frozen graph/admission
policy. It never writes to, regenerates, or reinterprets the frozen V1 baseline
in `../tasknode-unl-attack-simulation-20260907/`.

The core preregistration was locked at
`94bc0979d95dbbfcb60b31ddb28fdc3ae5f96d284eb3b62438ff166083eb1206`
before the first trial. Before running the additional dimensions stated
explicitly in amendment §5, the immutable extension was locked at
`180698bd9930351f6eb23f9079296cb5fd5f3b8e765412a3c2b82c84c9eb99b9`.
The driver verifies both digests and every V1 artifact hash at startup.

Run from the repository root:

```bash
python3 benchmarks/ai-governance/tasknode-unl-v2-gate-20260908/run_gate.py
```

The committed `trial-history.json` records every executed build, including the
core-only run and the reporting-label correction; no result or parameter was
silently discarded. The committed `outputs/` contain V2 results, attack audit
records, four acknowledgement-availability cells, the 30 original low/base/high cells, all 27
damping/steps/floor cells, 30 published seeded topologies over three windows,
and a manifest. The driver constructs two normal results and one reordered-input
result and refuses to write outputs unless their canonical bytes match.

The paired consent-complete fixture reports 14/14 honest controls admissible.
Unsolicited funding is audit-only and invariant; declared changes and recovery
hold until post-epoch evidence is complete. Hidden unchanged-key changes and
undeclared common funding remain explicitly unresolved public-input limits.
Accepted seed-connected relations can admit attacker-controlled accounts within
the tested budget. The Section C gate therefore passes its preregistered
regression conditions but remains shadow-only, as the amendment requires.
