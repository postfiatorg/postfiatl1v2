# Feedback — Pillar 7 — Transparent Fees for NAVCoins

*Review round 2.*

> **Mean score:** 76/100 · Gemini 4 Neon 74 · Claude Opus 5.5 78
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/07-navcoin-fees.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

> **Note:** Gemini's first call for this page hit the 32,000-token output cap and returned only 244 characters (score 77, no reasoning). It was re-run once with a 96,000-token cap; the complete answer is below. The truncated response is kept in `raw/07-navcoin-fees--gemini-4-neon.truncated.json`.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 32524 output tokens*

#### Summary judgement

This is a well-grounded, thoughtfully structured specification that clearly benefited from a first revision pass. It anchors every design decision in real files across `postfiatl1v2` and `tasknode`, cleanly separates primary-market spreads (settled in cash) from ongoing management/performance fees (minted in units), correctly uses block heights rather than nonexistent header timestamps, and avoids direct-credit failures inside epoch finalisation via an unclaimed fee balance plus `nav_fee_claim`. Option A's integer fee math is accurate and verified to the atom.

However, the spec is not yet safe to hand directly to implementation. **Option B's math contains multiple fatal bugs** (most critically, resetting `B = V - F_p` at crystallisation causes phantom performance fees on flat NAV in subsequent epochs, and full exits brick accrued `K`). Beyond Option B, there are **critical protocol and security gaps**: `nav_fee_schedule_attach` has no guard against bypassing the notice period on live assets with `S > 0`; cycling `p -> 0 -> p` can wipe out the High-Water Mark; R10's strict supply check creates a proving-window DoS vector when mints occur between reserve observation and finalisation; R8/R11 do not clarify how storing `N_after` avoids breaking `NavReservePacket` hashing; and checked `u128` intermediates silently zero fees on 18-decimal or high-supply assets. Finally, Task Node's issuer UI for setting fees is missing from the technical requirements despite being central to the founder's story, and 13 open questions have no recommended defaults.

#### Correspondence to the founder's intention

The sub-page aligns closely with **Deliverable 7** and honestly engages with the founder's broader NAVCoin narrative:

- **Core Deliverable 7 coverage (strong):** The spec directly solves every point in the founder's user story—setting deterministic management and performance fees, representing fee types and caps on chain, disclosing fees in Task Node, and surfacing schedule changes, spreads, fix rates, and per-epoch accruals on the block explorer and via RPC.
- **Broader NAVCoin primitive (good, with an ownership loose end):** The spec maps the founder's five NAVCoin elements (Uniswap wrapper, on-chain PFTL asset, USDC→pfUSDC bridge, wrapper→reserves exit, and London-style FX fix) to live code and attaches fees to each, and it adds the requested NAVCoin SDK (`R17`–`R18`) and website guide (`R19`). It is appropriately candid that Hyperliquid-on-Solana tracking and scheduled fix settlement are not built yet (Open Question 13), though leaving both unowned across the planning doc risks dropping what the founder called a "very near term thing."
- **Missing issuer UI in requirements:** The founder explicitly complained there is "no way to display a fee if there is one, set the type of fees etc," and Augmented Story step 1 mentions creating/setting fees in the "SDK or Task Node issuer view." Yet `R14` only specifies the read-only **Fees** card, and `R5` only mentions migrating the existing cash-in spread control. Setting/editing the full fee schedule, cancelling a pending schedule, and claiming fees from Task Node's issuer view (`issuer/index.jsx` and `server/l1/navcoin-routes.js`) are omitted from `R1`–`R19`.
- **Decision overhead:** Asking the founder 13 open questions without providing a recommended default for any of them stalls engineering. Most of these (especially Option A as v1 default, protocol caps, `accrual_cap_blocks`, recipient change notice, and `max_supply` clipping) should carry explicit defaults that ship unless the founder overrides them.

#### Technical robustness

**What works well:**
- **Option A math:** Management fee before performance fee, dilution-adjusted single mint `ΔS = floor(F × S / (V − F))`, conservative floor rounding everywhere, and the worked example all check out to the exact atom.
- **No-fail finalisation + unclaimed escrow (`R9`):** Separating accrual from `nav_fee_claim` cleanly handles issuer-credit restrictions, frozen trust lines, and Pillar 8 credential revocation without stalling NAV epochs.
- **Height-based clock and cross-pillar hooks:** Avoiding nonexistent timestamps, integrating Pillar 8 credential checks at attach/set/claim, and making Pillar 2 `disclosure_ref` optional are all sound engineering choices.

**Where technical robustness breaks down:**
1. **Option B math is broken in four distinct ways (see P1):** `B = V - F_p` at crystallisation understates `B` because fees are paid in units rather than withdrawn from `V`, creating infinite phantom performance fees on flat NAV; `ΔS_m` ignores `P` (and `F_p` on crystallisation), diluting management fees; total exit (`S = 0`) leaves realised `K` unmintable (`0/0`); and underwater entries, `F_m` mints, or `K` crystallisations distort per-unit HWM.
2. **Supply reconciliation and proving-window DoS (`R10`, see P3):** Requiring `packet.circulating_supply >= chain_valid_supply` without accounting for observation height `h_obs` vs finalisation height `h_finalize` means any user minting while a reserve proof is in flight causes the packet to be rejected. Conversely, redemptions during the proving window leave `packet.circulating_supply > chain_issued_supply`, causing `NavTrackedAsset.circulating_supply` and ledger `issued_supply` to diverge.
3. **Packet immutability vs post-fee NAV (`R8`/`R11`, see P3):** `R8` validates `packet.nav_per_unit == floor(V × U / S)` (pre-fee), while `R11` says "The stored `nav_per_unit` is `N_after`. Every route and every mint and exit at NAV already reads it." Overwriting `nav_per_unit` inside a hashed/proven `NavReservePacket` would break `packet_hash` integrity; storing it separately requires updating every route and mint/redeem reader.
4. **`u128` overflow silently zeroes fees (see P4):** Routine 18-decimal (ERC-20-aligned) or high-supply assets overflow `u128` in `g × S × p` and `F × S`, triggering the "overflow: no fee" fallback.
5. **Underspecified `max_supply` clamping (see P8):** When `S + ΔS > max_supply`, the split between `mgmt_fee_units` and `perf_fee_units` and the effect on `H'` are unspecified, risking Rust/Python divergence under AC 1.

