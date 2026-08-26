# Cobalt Governance

Cobalt is PostFiat's validator-trust governance lane. It does not order blocks,
finalize transactions, or replace Consensus v2.

## Plain English

PostFiat validators may have different local trust views. Cobalt lets them agree
on one ordered validator or trust-graph change only when every relevant trust
view has strong support. The current six-validator graph permits any valid five
to make progress and rejects every four. A validator that misses history must
verify and import the signed gap before it can advance.

Authority transfer is separate from agreement:

1. The Cobalt shadow services remain advisory and cannot mutate validator state.
2. The Foundation-administered registry signed the exact Cobalt transition with
   a distinct ML-DSA-65 quorum.
3. Consensus v2 ordered that transition at height 916.
4. The recorded authority state then allowed Cobalt to ratify validator-trust
   updates only; validator-5 key rotations committed at heights 917 and 924.
   Cobalt cannot authorize unrelated governance and a new validator set cannot
   authorize itself.
5. Returning to Foundation authority requires another signed forward transition;
   it cannot rewrite finalized history. The adversarial drill committed the
   final rollback/return pair at heights 922/923.

A Cobalt failure can pause validator governance. It cannot create a second
block-finality protocol.

## Current Controlled-Devnet State

The final authenticated E5 observation ran from
`2026-08-26T06:34:55Z` through `06:35:50Z` on
`postfiat-wan-devnet-2`. All six validators converged at height 924, Cobalt
was active for validator-trust governance, Consensus v2 remained block finality,
and all validator, RPC, and advisory shadow services were active. The final
signed rollback/return pair committed at heights 922/923 and the legitimate
validator-5 rotation committed at 924.

Every validator used node binary SHA-256 `d5e5ef63…c2696caf`; every shadow
used `d61e6d0f…b1a0a4a`. The observation is point-in-time evidence, not a
real-time query now.

See [Current State](../status/chain-state-current.md) for the exact roots,
runtime identities, repository state, campaign decision, and freshness
boundary. See the [adversarial results](cobalt-adversarial-verification-results.md)
for what was attacked, fixed, and left open. Current Git HEAD is not itself
deployment evidence.

Verify the qualification evidence directly:

```bash
PYTHONPATH=python python3 -m postfiat_rpc.cobalt scenario
PYTHONPATH=python python3 -m postfiat_rpc.cobalt readiness
```

`scenario` verifies the pinned 80-case matched Cobalt/RippleD packet before
reporting outcomes. `readiness` additionally verifies the disposable handoff,
negative cases, abort, forward rollback, scoped validator update, and
byte-identical live-fleet receipts. Both commands support `--json`.

## Operator Rules

Before scheduling any live authority transition, operators must rehearse the
exact transition and its surrounding sequence on a disposable clone bound to
the current chain, registry, authority history, and trust state. This rule
covers activation, rollback, and every return to Cobalt. A previous rehearsal
that ends at rollback does not qualify for a later rollback-then-return
sequence.

For a return to Cobalt, the clone rehearsal must construct the protocol-native
post-return trust graph, derive its trust root, bind the return transition to
that root, and verify the resulting authority history before any live action.
E5 is the reason for this standing rule: the accepted height-921 return used a
trust binding that did not match the protocol-native post-return graph, so the
signed corrective rollback/return at heights 922/923 was required. See
[what E5 fixed](cobalt-adversarial-verification-results.md#what-was-fixed) and
the [E5 evidence packet](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-adversarial-verification/e5/README.md).

## Operator CLI

The CLI uses the Rust owners and the signed shadow service; it does not
reimplement consensus rules:

```bash
# Trust model and local safety checks
PYTHONPATH=python python3 -m postfiat_rpc.cobalt graph
PYTHONPATH=python python3 -m postfiat_rpc.cobalt transition-witness
PYTHONPATH=python python3 -m postfiat_rpc.cobalt protocol-replay

# One running shadow service
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoint 127.0.0.1:9700 probe
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoint 127.0.0.1:9700 snapshot
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoint 127.0.0.1:9700 replay

# Whole fleet
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoints 127.0.0.1:9700,127.0.0.1:9701 fleet

# Completed adversarial campaign
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial

# Signed history recovery
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoint 127.0.0.1:9700 --start-sequence 1 \
  --output /tmp/cobalt-range.json history-export
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --endpoint 127.0.0.1:9701 --range /tmp/cobalt-range.json history-verify
PYTHONPATH=python python3 -m postfiat_rpc.cobalt \
  --source-endpoint 127.0.0.1:9700 \
  --target-endpoint 127.0.0.1:9701 --start-sequence 1 catch-up
```

A failed checksum, verifier, domain, root, signature, support certificate,
parent, sequence, size bound, or replay check is a failure—not partial success.

## Read-Only Browser Interface

```bash
PYTHONPATH=python python3 -m postfiat_rpc.cobalt_ui \
  --adversarial-packet benchmarks/cobalt-adversarial-verification/packet
```

Open the address printed by the process. The authenticated adversarial view deliberately separates:

- **Campaign gate:** all six experiment results and all nine named live
  rejections.
- **Proposal and authorization identities:** the final rollback, return, and
  legitimate rotation receipts.
- **Actual authority:** Cobalt's bounded validator-trust role, with Consensus v2
  shown separately as block finality.
- **Operator boundary:** protocol capability passed; operator decentralization
  is not claimed.

The service implements GET and HEAD only. POST returns `405 Method Not Allowed`.
It has no proposal, transition, registry-update, or activation route.

## Evidence

- Matched comparison: `benchmarks/cobalt-rippled-liveness/packet/`
- Disposable handoff: `benchmarks/cobalt-handoff-rehearsal/packet/`
- Pre-activation decision: `benchmarks/cobalt-activation-readiness/packet/`
- Terminal activation: `benchmarks/cobalt-activation-live/packet/`
- Completed adversarial packets: `benchmarks/cobalt-adversarial-verification/e1/` through `e6/`
- Consolidated adversarial packet: `benchmarks/cobalt-adversarial-verification/packet/`
- Results: [Cobalt Adversarial Verification Results](cobalt-adversarial-verification-results.md)
- Current state and freshness: [Current State](../status/chain-state-current.md)
- Implementation details: [Cobalt Implementation](cobalt-implementation.md)
- Validator lifecycle: [Validator Registry](validator-registry.md)
