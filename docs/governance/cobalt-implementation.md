# Cobalt Implementation

## Boundaries

| Component | Source | Responsibility |
| --- | --- | --- |
| Trust and agreement | `crates/consensus_cobalt` | Trust views, strong support, RBC, ABBA, MVBA, and DABC. |
| Shadow state and history | `crates/node/src/cobalt_shadow.rs` | Signed transport, durable locks, parent-linked ratifications, replay, and catch-up. |
| Shadow runtime | `crates/node/src/cobalt_shadow_runtime.rs` | Bounded probe, snapshot, replay, history-range, verification, and catch-up operations. |
| Authority handoff | `crates/node/src/cobalt_handoff.rs` | Foundation/Cobalt transitions and validator-trust-only authorization. |
| Disposable rehearsal | `crates/node/src/cobalt_handoff_rehearsal.rs` | Exercises production handoff checks on a cloned state; cannot address live node storage. |
| CLI | `python/postfiat_rpc/cobalt.py` | Human and JSON inspection plus checksum-pinned evidence decisions. |
| Browser UI | `python/postfiat_rpc/cobalt_ui.py` | Read-only view over the CLI decision, validated node state, and signed shadow status. |

Consensus v2 remains the sole owner of block finality. Cobalt output is advisory
unless a valid transition is explicitly authorized and ordered.

## Agreement and Recovery

The live shadow protocol accepts sorted, unique, registered signers only. It
uses the trust-view support rules in `consensus_cobalt`; the node does not add a
second integer quorum. On the current graph, any valid five-of-six certificate
can ratify, every four-of-six set fails, and different valid five-signer audit
certificates resolve to the same decision and ratification identity.

Each accepted ratification binds its previous durable parent. Signer-safety
high-water marks are separate from contiguous history: receiving N+1 without N
returns `catch_up_required` and does not commit N+1. Catch-up imports a bounded
range of signed transcripts and independently verifies domains, registry and
graph roots, ML-DSA signatures, support, parents, sequences, slots, and sizes
before atomic advancement.

## Authority Handoff

`CobaltGovernanceAuthorityTransitionV1` binds:

- chain, genesis, and protocol domains;
- source and destination authority modes;
- old and Cobalt registry roots, trust-graph root, and Cobalt lock;
- previous transition, amendment sequence, activation height, and scope;
- active validators, quorum, and distinct current-registry ML-DSA-65 approvals.

The old registry signs the transition and Consensus v2 orders it at the exact
activation height. Mixed authority, stale or replayed transitions, wrong roots,
new-set self-authorization, and non-forward rollback fail closed. After handoff,
Cobalt accepts exactly validator-trust updates; unrelated governance remains
rejected.

The rehearsal runner uses the production verification and application paths on
a disposable clone. The committed packet proves activation, negative cases,
pre-activation abort, one scoped update, and forward rollback without changing
the live fleet.

## Authenticated Decision Output

`scenario` and `readiness` do not trust arbitrary JSON summaries. They:

1. bound the packet and checksum-manifest sizes;
2. reject symlinks, malformed entries, duplicates, absolute paths, and path
   traversal;
3. verify every file against a pinned `SHA256SUMS` root;
4. require the expected verifier schema, a `passed` result, and every mandatory
   check;
5. pin the handoff verifier separately because that packet intentionally omits
   `verifier.json` from its checksum manifest.

`scenario` reports the 80 matched cases, pass counts, conflicts, deterministic
replay, native RippleD fork control, safe halts, and methodology boundary.
`readiness` adds the disposable handoff and returns either `GO` for a later,
separately authorized controlled-testnet cutover or `HOLD`. It always reports
actual observed authority separately and never performs activation.

## Browser Data Boundary

The browser collector reads three independent state classes:

- **Shadow health** comes from the Rust shadow status action for every persisted
  validator directory.
- **Rehearsal readiness** is the exact `readiness_result` used by the Python CLI,
  backed by the pinned benchmark and handoff packets.
- **Actual authority** comes from the node's bounded, integrity-framed
  `governance.json`. The collector runs Rust node `status` and requires that file
  to remain byte-identical across the validated read.

This separation prevents “ready to cut over” from being mistaken for “already
active.” The HTTP server exposes static assets and `/api/snapshot` through GET
and HEAD. POST returns 405. Security headers include a self-only content policy,
frame denial, MIME sniffing denial, and no-referrer policy.

## Verification

```bash
PYTHONPATH=python python3 -m unittest -q \
  python.tests.test_cobalt python.tests.test_cobalt_ui

PYTHONPATH=python python3 -m postfiat_rpc.cobalt scenario --json
PYTHONPATH=python python3 -m postfiat_rpc.cobalt readiness --json

cargo test -p postfiat-node cobalt_handoff --locked
cargo test -p postfiat-consensus-cobalt --locked
cargo fmt --all -- --check
```

The final decision packet records exact source commits, CLI/UI outputs, packet
roots, tests, read-only HTTP behavior, and a no-live-mutation receipt. A live
cutover remains separate Task Node-governed work requiring explicit user
authorization.
