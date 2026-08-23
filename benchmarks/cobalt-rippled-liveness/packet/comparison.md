# Matched Cobalt/RippleD Liveness Comparison

## Result

Both adapters consumed the same canonical 80-case manifest and produced **80/80 declared outcomes with zero conflicting decisions**. The packet passes its methodology and operational checks. This is controlled-testnet benchmark evidence only; it does **not** activate Cobalt authority or authorize a live handoff.

## Safety, liveness, and recovery

- Cobalt: zero conflicting decisions; every committed case replayed identically; every case kept both authority flags false. Declared beyond-budget and correlated-loss cases halted safely rather than manufacturing progress.
- RippleD CSF: zero conflicting decisions; the same declared fault cases halted safely, while no-fault, one-fault, transport-fault, membership, key-rotation, partition-heal, and overlap-sweep cases completed.
- The upstream pinned `Consensus` suite, including `testFork`, passed with 13 cases and 1,370 elementary tests. RippleD's branch detector remains the native fork control.

## Quorum and overlap margin

Any single declared validator loss remained live in every topology. The smallest observed blocking loss was the declared budget plus one (two for live-six/control-7, three for control-10, five for control-20). No tested overlap, graph/list drift, partition, or membership transition produced a conflict. These are exact observed scenario margins, not an exhaustive subset-search claim.

`ValidatorList::calculateQuorum` is recorded as a local computation. It does not prove global UNL overlap. The overlap sweep is explicitly derived from the separately pinned AGTI downstream report and is not labeled as an upstream XRPLF test.

## Latency and resource interpretation

Cobalt reports cryptographic contribution/assembly/commit and RBC/ABBA/MVBA/DABC validation timings, modeled network stage time, signed messages, serialized transcript bytes, durable disk, RSS, and descriptors. RippleD reports its native virtual completion/recovery time, process wall/CPU time, in-memory message counters, RSS, and descriptors under upstream CSF. The transport and cryptographic models differ, so the packet reports both distributions without ranking one system's latency against the other and never compares Cobalt governance latency to XRPL payment latency.

## Methodology and evidence health

All source pins, manifest hash, adapter hashes, case order, statuses, replay/authority checks, and native fork control are verified in `verifier.json`. The earlier 49/80 Cobalt run and all build failures are disclosed as remediated preflight evidence rather than hidden. No methodology exception remains unresolved under the stated simulator-to-simulator scope.
