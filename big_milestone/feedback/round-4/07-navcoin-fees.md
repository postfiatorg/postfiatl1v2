# Feedback — Pillar 7 — Optional Deterministic Fees for NAVCoins

*Execution-hole audit · Round 4.*

> **Mean score:** 74/100 · Gemini 4 Neon 73 · Claude Opus 5.5 75
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/07-navcoin-fees.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 73/100

*Model `google/gemini-4-neon` · 18764 output tokens · `tech=15/30 econ=12/20 align=19/20 ship=13/15 read=14/15`*

#### Summary judgement

Pillar 7 is cleanly scoped, tightly aligned with every locked founder decision, and gets the core dilution accounting right (`ΔS = floor(F × S / (V - F))`, post-fee `H' = max(H, N_after)`, and counting unclaimed fee units inside circulating supply). It reads like a real engineering spec rather than bureaucratic filler, with concrete milestones and all four ship gates properly defined.

Before engineering writes consensus code, however, **seven real execution holes** (four critical, three major) must be closed. The biggest gaps are around state initialization and transitions (an unspecified initial high-water mark `H` and `h_last` can drain up to 50% of principal on epoch 1), bypasses around the 7-day notice period and strict NAV rule, a snapshot-vs-finalization supply race inside `nav_epoch_finalize`, and a cross-chain pricing mismatch between L1 post-fee NAV (`N_after`) and the Ethereum Uniswap NAV guard. None of these require changing product direction—all seven can be closed with tight, concrete consensus rules.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (15/30):** Sound core dilution math and unclaimed-bucket design, held back by four critical state-machine/cross-chain holes (H1–H4) and three major operational/arithmetic holes (H5–H7).
- **Economic & Incentive Coherence (12/20):** Holder-favorable rounding and anti-double-dip HWM math are strong, but sockpuppet attestor fee extraction, zero-notice attach/legacy-spread loopholes, and L1-to-Uniswap fee-epoch arbitrage create real exploit paths until closed.
- **Founder Decision Alignment & Zero Open Questions (19/20):** Faithfully implements every locked Pillar 7 decision (optionality, neutral framing, exact caps, Option A, no hurdle/protocol cut, zero open questions); minus one point only for the diagram's stale "checks recipient line" conflict with Pillar 8's permissionless default.
- **Actionable Milestones & 4 Ship Gates (13/15):** Clear M1–M4 build milestones and all four mandatory ship gates with concrete founder tests; needs explicit test coverage for live-attach notice rules and L1/Ethereum post-fee NAV parity.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Crisp, readable, zero claudese, good diagram and code references; minor clarity deduction for mixing whole-unit and atom-scale figures in the worked math example.

#### Execution holes

##### H1 — Unspecified initialization of high-water mark (`H`) and last accrual height (`h_last`)
- **Severity:** `critical`
- **Where:** *Detailed specification §1–2*
- **Hole:** The spec never defines how `H` and `h_last` are initialized or updated when: (a) a brand-new NAVCoin moves from `S = 0` to `S > 0`, (b) `nav_fee_schedule_attach` is called on a live NAVCoin (such as A666) whose `nav_per_unit` is already above zero, (c) `hwm_rate_bps` is raised from `0` to `> 0` after past appreciation, or (d) `S` drops to `0` and later restarts.
- **Why it breaks execution:** If `H` defaults to `0`, the first finalized epoch with `hwm_rate_bps = 5,000` treats 100% of NAV as gain and mints up to 50% of all holder assets to the fee recipient. On a live asset trading at `$2.50` (or an asset whose `hwm_rate_bps` was `0` while NAV doubled), failing to anchor `H` at current NAV retroactively charges gain fees on pre-fee history. Likewise, if `h_last` defaults to `0` when attaching at block `10,000,000`, `Δh` immediately clamps to `accrual_cap_blocks` and charges a lump-sum time fee on the first epoch.
- **Minimal plain-English fix:** Lock four explicit consensus rules: (1) on `attach` with `S > 0`, initialize `H = current_nav_per_unit` and `h_last = last_finalized_height` (or current height if none); (2) on a new asset (`S == 0`), charge `0` fee on the first finalized epoch with `S > 0` and set `H = N_after`, `h_last = h`; (3) keep updating `H = max(H, N_after)` even while `hwm_rate_bps == 0`, and set `H = max(H, current_nav_per_unit)` whenever `hwm_rate_bps` moves from `0` to `> 0`; (4) reset `H = 0` if `S` reaches `0` so a restarted asset bootstraps cleanly.

