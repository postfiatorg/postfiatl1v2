# Owned-object creation and asset-source inventory — 2026-07-16

Status: implementation-complete locally; immutable publication-candidate rerun pending.

This is the authoritative `P0-ASSET-01` constructor inventory. It distinguishes
live state transitions from tests and the isolated research prototype. The
security invariant is: an owned object's `asset` and `value` must be derived
from value removed from the same asset lane, and every rejection must leave the
ledger unchanged.

## Live `LedgerState` constructors

| Constructor | Source of value | Asset binding | Atomicity and collision rule |
|---|---|---|---|
| `apply_owned_transfer` in `crates/execution/src/owned_transfer.rs` | Current, owner-authorized input objects | Every output must exactly match the single input asset; checked output sum plus fee equals checked input sum | All inputs, outputs, zero values, resource limits, and content-addressed IDs are validated before inputs are retired; any live-ID collision rejects |
| `apply_owned_unwrap` in the same file | Current, owner-authorized input objects | Only exact `PFT` may credit `Account.balance`; change remains exact `PFT` | Destination overflow and change-ID collision are checked before input retirement; issued assets fail without mutation |
| `wrap_to_owned` in the same file | Exact debit from native `Account.balance` | Exact case-sensitive `PFT` only | Zero, insufficient balance, and existing object ID reject before debit; the unsigned remote wrapper remains unavailable |
| `apply_owned_deposit` in the same file | Source-signed, sequence-bound native account debit ordered through consensus | Exact case-sensitive `PFT` only; object value equals `amount_atoms`, while the separately declared fee is burned | Signature/domain/key/sequence/expiry/value/collision checks precede clone-and-publish mutation; deterministic object ID derives from all signed fields |

No other non-test module constructs or inserts `postfiat_types::OwnedObject`.
`crates/types/src/account_owned_asset_types.rs` only defines the serialized type.
`crates/node/src/state_commitment.rs` only reads and canonically commits objects.

## Non-production constructors

- `crates/execution/src/owned_transfer.rs` after its `#[cfg(test)]` boundary,
  `crates/node/src/tests/**`, and `crates/fuzz_harness/src/main.rs` construct
  adversarial fixtures only.
- `crates/fastpay-prototype/src/state.rs` uses a separate research-only
  `OwnedObject` type. Its genesis-fixture insertion now rejects zero, malformed,
  and duplicate objects. Its apply path rejects duplicate inputs, overflow,
  zero/mixed outputs, and output-ID collisions before mutation.
- `crates/fastpay-prototype/src/flow.rs` no longer pushes unbacked objects. The
  executable demo moves 100 native atoms from an account through the production
  `wrap_to_owned` transition before demonstrating transfer and replay rejection.

## Adversarial evidence

- The execution regression first reproduced four real defects: duplicate wrap
  IDs debited and duplicated state; issued objects could credit native PFT;
  legacy overflow consumed the input; certified overflow consumed inputs.
  `owned_transfer_tests` now covers those cases, zero output, output collision,
  wrong labels, replay, owner/version errors, and exact conservation.
- The node regression
  `certified_owned_unwrap_cannot_convert_issued_asset_to_native_balance` submits a
  correctly owner- and validator-signed `pfUSDC` unwrap certificate through the
  real apply/store boundary and proves `UnsupportedAsset` plus byte-identical
  ledger state.
- Eight simultaneous unsigned wrap requests all return `PermissionDenied` and
  leave the real persisted ledger byte-identical. The live signed replacement
  is consensus-serialized and its same-sequence replay test admits only one
  deterministic deposit.
- `postfiat-fuzz owned-object-asset-invariants --iterations 256` exercised 2,816
  wrong-label, zero, overflow, collision, issued-unwrap, valid-wrap, replay, and
  transfer cases with zero invariant failures.
- The prototype regressions prove a duplicate genesis ID and duplicate input can
  neither overwrite nor inflate value, and duplicate validator IDs cannot inflate
  a certificate count.

The final immutable candidate must rerun the execution, node FastPay, prototype,
fuzz, replay, formatting, workspace check/test, and strict-Clippy gates before
`P0-ASSET-01` is release-closed.
