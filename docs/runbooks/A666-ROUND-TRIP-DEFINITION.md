# A666 Round-Trip Definition

## Normative definition

An **A666 round trip** is one complete, receipt-chained, live-funds traversal of
the canonical route:

```text
external Ethereum USDC
  -> successor-vault deposit
  -> PFTL pfUSDC claim
  -> primary A666 subscription at verified NAV
  -> PFTL A666 export
  -> proof-backed Ethereum wA666 mint
  -> Uniswap wA666-to-USDC sale
  -> Uniswap USDC-to-wA666 buy-back
  -> Ethereum wA666 return burn
  -> PFTL A666 return import
  -> primary A666 redemption at verified NAV
  -> PFTL pfUSDC burn-to-redeem
  -> successor-vault Ethereum USDC withdrawal
  -> PFTL settlement and final reconciliation
```

All stages belong to the same value lineage. A round starts with the external
USDC deposit and ends only after the final external USDC credit, PFTL
settlement, six-validator convergence, replay checks, and conservation report.

The following is **not** an A666 round trip:

```text
USDC -> pfUSDC -> USDC
```

That sequence tests only the pfUSDC bridge ingress and egress lane. It omits the
A666 product, verified-NAV pricing, Ethereum wA666 export, Uniswap execution,
return import, and primary redemption. It must be labeled a **pfUSDC bridge
round trip**, never an A666 round trip.

## Units and reporting convention

USDC, pfUSDC, A666, and wA666 use six decimal places in this workflow. Evidence
files store integer atomic units, so:

```text
1,000,000 atoms = 1.000000 token
10,000,000 atoms = 10.000000 tokens
103,000,000 atoms = 103.000000 tokens
```

Operational reports must state the human-readable token amount first and may
include the integer atoms in parentheses for exact reconciliation. For example,
write **10.000000 USDC (10,000,000 atoms)**. Never describe 10,000,000 atoms as
"10 million USDC"; that overstates the economic amount by a factor of one
million.

## Assets and economic roles

| Asset or state | Role in the round |
|---|---|
| Ethereum USDC | External starting capital, Uniswap quote asset, and final external payout. |
| PFTL pfUSDC | Source-labeled bridge claim and settlement asset used to fund A666 primary issuance. |
| PFTL A666 | NAV-tracked asset newly issued by the primary market and later retired through primary redemption. |
| Ethereum wA666 | Wrapped representation minted only from a finalized PFTL export and burned for the PFTL return. |
| A666 NAV reserve | Holds the subscription-funded pfUSDC principal backing valid A666 supply. |
| Uniswap v4 wA666/USDC pool | Secondary-market venue used once in each direction after primary issuance and export. |

pfUSDC is not A666, and registering pfUSDC as a PFTL NAV-accounted asset does
not perform the required pfUSDC-to-A666 subscription. Likewise, exporting A666
to wA666 is a custody-domain transition, not a Uniswap trade.

## What “verified NAV” requires

The A666 subscription and redemption must use the active, governed primary
market route and a policy-pinned NAV that is:

1. derived from the governed reserve-source set;
2. bound to the intended A666 asset, pfUSDC settlement asset, route, policy
   epoch, proof profile, and reserve packet;
3. proven through the required NAV proof path rather than manually entered;
4. finalized identically by all six PFTL validators;
5. fresh under the active policy's maximum-age rule; and
6. converted into asset amounts with checked integer arithmetic and the
   governed issue or redemption multiplier.

Neither of these qualifies as verified A666 NAV:

- the Uniswap spot price; or
- a `nav_profile_register` or `nav_asset_register` operation by itself.

Registration establishes configuration. It does not prove a current A666 NAV,
reserve value, subscription, or redemption.

## Required execution stages

### Stage 0 — Freeze and verify the live baseline

Before the first mutation, record and independently verify:

- all six validators' release identity, finalized height, state root, route
  state, and empty mempool;
- the active A666/pfUSDC route and policy epoch;
- the fresh finalized A666 NAV packet and its proof bindings;
- the successor pfUSDC vault, verifier, route epoch, and unpaused status;
- the Ethereum chain, canonical USDC, wA666 controller/token, and exact
  Uniswap v4 pool identity;
- wallet USDC, ETH, pfUSDC, A666, and wA666 balances;
- A666 valid supply, NAV reserve principal, export entitlements, outstanding
  wrapped claims, pfUSDC obligations, and vault USDC;
- current pool liquidity and quote viability in both directions;
- capacity and spend limits; and
- a fresh evidence directory and fresh identifiers for the round.

The protected wallet wA666 baseline is not campaign inventory. It must remain
untouched. Only wA666 minted from the current round's export may enter that
round's Uniswap sale.

### Stage 1 — Ethereum USDC to spendable PFTL pfUSDC

1. Approve the exact successor vault allowance required for the deposit.
2. Deposit the round's exact USDC amount into the successor Ethereum vault.
3. Wait for the required Ethereum finality and build the ingress proof.
4. Propose and finalize the PFTL deposit evidence.
5. Claim exactly the corresponding pfUSDC amount to the bound PFTL holder.

