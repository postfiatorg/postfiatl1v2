# pfUSDC Issuance-Path Inventory

Status: Stage 0 inventory — mutation-site master list for Stage 1's conservation invariant.
Author: Snaga/Burzum. Date: 2026-07-05.

Governing plan: `orc_directives/NAVCOIN-BRIDGE-VERIFICATION-BUILD-PLAN.md` (Rev 2), Stage 0.
Scope: every code path under `crates/execution/src/` that can change pfUSDC `issued_supply`,
plus the burn side needed to state conservation. This document is the spec Ghash attaches the
Stage-1 conservation invariant to.

> **How `issued_supply` is represented (read this first).** `issued_supply` is **derived, not a
> stored counter.** `issued_asset_supply()` (part_03.rs:549) computes it as
> `Σ(trustline balances for the asset) + Σ(open escrow amounts) + Σ(open offer locked amounts)`.
> Therefore a **mint** = crediting a trustline balance; a **burn** = debiting one. Every mint
> path re-reads `issued_asset_supply(...).checked_add(amount)` and compares the result to the
> cap (`asset.max_supply`, and for NAV assets `nav_asset.circulating_supply`). The
> `issued_supply_overflow` / `issued_supply_cap_exceeded` error codes are the greppable
> fingerprint of that check.

## 0. Authoritative site list (Nazgûl-ratified: 12 grouped anchors, 3 files)

`rg -n "issued_supply_overflow|issued_supply_cap_exceeded" crates/execution/src/ | grep -v tests_parts`
returns 19 literal error-code lines. They group into the following 12 issuance/read-side
anchors:

```
part_01.rs:1574,1587   apply_asset_operation  (nav_mint_at_nav branch)
part_02.rs:569,576     apply_nav_redeem_vault_bridge_settlement
part_02.rs:1367,1374   apply_vault_bridge_deposit_claim
part_02.rs:1814,1821   apply_vault_bridge_mint_from_receipts
part_02.rs:3320,3327   credit_issued_asset_balance  (shared helper)
part_03.rs:84,91       apply_issued_payment
part_03.rs:560,572     issued_asset_supply            (READ-side fold)
part_03.rs:591         issued_asset_open_offer_locked_total   (READ-side fold)
part_03.rs:609         issued_asset_open_escrow_total         (READ-side fold)
part_03.rs:637         issued_asset_reserved_total_for_account         (READ-side fold)
part_03.rs:661         issued_asset_reserved_escrow_total_for_account  (READ-side fold)
part_03.rs:692         issued_asset_reserved_offer_total_for_account   (READ-side fold)
```
(paths under `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/`.)

> **Correction to the dispatch brief (declared).** The brief described the part_03 cluster as
> "a separate issuance lane (NAV mint)". Code-verified: that is inaccurate. Of the 8 part_03
> hits, **only `apply_issued_payment` (line 84) is an actual issuance path.** The other 7
> (`issued_asset_supply` and its reservation/escrow/offer aggregators) are **read-side
> accumulation-overflow guards** inside the functions that *compute* `issued_supply`; they
> never mutate supply. The NAV mint lives in **part_01.rs:1574**. Ghash must attach the
> conservation invariant to the 6 mutation sites in §1 (and the burn side in §3), **not** to
> the read-side folds in §2.

## 1. Mutation sites — MINT (net `issued_supply` +)

