# A666 Variable-Size Real-NAV Round-Trip Execution Spec

**Date:** 2026-07-29  
**Priority:** P0  
**Status:** proposed; no live funds authorized until Gate 0 passes  
**Network:** Ethereum mainnet and the six-validator PFTL fleet  
**Asset route:** Ethereum USDC ↔ pfUSDC ↔ A666 ↔ Ethereum wA666  
**Target:** one small transparent round trip, one 100× larger private-middle
round trip, and one intervening NAV mark built from fresh real StakeHub
sources

This is the clean execution specification for the next user-facing campaign.
It does not rewrite the historical 2026-07-28 campaign or inherit that
campaign's value lineages.

The required order is:

1. deposit a small amount of Ethereum USDC and transparently issue/export
   `1.000000 A666`;
2. deposit a larger amount of Ethereum USDC and privately issue/export
   `100.000000 A666`;
3. recalculate and finalize A666 NAV from fresh real StakeHub reserve sources;
4. transparently return and redeem the `1.000000 A666` position; and
5. privately return and redeem the `100.000000 A666` position.

The campaign proves that orders of different sizes create new A666 supply at
verified NAV without buying from the Uniswap pool, and that the resulting
Ethereum wA666 can later be returned and retired for subscription-funded
reserve principal.

`MUST`, `MUST NOT`, and `REQUIRED` are normative.

## 1. Product claim

A passing campaign supports this claim:

> A user with Ethereum USDC can acquire newly created A666 at the governed
> primary issue price, receive standard wA666 in an Ethereum wallet without
> consuming Uniswap liquidity, and later redeem it at the governed primary
> redemption price. The PFTL middle can be transparent or private, and the
> price is tied to a proof-backed StakeHub NAV rather than the Uniswap spot
> price.

This flow is not:

- an OTC transfer of existing A666;
- a purchase from an issuer or market maker;
- an owner mint;
- a swap through Uniswap;
- a withdrawal from Uniswap liquidity; or
- a redemption funded by a separate prefunded A666 or pfUSDC inventory
  bucket.

Primary issuance increases valid A666 supply and subscription-funded reserve
principal together. Primary redemption decreases both. Export, return,
shielding, and unshielding only move custody and MUST NOT change global A666
supply.

## 2. Privacy boundary

The two routes differ only in the PFTL middle:

| Stage | Transparent run | Private-middle run |
|---|---|---|
| Ethereum USDC deposit | Public | Public |
| PFTL pfUSDC/A666 activity | Transparent | Asset-Orchard notes, proofs, and nullifiers |
| Ethereum wA666 mint | Public | Public |
| Ethereum wA666 burn | Public | Public |
| Final Ethereum USDC release | Public | Public |

The private run MUST be described as **private middle**, not fully private
end to end. Amounts, Ethereum addresses, transaction hashes, timing, wA666
mint/burn events, and final USDC movements remain public.

Private note openings, note seeds, spending keys, and unencrypted private
witnesses MUST remain on the designated private execution host in mode
`0600`. They MUST NOT enter Git, the evidence bundle, logs, command lines, or
the resident service's public response.

## 3. Fixed run sizes

| Run ID | Mode | A666 output | Relative size |
|---|---|---:|---:|
| `T1` | transparent | `1.000000 A666` | `1×` |
| `P100` | private middle | `100.000000 A666` | `100×` |

The two runs MUST use independent workflow IDs, deposits, claims,
reservations, subscription nonces, notes, nullifiers, export packets, return
nonces, redemption nonces, egress packets, and Ethereum withdrawal IDs.

The 100× size demonstrates amount parameterization and materially different
capacity. It does not by itself prove the route's maximum configured order
size.

## 4. Canonical pricing

All amounts MUST be derived with checked integer arithmetic from the
policy-pinned finalized NAV. Floating point is forbidden in operation
builders, run manifests, consensus inputs, and acceptance assertions.

For A666 output quantity `Q_atoms`, finalized NAV `NAV_USD_E8`, and a
six-decimal settlement asset:

```text
base_value_atoms =
  ceil(Q_atoms * NAV_USD_E8 / 100_000_000)

issue_due_atoms =
  ceil(base_value_atoms * issue_multiplier_bps / 10_000)

redeem_out_atoms =
  floor(base_value_atoms * redeem_multiplier_bps / 10_000)
```

The active policy is expected to expose:

```text
issue_multiplier_bps  = 10050   # 1.005 × NAV
redeem_multiplier_bps =  9995   # 0.9995 × NAV
```

The live route readback controls. A mismatch blocks funding.

### 4.1 Planning example at the current mark

