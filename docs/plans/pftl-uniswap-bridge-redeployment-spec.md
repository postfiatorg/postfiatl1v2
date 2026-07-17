# PFTL-to-Uniswap NAVCoin Redeployment Spec

Date: 2026-07-01
Status: design spec for implementation planning
Repos covered: `postfiatl1v2`, `StakeHub`, `postfiatorg.github.io`
Owner: release owner, with protocol, wallet, Ethereum contracts, and StakeHub
sign-off before Gate 6.
Change control: any change to token, bridge controller, verifier, trust class,
pool key, seed parameters, caps, deadlines, route labels, or proof policy after
fork rehearsal produces a new config digest and requires a new rehearsal before
live use.
Digest authority: the Rust node is the sole canonical digest authority. No
wallet, proxy, script, or service may recompute a canonical config digest from
its own serialization; any component that displays or compares a digest must
consume a node-produced value.
Evidence immutability: checked-in evidence packets are immutable. Any change to
a raw report requires re-running the generating harness; hand-edited evidence
is invalid for gate exit.

## 1. Objective

Redeploy NAVCoin-to-Uniswap liquidity using a bridge-aware venue token whose
supply is controlled by PFTL bridge packets, not by the legacy standalone
Ethereum `a651` token.

The first Ethereum `a651/USDC` Uniswap pool proved that a NAVCoin-shaped ERC-20
can be launched, priced from a proof snapshot, seeded into Uniswap v4, and
traded as secondary liquidity. It did not prove the final architecture.

The structural flaw was that the old pool traded a standalone Ethereum token
whose mint authority and proof adapter lived on Ethereum. It was not a
bridge-aware representation of a PFTL-finalized debit. The pool could trade, but
it could not atomically consume a PFTL receipt, mint a venue representation, run
the Uniswap leg, and leave the user with exactly one terminal outcome. PFTL was
not the source of truth for that venue token.

The replacement route is:

```text
pfUSDC or another counted PFTL input
  -> PFTL NAV policy
  -> native PFTL NAVCoin, e.g. a666
  -> PFTL bridge export debit or lock
  -> finalized PFTL bridge packet
  -> Ethereum verifier or optimistic acceptance
  -> bridge-aware wrapped venue token
  -> optional atomic Uniswap mint-and-swap
```

The new Uniswap pool must trade the bridge-aware wrapped venue token, not the
legacy `a651` token. Uniswap is an execution venue. It is not the NAV oracle, not
the bridge, not the supply accountant, and not the refund authority.

Public redeployment is allowed only after the selected trust label, migration
treatment, pool identity, proof freshness, refund semantics, and monitoring are
explicit and enforced by the route registry, Ethereum controller, wallet, and
deployment config digest.

### 1.1 TL;DR

- The old `a651/USDC` pool failed as the final design because it traded a
  standalone Ethereum token. It did not consume a PFTL-finalized source debit.
- The new design needs a new PFTL NAVCoin instance, a new bridge-aware Ethereum
  wrapped token, a new Uniswap pool, and a packet state machine that binds
  source debit, destination consume, swap execution, and refund.
- The first implementation may be controlled or optimistic, but it must say so
  in machine-readable route state and wallet copy. Do not call it trustless
  until Ethereum verifies PFTL finality and receipt inclusion under the stated
  verifier assumptions.
- Primary PFTL subscriptions create new NAVCoin supply against the user's paid
  settlement asset. Uniswap buys transfer existing wrapped venue tokens from
  pool liquidity; Uniswap does not create canonical NAVCoin supply.
- The route is not production-ready until the return path, replay protection,
  refund/challenge economics, config digest, wallet label checks, and monitoring
  gates pass.

### 1.2 Primary Supply Answer

The plan must support two different user purchases and label them differently:

1. Primary PFTL subscription: the user pays counted settlement asset, such as
   `pfUSDC`, and PFTL mints new native NAVCoin supply at the finalized
   pre-inflow NAV price. The user's payment is added to reserves after the fill,
   so the fill is economically backed but the user's own inflow cannot raise the
   price against the same transaction.
2. Uniswap buy: the user buys existing `wA666` from pool or market-maker
   liquidity at AMM price and slippage. That trade transfers already-issued
   wrapped supply and does not mint new canonical NAVCoin supply.
3. Composite route: the wallet can start from `pfUSDC`, run the primary PFTL
   subscription, export the minted native NAVCoin through the bridge, mint
   `wA666` on Ethereum, and optionally execute a Uniswap swap. The quote must
   disclose the primary NAV price and the AMM execution price as separate facts.

Example: if the finalized pre-inflow NAV is 1,000 USDC per NAVCoin and a user
subscribes 100,000 USDC, the expected primary fill is about 100 NAVCoin before
fees and rounding. After the transition, reserves include the user's 100,000
USDC and canonical supply includes the new 100 NAVCoin. That same 100 NAVCoin
can then stay native on PFTL or be exported as `wA666`; it must not be treated as
operator inventory.

That example is not optional launch copy; it is the required primary-fill
semantics. A large primary subscription must be filled against the finalized
pre-inflow NAV snapshot, then the paid settlement asset is included in reserves
after the fill. If a route cap, wallet cap, proof freshness limit, or rounding
rule prevents filling the entire requested amount in one transition, the
unfilled amount must be rejected, refunded, or queued as an explicit next-step
state. It must not silently fall back to Uniswap liquidity, fixed operator
inventory, or a post-inflow self-priced NAV.

This is a release-blocking requirement. Any implementation that only lets users
buy from fixed inventory, or that implies Uniswap creates NAVCoin supply, is not
this plan.

### 1.2.1 Required User Routes

This plan explicitly includes end-user purchase and exit routes. A deployment
that cannot execute these routes is incomplete even if the token, bridge
controller, pool, and proofs exist.

1. `pfUSDC -> a666`: primary PFTL subscription. The user spends counted PFTL
   settlement asset and receives fractional native NAVCoin minted at the
   finalized pre-inflow NAV price.
2. `pfUSDC -> wA666`: primary subscription plus bridge export. The user spends
   counted PFTL settlement asset, PFTL mints native NAVCoin, exports the minted
   amount through a bridge packet, and Ethereum mints the same amount of wrapped
   venue token. If destination consume cannot complete, the route must expose a
   terminal refund or recovery state.
3. `USDC -> wA666` and `wA666 -> USDC`: secondary Uniswap trades against seeded
   pool or market-maker liquidity. These trades move existing wrapped venue
   supply and must not change canonical PFTL NAVCoin supply.
4. Optional composite route: `pfUSDC -> primary subscription -> bridge ->
   wA666` or Uniswap output. The quote must show the primary NAV issuance price,
   bridge packet fields, AMM execution price, slippage, and minimum output as
   separate facts.

The primary route is not constrained by pre-existing pool liquidity or operator
inventory. It is constrained only by route caps, reserve/proof freshness,
settlement balance, deterministic rounding, and protocol pause state. A
100,000 USDC subscription at a finalized 1,000 USDC NAV must be treated as a
primary issuance of about 100 NAVCoin before fees and rounding, then that newly
issued amount can be held on PFTL, exported to Ethereum as `wA666`, or sold
through Uniswap if there is secondary liquidity.

Wallets must present these as different actions:

```text
Primary issuance: creates new NAVCoin supply at finalized NAV.
Bridge export: moves that issued supply to Ethereum as wA666.
Uniswap trade: buys or sells existing wA666 at AMM price.
```

If the wallet cannot clearly tell the user which of those actions occurred, what
asset they now own, and which terminal state the packet reached, Gate 6 fails.

### 1.3 Not Production Ready Yet

This document is an execution plan, not a launch approval. Public-value routing
must remain disabled unless all of the following are true:

- Gate 0 decisions are signed off and encoded in config.
- Gate 1 and Gate 2 prove packet replay, cap, and supply invariants.
- Gate 3 proves mint-only and mint-and-swap behavior on a fork using official
  Uniswap deployment addresses.
- Gate 4 proves the return path twice without manual state edits.
- The selected Gate 5 path proves either optimistic challenge safety or direct
  PFTL finality verification.
- Gate 6 proves wallet/proxy trust labels match the PFTL registry, Ethereum
  controller, and config digest.

Any controlled Ethereum controller, mock router, packet-hash experiment, or
wallet-side route metadata added before these gates is test scaffolding only.
It must not be treated as the current launch plan, a live wallet route, or proof
that the redeployment is implemented. The plan remains the primary PFTL
subscription plus bridge-aware venue-token route described in section 7.

### 1.4 Glossary

| Term | Meaning in this spec |
| --- | --- |
| Native NAVCoin | A NAV-tracked issued asset on PFTL, such as the proposed `a666`. |
| Venue token | An ERC-20 representation that can trade on Ethereum, such as `wA666`. |
| Legacy `a651` | The historical NAVCoin/token/pool used for the first launch, not the new bridge route. |
| Bridge packet | Canonical PFTL export data binding source debit, destination token, recipient, action, nonce, and deadlines. |
| Consume | Destination-side acceptance of a packet, which must happen at most once. |
| Outstanding claim | Supply encumbrance for a debited source packet that has not reached a terminal destination consume or refund state. |
| Trust class | Machine-readable route safety label: `CONTROLLED`, `OPTIMISTIC`, `TRUSTLESS_FINALITY`, or `DISABLED`. |
| Atomic mint-and-swap | Destination consume, venue-token mint, and Uniswap swap in one transaction, reverting without consume if the swap leg fails. |

## 2. Decision Register

Gate 0 must close all P0 decisions before any implementation path can move live
funds.

| Priority | Decision | Default or required action | Blocks | Risk if wrong | Owner | Target |
| --- | --- | --- | --- | --- | --- | --- |
| P0 | Token strategy | Use a new PFTL NAVCoin instance plus a new Ethereum wrapped venue token. Example: new `a666/wA666`. Legacy `a651` is historical only; no active wallet routing and no implicit migration. | All gates. | Legacy token/pool gets misrepresented as canonical. | Product + protocol | Gate 0 |
| P0 | Movement model | Use burn/mint for the first bridge-aware deployment. | Gates 1-4. | Double counting or ambiguous backing. | Protocol | Gate 0 |
| P0 | Primary subscription economics | Fractional primary PFTL subscriptions create new native NAVCoin supply from the user's settlement inflow. Price from finalized pre-inflow NAV, then atomically increase reserves, authorized supply, and user balance. Wallet must not present this as fixed inventory or as Uniswap-created supply. | Gates 1, 3, 6. | Users cannot buy fractional NAV, or fills use wrong economics. | Protocol + wallet | Gate 0 |
| P0 | End-user route completeness | Public launch requires working `pfUSDC -> a666`, `pfUSDC -> wA666`, `USDC -> wA666`, and `wA666 -> USDC` routes, with the composite PFTL-to-Ethereum route capped and trust-labeled if controlled or optimistic. | Gates 1, 3, 4, 6. | The product launches a token or pool that users cannot actually enter or exit through the advertised path. | Product + protocol + wallet | Gate 0 |
| P0 | Tradable venue supply | A public `wA666/USDC` route must prove real pool tradeability, not only token deployment. Pool seed supply must come from canonical primary subscription plus bridge export, and fork rehearsal must execute at least one external buy and one external sell against the seeded pool. | Gate 3. | The token exists but users cannot actually trade it, or seed supply is unexplained operator inventory. | Ethereum contracts + StakeHub | Gate 0 |
| P0 | Trust class for public value | Do not use unqualified `trustless` until direct or succinct PFTL finality verification exists. `CONTROLLED` is allowed for testnet and internal live rehearsals only by default. Public wallet routing must show and enforce `CONTROLLED` if ever enabled, and remains disabled unless explicitly capped and labeled. | Gates 2 and 6. | Users see a trusted route as trustless. | Protocol + wallet | Gate 0 |
| P0 | Refund model | Use optimistic refund challenge for the staged path; verified non-consumption is the target final path. | Gates 1, 4, 5. | Destination consume and source refund can race. | Protocol security | Gate 1 |
| P1 | Uniswap execution path | First pool is new `wA666/USDC`, hookless Uniswap v4 unless a real v4 hook passes the fork gate. Seed `wA666` only through canonical primary subscription plus bridge export. Use a minimal settlement adapter with bound router/path hash. | Gate 3. | Router mutation, unexplained seed inventory, or failed swap consumes packet. | Ethereum contracts | Gate 2 |
| P1 | Governance | Bind config digest, on-chain trust class, pause roles, and verifier-key policy. | Gates 5-6. | Silent verifier weakening or bad upgrade. | Release owner | Gate 2 |
| P1 | Shielded egress | Public bridge-out disclosure in v1; private bridge is separate. | Wallet copy and privacy claims. | Privacy overclaim. | Product + protocol | Gate 3 |
| P2 | Fees | User pays PFTL gas and EVM gas/relayer fee; fee asset finalized later. | Wallet quote quality. | Bad UX or stuck packets. | Wallet + relayer | Gate 3 |
| P2 | L2 order | Ethereum first; Arbitrum/Base after return path. | Multi-chain scope. | Parallel launches repeat old fragmentation. | Product | Gate 4 |
| P2 | Legacy migration | Separate spec only. | Public migration claims. | Old holders/LPs treated inconsistently. | Product + legal | After Gate 4 |

## 3. Staged Trust Model

Execute the redeployment in this order:

1. Controlled threshold-signed testnet packets for packet shape and UX.
2. Bridge-aware venue token plus strict supply/in-flight accounting.
3. Mint-only and mint-and-swap Uniswap adapter fork tests.
4. Optimistic verifier with permissionless challenges, or direct/succinct PFTL
   finality verification.
5. Public redeployment only after the chosen trust label, migration treatment,
   pool identity, proof freshness, refund semantics, and monitoring are explicit.

Trust labels are protocol state, not copy text. Every route must expose one
machine-readable trust class:

```text
CONTROLLED
OPTIMISTIC
TRUSTLESS_FINALITY
DISABLED
```

The `VenueBridgeController`, PFTL route registry, wallet route planner, and
StakeHub launch config digest must agree on this value. A route cannot be shown
as `trustless` unless the on-chain trust class is `TRUSTLESS_FINALITY`.
For the consensus-completion sprint, destination-consume and return-burn inputs
remain operator-attested under `CONTROLLED`: the issuer or NAV reserve operator
signs the PFTL-side transition, consensus enforces route authorization,
finality-depth arithmetic, packet status, replay, and supply movement, but Gate
5 is still required before wallets may describe these events as verified,
trustless, or independently finalized.

## 4. Source Inventory and Evidence Snapshots

### Primary Local Sources

- `docs/navcoins/uniswap-pool.md`
- `docs/plans/trustless-navswap-wallet-integration-spec.md`
- `docs/specs/vault-bridge-navcoin-profile.md`
- `docs/navcoins/reserve-primitives.md`
- `docs/status/arbitrum-contracts-code-review-2026-06-19.md`
- `StakeHub/docs/navcoin-uniswap-launch-plan.md`
- `StakeHub/docs/navcoin-launch-runbook.md`
- `StakeHub/docs/launches/a651-ethereum-mainnet-2026-06-15.md`
- `StakeHub/zk/contracts/src/navcoin/NavCoin.sol`
- `StakeHub/zk/contracts/src/navcoin/NavBridgeController.sol`
- `StakeHub/zk/contracts/src/navcoin/NavProofAdapter.sol`
- `StakeHub/zk/contracts/src/navcoin/NavMintRedeemController.sol`
- `content/research/trustless-pftl-uniswap-bridges.md` in
  `postfiatorg.github.io`
