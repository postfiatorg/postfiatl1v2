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

1. Cobalt runs as an authenticated shadow service with no live authority.
2. The active Foundation registry signs one exact transition with a distinct
   ML-DSA-65 quorum.
3. Consensus v2 orders that transition at its activation height.
4. Only then may Cobalt authorize validator-trust updates. It cannot authorize
   unrelated governance and a new validator set cannot authorize itself.
5. Returning to Foundation authority is another signed forward transition; it
   never rewrites finalized history.

A Cobalt failure can pause validator governance. It cannot create a second
block-finality protocol.

## Current Decision

The authenticated benchmark and disposable handoff packets support **GO for a
later, separately authorized controlled-testnet validator-trust cutover**.
That is not an activation. Foundation validator-trust authority remains active
and Consensus v2 remains the only block-finality protocol.

Run the decision directly:

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
- Final decision: `benchmarks/cobalt-activation-readiness/packet/`
- Implementation details: [Cobalt Implementation](cobalt-implementation.md)
- Validator lifecycle: [Validator Registry](validator-registry.md)