#### Economic coherence

**What works well:**
- Minting units atomically into supply to pay ongoing fees (rather than trying to deduct off-chain reserves) is the right economic primitive for off-chain-backed NAVCoins.
- Keeping spreads in the settlement asset ("non-NAV fee custody") prevents spread revenue from contaminating holder NAV.
- Charging a rate-change interval at `min(old_rate, new_rate)` prevents retroactive fee hikes.
- Requiring `nav_per_unit == floor(V × U / S)` across all proof profiles (`R8`) closes the "hidden fee via understated NAV" loophole.

**Where economic coherence breaks down:**
1. **Option B contradicts its own fairness claim (see P1):** The comparison table claims Option B's late entrant "does not dilute others." In reality, entering underwater (`N < B/S`) with `B += entry_base_value` lowers aggregate per-unit HWM `B/S`, forcing underwater original holders to pay performance fees before recovering to their original HWM.
2. **Notice-period bypass via `nav_fee_schedule_attach` (see P2):** Nothing stops an issuer of a live NAVCoin with `S > 0` (including B24/A666) from calling `nav_fee_schedule_attach` with maximum management and performance fees and zero notice.
3. **High-water mark wipeout via `p -> 0 -> p` (see P2):** Because reducing `p` to `0` is a pure decrease (immediate) and "`H` starts at `N` on the first finalisation with `S > 0` after a schedule with `p > 0` activates," an issuer in a deep drawdown can reset `H` down to depressed NAV by cycling `p` to `0` and back up.
4. **Notice-period neutralization via `nav_halt` or deadman stall, plus uncapped `accrual_cap_blocks` (see P5):** An issuer can set a fee increase, halt dealing (or stop proving reserves until the deadman trips) for the entire notice window so holders cannot exit at NAV, and then finalise at the new rate. Meanwhile, `R2` caps rates but places no protocol maximum on `accrual_cap_blocks`, allowing arbitrary management-fee accrual during downtime.
5. **Route spread/recipient arbitrage (`R5`, see P6):** `R5` binds spreads to `PftlUniswapPrimaryMarketPolicyV2`, leaving `NavMintAtNav`, `NavRedeemAtNav`, private primary routes, and `spread_recipient` unbound or ambiguous.

#### Problems

##### P1 — Option B math has four critical accounting bugs and contradicts its own comparison table
- **Severity:** critical
- **Where:** `## Fee math and economics` → `### Entering and exiting between finalisations` (Option B table and comparison table)
- **What is wrong:**
  1. **Phantom fees on flat NAV after crystallisation:** At crystallisation, the rule sets `B = V − F_p` (if there was a gain) while paying `F_p` by *minting units* (`ΔS`), leaving total reserves equal to `V`. Because `B` is set below `V` by `F_p`, the very next finalisation with zero market gain computes `V − K − B = F_p > 0` and accrues another `F_p × p` performance fee—repeating geometrically on flat NAV. (Example: `V = 390, S = 200, F_p = 10` → `ΔS = 5.263`, `S' = 205.263`, `N = 1.90`, yet `B = 380` gives displayed mark `1.8513 < 1.90` and immediate phantom gain of `10`.)
  2. **Management fee mint ignores `P` and `F_p`:** On non-crystallisation epochs, `ΔS_m = floor(F_m × S / (V − K − F_m))` values the fee units without subtracting `P`, so at `N = (V − K − P) / S'` the minted management fee units are immediately worth less than `F_m`. On crystallisation epochs, sequential minting (with `S` ambiguously used instead of `S'` in `floor(F_p × S / (V − F_p))`) further dilutes `ΔS_m`.
  3. **Full exit (`S = 0`) bricks `K` and crashes underflow `V − K`:** If all holders exit (`y = S`), `S = 0` while `V = K > 0`. At crystallisation `floor(F_p × S / (V − F_p))` becomes `0 / 0`, and the `S = 0` edge-case rule drops `K` permanently, stranding reserves in `V`. Likewise, if reserve value drops below `K` (`V <= K`), unsigned subtraction `V − K` underflows.
  4. **Underwater entries, `F_m` mints, and `K` crystallisations lower per-unit HWM:** Adding `B += entry_base_value` when `N < B / S` lowers `B / S`. On recovery (while still below original HWM), `V − B > 0` charges a performance fee that dilutes original underwater holders—directly contradicting the comparison table's claim that Option B's late entrant "does not dilute others." The same HWM drop happens when `F_m` is minted underwater (`B += F_m`) or `K` is crystallised with `P = 0` (`B` unchanged while `S` grows).
