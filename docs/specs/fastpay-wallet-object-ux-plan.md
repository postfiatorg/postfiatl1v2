# FastPay Wallet Object UX and Unwrap Plan

Status: implemented and deployed on WAN devnet for signed standard unwrap; background dust consolidation remains future work
Last updated: 2026-06-29

## Summary

FastPay sends now work through the wallet path, and standard unwrap has been converted from a whole-object UX problem into a signed/certified owner mutation. The legacy public `unwrap_owned` path moved value from the FastPay owned lane to the account lane without requiring proof that the caller controlled the owner private key. That path now fails closed for public wallet use.

With that lock-down in place, the wallet hides the underlying owned-object model for standard unwrap. The immediate UX symptom was that "unwrap" appeared to be stuck at a specific amount, such as 2 PFT. That was not an innate FastPay rule. It was the wallet surfacing whole-object unwrap behavior instead of presenting a normal amount-based money movement.

The target product behavior is simple:

- Users see one FastPay balance.
- Users enter an amount to send, wrap, or unwrap.
- The wallet and RPC select, split, merge, and create change objects internally.
- The wallet updates balances through the live feed without requiring refresh.

The secure method is to make unwrap an owner-signed, validator-certified FastPay mutation, preferably by reusing the existing owned-transfer certificate envelope and vote/quorum machinery rather than creating a separate unaudited authorization path.

## Critical Security Issue

The legacy `unwrap_owned` RPC shape is unsafe for public use:

```text
unwrap_owned(object_id, owner_pubkey_hex, to_address)
```

It checks that the object is recorded under `owner_pubkey_hex`, but the request does not prove that the caller controls the corresponding owner secret. Object ids and owner public keys are not secrets. A wallet feed or object query can expose enough information for a third party to attempt an unwrap to an arbitrary account.

Implemented safety action:

- Public `unwrap_owned` fails closed and tells callers to use `owned_unwrap_sign` plus `owned_unwrap_apply`.
- Default wallet UI controls no longer call unsigned `unwrap_owned`.
- Wallet and Python tooling use owner-signed `OwnedUnwrapOrder` plus validator quorum certificate apply.

Required regression coverage:

- Add a regression test proving that `object_id + owner_pubkey_hex` is not sufficient to move funds.
- Do not add future partial unwrap, object selection, or amount-based unwrap features on top of the unsigned primitive.

This does not invalidate FastPay sends. Sends already use owner signatures, validator votes, and BFT quorum. The security issue is the unwrap/exit path from FastPay owned objects back to account balances.

## Current PostFiat Behavior

PostFiat currently has two PFT balance surfaces:

- Account lane: account-address balance.
- FastPay owned lane: a set of owned objects, each with `id`, `version`, `owner_pubkey_hex`, `value`, and `asset`.

FastPay transfer already uses the right model:

- The wallet signs an `OwnedTransferOrder`.
- Validators vote on the signed order.
- Apply requires owner auth plus BFT quorum.
- The order can consume inputs and create recipient/change outputs, so amount-based sends can be implemented without exposing object denominations to users.

Relevant local code:

- `crates/execution/src/lib_parts/owned_transfer.rs:68` verifies owned-transfer certificates.
- `crates/execution/src/lib_parts/owned_transfer.rs:104` applies only certified owned transfers.
- `wallet-web/src/lib/tx-builder.js` builds wallet FastPay transfers.

Legacy unwrap did not have the same shape:

- `crates/execution/src/lib_parts/owned_transfer.rs` had an internal `unwrap_from_owned` helper.
- `crates/node/src/rpc_cli.rs` exposed `unwrap_owned`.
- `wallet-web/src/lib/rpc-client.js` called `unwrap_owned(object_id, owner_pubkey_hex, to_address)`.