- `content/research/canonical-navcoin-transaction.md` in
  `postfiatorg.github.io`

### External Source Checked

- Uniswap v4 deployments page:
  `https://developers.uniswap.org/docs/protocols/v4/deployments`

### Read-Only Live Inventory

- Command:
  `node scripts/navswap-custody-inventory.mjs --out-dir /tmp/navswap-custody-inventory-uniswap-spec-20260701T025341Z`
- Result:
  `legacy_pool_status = legacy_pool_inactive_zero_stateview_liquidity`
- Artifact:
  `/tmp/navswap-custody-inventory-uniswap-spec-20260701T025341Z/inventory.md`
- Artifact hashes:
  - `inventory.md`: `cd154c440cb5b8222f2fe47e034a2e971da1323c8ede869916074b3935888f46`
  - `inventory.json`: `0375cda7724900e47b0383fb5e517cd99daf4aba509eecb1cf10c98b87698c02`

Timestamped inventory and Uniswap address rows are evidence snapshots. They are
not deployment authority. The deploy command must pin the current official
Uniswap table and live-read custody state into a config digest at launch time.

## 5. Current State

### 5.1 Legacy Ethereum `a651/USDC` Pool

The live launch record and pool docs identify the historical pool:

| Field | Value |
| --- | --- |
| Chain | Ethereum mainnet, chain id `1` |
| Pair | legacy `a651` / USDC |
| Pool | Uniswap v4 |
| Pool id | `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84` |
| Legacy token | `0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e` |
| USDC | `0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48` |

The 2026-07-01 read-only custody inventory reports:

```text
StateView liquidity: 0
Status: legacy_pool_inactive_zero_stateview_liquidity
PoolManager legacy a651 balance: 0.093936271343217656 a651
Operator legacy a651 balance: 3815.876054831038397894 a651
Operator USDC balance: 5717.752289 USDC
Arbitrum old pfUSDC vault: 236.982047 USDC
```

This pool is historical secondary liquidity. It is not an active wallet route
and not the trustless PFTL-to-Uniswap handoff.

### 5.2 StakeHub Launch Stack

StakeHub deployed a real launch stack:

- `NavCoin`: ERC-20 venue token with controller-gated mint/burn, pause,
  quarantine, allocation registry binding, and controller lock.
- `NavProofAdapter`: reads `StakeHubLeverageVerifier.latest()`, checks schema,
  mode, program key, accepted policy hash, proof age, and computes NAV/unit.
- `NavAllocationRegistry`: records venue caps and allocation policy.
- `NavMintRedeemController`: mints and redeems against fresh NAV and USDC
  liquidity, currently operator-gated by default.
- `NavBridgeController`: tracks authorized supply and owner-only
  `burnForRemote` / `mintFromRemote` launch scaffolding.
- `NavCoinV4LaunchHelper`: initializes and seeds the real Uniswap v4 pool.

This stack was useful for the launch, but its bridge controller is not a
trustless PFTL bridge. The runbook says the multi-chain path is standalone per
chain: Arbitrum/Base launches are fresh independent deployments, not supply
movement through a live cross-chain channel.

### 5.3 l1v2 Bridge and Market-Ops Stack

The l1v2 repo has separate generic primitives:

- vault bridge receipt accounting for `pfUSDC`;
- `ERC20BridgeVault` and `PFTLWithdrawalVerifier` for controlled source-chain
  withdrawal packets;
- `MarketOpsEnvelope`, `PFTLBridgeAdapter`, `MarketOpsVault`, `MintController`;
- `NAVGuardHook`, a v4-shaped venue observation adapter.

The 2026-06-19 code review is binding for this spec:

- `PFTLWithdrawalVerifier` is threshold-signer based, not trustless PFTL
  finality.
- `ERC20BridgeVault` had unresolved challenge-griefing, replay-domain, and
  expiry recovery risks.
- `NAVGuardHook` is not a real Uniswap v4 hook and accepts caller-supplied
  market data from its configured manager/adapter.

These components can inform the design, but they cannot be treated as the final
public trustless route until their trust and replay boundaries are fixed.

## 6. Why the Legacy Uniswap Pool Failed as the Final Design

The first pool failed as the final design because it made Uniswap the visible
access point before the PFTL bridge and supply-perimeter model existed.

### 6.1 The Ethereum Token Was Not a PFTL-Verified Representation

The legacy token could be minted or burned by its Ethereum controller. It did
not require a verified PFTL source debit packet. The Ethereum proof adapter
verified StakeHub reserve proof state, but that is not the same as verifying
that a user's PFTL `a651` was debited and should now exist on Ethereum.

Correct bridge-aware venue supply requires:

```text
PFTL source debit or lock
  -> finalized PFTL receipt
  -> destination verifier consumes exact packet once
  -> Ethereum venue token mints or unlocks
```

The legacy pool had no such packet-consumption boundary.

### 6.2 The Pool Had No Atomic Relationship to PFTL Settlement

An atomic user flow must enforce:

```text
either:
  PFTL debit is consumed on Ethereum and the user receives venue tokens or swap output
or:
  PFTL debit remains safely refundable after expiry
never both
never neither without a recovery path
```

The old pool was just a Uniswap pool over an ERC-20. It could not see PFTL
finality, PFTL receipt inclusion, packet nonces, refund windows, or outstanding
claim state. A user could not make a trustless atomic promise of:

```text
spend pfUSDC -> mint native NAVCoin -> bridge to Ethereum -> swap in Uniswap
```

because there was no single packet state machine binding those legs.

### 6.3 The Controller Boundary Was Locked and Not Repointable

The legacy `NavCoin` supports `controllerLocked`. Once locked, the token's
controller cannot be replaced by a future PFTL bridge minter. Any design that
assumes the old token can be turned into a bridge-aware token by repointing the
controller is invalid.

The honest choices are:

1. Deploy a new bridge-aware wrapped venue token.
2. Wrap legacy `a651` with explicit backing separation.
3. Publish an explicit migration plan.

The clean protocol path is a new bridge-aware wrapped token.

### 6.4 The Multi-Chain Representatives Were Registry Records, Not Live Bridge Venues

The launch stack included Arbitrum and Base venue token representative records,
but the runbook states they are Ethereum registry representatives. Real
Arbitrum/Base deployments are separate independent launches. The old stack did
not implement a global bridge channel moving supply across PFTL, Ethereum,
Arbitrum, and Base.

That means the old pool could not be the endpoint for a PFTL-originating
trustless route.

### 6.5 The LP Could Mislead Users About Canonical Liquidity

Pool liquidity is not portfolio backing. The pool was a secondary-market venue,
not the canonical supply ledger. Keeping active LP in a pool that users might
read as the PFTL bridge route would blur three different concepts:

- reserve NAV;
- valid supply;
- local secondary-market liquidity.

The LP being gone is consistent with the design boundary: the legacy pool should
not be used for wallet routing while the bridge-aware pool does not exist.

## 7. Target Architecture

### 7.1 Core Principle

PFTL decides what exists. Ethereum and Uniswap decide only where a verified
claim can trade.

The bridge should move supply, not inventory. A relayer may transport packets or
pay gas. A relayer must not be the mint authority, the NAV oracle, or the
accounting oracle.

### 7.2 Threat Model and Trust Assumptions

| Actor or component | Trusted for | Not trusted for | If compromised |
| --- | --- | --- | --- |
| User wallet | Local signing and destination approval. | NAV proof generation, bridge proof truth, relayer honesty. | User can lose only what they sign; wallet must show packet/action fields before signing. |
| Relayer | Liveness and gas payment. | Packet truth, route mutation, recipient mutation, mint authority. | User can self-relay or wait/refund; relayer cannot change a bound packet. |
| Threshold signers | Controlled-stage packet attestation only. | Public trustless finality. | They can authorize invalid packets unless caps and pauses contain damage. |
| Optimistic watchers | Detecting invalid optimistic packets and invalid refund attempts. | Always-online certainty. | Invalid packets can pass if every watcher fails; trust label must say optimistic. |
| Optimistic challenge resolver | Owner/governance-arbitrated resolution of challenged optimistic receipt claims until direct or succinct PFTL finality verification exists. | Unqualified trustless or objective finality. | A bad resolver can incorrectly resolve challenged claims within the owner-arbitrated trust model; route caps, pause controls, launch binding, monitoring, and wallet copy must disclose and contain that risk. |
| Direct/succinct verifier | Source-chain finality and receipt inclusion under its assumptions. | NAV truth beyond the finalized PFTL state it verifies. | Verifier bug can mint/release wrong supply; upgrades require governance and pause gates. |
| Verifier-key governance | Upgrades, pauses, trust-class changes. | Silent reclassification of route safety. | A malicious upgrade can weaken the bridge; config digest and wallet checks must expose this. |
| Uniswap pool/router | Swap execution. | NAV, finality, packet truth, refund authority. | Bad route or slippage can harm execution, but cannot mint without a consumed packet. |
| StakeHub operator | Launch automation and proof publication. | User custody or hidden route mutation. | Operator can misconfigure deploys unless config digest, fork rehearsal, and wallet checks catch it. |

### 7.3 Assets

Use placeholder names until the product asset is finalized:

| Name | Meaning |
| --- | --- |
| `pfUSDC` | PFTL vault-backed settlement asset funded by Circle CCTP and vault bridge receipts. |
| `a666` | Example next NAVCoin instance on PFTL. Could be any new NAV-tracked asset id. |
| `wA666` | Ethereum bridge-aware wrapped venue representation. |
| Legacy `a651` | Historical Ethereum token and PFTL demo NAVCoin, not the new route target unless migration is chosen. |

### 7.4 Supply Model

Use burn/mint for the first clean bridge-aware deployment.

Primary PFTL subscriptions mint new native NAVCoin supply against newly paid
settlement assets. They are not filled from a fixed inventory unless a route is
explicitly labeled as secondary-sale inventory.

Subscription pricing must use the finalized NAV snapshot before the subscriber's
own inflow is added to reserves:

```text
pricing_nav_epoch = latest finalized NAV epoch before subscription apply
pricing_reserve_snapshot = reserves at pricing_nav_epoch
pricing_supply_snapshot = supply at pricing_nav_epoch

minted_nav_atoms =
  floor(subscription_settlement_value_atoms / nav_price_at_pricing_epoch)
```

The exact integer scale and rounding rule must be part of the route config
digest. Default rounding is down in favor of the reserve; any dust is either
refunded or recorded as an explicit fee. No consensus or bridge-critical code may
use floating point arithmetic for this calculation.

After the fill, the state transition atomically applies:

```text
settlement reserves += subscriber payment
native a666 supply += minted_nav_atoms
subscriber balance += minted_nav_atoms
```

The subscriber's own payment is therefore included in backing after the mint, but
it is not allowed to raise the NAV price against that same subscription. Example:
if the finalized pre-subscription NAV is 1,000 USDC per NAVCoin and the user
subscribes 100,000 USDC, the expected fill is about 100 NAVCoin before fees and
rounding. The next NAV epoch may include the new 100,000 USDC reserve asset and
the new 100 NAVCoin supply.

For oversized or partially fillable subscriptions, the accepted amount and
unfilled amount must be separate machine-readable fields in the receipt. The
receipt must bind `requested_settlement_atoms`, `accepted_settlement_atoms`,
`minted_nav_atoms`, `refund_settlement_atoms`, the pricing epoch, and the exact
rounding rule. Wallets must display the accepted primary fill separately from any
secondary Uniswap action. A subscription receipt that omits this split is invalid
for the bridge-aware venue-token route.

PFTL to Ethereum:

```text
PFTL burns or debits X native a666
outstanding_bridge_claims += X
Ethereum verifies packet
Ethereum mints X wA666
outstanding_bridge_claims -= X
```

Ethereum to PFTL:

```text
Ethereum burns X wA666
PFTL verifies Ethereum burn event or accepted packet
PFTL mints or reissues X native a666
```

Define `authorized_valid_supply(a666)` as the live PFTL liability ledger: all
NAVCoin units issued by valid primary subscriptions or genesis seed events,
minus units redeemed or otherwise retired. It is not a static genesis amount and
not a venue-token balance read. The supply cap is a separate upper bound.

The conservation invariant is:

```text
pftl_spendable_supply(a666)
  + ethereum_spendable_supply(wA666)
  + other_registered_venue_supply(a666)
  + outstanding_bridge_claims(a666)
  + pending_return_import_claims(a666)
== authorized_valid_supply(a666)

authorized_valid_supply(a666) <= route_supply_cap(a666)
```

Enforcement is mandatory at the transition boundary, not by dashboard
reconciliation. Each state transition must update exactly one side of the
equation under replay-safe packet ids:

| Transition | Required accounting effect |
| --- | --- |
| Primary PFTL subscription | Price from the pre-inflow NAV epoch; collect settlement asset; increase reserve balance, `authorized_valid_supply`, and native NAVCoin spendable supply by the deterministic minted amount in one transition. |
| Primary PFTL redemption | Burn or lock native NAVCoin; decrease `authorized_valid_supply`; release settlement asset under the route's redemption rules. |
| PFTL export debit | Decrease PFTL spendable supply or lock spendability; increase `outstanding_bridge_claims`. |
| Ethereum consume | Mint or unlock exactly the packet amount; mark packet consumed; decrease `outstanding_bridge_claims` only when the source chain accepts the consume proof or accepted optimistic consume terminal state. |
| Source refund | Restore PFTL spendability; decrease `outstanding_bridge_claims`; permanently reject later destination consume. |
| Ethereum return burn | Decrease Ethereum spendable supply; create one `pending_return_import_claims` entry for PFTL. |
| PFTL return import | Restore PFTL spendability once; decrease `pending_return_import_claims`; mark the Ethereum burn event consumed. |

Gate 1 and Gate 4 tests must replay every transition from genesis snapshots and
assert the invariant after each block. If the current l1v2 bridge primitives
cannot expose enough state to run that replay, the implementation is incomplete;
the route remains `DISABLED`.

Expired packets stay in `outstanding_bridge_claims` until they reach a terminal
refund state. Expiry alone does not erase supply accounting.

Return burns stay in `pending_return_import_claims` until PFTL consumes the burn
event and restores native spendability. A burn event hash can be consumed once.

### 7.5 Packet State Machine

The PFTL-to-Ethereum bridge packet state machine is:

```text
None
  -> SourceDebited
  -> SettleableInFlight
     -> DestinationConsumed
     -> ExpiredRefundable
        -> SourceRefunded
```

Terminal safety rule:

```text
not (destination_consumed(packet_hash) && source_refunded(packet_hash))
```

Refunds require either:

- verified Ethereum non-consumption at a finalized block after the deadline; or
- an optimistic refund challenge window where anyone can submit a valid
  destination consume proof.

Absence of a relayed consume proof is not proof of non-consumption.
In the controlled consensus sprint, `destination_consumed` is an
operator-attested terminal event signed by the route issuer/reserve operator.
It moves supply from `outstanding_bridge_claims` to
`ethereum_spendable_supply_atoms` and blocks later source refund, but it is not
a Gate 5 proof and must keep the route in the `CONTROLLED` trust class.
Source refunds use a canonical
`postfiat.pftl_uniswap.non_consumption_commitment.v1` commitment over
`route_id`, `packet_hash`, and `refund_not_before_height` during the controlled
stage. That commitment prevents arbitrary placeholder hashes, but it is not a
verified non-consumption proof; Gate 5 must replace the commitment placeholder
with source-derived proof semantics before any trust-minimized refund claim.