- **Why it matters:** If the founder selects Option B in Open Question 1, implementing this table will silently overcharge holders on flat markets, underpay management fees, break on total redemptions, and violate the HWM invariant.
- **How an agent should investigate:** Write a small Python simulation testing Option B across four paths: (a) gain → crystallise → flat epoch; (b) drawdown → late entry → partial recovery below original HWM; (c) gain → 50% exit (`K > 0`) → crash below `B` and below `K` → crystallise; (d) gain → 100% exit (`S = 0`) → crystallise. Verify invariants `N_after * S_after ≈ V` and `P == 0` on flat NAV.
- **Suggested direction for the fix:**
  - Recommend **Option A as the v1 default** (simpler, touches only finalisation, zero cross-route state coupling) and keep Option B as a future upgrade or fix its math completely before offering it:
  - If retaining Option B: (1) set `B = V` (not `V − F_p`) on gain crystallisation; (2) combine management + crystallised performance mints into one dilution-adjusted mint `ΔS = floor((F_m + F_p) × S / (V − F_m − F_p))` on crystallisation, and price `ΔS_m` against `V − K − P − F_m` (or compute `P` after `F_m` and solve jointly); (3) explicitly cap `K` at `V` on crashes and mint `K` into unclaimed fee units *immediately upon exit* (or mint `K` at the last known `N` if `S` hits `0`) rather than deferring unminted `K` to `S = 0`; (4) acknowledge honestly in the table that pooled-`B` accounting still mixes per-unit HWMs on underwater entries (unlike per-unit `H` in Option A).

##### P2 — Notice-period bypass on `nav_fee_schedule_attach` and HWM wipeout via `p -> 0 -> p`
- **Severity:** critical
- **Where:** `## Fee math and economics` → `### Every finalisation`; `## Technical requirements` → `R1`, `R3`, `R4`, `R5`
- **What is wrong:**
  1. **Unrestricted `attach` on live assets:** `R5` adds `nav_fee_schedule_attach` to migrate existing assets (like B24/A666, which already has holders and real funds), and `R1`/`R3`/`R4` never clarify whether newly created NAVCoins set their initial schedule atomically in `nav_asset_register` or via a second `attach` transaction. Crucially, no rule restricts `nav_fee_schedule_attach` from attaching nonzero `management_bps_per_year` or `performance_bps` *immediately without notice* to an asset that already has `S > 0`.
  2. **HWM wipeout:** The spec states: "`H` starts at `N` on the first finalisation with `S > 0` after a schedule with `p > 0` activates." If an issuer lowers `performance_bps` to `0` during a drawdown (a pure decrease → takes effect immediately at the next finalisation under `R4`), and then raises `performance_bps > 0` (after notice), re-initializing `H` to the depressed `N` wipes out the historical HWM. It is also ambiguous whether initial `H` is pre-fee `N` or post-management-fee `N_after`.
- **Why it matters:** Both are direct economic exploits against holders: an issuer can impose sudden fees on live assets without notice, or erase a drawdown HWM so holders pay performance fees twice on the same recovery.
- **How an agent should investigate:** Inspect `NavAssetRegisterOperation` in `crates/types/src/transactions_mempool_receipts.rs` and `crates/execution/src/nft_escrow_asset_execution.rs` to check how initial asset registration and live-asset migration work, and trace state transitions for `schedule: None -> Some(...)` and `p: >0 -> 0 -> >0`.
- **Suggested direction for the fix:**
  - Require brand-new NAVCoins to include `NavFeeScheduleV1` atomically in `NavAssetRegisterOperation` (v2 or optional field), or restrict `nav_fee_schedule_attach` with nonzero management/performance fees to assets with `circulating_supply == 0` and zero finalised epochs.
  - For live assets with `S > 0`, restrict `nav_fee_schedule_attach` to `management_bps_per_year = 0`, `performance_bps = 0`, and spreads matching the asset's existing active route policy (so any introduction of ongoing fees must go through `nav_fee_schedule_set` with the full notice period).
  - Persist `H` monotonically across the lifetime of the NAVCoin (`H` is never reset when `p` drops to `0`; when `p > 0` first activates with `H == 0` and `S > 0`, initialize `H = N_after`, and on any later reactivation keep `H = max(H, N_after)`).

##### P3 — Proving-window supply mismatch / DoS in `R10`, and `packet.nav_per_unit` mutation vs `packet_hash` integrity in `R8`/`R11`
- **Severity:** critical
- **Where:** `## Fee math and economics` → `### Notation`; `## Technical requirements` → `R8`, `R9`, `R10`, `R11`
- **What is wrong:**
  1. **Proving-window DoS & supply divergence (`R10`):** Notation defines `S` as the reserve packet's `circulating_supply`, while `R10` states: "`ΔS` is added to issued supply and to `NavTrackedAsset.circulating_supply`... A later packet whose `circulating_supply` is below the chain's valid supply, fee units included, is rejected." If a user mints even 1 atom *between* the reserve packet's observation height `h_obs` and `nav_epoch_finalize` at height `h`, `packet.circulating_supply` (snapshot at `h_obs`) will be less than chain supply at `h`, causing `R10` to reject a legitimate reserve proof (a trivial DoS against finalisation). Conversely, if a user *redeems* between `h_obs` and `h`, `packet.circulating_supply > chain_issued_supply`, which `R10` does not reject—causing `ΔS` and `N_after` to be computed on stale supply and desynchronizing `NavTrackedAsset.circulating_supply` from ledger `issued_supply`.
  2. **Ledger conservation invariant omitted (`R9`/`R10`):** `R10` adds `ΔS` to `issued_supply` when crediting `unclaimed_fee_units`, but `R9` does not explicitly state that `nav_fee_claim` moves units from `unclaimed_fee_units` to recipient balance *without* re-incrementing `issued_supply`, nor check whether existing ledger invariants assert `issued_supply == sum(holder_balances)`.
  3. **`NavReservePacket.nav_per_unit` mutation vs `packet_hash` (`R8`/`R11`):** `R8` checks `packet.nav_per_unit == floor(V × U / S)` (pre-fee), while `R11` states: "The stored `nav_per_unit` is `N_after`. Every route and every mint and exit at NAV already reads it." If routes and `NavMintAtNav`/`NavRedeemAtNav` read `nav_per_unit` from the `NavReservePacket` struct whose hash is pinned in `NavTrackedAsset` and Ethereum bridge commitments, overwriting `packet.nav_per_unit` mutates a hashed/proven artifact.
