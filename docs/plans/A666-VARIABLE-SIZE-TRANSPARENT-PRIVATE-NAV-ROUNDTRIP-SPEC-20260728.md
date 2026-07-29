# A666 Variable-Size Transparent/Private NAV Round-Trip Spec

**Date:** 2026-07-28
**Priority:** P0
**Status:** executed — business flow PASS, release gate FAIL
**Parent economics:** `A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`
**Parent acceptance:** `A666-TRANSPARENT-PRIVATE-ISSUE-REDEEM-ACCEPTANCE-SPEC-20260728.md`
**Starting checkpoint:** commit `367a449ae20cbaed8652935690ea45420a2b0646`,
workflow `a666-p9-20260728`

This campaign proves one complete variable-size product flow:

1. one small transparent primary acquisition;
2. one materially larger private-middle primary acquisition;
3. a fresh A666 NAV calculation from real StakeHub reserve sources while both
   positions are outstanding;
4. transparent primary redemption of the small position; and
5. private-middle primary redemption of the larger position.

The campaign demonstrates that primary-market size changes supply and reserve
principal without trading through the wA666/USDC Uniswap pool. It also proves
that NAV comes from the governed StakeHub reserve calculation rather than the
pool price.

`MUST`, `MUST NOT`, and `REQUIRED` are normative. This document does not
change the canonical issue or redemption formulas, trust boundaries, route
limits, proof systems, or privacy claims defined by the parent specifications.

Execution evidence is in
`../evidence/a666-variable-size-nav-roundtrip-20260728/`. The live campaign
reached a safe, fully reconciled terminal state at PFTL height 440. Functional
issuance, real StakeHub NAV marking, transparent redemption, and private-middle
redemption passed. The top-level release verdict is `FAIL` because measured
fresh operations exceeded the 1,500-second SLO and the run was not
intervention-free.

## 1. User-visible outcome

At a `$1.00` finalized NAV, the target flow is:

```text
Small transparent path
  1.005000 Ethereum USDC
    -> 1.000000 newly issued A666
    -> 1.000000 Ethereum wA666

Larger private-middle path
  100.500000 Ethereum USDC
    -> private pfUSDC settlement on PFTL
    -> 100.000000 newly issued private A666
    -> 100.000000 Ethereum wA666

Fresh real StakeHub reserve proof and A666 NAV mark

Transparent redemption
  1.000000 wA666
    -> 0.999500 Ethereum USDC at unchanged $1.00 NAV

Private-middle redemption
  100.000000 wA666
    -> private A666 redemption on PFTL
    -> private pfUSDC
    -> 99.950000 Ethereum USDC at unchanged $1.00 NAV
```

The `$1.00` figures are examples, not hardcoded pricing authority. Every live
amount MUST be derived from the finalized NAV and canonical integer formulas.

The business claim established by a pass is:

> A user can acquire newly created A666 at verified NAV in both transparent
> and private-middle modes, in meaningfully different sizes, receive wA666 in
> an Ethereum wallet without consuming Uniswap liquidity, and redeem both
> positions against subscription-funded reserve principal.

This is not OTC and does not transfer existing A666 inventory to the buyer.

## 2. Privacy boundary and terminology

Ethereum deposits and withdrawals are public. Ethereum addresses, token
amounts, transaction hashes, block times, wA666 mint/burn events, and USDC
movements remain observable.

For this campaign:

- **transparent deposit** means the PFTL pfUSDC claim, A666 subscription,
  A666 custody, export, return, and redemption are transparent;
- **private deposit** means the Ethereum deposit remains public, but the PFTL
  settlement and primary-issuance middle uses Asset-Orchard private notes and
  the production private-primary issue action;
- **private withdrawal** means the Ethereum wA666 burn and final USDC release
  remain public, while the returned A666, primary redemption, and pfUSDC
  middle use Asset-Orchard private notes and proofs; and
- **MetaMask/Uniswap integration** means the user receives standard ERC-20
  wA666 at the intended Ethereum address and it is transferable and eligible
  for the deployed wA666/USDC Uniswap v4 pool. MetaMask is a wallet, not the
  pool.

The evidence and product copy MUST use “private middle,” not “fully private
end to end.”

## 3. Run matrix