| # | Site | Enclosing fn | Trigger op | Δ | Cap enforced | Conservation |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | part_02.rs:1367 | `apply_vault_bridge_deposit_claim` | `vault_bridge_deposit_claim` on a finalized deposit | **+** | `max_supply` | **The intended lane.** Mint is matched 1:1 by a counted `bridge_deposit` receipt whose `finality_ref` binds a source-chain deposit. Credits `Σcounted`; preserves `issued == Σcounted − Σredeemed`. |
| M2 | part_02.rs:1814 | `apply_vault_bridge_mint_from_receipts` | `vault_bridge_mint_from_receipts` | **+** | `max_supply` | Vault-bridge lane. Mints pfUSDC against already-counted receipt capacity (allocates `vault_bridge_supply`); no new counted value created, so it moves capacity from unallocated→circulating within the same `Σcounted`. |
| M3 | part_02.rs:569 | `apply_nav_redeem_vault_bridge_settlement` | `nav_redeem_settle` where settlement asset is a vault-bridge NAV asset | **+** | `max_supply` | Credits the settlement (vault-bridge) asset to the redeemer as the *settlement leg* of a NAV redemption; balanced against the NAV redemption it settles. In-lane (vault-bridge). |
| M4 | part_01.rs:1574 | `apply_asset_operation` → `nav_mint_at_nav` branch | `nav_mint_at_nav` | **+** | `nav_asset.circulating_supply` **and** `max_supply` | **NAV mint lane — NOT vault-bridge deposit credit.** Mints a NAV asset up to the finalized reserve-packet `circulating_supply`. Reachable for pfUSDC (pfUSDC is a NAV asset). See §4. |
| M5 | part_02.rs:3320 | `credit_issued_asset_balance` (shared helper) | called by `apply_pftl_uniswap_primary_subscribe` (2464), `apply_pftl_uniswap_refund_source` (2701), `apply_pftl_uniswap_return_import` (2928) | **+** | `max_supply` | PFTL↔Uniswap / wrapped-NAV lanes. Generic credit of whatever `asset_id` the caller passes. These are the NAV/AMM bridge legs; net supply changes are paired with an Ethereum-side burn/mint. See §4. |
| M6 | part_03.rs:84 | `apply_issued_payment` | `issued_payment` with `from == issuer` | **+** | `max_supply` | **Generic direct issuance.** When the sender is the asset issuer, `issued_payment` mints (no debit leg). No NAV/vault-bridge guard at the execution layer. **Open direct-mint lane** — see §4. |

Every mint site does `issued_asset_supply(ledger, asset).checked_add(amount)` then a cap
comparison; the two error codes at each site are `issued_supply_overflow` (u64 wrap) and
`issued_supply_cap_exceeded` (over the applicable cap).

## 2. NON-mutation sites — read-side accumulation guards

These 7 hits sit inside the functions that **compute** supply/reservation totals by folding
over ledger vectors. They emit `issued_supply_overflow` only if a `u64` sum overflows while
*reading*. They never change state and are **out of scope** for the conservation invariant.

| Site | Function | What it sums |
| --- | --- | --- |
| part_03.rs:560 | `issued_asset_supply` | trustline balances for the asset |
| part_03.rs:572 | `issued_asset_supply` | trustline + open-escrow + open-offer-locked totals |
| part_03.rs:591 | `issued_asset_open_offer_locked_total` | open offers' `taker_gets_amount_remaining` |
| part_03.rs:609 | `issued_asset_open_escrow_total` | open escrows' `amount` |
| part_03.rs:637 | `issued_asset_reserved_total_for_account` | per-account escrow + offer reservations |
| part_03.rs:661 | `issued_asset_reserved_escrow_total_for_account` | per-account open-escrow reservations |
| part_03.rs:692 | `issued_asset_reserved_offer_total_for_account` | per-account open-offer reservations |

## 3. Burn side (net `issued_supply` −) — for invariant completeness

`issued_supply` is a balance sum, so burns are debits (no `overflow`/`cap` code). The lanes
Ghash's invariant must also cover:

| Site | Enclosing fn | Trigger op | Δ |
| --- | --- | --- | --- |
| part_02.rs:3362 | `debit_issued_asset_balance` (shared helper) | `apply_pftl_uniswap_export_debit` (2568) and callers | **−** |
| part_02.rs:3616 | `apply_vault_bridge_burn_to_redeem` | `vault_bridge_burn_to_redeem` | **−** (burns pfUSDC to open a redemption) |
| part_02.rs:2223 | `apply_pftl_uniswap_export_debit` region | uniswap export settle | **−** |
| — | `apply_asset_burn` (dispatch part_01.rs:~843, `AssetBurn`) | `asset_burn` | **−** (generic holder burn) |
| — | `apply_issued_payment` with `from != issuer` | `issued_payment` transfer | **0** (debit + credit; net supply unchanged) |

`apply_vault_bridge_redeem_settle` (part_02.rs:3622) settles a redemption and updates bucket
`outstanding`/`counted` accounting (the `Σredeemed` term); the pfUSDC was already burned at
`burn_to_redeem`.

