# NAVCoin Create and Swap: Product and Build Specification

**Status:** draft for build, 2026-09-30  

!!! note "2026-11-25: dated status"

    The status columns, gate states and build progress below are those of
    30 September to 5 October 2026 and were not updated in the review of
    25 November; gate states are the owner's. Since then BMNRC was created
    (2 October; one reading, epoch 1 at height 1099), B24 went onto the devnet
    with real money, and the validators moved to releases cut from `main`.
    The state on 25 November is on [NAVCoins](index.md#today) and
    [Network Status](../status/network-status.md).

**Owner:** Post Fiat  
**First instance:** the BMNR carry NAVCoin (`BMNRC`), which holds BMNRon long on Felix/Ethereum and shorts `xyz:BMNR` on Hyperliquid.

**Controlling documents:**

- [Canonical Primary-Market Accounting](primary-market-accounting.md) governs all reserve, issue, and exit arithmetic in this specification.
- [Proof-of-Reserve Primitives](reserve-primitives.md) governs proof profiles and packets.

This specification has two parts:

- **Part I, The product (§1–§6):** the product contract, value flows, route states, liquidity, and every screen an investor or creator sees.
- **Part II, The build (§7–§16):** the implementation map, executable invariants, release dependencies, phased work, acceptance evidence, capital plan, and risks.

The operator build is demonstrated when Post Fiat uses its own real money in the wallet to:

1. Create the BMNR carry NAVCoin.
2. Provide liquidity to its PFTL route and Uniswap pool.
3. Walk an investor from USDC in MetaMask into the NAVCoin, out to Ethereum, and back to USDC, both transparently and privately.

No outside money is permitted until every outside-money gate has passed. In particular, outside money requires consensus to reject trades whose NAV observation is older than 4 hours. The three-host pause mechanism used for the operator demo is not sufficient for outside money.

## Status at a glance

**Release stage:** operator prototype.

Only the operator creates NAVCoins in the first release. The public may buy, sell, bridge, and swap them only after every outside-money gate in this section passes and Post Fiat separately decides to open the product.

| Capability | Today (2026-09-30) | Delivered by this plan |
|---|---|---|
| NAVCoin issuance on PFTL | Live. A666 v2 was created by hand-built operations at blocks 337–348. | Wallet Create flow (§5.4) |
| NAV proof for a stock-plus-perp mandate | Prototype. The BMNR dry run passes (§8.3). | Governed, consensus-enforced proof (B1.1–B1.5) |
| Buy and sell at NAV on PFTL | Live for A666. One hard-coded route. | Every route (B5.3) |
| Ethereum wrapped asset and Uniswap pool | Live for A666, which trades 25.8% below NAV | Per-NAVCoin stack with NAV band and keeper (Phase 4) |
| USDC ↔ pfUSDC bridge | Deployed. The live route reports `live_value_enabled=false`. | Ready route (B0.4) |
| Shielded buy | Proven once (block 420) | Wallet screen with a supervised prover (B5.4) |
| Price history, TVL, and location | Not built | Indexer and Asset-detail panels (B5.2, B5.5) |

### Readiness levels and release boundaries

There are two readiness levels:

1. **Operator demo:** Post Fiat's own money only. It requires the operator-demo scope of Phases 0–5. Deferred outside-money protocol items B2.5 and B2.6, and later-release item B3.4, do not block the operator demo.
2. **Outside money:** requires the operator demo plus every gate below.

B0.0 blocks both readiness levels. No other item deploys or moves money until B0.0 containment is live. Read-only investigation and code preparation for later items may run in parallel. Until B0.0 closes, the wallet is not shown to anyone outside Post Fiat. B2.6 is the fail-closed freshness boundary: before it passes, wall-clock freshness depends on operator transactions and cannot be guaranteed by consensus.

| Gate | Build item | Evidence that closes it | Status |
|---|---|---|---|
| Credential-exposure incident SEC-20260930-01 closed | B0.0 | Redacted public closure note in `PF/docs/security/` with the external verification; separate regression evidence that the wallet still works from its public URL | **closed 2026-10-01** |
| 6/6 validators on a build in the canonical repository | B0.2 | Six `status` reads: same height, same state root, a `PF` commit | open |
| pfUSDC route ready | B0.4 | 1 USDC deposit claimed end to end; readiness `ready:true` | operator scope passed 2026-09-30; public enablement open |
| pfUSDC supply reconciled | B0.7 | Reconciliation table balances to the atom | **closed 2026-09-30** |
| Signer pinned, spend bounded | B0.8 | Named commit loaded; per-job gas budget refuses an over-budget job | open |
| Trusted wallet TLS | B0.9 | Stock browser and MetaMask open the wallet with no certificate warning | open |
| Consensus-enforced NAV and proof binding | B1.3 | Replayed proof rejected on ce22; packet with a wrong `N` rejected | open |
| Route principal counted in `V` | B1.5 | Fork epoch after a buy and a sell shows `V` = perimeter + overlay | open |
| Calendar pauses and watchdog | B2.4 | Quiet-chain test (§8.5) passes; one calendar pause/resume cycle | open |
| Dedicated reserve perimeter | B3.3 | Three perimeter addresses published in the mandate; the proof covers exactly them | open |
| Independent second attestor | B1.2 | Price and hedge attestations require two keys, one held outside Post Fiat | open |
| Consensus-enforced wall-clock freshness | B2.6 | A buy against an observation older than 4 hours is rejected by consensus on ce22 | open |
| Finalize without the issuer's tap | B2.5 | An unchallenged epoch finalizes with the reserve-operator key alone on ce22 | open |
| Role separation | §10 | Every role in the role table is held by a distinct key, and outside-money roles meet the table's separation column | open |
| Keeper and band live | B4.2, B4.3 | Pool within the band for 7 consecutive days of the operator demo | open |
| Investor round trip, transparent and shielded | B6.2, B6.3 | All hashes and heights recorded; supply conservation (I2) holds, including shielded supply | open |
| Stranded-funds disclosure | B0.6 | Note published | open |
| Independent security audit | B7.1 | Audit report on the Ethereum contracts, `sp1-nav-reserve-v1`, and the Orchard circuit, with every high-severity finding closed | open |
| Legal review of issuance and holder eligibility | B7.2 | Written opinion on file for each launch jurisdiction; eligibility rules enforced by Create and Access | open |

**Build progress (2026-10-01 03:35 UTC).**

- **Done:**
  - B0.0, B0.4 (operator scope), B0.5 and B0.7;
  - B3.1–B3.3, including a live $20 pair open and close on the dedicated perimeter;
  - B4.1–B4.3, on mainnet forks.
- **Accepted for the operator demo on fork evidence:** Phase 5.
- **In progress:** B0.1, and Phase 6 stage A, the BMNRC perimeter opening.
- **RED:** B2.6, in the unreleased validator candidate. Production is unaffected.

The authoritative per-item record is the [2026-10-01 handoff](../handoffs/2026-10-01___nazgul__navcoin_create_and_swap_build_phase0_and_bmnrc_opening.md).

Passing every gate makes BMNRC eligible for outside money. Opening it to the public remains a separate, explicit decision by Post Fiat.

Until then, only the operator's own money touches the product.

### Terms

| Term | Meaning |
|---|---|
| **ce22** | The live six-validator PFTL network (`postfiat-wan-devnet-2`). |
| **Block N**, **hN** | PFTL block height N. PFTL blocks carry no timestamps (§3). |
| **Epoch** | One finalized NAV observation for a NAVCoin. |
| **Build agent** | The Codex agent that executes the build items; Post Fiat reviews its evidence at each gate. |

---

# Part I: The product

## 1. Product contract

### 1.1 What is specified

A NAVCoin is a native PFTL asset that trades to its net asset value (NAV). Its accounting relationship is:

```text
N = V / S
V = verified net assets attributable to holders
S = valid global supply across PFTL, Ethereum, and shielded notes
N = finalized NAV per unit
```

The product makes only the following conditional claims:

1. Consensus caps supply against a proof-checked reserve.
2. Under `sp1-nav-reserve-v1`, consensus computes `N` as `floor(V / S)`.
3. While the primary route is Open, every route trade is priced from the finalized `N`.
4. Outside money is not permitted until consensus also rejects a trade whose observation is older than 4 hours.
5. A sell at NAV is limited by available unencumbered route principal and the other limits in §2. It is not an unconditional cash-exit promise.

Three mechanisms connect trading to `N`:

| Mechanism | Required behavior | Enforcement |
|---|---|---|
| **Proof-checked reserve** | Each epoch, the reserve operator submits a packet valuing the reserve. The epoch cannot finalize unless an SP1 Groth16 proof over the packet verifies. | PFTL consensus inside `nav_reserve_submit` |
| **Consensus-computed NAV** | Under the `sp1-nav-reserve-v1` verifier used for BMNRC, consensus rejects an epoch whose `N` is not exactly `floor(V / S)`. Minting cannot exceed finalized circulating supply. | PFTL execution: `nav_reserve_nav_floor_mismatch` and the mint cap |
| **Primary route at NAV** | While the route is Open, anyone permitted to hold may buy new units at `N × 1.005` or sell units at `N × 0.9995`. Sells cannot exceed unencumbered reserve principal. Buying below NAV elsewhere and selling at NAV can be profitable, creating an arbitrage path toward `N`. | Consensus prices each route trade from the finalized epoch |

### 1.2 Trust summary

“Proof-checked” means that quantities and arithmetic are proven while prices are attested. It does not mean every value-moving input is independently proven.

| Input | Assurance | First release |
|---|---|---|
| Stock and cash quantities | **Cryptographic:** EIP-1186 storage proofs against an Ethereum block, checked in the SP1 program and verified by consensus | Yes |
| Route principal | **Consensus state:** read directly by PFTL through the route-principal overlay | Yes |
| Stock price and hedge-account value | **Attested:** signed observations of the Felix and Hyperliquid APIs | One Post Fiat key, which is also the issuer. A second, independent attestor is an outside-money gate. |
| Ethereum → PFTL return bridge | **Validator checkpoint:** 5 of 6 PFTL validators | Yes |
| Epoch finalization | **Issuer action:** the creator's wallet taps finalize | Until B2.5 |
| Observation age in wall-clock time | **Operator transaction:** three hosts hold a pre-signed pause | Operator demo only. Consensus enforcement is required before outside money under B2.6. |

The Proof panel displays these assurance levels for every NAVCoin (§5.2).

### 1.3 What is not promised

A NAVCoin is not a stablecoin and does not promise a peg. If the reserve loses value, `N` falls.

Buy and sell access at NAV is conditional on route state, supply policy, available principal, and per-order limits. Secondary venues may trade above or below `N`, including while the route is Paused, Stale, or Halted.

The protocol guarantee is narrower and auditable: supply is capped by a proof-checked reserve, and whenever the route is Open, consensus prices every route trade from that reserve.

### 1.4 BMNR carry NAVCoin

BMNRon is an Ondo total-return asset that tracks BitMine Immersion Technologies stock at 1.0000 shares per unit. The reserve holds BMNRon and shorts the same number of shares on Hyperliquid's `xyz:BMNR` perpetual. Price moves on the two legs cancel. What remains is the funding that perpetual longs pay shorts.

`xyz:BMNR` funding, from 621 hourly observations since 2026-09-04 read on 2026-09-30:

| Measure | Value |
|---|---|
| 30-day average | 40.8% APR |
| 7-day average | 31.5% APR |
| Hours with positive funding | 84.7% |
| Worst rolling week | +17.3% APR |
| Best rolling week | +60.5% APR |

Each epoch proof-checks the reserve supporting the asset.

## 2. Accounting and value flow

All arithmetic follows [Canonical Primary-Market Accounting](primary-market-accounting.md).

### 2.1 Reserve composition

`V` has two parts:

1. **Off-chain perimeter:** the NAVCoin's own Felix account holding the stock, its own Hyperliquid account holding the hedge, and its own cash wallet holding USDC. The epoch proof values this perimeter.
2. **On-chain principal:** pfUSDC paid into the route by buyers, less amounts withdrawn by sellers. The `sp1-nav-reserve-v1` verifier supports this through its route-principal overlay: consensus adds the route principal, under its own source root, to the proof value and checks the composite root.

Therefore:

```text
V = proven perimeter + route-principal overlay
```

### 2.2 Opening and trading transitions

**Opening.** The creator supplies the perimeter with stock, hedge margin, and cash. The first proof values it. Opening supply is minted to the creator at $1.00 per unit through `nav_mint_at_nav`. A666 v2 followed this path at h342–344.

**Buy at NAV.** For `x` units, the buyer pays `ceil(x × N) × 1.005` in pfUSDC. One atomic transition performs all three effects:

- reserve principal rises by `ceil(x × N)`;
- the 0.5% fee moves to non-NAV fee custody;
- supply rises by `x`.

`N` is unchanged.

**Sell at NAV.** For `x` units:

- reserve principal falls by `floor(x × N)`;
- the seller receives that amount × 0.9995;
- supply falls by `x`;
- `N` is unchanged.

Available exit capacity is:

```text
min(policy exit capacity, valid supply, unencumbered NAV reserve principal, per-order limit)
```

The quote displays that amount before the seller signs.

### 2.3 Liquid tranche and exit limits

Route principal is the NAVCoin's buy-and-sell liquidity at NAV. It is part of `V`, not a separate exit bucket.

**First release, v1.** Principal remains on PFTL as pfUSDC. Buys increase the cash share of `V`, reducing the stock-plus-hedge share. Route issue capacity is therefore capped at the opening `V` per epoch, and Explore displays the live cash share.

In v1, a holder can sell at NAV only up to available principal. If principal is exhausted, future buys can add principal, but the product does not guarantee immediate replenishment from the perimeter.

**Later release, v2, B3.4.** Two reserve-operator operations move value between route principal and the perimeter cash wallet through the pfUSDC bridge:

- `route_principal_release`
- `route_principal_restore`

Each operation leaves `V` unchanged and is bounded by the mandate cash targets. The executor then enforces:

| Parameter | BMNRC value | Executor action |
|---|---|---|
| `liquid_tranche_target` | 30% of `V`, held as on-chain principal | Deploys principal above the target into stock plus hedge |
| `liquid_tranche_floor` | 15% of `V` | Unwinds stock plus hedge back into principal when principal falls below the floor |

The Exit-limited state in §3 therefore has different recovery behavior by release:

- **v1:** sells remain limited to current principal; later buys may add principal.
- **v2 after B3.4:** the executor can unwind stock plus hedge to replenish principal within the mandate limits.

### 2.4 Excluded value and carry

Issue and exit fees remain outside `V` in non-NAV fee custody.

The arbitrage keeper inventory in §6.2 is an **alignment reserve**. Canonical accounting forbids counting it as backing or exit principal.

Between epochs, the hedge account accrues funding and pays trading costs. Each proof captures the net result in `V` and therefore in `N`. When funding is positive and exceeds costs, `N` rises. When funding reverses, `N` falls. Section 16 specifies the stress case and response.

## 3. Route states and freshness contract

The primary route is always in exactly one displayed state:

| State | Condition | Buy | Sell |
|---|---|---|---|
| **Open** | Latest epoch finalized ≤100 blocks ago; stock venue open; route not paused | Yes, up to capacity | Yes, up to available principal |
| **Exit-limited** | Open, but requested exit > available principal | Yes | Up to available principal. In v1, later buys may add principal; after B3.4, the executor can replenish the tranche under §2.3. |
| **Paused** | The reserve operator or watchdog paused the route through `pftl_uniswap_route_pause` because the stock venue is closed, the latest observation is older than 4 hours, or the epoch pipeline failed | No | No |
| **Stale** | Latest epoch older than 100 blocks. At the observed average of about 11 blocks per day, this is roughly nine days, and far longer on quiet days. It is only a backstop. | No | No |
| **Halted** | Issuer `nav_halt`, hedge venue halted, or risk limit breached | No | No |

Secondary venues continue trading in every state. Shielded transfers and the Uniswap pool (§6.2) are unaffected by route state. Asset detail displays the pool premium or discount against the last finalized `N`.

### 3.1 Block time limitation

PFTL block headers contain:

- `batch_id`
- `block_hash`
- `height`
- `parent_hash`
- `proposer`
- `state_root`
- `view`
- the certificate
- receipt roots

They contain no timestamp.

Blocks are produced only when a batch is certified, so height measures activity rather than elapsed time. Dated records place:

- block 342 on 2026-07-27;
- block 994 on 2026-09-06;
- block 1034 around 2026-09-25;
- block 1064 on 2026-09-30.

That is 722 blocks in 65 days, an average of about 11 per day. The rate is uneven: it follows activity, so quiet periods produce far fewer blocks. A 100-block freshness window therefore lasts at least a week on average and has no upper bound in wall-clock time. Block-based staleness alone does not close the route on a quiet weekend.

The operator demo uses the explicit pause mechanism in §8.5. Outside money requires B2.6, which makes wall-clock freshness a consensus rule.

### 3.2 BMNRC timing parameters

| Parameter | BMNRC value | Source |
|---|---|---|
| Epoch cadence | At each session open, then every 2 hours while the stock trades, and whenever the perimeter moves more than 0.5% of `V` | Epoch service (§8.4) |
| Maximum observation age | 4 hours of wall-clock time. Before B2.6, the chain cannot measure it and an independent watchdog enforces it. | Watchdog |
| Proof time | About 18 minutes on the GPU prover | A666 v2 opening proof |
| Calendar | One source: Felix `GET /v1/market/status/{sym}`. It covers every session, DST, NYSE holidays, per-stock weekend availability, and ex-dividend pauses. The route is paused whenever the stock status remains non-open for longer than 15 minutes and resumes at the next epoch after reopening. Resume requires a fresh `N` and live value enabled. | `apply_pftl_uniswap_route_pause` (`L1/crates/execution/src/nav_vault_asset_execution.rs:5626`); pause authority is the issuer or reserve operator (`:6415`) |
| Attestor | One ed25519 key operated by Post Fiat and published in the profile. The issuer and attestor are the same party in the first release, and the Proof panel states that fact. | B1.2 |

## 4. Scope

### 4.1 In scope

- Mandate family `long_stock_short_perp`: one Felix stock asset held long and hedged by the matching Hyperliquid HIP-3 perpetual held short.
- Venues: Felix for Ondo Global Markets assets on Ethereum mainnet, and Hyperliquid HIP-3 dexes `xyz`, `para`, and `io`.
- Chains: PFTL through the ce22 fleet and Ethereum mainnet.
- The Arbitrum pfUSDC route is deprecated and is not used.
- Surfaces:
  - the public self-custody wallet, `wallet-web`, and its proxy;
  - the StakeHub prover and agent;
  - the Ethereum contracts.

### 4.2 Out of scope

- Multi-stock baskets.
- Short legs using exchange-traded pooled products.
- Venues other than Felix and Hyperliquid.
- Third-party creators operating their own Felix and Hyperliquid accounts. The first release executes venue legs through the issuer's mandate executor (§10), and creation is restricted to the operator.
- A new consensus AMM on PFTL. The existing primary route already provides consensus-priced, two-sided access at NAV (§6.1).

## 5. Wallet screens

The NAVCoin product uses the existing public wallet. The following properties must remain unchanged:

- keys are generated and held in the browser through WASM ML-DSA;
- MetaMask signs Ethereum transactions;
- the wallet opens remotely without a tunnel or token.

The bottom navigation already has seven entries in `NAV_ITEMS` (`App.jsx:54-62`). New screens are therefore sub-tabs:

| Tab | Sub-tabs |
|---|---|
| **Assets** | `Explore` for all NAVCoins, extended `Asset detail`, and creator-only `Create` |
| **Trade** | `Buy / Sell at NAV` for every route, `Uniswap`, and `Private swap` |
| **Bridge** | USDC ↔ pfUSDC unchanged, plus NAVCoin ↔ Ethereum export and return |

### 5.1 Explore

Explore lists every live NAVCoin route. Each row shows:

- name and mandate family;
- `N` and its 24-hour change;
- total value;
- route state from §3;
- Uniswap premium or discount;
- proof age.

### 5.2 Asset detail

| Investor question | Required panel content |
|---|---|
| **What is the price?** | `N`, buy price `N × 1.005`, sell price `N × 0.9995`, available exit amount, route state, Uniswap spot, and premium or discount |
| **What is the price history?** | `N` and Uniswap spot over 24 hours, 7 days, 30 days, and all time through an inline SVG chart |
| **What is the TVL?** | `V` split into stock, hedge, cash, and on-chain principal; Uniswap pool depth in USD shown separately and labelled “not backing” |
| **Where is my asset?** | Global supply split among PFTL native, Ethereum wrapped, and in-flight; the investor's split among the PFTL wallet, MetaMask, and shielded notes |

Two additional panels support due diligence:

- **Mandate:** strategy; both legs with venue and instrument; hedge ratio; cash targets; risk limits; mandate hash and verify link.
- **Proof:** epoch; verifier kind; verification height; proof age; reserve breakdown; and the assurance level of each input, including whether it is proven or attested and by whom.

### 5.3 Trading screens

- **Buy / Sell at NAV:** obtains a quote from the route (§6.1) and displays the fee, available exit amount, and route state before the investor signs once in the wallet.
- **Uniswap:** swaps the wrapped NAVCoin against USDC through MetaMask. If the pool differs from `N` by more than the route's 55 bps round-trip spread while the route is Open, the screen displays the buy-or-sell-at-NAV alternative.
- **Private swap:** buys the NAVCoin from a shielded pfUSDC note at `N × 1.005`. The amount, owner, and price are hidden on-chain as specified in §12.3.

### 5.4 Create

Create is a seven-step creator flow. Each step states what it does, what it costs, and what it produced.

| Step | Screen | Required behavior | Signer |
|---|---|---|---|
| 1 | **Pick a stock** | Display the pair table from §13.1: Felix price, perp mark, basis, current funding, 7-day funding, 30-day funding, worst week, open interest, dex, isolated-only flag, weekend availability, round-trip cost, and break-even days. Disable pairs with basis over 15 bps or hedge depth below ticket size. | none |
| 2 | **Size** | From notional, compute token quantity, short size `token_qty × shares_per_token`, margin at chosen leverage, liquidation distance, entry cost, liquid-tranche amount, and expected carry. | none |
| 3 | **Name and mandate** | Collect code, display name of ≤64 bytes, public or private visibility, cash targets, and risk limits. Display the generated mandate and hash. | none |
| 4 | **Open the reserve** | Buy the stock on Felix and open the matching Hyperliquid short in dedicated NAVCoin accounts. Display fills, fees, and residual hedge. | Mandate executor |
| 5 | **Prove and mint** | Prove epoch 1. Create the asset, register the profile and NAV asset, submit and finalize epoch 1, and mint opening supply at $1.00. | Prover; creator wallet as issuer |
| 6 | **Provide liquidity** | The creator buys the liquid tranche through the route with pfUSDC, providing buy-and-sell liquidity at NAV. The creator then exports units to Ethereum and seeds a Uniswap band around `N`. | Creator wallet; MetaMask |
| 7 | **Launch** | Display asset id, route id, wrapped asset, pool id, epoch-1 proof, and links. Add the NAVCoin to Explore. | none |

Private NAVCoins also display an **Access** panel. Holders request access, and the creator approves each holder through `trust_set authorized=true`. This operation authorized A666's first holder at h341.

The holder list is enforced on PFTL only. The Ethereum wrapper and its Uniswap pool are freely transferable. Step 6 therefore asks private creators to choose one of two options:

- **PFTL only:** no export and no pool, so the holder list covers every unit.
- **PFTL and Ethereum:** the holder list covers PFTL balances only. A666 uses this option.

## 6. Liquidity

### 6.1 PFTL route

PFTL has no AMM object. The primary route provides liquidity at NAV, and consensus prices each trade from the finalized epoch.

The route does not use a fixed pot:

- **Buys** bring principal and are limited by policy. A666 allows 2,000,000 units per epoch.
- **Sells** consume the liquid tranche described in §2.

In the Create flow, providing PFTL liquidity means the creator buys the first liquid tranche through the route.

### 6.2 Uniswap pool

Each NAVCoin has a wrapped ERC-20 on Ethereum and a Uniswap v4 pool paired with USDC:

- the route binds the pool to the NAVCoin;
- the bridge conserves supply across chains.

The pool is secondary-market access, not backing. Pool trades change neither `V` nor `S`.

#### Existing A666 result

On 2026-09-30, A666 traded at 0.768 USDC against an `N` of 1.0352, a 25.8% discount. Three conditions allowed the discount to persist:

1. The pool's $3,000 per side was spread over the full price range, so small sells moved the price substantially.
2. No keeper was running.
3. The export and return relays required for movement between the pool and route reported unavailable.

#### BMNRC alignment mechanisms

**1. Concentrated band liquidity**

The seed is placed from `N × 0.99` to `N × 1.01`.

- With $400 of USDC and $400 of NAVCoin inside the band, a $100 trade moves price about 0.25%.
- Only net flow above about $400 can push price outside the band.
- The epoch service re-centers the band after each epoch.

**2. Keeper with pre-positioned inventory**

The keeper holds an alignment reserve on both chains:

- wrapped NAVCoin and USDC on Ethereum;
- pfUSDC and NAVCoin on PFTL.

When the pool leaves the band, the keeper trades from Ethereum inventory and later restores inventory through the route and bridge. The bridge's 18-minute export proof delays inventory refill rather than the initial correction.

- Above the band: sell wrapped units into the pool, then mint replacements on PFTL at `N × 1.005`.
- Below the band: buy units from the pool, then sell units at `N × 0.9995` on PFTL.

The keeper profits only beyond the 55 bps round-trip spread.

BMNRC's first keeper holds a $300 alignment reserve. It can absorb about $300 of one-directional flow before waiting for a refill. Additional flow can move the pool outside the band, which Asset detail displays.

#### Paused periods

While the route is Paused, the keeper may trade from existing inventory but cannot refill through the route. Pool defense is then limited to about $300 of keeper inventory plus band depth.

Net one-way flow beyond that amount can move price outside the band and leave it there until the route reopens. Asset detail displays the deviation throughout.

---

# Part II: The build

## 7. Code and evidence map

Code citations use `symbol (path:line)`. Line numbers are pinned to the commits below and were verified on 2026-09-30. They must be re-pinned whenever a commit changes.

| Root | Repository | Pinned commit | Contents |
|---|---|---|---|
| `PF` | `~/repos/postfiatl1v2` on canonical `main` | `3d0e5c0129` | Docs, deployments, scripts |
| `L1` | `~/repos/postfiatl1v2-fastpay-release-20260925` | `943c4ca7b6` | Execution and RPC code; nearest local source to the live fleet build |
| `FL` | `~/repos/a666-eth-fast-lane-combined-20260724` | `e8fb35ffc2` | Served wallet, proxy, and Ethereum contracts |
| `SH` | `~/repos/StakeHub` | `fe21283df4` | Prover, agent, tokenized-equity adapter, and `pft` CLI |
| `WA` | `~/repos/work_archive` | not a git repository | Felix execution scripts and capital records |

Two repository conditions block implementation until Phase 0:

1. The live fleet runs `c93b2137`, which is not present in any local repository. `L1` is nearest-source only.
2. The served wallet is in `FL`, while `PF/wallet-web` on `main` is four wallet commits behind.

B0.1 and B0.2 resolve these conditions.

## 8. Creation engineering

### 8.1 PFTL issuance sequence

Creating a NAVCoin requires no chain governance. A666 v2 was created using only issuer and reserve-operator keys in `PF/deployments/a666-mainnet-20260727/*.ops.json` at h337–348.

The wallet must reproduce this sequence:

| # | Operation | Type in `L1/crates/types/src/transactions_mempool_receipts.rs` | Execution in `L1/crates/execution/src/` | Signer |
|---|---|---|---|---|
| 1 | `asset_create` | `AssetCreateOperation` (:257) | `nft_escrow_asset_execution.rs:817-848` | issuer |
| 2 | `nav_profile_register` | `NavProfileRegisterOperation` (:1093) | `nft_escrow_asset_execution.rs:2354-2372` | issuer |
| 3 | `nav_asset_register` with unit `USD_1E8` | (:467) | `nft_escrow_asset_execution.rs:1070-1160` | issuer |
| 4 | `trust_set` with `authorized=true` for private NAVCoins | (:318) | `nft_escrow_asset_execution.rs:849-975` | holder; issuer |
| 5 | `nav_reserve_submit` | (:512) | `nft_escrow_asset_execution.rs:1161-1438` | reserve operator |
| 6 | `nav_epoch_finalize` | (:656) | `nft_escrow_asset_execution.rs:1551-1727` | issuer |
| 7 | `nav_mint_at_nav` | (:917) | `nft_escrow_asset_execution.rs:2126-2228` | issuer |
| 8 | `pftl_uniswap_route_init_v2` → `…_route_epoch_advance` | enum (:3517, :3529) | `nav_vault_asset_execution.rs:4151, 5527` | issuer |

Signing path:

1. `TxBuilder.signAssetTransaction` at `FL/wallet-web/src/lib/tx-builder.js:660`
2. fee quote
3. `wasm.wallet_sign_asset_transaction_fields` at `:690`
4. `mempool_submit_signed_asset_transaction_finality`

`buildTrustSetOperation` at `tx-builder.js:136` is the implementation pattern for B2.1.

#### Large proofs

SP1 proof bytes exceed the RPC's 4,096-byte parameter limit. A666's first reserve submission failed on that limit. Bundle 06b succeeded at h342 after validator release `a666-mainnet-0f0a621`.

The fee quote still enforces the limit. B2.2 therefore quotes submission by proof hash.

#### Keys and finalization

- **Issuer key:** creator wallet key held in the browser. It signs creation, finalization, mint, route, and authorization operations.
- **Reserve-operator key:** epoch-service key. It signs only `nav_reserve_submit`. Execution admits the issuer or reserve operator at `nft_escrow_asset_execution.rs:1201`.

Finalization is issuer-only today. Until B2.5, each epoch appears as a one-tap **Finalize epoch N** prompt in the creator wallet. B2.5 permits reserve-operator finalization after an unchallenged window.

#### Rotating the route's Ethereum bridge committee

`pftl_uniswap_route_bridge_policy_update` moves a route's `ethereum_verification_policy` to the next checkpoint committee. Without it the policy is fixed at route init. The Ethereum verifier holds no committee, so no Ethereum transaction is needed.

1. The validators activate committee `N+1` with FastLane `ActivateCommittee`. That needs epoch `N`'s final drained checkpoint (`fastswap_control.rs:167`, `:386`), so epoch `N` keeps signing until then.
2. The issuer signs the update: `route_id`, `authority_epoch = N+1`, the new `committee_root`, and `minimum_confirmations`, `handoff_controller_code_hash` and `wrapped_navcoin_code_hash` copied unchanged from the current policy.
3. Execution (`nav_vault_asset_execution.rs:5643`) rejects the update unless:
   - the signer is the native NAV issuer;
   - the epoch is current + 1;
   - the echoed fields match exactly;
   - the route has no outstanding export or return claim;
   - `(N+1, root)` names a governed committee on this chain (`pftl_uniswap_ethereum_verification.rs:380`) that is the newest one, has no StopPrepare fence, and whose root recomputes from its roster (`fastswap_types.rs:177`).
4. The receipt transition is `route_bridge_policy_updated`. The new epoch and root enter the canonical route string (`market_nav_asset_types.rs:3190`). Checkpoint signing then requires the new committee's keys.

Build the operation with `scripts/a666-build-route-epoch-advance.py bridge-policy-update --current-policy <policy.json> --committee-root <hex> --issuer-key-file <key> --output-dir <dir>`.

### 8.2 Mandate commitment and visibility

`AssetDefinition` in `L1/crates/types/src/account_owned_asset_types.rs:418` gives a PFTL asset:

- a `code`;
- a `display_name` of at most 64 bytes;
- opaque hashes.

It has no mandate or privacy field.

The design uses existing fields:

- **Commitment:** canonical JSON is hashed with SHA-256. The result becomes the profile's `valuation_policy_hash` in `NavProofProfile` at `account_owned_asset_types.rs:3456-3460`. The field's documentation already covers “strategy invariants (hedge bands, margin ratios).” Each proof is therefore verified against a profile committed to the mandate.
- **Publication:** the proxy serves `GET /api/navcoin/mandates/:hash`. It returns 404 unless the bytes served hash to `:hash`. Mandates are also committed under `PF/docs/navcoins/mandates/`.
- **Visibility:**
  - public sets `requires_authorization=false`;
  - private sets `requires_authorization=true`, allowing only issuer-authorized holders;
  - A666 v2 is private in this sense;
  - shielded swaps work in both modes.

**Mandate schema `postfiat.navcoin.mandate.v1`, BMNRC instance:**

```json
{
  "schema": "postfiat.navcoin.mandate.v1",
  "code": "BMNRC",
  "display_name": "BMNR Carry NAVCoin",
  "visibility": "public",
  "family": "long_stock_short_perp",
  "long_leg": {
    "venue": "felix", "chain_id": 1, "symbol": "BMNRon",
    "token": "0x33483a58079b4225b10e57958ca28ad7b9cdbaf7", "decimals": 18,
    "balance_slot": "0x33", "beacon": "0x985462c9aa4d6c3ad59ae6e1e9c0c11347ed1598"
  },
  "short_leg": {
    "venue": "hyperliquid", "dex": "xyz", "coin": "xyz:BMNR",
    "margin_mode": "isolated", "max_leverage": 3
  },
  "hedge": { "ratio": "token_qty * shares_per_token", "rebalance_band_bps": 200 },
  "cash": { "liquid_tranche_target_pct": 30, "liquid_tranche_floor_pct": 15 },
  "reserve_accounts": {
    "felix_account": "<dedicated EIP-7702 account>",
    "hyperliquid_account": "<dedicated account>",
    "cash_wallet": "<dedicated EOA>"
  },
  "valuation": {
    "stock_quantity": "proven: EIP-1186 storage proof",
    "cash": "proven: EIP-1186 storage proof",
    "stock_price": "attested: Felix primaryMarket.price",
    "hedge_value": "attested: Hyperliquid clearinghouseState(dex=xyz).accountValue",
    "route_principal": "PFTL state at submission height"
  },
  "pricing": { "initial_nav_usd": "1.00", "mint_multiplier_bps": 10050, "burn_multiplier_bps": 9995 },
  "risk_limits": {
    "min_liquidation_distance_pct": 25, "max_basis_bps": 15,
    "carry_floor_apr_pct": 0, "carry_floor_window_days": 14
  },
  "sessions": { "stock_weekend_trading": false, "calendar": "felix /v1/market/status", "max_observation_age_hours": 4 }
}
```

### 8.3 Reserve proof

| Component | Code | State |
|---|---|---|
| Policy validation | `validate_policy` (`SH/stakehub/tokenized_equity.py:83`) | Tested |
| Price observation with age check | `price_row` (`tokenized_equity.py:174`) | Tested |
| Token balance through EIP-1186 proof with beacon and code-hash guards | `collect_token` (`tokenized_equity.py:204`) | Tested |
| Hedge as isolated position; only `marginUsed` counts as cash, and notional never counts | `hedge_row` (`tokenized_equity.py:247`) | Tested |
| Packet preparation and sealing | `prepare`, `seal` (`SH/stakehub/tokenized_equity_nav.py:130, 214`) | 75 Python + 8 Rust tests; never submitted |
| Circuit leg for any ERC-20 balance | `Erc20SpotBalanceProof` (`SH/zk/shared/src/evm_spot.rs:47-57`), checked against the state root at `:138-151` | Implemented |

#### Existing BMNR evidence

`SH/config/tokenized-equity-bmnr.json` ran read-only against the live BMNR pair on 2026-09-30.

The run:

- verified storage proofs for 294.5387 BMNRon;
- valued them at $7,894.33;
- used Ethereum block 26,091,430;
- passed the native EVM verifier;
- produced candidate net assets of $10,892.67.

Appendix B records the full output and hashes.

The report identified five requirements before activation, which are the Phase 1 gate:

1. an authenticated source checkpoint;
2. reserve-wallet authorization and NAV allocation;
3. governed valuation evidence;
4. a Hyperliquid receipt proof for the selected dex and isolated position;
5. a profile-bound aggregate proof and epoch finalization.

The run also identified two B1.1 defects:

- the hedge-basis note is hard-coded to the previous UNGon/NATGAS pair;
- `publicnode` rejects `eth_getProof` because the collector sends no User-Agent.

#### Dedicated perimeter

The six-leg StakeHub proof values the entire StakeHub book. Each NAVCoin instead receives:

- its own Felix account;
- its own Hyperliquid account;
- its own cash wallet.

The proof must cover exactly these positions under B3.3.

#### Hyperliquid receipt-reader limitation

`HyperCoreReader` snapshots one `perpDexIndex`, and the operator constant is `0`:

- `SH/stakehub/hl_snapshot.py:33-35`
- `SH/zk/contracts/src/HyperCoreReader.sol:64-73`

HIP-3 positions are therefore attested rather than receipt-proven.

The `xyz:BMNR` precompile index, 10118, fits the reader's `uint16`. Extending the reader remains follow-up work beyond this build.

#### Proof binding and freshness

The live `sp1-groth16` verifier verifies the proof and decodes totals, but binds no genesis, asset, epoch, or observation time:

- `verify_sp1_groth16_with_options` at `L1/crates/execution/src/nav_sp1_verifier.rs:344-393`;
- finding G1 in `~/repos/postfiatl1v2-fastpay-committee-20260925/docs/review/nav-reserve-proof-mechanics-review-20260924.md`.

`sp1-nav-reserve-v1` closes these gaps. `NavReserveVerifyContext`, built at `L1/crates/execution/src/nft_escrow_asset_execution.rs:1343-1362`, binds:

- PFTL genesis hash;
- NAV asset;
- proof profile;
- valuation policy hash and therefore the mandate;
- source manifest;
- valuation unit;
- observation epoch;
- current height, checked at `nav_sp1_verifier.rs:262-285` through `ObservationStale`;
- route-principal overlay under a composite source root.

Execution rejects a packet unless `N` is exactly `floor(V / S)` through `nav_reserve_nav_floor_mismatch` at `nft_escrow_asset_execution.rs:1370-1384`.

B1.3 moves BMNRC to this verifier and is an outside-money gate.

This height binding does not provide wall-clock freshness. B2.6 is separately required before outside money.

### 8.4 Epoch service

Each epoch automates the loop that A666 v2 ran manually for epochs 1–8 at h342–996:

1. **Collect:** collect the perimeter and read route principal from PFTL.
2. **Prove:** run SP1 Groth16. A666's opening proof took 18 minutes on the GPU prover.
3. **Submit:** sign `nav_reserve_submit` with the reserve-operator key. Set `N = floor(V × 1e8 / S)`, leaving only rounding surplus.
4. **Finalize:** use the creator's tap until B2.5.
5. **Advance:** advance the route epoch and re-center the Uniswap band.
6. **Publish:** send the reconcile report and proof hashes to the indexer.

A systemd timer runs this loop on the cadence in §3.

When the calendar reports the stock closed, the service pauses the route instead of waiting for block staleness. BMNRon does not trade from Saturday 00:05 to Sunday 23:55 UTC.

### 8.5 Invariants and failure handling

#### Executable invariants

| ID | Required invariant | Enforcement | Acceptance or operational check |
|---|---|---|---|
| I1 | `N = floor(V / S)` for every finalized epoch | Consensus through `sp1-nav-reserve-v1` and `nav_reserve_nav_floor_mismatch` | B1.3 |
| I2 | PFTL spendable + Ethereum spendable + in-flight bridge claims = authorized supply | Bridge through `navcoin_bridge_supply_status.invariant_holds` | Indexer alarm on any `false` |
| I3 | Fees and keeper alignment reserve never count in `V` | Packet construction and separate custody accounts | Reconcile report lists every counted account |
| I4 | Route trades only while Open: not paused, epoch ≤100 blocks, and observation ≤4 hours old | Consensus enforces the first two conditions. Before B2.6, hosts enforce the third; after B2.6, consensus enforces all three. | B2.4 quiet-chain test for operator demo; B2.6 stale-buy rejection for outside money |
| I5 | Hedge residual ≤1 share | Executor | Check after every fill and epoch |
| I6 | Liquidation distance ≥25% | Executor margin top-up | Check each epoch; watchdog pauses below 20% |

I4 has two distinct acceptance levels:

- **Operator demo:** B2.4 demonstrates that the pause infrastructure works with Post Fiat's own money.
- **Outside money:** B2.6 must make stale-observation rejection fail closed in consensus. No operator-host availability assumption may be required for this rule.

#### Drift between observations

| Channel | Typical size | Bounding rule |
|---|---|---|
| Carry | 40% APR on a hedge notional of 44–67% of `V` is about 0.05–0.07% of `V` per day | none needed at this size |
| Basis between Felix price and perp mark | ±20 bps observed across 87 pairs on 2026-09-30 | Executor samples every minute and pauses above 50 bps |
| Hedge residual | at most 1 share | Executor check after every fill |

Within these limits, drift during the 4-hour observation window is well under the 55 bps route spread. The spread is a buffer, not a guarantee. Tail events beyond these limits, including a stock halt while the perpetual continues trading, are handled by pausing rather than by relying on the spread.

#### Operator-demo pause mechanism

Before B2.6, failures resolve to Paused or Halted through an operator-side transaction. This is an operational mechanism, not a consensus guarantee.

Each resume is paired with a pre-signed pause transaction held on three independent hosts:

1. epoch service;
2. watchdog;
3. third monitor.

Any one may submit it when the observation reaches 4 hours.

Two stale-trade cases remain possible before B2.6:

1. all three hosts are down at the 4-hour mark;
2. the validator fleet is down across the 4-hour mark and, after recovery, queued pause and trade transactions land in the same batch.

These cases are accepted only for the operator demo with Post Fiat's own money. They are why outside money is blocked until B2.6. After B2.6, certified block time causes consensus to reject a trade using an over-age observation, and pausing becomes a second line of defense.

| Failure | Detection | Automatic response | Displayed result |
|---|---|---|---|
| Epoch service or prover stops | Watchdog sees no finalized epoch within 4 hours of an open session | Watchdog submits `pftl_uniswap_route_pause` using reserve-operator pause authority on a separate host | Route Paused; Uniswap and shielded transfers continue |
| Issuer misses finalize tap | Same | Same | Same |
| Stock venue closes or halts | Calendar | Pause | Route Paused until the next epoch after reopening |
| Hedge venue halts or delists | Executor health check | Issuer executes `nav_halt` after a creator-wallet prompt | Route Halted |
| Liquidation distance <20% | Executor | Margin top-up; pause if top-up fails | Route Paused |
| I2 fails | Indexer | Pause and alert | Route Paused |
| Watchdog is down | Epoch-service and third-monitor heartbeats | Either submits the pre-signed pause | Route Paused |
| Live basis >50 bps | Executor sampling every minute | Pause | Route Paused until basis <30 bps and a new epoch finalizes |

#### Quiet-chain test for B2.4

On a fork with all validators running:

1. finalize an epoch;
2. submit no other transactions for 48 hours of wall-clock time;
3. require the pre-signed pause to finalize within 4 hours and 10 minutes of the observation;
4. submit a buy at hour 48 and require rejection because the route is paused.

A second run stops the watchdog and confirms that the third monitor submits the pause.

This test validates the operator-demo mechanism only. The separate B2.6 test must prove consensus rejection without any pause transaction.

## 9. Liquidity engineering

### 9.1 PFTL route

| Action | Wallet builder | Consensus handler in `L1/crates/execution/src/nav_vault_asset_execution.rs` |
|---|---|---|
| Buy | `buildNavcoinIssueOperations` at `FL/wallet-web/src/lib/navcoin-primary-route.js:213`: order reserve → subscribe → release | `apply_pftl_uniswap_primary_subscribe_v2` at :4699; pricing at :6465-6577 |
| Sell | primary exit builder at `navcoin-primary-route.js:375` | `apply_pftl_uniswap_primary_redeem` at :5344 |
| Capacity | — | Buys increase circulating supply within `issue_capacity_atoms`; sells lower it at :4798-4912 and :5290-5316 |
| Block freshness | — | NAV ≤100 blocks old at `L1/crates/node/src/core_chain.rs:250` |

The Trade screen currently reaches one route through the hard-coded adapter `'pftl-uniswap-primary-v2'` at `FL/wallet-web/src/lib/navcoin-markets.js:87`. B5.3 keys this adapter by route.

### 9.2 Ethereum stack per NAVCoin

| Contract | A666 v2 instance | Code | Reusable? |
|---|---|---|---|
| `WrappedVenueNAVCoin` | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5` | `FL/crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol:903` | Yes. Constructor accepts name, symbol, and decimals. |
| `PFTLReceiptFinalityVerifierV1` | `0xb79FF97Ecc11574A8a78D0B5a9d7C8C2A94bF96a` | `PFTLReceiptFinalityVerifierV1.sol:174, 217` | One per NAVCoin with its own checkpoint |
| `PFTLUniswapPrimaryMarketV2` | `0x9A0262C0572fb4DB08765408eB225E207F40c3d9` | `PFTLUniswapPrimaryMarketV2.sol:164, 218` | **No.** `A666_ROUTE_SUPPLY_CAP_ATOMS` and `A666_PACKET_NOTIONAL_CAP_ATOMS` are constants, and other values revert at :90-91 and :137-138. |
| Deploy script | — | `script/DeployA666PrimaryMarket.s.sol:82` | **No.** It hard-codes “Post Fiat a666” and “wA666”. |

Pool parameters:

- fee 500, equal to 0.05%;
- tick spacing 10;
- both match the A666 pool and a651 helper at `NavCoinV4LaunchHelper.sol:61-62`.

Initial price is `N`, using the `sqrtPriceX96` derivation at `NavCoinV4LaunchHelper.sol:249-265`.

Band seeding reuses `NavCoinV4LaunchHelper.seedExistingPosition` at `SH/zk/contracts/src/navcoin/NavCoinV4LaunchHelper.sol:159-199`.

The a651 pool used such a band as NFT 334741 on 2026-07-04.

`NAVGuardHook.sol` exists at `FL/crates/ethereum-contracts/src/NAVGuardHook.sol:8` but has never been deployed. It remains a later option after keeper behavior is measured.

## 10. Mandate executor and role separation

Felix trades use EIP-712 intents signed through a Turnkey session. They currently run outside the StakeHub agent in `WA/bmnr-roll-20260924.cjs`, which implements:

- login;
- quote;
- envelope checks through `checkEnvelope`;
- order;
- fill polling.

Hyperliquid orders use the agent's `hl_order` operation at `SH/stakehub/agentd.py:2863-2911`, which applies agent spending caps.

### 10.1 Executor operations

| Operation | Effect | Guards |
|---|---|---|
| `open(notional)` | Felix buy in the NAVCoin Felix account plus Hyperliquid IOC short of `token_qty × shares_per_token` in its Hyperliquid account | quote age ≤30 s; fee = 20 bps; price bounded by perp book; basis ≤ `max_basis_bps`; post-fill residual ≤1 share |
| `rebalance()` | Restore share hedge after dividends or splits; top up margin below `min_liquidation_distance_pct`; after B3.4, deploy or replenish the liquid tranche under §2 | same guards; daily cap |
| `close(fraction)` | Sell stock and buy back the short pro rata | same guards |

### 10.2 Roles

Post Fiat holds every role during the operator demo. Outside money requires the separation in the final column.

| Role | Key or credential | Operator demo | Required before outside money |
|---|---|---|---|
| Issuer for creation, minting, holder authorization, and halt | Creator wallet key in browser | Post Fiat | Post Fiat using a distinct key |
| Reserve operator for epoch submission, pause, and finalization after B2.5 | Epoch-service key | Post Fiat | Distinct key held only on the epoch-service host |
| Attestors for stock price and hedge value | ed25519 keys | One Post Fiat key | Two keys, one held outside Post Fiat under B1.2 |
| Executor for Felix and Hyperliquid legs | Perimeter account keys in the BMNRC executor signer, with per-mandate caps | Post Fiat | Distinct keys with caps set per mandate |
| Keeper for alignment reserve | Keeper wallets | Post Fiat | Distinct keys and assets, never taken from the perimeter |
| Watchdog and third monitor for pause | Pre-signed pause transactions | Post Fiat | Separate hosts; after B2.6, backstop only |

### 10.3 Authentication and incident boundary

The Create screen authenticates to the executor using a **creator credential** issued at creation and separate from the public wallet session.

This separation is required because SEC-20260930-01, an open wallet-proxy credential-exposure incident, affects a component on the money path. Incident details remain private; this specification intentionally includes no endpoint, code location, mechanism, or wallet network address for it.

B0.0 requires containment, token rotation, request logging, gas-spend reconciliation, and external verification before any other work. Availability from the public wallet URL is only regression evidence and is not evidence that the security incident is closed.

## 11. Investor path

| Step | Action | Mechanism and code | Measured result |
|---|---|---|---|
| 1 | Deposit USDC | MetaMask `approve` + `depositV2` at `FL/wallet-web/src/lib/evm.js:420, 435` → relay job `POST /api/bridge/jobs` at `FL/wallet-proxy/navswap-persistence-http.js:1618` → SP1 Ethereum-finality proof → pfUSDC claim | 20 min 12 s deposit-to-withdrawal round trip in `FL/docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/` |
| 2 | Buy at NAV | Route buy under §9.1 | one PFTL round |
| 3 | Export to Ethereum | `pftl_uniswap_export_debit` at `nav_vault_asset_execution.rs:5681` → SP1 receipt proof → `verifyAndAccept` → `consumeMintOnly` at `PFTLUniswapPrimaryMarketV2.sol:164` | 18 min proof for A666 opening export |
| 4 | Trade on Uniswap | MetaMask swap under §5.3 | one block |
| 5 | Return to PFTL | `burnForPftlReturn` at `PFTLUniswapPrimaryMarketV2.sol:218` → 5-of-6 validator checkpoint → `pftl_uniswap_return_import` at `nav_vault_asset_execution.rs:6175` | six round trips on 2026-08-10 |
| 6 | Sell at NAV | Route sell under §9.1 | one PFTL round |
| 7 | Withdraw USDC | `vault_bridge_burn_to_redeem` → Groth16 egress proof → `withdrawWithProof` at `FL/crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:191` | egress proof 136 s on an A100 |

The complete path has run once with real money.

On 2026-09-07:

- 10 USDC became 9.611565 A666;
- the A666 moved to wA666;
- it traded through Uniswap;
- it returned to PFTL;
- it exited as 9.932860 USDC;
- result: PASS at h1020.

Evidence: `SH/docs/handoffs/navcoin-recovery-20260907/resumed-epoch10/roundtrip-PASS.json`.

### Bridge trust

- **Export:** trustless SP1 proof of the PFTL receipt.
- **Return:** 5-of-6 validator checkpoint, not an Ethereum light client.
- **pfUSDC ingress:** Ethereum finality verified through SP1 Groth16 using `sp1-ethereum-finality-v1`, route epoch 10, activated h1008.

## 12. Shielded swap

### 12.1 Mechanism

The Asset-Orchard pool is `asset-orchard-v1`. Its circuit is `asset_orchard.swap.pricing_bound.v4` at `L1/crates/privacy_orchard/src/asset_orchard.rs:19-28`.

Supported pricing modes at `asset_orchard.rs:847-902` are:

- `at_nav`
- `at_nav_with_band`
- `negotiated`

A shielded buy is a private primary issue. A shielded pfUSDC note is spent, and an encrypted NAVCoin note is minted at `N × 1.005` through `apply_asset_orchard_private_primary_issue_route_transition` at `nav_vault_asset_execution.rs:4934`.

### 12.2 State on 2026-09-30

| Item | State |
|---|---|
| Conserved private round trip, pfUSDC → a651 → pfUSDC | Ran repeatedly from 2026-07-20 to 07-23 |
| One-way private buy through A666 private primary issue | Proven once at h420 on 2026-07-28; limited to one wallet and one unit per request |
| Wallet screen | `PftlPrivatePrimary.jsx:98` exists but is unwired; Caddy does not proxy `/api/pftl-private-swap/*` |
| Prover | `postfiat-pftl-high-core-prover.service` is inactive |

### 12.3 Privacy boundary

Hidden inside the pool:

- asset ids;
- values;
- owners;
- recipients;
- price.

Public at entry or exit:

- asset;
- value.

Public at all times:

- reserve packets;
- supply;
- NAV epochs.

B5.4 wires the screen, proxies the API, supervises the prover, and increases the per-request limit to the route packet cap.

## 13. Data services

### 13.1 Pair universe

Felix API base:

```text
https://spot-equities-proxy.white-star-bc1e.workers.dev
```

Calls use:

```text
Origin: https://trade.usefelix.xyz
```

Join process:

```text
felix GET  /v1/market/universe                      -> tokens by ticker, Ethereum address
hl    POST /info {"type":"perpDexs"}                 -> dexes (skip the null main dex and delisted dexes)
hl    POST /info {"type":"metaAndAssetCtxs","dex":d} -> mark, funding, OI, isolated and growth flags
join  exact ticker match                             -> 87 pairs on 2026-09-30 (63 xyz, 19 para, 5 io)
felix GET  /v1/market/prices/{sym}                   -> shares_per_token = primary / underlying
felix POST /v1/trading/soft-quote + hl l2Book         -> basis, spread, depth (sampled together)
hl    fundingHistory (hourly, cached)                -> 7-day, 30-day, worst-week APR
```

Evidence collectors:

- `r3_collect.py`
- `r3_basis.py`
- location: `~/repos/orc_directives/docs/navcoin-spec-recon/R3-evidence/`

B5.5 serves the result at `GET /api/navcoin/universe`.

Refresh cadence:

- prices every 5 minutes;
- funding hourly.

The server precomputes the universe because a cold 30-day funding pull costs about 6,600 weight against Hyperliquid's 1,200-per-minute budget.

Required screen rules:

- Show every dex for a ticker. NBIS pays 29% APR on `io` and 8% on `xyz`.
- Size shorts using `shares_per_token`. NFLXon represents 10 shares per unit.
- Flag `strictIsolated`, where margin cannot be withdrawn.
- Flag `growthMode`. Taker fee is 0.9 bps with it and 9 bps without it.
- Read weekend availability per stock. 27 pairs trade on weekends; BMNRon does not.

### 13.2 History and TVL indexer

No history or TVL endpoint exists today:

- `nav_reserve_proof_status` returns only the 16 newest packets at `L1/crates/node/src/rpc_dispatch.rs:576`;
- `navcoin-market-data.js` serves spot for one hard-coded route at `FL/wallet-proxy/navcoin-market-data.js:3, 97-101`.

B5.5 adds `navcoin-indexer.js`.

Every 5 minutes and at each epoch, it samples every live route:

- `N`, epoch, and `V`;
- supply split through `navcoin_bridge_supply_status`;
- Uniswap spot and pool reserves through `StateView`;
- route principal and state.

Samples are stored in SQLite and served through:

- `GET /api/navcoin/:route/history?window=24h|7d|30d|all`
- `GET /api/navcoin/:route/tvl`

Charts use inline SVG, which complies with the wallet content-security policy:

```text
script-src 'self' 'wasm-unsafe-eval'
```

Source: `FL/wallet-web/vite.config.js:9, 14-26`.

## 14. Build plan

### 14.1 Dependency rules

The build agent is Codex, “the orc.” Post Fiat reviews acceptance evidence.

The dependency rules are:

1. **B0.0 containment deploys first.** No other item deploys or moves money before it. Read-only investigation and code preparation may run in parallel.
2. Within the operator-demo track, Post Fiat reviews each phase before dependent work proceeds.
3. B2.5 and B2.6 are outside-money protocol gates and may continue after operator-demo-dependent Phase 2 work. They do not permit later outside-money release steps to pass until complete.
4. B3.4 is a later v2 item and does not block the v1 operator demo.
5. Phase 6 may use only Post Fiat's own money unless every outside-money gate in Status at a glance has passed.
6. No outside-money release may rely solely on the B2.4 three-host pause mechanism. B2.6 must pass.
7. B0.0 containment does not wait for a signer restart. Until B0.8 ships, operator gas is bounded by the proxy's per-job economic preflight and job caps from B0.0. B0.8 adds the signer-enforced budget and gates outside money.
8. In Phases 0–5, acceptance tests that move money use Post Fiat-owned test wallets only. Each flow is capped at $20 and each phase at $50. Phase 6 spends under the §15.2 capital plan. Every transaction hash, height, and receipt code is recorded in the item's evidence.
9. Phase 7 gates outside money only and does not block the operator demo.

### Phase 0: platform — blocks everything

| Item | Required work | Acceptance evidence |
|---|---|---|
| B0.0 | **First action:** close SEC-20260930-01 under its private containment plan, including fix, token rotation, request logging, and reconciliation of operator gas spend. Containment is a wallet-proxy change and does not wait for B0.8 | Private incident record marked closed with external verification attached; redacted public closure note in `PF/docs/security/`; separate regression evidence that the wallet still trades and withdraws from its public URL; the deposit leg is evidenced in B0.4 |
| B0.1 | Merge `FL` wallet-web, proxy, and contracts into `PF`; serve the wallet from a `PF` build | Served `dist/` hash equals a `PF` `main` build; wallet opens remotely and shows real balances |
| B0.2 | Restore 6/6 validator convergence on a build whose commit exists in `PF` | All six validators report the same height, same state root, and a `PF` commit |
| B0.3 | Issue creator credentials for Create and the executor, separate from wallet sessions | Wallet session cannot call executor or creation routes; creator credential can |
| B0.4 | Repin pfUSDC ingress relay to the epoch-10 vault and clear readiness | 1 USDC deposit claims pfUSDC end to end; readiness returns `ready:true` |
| B0.5 | Revive export and return relays | 1-unit export and 1-unit return through relay endpoints |
| B0.6 | Publish stranded-assets note for epoch-5 vault, 195.03 USDC and impaired, and epoch-6-successor vault, 14.08 USDC | Note with recovery plan merged in `PF/docs/security/` |
| B0.7 | Reconcile pfUSDC supply. Today `asset_info.outstanding_supply` and the bridge status counters (issued, circulating, shielded) disagree, and no document explains the difference | One table, to the atom: ledger supply = transparent balances + shielded pool + in-flight exports, and bridge issued − burned = ledger supply = USDC held by live vaults, net of the B0.6 stranded amounts. Any unexplained difference is RED and blocks B0.4 |
| B0.8 | Pin the StakeHub signer and bound its spend. The running signer loads a different checkout from its service unit, and it sets withdrawal gas with no per-job cap. Deliver a pinned checkout and a signer-enforced per-job gas budget as one package installed in one planned restart | Loaded code = unit working directory = named commit; signer tests refuse a job whose worst-case signed gas cost exceeds its budget, including retries; one restart; $1 withdrawal completes afterward from the public URL |
| B0.9 | Serve the public wallet with a CA-trusted TLS certificate under managed renewal | A stock browser and MetaMask open the wallet URL with no certificate warning; renewal is scheduled and survives a host reboot |

### Phase 1: mandate proof

| Item | Required work | Acceptance evidence |
|---|---|---|
| B1.1 | Create BMNR policy for dedicated perimeter; fix hard-coded hedge-basis note and missing User-Agent | `validate_policy` passes; dedicated-perimeter dry run reproduces balances by storage proof; live-pair dry run in §8.3 already passes |
| B1.2 | Build ed25519 attestor service for Felix price and Hyperliquid hedge value; outside-money configuration requires the independent second key in Status at a glance | Attestations verify against profile attestor key; before outside money, price and hedge attestations require two keys and one is held outside Post Fiat |
| B1.3 | Adopt `sp1-nav-reserve-v1` for new profiles | Proof replayed from a prior epoch is rejected on ce22; packet with incorrect `N` is rejected |
| B1.4 | Implement end-to-end collect → GPU prove → packet | Packet `V` equals an independent read of perimeter plus route principal to the cent |
| B1.5 | Count route principal through the `sp1-nav-reserve-v1` route-principal overlay | Fork epoch after a buy and sell shows `V` = proven perimeter + overlay, and `N` is unchanged by each transition |

### Phase 2: PFTL issuance and freshness tooling

| Item | Required work | Acceptance evidence |
|---|---|---|
| B2.1 | Wallet builders for all eight operations in §8.1, signed by the served wallet's WASM signer | Each reproduces the matching A666 v2 ops file byte for byte in tests; the WASM signer signs every operation kind and ce22 accepts each one |
| B2.2 | Fee quote by proof hash for `nav_reserve_submit` | Full-size SP1 submission is quoted and accepted from wallet |
| B2.3 | Mandate service for canonicalization, hashing, serving, and verification | Served bytes hash to profile `valuation_policy_hash` |
| B2.4 | Epoch service with cadence, calendar pause and resume, plus independent watchdog on separate host | Three consecutive scheduled epochs finalize with no manual action except finalize tap; one calendar pause/resume cycle; §8.5 quiet-chain test passes |
| B2.5 | Consensus change allowing reserve operator to finalize after an unchallenged window; ships in validator release | On ce22, epoch finalizes without issuer signature and challenged epoch does not |
| B2.6 | Consensus change making validators sign median wall-clock time into each block certificate; route pricing rejects observations older than mandate `max_observation_age_hours`; ships in validator release | On a fork with no epoch for 4 hours of wall-clock time, a buy is rejected by consensus without a pause transaction; on ce22, a buy using an observation older than 4 hours is rejected by consensus |

### Phase 3: mandate execution

| Item | Required work | Acceptance evidence |
|---|---|---|
| B3.1 | Add perimeter operations `hl_withdraw`, restricted to the perimeter's own Arbitrum address, and `hl_usd_transfer` to the executor signer from B3.3. The StakeHub agent is not changed | Executor-signer test suite passes; $5 withdrawal lands |
| B3.2 | Build executor `open`, `rebalance`, and `close` with §10 guards | $20 BMNR open and close with residual ≤1 share |
| B3.3 | Provision a dedicated Felix account, Hyperliquid account, and cash wallet under distinct keys held by a BMNRC executor signer with per-mandate caps, separate from the StakeHub agent | Three addresses published in mandate; executor signer refuses an order above its cap |
| B3.4 | Design and build `route_principal_release` and `route_principal_restore` for §2 v2 | On a fork, each operation leaves `V` unchanged and respects cash targets |

### Phase 4: Ethereum stack

| Item | Required work | Acceptance evidence |
|---|---|---|
| B4.1 | Parameterize `PFTLUniswapPrimaryMarketV2` caps and generalize deploy script | Fork test deploys second route with non-A666 caps |
| B4.2 | Generic pool initializer and band seeder replacing deleted A666 script from `git show 09d0c6350:scripts/a666-uniswap-mainnet-initialize.py` | Fork pool initializes at `N`; band is `N × [0.99, 1.01]` |
| B4.3 | Arbitrage keeper with two-chain alignment reserve | Fork tests: injected 5% deviation returns to band in one cycle using inventory alone; with route paused, sustained one-way flow of 2× keeper inventory is recorded and displayed deviation matches pool price |

### Phase 5: wallet

| Item | Required work | Acceptance evidence |
|---|---|---|
| B5.1 | Create stepper from §5.4 | All seven steps run on a fork |
| B5.2 | Explore and extended Asset detail from §5.1–§5.2, including §3 route state | Every panel populated from live A666 and BMNRC data |
| B5.3 | Route-keyed Buy / Sell adapter | Buy and sell work for both routes from one screen |
| B5.4 | Wire `PftlPrivatePrimary.jsx`, proxy API, and supervise prover | One shielded BMNRC buy |
| B5.5 | Universe and indexer endpoints from §13 | Pick table serves 87+ pairs; history serves 7 days |

### Phase 6: live creation and operator demo

| Item | Required work | Acceptance evidence |
|---|---|---|
| B6.1 | Create BMNRC through Create using real money | Asset, route, wrapped asset, and pool ids published; epoch 1 verified on PFTL |
| B6.2 | Run investor walkthrough from §11 starting in MetaMask | Every transaction hash and height recorded; supply conservation checked |
| B6.3 | Shielded buy and sell of BMNRC | Batch ids recorded |
| B6.4 | Complete §15 documentation | Pages merged and linked from NAVCoin index |

### Phase 7: outside-money release gates

| Item | Required work | Acceptance evidence |
|---|---|---|
| B7.1 | Independent audit of `PFTLUniswapPrimaryMarketV2`, the wrapped-asset and vault contracts, the `sp1-nav-reserve-v1` program, and the Orchard circuit | Published report; every high-severity finding fixed and re-verified by the auditor |
| B7.2 | Legal review of the issuance structure, holder eligibility, and the Felix/Ondo eligibility constraints | Written opinion per launch jurisdiction; its eligibility rules are encoded in Create and the Access panel |

## 15. Capital and documentation

### 15.1 Capital source

The operator demo uses capital from the StakeHub book.

On 2026-09-30, the weakest steady carrier, the HYPE basis pair at 6.7–9.1% APR realized, was unwound. $5,000 moved to Ethereum mainnet. Appendix A contains the execution record.

### 15.2 BMNRC capital use

| Allocation | Amount | Location |
|---|---|---|
| Stock: BMNRon on Felix | $400 | Ethereum perimeter |
| Hedge margin: `xyz:BMNR`, ≤3x | $200 | Hyperliquid perimeter account |
| Liquid tranche through creator route buy | $300 pfUSDC | PFTL route principal |
| Uniswap band, USDC side | $400 | Ethereum |
| Uniswap band, NAVCoin side using exported units | ≈400 units | Ethereum |
| Keeper alignment reserve | $300 across both chains | Ethereum and PFTL |
| Investor walkthrough | $100 | MetaMask → PFTL |
| Gas and proving | $150 | Ethereum; GPU prover |

At opening:

- `V` ≈ $600 from stock plus hedge account;
- opening supply is about 600 units at $1.00.

The liquid-tranche buy then adds:

- about 298.5 units;
- $298.50 of principal;
- the remainder is the 0.5% fee.

This brings `V` to about $900, with principal equal to 33% of `V`.

In v1, this principal remains on PFTL as exit liquidity. After B3.4, the executor deploys principal above the 30% target into stock and hedge. About 400 of the creator's units are exported to seed the band.

### 15.3 Documentation deliverables

The following pages under `PF/docs/navcoins/` are updated each phase and linked from the index:

- this specification, with a status column for each build item;
- `mandates/README.md` and every published mandate;
- `navcoin-creation-runbook.md`, including recovery for every failed step;
- `launches/<code>-<date>.md`, recording every operation, height, hash, and amount;
- updates to `primary-market-accounting.md` for the liquid tranche and alignment reserve;
- updates to `assets-and-venues.md`;
- updates to `pftl-tools.md`.

`SH/docs/` receives:

- executor documentation;
- tokenized-equity profile documentation;
- capital record.

## 16. Risks and release decisions

| Risk | Consequence | Required response |
|---|---|---|
| **Security incident SEC-20260930-01** affecting the wallet proxy (closed 2026-10-01) | Impact class is unauthorized invocation of operator-paid relay services. Closure must confirm that no user-signed transfer is within its reach. | B0.0 closes before any other work. Redacted closure note is published in `PF/docs/security/`. Closure gates both readiness levels. |
| Stale NAV during host or validator outage before B2.6 | A trade may execute using an observation older than 4 hours if all pause hosts fail or queued transactions race after fleet recovery. | Operator demo only with Post Fiat money under the three-host mechanism. Outside money is prohibited until B2.6 makes stale-observation rejection a consensus rule. |
| Funding reversal | `N` falls while shorts pay funding. At −20% APR on a hedge equal to 44% of `V`, the BMNRC mix after the liquid-tranche buy, `N` falls about 0.7% per month. | `carry_floor_apr_pct` of 0% over 14 days triggers `close(0.5)` and a creator alert. BMNR's worst rolling week to date was +17.3% APR. |
| Hedge-venue halt; six of ten HIP-3 dexes are fully delisted, including Felix's `flx` | Short cannot be adjusted | Prefer `xyz`, the largest deployer; route moves to Halted |
| Weekend gap with stock frozen and perpetual live | Mark divergence | Calendar pause while stock venue is closed; liquidation buffer ≥25% |
| Attested stock price and hedge value | Dishonest attestor can misstate `V` | Display assurance level per input; require independent second attestor before outside money; HIP-3 receipt proof remains named follow-up |
| `sp1-groth16` binds no epoch or height | Proof may replay across epochs | BMNRC launches on `sp1-nav-reserve-v1`; B1.3 is a launch gate |
| Submitter chooses `nav_per_unit` under `sp1-groth16` | Only collateral check constrains the value | BMNRC launches on `sp1-nav-reserve-v1`, where consensus enforces `N = floor(V / S)`; B1.3 is a launch gate |
| Heights measure activity instead of time | Block freshness can span more than a week | Operator demo uses calendar pause and three-host pre-signed pause under §8.5. Outside money requires B2.6. |
| Sell demand exceeds route principal | Holders cannot immediately sell their full amount at NAV | Quote exposes available amount. v1 is explicitly principal-limited. B3.4 v2 can replenish principal by unwinding stock plus hedge within mandate limits. |
| Return bridge uses a 5-of-6 checkpoint | Ethereum → PFTL depends on validator trust | Disclose on Bridge screen |
| Felix API is undocumented | Quote or order failure | Executor fails closed; display ages |
| Felix/Ondo eligibility excludes US persons | Legal exposure | Creation restricted to operator |

### Decisions and recommendations

1. **Creator credential scope:** operator only in the first release.
2. **Reserve-operator finalization, B2.5:** design now and ship after the operator demo; it remains mandatory before outside money.
3. **Certified wall-clock freshness, B2.6:** may ship after the operator demo but is mandatory before outside money.
4. **Principal movement, B3.4:** later v2 capability and not part of the v1 operator demo.

---

# Appendices

## Appendix A: Capital record, 2026-09-30

The HYPE basis pair was the weakest steady carrier in the StakeHub book, with 6.7–9.1% APR realized on every window. It was unwound to finance the operator demo.

| Step | Action | Record |
|---|---|---|
| 1 | Sold 71.21 HYPE spot at $87.691 and bought back the 71.21 HYPE short at $87.609, with entry at $93.89 | `WA/stakehub-demo-capital-20260930/unwind-log.jsonl`; agent journal `~/.stakehub/journal.jsonl:14862-14863` |
| 2 | Hyperliquid USDC rose from $11,669 to $17,904; portfolio margin ratio moved from 0.206 to 0.178 | same |
| 3 | $5,000 moved to the whitelisted basis wallet on Hyperliquid at 16:12Z, was withdrawn to `0x1455…8c0` on Arbitrum at 16:13Z and arrived as $4,999.00, then moved through the agent's CCTP operation to Ethereum mainnet in 19 minutes: burn `0x63a9ee72…a3792b` on Arbitrum and mint `0xe54df36b…69739c` on Ethereum. `0x1455…8c0` then held $5,673.01 USDC on Ethereum. | `WA/stakehub-demo-capital-20260930/capital-log.jsonl` |

## Appendix B: BMNR dry-run output, 2026-09-30

A BMNR policy was created only as configuration at `SH/config/tokenized-equity-bmnr.json`.

It is the UNGon template with token address, symbol, and hedge coin changed. Slot `0x33`, the beacon, and the code hash are identical.

It ran read-only against the live BMNR pair:

```text
$ stakehub prove-reserves --tokenized-equity config/tokenized-equity-bmnr.json --dry-run \
    --mainnet-rpc https://eth.drpc.org --out-dir /home/postfiat/var/tokenized-equity-bmnr-20260930/run2
BMNRon: 294.538733225082475713 | USD 7894.33
USDC: 5.893736 | USD 5.89
xyz:BMNR: -294.53 (short)
Assigned isolated equity: USD 2992.44
Candidate net assets: USD 10892.67
Native EVM verifier: PASS
```

| Field | Value |
|---|---|
| Ethereum block | 26,091,430, state root `0x1a550f01…6b26b5` |
| Policy SHA-256 | `8b9de9eb…d373d9` |
| Evidence SHA-256 | `7f2ca792…a188b7` |
| Net assets | `1,089,266,506,907` (USD × 1e8) |
| Short notional | $7,895.76, excluded from assets as exposure |