# Settlement Lanes

PostFiat has several settlement lanes because account payments, prefunded
object payments, bilateral DvP, and private notes have different authorization
and recovery requirements. They share deterministic value accounting, bounded
inputs, domain-separated signatures, replay protection, and fail-closed receipt
semantics. They do not share one interchangeable finality rule.

## Current implementation matrix

| Lane | What moves | Authorization | Terminal proof | Current boundary |
| --- | --- | --- | --- | --- |
| Consensus transaction | Native PFT, issued assets, escrows, NFTs, offers, NAV/bridge actions | Transaction-family signature(s) | Valid block certificate plus matching accepted receipt | Implemented; consensus v2 is used only at/above its configured activation boundary. |
| W6 atomic swap | Two account/issued-asset legs | Two distinct ML-DSA-65 owner authorizations over one exact intent | Consensus certificate plus accepted atomic-swap receipt | Implemented and covered by six-validator both-or-neither/conservation tests. |
| FastPay | Prefunded single-owner PFT objects | Owner-signed order plus distinct-validator certificate | Durable certified apply under the active FastPay policy | Implemented in source with ordered consume-or-cancel recovery; production activation and custody remain deployment gates. |
| FastSwap | Prefunded two-owner asset objects | Two owner signatures plus phase-specific distinct-validator certificates | Terminal Confirm EffectsQC, or terminal Cancel decision | Implemented in source with exact-six catch-up/restart evidence; shared-network activation is separate from source availability. |
| Asset-Orchard | Shielded PFT/issued-asset notes | RedPallas action authorization plus Halo2 proof; block inclusion is validator-certified | Accepted shielded receipt, new commitments/nullifiers, and public turnstile accounting | Implemented private ingress, transfer/swap, recovery, and egress; boundary metadata remains public. |

## W6 consensus atomic swap

`SignedAtomicSwapTransaction` binds both complete legs, both owner
authorizations, chain/genesis/protocol identity, quote/freshness inputs, parent
state, fees, and replay fields. It is executed inside the ordinary block state
transition. There is no intermediate one-leg success state: validation or
arithmetic failure returns a rejected receipt without partial mutation.

Client success requires both:

1. a valid quorum block certificate; and
2. the matching receipt with `accepted=true` and an accepted code.

Replica convergence is valuable audit evidence, but it cannot turn a rejected
receipt into success.

## FastPay payment lane

FastPay is for single-owner prefunded PFT payments and unwraps, not bilateral
swaps. Admission verifies the complete owner-signed envelope and current object
state before atomically persisting a lock and returning a validator vote.
Certificates count distinct validator identities. Apply persists the complete
certificate, effects, terminal fence, and replay indexes atomically.

Versioned recovery resolves an expired lock through the ordered ledger:

- a complete revealed certificate confirms the payment exactly once; or
- absence of a complete certificate produces a certified cancellation and
  advances the object version.

Local timeout deletion is forbidden because a late certificate could otherwise
double-spend the unlocked object.

## FastSwap DvP lane

FastSwap is a separate dual-owner object protocol. Both owners sign the same
canonical intent. Validators reserve all referenced inputs atomically and use
distinct-validator certificates across three dependent phases:

1. prepare produces the lock certificate;
2. decision chooses exactly one Confirm-or-Cancel result; and
3. effects applies the confirmed DvP or records cancellation.

Confirm moves both legs and preserves per-asset totals. Cancel releases the
reservation without moving either leg. WAL persistence, terminal tombstones,
round fencing, policy/committee binding, catch-up, checkpointing, and
fail-closed rotation prevent a stale or delayed certificate from reviving a
completed swap.

The repository's fast local measurements are six-process controlled evidence,
not WAN or mainnet promises. Network topology, deployment, public edge policy,
key custody, and activation remain separate operational claims.

## Asset-Orchard

Asset-Orchard proves note membership, nullifier correctness, authorization,
value conservation, output commitments, and public accounting. Ingress and
egress deliberately reveal their public boundary asset/value/destination data;
the internal note openings and supported private-swap terms remain hidden.
Legacy cleartext note actions are accepted only under authenticated historical
replay predicates.

## Source anchors

- `crates/types/src/transactions_mempool_receipts.rs`
- `crates/types/src/fastswap_types.rs`
- `crates/execution/src/entrypoints.rs`
- `crates/execution/src/owned_transfer.rs`
- `crates/execution/src/owned_transfer_recovery.rs`
- `crates/execution/src/fastswap.rs`
- `crates/node/src/atomic_swap_rpc_server.rs`
- `crates/node/src/fastswap_service.rs`
- `crates/storage/src/fastswap_store.rs`
- `crates/privacy_orchard/src/lib.rs`
- `docs/specs/fastpay-payment-recovery-v1.md`
