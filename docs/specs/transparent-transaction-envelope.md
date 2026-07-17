# Transparent Transaction Envelope v1

Status: canonical v1 transparent-transfer envelope
Date: 2026-05-13
Scope: controlled-testnet transparent PQ settlement path

This document freezes the current transparent transfer signing surface. Any
change to these bytes is a protocol change and must use a new version label or
new envelope kind. The purpose is to make wallet, RPC, mempool, execution, and
external review agree on exactly what is signed and replay-protected.

## Implemented Code References

- Unsigned transfer fields and signing bytes:
  `crates/types/src/lib.rs:852`.
- Signed transfer envelope:
  `crates/types/src/lib.rs:904`.
- Transfer execution and replay/domain rejection:
  `crates/execution/src/lib.rs:13`.
- Transfer transaction id:
  `crates/execution/src/lib.rs:321`.
- Wallet deterministic test vector:
  `crates/node/src/lib.rs:8066`.
- Test-vector drift lock:
  `crates/node/src/lib.rs:12308`.
- Mempool admission for externally signed transfers:
  `crates/node/src/lib.rs:2570`.

## Unsigned Envelope Fields

`UnsignedTransfer` contains these fields, in the canonical order used for
signing bytes:

1. `chain_id`
2. `genesis_hash`
3. `protocol_version`
4. `address_namespace`
5. `transaction_kind`
6. `signature_algorithm_id`
7. `from`
8. `to`
9. `amount`
10. `fee`
11. `sequence`

Controlled-testnet transparent transfers currently use:

- `address_namespace = postfiat.address.v1`
- `transaction_kind = transparent_transfer`
- `signature_algorithm_id = ML-DSA-65`
- `protocol_version = 1`

## Signing Bytes

The signed message is UTF-8 text with newline-delimited `key=value` fields. It
has a leading version label and a trailing newline after `sequence`.

```text
postfiat.transfer.v1
chain_id={chain_id}
genesis_hash={genesis_hash}
protocol_version={protocol_version}
address_namespace={address_namespace}
transaction_kind={transaction_kind}
signature_algorithm_id={signature_algorithm_id}
from={from}
to={to}
amount={amount}
fee={fee}
sequence={sequence}
```

There is no implicit serialization, field sorting, or optional-field omission
inside the signing bytes. The order above is consensus-critical.

## Signature And Transaction Id

The transparent path signs the exact bytes above with ML-DSA-65 using the
transaction signature context `postfiat-l1-v2/tx/v1`.

`SignedTransfer` then carries:

- `unsigned`
- `algorithm_id`
- `public_key_hex`
- `signature_hex`

The transaction id is:

```text
hash_hex("postfiat.tx_id.v1", bytes)
```

where `bytes` is the unsigned signing bytes followed by:

```text
algorithm={algorithm_id}
public_key={public_key_hex}
signature={signature_hex}
```

with newline separators and a trailing newline.

## Replay And Domain Binding

The envelope is bound to a specific network by `chain_id`, `genesis_hash`, and
`protocol_version`. Execution rejects mismatches before applying balance state.

Replay controls currently implemented:

- wrong chain id is rejected as `wrong_chain`;
- wrong genesis hash is rejected as `wrong_genesis`;
- wrong protocol version is rejected as `wrong_protocol_version`;
- wrong address namespace is rejected as `wrong_address_namespace`;
- wrong transaction kind is rejected as `wrong_transaction_kind`;
- unsupported algorithm ids are rejected;
- bad ML-DSA signatures are rejected;
- duplicate transaction ids are rejected at mempool admission;
- duplicate pending sender sequences are rejected at mempool admission;
- ledger sequence rules reject non-next sender sequences at execution.

The v1 transparent envelope does not include an expiry field. Adding expiry,
memo, multi-operation payloads, batching fields, or alternative fee semantics
must create a versioned envelope change instead of silently extending
`postfiat.transfer.v1`.

## Canonical Fixture

The public fixture is intentionally deterministic and must not be funded.

Inputs:

- `chain_id = postfiat-vector-test`
- `validator_count = 5`
- `master_seed_hex = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`
- `account_index = 0`
- `to = pfvectortestrecipient000000000000000001`
- `amount = 17`
- `sequence = 1`
- `signature_seed_hex = 1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100`

Expected public outputs:

- `genesis_hash = aeb7f3f558e61dfd91a21889ef5fb61ca33725bdcc2669735f115ce62c54e4496c8882a384c10c13d5f0f928ca1847d7`
- `from = pf857c81edb95af1d64262ed6c0fdcf3ef7aff56fe`
- `fee = 32`
- `transfer_signing_hash = ac3945f0b667d58df9ef6c938a16a199c9e66a64d86b6403e48383a0e369f8dd514b5b32490c7e3aef98e401eb1dc1a1`
- `tx_id = 43bb98835f4219afe5cfa5f3aab57692d973fdaa6a5f4b6bfc09113d99b4a8b0b6831bda035605ed6cef26561a53d89a`

Expected signing bytes:

```text
postfiat.transfer.v1
chain_id=postfiat-vector-test
genesis_hash=aeb7f3f558e61dfd91a21889ef5fb61ca33725bdcc2669735f115ce62c54e4496c8882a384c10c13d5f0f928ca1847d7
protocol_version=1
address_namespace=postfiat.address.v1
transaction_kind=transparent_transfer
signature_algorithm_id=ML-DSA-65
from=pf857c81edb95af1d64262ed6c0fdcf3ef7aff56fe
to=pfvectortestrecipient000000000000000001
amount=17
fee=32
sequence=1
```

The drift lock in `wallet_test_vector_is_deterministic_and_redacted` asserts
these bytes, the signing hash, and the transaction id.