Receipt retention decision (2026-07-01): the bridge must not depend on an
unbounded `pftl_uniswap_receipts` vector. The selected design is a consensus
checkpoint window. Add route/ledger checkpoint fields before public routing:
`pftl_uniswap_receipts_checkpoint_hash`,
`pftl_uniswap_receipts_checkpoint_count`, and
`pftl_uniswap_receipts_retained_from_index`. A deterministic maintenance
transition may fold the oldest contiguous finalized receipts into
`H(domain_tag, previous_checkpoint_hash, first_index, last_index,
ordered_receipt_hashes)` and prune those receipt rows. Replay then starts from
the checkpoint plus the retained live window. The existing
`MAX_PFTL_UNISWAP_RECEIPTS = 131_072` remains the live-window cap, not a
lifetime route cap. Pruning must not remove receipts needed for unresolved
`source_debited` packets, pending return imports, active challenge windows, or
any selected Gate 5 verifier retention window. Implementation is a protocol
follow-up owned by the consensus slice before public routing or before any route
can approach the cap; this sprint records the decision only.

### 7.6 Packet Schema

Use fixed-width canonical encoding for signed and verified payloads. JSON is
allowed for display only.

Minimum packet fields:

```text
BridgePacketV1 {
  domain
  version
  source_chain_id
  destination_chain_id
  source_bridge_id
  destination_bridge_address
  source_asset_id
  destination_token
  movement_model
  amount_atoms
  sender_commitment
  recipient
  refund_recipient
  action_kind
  action_payload_hash
  nonce
  source_height
  source_receipt_root
  source_expiry_height
  destination_deadline
  refund_not_before_height
  fee_amount_atoms
}
```

For atomic Uniswap execution:

```text
UniswapActionV1 {
  router
  token_in
  token_out
  pool_id_or_path_hash
  amount_in
  min_amount_out
  recipient
  deadline
}
```

The packet hash must bind the action payload hash. The action payload must bind
the router/path, output token, minimum output, recipient, and deadline.

The route config and launch config must also bind the settlement adapter address
separately from the Uniswap router. The adapter is the controlled settlement
surface the controller or wallet targets; the router is the Uniswap execution
endpoint the adapter is allowed to call. A config digest that collapses those
addresses into one field is invalid for Gate 3 and Gate 6.

Every PFTL bridge transition must also emit a canonical transition receipt:

```text
PFTLUniswapTransitionReceiptV1 {
  transition
  route_id
  route_config_digest
  route_trust_class
  native_nav_asset_id
  wrapped_navcoin_token
  ethereum_chain_id
  packet_hash?
  nonce?
  return_burn_event_hash?
  source_wallet?
  ethereum_recipient?
  pftl_recipient?
  amount_atoms?
  settlement_amount_atoms?
  pricing_nav_epoch?
  pricing_reserve_packet_hash?
  non_consumption_proof_hash?
  source_height?
  destination_deadline_seconds?
  refund_not_before_height?
  burn_height?
  finalized_height?
  state_before_hash
  state_after_hash
}
```

The receipt hash is the canonical PFTL hash of that receipt. The receipt root
for a block or replay segment is the canonical hash of the ordered receipt-hash
list. Reordering, dropping, or mutating any receipt must change the root. This
is an ordered batch commitment for the controlled prototype; Merkle inclusion
proofs or succinct receipt proofs remain part of the selected Gate 5 verifier
path.

### 7.7 Ethereum Contracts

Deploy a new Ethereum stack for `wA666` with these requirements:

| Contract | Requirement |
| --- | --- |
| `PFTLFinalityVerifier` or `OptimisticPFTLPacketAdapter` | Verifies or accepts PFTL finality and receipt inclusion under the stage trust model. |
| `ControlledPFTLReceiptVerifier` | Controlled-stage adapter that accepts only explicitly registered PFTL receipt root/hash/config/trust-class tuples. It is not a trustless verifier. |
| `WrappedVenueNAVCoin` | ERC-20 representation mintable and bridge-burnable only by the bridge controller. |
| `VenueBridgeController` | Consumes verified packets once, mints `wA666`, burns returns, enforces caps and route status. |
| `PacketReplayRegistry` | Stores consumed packet hashes and return nonces. |
| `UniswapSettlementAdapter` | Optional adapter for `mint_and_swap_uniswap`; never an accounting oracle. |
| `EmergencyPause` | Pauses mint/settlement paths while preserving read-only inspection and user return burns unless a separately tested return-pause is introduced. |

`VenueBridgeController` must expose `verifierTrustClass()` and include that trust
class in every route/pool config digest. It must reject `mint_and_swap` for
`DISABLED` routes, and it must emit the trust class in packet-consume events so
wallets and monitors can detect a mismatch between copy, config, and chain
state.

Do not reuse the legacy `a651` token as the bridge-aware token unless the
project explicitly chooses wrapper or migration, and then only under a separate
migration spec.

### 7.8 PFTL Changes

PFTL needs first-class bridge export/import state for NAVCoin assets:

- asset route registry mapping PFTL asset id to destination token, venue id,
  movement model, caps, status, and verifier mode;
- bridge export transaction that burns or locks native NAVCoin and emits a
  finalized receipt;
- outstanding claim ledger included in NAV/supply status;
- packet nonce registry;
- source refund transaction with non-consumption proof or optimistic challenge;
- Ethereum inbound verifier or optimistic event adapter for `wA666` burns;
- status RPC for packet, route, NAV epoch, reserve packet, supply packet, and
  bridge freshness.

The route registry must include `trust_class`, `movement_model`, `refund_model`,
and `live_value_enabled`. Wallets must reject public live-value routing when the
registry says `DISABLED`, or when the UI trust label differs from the registry
and Ethereum controller trust class.

The wallet should not be forced to infer this from raw balances.

### 7.9 Uniswap Pool

Create a new Uniswap pool for the new bridge-aware token.

Preferred first pool:

```text
chain: Ethereum mainnet, after testnet/fork gates
pair: wA666 / USDC
pool type: Uniswap v4
hook: address(0) initially
execution: mint-only plus optional mint-and-swap adapter
```

The initial Uniswap token side must come from the same canonical supply path as
user venue-token mints. Do not manually mint venue-token inventory outside the
bridge controller.

Pool seeding flow:

```text
operator or treasury settlement asset
  -> primary PFTL subscription priced from a finalized pre-inflow NAV epoch
  -> native a666 issued or directly encumbered for export
  -> bridge export packet consumed on Ethereum
  -> wA666 minted to the launch liquidity manager
  -> wA666 + USDC deposited into the official Uniswap position manager
```

The seed amount must be deterministic:

```text
seed_wA666_atoms = floor(seed_usdc_value_atoms / nav_price_at_seed_epoch)
```

The launch config digest must bind `seed_nav_epoch`, reserve packet hash, seed
USDC amount, seed `wA666` amount, pool key, tick range, fee tier, liquidity
recipient, LP token or position id recipient, and whether seed LP is controlled,
locked, or market-maker held.

Uniswap buyers receive existing `wA666` from pool or market-maker inventory at
AMM execution price. That trade does not create new NAVCoin supply. New supply
enters the Uniswap venue only through a primary subscription plus bridge export
packet, or through a later governance-approved liquidity operation using the
same packet rules. The wallet must label this difference:

```text
Primary PFTL mint: priced at finalized NAV snapshot; creates new supply.
Uniswap buy: priced by AMM liquidity and slippage; transfers existing wA666.
Composite mint-and-swap: primary mint plus bridge packet plus Uniswap execution.
```

Do not deploy a custom v4 hook in the first public bridge release unless it is a
real Uniswap v4 hook tested against faithful v4 callback shapes. The old
`NAVGuardHook` can be a market-ops evidence research track, not a launch
dependency for user bridge settlement.

### 7.10 Official Uniswap Address Gate

Before any fork rehearsal or live deployment, fetch and record the official
Uniswap v4 deployment table. As of the 2026-07-01 check:

| Chain | PoolManager | PositionManager | Universal Router | StateView |
| --- | --- | --- | --- | --- |
| Ethereum `1` | `0x000000000004444c5dc75cB358380D2e3dE08A90` | `0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e` | `0x66a9893cc07d91d95644aedd05d03f95e1dba8af` | `0x7ffe42c4a5deea5b0fec41c94c136cf115597227` |
| Arbitrum `42161` | `0x360e68faccca8ca495c1b759fd9eee466db9fb32` | `0xd88f38f930b7952f2db2432cb002e7abbf3dd869` | `0xa51afafe0263b40edaef0df8781ea9aa03e381a3` | `0x76fd297e2d437cd7f76d50f01afe6160f86e9990` |
| Base `8453` | `0x498581ff718922c3f8e6a244956af099b2652b2b` | `0x7c5f5a4bbd8fd63184577525326123b519429bdc` | `0x6ff5693b99212da76ad316178a184ab56d299b43` | `0xa3c0c9b65bad0b08107aa264b0f3db444b867a71` |

The gate is not "these addresses are in this document." The gate is "the
deployment script fetched or pinned the current official table and the manager
approved the exact addresses in the config digest."

## 8. User Flows

### Flow A: PFTL Primary Mint Only

```text
pfUSDC
  -> transparent NAV subscription
  -> native a666 on PFTL
```

This proves NAV accounting but does not touch Ethereum.

Required user evidence:

- pfUSDC amount and receipt source selected;
- pricing NAV epoch and reserve packet hash;
- pre-subscription NAV price used for the fill;
- supply packet hash;
- minted a666 amount;
- rounding, dust, or fee treatment;
- PFTL transaction id and finality certificate.

The wallet must distinguish primary subscription pricing from Uniswap secondary
market pricing. In the primary path, the displayed fill is computed from the
pre-subscription NAV snapshot, then the user's settlement asset is added to
reserves after the mint. In the Uniswap path, execution follows AMM price and
slippage unless the route explicitly composes a primary subscription with a
bridge mint-and-swap packet.

### Flow B: Mint Ethereum Venue Token

```text
native a666 on PFTL
  -> PFTL bridge export packet
  -> Ethereum verifies packet
  -> wA666 minted to user's EVM address
```

Required user evidence:

- PFTL packet hash;
- source debit transaction;
- finalized PFTL receipt root;
- verifier mode and trust label;
- Ethereum consume transaction;
- destination token amount and token address.

### Flow C: Atomic Mint-and-Swap

```text
native a666 on PFTL
  -> PFTL bridge export packet with UniswapActionV1
  -> Ethereum consumes packet
  -> mints wA666 to adapter
  -> adapter executes exact-input swap
  -> output token sent to recipient
```

Preferred failure behavior:

```text
if Uniswap swap reverts:
  revert entire Ethereum settlement
  consumed[packet_hash] remains false
  wA666 mint reverts
  packet can be relayed again before deadline or refunded after safe expiry
```

An alternative "claim wA666 if swap fails" mode is allowed only if explicitly
implemented, displayed, and tested.

### Flow D: Composite Primary Mint to Ethereum Venue

```text
pfUSDC on PFTL
  -> primary NAV subscription priced from pre-inflow NAV snapshot
  -> native a666 minted and immediately exported, or minted to wallet first
  -> PFTL bridge export packet
  -> Ethereum consumes packet
  -> wA666 minted to user or settlement adapter
  -> optional Uniswap exact-input swap
```

This is the wallet route that lets a user start with PFTL settlement balance and
end with the Uniswap-tradable token or Uniswap output. The quote must disclose
both prices if both are involved:

- primary NAV subscription price from the finalized PFTL NAV epoch;
- Uniswap AMM execution price, slippage, deadline, and minimum output.

If the route mints native `a666` and exports it in one PFTL transaction, the
minted amount must be either credited to the user's native balance or moved into
`outstanding_bridge_claims`; it cannot disappear into an off-ledger operator
inventory. If the Ethereum consume fails after source export, refund restores
the user's native `a666` spendability unless the route explicitly offers and
tests USDC redemption instead.

### Flow E: Return from Ethereum to PFTL

```text
wA666 burn on Ethereum
  -> finalized Ethereum event or accepted optimistic event packet
  -> PFTL consumes event once
  -> native a666 minted or reissued to PFTL recipient
```

This must exist before the public UI presents the route as round-trippable.

## 9. Execution Plan and Stage Gates

### 9.1 Critical Path

| Gate | Primary owner | Target timing | Effort/risk | Blocks |
| --- | --- | --- | --- | --- |
| Gate 0 | Protocol + product | Day 0 | Medium | All implementation. |
| Gate 1 | l1v2 protocol | Days 1-3 | High | Ethereum packet consumer and all refund tests. |
| Gate 2 | Ethereum contracts | Days 2-5 | High | Uniswap adapter and wallet route. |
| Gate 3 | StakeHub + Ethereum contracts | Days 4-6 | Medium | Public pool launch. |
| Gate 4 | l1v2 protocol + Ethereum contracts | Days 5-8 | High | Any round-trip claim. |
| Gate 5 optimistic | Protocol security | Days 6-12 | Very high | Trust-minimized public claim. |
| Gate 5 direct/succinct | Protocol research | Separate milestone | Very high | Unqualified trustless claim. |
| Gate 6 | Release owner | After selected Gate 5 | High | Live public routing. |

The schedule risk is Gate 5, not Uniswap. The security risk is refund and
challenge economics. The optimistic path needs a written economic bound:

```text
challenge_bond >= max(
  challenger_gas_cost_with_margin,
  griefing_cost_bound,
  invalid_profit_bound + safety_margin,
  policy_floor
)
challenge_window >= destination_finality + proof_submission_margin
```

If that bound is not available, the route remains `CONTROLLED` or `DISABLED`.
Final numbers must be bound into the launch config digest and published with the
route. The bound must use current destination-chain gas, proof size, route caps,
and challenge rules. Do not lower the bond by dividing through an assumed
challenge probability.

The bond does not make an unchallenged invalid packet safe. If every watcher is
offline or censored, any finite bond can fail. The bond only makes invalid
submissions economically unattractive when at least one watcher challenges and
pays challengers enough to act. Route caps define the maximum loss accepted if
the optimistic assumption fails. This is why the route label is `OPTIMISTIC`,
not `TRUSTLESS_FINALITY`.

Gate 5 must publish a parameter table with these fields:

| Parameter | Meaning |
| --- | --- |
| `packet_notional_cap` | Maximum value of one packet under the route cap. |
| `invalid_profit_bound` | Conservative upper bound on attacker profit from one invalid accepted packet. Must be at least the packet notional cap unless a stronger proof is written. |
| `challenger_gas_cost_with_margin` | Destination challenge gas, proof-generation cost if paid by challenger, and congestion margin in the same value unit as the bond. |
| `griefing_cost_bound` | Maximum tolerated cost of invalid or spam challenges under the challenge rules. |
| `policy_floor` | Governance floor, usually expressed as a multiple of packet notional. |
| `destination_finality` | Destination-chain block or time delay before a packet/event can be treated as final. |
| `proof_submission_margin` | Time for watcher detection, proof build, propagation, and inclusion. |
| `watcher_liveness_slo` | Monitoring SLO only. It must not reduce `challenge_bond`. |

The route must fail closed when current configured bond, cap, finality, watcher
liveness, or gas assumptions no longer satisfy the published bound.

Gate work can run in parallel, but exit dependencies cannot be skipped. Gate 1
packet/state work, Gate 2 Ethereum controller scaffolding, and Gate 3 Uniswap
fork setup can overlap. Gate 4 depends on both sides having replay-safe consume
and burn paths. Gate 6 depends on the chosen Gate 5 evidence and cannot be
papered over by a fork test alone.

### 9.2 Controlled MVP Track

The controlled MVP is allowed to ship before a trustless verifier only if the
route label, caps, and wallet copy make the trust boundary unavoidable.

