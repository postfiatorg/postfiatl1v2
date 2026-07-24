# Lightning/PFTL synthetic coordinator core

This directory contains the network-independent coordinator state machine and
the direct-LND-gRPC adapter contract for the no-value regtest demo.

## Stable interfaces

- `protocol.py`: fixed-width PREIMAGE-SHA-256 vectors and LND invoice binding.
- `quote.py`: exact-field canonical JSON quote validation.
- `signing.py`: independently verifiable Ed25519 quote envelopes.
- `journal.py`: WAL/FULL SQLite state, caps, events, side effects, retries, and
  crash recovery.
- `service.py`: typed lifecycle façade for the E2E runner.
- `lnd_grpc.py`: direct calls to an injected generated Lightning gRPC stub;
  runtime never invokes a hosted API, REST, or `lncli`.

The environment wires generated LND protobufs as follows:

```python
factories = LndRequestFactories.from_proto_modules(lightning_pb2, router_pb2)
lnd = LndGrpcAdapter(
    lightning_pb2_grpc.LightningStub(channel),
    router_pb2_grpc.RouterStub(channel),
    factories,
    network="regtest",
)
```

The LND TLS/macaroon-authenticated `grpc.Channel` is owned by the environment
layer. Runtime methods are `AddInvoice`, `DecodePayReq`, `LookupInvoice`, and
the Router service's server-streaming `SendPaymentV2` on those direct stubs.

## Signature decision

Python's standard library has no public-key signature implementation.
`QuoteSigner` is therefore an injected interface and the concrete demo
implementation delegates Ed25519 to `cryptography`; there is no HMAC or
home-grown signature fallback. Public key, key ID, signature, canonical quote
hash, and a deterministic test vector are explicit. The deterministic private
seed under `tests/` is test-only and must never be used outside the synthetic
harness.

## Persistence and secret boundary

Every accepted quote reserves exposure immediately. Legal state transitions,
event idempotency, and side-effect intent are committed atomically. Remote
effects remain at-least-once: adapters must submit/query using the durable
`effect_key`, because SQLite cannot make a remote LND or PFTL call exactly
once.

The settled-payment result also reports every terminal LND HTLC route's
`total_time_lock` as `payer_htlc_expiries`; the harness uses the maximum value
as `B_pay_exp` evidence for its cross-ledger margin check.

Preimages live in the separate `secrets` table and are excluded from public
audit exports. Public event/effect payloads reject secret-bearing field names.
This plaintext secret table is acceptable only for the synthetic demo;
production would require a sealed keystore. `s` appears only in the dedicated
protocol-vector artifact.

## Offline tests

From the repository root:

```sh
python3 -m unittest discover \
  -s tools/lightning_navcoin_demo/coordinator/tests -v
```