##### H2 — Notice-period, legacy-spread, and exit-capacity bypasses
- **Severity:** `critical`
- **Where:** *Locked-in design decisions* & *Detailed specification §2*
- **Hole:** Four loopholes undermine the 7-day fee-increase protection:
  1. `nav_fee_schedule_attach` does not require a 7-day pending notice when `S > 0`.
  2. Existing legacy spread-update paths (`issue_multiplier_bps`, `redeem_multiplier_bps`) and `FxFixPacketV1.fee_bps` are not explicitly blocked or globally capped when no schedule is attached (or when a schedule is attached/locked).
  3. Reducing `increase_notice_blocks` or raising `accrual_cap_blocks` is not explicitly treated as a fee-increasing change, nor are overlapping pending updates defined.
  4. The rule that "routes cannot cut exit capacity" during a pending hike has no specified on-chain enforcement mechanism.
- **Why it breaks execution:** An issuer can launch a "zero-fee" NAVCoin, attract deposits, and immediately call `nav_fee_schedule_attach` (or mutate legacy spreads / set an uncapped FX fix fee) with zero notice; or shrink a 30-day notice period to 7 days immediately; or drain off-chain `pfUSDC` redemption liquidity during the 7-day countdown so holders cannot actually exit before the hike takes effect.
- **Minimal plain-English fix:**
  - Require `nav_fee_schedule_attach` to enter the `>= 7`-day pending queue whenever `S > 0` and any rate is non-zero or exceeds existing spreads (instant only when `S == 0` or zero-rate/matching existing spreads).
  - Once a schedule is attached, reject direct legacy spread mutations outside `nav_fee_schedule_set`; enforce the `100 bps` `FxFixPacketV1.fee_bps` protocol cap even when no schedule is attached, and always show non-zero legacy spreads on the explorer.
  - Classify lowering `increase_notice_blocks` or raising `accrual_cap_blocks` as an increase subject to the *current* `increase_notice_blocks`, and reject a new pending update until the existing one is cancelled or applied.
  - Specify the exact on-chain freeze during a pending hike (block downward edits to on-chain redemption limits/caps and pause flags) and honestly note that off-chain broker liquidity is governed by existing deadman/exit guarantees rather than L1 consensus.

##### H3 — Snapshot-vs-finalization supply race and non-scheduled hidden-fee loophole
- **Severity:** `critical`
- **Where:** *Detailed specification §1–2*
- **Hole:** First, `nav_epoch_finalize` combines reserve value `V` (measured at snapshot height `h_snap`) with circulating supply `S`, without binding `S` to the exact on-chain supply (including `unclaimed_fee_units`) at `h_snap`, specifying whether `Δh` is measured from `h_snap` or finalization height `h_fin`, or defining how mints/redemptions between `h_snap` and `h_fin` are reconciled. Second, strict `nav_per_unit == floor(V × U / S)` is only enforced "for any NAVCoin with an attached schedule."
- **Why it breaks execution:** If redemptions occur between `h_snap` and `h_fin` and finalization divides stale `V(h_snap)` by the smaller live `S_fin`, `N_m` spikes artificially and triggers up to a 50% HWM gain fee on phantom gains (conversely, mints artificially crush NAV). Measuring `Δh` from `h_fin` instead of `h_snap` also lets an issuer delay finalizing a stale packet to collect extra time fees. Meanwhile, exempting non-scheduled NAVCoins from strict `floor(V × U / S)` lets any issuer display "0% fee" while quietly skimming hidden fees by understating `nav_per_unit` on non-SP1 proof profiles.
- **Minimal plain-English fix:** Bind every reserve packet to an explicit strictly-increasing `as_of_height` (`h_snap`), verify in consensus that `S_packet` equals on-chain `circulating_supply` (including `unclaimed_fee_units`) at `h_snap` (or reject overlapping mints/redemptions across unfinalized snapshots), compute `Δh = min(h_snap - h_last_snap, accrual_cap_blocks)`, distinguish pre-fee packet validation (`floor(V × U / S_snap)`) from stored post-fee `N_after = floor(V × U / (S_snap + ΔS))`, and enforce strict `floor(V × U / S)` on **all** new NAVCoins across all proof profiles regardless of whether a fee schedule is attached.