- **Why it matters:** Either epoch finalisation becomes DoS-able whenever minting is active during proof generation, or ledger supply / packet hashes break consensus invariants.
- **How an agent should investigate:** Inspect `crates/execution/src/nft_escrow_asset_execution.rs`, `crates/execution/src/nav_vault_asset_execution.rs`, `crates/execution/src/issued_asset_ledger_helpers.rs`, and `crates/types/src/nav_reserve_public_values.rs` to check: (a) how `circulating_supply` in `NavReservePacket` is currently validated against on-chain supply; (b) whether mints/redemptions are allowed during proof generation; (c) whether `issued_supply` is checked against sum of balances (or if a synthetic escrow account is cleaner); and (d) which exact struct field routes read for `nav_per_unit` and whether `packet_hash` commits to it.
- **Suggested direction for the fix:**
  - Validate `packet.circulating_supply` against the chain's supply **at the packet's observation height** (or handle the delta of mints/redemptions between `h_obs` and `h_finalize` explicitly, or verify how the existing protocol already freezes/reconciles supply across `h_obs`). Specify explicitly whether `S` in the fee formula is chain supply at `h_finalize` or `packet.circulating_supply` and ensure `NavTrackedAsset.circulating_supply` cannot drift from `issued_supply`.
  - Document the conservation invariant `issued_supply == sum(account_balances) + unclaimed_fee_units` (or hold unclaimed fees in a deterministic protocol-owned escrow key so standard balance sums hold) and specify that `nav_fee_claim` is a transfer, not a mint.
  - Keep `NavReservePacket.nav_per_unit` immutable (pre-fee gross NAV matching the proof/hash), store `finalized_nav_per_unit = N_after` on `NavTrackedAsset` (or `NavEpochFinalize` state), and explicitly require routes, `NavMintAtNav`, and `NavRedeemAtNav` to read `finalized_nav_per_unit`.

##### P4 — Checked `u128` intermediates overflow and silently zero fees on high-precision or high-supply assets
- **Severity:** major
- **Where:** `## Fee math and economics` → `### Notation` and `### Every finalisation`
- **What is wrong:** The spec specifies checked `u128` intermediates and declares "`F >= V`, or an overflow: no fee." For a NAVCoin with 18 decimals (`U = 10^18`, standard for ERC-20 wrappers) and modest scale (`S = 10^24` atoms = 1M tokens, `g = 10^18`, `p = 1,000`), `g × S × p = 10^45` and `F × S ≈ 10^46`, both vastly exceeding `u128::MAX` (`3.4 × 10^38`). Even 9–12 decimal assets with large token supplies overflow `u128`.
- **Why it matters:** Valid, ordinary NAVCoins will hit the overflow fallback on every epoch and silently collect zero fees, defeating the pillar.
- **How an agent should investigate:** Check the integer types (`u64` vs `u128`) and maximum `precision` allowed for issued assets and `verified_net_assets` in `crates/types/src/account_owned_asset_types.rs` and `crates/types/src/market_nav_asset_types.rs`, and see whether a `U256` / 256-bit `mul_div` helper already exists in `postfiatl1v2`.
- **Suggested direction for the fix:** Replace checked `u128` intermediates with exact 256-bit widening `mul_div_floor` (e.g., `U256` or two-limb `u128` arithmetic) across all fee formulas, and add high-precision (`precision = 18`) and large-supply test vectors to AC 1.

##### P5 — Issuer can neutralize the notice period via `nav_halt` or deadman stall; `accrual_cap_blocks` lacks a protocol maximum
- **Severity:** major
- **Where:** `## Technical requirements` → `R2`, `R4`; `## Open questions for the founder` → OQ3, OQ6
- **What is wrong:**
  1. **Notice subversion via halt/deadman:** `R4` measures notice purely by block height (`h_set + notice`). Because the issuer controls `nav_halt` and `nav_epoch_finalize`, a malicious issuer can schedule a fee increase, immediately call `nav_halt` (or withhold reserve proofs until `max_epoch_gap_blocks` halts dealing) for the full notice period so holders cannot exit at NAV, and then unhalt and finalise at the higher rate.
  2. **Uncapped `accrual_cap_blocks` (and unbounded `crystallisation_interval_blocks`):** `R2` caps rates and enforces a minimum notice period, but puts no protocol upper bound on `accrual_cap_blocks` (which is a creator-supplied field in `R1`), nor a minimum bound on `crystallisation_interval_blocks` under Option B. A creator can set `accrual_cap_blocks = u64::MAX`, allowing full management-fee accrual across arbitrary downtime or deadman freezes.
- **Why it matters:** The notice period is the primary economic defense protecting holders against hostile fee hikes, and `accrual_cap_blocks` is the primary defense against paying management fees during stalled epochs. Both can currently be trivially bypassed by the issuer.
- **How an agent should investigate:** Inspect `nav_halt` and `max_epoch_gap_blocks` deadman logic in `crates/execution/src/nft_escrow_asset_execution.rs` and `crates/types/src/account_owned_asset_types.rs` to see how halt state and epoch gaps are tracked on `NavTrackedAsset`.
- **Suggested direction for the fix:**
  - Extend `R4` so a pending fee increase cannot activate until at least `notice_blocks` of **unhalted, non-deadman** blocks have elapsed (or automatically extend `effective_height` by the duration of any halt/deadman window, or require at least one live epoch finalisation during the notice window).
  - Extend `R2` to enforce a protocol maximum on `accrual_cap_blocks` (e.g., `<= min(protocol_max_accrual_cap, max_epoch_gap_blocks)` so management fees never accrue while the deadman has stopped dealing), and if Option B is retained, a protocol minimum on `crystallisation_interval_blocks`.

