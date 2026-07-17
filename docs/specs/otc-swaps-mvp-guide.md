# NAVCoin OTC Swaps — End-to-End MVP Guide

> Status: **active spec.** Supersedes the speed-first framing. This is the canonical
> guide the build agent (and reviewers) execute against.
> Owner: Post Fiat founder. Repo of record: `postfiatl1v2`. Cross-repo docs also land
> in `StakeHub`.

## Purpose

A **professional, structurally-correct demo** of NAVCoin OTC functionality: real value
moving through the full **pfUSDC ↔ NAVCoin round trip** on live Arbitrum + PFTL, proven
by an end-to-end test battery, documented in both repos, then extended to cross-NAVCoin
swaps, code-reviewed, and shipped as a **blog post with diagrams**.

## North-star priority

**Correctness and structural soundness over speed.** Time is not the constraint. The
constraint is proving it actually works — real value transferred, invariants observable —
flow by flow. No simulated or empty-asset swaps. No claim of "done" until the assertion
for that flow passes against live state.

## The 6-flow end-to-end battery (the MVP proof)

Each flow uses **real (small-dollar) value** and must pass a concrete assertion before
the next runs. Together they prove the full round trip in both directions, with the
NAVCoin backing visibly adjusting.

| # | Flow | Action | Must-pass assertion |
|---|------|--------|---------------------|
| 1 | **Bridge IN** | Arbitrum USDC → pfUSDC: `deposit` into `ERC20BridgeVault`, relay (propose/attest/finalize/claim), mint pfUSDC on PFTL | pfUSDC holder balance increases by deposit amount; vault USDC holdings increase by the same; relay finalized on the WAN devnet |
| 2 | **Bridge OUT** | pfUSDC → Arbitrum USDC: burn pfUSDC → withdrawal packet → vault proof/challenge/finality → `claimWithdrawal` | pfUSDC balance → 0; USDC arrives at the recipient on Arbitrum; vault USDC holdings decrease |
| 3 | **Subscribe INTO NAVCoin** | pfUSDC → a651 via PFTL rails (subscription/swap) | buyer a651 balance increases; pfUSDC balance decreases by the subscription amount |
| 4 | **NAVCoin TVL reflects the deposit** | (consequence of 3) the NAVCoin reserve packet recomputes | `verified_net_assets` increases by the subscribed value; the deposit is **observable** in the reserve composition (the pfUSDC/USDC source bucket line item grows); the NAV invariant `verified_net_assets ≥ valid_global_supply × nav_floor` still holds |
| 5 | **Exit NAVCoin** | a651 → pfUSDC via PFTL rails | a651 balance decreases, pfUSDC balance increases; reserve composition adjusts back down; invariant still holds |
| 6 | **Full exit to Arbitrum** | pfUSDC → USDC (repeat of flow 2) | USDC claimed on Arbitrum; round-trip closes with net USDC recoverable |

**Round-trip integrity check (cross-flow):** after the full cycle, net USDC is
recoverable and the NAVCoin backing has adjusted correctly on the way in (flow 4) and
out (flow 5) — i.e., the reserve is neither inflated nor drained by the demo traffic.

## After the 6 flows are proven (in order)

1. **Cross-NAVCoin swaps.** Swap between NAVCoin instances (e.g., a651 ↔ a second
   NAVCoin) via PFTL rails. Proves the NAVCoin↔NAVCoin path, not just NAVCoin↔cash.
2. **Exhaustive code review** of the Arbitrum smart contracts:
   `ERC20BridgeVault.sol`, `PFTLWithdrawalVerifier.sol`, and the NAVCoin-referencing
   Uniswap contract (see Integration below). Focus: fund-loss bugs, signature/replay
   security, challenge-window correctness, withdrawal-recipient binding, no operator
   ability to invent deposits or custody-withhold valid withdrawals.
3. **Professional demo + blog post with diagrams.** The end state: a published,
   diagrammed write-up of the working MVP.

## Integration requirement: the Uniswap contract

There is an existing Uniswap contract that **references the NAVCoin** but is currently
"out of the picture." It must be **identified and wired in** so the whole system ties
together — nothing orphaned.

- **Likely candidate:** `NAVGuardHook.sol` (the Uniswap v4 hook from the collateralization
  system that records venue evidence) and/or the a651/USDC Uniswap v4 pool used as a
  market-ops venue (`ethereum_uniswap_v4_a651_usdc` per the architecture post).
- **Action:** identify the exact deployed/created contract, confirm its role in the
  NAVCoin market-operations policy (venue price/observations feed the discount/premium
  triggers and reserve-deploy caps), and wire it into the end-to-end picture so the
  NAVCoin's venue evidence is live and consistent with the swap flows above.

## Funding

StakeHub holds the operating Arbitrum wallet (USDC for deposits + ETH for gas). Live
small-dollar Arbitrum deployment/transfer is **authorized**.

- **Current known balance:** ~9 USDC + gas (after the initial 1-USDC deposit test).
- This is sufficient for the structural MVP battery (small but real transfers).
- **Flag the operator IMMEDIATELY** if any flow needs more USDC or ETH than StakeHub
  currently holds — the operator will unlock/top up StakeHub.
- For flow 4 (NAVCoin TVL), the deposit must be large enough that the
  `verified_net_assets` increase is **observable** in the reserve composition (the
  source-bucket line item, not necessarily NAV-per-unit). Assess and flag if a top-up
  is needed for the TVL signal to be legible.

## Hard constraints (do not violate)

- **Real a651 only.** a651 is already registered on the PFTL WAN devnet (current binary
  confirmed by SHA256 match `37d00a5e…`). Do NOT bootstrap a fresh/parallel a651; use
  the real one.
- **PFTL is the canonical NAV/supply ledger.** The NAV invariant
  `verified_net_assets ≥ valid_global_supply × nav_floor` must hold through every flow.
- **Generic contracts, no token hardcoding.** `ERC20BridgeVault` + `PFUSDC` code at
  bootstrap; no `pfusd`/native-USDC literals in L1/contract source.
- **Hygiene:** SSH key (not sshpass); no keys/passwords in the tree or fixtures;
  pre-push secret scan; never mask a failing test — fix root cause; targeted tests.

## Documentation deliverables (both repos)

- **postfiatl1v2:** this guide + impl docs separated into (a) protocol spec, (b) product
  runbook ("how to deploy native Arbitrum USDC as a PFTL bridge asset"), (c) operator
  emergency guide (pause/challenge/signer-rotation/liquidity).
- **StakeHub:** the operator-side runbook — how to fund, deposit, relay, redeem, and
  monitor via StakeHub.

## Operating mode for the build agent

Correctness-first. Prove each of the 6 flows with real value + its assertion before the
next. If a flow fails, stop and fix the root cause (do not paper over it). Surface
funding needs and the Uniswap-integration questions early. The validators are already on
the current binary, so go straight at the flows — no upgrade detour.