##### H4 — L1 post-fee `N_after` vs Ethereum Uniswap NAV guard divergence
- **Severity:** `critical`
- **Where:** *How deterministic fee accrual works* & *Detailed specification §2*
- **Hole:** On fee-accruing epochs, `nav_epoch_finalize` mints `ΔS` units on `postfiatl1v2` and updates L1 `nav_per_unit` to post-fee `N_after = floor(V × U / (S + ΔS))`, while preserving existing Ethereum `policy_hash`. The spec does not specify how the Ethereum Uniswap ERC-20 NAV guard receives post-fee `N_after` instead of the reserve packet's pre-fee `N_before = floor(V × U / S)`.
- **Why it breaks execution:** If the Ethereum NAV guard validates raw reserve packets (`N_before`) while `postfiatl1v2` primary routes price at `N_after`, every fee-accruing epoch opens an immediate risk-free arbitrage gap between Uniswap and L1 routes (potentially several percent on a high-water-mark epoch), draining pool or primary-route liquidity.
- **Minimal plain-English fix:** Specify that the Ethereum NAV guard / bridge relay consumes the finalized L1 epoch commitment carrying `N_after` (from `NavFeeAccrualV1` / finalized epoch state) rather than pre-fee reserve-packet NAV, and add a concrete M2 test verifying L1 and Uniswap NAV guard parity on a fee-accruing epoch without breaking `policy_hash`.

##### H5 — Unenforced "non-issuer attestor" rule allows sockpuppet HWM fee extraction
- **Severity:** `major`
- **Where:** *Locked-in design decisions* (*Attested reserves & v1 scope*)
- **Hole:** The spec allows attested reserves (e.g., Hyperliquid positions) to accrue fees on total `V` "only when backed by a non-issuer attestor," but specifies no on-chain attestor registry, identity check, or change-notice rule.
- **Why it breaks execution:** A trivial `attestor_key != issuer_key` check is trivially bypassed with a second issuer-controlled keypair. A malicious issuer can appoint a sockpuppet attestor, report an inflated `V` for one epoch to mint massive HWM fee units (`ΔS`), claim and sell those units into the `pfUSDC` route or Uniswap pool, and report true `V` on the next epoch.
- **Minimal plain-English fix:** Define how consensus verifies a non-issuer attestor—require the attestor key to belong to a governance/validator-ratified attestor list (or an immutable/7-day-notice-gated attestor identity bound on-chain via Pillar 6/8 and displayed prominently on the explorer alongside the proven/attested reserve split).

##### H6 — Ambiguous `nav_fee_claim` ownership, recipient rotation, `lock_schedule` trap, and Pillar 8 trust-line conflict
- **Severity:** `major`
- **Where:** *How deterministic fee accrual works* & *Detailed specification §2*
- **Hole:** Three tightly related recipient/claiming bugs:
  1. `recipient` changes apply immediately with zero notice, yet the spec does not specify who signs `nav_fee_claim` (`recipient` vs `issuer`) or whether already-accrued `unclaimed_fee_units` follow the new `recipient` or stay owed to the prior `recipient`.
  2. It is unspecified whether the one-way `lock_schedule` flag also permanently freezes `recipient`.
  3. The flow diagram states `nav_fee_claim` "checks recipient line & Pillar 8 policy," conflicting with Pillar 8's locked rule that trust lines are optional by default.