That function consumes the whole selected object and credits the whole object value to an account. It checks that the supplied public key equals the object's owner public key, but the RPC request does not prove the caller controls that key. Anyone who can discover an object id and owner public key can attempt to unwrap to an arbitrary account address. This must be fixed before treating unwrap as production FastPay UX.

Current implementation status:

- Public node `unwrap_owned` fails closed and instructs callers to use `owned_unwrap_sign` plus `owned_unwrap_apply`.
- The wallet and Python helper use `OwnedUnwrapOrder`, wallet owner signature, validator votes, and quorum apply.
- The standard wallet unwrap is amount-based. The wallet selects a covering object and returns FastPay change automatically.
- Wallet and Python tooling select one or more FastPay objects automatically, up to the 2048-object protocol input cap, and return change as a new FastPay object. Background dust consolidation remains a future enhancement.

Deployment status:

- WAN devnet node binary `4d124e34fa7549abd1042c1ec20166e125503a9017d4246d5404392afce0a6b0` was deployed to all six validators on 2026-06-29.
- Post-deploy strict preflight was green: `6/6` reachable, `6/6` same ledger group, empty mempools, and a single remote binary hash group.
- Evidence: `reports/transaction-improvement/20260629T012710Z-fastpay-owned-objects-read-cap2048-deploy/post-deploy-preflight.json`.

## User-Facing Problem

The wallet currently leaks object accounting:

- Receiving 20 FastPay transfers of 0.1 PFT creates 20 owned objects.
- Wrapping 2 PFT creates a 2 PFT owned object.
- Whole-object unwrap makes the UI appear to offer arbitrary object sizes instead of a clean amount field.
- If the wallet chooses the largest object, unwrap appears to be "always 2 PFT" even though the real issue is object selection.

This is not acceptable consumer wallet behavior. Users should not need to understand object ids, object versions, object fragmentation, or which object to consume.

## How Sui Treats This

Sui is also object-centric, but it mostly hides object fragmentation from normal users.

Sui coins are objects, and Sui transactions can explicitly split, merge, and transfer coins inside programmable transaction blocks (PTBs). A PTB is atomic: if a command fails, the transaction effects do not apply. Sui documents `splitCoins`, `mergeCoins`, and `transferObjects` as PTB commands, and the Move `sui::coin` module exposes `split` and `join` for coin objects.

Sui also provides operational affordances so wallets and integrators do not force users to manually manage denominations:

- PTBs let a wallet combine coin selection, split, transfer, and change handling in one transaction.
- Gas smashing automatically merges multiple gas coins supplied for gas payment.
- Address balances can remove gas coin selection entirely for gas payment.
- Exchange guidance says balances must account for both coin objects and address-balance accumulators, while user-facing withdrawal logic should handle both.
- Fastpath objects require current object id and version, and conflicting transactions against the same mutable owned object version are rejected, so wallets must avoid concurrent reuse of the same owned object.

In short: Sui does not make users pick a specific coin object for ordinary payments. The wallet or SDK performs coin selection, splitting, merging, and change creation.

References:

- Sui PTBs: https://docs.sui.io/develop/transactions/ptbs/prog-txn-blocks
- Sui coin split/join: https://docs.sui.io/references/framework/sui_sui/coin
- Sui gas smashing: https://docs.sui.io/develop/transaction-payment/gas-smashing
- Sui object versioning and fastpath: https://docs.sui.io/develop/objects/versioning
- Sui exchange balance guidance: https://docs.sui.io/operators/exchange-integration

## Proposed PostFiat Treatment

PostFiat should follow the same product pattern: preserve the owned-object fast path internally, but expose amount-based wallet actions.

### 1. Replace unsigned unwrap with a certified owned mutation

The preferred secure method is not a fully separate unwrap protocol. Unwrap should reuse the same security structure as owned transfer:

- Wallet signs canonical owned-mutation bytes with the owner private key.
- Validators verify owner authorization before voting.
- Apply verifies validator votes and requires BFT quorum.
- Apply consumes owned objects only at the referenced id/version.
- Apply enforces conservation.

