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
| Asset-Orchard | Shielded PFT/issued-asset notes | RedPallas action authorization plus Halo2 proof; block inclusion is validator-certified | Accepted shielded receipt, new commitments/nullifiers, and public turnstile accounting | Implemented private ingress, transfer/swap, recovery, and egress; boundary metadata remains public. On the live devnet, public submission of shielded actions is off on every validator, as recorded in the [handoff of 23 November 2026](../status/handoff-2026-11-23.md). |

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

Recovery committee V1 preserves historical roots and certificate commitments.
A signed, ordered governance update installs committee V2, whose root also binds
the admission heights. Installation switches state commitments for every retained
reveal and fence to the complete-certificate V2 encoding. The committee's future
admission height separately controls when validators accept new orders. Earlier
committees remain available for recovery; a V2-to-V1 downgrade is rejected.
See [release compatibility](../specs/release-history-compatibility-20260916.md).

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

## Solana USDC route

Checked against `main` and the live devnet on 25 November 2026. Both
directions are consensus transactions; the Solana side is a vault program and
an exit verifier on Solana mainnet. It runs on a controlled devnet with small
caps.

- **Deposit.** A holder deposits USDC into the vault program on Solana
  (`solana/pfusdc-vault`). A proof of the finalized deposit is submitted to
  Post Fiat as `solana_bridge_proof_v1`; execution verifies it and credits
  pfUSDC in the series and backing bucket of the ingress route revision that
  verified it (each revision has its own bucket and series; the handoffs call
  one such bucket a deposit batch)
  (`apply_proof` and `credit_verified` in `crates/execution/src/solana_bridge.rs`).
- **Withdrawal.** The holder burns pfUSDC with `vault_bridge_burn_to_redeem` and
  a `solana-spl:` destination; the burn opens a pending redemption
  (`crates/execution/src/nav_vault_asset_execution.rs`). A payout worker proves
  the finalized burn to the exit verifier (`solana/pfusdc-egress-verifier`),
  which pays USDC from the vault and writes the burn's one-use marker account.
  A validator decision then settles the redemption on Post Fiat
  (`crates/node/src/solana_exit_governance.rs`).
- **Refund.** A burn that cannot be paid can be cancelled on Solana by its
  destination owner or the verifier's pause authority. Payout and cancel create
  the same marker account, as paid or as cancelled, and both refuse a marker
  that exists (`release_burn` and `cancel_burn` in
  `solana/pfusdc-egress-verifier/src/lib.rs`). A validator decision then
  credits the burner back
  ([design note](../specs/solana-exit-refund-design-20261124.md)). A cancel
  and refund has not been run on the live chain.
- **Releases.** The first withdrawal rules are active from block 1841 and the
  second from block 1890 (every bucket of the shared custody can exit, refund path). The
  second release's height is the `SECOND_RELEASES` entry in
  `crates/types/src/solana_exit.rs`; it was set by the release branch
  `release/sol-exit-r2` (build `04600868`, live from 24 November) and is on
  `main` since #234. Since 25 November the validators run `sol-exit-r3`
  (`2356caf1`), in which a Solana exit payout or refund settles with five of
  six validators ([Network Status](../status/network-status.md)).
- **When the route is open.** Since 26 November proofs are made only during a
  daily proving session, planned for 17:00 to 19:00 UTC; outside it Solana
  deposits and withdrawals are closed on purpose
  ([Network Status](../status/network-status.md#usdc-bridges)).
- **Limits on 24 November** ([handoff of 25 November](../status/handoff-2026-11-25.md)):
  one withdrawal burns from one bucket and its series; withdrawals below 0.30 USDC are
  refused; the vault accepted 25 USDC more; a withdrawal takes about four
  minutes; cancel is refused while the route is paused.

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