The current planning snapshot is `$0.90103113` per A666. This is not frozen
for execution; the pre-deposit manifest MUST use a fresh live read.

At that mark:

| Run | Base NAV value | Required USDC deposit | Issue spread |
|---|---:|---:|---:|
| `T1` | `0.901032` | `0.905538` | `0.004506` |
| `P100` | `90.103113` | `90.553629` | `0.450516` |
| Total | `91.004145` | `91.459167` | `0.455022` |

If the post-issuance StakeHub mark remained exactly `$0.90103113`, the
illustrative redemption outputs would be:

| Run | Redemption output |
|---|---:|
| `T1` | `0.900581 USDC` |
| `P100` | `90.058061 USDC` |

Actual redemption amounts MUST be recomputed from the newly finalized
post-issuance NAV. These examples MUST NOT be copied into a funded operation.

## 5. Gate 0 — implementation and pre-funding readiness

No Ethereum deposit may be sent until every item in this section passes and a
content-addressed campaign manifest is committed and pushed.

### 5.1 Required NAV-aware orchestration correction

The consensus transition already derives primary issue value from the
policy-pinned NAV. The pre-Gate-0 orchestration did not fully reflect that
behavior and MUST be corrected and regression-tested before funding:

- `scripts/a666-mainnet-primary-issue-ops.py` derived settlement as
  `mint_amount × issue_multiplier` without first applying NAV;
- `scripts/a666-mainnet-transparent-issue-after-deposit.sh` assumed
  settlement was at least the A666 atom count, derived spread as
  `settlement - mint`, and expected reserve principal to increase by the mint
  atom count; and
- the existing private issue runner consumes the same incorrect operation
  manifest, so this is a shared transparent/private defect.

The corrected issue builder MUST:

1. accept or obtain a finalized NAV manifest;
2. verify its epoch and reserve-packet hash against the live route;
3. derive `base_value_atoms`, `issue_due_atoms`, and spread with the canonical
   integer formulas;
4. put the exact `issue_due_atoms` in the reservation, subscription, deposit,
   and export packet;
5. record the exact `base_value_atoms` and spread in the immutable manifest;
6. reject a route/NAV/amount mismatch before deposit; and
7. support both `1.000000` and `100.000000 A666` without special-case
   branches.

Post-issue assertions MUST expect:

```text
authorized_valid_supply += Q_atoms
settlement_reserve       += base_value_atoms
non_nav_spread           += issue_due_atoms - base_value_atoms
outstanding_bridge_claims += Q_atoms after export
wA666 total supply        += Q_atoms after Ethereum mint
```

Required regression coverage:

- one-atom rounding boundaries for base, issue, and redemption arithmetic;
- NAV below, equal to, and above `$1.00`;
- `T1` and `P100` through the same builder path;
- stale or mismatched NAV epoch and reserve packet rejection;
- deposit amount differing by one atom rejection;
- private action amount differing from its issue manifest rejection; and
- reserve/spread assertions that use value atoms, not A666 quantity atoms.

### 5.2 Required runners

Before funding, the repository MUST provide resumable, amount-parameterized
runners for:

- transparent issue and export;
- private-middle issue and export;
- fresh StakeHub capture, proof, NAV finalization, and route epoch advance;
- transparent return, primary redemption, pfUSDC exit, and USDC withdrawal;
- private-middle return, primary redemption, pfUSDC exit, and USDC
  withdrawal; and
- final independent collection and reconciliation.

The transparent redemption path MUST not depend on reconstructing operations
manually during the funded run. The StakeHub path MUST not depend on manually
copying the prior campaign's source snapshot or proof.

Every runner MUST:

- write a durable workflow journal before its first mutation;
- detect already completed stages by canonical identifiers;
- refuse ambiguous or conflicting state;
- preserve a single value lineage across safe resume;
- record structured start/end timestamps and failure codes; and
- avoid overwriting prior evidence.

### 5.3 Fleet and service readiness

The frozen manifest MUST prove:

- all six validators have the same release, topology, finalized height, state
  root, and empty mempool;
- the live route is enabled, unpaused, invariant-holding, and bound to the
  intended A666, pfUSDC, wA666, controller, verifier, Ethereum chain, and pool;
- issue, redeem, export, return, and packet capacities exceed the two exact
  orders;
- the policy-pinned NAV is finalized, fresh, and identical on all validators;
- current A666 supply, route custody, bridge claims, wA666 supply, pfUSDC
  obligations, vault USDC, reserve principal, and spread reconcile;
- no stale runner, proof worker, pending wallet transaction, or unresolved
  value lineage exists;
