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

Consensus v2 remains the sole owner of block finality. Cobalt became the live
controlled-testnet authority for validator-registry and trust-graph changes at
height 916. The sidecar remains an advisory protocol/runtime helper; authority is
recorded and enforced by the consensus-ordered governance state.

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
a disposable clone. After that release-qualified rehearsal, the live transition
committed at height 916 and a Cobalt-authorized validator-5 key rotation committed
at height 917. All six validators converged at height 919 after two timeout
certificates and a valid view-2 round. The active registry root is
`945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e`.

Commands that write validator safety state, including timeout votes and proposal
rounds, must run as the validator service user. Key staging and release operations
must preserve ownership of the validator data directory. A root-owned safety file
can prevent the service account from signing the next view even when protocol
state is otherwise valid.

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

`scenario` reports the frozen comparison cases, pass counts, conflicts,
deterministic replay, native RippleD fork control, safe halts, and methodology
boundary. `readiness` authenticates the disposable rehearsal. `live-status` reads
the persisted governance state, runs the Rust non-uniform verifier, checks the
active transition and registry update, and reports the terminal `ACTIVATE /
ACTIVATED` result. Explicit release-binary paths make `live-status` usable without
a source checkout.

## Browser Data Boundary

The browser has two bounded input modes. The repository mode reads shadow health,
checksum-pinned comparison/rehearsal evidence, and integrity-framed node governance
state. The installed mode reads the authenticated `live-status` receipt produced
by the same CLI used for operator verification. Both display Cobalt as the actual
validator-trust authority only when the persisted state and Rust verifier agree.

The HTTP server exposes static assets and `/api/snapshot` through GET and HEAD.
POST returns 405. Security headers include a self-only content policy, frame
denial, MIME sniffing denial, and no-referrer policy. The interface exposes no
governance mutation route.

## Verification

```bash
PYTHONPATH=python python3 -m unittest -q \
  python.tests.test_cobalt python.tests.test_cobalt_ui

PYTHONPATH=python python3 -m postfiat_rpc.cobalt scenario --json
PYTHONPATH=python python3 -m postfiat_rpc.cobalt readiness --json
PYTHONPATH=python python3 -m postfiat_rpc.cobalt live-status --json \
  --node-data-dir /var/lib/postfiat/validator-0 \
  --node-bin /opt/postfiat/releases/cobalt-live-governance-audit-05507758/postfiat-node \
  --shadow-bin /opt/postfiat/releases/cobalt-shadow-registry-reset-43ac8a7d/postfiat-cobalt-shadow \
  --shadow-data-dir /var/lib/postfiat-cobalt-shadow
python3 benchmarks/cobalt-activation-live/packet/verify_packet.py

cargo test -p postfiat-node cobalt_handoff --locked
cargo test -p postfiat-consensus-cobalt --locked
cargo fmt --all -- --check
```

The [live activation packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-activation-live/packet)
records the terminal authority state, Cobalt-authorized registry update, six-node
convergence, view-change recovery, complete 919-block replay, CLI/UI output,
read-only HTTP behavior, source pins, prior packet roots, and redaction checks.
The result proves controlled-testnet protocol capability. It does not claim
independent human operators or operational decentralization.