| MVP step | Shippable result | Hard stop |
| --- | --- | --- |
| MVP 1 | Fractional primary PFTL NAV mint using pre-inflow NAV pricing for arbitrary capped subscription sizes, not fixed seed inventory. | No Ethereum or Uniswap claim. |
| MVP 2 | Controlled mint-only bridge to `wA666` on fork/devnet with exact packet replay tests. | No public Uniswap routing. |
| MVP 3 | Controlled `wA666/USDC` pool seed rehearsal with canonical seed packet and LP custody digest. | No live user routing unless capped and labeled `CONTROLLED`. |
| MVP 4 | Capped internal or explicit-beta composite route: `pfUSDC -> primary mint -> bridge -> wA666` or Uniswap output. | No `trustless` copy, no uncapped value, no legacy pool fallback. |
| MVP 5 | Return path rehearsal twice without manual state edits. | No round-trip public claim until it passes. |

This track is not a shortcut around the gates. It is the smallest useful path
for engineering validation and capped beta testing while Gate 5 continues. Every
MVP release still needs a config digest, route cap, pause switch, packet status
RPC, and wallet warning matching the deployed trust class.

### 9.3 Implementation Status Checklist

Status date: 2026-07-01.
Work timer started: 2026-07-01 17:16:04 UTC.

To check current UTC time:

```bash
date -u '+%Y-%m-%d %H:%M:%S UTC'
```

To check elapsed time since this timer started:

```bash
python3 - <<'PY'
from datetime import datetime, timezone

start = datetime(2026, 7, 1, 17, 16, 4, tzinfo=timezone.utc)
now = datetime.now(timezone.utc)
print(f"now={now:%Y-%m-%d %H:%M:%S UTC}")
print(f"elapsed={str(now - start).split('.')[0]}")
PY
```

This checklist is the live completion tracker for the redeployment goal. A gate
is `Complete` only when the required evidence is checked in or linked from this
document. Local code and focused tests without a devnet, fork, or deployment
evidence packet count as `Partial`, not complete.

Current summary:

- Formal launch gates complete: `0 / 7`.
- Controlled MVP steps complete with evidence: `5 / 5`.
- Controlled MVP steps complete at code-test level: `5 / 5`.
- PFTL-Uniswap consensus-completion sprint tasks complete: `6 / 6`;
  receipt-retention pruning is recorded as a required follow-up, not shipped
  pruning code.
- Public wallet routing status: `DISABLED / NOT READY`.
- Current permitted claim: consensus-backed controlled PFTL-Uniswap bridge round
  trip with operator-attested destination events and source-chain facts,
  wallet-proxy digest authority from node-produced route digests, PFTL-side
  bridge accounting/replay, the controlled Ethereum packet consumer, the
  controlled Uniswap v4 fork seed/buy/sell rehearsal, the controlled return-path
  round-trip rehearsal, and MVP4 Gate 0 controlled launch-config digest,
  wallet/proxy controlled-beta route acceptance, run-packet checks, controlled
  fork consume/swap execution, clean local Gate 1 devnet replay, Gate 5
  optimistic verifier code-test evidence, fork-measured optimistic
  packet/challenge evidence, watcher runbook/SLO evidence, Gate 5 optimistic
  preflight evidence, an optimistic launch binding digest, Gate 6 wallet/proxy
  acceptance and display-policy evidence, and Gate 6 monitoring/runbook
  preflight evidence have checked evidence. Owner signoff, production watcher
  execution against final deployed addresses, trustless verifier evidence, Gate
  6 final deployment digest, release approval, monitor alert delivery, and
  public-route evidence remain pending. Do not call this trustless or public
  routing.
- 2026-07-01 CTO review: the checked PFTL block and all
  `docs/evidence/pftl-uniswap-*` packets were independently verified (cited
  test suites re-run, evidence digest chains recomputed from disk) and stand at
  their stated controlled sidecar scope. The review found two release-blocking
  defects in the Ethereum contracts; both blocking fixes now have fresh forge
  and regenerated Gate 5 fork/preflight evidence. Resolver rotation/governance
  and owner-arbitrated wallet/evidence disclosure are now implemented with
  fresh Forge, wallet, Gate 5, and Gate 6 preflight evidence. Pause semantics now block inbound mint/settlement while preserving return burns for wrapped holders, with fresh Foundry and regenerated Gate 5/Gate 6 evidence. Replay persistence is now moved into a standalone `PacketReplayRegistry` with fresh Foundry and regenerated Gate 5/Gate 6 evidence. Adapter/router trust now verifies actual token balance delta and pool-bound routers, with fresh Foundry and regenerated Gate 5/Gate 6 evidence. Route cap semantics now use net outstanding wrapped exposure while preserving lifetime minted as an audit counter, with fresh Foundry and regenerated Gate 5/Gate 6 evidence. Wrapped-token decisions are now explicit: zero-value transfers follow ERC-20 behavior, mint/burn remain nonzero-only, and controller lock remains non-repointable, with fresh Foundry and regenerated Gate 5/Gate 6 evidence. Forge-level official-Uniswap fork evidence now checks the bound v4 deployment addresses on a mainnet fork. The CTO Ethereum checklist is complete; production watcher/signoff/public release blockers still remain.
  block. See `CTO Review Directives (2026-07-01)` and the Ethereum task block.
- Current forbidden claims: public Uniswap launch, public round trip, live
  trustless bridge, unqualified `trustless` routing, or canonical `a651`
  migration.

Formal gate status:

| Gate | Status | Done | Not done / evidence gap | Next action |
| --- | --- | --- | --- | --- |
| Gate 0: Spec and legacy boundary | Partial / owner signoff pending | The spec states the default new-token strategy `a666/wA666`, burn/mint movement, fractional primary subscription economics, legacy `a651` historical treatment, route completeness requirement, tradable venue supply requirement, and controlled/trust label requirements. Checked evidence at `docs/evidence/pftl-uniswap-gate0-legacy-route-rejection-2026-07-01/` proves the wallet action verifier rejects route `legacy_a651_uniswap` as inspection-only and rejects legacy PFTL/Ethereum `a651` token fields when a prepared wallet action claims the composite PFTL-to-Ethereum venue route; `node --test src/lib/navswap-actions.test.js` reports `29` passing tests and `0` failures. Checked controlled launch-config evidence at `docs/evidence/pftl-uniswap-gate0-launch-config-2026-07-01/` binds the selected `a666/wA666` strategy, burn/mint model, fractional primary economics, legacy disablement, controlled trust class, selected optimistic Gate 5 path, public routing disabled state, and evidence refs under digest `62058b7756cfa1fc219b65df469447e9fecc3e3d0b1c3c1e395787c8337d910227bc7699a226c2bf214d68f78e86ce6d`. | Owner signoff is still pending; `launch-config.json` explicitly records `owner_signoff.status=pending_owner_signoff`. No live route claim is allowed. | Record owner signoff against the checked config digest before any live route claim. |
| Gate 1: PFTL packet prototype on local devnet | Evidence complete / formal gate dependency pending | Rust bridge code has primary subscription, export debit, destination consume marker, source refund, return burn, return import, deterministic receipt replay, mutation/reordering/wrong-final/empty replay failures, fractional primary mint tests, and launch-config-bound packet preflight tests. Node/RPC/SDK/Python plumbing exposes `navcoin_bridge_receipt_replay`; node/RPC/SDK plumbing exposes `navcoin_bridge_packet_preflight`. Checked controlled-sidecar evidence at `docs/evidence/pftl-uniswap-gate1-2026-07-01/` initializes the route, applies primary mint/export/consume/return transitions, persists five receipts, and verifies replay status `verified` with receipt root `d77256242815519edf4127cac1c6ed90914b629df6d66b80bf29e749d0fdd8cc330999cc9a001be9b8aad0483b480900`. Fresh local-devnet evidence at `docs/evidence/pftl-uniswap-gate1-devnet-2026-07-01/` creates a clean four-validator devnet, proves validator consensus before and after the Gate 1 bridge-side transition set, copies the node0 bridge ledger/receipt files, and verifies `navcoin-bridge-receipt-replay` status `verified`, receipt count `5`, receipt root `d77256242815519edf4127cac1c6ed90914b629df6d66b80bf29e749d0fdd8cc330999cc9a001be9b8aad0483b480900`, final ledger hash `712463fcda71681551f54a990c571845d60ed9e738a89443a1c59d08d1b80e8970ac3bb345612ec5e4ebc3dca74f7ff1`, and supply invariant `true`. Focused local evidence from `cargo test -p postfiat-node navcoin_bridge_packet_preflight --lib` reports `2` passing tests and `0` failures, including rejection of mismatched route digest, settlement asset, native NAV asset, wrapped venue token, pool id, USDC token-out, pricing NAV epoch, and pricing reserve packet hash before relay. Focused local evidence from `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib` reports `1` passing test and `0` failures, including duplicate primary subscription nonce rejection, persisted primary receipt bindings, deterministic per-wallet native balance rows, and supply `ledger_hash` output. Focused local evidence from `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_rejects_export_from_wrong_native_wallet --lib` reports `1` passing test and `0` failures, proving an export debit fails when the source wallet lacks native balance even though route aggregate spendable supply exists. Focused local evidence from `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_exports_refunds_and_preserves_invariant --lib`, `cargo test -p postfiat-bridge pftl_uniswap_status_reports_expose_route_packet_claims_and_supply --lib`, and `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib` proves export debit creates a `SourceDebited` packet, debits native/PFTL spendable supply, increments outstanding bridge claims, exposes packet/claim status rows, and later terminal transitions clear the outstanding claim. | This is controlled local-devnet and local test evidence, not a public bridge or selected Gate 5 verifier path. Gate 0 signoff/config and later public-route gates still block launch. | Use this as the Gate 1 local-devnet and preflight evidence packet for Gate 5 verifier work; keep public routing disabled. |
| Gate 2: Controlled Ethereum packet consumer | Evidence complete / formal gate dependency pending | `docs/evidence/pftl-uniswap-gate2-2026-07-01/` links `forge test --match-path test/PFTLUniswapHandoffController.t.sol -vv` with `20` passing tests, including `testGate1SidecarExportVectorConsumesMintOnly`, which consumes the exact Gate 1 sidecar export packet hash, export receipt hash, receipt root, route config digest, asset ids, chain id, and recipient into `wA666`. The controller/verifier bind receipt acceptance to the exact EVM packet digest, preserve 48-byte PFTL ids as `bytes`, reject wrong chain, wrong bridge, wrong wrapped token, mutated recipient, mutated action payload, replay, cap overflow, stale pricing, unaccepted receipt, disabled route, unauthorized executor, and reentrancy attempts. `verifierTrustClass()` returns the machine-readable controlled trust class. | Formal Gate 2 is not a public route approval. It still depends on Gate 0 signoff/config digest and later wallet evidence before any controlled public routing. | Use this as the controlled Ethereum consumer evidence packet for MVP 3 pool seeding; do not enable public routing until Gate 3 and wallet evidence pass. |
| Gate 3: Uniswap fork integration | Evidence complete / formal gate dependency pending | `docs/evidence/pftl-uniswap-gate3-2026-07-01/` records a controlled Ethereum fork rehearsal for route `pftl-a666-ethereum-wA666-usdc-gate3-v1`. The route/launch configs bind the official Uniswap v4 deployment snapshot, route config digest `23c4522e0f65c728e555418e486bbf09ad85f335df2b99b58c17415ed3836ff78c31ce271244ad0d66cc78aa35c57e71`, launch config digest `49f01c07dfc638250d1f31dbbceaa0dd259ccabbaff323341c0a72c9ab95611e7609d78e523d70f784b10a07a78052f1`, pool id `0x5c7ea7b5e0091029297604a5908e13ee671b937917c96bc62e940796a269443d`, seed NAV epoch `7`, fee tier `500`, full-range tick bounds, LP recipient/custody, controller, adapter, verifier, and trust class. The fork collector checked bytecode at official PoolManager, PositionManager, Universal Router, Permit2, and StateView; consumed a canonical seed export packet; seeded the v4 pool; ran external buy/sell transactions with nonzero deltas (`1,000,000` USDC atoms in for `988` wA666 atoms out; `494` wA666 atoms in for `500,412` USDC atoms out); showed StateView liquidity `3,162,277` after seed/buy/sell; proved canonical supply stayed `100,000` atoms across external AMM trades; submitted mint-only and mint-and-swap packets; and recorded rehearsal digest `74f78d4dbf19a0b79e3570b90d66ebd094f9ff339f4d8f8b114a581d6a7f3b369443fa6b91ac644838944405b7e1c935` through `navcoin-bridge-record-fork-rehearsal`. | Formal Gate 3 is still not public routing. It depends on Gate 0 signoff/config policy, wallet public-route acceptance evidence, and the selected Gate 5 verifier path before any public route claim. The rehearsal uses a controlled fork route and controlled verifier. | Use this as the controlled pool-seed evidence packet for verifier and wallet acceptance work; keep public routing disabled until Gate 5 and Gate 6 pass. |
| Gate 4: Return path | Evidence complete / formal gate dependency pending | `docs/evidence/pftl-uniswap-gate4-2026-07-01/` records a controlled return-path fork/sidecar rehearsal for route `pftl-a666-ethereum-wA666-usdc-gate4-v1`. The route config digest is `23522327f42b1a9c9bbfb8a87ff5005cc447b52fd32f3ee3f0f2320076cb201b9eea0027f38965b1fb29d71a14db5a76`; the export receipt root is `74ae07b76d7ef844f8f268576136fea3771f3aaec668965ee492bcde6eaa73e083b1ac11d5e896ab6d5c50015b53cc9a`; the final replay root is `65621357b6749a1d3675f5a2ccf9e3f6c83cca820b908d43887337cdcfeb9715ae47f807c13c001aa8e84f484163c111` with replay status `verified` and `9` receipts. Two PFTL exports were consumed on the fork (`0xe63e973389d844ff384d977b70c33a9e384a636a7fe3938d56ec7e683a45c00d`, `0xd34ad2d58fa695cc256fc3abfc78f608d8ee86147cbe99f0d1f47a414c70278b`), then burned and imported back through the node CLI. Roundtrip 1 burned `25` atoms in tx `0xf93c50195569f9956c39d5759c6512ee1aed07e2c51eca75a586cca9ee3f530a` with burn id `6e4270de4a8a5d4c43f519608ea1c30ae2d6966ecad23686ffc08c1241bd277f`; roundtrip 2 burned `17` atoms in tx `0x50dccb82750f2e5660d747a80c57ca43e72c7ba1ac48f464b19e1c1270876c22` with burn id `9627f50555b4beb2cfeda5748ea1142252edf9402f0c0a7782c838ffdc7b374e`. Final supply evidence shows `42` PFTL spendable atoms, `0` Ethereum spendable atoms, `0` outstanding bridge claims, `0` pending return imports, `0` final wrapped total supply, and invariant `true`. | This is controlled fork/sidecar evidence, not public or wallet-visible routing. Gate 4 still depends on Gate 0 signoff/config policy, the selected Gate 5 verifier path, and Gate 6 wallet acceptance before any public round-trip claim. | Use this as the controlled round-trip evidence packet for verifier and wallet acceptance work; keep public routing disabled until Gate 5 and Gate 6 pass. |
| Gate 5: Optimistic or trustless verifier | Partial / optimistic fork evidence; production watcher/signoff pending | The selected in-repo path is optimistic for the first non-controlled verifier work. `OptimisticPFTLReceiptVerifier` in `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol` exposes trust class `OPTIMISTIC`, bonded permissionless receipt-claim posting, bonded permissionless challenges with nonzero evidence hash, finalization after the challenge window, challenge resolution, fail-closed rejection/refund for unresolved challenges, source-receipt claim replay binding, pull-based bond credits, and fail-closed `isReceiptAccepted` behavior. Checked evidence at `docs/evidence/pftl-uniswap-gate5-optimistic-2026-07-01/` includes `parameters.json` with every required Gate 5 parameter-table field, `watcher-runbook.md`, `watcher-slo-evidence.json`, `reports/pftl-uniswap-handoff-controller-forge-test.txt`, `reports/pftl-uniswap-official-v4-fork-forge-test.txt`, and `reports/gate5-optimistic-preflight.json`; `forge test --match-path test/PFTLUniswapHandoffController.t.sol -vv` reports `34` passing tests and `0` failures, and `forge test --match-path test/PFTLUniswapOfficialFork.t.sol --fork-url https://ethereum-rpc.publicnode.com --fork-block-number 25440268 -vv` reports `1` passing fork test and `0` failures. The Gate 5 optimistic preflight reports `preflight_passed=true`, `public_launch_ready=false`, validates the parameter table, watcher SLO/runbook, fork claim and challenge reports, launch binding digest, and disabled public routing state, and records the remaining blockers: missing final manager approval, missing production watcher run against final deployed verifier/controller/replay registry addresses, and missing Gate 6 final deployment digest/release approval. Tests prove underbonded posts and challenges reject, challenged claims stay unaccepted and block settlement before mint, valid challenge resolution keeps settlement rejected, unresolved challenges fail closed after the resolution deadline without accepting the claim or paying the challenger bond to the poster, source-receipt reuse cannot back a second accepted optimistic claim, valid unchallenged claims finalize after the window and allow consume, late challenges cannot grief finalized valid claims, router return values cannot overstate actual token settlement, controller deployment rejects a router whose pool id differs from the route, route supply cap is enforced against net outstanding wrapped exposure after return burns, zero-value wrapped token transfers follow ERC-20 behavior, locked wrapped-token controllers cannot be repointed, and official Uniswap v4 PoolManager, PositionManager, Universal Router, Permit2, and StateView bytecode exists on the mainnet fork. Fork evidence at `docs/evidence/pftl-uniswap-gate5-optimistic-fork-2026-07-01/` deploys the optimistic verifier/controller on an Ethereum fork, initializes route `pftl-a666-ethereum-wA666-usdc-gate5-optimistic-v1` with route config digest `127d9a525a0bc688d70cd812a692af33fa41a365b420a592d93904774e527d129f9bb227d70eb9b0c555007b5f5515cd`, finalizes seed, mint-only, and mint-and-swap claims before consume, executes external buy/sell, records challenge gas with 4x margin `414108696044016` wei, writes parameter calibration, and binds verifier parameters/address, controller address, replay registry address, owner-arbitrated challenge resolution mode, resolver address, resolver owner, launch digest, and public-routing-disabled state under optimistic binding digest `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`. | This is fork-measured optimistic evidence, not final public launch approval and not direct/succinct PFTL finality verification. Production watcher service has not run against final deployed verifier/controller/replay registry addresses. Final manager approval of the optimistic launch binding digest is pending. Gate 6 final deployment digest, release approval, and monitor alert delivery evidence are pending. No direct/succinct finality verifier exists. Direct on-chain objective challenge evidence remains a later trustless-verifier milestone because the current mode is disclosed as owner-arbitrated. The `OPTIMISTIC` label may not be shown to wallets as an unqualified trust-minimized label while resolution is owner-arbitrated (see amended optimistic exit criteria). | Run the production watcher/SLO check against final deployed verifier/controller/replay registry addresses, record manager approval of the optimistic binding digest, and complete Gate 6 final deployment digest/release approval/monitor alert delivery before any public optimistic route claim. |
| Gate 6: Public redeploy | Partial / wallet + monitoring preflight evidence; public deploy blocked | Wallet/proxy route acceptance policy now has checked evidence at `docs/evidence/pftl-uniswap-gate6-wallet-acceptance-2026-07-01/`. The focused wallet route/action suite report `reports/wallet-route-acceptance.tap` records `54` passing tests and `0` failures; the full wallet lib suite report `reports/wallet-npm-test.tap` records `169` passing tests and `0` failures; the proxy NAVSwap adapter report `reports/wallet-proxy-navswap-adapter.txt` records `navswap adapter tests passed`. The wallet policy accepts a capped `OPTIMISTIC` public-beta route only when public routing is explicitly enabled, route cap, remaining cap, packet cap, poster bond, challenger bond, challenge gas margin, challenge window, challenge resolution window, challenge resolution mode, watcher liveness SLO, fail-closed conditions, a 96-hex optimistic launch binding digest, and the Gate 6 pre-sign display payload are present before signing. The required display payload includes canonical NAV, Uniswap market price, proof freshness, bridge verifier mode, challenge resolution mode, packet status, refund deadline, and route trust label. The wallet-owned action verifier accepts the PFTL-Uniswap route for `CONTROLLED` non-public beta actions or capped `OPTIMISTIC` public-beta actions only; the optimistic action path requires the same bond/window/SLO, binding digest, fail-closed, and visible trust fields before signing. The proxy refuses finality-class display unless the PFTL route registry, Ethereum controller, and config digest all report `TRUSTLESS_FINALITY`; before that three-way agreement it exposes `DISABLED`, sanitizes the visible verifier mode to `finality_pending`, and keeps quote/run disabled. The checked wallet/proxy paths reject or avoid trustless display copy on controlled/optimistic/legacy routes, missing challenge terms/watcher SLO, missing Gate 6 pre-sign display fields, uncapped or over-cap routes, disabled public routing for public beta, legacy fallback, misleading visible trust labels, arbitrated optimistic routes whose visible label omits `ARBITRATED`, and legacy `a651` token/pool fields. Monitoring/runbook evidence at `docs/evidence/pftl-uniswap-gate6-monitoring-2026-07-01/` adds required alert classes for stale proof, route pause, cap exhaustion, verifier issue, challenge event, replay rejection, and pool liquidity drop; `reports/gate6-monitoring-preflight.json` reports `monitoring_preflight_passed=true`, `gate5_preflight_passed=true`, `wallet_acceptance_preflight_passed=true`, `public_launch_ready=false`, no validation errors, config digest `31e02e111c2be4ef45934ddc8cced0e4187108a9b56c2121a1a5847f596bb21231b43845a768fa03e6430568cd85b6bf`, and the Gate 5 optimistic binding digest `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`. | This remains pre-public evidence only. There is still no final public deployment digest, release-owner approval, live route enabled, production watcher evidence against final deployed verifier/controller/replay registry addresses, resolver address, and resolver owner, or monitor alert delivery report. The runbook is not final launch approval. | Do not enable public routing until Gate 5 production watcher/signoff is complete and Gate 6 final deployment digest, release approval, production watcher report, monitor alert delivery report, and route caps are recorded. |