- the ingress/export/egress SP1 provers are staged and healthy;
- the resident Asset-Orchard service is healthy, has the required proving key
  warm, and has not restarted unexpectedly;
- the funding wallet has exact principal plus a frozen gas budget; and
- the evidence root and all workflow IDs are new.

No code, binary, configuration, route, pricing policy, arithmetic, or
campaign-manifest change is allowed after the first deposit. The planned NAV
mark and its route-epoch advance are the only governed state changes allowed
between issuance and redemption.

## 6. Execution order

### Phase 1 — `T1` transparent deposit and issuance

```text
exact Ethereum USDC deposit
  -> finalized proof-gated pfUSDC ingress
  -> transparent PFTL pfUSDC claim
  -> governed primary reservation/subscription
  -> exactly 1.000000 new A666
  -> transparent A666 export
  -> finalized PFTL receipt proof
  -> exactly 1.000000 wA666 in the user Ethereum wallet
  -> proof-backed destination-consume record on PFTL
```

Pass conditions:

- the deposit and claim execute exactly once;
- the user is observed holding `1.000000 A666` on PFTL before export;
- valid A666 supply and reserve principal increase by the exact canonical
  deltas;
- export changes custody but not valid supply;
- wA666 supply and the user's wA666 balance increase by exactly `1.000000`;
- no Uniswap swap, pool balance, or LP position funds the issuance;
- destination consume is recorded exactly once; and
- the operation completes without post-deposit intervention.

Phase 2 cannot be funded until Phase 1 is fully reconciled.

### Phase 2 — `P100` private-middle deposit and issuance

```text
exact Ethereum USDC deposit
  -> finalized proof-gated pfUSDC ingress
  -> PFTL pfUSDC claim boundary
  -> private pfUSDC note
  -> private-primary issue
  -> private A666 note
  -> private A666 egress to the bound export account
  -> transparent A666 export
  -> finalized PFTL receipt proof
  -> exactly 100.000000 wA666 in the user Ethereum wallet
  -> proof-backed destination-consume record on PFTL
```

Pass conditions:

- the private pfUSDC input is consumed exactly once;
- its nullifier is unique and replay rejects without mutation;
- private primary issuance increases valid A666 supply by exactly
  `100.000000`;
- reserve principal and spread increase by the canonical value deltas;
- the private A666 output is recoverable by a fresh note scan;
- no transparent user A666 exists before private egress;
- export changes custody but not valid supply;
- wA666 supply and the user's balance increase by exactly `100.000000`;
- no owner mint, OTC inventory, market-maker inventory, Uniswap trade, or LP
  withdrawal funds the acquisition; and
- the operation completes without post-deposit intervention.

Phase 3 cannot start until both Ethereum wA666 positions are spendable and
both issue lineages reconcile independently.

### Phase 3 — fresh real StakeHub NAV mark

The NAV mark MUST use fresh production source observations and the governed
StakeHub aggregate proof path. It MUST NOT reuse the prior campaign's proof,
public values, source snapshot, or manually entered NAV.

The job MUST:

1. collect the governed production reserve legs from their real sources;
2. record source identifiers, observation heights/timestamps, freshness,
   ownership evidence, and source-specific proof inputs;
3. build and verify the production aggregate SP1 proof;
4. record the proof profile, program vkey, proof hash, public-values hash,
   source root, and attestor root;
5. derive proof-backed portfolio net assets under the active valuation
   policy;
6. include the current subscription-funded route reserve exactly once;
7. exclude issue spread and all disclosed non-NAV fee custody;
8. prevent vault USDC from being counted both as a StakeHub source and as a
   route-reserve overlay;
9. use post-issuance valid A666 supply as the denominator;
10. derive NAV with the production integer scale and rounding;
11. submit and finalize the next reserve packet and NAV epoch through normal
    PFTL consensus;
12. advance the route policy to the new NAV epoch and reserve packet through
    the governed route transition; and
13. confirm identical final height, NAV, packet, policy, and state root on all
    six validators.

The report MUST independently recompute:

```text
post_issue_supply =
  baseline_valid_supply + 1.000000 A666 + 100.000000 A666

post_issue_net_assets =
  fresh governed StakeHub portfolio value
  + subscription-funded route reserve not already counted
  - governed liabilities
```

Uniswap price and liquidity MUST NOT enter the NAV calculation.

If NAV moves, the report MUST attribute the movement to source prices,
balances, liabilities, reserve changes, freshness, or deterministic rounding.
The campaign MUST report the observed value; it must not force NAV to remain
at `$1.00` or at the prior epoch's mark.

