# Cobalt Implementation

## Component Map

| Component | Source | Responsibility |
| --- | --- | --- |
| Trust and agreement | `crates/consensus_cobalt` | Trust views, linkedness, safety cover, RBC, ABBA, MVBA, and DABC. |
| Shadow service | `crates/node/src/cobalt_shadow.rs` | Durable signed transport, randomness, restart/replay safety, and advisory convergence. |
| Authority handoff | `crates/node/src/cobalt_handoff.rs` | Versioned Foundation-to-Cobalt transition and Cobalt validator-trust authorization. |
| Consensus admission | `crates/node/src/consensus_artifacts.rs` | Existing governance batch ID, signature, and consensus-ordering boundary. |
| Committed state | `crates/node/src/state_commitment.rs` | Canonical transition and Cobalt authorization commitments. |
| CLI | `python/postfiat_rpc/cobalt.py` | Human-readable and JSON inspection of the real Rust examples and shadow binary. |
| Browser UI | `python/postfiat_rpc/cobalt_ui.py` | Read-only aggregation of CLI checks, validated node state, and signed shadow status. |

## Authority Handoff

`CobaltGovernanceAuthorityTransitionV1` binds one transition to:

- chain and genesis domain;
- source and destination authority modes;
- old registry root, Cobalt registry root, trust-graph root, and Cobalt lock;
- previous transition, amendment sequence, activation height, and protocol
  version;
- validator-trust-only scope, active validators, quorum, and distinct
  ML-DSA-65 approvals.

The active registry signs the exact transition. Existing consensus v2 orders
the governance batch and the normal execution path commits it. After handoff,
each Cobalt-authorized validator update must bind the active transition, parent
lock, next sequence, and proposal slot. Mixed Foundation/Cobalt authorization,
new-set self-authorization, replay, stale parents, and non-forward rollback are
rejected.

## Shadow Service

`postfiat-cobalt-shadow` persists a mode-0600 ML-DSA-65 signer and signed public
state. Its queues, peers, seen-message set, and randomness history are bounded.
The adversarial drill covers restart recovery, duplicate delivery,
equivocation, bad signatures, partition healing, censorship healing, member
loss, and randomness failure. Every status explicitly reports
`shadow-advisory`, `live_authority=false`, and
`controls_block_consensus=false`.

```bash
cargo run -p postfiat-node --bin postfiat-cobalt-shadow -- \
  drill --data-dir /path/to/shadow-fleet
```

## Browser Data Boundary

The browser service does not read arbitrary display fixtures:

- trust and safety-witness panels execute the same functions used by
  `postfiat_rpc.cobalt`;
- the proposal panel runs the Rust node `status` command to validate the node
  store, requires `governance.json` to remain byte-identical across that check,
  and then parses its bounded `pftmac1` payload;
- convergence calls the Rust shadow binary's `status` action for every
  persisted validator directory;
- activation is `HOLD` unless trust and witness checks pass, the shadow fleet
  converges, and node state records the old-registry handoff plus Cobalt mode.

Only GET and HEAD are implemented. POST returns `405 Method Not Allowed`.

## Verification

```bash
PYTHONPATH=python python3 -m unittest -q \
  python.tests.test_cobalt python.tests.test_cobalt_ui

cargo test -p postfiat-node --lib cobalt_handoff
cargo test -p postfiat-consensus-cobalt
cargo clippy -p postfiat-types -p postfiat-consensus-cobalt \
  -p postfiat-node --lib -- -D warnings
```

The checked browser evidence uses a freshly initialized four-validator node and
the real four-validator adversarial shadow drill. It truthfully shows Foundation
authority, zero recorded proposals, converged shadow state, and activation on
`HOLD` because that node has no ordered handoff.

- [Desktop capture](../assets/cobalt-governance-observatory.png)
- [Mobile capture](../assets/cobalt-governance-observatory-mobile.png)