- **Why it breaks execution:** Instant recipient swaps can redirect already-earned fees away from an external fee recipient (or let a compromised issuer key sweep unclaimed fees); permanently freezing `recipient` under `lock_schedule` strands all future fees if the recipient key is lost or compromised; and requiring a trust line breaks `nav_fee_claim` on default permissionless NAVCoins.
- **Minimal plain-English fix:** Track unclaimed fee balances per `(asset_id, recipient)` (so rotating `recipient` only affects future epochs and prior recipients can still claim earned units), let `recipient` sign `nav_fee_claim` (or allow anyone to push to `recipient`), explicitly exclude `recipient` rotation from `lock_schedule` (keep rates/caps/notice immutable, allow recipient rotation with event visibility), and replace "checks recipient line" with a call to Pillar 8's single `asset_gate(asset_id, recipient)` helper.

##### H7 — Atom-scale ambiguity, `u128` silent-zero overflow, and unrecorded `blocks_per_year`
- **Severity:** `major`
- **Where:** *Locked-in design decisions* & *Detailed specification §1*
- **Hole:** The worked example mixes whole-unit numbers (`V = 1,100,000`, `F_m = 110`, `F_p = 9,989`) with fractional atom output (`ΔS = 9,265.979203`) without formally stating the exact integer domains of `V`, `S`, `U`, `H`, and `N`. Worse, three-way checked `u128` multiplications (`g × S × p`, `V × m × Δh`) can overflow `u128::MAX` (`~3.4 × 10^38`) for high-supply/high-precision assets and silently zero out fees under the fail-safe rule. Finally, `blocks_per_year` is referenced without stating its value or recording it per epoch.
- **Why it breaks execution:** Rust consensus and Python SDK (`recompute_fees`) will diverge on unit scaling and overflow behavior (Python `int` never overflows while Rust `u128` silently returns zero), and any future protocol update to `blocks_per_year` (e.g., if Cobalt block cadence shifts) will break historical `recompute_fees` replay.
- **Minimal plain-English fix:** State exact atom domains explicitly (`V`, `F_m`, `F_p`, `F` in quote atoms; `S`, `ΔS` in base atoms; `U = 10^asset_precision`; `N`, `H` in quote atoms per `U` base atoms), use widening `U256` multiply-divide for intermediate products so valid assets never silently zero out, and store the exact `blocks_per_year`, `Δh`, `V`, `S`, and `H` snapshot inside each `NavFeeAccrualV1` record (and expose `blocks_per_year` via `nav_fee_schedule` RPC) so every historical epoch is permanently self-verifying.

## Claude Opus 5.5 — 75/100

*Model `anthropic/claude-opus-5.5` · 7258 output tokens · `tech=18/30 econ=13/20 align=19/20 ship=13/15 read=12/15`*

#### Summary judgement

This is a strong, tightly scoped pillar. It matches the locked decisions exactly:
- Fees are optional, with neutral wording.
- The caps are 5%/yr annual, 50% HWM (high-water-mark) gain, 5% spread and 1% fix, plus a 7-day minimum notice on increases.
- It uses Option A (HWM checked per finalized epoch), with no hurdle rate and no protocol cut.
- Hyperliquid and the FX Fix are correctly pushed out to the README.

The fee math is internally consistent. I re-derived the worked example: the dilution formula makes the minted units worth exactly `F` at post-fee NAV, and `N_after = 1.089901` checks out.

The holes sit around the edges of the math rather than in it:
- **High-water-mark state.** How it starts on live assets like A666, and whether cancelling and re-setting a schedule can reset it.
- **Who picks finalization timing.** An issuer who chooses when epochs finalize turns the HWM fee into a free option on volatility.
- **Ambiguous notice rules.** "What counts as an increase" is undefined, and attaching a schedule to a live asset may skip the notice entirely.
- **Assets without a schedule.** The "no hidden fees" and "no silent hikes" guarantees cover only NAVCoins that opt into a schedule. A fee-free NAVCoin can still understate NAV on non-SP1 profiles or raise its spread instantly.
- **Cross-chain NAV.** Fee dilution on L1 is not shown to reach the live Ethereum NAV guard.