Fresh transparent and private redemption manifests MUST be built only after
the new NAV and route epoch finalize. No Ethereum wA666 may be burned before
those manifests pass preflight.

### Phase 4 — `T1` transparent withdrawal

```text
1.000000 wA666 burn on Ethereum
  -> finalized return proof
  -> transparent A666 return on PFTL
  -> transparent primary redemption at 0.9995 × refreshed NAV
  -> exact transparent pfUSDC
  -> proof-gated pfUSDC exit
  -> exact spendable Ethereum USDC
```

Pass conditions:

- burn, return import, redemption, pfUSDC exit, and USDC release each execute
  exactly once;
- returned A666 is observed before redemption;
- wA666 supply, bridge claims, and valid A666 supply each decrease by exactly
  `1.000000`;
- reserve principal decreases by the refreshed base NAV value;
- the user receives the exact integer redemption output;
- no issuer signature, inventory transfer, Uniswap trade, or LP withdrawal is
  used;
- all replay attempts reject without mutation; and
- the operation completes without post-burn intervention.

Phase 5 cannot start until Phase 4 is fully reconciled.

### Phase 5 — `P100` private-middle withdrawal

```text
100.000000 wA666 burn on Ethereum
  -> finalized return proof
  -> PFTL A666 return boundary
  -> private A666 note
  -> private-primary redemption at 0.9995 × refreshed NAV
  -> private pfUSDC note
  -> private pfUSDC egress
  -> proof-gated pfUSDC exit
  -> exact spendable Ethereum USDC
```

Pass conditions:

- the returned A666 enters private custody exactly once;
- the A666 nullifier and redemption nonce are unique;
- private primary redemption retires exactly `100.000000 A666`;
- reserve principal decreases by the refreshed base NAV value;
- the private pfUSDC output equals the exact integer redemption result;
- the intended output is recoverable by a fresh note scan;
- private egress consumes it exactly once;
- the exact pfUSDC value is released as spendable Ethereum USDC;
- replay of return, redemption, private egress, proof exit, or Ethereum
  withdrawal rejects without mutation;
- no issuer discretion, prefunded redemption bucket, inventory transfer,
  Uniswap trade, or LP withdrawal is used; and
- the operation completes without post-burn intervention.

## 7. Conservation and final state

Independent before/after readbacks MUST prove:

```text
A666 valid supply
  = transparent A666
  + Asset-Orchard A666 custody
  + outstanding bridge claims
  + enumerated canonical nonspendable custody
```

```text
wA666 total supply
  = finalized outstanding PFTL bridge claims
```

```text
pfUSDC obligations
  = transparent pfUSDC
  + Asset-Orchard pfUSDC custody
  + accepted deposits not yet claimed
  + burned exits not yet released
  - canonical released terms
```

At terminal state:

- valid A666 supply returns exactly to the campaign baseline;
- wA666 total supply and outstanding bridge claims return exactly to baseline;
- the user's campaign wA666 balance returns exactly to baseline;
- active reservations, export entitlements, return claims, redemptions,
  egresses, and withdrawals for both lineages are zero;
- route reserve principal equals baseline plus issue base values minus
  refreshed-NAV redemption base values;
- non-NAV spread equals baseline plus both issue spreads and both redemption
  spreads;
- pfUSDC and Ethereum vault conservation hold exactly;
- all six validators converge on one final height and state root with empty
  mempools; and
- Uniswap liquidity is unchanged by this campaign, except for independently
  identified third-party market activity.

The user's net USDC result will reflect the disclosed issue/redemption spreads,
any NAV movement between issue and redemption, and Ethereum gas. It is not
expected to equal the starting USDC balance.

## 8. Timing gate

Each user value-moving leg has a hard 25-minute wall-time target measured from
authoritative Ethereum block timestamps:

```text
deposit inclusion -> spendable wA666       <= 1,500 seconds
```

```text
wA666 burn inclusion -> spendable USDC     <= 1,500 seconds
```

This produces four separately scored legs:

- `T1` transparent issue;
- `P100` private-middle issue;
- `T1` transparent redemption; and
- `P100` private-middle redemption.

The StakeHub NAV job is measured and reported separately. It does not pause or
erase an issue/redemption timer.

The timing report MUST break out Ethereum finality, proof queue, proving,
PFTL consensus, Asset-Orchard proving, export/return proving, Ethereum
submission, and retries. A functional result over 1,500 seconds is a latency
failure.

## 9. Failure and recovery

On validator divergence, state-root mismatch, stale NAV, proof mismatch,
unexpected amount, duplicate identifier, accounting failure, or ambiguous
workflow state, the runner MUST stop before the next mutation and emit
`RECOVERY_REQUIRED`.