Controlled MVP status:

| MVP step | Status | Done | Not done / evidence gap | Next action |
| --- | --- | --- | --- | --- |
| MVP 1: Fractional primary PFTL NAV mint | Complete for controlled sidecar and local-devnet evidence | Fractional primary subscription economics are implemented in Rust tests: arbitrary capped subscription sizes mint fractional native NAV from pre-inflow NAV pricing and update reserves/supply/user balance. Checked evidence at `docs/evidence/pftl-uniswap-gate1-2026-07-01/` includes `inputs/primary-subscription.json`, `reports/02-primary-subscription.json`, `reports/11-supply-status.json`, and `reports/12-receipt-replay.json`; the evidence mints `100` native NAV atoms from `200` settlement atoms at pre-inflow price `2`, supply invariant is `true`, and receipt replay status is `verified`. Local-devnet evidence at `docs/evidence/pftl-uniswap-gate1-devnet-2026-07-01/` verifies the same receipt root and final ledger hash from a clean four-validator devnet node0 data dir with validator consensus before and after. | No Ethereum or Uniswap claim is made by this MVP. | Use this evidence as the controlled baseline for MVP 2; do not enable Ethereum or Uniswap routing until MVP 2 and MVP 3 pass. |
| MVP 2: Controlled mint-only bridge to `wA666` | Complete for controlled local-EVM evidence | PFTL-side export accounting and receipt replay are checked in under `docs/evidence/pftl-uniswap-gate1-2026-07-01/`. Ethereum-side controlled mint evidence is checked in under `docs/evidence/pftl-uniswap-gate2-2026-07-01/`; `testGate1SidecarExportVectorConsumesMintOnly` consumes the exact Gate 1 export vector and mints `40` `wA666` atoms to the recorded Ethereum recipient. | This is not public routing and not a return path. | Proceed to MVP 3/Gate 3 evidence for pool seed and external tradeability. |
| MVP 3: Controlled `wA666/USDC` pool seed rehearsal | Complete for controlled fork evidence | `docs/evidence/pftl-uniswap-gate3-2026-07-01/` contains the generated route config, launch config, sidecar primary/export receipts, fork collector output, recorded fork rehearsal sidecar, and summary. The seed supply comes from primary subscription plus bridge export, not manual EVM minting. The evidence publishes pool id, LP custody policy, seed packet, seed mint tx `0x141ff50a2ad155c41c557c1883cd281eb02872c036a9f8dd519f8d8de8333ab5`, seed LP tx `0x2d6bdd9568646eb295607572f6c7fee768689ac7197f5b5451cbe1bf33070e60`, external buy tx `0xeaa0b620061287a7b70499953d532ad1da0ba9a0ca7ed480ae9c46fa1af57dc3`, external sell tx `0x4251f13d2efacfd5ba8091b442487bc9c9dfa52095b985bc1d3e776d287f80f2`, mint-only tx `0xfcaa679eb8a966858e582491eb9a7256ae8139c302ba288f3e64a9cc19dfdb17`, and mint-and-swap tx `0x833397598097c0a20cb09d8d699b5ff52be9424511c6c5504e8ff535de3085ab`. | This is controlled fork evidence only. It is not a public Uniswap deployment, not trustless verification, and not wallet-enabled routing. | Proceed to MVP 4 only with explicit `CONTROLLED` labels, route caps, pause behavior, and no legacy fallback. |
| MVP 4: Capped internal/beta composite route | Complete for controlled beta fork evidence | The required route shape and labels are specified. Checked evidence at `docs/evidence/pftl-uniswap-mvp4-beta-route-2026-07-01/` adds wallet-side route policy for `uniswap_atomic_handoff`, allows signer submission only for controlled composite primary-mint-to-Ethereum handoff actions, and proves `CONTROLLED` labeling, explicit/internal beta gating, route cap enforcement, packet cap enforcement, pause rejection, public-route rejection, and no legacy `a651` fallback. The wallet proxy fails closed unless `NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE=true` and the route is capped, unpaused, non-public, non-legacy, and `CONTROLLED`; when `NAVSWAP_ENABLE_UNISWAP_BETA_RUNS=true`, it emits a bounded `postfiat-pftl-uniswap-controlled-beta-run-packet-v1` packet. `reports/uniswap-beta-gate3-bound-run-packet.json` proves the proxy capability, quote, and run packet bind the checked Gate 3 route config digest `23c4522e0f65c728e555418e486bbf09ad85f335df2b99b58c17415ed3836ff78c31ce271244ad0d66cc78aa35c57e71` without digest drift. Fresh controlled fork evidence at `fork-execution/reports/13-mvp4-beta-run-packet.json` reports capability status `controlled_beta_run_ready`, quote status `controlled_beta_run_ready`, run status `controlled_beta_packet_ready`, `public_routing_enabled=false`, amount `10`, minimum output `1`, and the same Gate 3 route digest. `fork-execution/reports/14-mvp4-beta-consume-evidence.json` records destination consume/swap tx `0x6e2335951893c4f09184c3879fde6d66e9ec69d798beb7c642617f5a6c113ef3`, terminal state `destination_consumed_and_swapped`, packet consumed `true`, source packet consumed `true`, and `9,089` USDC output atoms. | This is controlled fork evidence only. It is not public routing, not an uncapped value route, and not trustless verification. Formal Gate 0, selected Gate 5 verifier evidence, and Gate 6 wallet acceptance still block public launch. | Use this as the controlled beta route execution packet for Gate 5 and Gate 6 work; keep public routing disabled until the formal launch gates pass. |
| MVP 5: Return path rehearsal twice | Complete for controlled fork/sidecar evidence | `docs/evidence/pftl-uniswap-gate4-2026-07-01/` executes two complete controlled `PFTL -> Ethereum -> PFTL` round trips without manual ledger edits. The evidence mints `42` `wA666` atoms from two consumed exports, records two `burnForPftlReturn` events whose burn ids match the PFTL CLI derivation, imports both burns, and verifies final receipt replay status `verified`. Final supply has `42` atoms restored to PFTL spendable supply, `0` Ethereum spendable supply, `0` outstanding claims, `0` pending return imports, and EVM wrapped supply `0`. | This is controlled evidence only; it is not public routing and not wallet acceptance. | Use this evidence for Gate 5 verifier work and Gate 6 wallet acceptance; do not enable public route labels yet. |

### CTO Review Directives (2026-07-01)

Review scope: independent re-verification of the checked PFTL block, the
Ethereum contracts, and every `docs/evidence/pftl-uniswap-*` packet. Result:
the cited test suites pass when re-run (21 postfiat-bridge, 10 postfiat-node,
26 postfiat-rpc-sdk, 24 forge), the evidence digest chains recompute from the
files on disk, and the fork evidence is genuinely chain-collected with
execution signatures that are impractical to fabricate. The checked PFTL items
stand at their stated controlled sidecar scope. The directives below are
binding and ordered.

1. Fix the two release-blocking Ethereum contract defects before checking off
   any Ethereum task: fail-open challenged claims and ineffective
   source-receipt replay keying (see the BLOCKING FIX items in the Ethereum
   task block). Re-run the forge suite and the Gate 5 optimistic fork harness
   after the fixes; the prior Gate 5 optimistic evidence is frozen and does not
   count toward gate exit.
2. Do not commit the pending `wallet-proxy/server.js` `route_family` edit as
   written. Verified 2026-07-01: the `route_family` additions in
   `scripts/pftl-uniswap-gate3-fork-execute.py` and
   `scripts/pftl-uniswap-gate4-return-execute.py` are digest-neutral because
   the Rust node is the digest authority and skips the default value during
   canonical serialization; the proxy edit changes the proxy-computed route
   config digest from `23c4522e...` to `e3a33db4...` because the proxy hashes
   its own `JSON.stringify` output, which breaks the MVP4
   `expected_gate3_route_config_digest` binding. Land the proxy change only
   together with the digest-authority item in the Wallet block (consume
   node-produced digests, or byte-match the node canonical form by omitting
   default-valued optional fields from the digest preimage). Closed 2026-07-01:
   the proxy no longer computes the route config digest from local
   serialization; it consumes the node-produced digest and lands the
   `route_family` copy with the pinned node digest fixture.
3. Evidence packets are immutable. Commit `23862602` hand-edited raw MVP4 run
   packets to track a wallet-proxy display-copy change instead of re-running
   the harness. Do not repeat this. Either regenerate that packet with a fresh
   run or add a note to its README recording exactly which fields were edited
   and why.
4. Push the local commits. `main` is 37 commits ahead of `origin/main` on a
   single machine. Push before starting new work and at least daily thereafter.
5. After the Ethereum block, the next milestone is wiring the PFTL bridge into
   consensus (see the new unchecked PFTL tasks), not additional sidecar
   evidence breadth. Sidecar-scope work beyond the current checklist requires
   explicit owner approval.