None of these require redesign. Each is a one-sentence rule that engineering will otherwise guess at.

#### Rubric breakdown

- **Tech (18/30):** The integer math, overflow and fail-safe handling, and unclaimed-in-supply design are good. However, HWM initialization and persistence, the accrual window straddling a rate change, and cross-chain NAV propagation are unspecified, and each produces a wrong-fee or arbitrage bug.
- **Econ (13/20):** The caps and dilution are coherent. But issuer-controlled finalize timing games the HWM fee, non-schedule assets keep hidden-fee and instant-spread-hike paths, and the "recipient ≠ issuer" rule is cosmetic.
- **Align (19/20):** Every locked decision is honored and there are no open questions. It loses one point only because "no hidden fees" is enforced only for opted-in assets, which weakens the spirit of transparent fees.
- **Ship (13/15):** M1–M4 are concrete and all 4 ship gates are present. The user test is strong. M1 is overloaded (state, ops, math, strict check and RPCs in one milestone) and should be split.
- **Read (12/15):** Mostly plain English, and the math block is necessary. Some dense parentheticals ("cash-in exit capacity", "deadman halts dealing") go unexplained. The worked example mixes display units and atoms without saying so.

#### Execution holes

**H1**
- **Severity:** critical
- **Where:** §2 Strict NAV derivation; "What exists today" hidden-fee gap
- **Hole:** The `floor(V × U / S)` check is enforced only "for any NAVCoin with an attached schedule". Fee-free NAVCoins on non-SP1 proof profiles can still understate NAV.
- **Why it breaks execution:** An issuer avoids all disclosure by not attaching a schedule. It then mints to itself at an understated NAV, or pays redeemers less. This is exactly the hidden fee the pillar claims to make impossible. The "zero fee clutter" path becomes the "hidden fee" path.
- **Minimal plain-English fix:** Enforce the strict NAV equality for every NAVCoin on every proof profile, schedule or not. Existing legacy assets switch over at a single activation height.

**H2**
- **Severity:** major
- **Where:** §1 Step 2; §2 `nav_fee_schedule_attach`
- **Hole:** The starting high-water mark `H` is undefined when a schedule is attached to a live asset. It is also unclear whether `H` survives `cancel` followed by a new `set`.
- **Why it breaks execution:** Two failures follow.
  - If `H` starts at the initial 1.0 or at zero, A666 holders are charged up to 50% on gains made before fees existed.
  - After a drawdown, the creator can cancel and re-attach to reset `H` lower, then collect the gain fee again on the recovery.
- **Minimal plain-English fix:**
  - On first attach, set `H` to the current `nav_per_unit`.
  - Store `H` on the asset, not the schedule, and never lower it, including across cancel and re-attach.

**H3**
- **Severity:** major
- **Where:** Locked design decisions, Option A; mermaid `nav_epoch_finalize`
- **Hole:** The spec never says who triggers finalization or whether epochs can be skipped. The per-epoch HWM fee crystallizes on whatever NAV is finalized.
- **Why it breaks execution:** An issuer who controls reserve submission and finalize timing can:
  - finalize only at local peaks, such as a mark spike on attested Hyperliquid positions;
  - collect the 50% gain fee;
  - let NAV fall back.

  Repeating this harvests fees on volatility with zero net gain to holders.
- **Minimal plain-English fix:** Fee accrual happens only on a fixed epoch cadence set at registration. Missed epochs accrue no gain fee, and the issuer cannot pick finalization moments. Optionally, the gain fee applies only to the proven portion of `V` unless the attestor is non-issuer, which is already required.

**H4**
- **Severity:** major
- **Where:** Locked decisions, protection during pending hikes; §2 operations
- **Hole:** "Any change that could raise what holders pay" is not defined field by field.
- **Why it breaks execution:** Several cases are ambiguous:
  - Raising `accrual_cap_blocks` or switching HWM from 0 to non-zero.
  - Lowering the annual rate while raising the HWM rate in one transaction.
  - Attaching a schedule to a live asset with holders, which is an increase from zero.

  Implementers will disagree, and validators running different interpretations fork or let hikes skip the 7-day notice.