It MUST NOT:

- create another deposit to work around an unresolved deposit;
- burn replacement wA666 for an unresolved return;
- rebuild a new note to replace an unresolved private lineage;
- edit a validator ledger;
- bypass proof or replay verification;
- change arithmetic or manifests after funding;
- use an owner mint, operator inventory, discretionary payment, OTC fill,
  Uniswap trade, or LP withdrawal as recovery; or
- label a manually repaired run as intervention-free.

A recovery packet MUST state the last successful mutation, first failed
operation, exact current custody, canonical identifiers, safe next operation,
and the condition required to resume.

## 10. Evidence package

Write one new immutable root:

```text
docs/evidence/a666-variable-size-real-nav-roundtrip-20260729/
```

Required layout:

```text
campaign-manifest.json
baseline/
t1-transparent-issue/
p100-private-issue/
stakehub-nav-mark/
t1-transparent-redeem/
p100-private-redeem/
final/
acceptance-summary.json
artifact-sha256.txt
README.md
```

Each value-moving leg MUST contain:

- immutable operation manifest;
- Ethereum transaction, receipt, block number, and timestamp;
- proof reports and hashes;
- PFTL consensus certificates and six-validator finality;
- before/after balances, supply, reserve, custody, and claims;
- replay results;
- structured timing;
- intervention log; and
- terminal or recovery status.

The NAV directory MUST contain fresh source observations, freshness evidence,
aggregate witness, proof/public values, verification report, valuation
reconciliation, overlay/no-double-count report, NAV operations, route epoch
advance, and six-validator convergence.

The final machine result MUST include:

```json
{
  "schema": "postfiat.a666.variable_size_real_nav_roundtrip.v1",
  "verdict": "PASS|FAIL|RECOVERY_REQUIRED",
  "t1_transparent_issue_pass": false,
  "p100_private_issue_pass": false,
  "real_stakehub_nav_pass": false,
  "t1_transparent_redeem_pass": false,
  "p100_private_redeem_pass": false,
  "four_leg_slo_pass": false,
  "supply_conservation_pass": false,
  "reserve_conservation_pass": false,
  "wrapper_conservation_pass": false,
  "pfusdc_conservation_pass": false,
  "replay_rejection_pass": false,
  "six_validator_convergence_pass": false,
  "uniswap_liquidity_consumed_atoms": 0,
  "intervention_free_after_first_deposit": false
}
```

## 11. Acceptance gates

| Gate | Pass condition |
|---|---|
| `G0` — ready | NAV-aware builders/runners pass tests; identities, fleet, accounting, capacities, wallet, provers, and recovery paths are frozen and pushed before funding. |
| `G1` — small transparent issue | Exact deposit creates `1 A666`, exports `1 wA666`, consumes no pool liquidity, and finishes hands-off within 1,500 seconds. |
| `G2` — larger private issue | Exact deposit privately creates `100 A666`, exports `100 wA666`, rejects replays, consumes no pool liquidity, and finishes hands-off within 1,500 seconds. |
| `G3` — real NAV | Fresh governed StakeHub sources and proof produce an independently recomputable NAV and route epoch finalized identically by all six validators. |
| `G4` — transparent withdrawal | Exactly `1 A666` is returned and retired for exact refreshed-NAV USDC, hands-off within 1,500 seconds. |
| `G5` — private withdrawal | Exactly `100 A666` is privately returned and retired for exact refreshed-NAV USDC, with proof/replay checks, hands-off within 1,500 seconds. |
| `G6` — reconciled | Supply, reserve, custody, bridge, wrapper, vault, spread, replay, Uniswap non-consumption, and validator-state checks all pass. |

Failure at a gate blocks the next value-moving gate. Safe diagnostic
collection and recovery of the existing lineage may continue.

## 12. Definition of done

The campaign is complete only when:

- both deposits use the exact NAV-derived input values;
- the transparent run creates and exports exactly `1 A666`;
- the private-middle run creates and exports exactly `100 A666`;
- both acquisitions consume zero Uniswap liquidity;
- a fresh real StakeHub proof and post-issuance NAV are finalized before any
  burn;
- the transparent position is returned and redeemed first;
- the private-middle position is returned and redeemed second;
- both redemptions use the new NAV and exact integer policy arithmetic;
- all four value-moving legs pass the 25-minute gate;
- no code/config/policy repair occurs after the first deposit;
- all replay and conservation checks pass;
- all six validators converge;
- the complete non-secret evidence bundle is committed and pushed; and
- `acceptance-summary.json` reports `verdict: "PASS"`.