### Gate 0: Spec and Legacy Boundary

Exit criteria:

- Legacy `a651` token and pool are labeled historical secondary liquidity.
- New asset strategy is chosen. Default: new `a666/wA666`; no implicit legacy
  migration.
- Burn/mint is accepted as the first bridge movement model.
- Packet schema and domain separators are specified.
- Refund design is selected. Default staged path: optimistic refund challenge.
- Official Uniswap address source is pinned in deploy tooling.
- Trust-class enum and config-digest binding are specified.
- Manager signs off that no live funds move at this stage.

Permitted claim:

```text
bridge-aware Uniswap redeployment design locked
```

### Gate 1: PFTL Packet Prototype on Local Devnet

Exit criteria:

- PFTL can burn/debit devnet `a666` and record `outstanding_bridge_claims`.
- PFTL emits a canonical bridge export receipt committed to a receipt root.
- Receipt roots bind ordered transition receipts with state-before and
  state-after hashes.
- Duplicate nonce is rejected.
- Expired packet remains counted until terminal refund.
- Refund path rejects before `refund_not_before_height`.
- State replay proves the supply invariant after every transition.

Tests:

- valid export;
- duplicate nonce;
- wrong route;
- stale NAV;
- cap exceeded;
- refund before window;
- expired-but-unrefunded remains encumbered;
- receipt mutation changes the receipt root;
- receipt reordering changes the receipt root.

Permitted claim:

```text
local PFTL bridge packet prototype
```

### Gate 2: Controlled Ethereum Packet Consumer

Exit criteria:

- Threshold-signed or mocked packet verifier accepts only exact packet fields.
- Ethereum packet fields preserve full PFTL 48-byte hashes and asset ids; no
  route config digest, asset id, reserve packet hash, or source packet hash is
  truncated to fit a `bytes32`.
- `VenueBridgeController` rejects mint-only and mint-and-swap packets unless the
  configured receipt verifier has accepted the exact PFTL source receipt root,
  source receipt hash, route config digest, and route trust class.
- `VenueBridgeController` rejects packets whose pricing NAV epoch or pricing
  reserve packet hash differs from the route-bound controller configuration.
- `WrappedVenueNAVCoin` mints only from `VenueBridgeController`.
- `VenueBridgeController` writes `consumed[packet_hash]` before mint/external
  action or is otherwise reentrancy safe.
- `VenueBridgeController` rejects reuse of the same source packet hash even if
  a relayer mutates destination-side swap terms.
- `VenueBridgeController` rejects reuse of the same accepted source receipt
  commitment even if a relayer mutates source packet hash, nonce, recipient, or
  swap terms.
- Wrong chain, bridge, asset, token, amount, recipient, and action payload are
  rejected.
- Local cap and epoch limit are enforced.
- `verifierTrustClass()` returns `CONTROLLED` for threshold/mock verifier.
- Public wallet route is disabled unless the route explicitly permits
  `CONTROLLED` and displays that label.

Tests:

- valid packet mints once;
- replay rejected;
- mutated replay with the same source packet hash rejected;
- mutated replay with the same accepted source receipt rejected;
- unaccepted source receipt rejected;
- pricing NAV epoch and pricing reserve packet mismatch rejected;
- verifier trust-class mismatch rejected;
- wrong chain rejected;
- wrong bridge rejected;
- modified recipient rejected;
- modified action payload rejected;
- expired destination deadline rejected;
- cap overflow rejected;
- reentrancy attempt rejected.

Permitted claim:

```text
controlled packet consumer, not trustless bridge
```

### Gate 3: Uniswap Fork Integration

Exit criteria:

- Fork uses real official Uniswap v4 PoolManager, PositionManager, Universal
  Router, Permit2, and StateView for target chain.
- New `wA666/USDC` pool initializes at current PFTL NAV.
- Seed `wA666` supply is generated by a primary subscription plus bridge export
  packet, not by out-of-band manual minting.
- Fork rehearsal executes an external USDC-to-`wA666` buy and a `wA666`-to-USDC
  sell through the selected router/path, with balances, pool state, and supply
  invariant checked after both trades.
- LP position recipient, custody policy, tick range, fee tier, and seed NAV
  epoch are published in the config digest.
- Mint-only packet settles to user.
- Mint-and-swap packet settles through adapter and pays output recipient.
- Composite `pfUSDC -> primary subscription -> bridge -> wA666` route displays
  primary NAV price separately from Uniswap AMM price when a swap leg is used.
- Swap failure reverts without consuming the packet, unless explicit claimable
  fallback is implemented.
- StateView reads show pool liquidity and price after seed.

Gate 3 rehearsal evidence must be collected from the fork chain, not hand-built.
The operator command is:

```bash
node scripts/pftl-uniswap-gate3-fork-rehearsal.mjs \
  --launch-config-file docs/plans/pftl-uniswap-launch-config.json \
  --launch-config-digest <96-hex from navcoin-bridge-launch-config-template> \
  --rpc-url "$ETHEREUM_RPC_URL" \
  --seed-export-packet-hash <96-hex> \
  --seed-receipt-root <96-hex> \
  --seed-mint-tx <0x...> \
  --seed-lp-tx <0x...> \
  --external-buy-tx <0x...> \
  --external-sell-tx <0x...> \
  --mint-only-packet-tx <0x...> \
  --mint-and-swap-packet-tx <0x...> \
  --user-buy-usdc-spent-atoms <u64> \
  --user-buy-wrapped-received-atoms <u64> \
  --user-sell-wrapped-spent-atoms <u64> \
  --user-sell-usdc-received-atoms <u64> \
  --canonical-supply-before-external-trades-atoms <u64> \
  --canonical-supply-after-external-trades-atoms <u64> \
  --output-file docs/plans/pftl-uniswap-fork-rehearsal-evidence.json
```

That script checks the RPC chain id, bytecode at every bound official Uniswap
and bridge contract address, mined transaction receipts, nonzero StateView
liquidity after seed/buy/sell, a non-mutating `getSlot0` read at the same
blocks, nonzero user buy/sell deltas, and unchanged canonical supply across the
external AMM trades. The `--launch-config-digest` value must come from the
node-generated launch config template report, because the Rust node verifier is
the canonical launch-config digest authority. The resulting evidence JSON must
then be recorded with:

```bash
cargo run -p postfiat-node -- navcoin-bridge-record-fork-rehearsal \
  --route-id <route-id> \
  --evidence-file docs/plans/pftl-uniswap-fork-rehearsal-evidence.json
```

Tests:

- exact-input happy path;
- min-output too high;
- expired Uniswap deadline;
- route hash mismatch;
- wrong output token;
- external buy from seeded pool;
- external sell back into seeded pool;
- seed packet missing or not tied to launch config digest;
- LP position recipient mismatch;
- Universal Router failure propagation;
- reentrancy and replay;
- post-swap balances and packet state.

Permitted claim:

```text
fork-tested bridge-aware Uniswap handoff
```

### Gate 4: Return Path

Exit criteria:

- Ethereum burn event binds chain id, bridge, token, amount, recipient, nonce,
  and destination PFTL asset.
- Wrapped venue token cannot be directly minted or bridge-burned by users,
  owners, routers, or relayers; only the bridge controller can mint from a
  consumed PFTL receipt or burn through a return request.
- Return burn id binds chain id, bridge controller, wrapped token, destination
  native asset id, Ethereum sender, PFTL recipient, amount, and nonce.
- PFTL consumes event once under the selected verifier model.
- Return path restores native `a666` or marks failure with a recoverable state.
- Round trip works twice without manual ledger edits.
- Return trust class matches outbound trust class or is more conservative.

Tests:

- burn event replay rejected;
- wrong token rejected;
- wrong PFTL recipient rejected;
- event below finality rejected;
- event on wrong chain rejected;
- route pause blocks release;
- two consecutive PFTL -> Ethereum -> PFTL round trips.

Permitted claim:

```text
controlled round-trip bridge prototype
```

### Gate 5: Optimistic or Trustless Verifier

Choose one path.

#### Optimistic Path Exit Criteria

- Anyone can post packets.
- Anyone can challenge invalid packets.
- Challenge evidence formats are objective and testable.
- Bonds and challenge windows are configured.
- Bond sizing and challenge-window inequalities are documented.
- Watcher runbook exists.
- Invalid packet tests prove challenges freeze or reject before settlement.
- Valid packet tests prove challenges cannot grief indefinitely under the rules.
- A challenged claim fails closed (added 2026-07-01): a claim challenged before
  finalization must never transition to accepted because a resolver missed the
  resolution deadline, and an unresolved challenge must never transfer the
  challenger's bond to the poster. Tests must cover valid challenge plus
  offline resolver: settlement stays blocked and the challenger's bond is not
  confiscated.
- Challenge resolution is either verified on-chain against objective evidence,
  or the resolver role is explicitly documented as arbitrated, supports key
  rotation, and every wallet-visible surface discloses the arbitration
  assumption. An owner-arbitrated resolver with no on-chain evidence check is
  not a sufficient basis for an unqualified `OPTIMISTIC` public label.
- A consumed PFTL receipt commitment (receipt root plus receipt hash) cannot
  back a second accepted claim or a second consume, even when the source packet
  hash, nonce, recipient, or swap terms are mutated.

Permitted claim:

```text
optimistic or trust-minimized bridge under watcher assumptions
```

#### Direct or Succinct Verifier Exit Criteria

- Ethereum verifies PFTL finality and receipt inclusion directly, or verifies a
  succinct proof of those facts.
- Validator set transition rules are verified.
- Receipt inclusion proof tests reject malformed proofs.
- Invalid finality certificate tests reject.
- Verifier-key governance and upgrade policy are documented.
- Gas/proof-size/prover-time measurements are published.

Permitted claim:

```text
trustless bridge relative to PFTL finality and verifier assumptions
```

### Gate 6: Public Redeploy

Exit criteria:

- Gate 0 through selected Gate 5 pass.
- Manager approves exact config digest, including trust class and route label.
- New token address, bridge controller, verifier, pool key, pool id, NAV proof
  source, route status, and trust label are published.
- Legacy pool UI is disabled for active wallet routing.
- Wallet and proxy refuse to display `trustless` unless PFTL route registry,
  Ethereum controller, and config digest all report `TRUSTLESS_FINALITY`.
- If `CONTROLLED` is ever enabled for public value, the route name must include
  `controlled`, the amount cap must be nonzero and published, and the user must
  approve a separate controlled-route warning.
- If `OPTIMISTIC` is enabled before direct or succinct finality verification,
  it is a capped public-beta route, not an unrestricted production route. The
  cap, watcher assumptions, challenge window, bond, and fail-closed conditions
  must be visible before signing.
- Uncapped or broadly marketed public routing requires `TRUSTLESS_FINALITY`.
- Wallet displays:
  - canonical NAV;
  - Uniswap market price;
  - proof freshness;
  - bridge verifier mode;
  - packet status;
  - refund deadline;
  - route trust label.
- Monitoring alerts cover stale proof, route pause, cap exhaustion, verifier
  issue, challenge event, replay rejection, and pool liquidity drop.

Permitted claim depends on Gate 5:

```text
controlled bridge-aware Uniswap route
optimistic bridge-aware Uniswap route
trustless bridge-aware Uniswap route
```

Do not collapse these labels.

## 10. Minimum Implementation Tasks

### PFTL

- [x] Add or extend NAVCoin bridge route registry. Checked by
  `PftlUniswapRouteConfig`, `PftlUniswapBridgeLedger`,
  `pftl_uniswap_bridge_ledger_from_config`, `navcoin-bridge-route-init`, and
  `navcoin_bridge_routes`: route configs now carry a machine-readable
  `route_family`, trust class, native/settlement/wrapped assets, controller,
  adapter, Ethereum chain, caps, and route digest into persisted route ledgers;
  public route status exposes `route_family` and `ledger_hash` to wallets and
  RPC clients. Verified with
  `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib`
  and
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results --lib`.
- [x] Add primary NAV subscription transition: collect counted settlement asset,
  price from finalized pre-inflow NAV, mint fractional native NAVCoin, update
  reserves and authorized supply atomically, and reject fixed-inventory-only
  routes unless they are explicitly labeled secondary inventory. Checked by
  `primary_subscription_quote`,
  `pftl_uniswap_apply_primary_subscription`,
  `pftl_uniswap_apply_primary_subscription_with_receipt`, and the persisted
  `route_family` guard: primary subscriptions require `primary_pftl_mint`; a
  route explicitly labeled `secondary_inventory` is rejected before supply,
  native balance, or reserve mutation. Verified with
  `cargo test -p postfiat-bridge pftl_uniswap_primary_subscription_mints_fractional_supply_from_pre_inflow_nav --lib`,
  `cargo test -p postfiat-bridge pftl_uniswap_primary_subscription_rejects_secondary_inventory_route --lib`,
  `cargo test -p postfiat-bridge primary_subscription_quote_uses_floor_and_reports_dust --lib`,
  and
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`.
- [x] Bind primary subscription requests and receipts to route id, source wallet,
  settlement asset id, subscription nonce, pricing NAV epoch, and reserve packet
  hash; reject subscription nonce replay. Checked by
  `crates/node/src/lib_test_parts/pftl_uniswap_bridge_rpc_tests.rs` and
  `crates/bridge/src/lib.rs`: the bridge validates route and settlement asset,
  persists nonce/source wallet, rejects duplicate nonces, requires current NAV
  epoch, records the pricing reserve packet hash in the receipt, and replay
  verifies the persisted sidecar receipts.
- [x] Validate bridge handoff packets against the launch config before relay with
  `navcoin-bridge-packet-preflight`: route digest, assets, pool id, USDC
  output, pricing NAV epoch, and pricing reserve packet hash must match
  exactly. Checked by `crates/node/src/lib_test_parts/pftl_uniswap_bridge_rpc_tests.rs`
  with `cargo test -p postfiat-node navcoin_bridge_packet_preflight --lib`,
  which rejects mismatched route digest, settlement asset, native NAV asset,
  wrapped venue token, pool id, USDC output, NAV epoch, and reserve packet hash
  before relay.
- [x] Add bridge export transaction for NAVCoin assets. Checked by
  `pftl_uniswap_export_debit` and
  `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_exports_refunds_and_preserves_invariant --lib`:
  export debits native/PFTL spendable supply, records export nonce and packet
  hash, rejects duplicate export nonce, and preserves the route invariant.
- [x] Add packet and outstanding-claim state. Checked by
  `cargo test -p postfiat-bridge pftl_uniswap_status_reports_expose_route_packet_claims_and_supply --lib`
  and
  `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib`:
  `SourceDebited` packets appear as outstanding bridge claims, packet/claim
  status rows expose the packet hash/status/class, and consume/refund terminal
  transitions clear outstanding claim atoms.
- [x] Add refund state and challenge/non-consumption proof path. Checked by
  `pftl_uniswap_refund_source_with_receipt`,
  `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_exports_refunds_and_preserves_invariant --lib`,
  and
  `cargo test -p postfiat-bridge pftl_uniswap_refund_receipt_commits_non_consumption_proof --lib`:
  source refunds require a bounded `non_consumption_proof_hash`, reject before
  `refund_not_before_height`, restore native/PFTL spendable supply, clear
  outstanding claim atoms, and replay from the refund receipt. 2026-07-01
  controlled-consensus slice: `postfiat-types` now derives the
  `non_consumption_proof_hash` as a canonical commitment over route id, packet
  hash, and `refund_not_before_height`; consensus and the sidecar reject
  arbitrary 96-hex placeholders. Refunds remain operator-attested until Gate 5
  supplies the actual non-consumption proof format.
