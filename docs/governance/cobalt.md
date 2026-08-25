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
   updates only; a validator-5 key rotation committed at height 917. Cobalt
   cannot authorize unrelated governance and a new validator set cannot
   authorize itself.
5. Returning to Foundation authority requires another signed forward transition;
   it cannot rewrite finalized history.

A Cobalt failure can pause validator governance. It cannot create a second
block-finality protocol.

## Current Controlled-Devnet State

An authenticated read-only probe completed at `2026-08-25T15:37:40Z` and
returned **ACTIVATED** on `postfiat-wan-devnet-2`: all six validators were active
and equal at height 919, Cobalt was the validator-trust authority, and Consensus
v2 remained the only block-finality protocol. The active consensus service was
release `cobalt-activation-8694b99d` with binary SHA-256
`431f194b…783f4`. A separate governance-auditor binary verified the authority
history, and the Cobalt shadow services remained advisory.

See [Current State](../status/chain-state-current.md) for the exact roots, all
runtime identities, the activation packet's stale consensus-label discrepancy,
repository state, adversarial campaign, and freshness boundary. The earlier
benchmark and handoff packets are qualification provenance; current Git HEAD is
not deployed.

Verify the qualification evidence directly:

```bash
PYTHONPATH=python python3 -m postfiat_rpc.cobalt scenario
PYTHONPATH=python python3 -m postfiat_rpc.cobalt readiness
```

`scenario` verifies the pinned 80-case matched Cobalt/RippleD packet before
reporting outcomes. `readiness` additionally verifies the disposable handoff,
negative cases, abort, forward rollback, scoped validator update, and
byte-identical live-fleet receipts. Both commands support `--json`.

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
  --node-data-dir /path/to/node \
  --shadow-root /path/to/shadow-fleet
```

Open the address printed by the process. The page deliberately separates three
facts:

- **Shadow health:** current sidecar transport, signed history, convergence, and
  non-authoritative flags.
- **Cutover readiness:** the same checksum-pinned `readiness` result exposed by
  the CLI.
- **Actual authority:** the Foundation/Cobalt mode read from the validated node
  governance state, with Consensus v2 shown separately as block finality.

The service implements GET and HEAD only. POST returns `405 Method Not Allowed`.
It has no proposal, transition, registry-update, or activation route.

## Evidence

- Matched comparison: `benchmarks/cobalt-rippled-liveness/packet/`
- Disposable handoff: `benchmarks/cobalt-handoff-rehearsal/packet/`
- Pre-activation decision: `benchmarks/cobalt-activation-readiness/packet/`
- Terminal activation: `benchmarks/cobalt-activation-live/packet/`
- Post-activation adversarial E1: `benchmarks/cobalt-adversarial-verification/e1/`
- Current state and freshness: [Current State](../status/chain-state-current.md)
- Implementation details: [Cobalt Implementation](cobalt-implementation.md)
- Validator lifecycle: [Validator Registry](validator-registry.md)