There were two acceptable implementation shapes:

- Extend the existing `OwnedTransferOrder`/certificate path with an account-credit output variant.
- Introduce a more general `OwnedMutationOrder` envelope that can represent `owned_to_owned` and `owned_to_account` effects while reusing the existing signature, vote, quorum, replay, and fixed-width signing-byte machinery.

The implemented path uses a dedicated `OwnedUnwrapOrder`, because unwrap credits the account lane rather than creating only owned outputs. It still follows the owned-transfer certificate pattern: fixed signing bytes, owner signature, validator votes over the same order bytes, BFT quorum, input id/version checks, and conservation.

Implemented shape:

```text
OwnedUnwrapOrder {
  inputs: Vec<OwnedObjectRef>,
  to_address: String,
  amount: u64,
  asset: String,
  fee: u64,
  nonce: u64,
  memos: Vec<PaymentMemo>
}

OwnedUnwrapCertificate {
  order: OwnedUnwrapOrder,
  owner_pubkey_hex: String,
  owner_signature_hex: String,
  votes: Vec<OwnedUnwrapVote>
}
```

Preferred generalized shape:

```text
OwnedMutationOrder {
  inputs: Vec<OwnedObjectRef>,
  owner_pubkey_hex: String,
  effects: Vec<OwnedMutationEffect>,
  fee: u64,
  nonce: u64,
  memos: Vec<PaymentMemo>
}

OwnedMutationEffect =
  CreateOwnedObject { owner_pubkey_hex, value, asset }
  CreditAccount { to_address, value, asset }

OwnedMutationCertificate {
  order: OwnedMutationOrder,
  owner_pubkey_hex: String,
  owner_signature_hex: String,
  votes: Vec<OwnedMutationVote>
}
```

Signing requirements:

- Use a dedicated domain/context for the generalized envelope, for example `postfiat-owned-mutation-v1`.
- Use fixed-width integer and length encodings, matching the fixed-width signing-byte bug fix already made for owned transfers.
- Owner signature must verify against the order bytes.
- Validator votes must verify against the same order bytes.
- Apply must require BFT quorum, not all validators.

RPC shape:

- `owned_unwrap_sign`: validator validates the owner-signed unwrap intent and returns a validator vote.
- `owned_unwrap_apply`: applies the certified unwrap at quorum.
- `unwrap_owned`: remove, disable, or restrict to a test/admin-only compatibility path until it is signature-protected.

Required tests:

- Correct owner signature plus quorum succeeds.
- 5 of 6 succeeds; forced all-6 mode fails with one down validator.
- Wrong owner signature fails with `OwnerAuthFailed`.
- Caller who knows `object_id` and `owner_pubkey_hex` but lacks the owner secret cannot unwrap.
- Wrong object version fails.
- Wrong asset fails.
- Amount greater than selected object value fails.
- Replay of a consumed object fails.

### 2. Add partial unwrap with change

The protocol should support unwrapping an amount, not only a whole object.

Apply semantics:

- Select one or more owned objects for the requested asset.
- Verify all selected objects belong to `owner_pubkey_hex`.
- Verify the selected total covers `amount + fee`.
- Consume the selected input objects.
- Credit exactly `amount` to `to_address` on the account lane.
- If there is remainder, create one owned change object back to `owner_pubkey_hex`.
- Enforce conservation: `input_total == account_credit + change_total + fee`.

Example:

- Input object: 2.0 PFT
- Requested unwrap: 0.3 PFT
- Account credit: 0.3 PFT
- FastPay change object: 1.7 PFT

This makes "unwrap 0.3 PFT" possible even when the wallet only has a 2 PFT FastPay object.

### 3. Add wallet-side coin selection

The wallet should select FastPay objects automatically.

Initial policy:

- Prefer exact match.
- Otherwise use the smallest object that covers the amount.
- Otherwise use largest-first until the amount is covered.
- Cap max inputs per operation to bound validator work. The current cap is 2048 owned inputs, matching Sui's current mainnet `max_input_objects` scale rather than the earlier prototype cap of 8.
- Return one change object to avoid increasing object count unnecessarily.

Later policy:

- Add background consolidation for dust.
- Opportunistically consolidate many small objects during sends or unwraps.
- Keep advanced object inspection for diagnostics, not the default workflow.

### 4. Clean wallet UX

Default UX:

- FastPay card shows one FastPay balance.
- "Move to Account" opens an amount input.
- "Move to FastPay" opens an amount input.
- Object ids and object denominations are hidden.
- Pending state shows until quorum apply or live-feed confirmation.
- The live wallet feed updates account and FastPay balances without refresh.

Advanced/debug UX:

- Object list is hidden behind an advanced disclosure.
- Each object shows id, version, value, and asset.
- Manual object unwrap remains available only if the certified owned-mutation path is used.

### 5. RPC/live-feed support

The wallet feed should continue to publish a normalized wallet snapshot:

```text
{
  account_balance: u64,
  owned_total: u64,
  owned_object_count: usize,
  owned_objects: Vec<OwnedObject>
}
```

The UI should render `owned_total` as the default FastPay balance. Object details are only needed for advanced views and wallet-side input selection.

Optional convenience RPC:

- `fastpay_unwrap_quote(owner_pubkey_hex, amount, asset)`: returns selected inputs, total input, expected account credit, expected change, and any fee.

The quote is advisory only. The signed owned-mutation order is the source of truth.

## Implementation Phases

### P0: Lock down unsafe unwrap

- Disable the unsigned public `unwrap_owned` path immediately, or make it require owner authorization before accepting any request. Done: public `unwrap_owned` now fails closed.
- Add a regression test proving object id plus public key is not enough to move funds.

### P1: Certified standard unwrap

- Add the signed/certified owned-mutation support needed for unwrap. Done.
- Add signing bytes, WASM signing export, native verification, validator votes, and quorum apply. Done.
- Credit exactly the requested amount to account lane and return FastPay change automatically. Done for one or more input objects.
- Update wallet and Python tooling calls to use the signed certificate path. Done.

### P2: Multi-input unwrap

- Extend wallet/RPC selection to consume multiple input objects when no single object covers the amount. Done in wallet and Python tooling up to the protocol cap.
- Mint a single FastPay change object when needed. Done in certified apply.
- Add conservation, overflow, version, owner, asset, quorum, and replay tests.

### P3: Amount-based wallet UI

- Replace the object dropdown in default unwrap with an amount field. Done.
- Keep object selection under an advanced/debug section only.
- Continue using the WSS wallet feed for refresh-free balance updates.

### P4: Dust management

- Add wallet coin selection heuristics.
- Add consolidation on send/unwrap where it reduces object count without surprising the user.
- Add object-count warnings only when the count affects performance or input limits.

## Acceptance Criteria

- A user can unwrap 0.1 PFT from a 2 PFT FastPay object and receives exactly 0.1 PFT on the account lane.
- The remaining 1.9 PFT stays in FastPay as change owned by the same wallet.
- A user with 20 received 0.1 PFT FastPay objects can unwrap 0.35 PFT without selecting object ids.
- A caller without the owner secret cannot unwrap any object, even if they know the object id and owner public key.
- FastPay send and unwrap both succeed at 5 of 6 validators.
- The wallet balance changes in real time through the live feed after quorum apply.
- The default wallet UI never requires users to choose an object denomination.

## Open Questions

- Should unwrap allow `to_address` other than the wallet's own account address, or should arbitrary recipients be handled as normal FastPay/account sends?
- What fee policy should apply to wrap and unwrap while FastPay is in devnet mode?
- Should object consolidation be explicit, automatic, or only opportunistic during user-initiated operations?