- [x] Add Ethereum burn-event import path. Checked by
  `pftl_uniswap_record_return_burn_with_receipt`,
  `pftl_uniswap_import_return_with_receipt`, and
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`:
  the sidecar records a return burn, imports it once, restores PFTL native
  spendable balance, clears pending return-import claims, and persists replayed
  receipts.
- [x] Recompute Ethereum return burn id from chain id, bridge controller, wrapped
  token, destination native asset id, Ethereum sender, PFTL recipient, amount,
  return nonce, and burn height before accepting a return import. The canonical
  helper now lives in `postfiat-types` and is shared by the sidecar and
  consensus return-import applier, so the operator supplies fields but the
  burn id itself is derived locally. Checked by
  `pftl_uniswap_return_burn_id`,
  `postfiat_types::pftl_uniswap_return_burn_id_from_fields`,
  `cargo test -p postfiat-types pftl_uniswap_return_burn_id_binds_burn_height --lib`,
  `cargo test -p postfiat-bridge pftl_uniswap_return_burn_id_matches_solidity_abi_vector --lib`,
  `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib`,
  `cargo test -p postfiat-bridge pftl_uniswap_return_path_rejects_replay_wrong_token_and_low_finality --lib`,
  and
  `cargo test -p postfiat-node navcoin_bridge_return_burn_request_derives_canonical_burn_id --lib`:
  mismatched burn ids, burn-height changes, wrong bridge/token fields, replay,
  and insufficient finality are rejected before import.
- [x] Status payloads for route, packet, claims, and supply views must be
  deterministic, row-bounded, and include the bridge ledger hash used to build
  the report. Checked by `PFTL_UNISWAP_STATUS_MAX_ROWS`,
  `pftl_uniswap_bridge_routes_status`, `pftl_uniswap_bridge_packet_status`,
  `pftl_uniswap_bridge_claims_status`, and the bounded supply status fields
  `native_spendable_balance_count`, `native_spendable_balance_limit`,
  `native_spendable_balances_truncated`, and
  `native_spendable_balance_sum_atoms`. Verified with
  `cargo test -p postfiat-bridge pftl_uniswap_status_reports_expose_route_packet_claims_and_supply --lib`,
  `cargo test -p postfiat-bridge pftl_uniswap_supply_status_bounds_native_balance_rows --lib`,
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`,
  and the RPC SDK response validator tests
  `read_response_validation_accepts_supported_results` and
  `read_response_validation_rejects_bad_shapes_and_private_key_leaks`.
- [x] Supply status must expose deterministic per-wallet native NAV balances, and
  the route ledger must reject an export debit when the requested source wallet
  lacks enough native balance even if aggregate route supply is sufficient.
  Checked by `pftl_uniswap_bridge_supply_status` building bounded wallet rows
  from the ordered native balance map and including `ledger_hash`; by
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`,
  which asserts deterministic two-wallet status rows; and by
  `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_rejects_export_from_wrong_native_wallet --lib`,
  which rejects an export from a wallet with no native balance while aggregate
  PFTL supply remains sufficient.
- [x] RPC SDK request builders and response validators must recognize
  `navcoin_bridge_routes`, `navcoin_bridge_packet`, `navcoin_bridge_claims`, and
  `navcoin_bridge_supply_status`, and `navcoin_bridge_receipt_replay`, with
  bounded params and schema-specific response validation before node handlers
  are treated as launch-ready. Checked by
  `navcoin_bridge_routes_request`, `navcoin_bridge_packet_request`,
  `navcoin_bridge_claims_request`, `navcoin_bridge_supply_status_request`,
  `navcoin_bridge_receipt_replay_request`, the bounded `limit` validator for
  claims, and schema-specific response validators for route, packet, claims,
  supply, and receipt replay reports. Verified with
  `cargo test -p postfiat-rpc-sdk request_validation_accepts_supported_kinds --lib`,
  `cargo test -p postfiat-rpc-sdk request_validation_rejects_wrong_method_bad_params_and_key_leaks --lib`,
  `cargo test -p postfiat-rpc-sdk request_builder_serializes_params_and_round_trips --lib`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results --lib`,
  and
  `cargo test -p postfiat-rpc-sdk read_response_validation_rejects_bad_shapes_and_private_key_leaks --lib`.
- [x] Controlled operator sidecar commands may initialize route ledgers and apply
  primary subscription, export, consume, refund, return burn, and return import
  transitions before the consensus transaction path is wired. These commands
  must persist transition receipts, stay local/operator-only, and must not be
  presented as public trustless settlement. Checked by the CLI-only
  `navcoin-bridge-route-init`, `navcoin-bridge-primary-subscribe`,
  `navcoin-bridge-export-debit`, `navcoin-bridge-destination-consume`,
  `navcoin-bridge-refund-source`, `navcoin-bridge-record-return-burn`, and
  `navcoin-bridge-import-return` commands and by the shared
  `pftl_uniswap_apply_transition` sidecar path, which writes the route ledger and
  appends validated transition receipts after each mutation. Verified with
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`,
  which covers init, primary subscription, export, consume, return burn, return
  import, persisted receipts, and replay/tamper rejection; and
  `cargo test -p postfiat-node navcoin_bridge_refund_source_persists_receipt_and_replays --lib`,
  which covers the local refund command restoring source wallet balance,
  clearing outstanding claims, persisting the `source_refunded` receipt with the
  non-consumption proof hash, and verifying receipt replay. These are local
  operator sidecar commands only; the checked evidence does not label them as
  public or trustless settlement.
- [x] Controlled operator sidecar commands must also persist and verify bridge-aware
  Uniswap launch configs and Gate 3 fork rehearsal evidence against the route
  ledger. Launch configs must bind official Uniswap deployment snapshots, pool
  key/id, seed NAV math, tick range, fee tier, LP recipient/custody, controller,
  adapter, verifier, and trust class. Fork rehearsal evidence must bind external
  buy/sell transaction hashes, nonzero StateView liquidity, unchanged canonical
  supply across external AMM trades, canonical seed packet provenance, and
  min-output failure reverting without packet consume. Checked by
  `navcoin-bridge-launch-config-template`, `navcoin-bridge-launch-config-init`,
  `navcoin-bridge-record-fork-rehearsal`,
  `validate_pftl_uniswap_launch_config_against_ledger`, and
  `validate_pftl_uniswap_fork_rehearsal_evidence`; by persisted Gate 3 evidence
  under `docs/evidence/pftl-uniswap-gate3-2026-07-01/`, including
  `reports/08-launch-config-template.json`, `reports/09-launch-config-init.json`,
  `reports/11-record-fork-rehearsal.json`, and `reports/12-summary.json`; and by
  `cargo test -p postfiat-node navcoin_bridge_launch_config --lib`,
  `cargo test -p postfiat-node navcoin_bridge_records_launch_config_and_fork_rehearsal_evidence --lib`,
  and
  `cargo test -p postfiat-bridge pftl_uniswap_fork_rehearsal_evidence_requires_tradeability_and_supply_invariant --lib`.
  The bridge test rejects changed canonical supply, missing external trade
  deltas, manual seed minting, zero StateView liquidity, wrong pool binding, and
  min-output failure that consumes the packet.
- [x] Expose status RPC:
  - `navcoin_bridge_routes`;
  - `navcoin_bridge_packet`;
  - `navcoin_bridge_claims`;
  - `navcoin_bridge_supply_status`;
  - `navcoin_bridge_receipt_replay`;
  - `market_ops_status`;
  - `vault_bridge_status`.
  Checked by `crates/node/src/rpc_cli.rs` RPC handlers and public allow-list
  entries for all seven methods; `docs/runbooks/rpc-method-inventory.md`
  classifying the methods as `read_only_public`;
  `cargo test -p postfiat-node rpc_serve_request_tests::rpc_serve_allows_navswap_planner_read_methods`;
  and the focused status implementation tests
  `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib`,
  `cargo test -p postfiat-node navcoin_bridge_receipt_replay_accepts_clean_empty_route --lib`,
  `cargo test -p postfiat-node market_ops_replay_cli_tests::market_ops_status_cli_reports_required_public_fields`,
  and
  `cargo test -p postfiat-node market_ops_replay_cli_tests::vault_bridge_status_cli_reports_source_backed_receipt_capacity`.
- [x] Add deterministic replay tests and a public read-only replay report for bridge
  state, proving persisted transition receipts reproduce the route ledger or
  fail closed on tampering. Checked by
  `pftl_uniswap_verify_transition_receipt_replay`,
  `navcoin_bridge_receipt_replay`, and the read-only RPC method
  `navcoin_bridge_receipt_replay`. Verified with
  `cargo test -p postfiat-bridge pftl_uniswap_transition_receipts_commit_ordered_root_and_mutations --lib`,
  which rejects wrong final ledgers, reordered receipts, and empty replay input;
  `cargo test -p postfiat-node navcoin_bridge_operator_mutations_persist_ledger_and_receipts --lib`,
  which verifies persisted sidecar receipts then fails closed on a tampered
  receipt; and
  `cargo test -p postfiat-node navcoin_bridge_receipt_replay_accepts_clean_empty_route --lib`,
  which exposes a bounded public report with `empty_clean` status for an
  unmutated initial route.
- [ ] Implement PFTL-Uniswap receipt retention checkpointing. Decision recorded
  2026-07-01: fold old ordered receipts into a consensus checkpoint hash and
  keep `MAX_PFTL_UNISWAP_RECEIPTS` as the retained live-window cap. This is a
  required follow-up before public routing or before any route can approach the
  cap; no code implementation landed in the consensus-completion sprint.
- [x] Wire the bridge state machine into PFTL consensus. Design and implement
  consensus transaction types for primary subscription, export debit, source
  refund, and return import; move route ledgers and transition receipts from
  operator sidecar JSON files into consensus state; debit real settlement asset
  balances and credit real native NAVCoin balances on the PFTL ledger.
  2026-07-01 implementation slices: added consensus `LedgerState`
  route/receipt records, canonical Rust-side route/receipt hashes, state-root
  commitments for those records, and signed asset transaction operations for
  route init, primary subscription, export debit, source refund, destination
  consume, and return import. Primary subscription, export debit, source refund,
  destination consume, and return import now debit/credit real issued-asset
  balances and consensus route supply buckets in the execution ledger, and
  `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib`
  proves signed route init -> subscription -> export -> destination consume ->
  return import with real settlement/native balance movement, plus early refund
  rejection, refund after consume rejection, and consume after refund rejection.
  Destination consume is explicitly operator-attested under the controlled trust
  class until Gate 5 replaces it with verifier-backed semantics. This closes
  the consensus wiring milestone only; it does not close the Gate 5
  verifier/proof milestone or permit trustless/public route claims.
- [x] Enforce PFTL-Uniswap consensus route authorization and route-table caps.
  Checked by `apply_pftl_uniswap_route_init`,
  `ensure_pftl_uniswap_native_asset_policy`,
  `ensure_pftl_uniswap_route_capacity`, and
  `LedgerState::validate_asset_state`: route init now requires the native asset
  to be registered as a NAV asset and the route operator to be the native NAV
  issuer or reserve operator; source refunds and return imports require the same
  operator gate; consensus validation rejects PFTL-Uniswap routes whose native
  asset is not NAV-registered; and route creation is capped per native NAV
  issuer under the global route cap. The route-init authority decision is
  issuer scoped: issuers may delegate to the NAV reserve operator, matching the
  existing vault-bridge operator pattern. Primary subscription remains
  user-signed so the wallet owner authorizes the settlement-asset debit; price
  binding is handled by the separate freshness/pricing item below. Verified by
  `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib`,
  which rejects unauthorized route init before any mint path and rejects the
  ninth route for the same native NAV issuer without mutating state.
- [ ] Replace remaining operator-attested freshness, finality, and proof
  inputs. The price commitment, return burn id, refund commitment placeholder,
  and pause semantics now have consensus-side bindings. What remains is the
  selected Gate 5 source-chain proof semantics for destination consumption,
  refund non-consumption/current-height, and return-burn event inclusion and
  finality. The controlled sprint deliberately leaves those source-chain facts
  operator-attested and keeps the route in the `CONTROLLED` trust class.
  2026-07-01 price-binding slice: primary subscription now derives price from
  the route native asset's finalized `NavTrackedAsset` state instead of trusting
  caller pricing. Route init requires the submitted
  `latest_finalized_nav_epoch` to match ledger state and stores the ledger value;
  zero-epoch route init remains allowed, but subscription rejects until a reserve
  packet is finalized. Subscription rejects halted, unfinalized, missing-height,
  stale, wrong-epoch, wrong-reserve-packet-hash, and wrong-price inputs. The
  consensus freshness bound is `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS = 100`,
  chosen as a short controlled-devnet/testnet guardrail matching the existing
  expectation that NAV proofs refresh on the same cadence as profile epoch-gap
  checks; it closes unbounded stale pricing and can be tightened or
  profile-parameterized later. The settlement price is derived by applying the
  existing valuation-unit conversion (`required_vault_bridge_settlement_atoms`)
  to one native NAV atom using the native asset precision, finalized
  `nav_per_unit`, native valuation unit, settlement valuation unit, and
  settlement precision; the operation price field is retained only as an
  equality-checked wallet commitment. Dust no longer folds into reserves:
  `minted_nav_atoms = floor(settlement_value_atoms / derived_price)` and the
  subscriber is debited exactly `minted_nav_atoms * derived_price`, leaving any
  remainder in the subscriber wallet. Verified by
  `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib`.
  2026-07-01 return-import binding slice: the return `burn_event_hash` is now
  consensus-derived from the submitted burn fields using the sidecar's canonical
  derivation, including `burn_height`; mismatches reject before balance movement.
  Consensus also enforces finality-depth arithmetic, deduplication, and
  `ethereum_spendable` -> `pftl_spendable` supply movement. Residual trust class:
  the burn fields themselves, `burn_height`, and `finalized_height` remain
  operator-attested until Gate 5 provides source-derived event inclusion and
  finality. 2026-07-01 refund commitment slice: `non_consumption_proof_hash` is
  now a deterministic commitment placeholder, not arbitrary hex, and consensus
  rejects mismatches; it remains operator-attested and not a real proof until
  Gate 5. Pause semantics are explicit in consensus: pause blocks
  `primary_subscribe`, `export_debit`, and `destination_consume`, but permits
  `refund_source` and `return_import` because those transitions shrink exposure
  and unblock exits. Remaining before this checkbox can close: the selected
  Gate 5 verifier must replace operator attestations for destination consume,
  refund non-consumption/current-height, and return-burn field/height/finality
  claims with source-derived finality, inclusion, and non-consumption proof
  semantics.

### Ethereum

Status note (2026-07-01 CTO review): `src/PFTLUniswapHandoffController.sol`
already contains `WrappedVenueNAVCoin`, the controller
(`PFTLUniswapHandoffController` in the `VenueBridgeController` role),
`ControlledPFTLReceiptVerifier`, `OptimisticPFTLReceiptVerifier`,
`UniswapSettlementAdapter`, inline replay mappings, pause, and
`verifierTrustClass()`; the fresh forge suite now passes `25` tests. The review
confirmed the Gate 2 substance (48-byte PFTL fields preserved as `bytes` with
no `bytes32` truncation, receipt acceptance bound to the exact packet digest,
caps, and mutation/replay/reentrancy/deadline rejection) and found the defects
and gaps below. Check an item only with fresh checked-in evidence.

- [x] BLOCKING FIX: challenged optimistic claims must fail closed. Checked by
  `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol`, which now
  rejects unresolved challenged claims on finalization timeout and refunds the
  poster/challenger bonds without accepting the claim; by
  `testOptimisticUnresolvedChallengeFailsClosedAfterResolutionDeadline`; by
  `forge test --match-path test/PFTLUniswapHandoffController.t.sol -vv`,
  which reports `34` passing tests and `0` failures in
  `docs/evidence/pftl-uniswap-gate5-optimistic-2026-07-01/reports/pftl-uniswap-handoff-controller-forge-test.txt`;
  and by the regenerated Gate 5 fork/preflight evidence under
  `docs/evidence/pftl-uniswap-gate5-optimistic-fork-2026-07-01/` and
  `docs/evidence/pftl-uniswap-gate5-optimistic-2026-07-01/reports/gate5-optimistic-preflight.json`.
- [x] BLOCKING FIX: on-chain source-receipt replay binding. Checked by
  `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol`, which now
  keys `consumed_source_receipt` on source receipt root plus source receipt hash
  independent of packet digest; by `OptimisticPFTLReceiptVerifier`, which now
  stores a source receipt commitment at claim post and refuses a second active
  or accepted claim for the same source receipt; by
  `testSourceReceiptHashRejectsAcceptedMutatedReplay` and
  `testOptimisticSourceReceiptReuseCannotPostSecondAcceptedClaim`; by the same
  `34`-test forge report; and by regenerated Gate 5 fork/preflight evidence
  with optimistic launch binding digest
  `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`.
- [x] Resolver governance: the challenge resolver is no longer a single
  immutable address. Checked by
  `OptimisticPFTLReceiptVerifier.transferOwnership`,
  `OptimisticPFTLReceiptVerifier.setChallengeResolver`,
  `testOptimisticResolverGovernanceRotatesResolver`, and regenerated Gate 5
  binding fields `challenge_resolution_mode=owner_arbitrated`,
  `challenge_resolver`, and `resolver_owner`. The owner-arbitrated resolver
  assumption is documented in section 7.2, Gate 5/Gate 6 evidence, monitoring
  config/runbooks, and wallet-visible copy; wallet route/action validation now
  requires `challenge_resolution_mode` and rejects arbitrated optimistic labels
  that omit `ARBITRATED`. Direct on-chain objective challenge evidence remains
  a later trustless-verifier milestone, not a claim of this optimistic route.
- [x] Pause semantics: `setPaused` now has Foundry coverage and the route-level
  decision is documented. Checked by `testSetPausedIsOwnerOnlyAndEmits` and
  `testPauseBlocksInboundConsumesButAllowsReturnBurn`: owner-only pause blocks
  inbound `consumeMintOnly` and `consumeMintAndSwap`, while
  `burnForPftlReturn` remains available so `wA666` holders are not trapped
  during incidents. A separate return-pause would need its own explicit test
  and release-owner approval.
- [x] Replay-state persistence policy: replay mappings no longer live inside
  the controller instance. Checked by standalone `PacketReplayRegistry`, route
  config field `replay_registry`, controller replay getter delegation,
  controller-authorized `consumePacket` / `consumeReturnNonce`, and
  `testReplayRegistrySurvivesControllerRedeploy`, which proves consumed packet
  and return-nonce state survives a replacement controller using the same
  registry. Gate 5 binding and Gate 6 monitoring config now include the replay
  registry address.
- [x] Adapter and router trust: router return values no longer define
  settlement by themselves. Checked by `IERC20Balance` balance-delta
  verification around router execution in both `UniswapSettlementAdapter` and
  `PFTLUniswapHandoffController.consumeMintAndSwap`; by controller constructor
  verification that the router exposes the configured `uniswap_pool_id`; by
  `testRouterReturnValueCannotOverstateDirectSwapSettlement`,
  `testRouterReturnValueCannotOverstateAdapterSettlement`, and
  `testRouterPoolMismatchFailsConstructor`; by the `34`-test forge report; and
  by regenerated Gate 5 fork/preflight and Gate 6 monitoring evidence under
  optimistic launch binding digest
  `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`.
- [x] Route cap semantics: the route supply cap is a net-outstanding wrapped
  exposure cap, not a lifetime throughput cap. `total_minted_atoms` remains a
  lifetime audit counter, `total_return_burned_atoms` records return burns, and
  `outstanding_minted_atoms()` is used for the cap check before mint/settle.
  Checked by `testRouteSupplyCapTracksNetOutstandingAfterReturnBurn`, which
  fills the cap, rejects one more mint, burns wrapped supply for return, refills
  exactly the reopened capacity, and still rejects another over-cap mint; by
  the `34`-test forge report; and by regenerated Gate 5 fork/preflight and
  Gate 6 monitoring evidence under optimistic launch binding digest
  `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`.
- [x] Confirm `WrappedVenueNAVCoin` decisions: zero-value `transfer` and
  `transferFrom` now follow standard ERC-20 behavior and succeed without
  balance changes; controller `mint` and `burnFromBridge` remain nonzero-only;
  and `lockController()` remains a permanent non-repointable boundary for this
  token generation. Checked by
  `testWrappedTokenZeroTransfersAndControllerLockDecision`, by the `34`-test
  forge report, and by regenerated Gate 5 fork/preflight and Gate 6 monitoring
  evidence under optimistic launch binding digest
  `0338d28ed3d2521f9011a370c3af7d28acc91500633c080fa9d0a5764c75c02b3830e9c9a615be3287cc200bc220610e`.
- [x] Add forge-level fork tests against official Uniswap v4 deployments.
  Checked by `PFTLUniswapOfficialFork.t.sol`, which verifies bytecode at the
  bound mainnet PoolManager, PositionManager, Universal Router, Permit2, and
  StateView addresses; by
  `forge test --match-path test/PFTLUniswapOfficialFork.t.sol --fork-url https://ethereum-rpc.publicnode.com --fork-block-number 25440268 -vv`,
  which reports `1` passing fork test and `0` failures in
  `docs/evidence/pftl-uniswap-gate5-optimistic-2026-07-01/reports/pftl-uniswap-official-v4-fork-forge-test.txt`;
  and by the normal offline `forge test -vv`, which keeps the fork test as a
  no-op unless a fork is supplied.
