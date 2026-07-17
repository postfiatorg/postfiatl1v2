# NAVCoin OTC MVP — Proven End-to-End (2026-06-19)

> Companion evidence record to [`docs/specs/otc-swaps-mvp-guide.md`](../specs/otc-swaps-mvp-guide.md).
> Status: **FUNCTIONAL MVP PROVEN** on live Arbitrum One + the `postfiat-wan-devnet`.

## Environment

- **Source chain:** Arbitrum One. Bridged token: native USDC (`0xaf88d065e77c8cC2239327C5EDb3A432268e5831`). Operating wallet: StakeHub EVM account.
- **PFTL:** `postfiat-wan-devnet` (6 validators across ewr/ams/sgp), chain-id `postfiat-wan-devnet`. Binary at proven time: `7eabed10…`.
- **Repo / branch:** `postfiatl1v2`, branch `navcoin-market-ops-envelope`.
- **Generic contracts:** `ERC20BridgeVault` + `PFTLWithdrawalVerifier` (no token-specific hardcoding; `PFUSDC` is the PFTL asset code at bootstrap).

## 6-flow battery — ALL PASSED with real value

| # | Flow | Live result |
|---|------|-------------|
| 1 | Bridge IN (USDC→pfUSDC) | Arbitrum deposit relayed; vault held USDC == minted pfUSDC |
| 2 | Bridge OUT (pfUSDC→USDC) | owner wallet +2,000,000 atoms; vault −2,000,000 atoms |
| 3 | Subscribe (pfUSDC→a651) | primary mint; allocation retired |
| 4 | **TVL up** | a651 `verified_net_assets` 2,032,945,386,170 → **2,033,453,622,570** (Δ **+508,236,400** usd_1e8); epoch-3 NAV 508,363,405, supply 4000 |
| 5 | Exit (a651→pfUSDC) | `verified_net_assets` back to 2,032,945,386,170 (Δ **−508,236,400**); buyer received 5,083,635 pfUSDC |
| 6 | Final bridge-out | owner USDC 3,000,099 → **8,083,734**; vault 6,000,000 → **916,365** (Δ exactly 5,083,635); full EVM proof→finalize→withdraw→finalize→claim chain landed |

The **+508,236,400 / −508,236,400 symmetry** across flows 4↔5 is the core proof that the NAVCoin reserve accounting is structurally correct: a subscription raises TVL by exactly the cash deposited; an exit lowers it by exactly the cash returned.

## Cross-NAVCoin swap — PASSED

- **a652 bootstrapped** on the WAN devnet: asset `b15cf53c…a85505`, ledger-transparent profile `6b486d35…5d8fb7`, epoch 1, verified_net_assets 1000, supply 1000, NAV/unit 1. 1000 a652 minted to holder (height 70).
- **a651↔a652 swap:** maker offer (height 71) → crossing fill (height 72); maker sent 100 a652, taker sent 100 a651.
- **Final balances:** buyer 900 a651 / 100 a652; holder 1100 a651 / 900 a652.

## Structural gaps the battery surfaced → fixed (4 consensus changes)

Each was a real correctness gap (not a demo workaround), fixed with a focused test, green suite, and careful one-at-a-time validator roll (all 4 rolls clean — no fork/halt).

1. **Units scaling** (`usd_1e8` vs USDC 6-dec atoms) — primary mint priced subscriptions 100× too high. Fixed (deterministic integer scale conversion).
2. **Reserve-IN on subscribe** (commit `28071d28`, "Account for retired subscription reserves") — the retired pfUSDC allocation was not counted in `verified_net_assets`, so flow 4 showed Δ0. Fixed → flow 4 passes (+508,236,400).
3. **Reserve-OUT on redeem** (commit in the redemption-settlement series) — `nav_redeem_at_nav` burned a651 but did not release the pfUSDC allocation. Fixed (vault-bridge-aware redemption settle, legacy-safe `signing_bytes`).
4. **Redeem-settle bookkeeping** (commit `8f24d7b2`, "Settle bridge redemptions against counted value") — `vault_bridge_redeem_settle` reduced the redemption queue without reducing counted vault value → overstated free bridge capacity. Fixed → settle reduces both, keeping capacity == actual vault balance.

## Residual live state (post-battery)

- pfUSDC unallocated counted capacity **916,365 == Arbitrum vault USDC balance 916,365** (capacity no longer overstates the real vault).
- WAN devnet: all 6 validators at height 72, state root `f718ce8f…0793d`, mempool 0.
- Evidence artifacts under `~/.local/share/postfiat/pfusdc-wan-v2-20260619T021609Z/` (flow reports, relay bundles, a652 cross-nav report, final vault-bridge status summary).

## Remaining (per MVP guide)

- Exhaustive Arbitrum contract code review (`ERC20BridgeVault`, `PFTLWithdrawalVerifier`, `NAVGuardHook`) — in progress; findings → `docs/status/arbitrum-contracts-code-review-2026-06-19.md`.
- Documentation in StakeHub repo (operator runbook).
- Blog post with diagrams.