### 3.1 Fixed output sizes

| Run | Mode | A666 output | Relative size | Required end state before NAV mark |
|---|---|---:|---:|---|
| `T-SMALL` | Transparent | `1.000000 A666` | `1×` | `1.000000 wA666` spendable on Ethereum |
| `P-LARGE` | Private middle | `100.000000 A666` | `100×` | `100.000000 wA666` spendable on Ethereum |

The 100× run proves that the same primary-market path handles different order
sizes without AMM slippage. It does not by itself prove the configured
`1,000,000 A666` maximum single-order capacity. Capacity scaling follows only
after this correctness campaign passes.

### 3.2 Canonical pricing

For output quantity `Q` and finalized NAV numerator/denominator `Nn/Nd`:

```text
base_value_atoms =
  ceil(Q_atoms * Nn / Nd)

issue_due_atoms =
  ceil(base_value_atoms * 10050 / 10000)

redeem_out_atoms =
  floor(base_value_atoms * 9995 / 10000)
```

All arithmetic MUST use checked integers. Floating point is forbidden in
manifests, consensus inputs, assertions, and acceptance calculations.

If NAV is exactly `$1.00`, the expected campaign values are:

| Run | Deposit | A666 issued | Redemption output | Round-trip spread |
|---|---:|---:|---:|---:|
| `T-SMALL` | `1.005000 USDC` | `1.000000` | `0.999500 USDC` | `0.005500 USDC` |
| `P-LARGE` | `100.500000 USDC` | `100.000000` | `99.950000 USDC` | `0.550000 USDC` |
| Total | `101.505000 USDC` | `101.000000` | `100.949500 USDC` | `0.555500 USDC` |

If the post-issuance StakeHub mark changes NAV, redemption outputs MUST use
the refreshed finalized NAV. The summary MUST show both the actual result and
the `$1.00` comparison; it MUST NOT force the comparison values to pass.

### 3.3 Independent lineages

`T-SMALL` and `P-LARGE` MUST have distinct:

- workflow IDs;
- Ethereum deposit IDs and nonces;
- PFTL ingress claims;
- primary reservations and subscription nonces;
- Asset-Orchard notes and nullifiers;
- export packet hashes and Ethereum packet digests;
- return nonces;
- redemption IDs and nonces;
- pfUSDC egress packets; and
- Ethereum withdrawal identifiers.

No transition from one run may be reused to complete the other.

## 4. Starting-state rule

The existing Phase 9 deposit is the `T-SMALL` deposit:

```text
workflow:    a666-p9-20260728
amount:      1.005000 USDC
deposit ID:  0xc1b73435029d42ebace223a0970d837736862da6569c9cf38cb7cef5c5ba5682
transaction: 0x88f4c9ffc95568e1c44f422d8e7ba2162da70fb1fb753fd43b45458fd6cf4a48
```

Its recovery checkpoint is:

`../evidence/a666-acceptance-20260728/phase-9-private-redeem-hands-off-verify/recovery-checkpoint-20260728T203650Z.json`

The campaign MUST resume that exact deposit lineage. It MUST NOT create
another small deposit while the existing one is unresolved.

If a frozen downstream reservation, export packet, or deadline has expired,
the campaign MUST stop at `RECOVERY_REQUIRED` and use a separately reviewed
recovery transition. It MUST NOT bypass the expiry, pretend the old intent is
fresh, or create a replacement deposit.

Because the existing deposit was deliberately paused, `T-SMALL` can establish
the functional transparent flow but cannot retroactively satisfy an
uninterrupted deposit-to-wA666 latency claim. The machine result MUST expose
this distinction. A later fresh run is required only for a clean release-SLO
claim; it is not required merely to complete this business-flow campaign.

`P-LARGE` MUST NOT be funded until:

1. the Phase 9 deposit record still matches the checkpoint;
2. all six validators still converge on the checkpoint state or a fully
   explained successor state;
3. the deposit has not already been relayed or claimed;
4. `T-SMALL` reaches spendable Ethereum wA666;
5. all `T-SMALL` supply, reserve, bridge, and wrapper deltas reconcile; and
6. no active runner, stale proof job, or ambiguous workflow state remains.

## 5. Pre-funding controls

