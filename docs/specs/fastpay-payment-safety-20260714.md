# FastPay single-owner payment safety tightening

**Status:** implemented locally for review; not deployed  
**Scope:** owned transfer/unwrap payments only; this does not make the lane safe
for W6 or any multi-owner atomic swap.

## Closed in code

1. Certificate verification rejects repeated validator IDs before counting
   signatures. The same rule applies to transfer and unwrap certificates.
2. `owned_sign` and `owned_unwrap_sign` accept an owner-authorized envelope,
   verify the ML-DSA-65 owner signature, and validate live object ownership,
   versions, conservation, duplicate inputs, and resource/memo bounds before
   taking any lock. A bare order is rejected during deserialization.
3. Transfer and unwrap share one durable lock transaction. A process-wide and
   cross-process OS file lock serializes read/check/write; the resulting JSON is
   written through the storage crate's synced atomic rename before the validator
   signature is produced. A crash after persistence can leave an availability
   lock, but cannot leave an emitted vote without its durable lock record.

## Structural safe-unlock finding

`owned_safe_unlock` previously deleted every old-registry lock. That is unsafe:
an already assembled old-registry certificate can arrive after deletion and
execute alongside a new spend of the released object.

Binding a registry ID into new messages alone does not invalidate certificates
already issued under the old format and does not prove they were drained.
Closing the hazard requires a canonical transition that stops new locks,
checkpoints every old-registry certificate/effect, carries unresolved lock and
spent state into the new registry, and proves the boundary before release. The
selected consume-or-cancel protocol and its executable safety model are now
specified in `docs/specs/fastpay-payment-recovery-v1.md`; its production storage,
ordered decision, activation and wallet paths are not implemented yet.

The command therefore now **fails closed and preserves every lock**. This closes
the late-certificate double-spend route without pretending to solve recovery.
The residual is liveness: an equivocation/abandoned lock can remain stuck until
the structural drain/cancel protocol is designed and deployed.

## Regression coverage

- duplicate transfer and unwrap validator vote rejection;
- duplicate-input rejection without ledger mutation;
- unauthorized and stale-input admission writes no lock;
- concurrent conflicting sign requests emit at most one vote and one lock;
- persisted-before-sign recovery re-emits only the same vote and refuses a
  conflict;
- safe-unlock returns `Unsupported` and leaves old locks byte-for-byte intact;
- the existing `fastpay_safety_chaos_gate` still passes.

## Compatibility note

The `owned_sign`/`owned_unwrap_sign` RPC parameter remains named `order_json` for
transport compatibility, but its value is now the complete signed envelope:

```json
{
  "order": { "inputs": [], "outputs": [], "fee": 0, "nonce": 0 },
  "owner_pubkey_hex": "...",
  "owner_signature_hex": "..."
}
```

Real requests must contain valid non-empty orders; the abbreviated object above
only documents the envelope shape.
