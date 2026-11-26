# Changelog — Pillar 7 — Transparent Fees for NAVCoins

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/07-navcoin-fees.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/07-navcoin-fees.md).

| Item | Change |
|---|---|
| Gemini P1 | Options A and B specified exactly; open question 1 |
| Gemini P2 | Fee math: units, order, per-unit HWM, `F × S / (V − F)` mint, rounding, initial HWM; R12 |
| Gemini P3 | Δh from heights, capped; notice in blocks; acceptance 6; open question 3 |
| Gemini P4 | R9 unclaimed balance and claim; edge cases never fail; R10. In code, the mint cap is the packet's supply plus `max_supply` |
| Gemini P5 | R3 signer; R4 classification, notice reduction counts as an increase, one pending slot; open question 10 |
| Gemini P6 | R5 mapping and migration; R11. Partly disputed: `NAVGuardHook` records observations and does not price at NAV |
| Gemini P7 | `fix_fee_cap_bps` (R1, R6); R15 includes schedule transactions |
| Gemini P8 | "Deduct from reserves" removed, with the reason |
| Gemini P9 | R17–R19; fee column in the five-parts table; open questions 4 and 13 |
| Gemini P10 | Pillar 2 optional via `disclosure_ref` (its R1 already lists `pf.navcoin.fee.*`) |
| Gemini P11 | Ten acceptance criteria covering each failure mode |
| Opus P1 | `V`, `N`, `H` defined; performance fee after management |
| Opus P2 | Mint sizing fixed; example checked to the atom |
| Opus P3 | Δh, cap, catch-up and deadman; open question 3 |
| Opus P4 | Open question 4; R14 shows the proof split; R8 |
| Opus P5 | Option B for late entrants; per-holder marks ruled out; open question 5 |
| Opus P6 | R11: routes read the post-fee NAV; fee units export normally |
| Opus P7 | R5 migration; acceptance 8 |
| Opus P8 | R4 mixed and non-rate changes; open question 7 |
| Opus P9 | Spreads under notice (R5); open question 6 |
| Opus P10 | Acceptance 2 uses a drawdown path and works for both options |
| Opus P11 | R16 and "Why fees are paid in units" |
| Opus P12 | Fix fee cap; open question 8 |
| Opus P13 | R12 schema, R15 surfaces and RPC |
| Opus P14 | Open question 11 |
| Opus P15 | Checked. Fixed route op names (`PftlUniswap*`), the spread destination, the NAV op execution file, and "operator-only" as a release rule |
| Pillar 8 cross-check | R9: rules for the recipient's credential and line on gated NAVCoins; fee claims are not FastPay. R6/R15: the fix is a private swap, so the fix fee shows as a rate. Open question 6: de-authorised holders cannot exit |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/07-navcoin-fees.md).

| Item | Change |
|---|---|
| Gemini P1 | Option B rewritten: `B = V_B` at crystallisation; one joint mint priced against `V_B − F_m − P`; management mint priced net of `P`; `K` removed, so an exit's liability share becomes fee units at once (no `0/0`, no `V − K` underflow); `V_B` scaling keeps headroom units out until minted. Table now states the pooled-mark effect on underwater entries. Example, invariants and acceptance 10 added, checked by simulation. "Recommend Option A": disputed, founder decision (open question 1) |
| Gemini P2 | R3: attach on a live asset must equal what holders pay today; anything higher is pending under notice. `H` (and Option B's `B`) persists for the asset's life and tracks highs while `p = 0`; first finalisation sets `H = N_after`. Acceptance 2 and 5 |
| Gemini P3 | R10: supply check at submission only, so window dealing cannot block finalisation; snapshot rule stated. R9: fee lane is a custody lane counted by `issued_asset_supply`; claim is a transfer. R11: packet `nav_per_unit` untouched. Partly disputed: routes and mint/exit already read `NavTrackedAsset.nav_per_unit`, not the packet (verified in `nav_vault_asset_execution.rs`) |
| Gemini P4 | Exact 256-bit intermediates; precision 18 and near-`u64` vectors in acceptance 1. Partly disputed: `S` is `u64`, so a 10^24-atom supply cannot exist, but `g × S × p` can still exceed `u128` |
| Gemini P5 | R4: notice counts live blocks only (halts and deadman pause it). R2: protocol maximum on `accrual_cap_blocks`, minimum Option B interval. Open question 3 adds live-block accrual |
| Gemini P6 | R5 binds every on-chain spread, including the private primary routes (same policy); direct mint/exit carry no on-chain spread (verified) and the Fees card says so; `spread_recipient` is a schedule field |
| Gemini P7 | R14 issuer view (attach, set, cancel, claim); R17 scope. Recommended defaults: disputed, founder decisions; replaced by a build order and mechanisms that work under each option. Ownership stays open question 12 |
| Gemini P8 | Acceptance 3 limited to performance fees, with the management bound; clamp order defined (management first) |
| Opus P1 | Snapshot rule, R10 submit-only check, Option B fee-state snapshot; acceptance 1 and 4 test window dealing |
| Opus P2 | Comparison row on the issuer's choice of heights; open question 1; acceptance 6 covers Option B. A minimum interval needs B's provisional accrual to stay fair at exit, so it lives in Option B |
| Opus P3 | "Cannot be sped up by the issuer"; Task Node shows the realised rate; open question 3 |
| Opus P4 | Option B example, invariants, acceptance 10 |
| Opus P5 | R8: typed, disclosed deduction field instead of a bare ban |
| Opus P6 | Open question 4 (designed to work under each option): fee-unit exemption and end-of-interval versus average `V` |
| Opus P7 | R5: epoch advance above active spreads rejected; re-pin rules after decreases and increases |
| Opus P8 | R4: live-block notice clock and exits on old terms are requirements. Capacity freeze and gated holders stay open question 6 |
| Opus P9 | Open question 4 kept; Hyperliquid NAVCoin can launch with `p = 0` at Step 1. Default: disputed, founder decision |
| Opus P10 | Build order (Step 1/2/3) with the questions each step waits on |
| Opus P11 | R1 says the rule is an execution constraint, not a protection |
| Opus P12 | R17 publishes a `pf.navcoin.fee.*` message once Pillar 2 ships; optional Pillar 1 dependency |
| Opus P13 | Open question 9 notes `max_supply` is fixed at `asset_create`, so the raise-the-cap incentive does not exist (verified); live-block accrual in open question 3; Fees card shows every field |
| Own check | Management remainder carried (R7, R12): route NAVCoins must finalise every 100 blocks (`MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS`), so small fees would round to zero. Corrected "routes price from the pinned packet" to `NavTrackedAsset.nav_per_unit`. Open questions 7 and 10 merged; later questions renumbered |
| Length | Page grew about 27%: the Option B rewrite, attach, notice, snapshot and issuer-view requirements and the build order. Redundant text cut elsewhere |