Before any new `P-LARGE` value moves, freeze one content-addressed campaign
manifest containing:

- repository commit and orchestration-script hashes;
- validator binary, topology, release, chain, genesis, and protocol hashes;
- matching status, height, state root, and empty mempool from all six
  validators;
- production A666, pfUSDC, wA666, vault, verifier, controller, route, pool,
  policy, and Asset-Orchard identifiers;
- live issue, redemption, export, and wrapped-exposure capacities;
- current NAV epoch, reserve packet, NAV value, freshness, and policy hash;
- exact `P-LARGE` issue arithmetic and maximum USDC debit;
- exact user PFTL and Ethereum recipients;
- Ethereum wallet USDC, ETH, allowance, and nonce state;
- prover binary, vkey, ELF, CUDA readiness, and remote work-directory hashes;
- the before-state accounting defined in Section 11;
- per-stage timeout and recovery checkpoint; and
- evidence paths and workflow IDs.

The manifest MUST fail closed before deposit if:

- the wallet lacks exact principal plus the frozen gas budget;
- an Ethereum transaction is pending from the funding address;
- any validator disagrees on release or state;
- a required prover is cold, missing, or already running an ambiguous job;
- any policy, NAV, route, verifier, controller, or pool binding differs;
- issue or redemption capacity is insufficient;
- the wA666/USDC pool is missing or has zero active liquidity;
- the standard witness/proof path fails a read-only dry run; or
- current supply, reserve, custody, vault, or wrapper accounting does not
  reconcile.

No release, binary, configuration, route, pricing arithmetic, or script
change is allowed after the `P-LARGE` deposit unless the run is explicitly
marked `RECOVERY_REQUIRED`. The only permitted governed state change before
redemption is the StakeHub NAV epoch—and, if required, its predeclared policy
reference rollover—frozen in the campaign manifest.

### 5.1 Required orchestration support

Before `P-LARGE` funding, the repository MUST provide:

- amount-parameterized transparent and private issue/redeem orchestration;
- checked derivation of settlement, output, and spread from quantity and NAV;
- rejection of any script/runtime amount mismatch;
- a reusable real StakeHub capture/prove/NAV builder that accepts the current
  epoch, live supply, proof, public values, and governed source policy;
- rejection of the hardcoded opening epoch, opening proof hashes, opening net
  assets, or opening supply as inputs to the new NAV mark;
- a persisted workflow journal with one terminal or resumable state for every
  external and PFTL mutation;
- idempotent detection of already completed deposit, claim, reservation,
  private action, export, mint, burn, return, redemption, egress, and
  withdrawal stages;
- one collector that computes all cross-system invariants from independent
  live readbacks; and
- one runner that produces the machine summary without reconstructing
  timestamps or interventions manually.

The currently proven one-unit scripts may be reused as components, but
changing literal `1.000000`/`0.999500` assumptions by hand during the funded
run is forbidden.

## 6. Ordered execution

### Phase 0 — Baseline

Record independently:

- Ethereum USDC wallet and vault balances;
- vault obligations and pending deposit records;
- user wA666 balance and total wA666 supply;
- Uniswap pool ID, active liquidity, balances, price, and observation block;
- A666 valid supply, transparent balances, private custody, reserve
  principal, spread account, reservations, export entitlements, and
  outstanding bridge claims;
- pfUSDC transparent supply, private custody, pending claims, burns, and
  releases;
- all six validator heights and state roots; and
- the current real StakeHub reserve packet, public values, verified net
  assets, valid supply denominator, and finalized NAV.

The baseline StakeHub read is required even though the requested new mark
occurs after issuance. It provides the before-value needed to explain the
after mark.

### Phase 1 — `T-SMALL` transparent issuance

Resume the exact Phase 9 Ethereum ingress capture and proof lineage:

```text
existing public Ethereum USDC deposit
  -> finalized ingress proof
  -> transparent PFTL pfUSDC claim
  -> transparent primary reservation/subscription
  -> 1.000000 new transparent A666
  -> transparent A666 export
  -> finalized PFTL receipt proof
  -> 1.000000 wA666 on Ethereum
  -> proof-backed destination-consume record on PFTL
```

Required assertions:

- the deposit is claimed exactly once;
- A666 valid supply increases by exactly `1.000000`;
- base reserve principal increases by the canonical base value;
- issue spread increases by `issue_due - base_value`;
- the user holds the new A666 on PFTL before export;
- export changes custody but not global A666 supply;
- outstanding bridge claims and wA666 supply each increase by `1.000000`;
- the intended Ethereum wallet balance increases by `1.000000 wA666`;
- the wrapper packet and destination consume are recorded exactly once;
- Uniswap liquidity and pool balances do not fund or change because of the
  primary fill; and
- wA666 is transferable by `eth_call` and eligible for the deployed pool.

### Phase 2 — `P-LARGE` private-middle issuance

After Phase 1 fully reconciles, create one new `P-LARGE` deposit for the exact
policy-priced input required to issue `100.000000 A666`.

```text
public Ethereum USDC deposit
  -> finalized ingress proof
  -> public PFTL pfUSDC claim boundary
  -> Asset-Orchard ingress
  -> private pfUSDC note
  -> private-primary A666 issue
  -> private A666 note
  -> private egress to the bound export account
  -> public A666 export
  -> finalized PFTL receipt proof
  -> 100.000000 public wA666 on Ethereum
  -> proof-backed destination-consume record on PFTL
```

Required assertions:

- the exact private pfUSDC input is consumed once;
- its nullifier is unique and rejects replay;
- private-primary issue increases global A666 valid supply by exactly
  `100.000000`;
- reserve principal and issue spread change by the canonical amounts;
- the private output note is recoverable after a fresh scan;
- no transparent user A666 balance is created before private egress;
- private egress creates only the exact exportable amount;
- export changes custody but not global supply;
- outstanding bridge claims and wA666 supply each increase by `100.000000`;
- the Ethereum wallet receives exactly `100.000000 wA666`;
- no owner mint, OTC inventory, Uniswap swap, LP withdrawal, or pool transfer
  funds the acquisition; and
- rejected/replayed private actions do not change anchors, nullifiers,
  commitments, supply, reserve, or capacity.

### Phase 3 — Real StakeHub NAV recalculation

This phase occurs after both Ethereum wA666 balances are spendable and before
either redemption burn.

The NAV job MUST:

1. run the production StakeHub six-leg reserve collection and aggregate proof
   path, not a fixture, cached opening packet, or manually entered price;
2. record source identifiers, source block/timestamp freshness, proof profile,
   program vkey, proof hash, and public-values hash;
3. compute verified net assets from the governed production policy;
4. include counted primary settlement reserve exactly once;
5. exclude issue spread and other disclosed non-NAV fee custody;
6. avoid double-counting vault USDC if it is already represented in a
   StakeHub cash/source leg;
7. use the post-issuance valid A666 supply denominator;
8. derive NAV using the production integer scale and rounding rule;
9. submit and finalize a new reserve packet and NAV epoch through normal PFTL
   consensus; and
10. confirm all six validators finalize the identical packet, NAV, height, and
    state root.

The acceptance calculation is:

```text
post_issue_valid_supply = baseline_valid_supply + 101.000000 A666

post_issue_verified_net_assets =
  governed live StakeHub asset components
  + counted primary settlement reserve not already in those components
  - governed liabilities
```

The collector MUST report every included component so the formula can be
independently recomputed. The exact source composition from the governed
StakeHub policy controls.

If markets and liabilities are unchanged, adding base reserve and valid
supply proportionally should preserve NAV apart from canonical integer
rounding. If NAV changes, the report MUST attribute the delta to:

- live portfolio price movement;
- cash or liability movement;
- primary settlement reserve;
- source freshness/availability; or
- deterministic rounding.

Uniswap spot price, pool liquidity, and the primary issue spread MUST NOT
enter the NAV calculation.

Before either burn, obtain fresh transparent and private redemption quotes
against the newly finalized NAV epoch and reserve packet. If policy rollover
is required to reference that mark, it MUST finalize before the user burns
wA666 and MUST preserve the posted `0.9995 × NAV` rule.

### Phase 4 — `T-SMALL` transparent redemption