Required result:

- wallet USDC decreases by the deposit amount;
- successor-vault USDC and bridge backing increase by that amount;
- the holder's spendable pfUSDC increases by that amount;
- the deposit ID and claim are consumed exactly once; and
- all six validators converge on the accepted claim state.

The pfUSDC must remain on PFTL for Stage 2. Burning it immediately for USDC
egress is a bridge-only loop and fails the A666 round.

### Stage 2 — pfUSDC to newly issued A666 at verified NAV

1. Resolve the issue quote from the active policy-pinned verified NAV.
2. Create a fresh primary-market order reservation for the exact pfUSDC input
   and computed A666 output.
3. Finalize the reservation before submitting the dependent subscription.
4. Submit the primary subscription using the same reservation, NAV packet,
   policy epoch, signer, holder, and amount bindings.

Required result:

- the exact pfUSDC subscription amount is debited;
- the exact NAV-derived A666 amount is newly issued to the subscriber;
- valid A666 supply increases by the issued amount;
- A666 reserve principal increases by the governed base-value amount;
- any issue spread is accounted in its designated non-NAV bucket;
- the reservation becomes terminal exactly once; and
- six-validator receipt and state convergence pass.

This is primary issuance. It must not be replaced by an OTC transfer, an owner
mint, existing A666 inventory, a market-maker fill, an LP withdrawal, or a
Uniswap purchase.

### Stage 3 — Export PFTL A666 and proof-mint Ethereum wA666

1. Debit the exact current-round A666 amount into a fresh export entitlement
   bound to the Ethereum recipient.
2. Finalize the export receipt on all six validators.
3. Advance the Ethereum PFTL checkpoint as needed for the receipt's bounded
   proof window.
4. Build and verify the receipt inclusion/finality proof.
5. Submit the controller's accept-and-mint transaction exactly once.
6. Record destination consumption on PFTL where required by the active route.

Required result:

- the holder's liquid PFTL A666 decreases by the exported amount;
- valid global A666 supply does not change during export;
- Ethereum wA666 supply and the bound wallet balance increase by exactly the
  receipt-authorized amount;
- the proof, receipt, packet, entitlement, and export nonce are consumed once;
  and
- replay rejects without mutation.

“Withdraw A666 to Uniswap” therefore means first exporting it as spendable
wA666 to the bound Ethereum wallet and then trading that minted wA666 through
the designated pool. It does not mean withdrawing tokens from pool liquidity.

### Stage 4 — Forward Uniswap trade: wA666 to USDC

1. Obtain a fresh quote against the exact designated wA666/USDC pool.
2. Bind the input amount, minimum output, deadline, pool identity, fee tier,
   recipient, Permit2 authorization, router calldata, and gas cap.
3. Approve and authorize only the exact required path.
4. Swap the current round's receipt-chained wA666 amount for USDC.

Required result:

- the current round's wA666 balance decreases by the exact swap input;
- wallet USDC increases by at least the bound minimum output;
- the transaction is mined with status `1`;
- pool and wallet deltas match the executed receipt; and
- the protected pre-existing wA666 baseline remains unchanged.

This secondary-market sale is mandatory. Merely holding exported wA666 in the
wallet is not a completed round.

### Stage 5 — Reverse Uniswap trade: USDC to wA666

1. Use the actual Stage 4 USDC proceeds as the reverse trade's value lineage.
2. Obtain a fresh reverse quote and bind the exact input, minimum output,
   deadline, pool, recipient, approvals, calldata, and gas cap.
3. Swap the receipt-chained USDC back to wA666 through the same designated
   pool.

Required result:

- wallet USDC decreases by the exact reverse input;
- wallet wA666 increases by at least the bound minimum output;
- the transaction is mined with status `1`;
- executed pool and wallet deltas reconcile; and
- the returned amount, including any pool fee and slippage effect, becomes the
  authoritative input to Stage 6.

The reverse amount must be taken from the mined receipt. It must not be
replaced with the original export amount or topped up from protected inventory.

### Stage 6 — Burn wA666 and import returned A666 to PFTL

1. Burn the exact Stage 5 wA666 output through the bound return controller.
2. Wait for Ethereum finality and capture the burn receipt.
3. Submit the PFTL return-import operation with the actual burn transaction,
   amount, recipient, route, and fresh return nonce.

Required result:

- Ethereum wA666 supply decreases by the exact burned amount;
- the round's outstanding wrapped claim decreases correspondingly;
- the bound PFTL holder receives the exact returned A666 amount;
- global valid A666 supply does not change during the custody return;
- the burn and return nonce are consumed once; and
- replay rejects without mutation.

### Stage 7 — Redeem returned A666 to pfUSDC at verified NAV

1. Resolve the redemption quote from the active policy-pinned verified NAV.
2. Use the actual imported A666 delta from Stage 6 as the redemption input.
3. Submit a fresh primary redemption bound to the correct holder, route,
   policy epoch, NAV packet, amount, and redemption nonce.