##### P6 — Spreads and recipient bindings are unspecified for non-Uniswap primary routes and `spread_recipient`
- **Severity:** major
- **Where:** `## Technical requirements` → `R1`, `R5`, `R13`, `R14`
- **What is wrong:**
  1. `R5` binds `issue_spread_bps` and `redeem_spread_bps` to `pftl_uniswap_route_init_v2` and route epoch advances, but does not state whether `NavMintAtNavOperation`, `NavRedeemAtNavOperation`, and the "private primary routes" mentioned in `R13` also enforce those spreads or are exempt (0 bps).
  2. `R1` defines a single `recipient` on `NavFeeScheduleV1`, whereas existing routes have `spread_recipient` (or route `non_nav_spread_atoms`). `R5` does not say whether `spread_recipient` must match `schedule.recipient` or can differ.
- **Why it matters:** If another primary route charges a different spread (or sends spreads to an undisclosed recipient), the Fees card (`R14`) and explorer (`R15`) will mislead holders about what they pay and who receives it.
- **How an agent should investigate:** Check `NavMintAtNavOperation`, `NavRedeemAtNavOperation`, and private primary route ops in `crates/execution/src/nft_escrow_asset_execution.rs` and `crates/execution/src/nav_vault_asset_execution.rs` to see who can invoke them and whether they currently apply spreads or carry a `spread_recipient`.
- **Suggested direction for the fix:** Explicitly specify in `R5` the spread rule for every primary-market entry/exit path (either bind all of them to `NavFeeScheduleV1` spreads, or explicitly state that issuer-only direct mint/redeem is 0 bps and disclose that in `R14`). Either bind `spread_recipient == schedule.recipient` (migration-compatible for A666 by setting `schedule.recipient` to A666's `spread_recipient`) or add `spread_recipient` as an explicit field in `NavFeeScheduleV1` and `R14`/`R15`.

##### P7 — Missing Task Node issuer UI requirements, narrow SDK scope, and 13 open questions without defaults
- **Severity:** major
- **Where:** `## Technical requirements` → `R5`, `R14`, `R17`; `## Open questions for the founder`
- **What is wrong:**
  1. **Issuer UI missing:** Despite the founder's story ("set the type of fees etc") and Augmented Story step 1 ("SDK or Task Node issuer view"), `R14` only covers read-only display and `R5` only mentions migrating the cash-in spread control. There is no requirement for `issuer/index.jsx` and `server/l1/navcoin-routes.js` to let issuers set management/performance fees, spreads, fix cap, recipient, notice period, cancel a pending schedule, or trigger `nav_fee_claim`.
  2. **SDK scope vs five-element creation:** `R17` defines `create_navcoin(...)` around fee schedule + credential policy, without clarifying whether the SDK also provisions/configures the other four founder-specified NAVCoin elements (Uniswap wrapper, pfUSDC bridge config, primary route, FX fix) or only the PFTL asset definition. Meanwhile, Hyperliquid-on-Solana and scheduled fix settlement remain unowned (OQ13).
  3. **No recommended defaults:** All 13 open questions are left open without recommended defaults, blocking engineering until every question is answered.
- **Why it matters:** Without an issuer UI requirement, Task Node users still cannot set management/performance fees through the app; without defaults, engineering stalls; without clear ownership, near-term founder priorities (Hyperliquid NAVCoin on Solana, scheduled FX fix) risk being dropped.
- **How an agent should investigate:** Review `tasknode/src/features/wallet/l1/issuer/index.jsx`, `tasknode/server/l1/navcoin-routes.js`, `python/postfiat_rpc/navcoin.py`, and the sibling planning pillars (`README.md` and other pillar docs) to see what UI exists today and whether any other pillar owns Hyperliquid-on-Solana or FX fix scheduling.
- **Suggested direction for the fix:**
  - Add a requirement (e.g., `R14b`) covering the Task Node issuer view and backend routes for `nav_fee_schedule_attach`, `nav_fee_schedule_set`, `nav_fee_schedule_cancel`, and `nav_fee_claim`, with client-side cap validation and pending-notice preview.
  - Add a **Recommended default** tag to each of the 13 open questions (e.g., OQ1: Option A for v1; OQ2: caps 500 bps mgmt / 3,000 bps perf / 500 bps spread / 100 bps fix, min notice = 7 days in blocks; OQ3: protocol `blocks_per_year` + `accrual_cap_blocks <= max_epoch_gap_blocks`; OQ7: recipient change applies after notice or immediately as an indexed event; OQ9: forgone; OQ10: optional irreversible lock/ceiling in v2).
  - Clarify `R17`'s exact scope (PFTL asset + route + fix configuration helpers) and either assign Hyperliquid-on-Solana and scheduled fix settlement to a named pillar or create a tracking stub.

##### P8 — Acceptance criterion 3 overstates management-fee neutrality, and `max_supply` clamping lacks a deterministic split rule
- **Severity:** minor
- **Where:** `## Fee math and economics` → `### Every finalisation`; `## Acceptance criteria` → AC 1, AC 3
- **What is wrong:**
  1. **AC 3 vs management fee timing:** AC 3 requires that "An exit just before a finalisation avoids no fee on gains it received, and a late entrant pays no fee on earlier gains." While true for performance fees, an exit just before finalisation *does* avoid management fees for the elapsed interval `Δh` (and a late entrant *does* absorb management fees for `Δh`), up to `m × Δh / Y`, as the spec itself explains under "Entering and exiting between finalisations."
  2. **Deterministic `max_supply` clamping rule missing:** When `S + ΔS > max_supply`, the spec says `ΔS` is clamped and the clipped value recorded, but does not define how the clamped `ΔS` is allocated between `mgmt_fee_units` and `perf_fee_units` (management-first vs pro-rata) or whether `H'` still updates to `N_after`.
- **Why it matters:** AC 3 as written cannot be passed literally for management fees, and missing clamping allocation rules will cause Rust and Python (`recompute_fees`, AC 1) to disagree on `max_supply` edge cases.
- **How an agent should investigate:** Check how `max_supply` is enforced in `crates/execution/src/nft_escrow_asset_execution.rs` and review AC 1 and AC 3 together against the edge-case rules in `### Every finalisation`.
- **Suggested direction for the fix:**
  - Reword AC 3 to specify "avoids no **performance** fee on gains it received, a late entrant pays no **performance** fee on earlier gains, and management-fee timing shift is bounded by `m × Δh / Y`."
  - Define the exact clamping allocation (e.g., management fee takes priority: `ΔS_m_actual = min(ΔS_m, ΔS_clamped)`, `ΔS_p_actual = ΔS_clamped − ΔS_m_actual`, and `H' = max(H, N_after)` computed from `S + ΔS_clamped`) and include a `max_supply`-clamped vector in AC 1.

## Claude Opus 5.5 — 78/100

*Model `anthropic/claude-opus-5.5` · 10722 output tokens*

#### Summary judgement

This is a strong sub-page. It takes a one-paragraph founder complaint ("I cannot set a deterministic performance or management fee… cannot display it… cannot represent it on the explorer") and turns it into a specified, code-grounded design:

- a fee schedule object;
- height-based accrual inside epoch finalisation;
- dilution-correct unit minting;
- a high-water mark;
- notice rules for increases;
- explorer and Task Node display;
- an SDK and guide;
- acceptance criteria;
- an honest list of open questions.

The worked example checks out to the atom. I recomputed `F_m = 110`, `N_m = 1.099890`, `F_p = 9,989`, `ΔS = 10,099·S/(V−F) ≈ 9,265.979`, and `N_after = (V−F)/S = 1.089901`. The "what exists today" table and the revision log suggest the author read the code.

It falls short of the top band for four reasons:

1. The interaction between the reserve packet's snapshot `(V, S)` and supply changes that happen before finalisation is not specified. That gap is load-bearing for every formula.
2. Option B is specified but not verified, and has no worked example.
3. Several economic claims are stronger than the design supports:
   - "heights cannot be sped up";
   - the issuer's control over *when* performance crystallises under Option A;
   - fees charged on attested value.
4. The page is heavy for a founder to decide from. It has 13 open questions and no phased minimum-viable cut that could ship alongside the near-term NAVCoin swap work.

#### Correspondence to the founder's intention

**What the founder asked for in Deliverable 7, and where it is met:**

| Founder ask | Where the page meets it |
|---|---|
| Set a deterministic management or performance fee | R1, R7, Fee math |
| Display fees, including fee type | R14 Fees card, `performance_method` |
| Represent the fee transaction on the explorer | R12, R15, RPC methods |

**The broader NAVCoin vision.** The founder wants NAVCoins as a primitive anyone can build, with an SDK in postfiatl1v2 and a guide on the website, built from five elements (Uniswap asset, PFTL asset, USDC→pfUSDC, wrapper→actual asset, and a London-fix-style pool). The page explicitly maps the fee schedule onto those five elements, makes fees a required part of `create_navcoin`, and folds in R17–R19 for the SDK and guide. That is good alignment.

**Gaps against intent:**

- **Ships into an operator-only world.** The founder's emphasis is "any user can build". Creation is still operator-only, and this pillar defers that to open question 12. The page could say more clearly what it unblocks on day one, beyond the A666 migration.
- **Near-term priority not served.** The founder named Hyperliquid NAVCoin swaps on Solana as "very near term". The page notes it has no owner (OQ13) and that its fees are unresolved (OQ4). It offers no minimal fee path that would let that NAVCoin launch, for example a management fee only with performance off.
- **No link to Deliverable 1.** The founder's frame is post-human, deterministic governance. Fee-schedule changes could be natural "information packet → acted upon" governance events, but the page does not connect to the Hive Chat governance module. This is minor, but it is a missed chance at coherence.
- **Decision load on the founder.** Thirteen open questions, several needing fund-economics expertise (equalisation, crystallisation), is a lot. The page gives no recommended defaults.

#### Technical robustness

**Strengths:**

- Integer-only arithmetic with explicit rounding direction.
- The mint-sizing formula `F·S/(V−F)` is correct.
- Management is charged before performance.
- Accrual can never fail finalisation; edge cases yield reason codes instead.
- Unclaimed fee balances avoid the issuer-credit and frozen-line failure modes.
- R8 closes the "state NAV below reserves" hidden-fee hole.
- R10 prevents supply omission.
- Epoch-pinned dealing means no trade is priced on pre-fee NAV.
- The spread migration keeps `policy_hash` stable.

**Weaknesses:**

- **Snapshot timing.** The relationship between the packet's observation height, dealing that continues on the old epoch, and the finalisation height is unspecified. This is P1.
- **Option B is under-specified.** It touches every supply path and has no worked example or invariant proof.
- **R8 may break existing profiles.** Applying it to every proof profile may be incompatible with profiles that legitimately state a NAV different from `V/S`, for example ones carrying attested liabilities.
- **The clock.** Blocks-per-year depends on a block rate that is not fixed.
- **Future spread changes are unaddressed.** Ethereum-side commitments (`policy_hash`, packet formats) are handled for the migration only, not for spread changes made after it.

#### Economic coherence

The core economics are sound:

- Fees are paid by dilution because reserves sit off PFTL.
- Spreads stay a separate channel outside holder value.
- The high-water mark is per unit.
- Late entrants are not charged for earlier gains.
- Increases require notice and decreases apply immediately.
- The Option A versus Option B trade-off is described accurately: crystallising every epoch charges rise-then-fall paths more.

The main economic gaps are about issuer discretion and self-reference:

- **Option A crystallisation timing.** Under Option A the issuer picks how often to finalise, and therefore when performance is crystallised. Frequent finalisation equals frequent crystallisation, which is a hidden fee lever. Acceptance criterion 6 only tests management fees.
- **Issuer controls the inputs.** The issuer both submits `V` (partly attested) and earns a fee on it.
- **Fee on fee units.** The management fee is charged on `V`, which includes the value of unclaimed fee units. The manager therefore charges a fee on its own fees.
- **Notice without exit is weak.** The notice-period protection depends on holders being able to exit at NAV, but cash-in capacity, credential gating and off-PFTL liquidity can all block exit (OQ6).

#### Problems

**P1 — Packet snapshot versus live supply between observation and finalisation**
- **Severity:** critical
- **Where:** Fee math (Notation: `V`, `S` from "the packet being finalised"); "Entering and exiting between finalisations"; R10; R11.
- **What is wrong:** `V` and `S` come from a reserve packet observed at some height. Dealing at the previous epoch's NAV keeps going until the new epoch finalises (mint and exit only fail *after* the newer epoch finalises). So supply and reserves can change between the observation height and the finalisation height:
  - If mints happen in that window, chain supply exceeds the packet's `S`. Under R10 the packet is then rejected, which is a liveness failure for any actively traded NAVCoin.
  - If redemptions happen in that window, the packet's `S` overstates real supply. `ΔS` and `N_after = V·U/S'` are then computed on the wrong base, so the stored NAV is wrong.
- **Why it matters:** Every fee number and the post-fee NAV depend on `(V, S)` describing the same instant as the supply the mint lands on. If they don't, either finalisation stalls or holders are mispriced.
- **How to investigate:** Read `NavEpochFinalizeOperation` execution and `nav_reserve_public_values.rs`. Find how the current code reconciles packet `circulating_supply` with ledger supply, and whether dealing is halted or frozen at observation. Check how A666/B24 handle it today.
- **Fix direction:** Specify one of these:
  - Dealing is frozen from the observation height to finalisation.
  - Finalisation adjusts `V` and `S` by the net mint and exit flows since observation, which are known on chain, before computing fees.
  - Compute fees on a chain-derived `S` at finalisation, with `V` adjusted by recorded flows.

  Add an acceptance test with dealing inside that window.

**P2 — Issuer controls performance crystallisation frequency under Option A**
- **Severity:** major
- **Where:** Option A; "Why heights" (which only addresses management); acceptance criterion 6; OQ5.
- **What is wrong:** Option A charges a performance fee on each finalisation above the high-water mark. The issuer chooses when to finalise, so it can finalise at local peaks. Finalising more often mechanically raises the expected performance fee on volatile paths, because crystallisation frequency has option value. The page guards management fees against finalisation frequency but not performance fees.
- **Why it matters:** This is a hidden, undisclosed fee lever. It contradicts "holders can check it deterministically" in spirit, because the number is deterministic but the issuer chooses the sampling.
- **How to investigate:** Simulate Option A on a random-walk NAV, finalising every N blocks against every 10N blocks, and compare total performance fees.
- **Fix direction:** Add a minimum crystallisation interval in blocks to Option A as well. Performance would crystallise only at the first finalisation after `crystallisation_interval_blocks`. Make that interval a schedule field under notice rules. Extend acceptance criterion 6 to cover performance.

**P3 — "Heights cannot be sped up" is overstated**
- **Severity:** major
- **Where:** "Why heights"; OQ3.
- **What is wrong:** Block rate depends on consensus parameters, load and validator behaviour (Cobalt). If blocks come faster than `Y` assumes, management fees over-accrue in wall-clock terms. The issuer can't speed blocks up, but the protocol and its validators can change the rate.
- **Why it matters:** The "bps a year" disclosure becomes inaccurate, systematically and in a direction holders cannot audit.
- **How to investigate:** Check the block-production cadence on the v2 chain (empty blocks? load-dependent?). Measure the variance in blocks per day on devnet and testnet.
- **Fix direction:** Recommend a default in OQ3, for example a validator-median timestamp in the header, or a governance-updated `Y` with bounded change per update. Reword the claim to "cannot be sped up by the issuer". Disclose the effective annualised rate on the Fees card.

**P4 — Option B is unverified**
- **Severity:** major
- **Where:** Option B rules table; R13.
- **What is wrong:** The state machine (`B`, `P`, `K`) is given without:
  - a worked example;
  - an invariant (for example, that the value holders lose equals `F_p` paid);
  - a proof that it is path-consistent across entry, exit, finalisation and crystallisation.

  Some specific points need checking:
  - `B += F_m` treats management-fee value as part of the benchmark.
  - `B = V − F_p` is applied only "if there was a gain".
  - The crystallisation mint uses `V` rather than `V − K`.
  - Stored `N = (V−K−P)·U/S'` must coexist with R8's packet check.
- **Why it matters:** Option B is offered as the "fairer" choice. If the founder picks it on an unverified spec, the R13 changes to every supply path carry high risk.
- **How to investigate:** Implement both options in Python. Run a multi-epoch path with entries and exits and conservation checks (holder value plus fee value equals `V`), and compare with the textbook crystallisation outcome.
- **Fix direction:** Add an Option B worked example and conservation invariants. Make acceptance criteria 1–3 explicitly cover both options. Otherwise recommend Option A plus a minimum interval (see P2) as the v1 default and defer Option B.

**P5 — R8 may be incompatible with existing proof profiles**
- **Severity:** major
- **Where:** R8; "What exists today" (NAV arithmetic row).
- **What is wrong:** Today only the SP1 provider-neutral profile forces `N = floor(V/S)`. Other profiles let a submitter state a NAV below reserves. There may be legitimate reasons for that, such as accrued liabilities, haircuts or attested liabilities not captured in `verified_net_assets`. Forcing equality for every NAVCoin with a schedule could break those profiles or force such deductions into `V` with no structure.
- **Why it matters:** Live or planned NAVCoins, the Hyperliquid one especially, may become unable to attach a schedule.
- **How to investigate:** Enumerate the profiles in `NavProofProfile` and why each allows a free NAV. Ask whether any production asset states NAV below `V/S`.
- **Fix direction:** Require any deduction below `V/S` to be itemised as a typed, disclosed liability field that is shown on the explorer, rather than banning it outright.

**P6 — Management fee is charged on the value of unclaimed fee units, and on end-of-interval `V`**
- **Severity:** minor
- **Where:** Step 1, `F_m = V·m·Δh/(10000·Y)`; R9 and R10 (unclaimed units count as supply).
- **What is wrong:** There are two issues:
  - `V` includes the share belonging to fee units, so the manager charges a fee on its own accrued fees.
  - The fee uses end-of-interval `V` for the whole interval, so gains (and inflows just before finalisation) are charged as if held all period.
- **Why it matters:** Both are small, but they are systematic bias, and a sophisticated reviewer would call them out.
- **How to investigate:** Quantify both over a year on the worked-example parameters.
- **Fix direction:** Exempt unclaimed fee units, i.e. use `V·(S − S_fee_unclaimed)/S`. Consider averaging `V` between consecutive packets, or document the bias.

**P7 — Future spread changes and Ethereum-side commitments**
- **Severity:** minor
- **Where:** R5.
- **What is wrong:** The migration keeps `policy_hash` stable. But after migration, a spread change through `nav_fee_schedule_set` changes the multipliers, and therefore `policy_hash`, which Ethereum mint packets commit to. The page does not say how a pending spread increase propagates to routes and Ethereum packets at the effective height, or what happens to in-flight packets.
- **Why it matters:** A spread increase could be rejected on the Ethereum side, or applied before notice through route epoch advances that are not gated.
- **How to investigate:** Trace `PftlUniswapPrimaryMarketPolicyV2`, route epoch advance and the Ethereum contract verification of `policy_hash`.
- **Fix direction:** Specify that a route epoch advance must reject multipliers that don't match the active schedule. Describe the re-pin sequence at activation and add an acceptance test.

**P8 — Notice protection depends on exit capacity the page doesn't guarantee**
- **Severity:** major
- **Where:** User story step 5; R4; OQ6.
- **What is wrong:** Holders are supposedly protected because they can exit during notice. But:
  - cash-in capacity is route-limited;
  - off-PFTL reserves may be illiquid;
  - gated holders may be unable to exit;
  - the deadman can halt dealing.

  The notice period may give no real protection at all.
- **Why it matters:** The central holder protection is conditional and the page doesn't say so.
- **Fix direction:**
  - Recommend that during a pending increase, cash-in capacity cannot be reduced and the deadman pauses the notice clock.
  - Alternatively, holders who exit during notice exit at the old spread.
  - Make this a requirement, not an open question.

**P9 — Charging fees on attested and issuer-supplied value**
- **Severity:** major
- **Where:** OQ4, OQ5; Near-term Hyperliquid paragraph.
- **What is wrong:** The issuer submits `V` (partly attested), finalises, and earns performance fees on it. That is a self-dealing loop, and the page leaves all mitigation open.
- **Why it matters:** This is the founder's near-term NAVCoin, and its `V` is mostly attested.
- **Fix direction:** Propose a default: performance fees only on the proven component, or zero unless a non-issuer attestation quorum exists. Allow management fees on full `V` with the split disclosed.

**P10 — No phased or minimum shippable version**
- **Severity:** minor
- **Where:** Whole page; OQ12, OQ13.
- **What is wrong:** All of R1–R19 is presented as one unit. There is no cut such as:
  - v1: schedule plus management fee plus disclosure and explorer, with spreads migrated;
  - v2: performance fee;
  - v3: Option B or hurdle rates.
- **Why it matters:** The founder wants near-term NAVCoin swaps and an "anyone can build" SDK, and a phased plan would let fees ship with that work.
- **Fix direction:** Add a phasing table and recommended defaults for each open question.

**P11 — The rule that the recipient is not the issuer is cosmetic**
- **Severity:** minor
- **Where:** R1.
- **What is wrong:** The issuer can trivially use a second account. The rule exists because of an execution constraint (crediting the issuer fails), not as a holder protection.
- **Fix direction:** Say so explicitly, so readers don't take it as a governance safeguard.

**P12 — No link to the governance and message-protocol narrative**
- **Severity:** minor
- **Where:** Dependencies; R1 `disclosure_ref`.
- **What is wrong:** Schedule changes could be published as Pillar 2 messages and surfaced in Hive Chat governance (Deliverable 1), which fits the founder's "information packets acted upon" thesis. The page makes this fully optional and never mentions Pillar 1.
- **Fix direction:** Recommend that schedule changes emit a `pf.navcoin.fee.*` message by default. Note optional Hive Chat surfacing.

**P13 — The clipped-fee and accrual-cap policies create quiet issuer incentives**
- **Severity:** minor
- **Where:** Edge cases; `accrual_cap_blocks`; OQ9.
- **What is wrong:** Each policy has an unexamined incentive:
  - If clipped fees are carried forward, the issuer has a reason to raise `max_supply`.
  - If they are forgone, it is not stated who bears the loss.
  - A cap on accrual blocks forfeits fees after long gaps. That is a good finalisation incentive, but it interacts with deadman halts (OQ3).
- **Fix direction:** Recommend these defaults:
  - forgo clipped fees;
  - no accrual while the deadman has halted dealing;
  - disclose `accrual_cap_blocks` on the Fees card.