```text
1.000000 wA666 burn on Ethereum
  -> finalized return proof
  -> transparent A666 return on PFTL
  -> transparent primary redemption at refreshed NAV × 0.9995
  -> exact transparent pfUSDC
  -> proof-native pfUSDC exit
  -> spendable Ethereum USDC
```

Required assertions:

- the burn, return, redemption, pfUSDC exit, and Ethereum release each execute
  exactly once;
- wA666 supply and outstanding bridge claims decrease by `1.000000`;
- returned A666 is observable before redemption;
- A666 valid supply decreases by exactly `1.000000`;
- reserve principal decreases by the refreshed base redemption value;
- the user receives the exact integer `redeem_out_atoms`;
- redemption spread is posted to non-NAV accounting;
- no issuer completion signature, Uniswap trade, or LP withdrawal is used;
- the final Ethereum USDC is spendable by the intended wallet; and
- exact replay attempts reject without state advancement.

### Phase 5 — `P-LARGE` private-middle redemption

Phase 5 starts only after Phase 4 reaches a reconciled terminal state.

```text
100.000000 wA666 burn on Ethereum
  -> finalized return proof
  -> public A666 return boundary on PFTL
  -> Asset-Orchard ingress
  -> private A666 note
  -> private-primary redemption at refreshed NAV × 0.9995
  -> private pfUSDC note
  -> private pfUSDC egress
  -> proof-native pfUSDC exit
  -> spendable Ethereum USDC
```

Required assertions:

- the returned A666 enters private custody exactly once;
- the private A666 nullifier is unique;
- private-primary redemption retires exactly `100.000000 A666`;
- reserve principal decreases by the canonical refreshed base value;
- the private pfUSDC output equals the exact integer redemption price;
- the intended output is recoverable after a fresh note scan;
- private egress consumes the output once and creates only the exact public
  exit value;
- A666 supply, wA666 supply, bridge claims, pfUSDC supply, custody, vault
  obligations, and USDC release all reconcile;
- replay of the private redemption, return import, egress, or Ethereum
  withdrawal rejects without mutation; and
- no issuer discretion, prefunded redemption bucket, Uniswap trade, LP
  withdrawal, or operator A666 inventory is used.

### Phase 6 — Final reconciliation

After both withdrawals:

- global A666 valid supply MUST equal the baseline supply;
- wA666 total supply and outstanding bridge claims MUST equal their baseline
  values;
- active reservations and export entitlements MUST be zero;
- no `T-SMALL` or `P-LARGE` return, redemption, egress, or withdrawal may
  remain pending;
- user campaign wA666 MUST return to its baseline value;
- counted primary reserve principal MUST reconcile under the two issue and
  two refreshed-NAV redemption transitions;
- non-NAV spread accounting MUST equal the independently calculated issue and
  redemption spreads;
- pfUSDC vault conservation MUST hold;
- all six validators MUST converge with empty mempools; and
- the official Uniswap position and active liquidity MUST be unchanged except
  for unrelated third-party market activity identified by block and event.

If NAV remained `$1.00`, the two user round trips together leave exactly
`0.555500 USDC` of disclosed non-NAV spread. A different finalized redemption
NAV produces a different result and MUST be reconciled from the canonical
integer formulas rather than forced to this example.

## 7. Cross-system invariants

Every phase MUST collect independent before/after readbacks proving:

```text
pfUSDC vault obligations
  = transparent pfUSDC
  + private pfUSDC custody
  + accepted deposits not yet claimed
  + burned exits not yet released
  - canonical released terms
```

```text
A666 valid supply
  = transparent A666
  + private A666 custody
  + outstanding bridge claims
  + enumerated canonical nonspendable custody
```

```text
wA666 total supply
  = finalized outstanding PFTL bridge claims
```

Primary issuance changes A666 supply and reserve principal together. Export,
return, shielding, and private egress only move custody and MUST NOT change
global supply. Primary redemption changes A666 supply and reserve principal
together in the opposite direction.

No acceptance collector may infer global supply from transparent wallet
balances alone.

## 8. Timing and intervention rules

For each fresh issue run:

```text
included Ethereum deposit -> spendable Ethereum wA666 <= 1,500 seconds
```

For each redemption:

```text
included Ethereum wA666 burn -> spendable Ethereum USDC <= 1,500 seconds
```

Record separately:

- Ethereum inclusion and finality;
- ingress capture, proof queue, proving, and verification;
- PFTL consensus transitions;
- Asset-Orchard proving and verification;
- export/return proof generation;
- Ethereum receipt acceptance and mint/release; and
- retry, recovery, and operator time.

The real StakeHub collection/proof/NAV-finalization duration is reported
separately and does not erase or pause a user issue/redemption timer.

The paused historical interval for the existing `T-SMALL` deposit MUST remain
visible. Its functional completion may pass while its uninterrupted issue SLO
is `NOT_MEASURED_FROM_FRESH_START` or `FAIL`; it may not be rewritten as a
25-minute run.

After the `P-LARGE` deposit and after each redemption burn:

- no code, release, config, route, policy arithmetic, or manifest change is
  permitted;
- no one-off witness editing or manual ledger repair is permitted;
- a persisted workflow may resume from a proven idempotent checkpoint; and
- any unplanned operator action sets `intervention_free: false`.

## 9. Failure and recovery behavior

The campaign is safety-first. On validator divergence, state-root mismatch,
accounting mismatch, stale or conflicting proof, duplicate identifier,
unexpected mutation, or ambiguous workflow state: stop before the next value
transition and emit `RECOVERY_REQUIRED`.

Specific recovery rules:

- Never create another deposit to work around a failed deposit relay.
- Never submit a second claim, reservation, export, return, or redemption
  until the first identifier is proven absent or terminal.
- If private issuance fails after deposit, preserve and recover that exact
  deposit; do not relabel a transparent fallback as a private pass.
- If the StakeHub NAV job fails, do not burn either wA666 position. The user
  retains transferable Ethereum assets while the mark is retried or reviewed.
- If a return fails after burn, preserve the return nonce and receipt lineage;
  do not burn replacement wA666.
- If private redemption or egress fails, preserve the exact note/nullifier and
  resume from its last non-mutating checkpoint.
- Uniswap sale, LP withdrawal, OTC settlement, owner mint, direct ledger edit,
  or discretionary operator payment is not an allowed recovery path.

Every failure packet MUST identify the last successful mutation, the first
failed operation, current asset custody, exact safe next operation, and the
condition required before resumption.

## 10. Adversarial checks

Before the live `P-LARGE` run, production-equivalent tests MUST reject without
mutation:

- replayed deposit, claim, private ingress, private issue, private egress,
  export, mint, burn, return, private redeem, pfUSDC exit, and withdrawal;
- wrong amount at `1×` versus `100×`;
- one atom below/at/above issue and redemption rounding boundaries;
- wrong StakeHub policy, vkey, public values, source root, NAV epoch, reserve
  packet, supply denominator, or source freshness;
- double-counted primary reserve in the NAV calculation;
- inclusion of the issue spread in NAV assets;
- substitution of Uniswap price for verified NAV;
- stale Asset-Orchard anchor or duplicate nullifier;
- wrong Ethereum/PFTL recipient or route binding;
- expired reservation, packet, policy, or redemption quote;
- malformed or oversized SP1/Halo2 proof inputs;
- proof-worker restart after capture and after proof completion; and
- one-validator outage plus below-quorum no-advance behavior.

The two sizes MUST exercise the same canonical arithmetic path. A special
one-unit branch or a hardcoded 100-unit branch is a test failure.

## 11. Evidence package

Write one immutable campaign root:

```text
docs/evidence/a666-variable-size-nav-roundtrip-20260728/
```

Required contents:

```text
campaign-manifest.json
baseline/
t-small-transparent-issue/
p-large-private-issue/
stakehub-nav-mark/
t-small-transparent-redeem/
p-large-private-redeem/
final/
artifact-sha256.txt
acceptance-summary.json
README.md
```

The StakeHub directory MUST contain:

- source observations and freshness;
- aggregate proof report;
- public values;
- proof profile, vkey, and hashes;
- verified-net-assets calculation;
- primary-reserve inclusion and no-double-count report;
- pre/post valid supply;
- derived NAV with integer scale/rounding;
- reserve packet and finalized NAV epoch; and
- six-validator convergence evidence.

Each issue/redemption directory MUST contain transaction and block
identifiers, proof reports, consensus certificates, balances, supply/reserve
deltas, timing, replay results, and an intervention log.