Required result:

- the returned A666 is retired;
- valid A666 supply decreases by exactly the redeemed amount;
- NAV reserve principal decreases by the governed redemption base value;
- the holder receives the exact policy-derived pfUSDC payout;
- any redemption spread is separately accounted; and
- all six validators accept and converge on the finalized state.

The redemption price comes from verified NAV and policy arithmetic, not the
Uniswap execution price.

### Stage 8 — pfUSDC to external Ethereum USDC

1. Burn the Stage 7 pfUSDC payout into a successor-lane redemption packet.
2. Finalize the PFTL burn and generate the required Ethereum withdrawal proof.
3. Execute the successor-vault withdrawal to the bound Ethereum destination.
4. Submit the PFTL settlement with observations matching the successor vault,
   withdrawal, recipient, and exact released amount.

Required result:

- pfUSDC decreases by the exact burned payout;
- external wallet USDC increases by the exact released amount;
- successor-vault USDC and obligations decrease by that amount;
- burn, withdrawal, proof, and settlement nullifiers are consumed once;
- replay rejects without mutation; and
- all six validators converge after settlement.

### Stage 9 — Terminal reconciliation

The round is complete only when one immutable report proves:

- every PFTL mutation has an accepted finalized receipt, certificate quorum,
  and six-validator convergence;
- every Ethereum mutation has a canonical mined receipt with status `1` and
  independently checked balance deltas;
- the entire deposit-to-withdrawal value lineage is continuous;
- all round-specific reservations, entitlements, claims, returns,
  redemptions, and withdrawals are terminal with no unexplained residual;
- A666 supply changes only at subscription and redemption;
- wA666 supply changes only at proof-backed export mint and return burn;
- pfUSDC obligations equal their canonical custody and in-flight components;
- vault USDC reconciles with pfUSDC bridge backing;
- A666 NAV reserve principal and non-NAV spread accounting reconcile;
- the forward and reverse Uniswap executions reconcile to actual pool and
  wallet deltas;
- protected balances and unrelated inventory did not move;
- every replay probe rejects without mutation;
- the final validator mempools are empty; and
- gas, pool fee, slippage, issue spread, redemption spread, and any NAV change
  explain the wallet's net external USDC result.

Exact conservation does **not** mean the final wallet USDC must equal its
starting balance. The economic result may differ because of governed primary
market spreads, Uniswap fees and slippage, Ethereum gas, and a legitimate NAV
change. Exact conservation means every atom and wei of that difference is
identified and reconciled.

## Receipt gates and recovery discipline

Each mutation is a separate receipt gate. The next dependent mutation must not
be submitted until the prior mutation is authoritative and its derived values
have been read from the finalized receipt.

Before every mutation:

1. reconcile the expected pre-state against both chains;
2. verify that the operation's unique identifiers have not already landed;
3. verify route, policy, NAV, amount, signer, recipient, nonce, deadline,
   capacity, and spend-cap bindings;
4. simulate or batch-only validate where the active runbook requires it; and
5. persist the intent and expected deltas before broadcasting.

On a rejection, timeout, ambiguous receipt, validator divergence, stale NAV,
wrong balance delta, or proof mismatch, stop that lineage and reconcile it.
Never blindly retry or create a replacement value lineage. A safe resume begins
from authoritative receipts and uses only still-unconsumed identifiers.

## Six-round campaign rule

For a six-round campaign, all six rounds must independently satisfy this entire
definition. Each round requires its own:

- workflow and evidence directory;
- Ethereum deposit and withdrawal IDs;
- PFTL claim, reservation, subscription, export, return, redemption, and
  settlement identifiers;
- packet hashes and proof/nullifier lineage;
- subscription, export, return, redemption, and bridge nonces;
- Uniswap quotes, deadlines, calldata, and mined receipts; and
- terminal conservation report with a `PASS` verdict.

Capital may be recycled only after the preceding round's final bridge-out and
terminal reconciliation prove it is available. A bridge-only
`USDC -> pfUSDC -> USDC` cycle counts as zero A666 rounds. A partial sequence
that stops after A666 issuance, wA666 export, or the forward Uniswap sale also
counts as zero completed rounds.

Campaign completion requires six independently evidenced A666 round trips, not
six deposits, six bridge cycles, or one completed round plus five partial
lineages.

## Authoritative source documents

- [A666 Five-Demo Program](../plans/A666-FIVE-DEMO-PROGRAM-20260809.md)
- [A666 Unified Execution Plan](../plans/A666-UNIFIED-EXECUTION-PLAN-20260808.md)
- [A666 End-to-End Live-Funds Runbook](A666-END-TO-END-LIVE-FUNDS-RUNBOOK-20260807.md)
- [A666 Variable-Size Real-NAV Round-Trip Execution Spec](../plans/A666-VARIABLE-SIZE-REAL-NAV-ROUNDTRIP-EXECUTION-SPEC-20260729.md)
- [A666 End-to-End Mainnet Primary-Issuance Spec](../plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md)