## 4. Paths that can mint pfUSDC OUTSIDE the vault-bridge credit lane — FLAGGED

The plan requires "issuance only via vault-bridge credit ops thereafter." Today that is **not
yet true at the execution layer.** These non-vault-bridge mint surfaces are reachable for a
NAV/issued asset such as pfUSDC and need closing or an explicit registration:

- **M6 `apply_issued_payment` (part_03.rs:84), issuer-origin.** Dispatch (part_01.rs:825-841)
  routes any `issued_payment` op straight to `apply_issued_payment` with **no NAV / vault-bridge
  guard**. `apply_issued_payment` blocks only issuer→issuer (`issuer_self_payment`); an
  issuer→holder payment with `from == issuer` mints, capped solely by `max_supply`. This is a
  direct-issuer mint of pfUSDC that bypasses the vault-bridge deposit lane. **Highest-priority
  close.**
- **M4 `nav_mint_at_nav` (part_01.rs:1574).** Mints a NAV asset up to the finalized reserve
  packet `circulating_supply`. It is a legitimate NAV lane but is *not* the vault-bridge deposit
  credit; for a vault-bridge-backed asset it is a second issuance surface bounded only by the
  reserve-packet supply.
- **M5 uniswap lanes (part_02.rs:2464 / 2701 / 2928 via `credit_issued_asset_balance`).** Mint
  whatever `asset_id` they are handed; reachable for pfUSDC if a pfUSDC route is configured.
  Their net supply change is intended to be paired with an Ethereum-side burn/mint, but the
  pairing is not the vault-bridge conservation identity.
- **M3 `apply_nav_redeem_vault_bridge_settlement` (part_02.rs:569)** mints the settlement asset;
  in-lane for vault-bridge but still a distinct `checked_add` site the invariant must cover.

Recommendation for Stage 1: assert in consensus that, for a vault-bridge-profiled NAV asset,
the only admissible `issued_supply`-increasing op is the vault-bridge credit lane (M1/M2/M3),
and reject M4/M5/M6 for such assets — the plan's "issuance only via vault-bridge credit ops."

## 5. Conservation identity

Target invariant (plan Stage 1): **`issued == Σcounted_value − Σredeemed`**, per bucket and in
aggregate, must balance on every credit, impairment, and redemption. From the W6 gate money map
(`end-state-money-map-after-run4.json`), it holds atom-exact at run-4 end state:

```
issued_supply          = 270019667
counted_value          = 609999693   (active 399999793 + impaired 209999900)
counted − issued       = 339980026   == Σredeemed (differences.counted_value_minus_issued_supply)
```

i.e. `issued(270019667) == counted(609999693) − 339980026`. **Zero drift on the issued/counted
identity.** Each mint site preserves it by pairing a `+issued` with a `+Σcounted` (M1) or by
reallocating existing counted capacity (M2); each burn/settle pairs `−issued` with `+Σredeemed`.
The risk M4/M5/M6 pose is a `+issued` with **no** matching `+Σcounted` — silent arithmetic
drift, exactly what Stage 1 exists to kill.

## 6. The 20 USDC delta — reconciled (MANDATORY)

The W6 gate flagged a stray issued-vs-counted delta (plan: "the stray 0.02 delta"). At the
final run-4 state it resolves, atom-exact, to the **20.000000 USDC vault-minus-counted**
over-backing driven by the fabricated h94 synthetic. There is **no unexplained residual** —
every atom maps to a specific gate-pack row. Sources: `end-state-money-map-after-run4.json`,
`final-vault-bridge-status-after-run4.json`, `source-finality-audit-after-run4.json`, and the
clean-unwind pack `w6_addendum3_unwind_20260705T043715Z/clean-unwind/flow1-burn-to-redeem`.

**The synthetic row.** `source-finality-audit-after-run4.json`: deposit
`bed939f2…`, block `0x88a720e1…`, `20000000` atoms (20 pfUSDC), `known_synthetic_row=true`,
`evm_block_exists=false`, `evm_tx_receipt_exists=false`. Its receipt `89404a6c…` lives in the
**impaired** bucket `6318b014…` (`erc20_bridge_vault:42161:0x6a70…:0xaf88…`).