The final machine summary MUST include at least:

```json
{
  "schema": "postfiat.a666.variable_size_nav_roundtrip_acceptance.v1",
  "verdict": "PASS|FAIL|RECOVERY_REQUIRED",
  "business_flow_pass": false,
  "release_slo_pass": false,
  "t_small": {
    "transparent_issue_pass": false,
    "transparent_redeem_pass": false,
    "issue_slo": "PASS|FAIL|NOT_MEASURED_FROM_FRESH_START",
    "redeem_slo": "PASS|FAIL"
  },
  "p_large": {
    "private_issue_pass": false,
    "private_redeem_pass": false,
    "issue_slo": "PASS|FAIL",
    "redeem_slo": "PASS|FAIL"
  },
  "stakehub_nav": {
    "real_sources": false,
    "proof_verified": false,
    "finalized_on_pftl": false,
    "uniswap_price_used": false
  },
  "supply_conservation_pass": false,
  "reserve_conservation_pass": false,
  "wrapper_conservation_pass": false,
  "pfusdc_conservation_pass": false,
  "uniswap_liquidity_consumed_by_primary_issue": 0,
  "intervention_free_after_p_large_funding": false
}
```

`business_flow_pass` may be true when the recovered `T-SMALL` lineage and all
fresh remaining legs complete with exact accounting, even though the old
paused issue interval prevents `release_slo_pass`.

The top-level `verdict` is:

- `PASS` only when the business flow passes, all fresh measured SLOs pass,
  all invariants pass, and no unresolved recovery remains;
- `FAIL` when the campaign reaches a safe terminal state but a functional,
  privacy, NAV, accounting, intervention, or timing gate fails; or
- `RECOVERY_REQUIRED` while any live-value lineage is incomplete or custody
  is ambiguous.

## 12. Gates

| Gate | Pass condition |
|---|---|
| `V0` — frozen readiness | Exact identities, capacities, accounting, release parity, warm provers, real StakeHub baseline, and recovery paths pass before new funding. |
| `V1` — small transparent issued | Existing Phase 9 deposit is claimed once; exactly `1 A666` is created and exactly `1 wA666` reaches Ethereum without pool liquidity. |
| `V2` — larger private issued | Fresh exact deposit creates `100 private A666`, exports exactly `100 wA666`, rejects replays, and consumes no pool liquidity. |
| `V3` — NAV re-marked | Real StakeHub sources produce a verified, independently recomputable post-issuance NAV finalized identically by all validators; Uniswap price and spread are excluded. |
| `V4` — transparent redeemed | Exactly `1 A666` is returned and retired; policy-priced USDC reaches Ethereum without issuer discretion or Uniswap. |
| `V5` — private redeemed | Exactly `100 A666` is privately returned/redeemed; policy-priced USDC reaches Ethereum with valid proofs and replay rejection. |
| `V6` — reconciled | Supply, reserve, private custody, bridge claims, wrapper supply, vault obligations, spreads, and validator state all reconcile. |

Failure at any gate blocks the next value-moving gate. Diagnostic collection
and safe recovery may continue.

## 13. Definition of done

This campaign is done when:

- the existing small deposit is resolved exactly once;
- transparent primary issuance creates and exports exactly `1 A666`;
- fresh private-middle primary issuance creates and exports exactly
  `100 A666`;
- both acquisitions consume zero Uniswap liquidity and produce spendable,
  transferable Ethereum wA666;
- a fresh real StakeHub reserve proof and NAV mark is finalized after both
  issuances and before either redemption;
- the NAV report proves the pool price and issue spread were excluded;
- transparent redemption retires the small position and releases exact USDC;
- private-middle redemption retires the larger position and releases exact
  USDC;
- final A666 supply and wrapper exposure return to baseline;
- pfUSDC, reserve principal, and spread accounting reconcile exactly;
- all replays reject without mutation;
- all six validators converge with empty mempools;
- each fresh issue and redemption satisfies the 25-minute gate or is honestly
  reported as an SLO failure; and
- the evidence distinguishes functional success, privacy boundary, NAV
  provenance, intervention status, and latency without qualification hidden
  in prose.