- **Minimal plain-English fix:** Compare old and new values field by field. If any rate, spread, fix cap or accrual cap rises, or a schedule is attached to an asset with non-zero supply, the whole update waits out the notice. It applies only if every field is a decrease or unchanged.

**H5**
- **Severity:** major
- **Where:** "What exists today" (spreads update immediately); §2 spreads sync
- **Hole:** NAVCoins without a schedule keep today's instant spread updates through the Task Node routes.
- **Why it breaks execution:** A "fee-free" NAVCoin can move its redeem spread from 0 to 500 bps in the same block as a large redemption. That is a silent 5% fee hike on exit, bypassing the 7-day protection that opted-in assets get.
- **Minimal plain-English fix:** Spread increases on any NAVCoin require the same minimum 7-day notice. Store spreads in the same notice-gated field whether or not a full schedule is attached.

**H6**
- **Severity:** major
- **Where:** §1 Step 1 (`Δh`); mermaid "first finalization after effective height"
- **Hole:** A pending rate change activates at the first finalization after its effective height, but `Δh` covers the whole span since the last accrual. Separately, `h_last` on first attach is unspecified.
- **Why it breaks execution:** Two over-charges follow.
  - The new, higher rate is charged retroactively on blocks that elapsed before the effective height, which defeats the notice period.
  - On first attach, `h_last` defaults to 0 or registration height, so the first finalization charges up to `accrual_cap_blocks` of fees on time when no schedule existed.
- **Minimal plain-English fix:**
  - Set `h_last` to the activation height.
  - Split `Δh` at any effective height inside the window: old rate before it, new rate after.

**H7**
- **Severity:** major
- **Where:** §1 Step 4 / stored `nav_per_unit`; §2 `attach` "without breaking Ethereum `policy_hash`"
- **Hole:** Fees dilute NAV and mint new units on L1. The spec does not say how the post-fee `nav_per_unit` and the new supply reach the live Uniswap ERC-20 NAV guard (B24) or the Solana routes.
- **Why it breaks execution:** If the Ethereum guard keeps the pre-fee NAV, arbitrageurs redeem or exit on Ethereum at a stale, higher NAV every epoch. That drains reserves from L1 holders. Keeping `policy_hash` unchanged suggests the Ethereum side is unaware of fees.
- **Minimal plain-English fix:**
  - Cross-chain routes must read the post-fee NAV from the finalized epoch.
  - Add an M2 test showing the Ethereum guard rejects any pre-fee NAV after an accrual.

**H8**
- **Severity:** minor
- **Where:** §2 `recipient` (must not equal issuer); recipient change "applies immediately"
- **Hole:** The issuer can name a second account it controls, so the "≠ issuer" rule is cosmetic. It is also undefined whether an immediate recipient change redirects already-accrued unclaimed units, and whether `locked` freezes the recipient.
- **Why it breaks execution:** The claim logic will be implemented inconsistently. A compromised issuer key can redirect accrued but unclaimed fees instantly.
- **Minimal plain-English fix:**
  - Drop or reword the "≠ issuer" rule as a display label.
  - Unclaimed units already accrued stay with the recipient active at the time they accrued.
  - A `locked` schedule also locks the recipient.

**H9**
- **Severity:** minor
- **Where:** §2 `FxFixPacketV1.fee_bps must respect fix_fee_cap_bps`
- **Hole:** The 1% fix cap is enforced only through the optional schedule field.
- **Why it breaks execution:** A NAVCoin with no schedule has no `fix_fee_cap_bps`. Its FX fix fee is then either uncapped or rejected, depending on how the code treats a missing field.
- **Minimal plain-English fix:** Enforce a global 100 bps protocol cap on `FxFixPacketV1.fee_bps` for every asset. The schedule can only set a lower cap.