**Lifecycle (issued side nets to zero):**
1. **MINT — height 95.** `apply_vault_bridge_deposit_claim` finalized/credited the synthetic
   (`finalized_at_height=95`; receipt `89404a6c` `counted_at_height=95`; allocation `bd7eba30…`,
   purpose `vault_bridge_supply`, `20000000`, `created_at_height=95`). `issued_supply += 20000000`.
   **No real Arbitrum USDC entered the vault** (block/tx nonexistent) — the hole.
2. **IMPAIR — ~height 157→161.** Bucket `6318b014…` was impaired via a `vault_bridge_bucket_impair`
   op (final state `status="impaired"`, `impairment_factor_bps=9130`, `last_updated_height=161`;
   pre-burn unwind snapshot showed `9374` at height 157). Impairment writes the bucket's
   `counted_value` down below its `outstanding`, removing the synthetic's counted contribution.
3. **BURN — height 158.** Redemption
   `35c92ddc8291c02471f8ca0bcc63028707df7af8f23493372f868e7a643fea6b4729947b9c4ca4b4e10f84a672111338`
   (bucket `6318b014…`, `amount_atoms=20000000`, `state="settled"`, `created_at_height=158`,
   `burn_tx_id=4e71d7c389…`) via `apply_vault_bridge_burn_to_redeem`. `flow1-burn-to-redeem.json`
   proves the debit: owner `pf323c…` pfUSDC `owner_balance_before=120019874 →
   owner_balance_after=100019874`, i.e. **`issued_supply −= 20000000`.** Remediation was
   protocol-clean ("not backed by new operator USDC") — no vault USDC was drained to pay a
   deposit that never funded the vault.

Net synthetic effect on `issued_supply`: `+20000000 (h95) − 20000000 (h158) = 0`.

**Atom-level reconciliation of the 20 USDC (vault − counted).** From
`end-state-money-map-after-run4.json`:

```
vault (I-1 Arbitrum USDC)         = 629999693
counted_value                     = 609999693
vault − counted                   =  20000000   (20 USDC)  ← the delta

decompose:
  vault − outstanding_total       =        26   = nav_subscription_allocations (impaired bucket)
  outstanding_total − counted     =  19999974   = impairment write-down (impaired bucket:
                                                    outstanding 229999874 − counted 209999900)
  ----------------------------------------------
  sum                             =  20000000   = 20.000000 USDC  ✓
```

Supporting bucket arithmetic (both buckets internally consistent, atom-exact):
```
impaired 6318b014: gross 320000000 (3×100M real + 20M synthetic)
   − redemptions 90000100 (synthetic 20000000 + 70000000 + 58 + 42)
   − nav_subscription 26
   = outstanding 229999874                                    ✓
active 006c9b4e: gross 400000000 − redemptions 207 − nav 0 = outstanding 399999793  ✓
counted_total = 399999793 + 209999900 = 609999693            ✓ (matches counted_value)
```

**Reading the delta:** the synthetic contributed `+20000000` to both `issued` and the bucket's
`counted/outstanding` at mint; the burn removed it from `issued`; the impairment removed it from
aggregate `counted` (19999974 write-down) with the residual 26 atoms consumed by NAV
subscriptions. Because the fake deposit never funded the vault and the unwind drained no real
USDC, the vault retains 20.000000 real USDC above `counted` — a permanent, fully-explained
subset of the ~360 USDC total over-backing (`vault − issued = 359980026`), held report-only per
`OVERBACKING-REPORT-ONLY-PLAN.md`. **No path is left open by this delta; it is a closed,
historical impairment artifact.**

## 7. Test-coverage evidence (not mutation paths)

These `tests_parts` hits assert the cap is enforced; they are coverage, not issuance lanes:
- `crates/execution/src/lib_parts/tests_parts/part_02.rs:161` — asserts
  `receipt.code == "issued_supply_cap_exceeded"`.
- `crates/execution/src/lib_parts/tests_parts/part_01.rs:1801` — asserts
  `receipt.code == "issued_supply_cap_exceeded"`.

Gap for Ghash: current tests cover the *cap* rejection, not the `issued == Σcounted − Σredeemed`
conservation invariant across M1–M6 and the burn lanes. Stage 1 adds that.