- [x] Re-cite the already-implemented items with fresh evidence once the
  BLOCKING FIX items land: `WrappedVenueNAVCoin`, controller mint-only and
  mint-and-swap consume paths, `ControlledPFTLReceiptVerifier` (mock variant;
  no threshold-signed variant exists, which Gate 2 permits),
  `UniswapSettlementAdapter`, mutation/replay/cap/reentrancy/deadline tests,
  and `verifierTrustClass()` emission are covered by the regenerated
  `34`-test handoff report at
  `docs/evidence/pftl-uniswap-gate5-optimistic-2026-07-01/reports/pftl-uniswap-handoff-controller-forge-test.txt`,
  the regenerated Gate 5 optimistic fork packet under
  `docs/evidence/pftl-uniswap-gate5-optimistic-fork-2026-07-01/`, and the
  regenerated Gate 5/Gate 6 preflight reports.

### StakeHub

- Add a launch command that targets the new bridge-aware token and pool.
- Keep legacy a651 launch command separate and clearly labeled.
- Generate pool seed `wA666` only through primary subscription plus bridge
  export, and record the seed packet evidence.
- Produce config digest over:
  - token;
  - bridge controller;
  - settlement adapter;
  - verifier;
  - trust class;
  - pool key;
  - router/path;
  - NAV proof policy;
  - seed NAV epoch, seed USDC, seed wA666, tick range, fee tier, LP recipient,
    and LP custody policy;
  - caps;
  - deadlines;
  - trust label.
- Require fork rehearsal before live.
- Use `scripts/pftl-uniswap-gate3-fork-rehearsal.mjs` to collect chain-backed
  Gate 3 evidence and record it through
  `navcoin-bridge-record-fork-rehearsal`; do not paste synthetic transaction
  hashes or StateView values into the route ledger.
- Add publish-list fields for packet/verifier status, not only pool metadata.

### Wallet

- Add route families:
  - primary PFTL mint;
  - bridge mint-only;
  - bridge mint-and-swap;
  - composite primary mint to Ethereum venue token or Uniswap output;
  - return from Ethereum.
- The wallet must expose the end-user route outcomes as first-class states:
  - user now owns native `a666` on PFTL;
  - user now owns wrapped `wA666` on Ethereum;
  - user bought `wA666` from Uniswap secondary liquidity;
  - user sold `wA666` into Uniswap secondary liquidity;
  - packet is pending, refundable, refunded, consumed, or return-imported.
- The wallet must not show a pool trade as primary issuance, and must not show a
  primary issuance as a pool trade. It must display route caps and proof
  freshness before submit, not only after failure.
- Make the UX separate:
  - NAV freshness;
  - primary NAV subscription price;
  - settlement asset freshness;
  - bridge packet status;
  - Uniswap AMM price and slippage;
  - Uniswap quote expiry;
  - route trust label.
- Never route active users to legacy `a651/USDC` as the bridge handoff.
- Reject any route where the wallet-computed label, PFTL route registry, and
  Ethereum controller trust class do not match.
- The wallet proxy must not recompute canonical route or launch config digests
  from its own serialization (see the digest-authority rule in the header
  change-control block). 2026-07-01 finding: the proxy hashes
  `JSON.stringify` of its own route-config object, which matches the node
  digest only by accidental key-order agreement, and adding the
  default-valued `route_family` field diverges it from the node-canonical
  Gate 3 digest because the node skips default values during canonical
  serialization. Required fix: consume node-produced digests over RPC; interim
  fallback: byte-match the node canonical form, including omitting
  default-valued optional fields from the digest preimage, with a test pinning
  proxy digest equality against a node-generated vector. 2026-07-01 fix:
  `navswapBridgeConfig()` now requires a node-produced
  `NAVSWAP_ROUTE_CONFIG_DIGEST` / `NAVSWAP_NODE_ROUTE_CONFIG_DIGEST` and
  passes it through with `route_config_digest_authority: "node"`; it no longer
  hashes `JSON.stringify(routeConfig)`. The default-valued
  `route_family: "primary_pftl_mint"` field is live in proxy route copy without
  changing the digest. Node route status already exposes `route_config_digest`
  on `PftlUniswapRouteStatusRow`; launch digests are exposed by
  `navcoin_bridge_packet_preflight` when packet/launch binding is needed.
  Checked by `node wallet-proxy/test_navswap_adapter.js`, including the pinned
  fixture `wallet-proxy/fixtures/pftl-uniswap-node-route-digest.json`.

## 11. Test Matrix

| Test | Expected |
| --- | --- |
| Primary subscription priced from pre-inflow NAV | User fill uses the finalized NAV epoch before the user's payment is added to reserves |
| Primary subscription reserve/supply apply | Settlement reserves and native NAVCoin supply increase atomically after the fill |
| Primary subscription rounding/dust | Deterministic integer rounding matches route config; dust is refunded or explicitly recorded as a fee |
| Primary subscription replay | Reusing a subscription nonce is rejected |
| Primary subscription receipt binding | Receipt commits source wallet, settlement asset id, subscription nonce, pricing epoch, reserve packet hash, settlement amount, and minted NAV amount |
| Primary subscription vs Uniswap buy | Wallet labels primary NAV pricing separately from AMM execution price and slippage |
| `pfUSDC -> a666` user route | User spends pfUSDC and receives fractional native NAVCoin at finalized NAV, with accepted/refunded settlement shown |
| `pfUSDC -> wA666` user route | User spends pfUSDC, primary NAVCoin is minted and exported, Ethereum mints equal wrapped supply, or the packet enters an explicit recovery/refund state |
| Large primary subscription | A capped 100,000 USDC-equivalent test fills from primary issuance at pre-inflow NAV, not from operator inventory or Uniswap liquidity |
| Pool seed supply provenance | Seed `wA666` is minted only from a canonical primary subscription plus bridge export packet |
| External Uniswap buy | User USDC decreases, user `wA666` increases, pool price/liquidity update, and canonical supply is unchanged |
| External Uniswap sell | User `wA666` decreases, user USDC increases subject to slippage, pool price/liquidity update, and canonical supply is unchanged |
| Pool seed digest mismatch | Launch or fork rehearsal rejects mismatched seed NAV epoch, amount, tick range, fee tier, or LP recipient |
| Composite PFTL-to-Uniswap quote | Quote discloses primary NAV price, bridge packet fields, AMM price, slippage, and min output separately |
| Composite PFTL-to-Uniswap failure | Failed Ethereum swap does not consume the packet or strand the primary minted amount without refund path |
| Valid PFTL packet mints `wA666` | Success |
| Same packet submitted twice | Second rejected |
| Wrong destination chain | Rejected |
| Wrong destination bridge | Rejected |
| Wrong source asset | Rejected |
| Wrong destination token | Rejected |
| Modified recipient | Rejected |
| Modified Uniswap action | Rejected |
| Destination deadline expired | Rejected without consume |
| Swap min-out too high | Revert without consume |
| Swap route hash mismatch | Rejected |
| Source refund before refund window | Rejected |
| Source refund without non-consumption/challenge completion | Rejected |
| Source refund after verified non-consumption | Success |
| Destination consume after source refund | Rejected |
| Destination consume and refund race | At most one terminal state |
| Ethereum burn return replay | Second rejected |
| Ethereum return burn id mismatch | Rejected |
| Ethereum return burn wrong bridge, token, native asset, malformed sender, or malformed nonce | Rejected |
| Invalid finality certificate | Rejected |
| Invalid receipt inclusion proof | Rejected |
| Invalid validator set transition | Rejected |
| Cap overflow | Rejected |
| Route pause | New mint/settlement rejected |
| Reentrancy during mint-and-swap | Rejected or safely reverted |
| Stale NAV proof | New export/mint rejected |
| Legacy pool route selected | Wallet rejects |
| Controller trust class mismatches UI label | Wallet/proxy rejects |

Gate coverage:

| Gate | Required evidence |
| --- | --- |
| Gate 0 | Spec review, config digest schema, legacy route disabled. |
| Gate 1 | PFTL replay and invariant tests. |
| Gate 2 | Foundry packet mutation/replay/cap/reentrancy tests. |
| Gate 3 | Uniswap fork tests with official v4 addresses. |
| Gate 4 | Two clean round trips without manual state edits. |
| Gate 5 | Optimistic challenge tests or direct/succinct verifier test vectors. |
| Gate 6 | Deployment digest, wallet label checks, monitor alerts, published runbook. |

## 12. Migration Decision

The recommended default is a new bridge-aware token and pool.

Do not migrate legacy `a651` until a separate migration spec answers:

- conversion rate;
- snapshot method;
- LP treatment;
- whether old tokens are locked, burned, wrapped, or left as legacy;
- treatment of unclaimed legacy holders;
- how old supply, new supply, and outstanding bridge claims reconcile against
  NAV;
- public labels in the wallet and docs.

If no migration is chosen, label legacy `a651` as:

```text
legacy Ethereum a651, historical secondary venue, not the PFTL bridge token
```

## 13. Recommendation

Do not try to revive the legacy `a651/USDC` pool as the trustless route.

Build the new route around:

```text
new PFTL NAVCoin instance
new bridge-aware Ethereum wrapped token
new Uniswap pool
explicit packet state machine
strict supply and outstanding-claim accounting
return path before round-trip claims
trust labels that match the verifier actually deployed
```

That is the shortest path to a user flow where `pfUSDC -> a666 -> wA666 ->
Uniswap output` can become a real atomic settlement path instead of a UI
composition over unrelated ledgers.
