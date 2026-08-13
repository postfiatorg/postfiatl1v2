# pfUSDC Source-Labeled Pool v1

Status: consensus specification for implementation and conformance testing

Version: `postfiat.pfusdc.source_labeled_pool.v1`

Normative arithmetic: unsigned integers only; all amounts are six-decimal atoms

This specification turns the accounting model in the local PostFiat.org
article `content/blog/pfusdc.md` into deterministic protocol rules. The name
`pfUSDC` is a wallet display family. It is not permission to merge claims on
different vaults, routes, policies, or impairment factors.

## 1. Identities

One source series is:

```text
source_series_id = SHA3-384(
  "postfiat.pfusdc.source_series.v1" || 0x00 ||
  canonical(
    pftl_chain_id,
    asset_family_id,
    source_chain_id,
    vault_address,
    token_address,
    route_epoch,
    policy_hash
  )
)
```

The canonical encoding is the length-bound newline encoding implemented by
`pfusdc_source_series_id`. Addresses and hashes are lowercase fixed-width hex.
Changing any field creates a different series. A bucket is the pair
`(asset_family_id, source_domain, policy_hash)` and resolves to exactly one
governed source series.

Existing fungible balances created before source-series enforcement are
classified as `legacy_pooled`. They MUST NOT be displayed as successor-vault
claims or uniformly redeemable at par until a certified migration attributes
them to source series.

## 2. Receipt lifecycle

A proof-bound source event creates at most one `VaultBridgeReceipt`:

```text
Pending -> Finalized -> Counted -> Retired
                    \-> Impaired
```

The receipt commits chain, vault, token, depositor, recipient, amount,
transaction, finalized block, route epoch, policy, verifier program, and replay
nullifier. No state before `Finalized` contributes counted value.

For a governed haircut `h` in basis points:

```text
counted_value_atoms = floor(amount_atoms * (10000 - h) / 10000)
```

`0 <= h <= 10000`. Multiplication is widened to `u128`; the result must fit
`u64`. Rounding always reduces issuance capacity.

## 3. Allocation and atomic ingress

For every bucket `b`:

```text
allocated[b] = outstanding_source_series[b]
             + nav_subscription_allocations[b]
             + redemption_queue[b]
             + other_protocol_allocations[b]

allocated[b] <= counted_cash[b]
```

One ingress transition atomically:

1. consumes the finalized deposit nullifier;
2. creates or validates its receipt;
3. counts the receipt using the governed haircut;
4. creates exactly one supply allocation;
5. credits exactly the proof-bound recipient;
6. updates the exact bucket;
7. advances the proof-bounded supply checkpoint and reserve root.

Any failure commits none of these effects. Replaying any deposit, receipt,
nullifier, allocation, or consumer ID is rejected.

## 4. Global supply

Every custody lane is counted exactly once:

```text
global_live_supply = transparent_trustlines
                   + escrow_custody
                   + FastPay_owned_supply
                   + external_bridge_custody
                   + AssetOrchard_live_supply
```

A proof-backed claim may grow the finalized checkpoint to `post_claim_supply`
only when:

```text
post_claim_supply <= prior_checkpoint
                   + finalized_unclaimed_backing_for_exact_route
```

The Orchard-aware calculation activates at the governed
`orchard_aware_bridge_claim_activation_height`. Historical replay below that
height uses the historical rule.

## 5. Transfer, private custody, and redemption

After source-series enforcement activates, every transparent balance, escrow,
FastPay object, Orchard note, NAV allocation, and bridge custody object carries
the same `source_series_id`. Movement changes custody location, never series or
global supply.

A redemption burns the holder's selected source series and queues liability
only in its bucket. Settlement releases only the vault named by that series.
A weak bucket cannot drain a stronger bucket.

## 6. Impairment and recapitalization

For impairment factor `f` in basis points:

```text
redeemable_atoms = floor(claim_atoms * f / 10000)
```

Wallet value and redemption use the same factor. An impaired claim cannot be
called par or successor-backed. Governance may reduce the factor through a
versioned impairment packet, but may not add counted value.

Recapitalization requires a new proof-backed receipt. A migration receipt names
both retired and replacement sources. The retired counted value is removed
before replacement value is counted; no state may count both.

## 7. Reserve replay

A replay packet commits sorted receipts, deposits, allocations, redemptions,
buckets, custody-lane supply, policy roots, source roots, and the finalized
state root. Collections sort by canonical identifier. Unknown custody lanes,
duplicate identifiers, overflow, negative residuals, or unexplained atoms fail
closed.

## 8. Conformance vectors

All values below are atoms.

| Vector | Inputs | Required result |
|---|---|---|
| Healthy | amount `15_000_000`, haircut `0` | counted/allocation/credit `15_000_000` |
| Haircut | amount `1_000_001`, haircut `25` | counted `997_500` |
| Partial allocation | counted `10`, existing allocated `6`, request `4` | accept; allocated `10` |
| Allocation overflow | counted `10`, existing allocated `6`, request `5` | reject; byte-identical state |
| Orchard regression | complete non-Orchard custody `269_700_595` (including non-NAV spread `2_166_461`), Orchard `20_000_000`, claim `15_000_000`, checkpoint `297_933_789` | global/checkpoint `304_700_595` |
| Insufficient proof backing | same regression, route unclaimed `14_999_999` | `proof_bounded_nav_cap_exceeded`; no effects |
| Exact proof backing | same regression, route unclaimed `15_000_000` | accept once; replay rejects |
| Transfer | series A sends `7` between holders | series A total unchanged; series B unchanged |
| Private move | transparent series A `-7`, Orchard series A `+7` | global and series A totals unchanged |
| Source redemption | burn series A `5` | only bucket A queue `+5` |
| Impairment | claim `9_932_863`, factor `0` | redeemable/value `0`; no par label |
| Recapitalization | retire old counted `5`, prove replacement `5` | aggregate counted unchanged; never `10` |
| Replay | repeat receipt/nullifier/allocation | reject; exact prior state root |
| Overflow | any checked sum exceeds `u64::MAX` | reject; exact prior state root |

The Rust transition corpus and an independent implementation must reproduce
these vectors byte-for-byte before source-series migration can activate.
