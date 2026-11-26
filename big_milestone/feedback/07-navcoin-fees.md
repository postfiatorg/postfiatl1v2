# Feedback — Pillar 7 — Transparent Fees for NAVCoins

> **Mean score:** 64/100 · Gemini 4 Neon 56 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../pillars/07-navcoin-fees.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 56/100

*Model `google/gemini-4-neon` · 18354 output tokens*

#### Summary judgement

This sub-page gets the **product direction and repository grounding** right, but it is **not safe to build from as written**.

On the positive side, the author clearly inspected the actual `postfiatl1v2` and `tasknode` codebases (identifying `NavTrackedAsset`, `PftlUniswapPrimaryMarketPolicyV2`, `FxFixPacketV1.fee_bps`, live B24/A666 parameters, and the Task Node issuer UI) and proposed the right high-level architecture: an on-chain fee schedule on the NAVCoin definition, protocol fee caps, deterministic accrual inside `nav_epoch_finalize`, notice periods for fee increases, and surfaced transparency across Task Node and the block explorer.

However, the specification breaks down at the exact layer where financial protocols live or die: **fund accounting math, open-ended vault economics, and consensus-critical state transitions**. Its fee formulas contain dimensional mismatches (`NAV` means total NAV in one sentence and NAV-per-share in the next), `Δt` is undefined, and its performance-fee crystallisation design creates textbook open-ended fund arbitrage (holders can redeem right before crystallisation to avoid performance fees entirely, while late minters get taxed on gains they never earned). In addition, minting fee shares inside `nav_epoch_finalize` introduces liveness/DoS failure modes against `mint_cap`, `supply == 0`, and Pillar 8 trust-line/KYC restrictions; the notice-period safeguard is trivially bypassable; and major parts of the founder's broader NAVCoin mandate (permissionless creation, SDK + website docs, Hyperliquid-on-Solana swaps, and the London-FX-style pfUSDC fix) are acknowledged in prose and then punted out of scope with no clear owner.

#### Correspondence to the founder's intention

The sub-page corresponds **well to Deliverable 7's narrow user story**, and **weakly to the founder's broader NAVCoin mandate** in the opening prompt.

- **What aligns well:**
  - Directly addresses Deliverable 7's four stated gaps: setting deterministic management and performance fees on a NAVCoin, configuring fee types, displaying fees in the UX (Task Node), and surfacing fee activity on the block explorer.
  - Preserves the core principle that fees must be deterministic, verifiable on-chain, and attached to the NAVCoin definition rather than handled off-chain.
- **What is missed or narrowed:**
  - **Explorer visibility of fee-configuration transactions:** The founder specifically wrote *"no way to ... set the type of fees etc - and **that transaction** cannot be represented in any way on the block explorer."* Yet R8 only requires the explorer to display epoch *accruals* and `nav_fee_history(asset_id)` — it omits explorer rendering for `NavAssetRegisterOperation` fee schedules, `nav_fee_schedule_set` transactions (including pending increases), and primary-market spread charges.
  - **Incomplete fee transparency:** `FxFixPacketV1.fee_bps` is listed in "What exists today" as an existing NAVCoin fee mechanism, but it is omitted from `NavFeeScheduleV1`, Task Node's Fees card, and the explorer. Leaving any fee rail outside the unified schedule undermines the "transparent fees" guarantee.
  - **Orphaned NAVCoin deliverables:** In the prompt's preamble, the founder explicitly calls out (a) near-term NAVCoin swaps via a Hyperliquid NAVCoin on Solana, (b) turning NAVCoins into a primitive *anyone* can build via a NAVCoin SDK in `postfiatl1v2` and onboarding docs on `postfiatorg`, and (c) a scheduled London-FX-style pfUSDC fix pool. None of the other nine deliverables owns these items. Pillar 7 mentions them in a descriptive section ("The NAVCoin primitive these fees attach to"), adds one bullet (R9) for `create_navcoin(...)`, and then punts permissionless creation into "Dependencies" and Open Question 4. Without either expanding Pillar 7 or carving out an explicit companion pillar, the founder's core NAVCoin vision has no execution owner.

#### Technical robustness

As an engineering specification for an L1 state transition (`nav_epoch_finalize` and `nav_fee_schedule_set`), the sub-page has **critical underspecification and failure modes**:

1. **Formula and unit ambiguity:** `management_accrual = NAV × rate × Δt` and `performance_accrual = max(0, NAV − HWM) × rate × supply` use `NAV` inconsistently (total NAV vs. NAV per share), and `NavFeeAccrual`'s `mgmt_atoms` / `perf_atoms` do not specify whether they are denominated in NAVCoin share atoms or reserve-currency atoms. Nor is there a specification of fixed-point precision, rounding direction, or dust handling (R4 merely asserts "integer math with documented rounding").
2. **Undefined time basis (`Δt`):** On a deterministic L1, `Δt` cannot be left implicit. Using attestor/issuer-submitted timestamps lets an issuer inflate `Δt`; using a fixed per-epoch constant lets an issuer spam epochs to drain management fees or blow through `increase_notice_epochs`; using block height couples financial accrual to variable block times.
3. **Epoch finalisation DoS vectors:** Because fee minting executes *inside* `nav_epoch_finalize`, any failure in the mint path bricks NAV updates for the entire asset. The spec does not handle `supply == 0` (division by zero when computing NAV per share), `supply + fee_shares > mint_cap` (R6 says mint-cap invariants must still hold, which implies epoch finalisation would revert at cap!), or recipient account failures (frozen account, missing trust line, or expired KYC credential under Pillar 8).
4. **Governance and access-control gaps:** `nav_fee_schedule_set` specifies neither who is authorized to call it nor how non-scalar changes (e.g., shortening `crystallisation`, lowering `increase_notice_epochs`, or swapping `recipient`) are classified as increases vs. decreases. As written, an issuer could drop `increase_notice_epochs` to `0` immediately and hike fees on the next block.
5. **Migration and cross-chain sync omitted:** Unifying `issue_spread_bps` / `redeem_spread_bps` into `NavFeeScheduleV1` creates a dual source of truth alongside live `PftlUniswapPrimaryMarketPolicyV2` (used by live real-money assets B24/A666). Worse, the spec never explains how post-fee diluted `nav_after` or updated spreads propagate to `PFTLUniswapPrimaryMarketV2.sol`, `NAVGuardHook.sol`, and `crates/pftl_uniswap_proofs`.

#### Economic coherence

The sub-page's economic model has **two fundamental flaws** that would make fee-charging NAVCoins exploitable in production:

1. **Broken open-ended performance fee accounting:** In any open-ended fund or tokenized vault where users can mint and redeem at NAV between crystallisation dates (monthly/quarterly), computing performance fees *only* on the crystallisation date as `max(0, NAV − HWM) × rate × supply` is economically broken:
   - **Redemption evasion:** Holders whose asset rose above HWM redeem one epoch before crystallisation at the unreduced (pre-fee) NAV, paying 0% performance fee. The issuer earns nothing on profits generated for redeeming holders.
   - **Late-entrant confiscation:** Anyone who mints shares shortly before crystallisation pays full pre-fee NAV (say $1.50) and immediately suffers a step-function loss on crystallisation as the fee is charged on `(1.50 − 1.00)` for their newly minted shares — paying performance fees on gains they never participated in.
   - **Predictable secondary-market arbitrage:** Because Uniswap (`NAVGuardHook`) and primary markets trade at pre-fee NAV up until crystallisation and then drop discontinuously at crystallisation, arbitrageurs have a risk-free trade around every crystallisation epoch.
   - Real-world funds and tokenized vaults solve this via **continuous (per-epoch) provisional performance-fee accrual** into a virtual liability that reduces quoted NAV per share on every epoch (and reverses if NAV falls before crystallisation), or by restricting v1 to per-epoch crystallisation with explicit trade-offs.
2. **Missing accounting mechanics and incoherent "deduct from reserves" alternative:**
   - The spec does not define whether performance fee is computed on NAV *before* or *after* management fee (standard practice: after management fee, so managers don't charge performance fees on their own management fees), nor whether `hwm_after` resets to `nav_before`, post-mgmt NAV, or `nav_after`, nor when HWM updates (crystallisation dates only, not intermediate epochs), nor how initial HWM is set for new or migrated assets.
   - Open Question 2 treats "deducting from reserves" as a simple toggle against "minting fee units (dilution)". In a NAVCoin whose reserves live off-chain or cross-chain (e.g., Hyperliquid or custodied reserves proven via `NavReservePacket`), PFTL **cannot move reserves**. "Deducting from reserves" on-chain without moving real assets would require an on-chain `unpaid_fee_liability` ledger, an off-chain withdrawal/claim mechanism, and reserve-proof reconciliation so withdrawals don't look like missing backing. Dilution via minted NAVCoin units is vastly cleaner because the recipient simply redeems those units through the normal primary-market redemption path.

---

#### Problems

##### P1 — Performance-fee crystallisation without per-epoch provisional accrual breaks open-ended mint/redeem economics
- **Severity:** critical
- **Where in the sub-page:** User story step 3; Architecture diagram; Technical requirements R1, R4, R5.
- **What is wrong:** R1 allows `crystallisation` to be monthly or quarterly while Step 3 / R4 computes `performance_accrual = max(0, NAV − HWM) × rate × supply` only on crystallisation dates, with no provisional accrual on intermediate epochs.
- **Why it matters:** Between crystallisation dates, NAV per share is overstated (ignoring accrued performance-fee liability). Holders can redeem one epoch before crystallisation to escape the performance fee entirely; new minters entering before crystallisation immediately lose principal to fees on historical gains they never earned; and Uniswap pools / `NAVGuardHook` experience predictable step-function NAV drops on crystallisation dates that invite arbitrage.
- **How an agent should investigate:** Inspect `crates/execution/src/nav_vault_asset_execution.rs` and `docs/navcoins/primary-market-accounting.md` to see how `NavMintAtNavOperation` and `NavRedeemAtNavOperation` price shares from the latest finalized epoch. Review standard ERC-4626 / hedge-fund equalisation and continuous-accrual-with-periodic-crystallisation accounting models.
- **Suggested direction for the fix:** On every epoch, compute a *provisional* (unpaid) performance-fee liability `pending_perf_liability = max(0, nav_post_mgmt − HWM) × perf_rate × supply` (which rises and falls with NAV and does not mint shares yet) and quote `nav_after` net of this liability for all mints, redemptions, and Uniswap proofs. On crystallisation epochs only, convert the liability into minted fee shares and update `HWM = max(HWM, nav_after)`. Alternatively, if keeping v1 minimal, restrict `crystallisation` in v1 to `each_epoch` and explicitly defer multi-epoch crystallisation until provisional liability accounting is built.

##### P2 — Dimensionally inconsistent fee formulas, ambiguous units, and unspecified HWM / fee-ordering rules
- **Severity:** critical
- **Where in the sub-page:** User story step 3; Architecture diagram; Technical requirements R4, R5.
- **What is wrong:**
  1. `management_accrual = NAV × rate × Δt` and `performance_accrual = max(0, NAV − HWM) × rate × supply` use the symbol `NAV` to mean total reserves in the first equation and NAV per share in the second.
  2. R5's `mgmt_atoms` and `perf_atoms` do not specify whether they are reserve-currency atoms or minted NAVCoin share atoms.
  3. The exact share-minting formula for dilution is omitted: to transfer fee value $F$ from reserves $R$ by minting $\Delta S$ shares at `nav_after`, $\Delta S = \frac{F \cdot S}{R - F}$ (not $F / \text{NAV}_{\text{before}}$); for management fee fraction $r_m = \text{rate} \cdot \Delta t$, reserves cancel out entirely ($\Delta S_m = S \cdot \frac{r_m}{1 - r_m}$).
  4. Order of operations (management fee before performance fee vs. simultaneous), HWM update rule (`nav_before` vs. `nav_after`, crystallisation-only vs. every epoch), initial HWM (at launch or migration), fixed-point scaling, and rounding direction are all unspecified.
- **Why it matters:** Two engineers implementing this spec will produce different consensus state transitions, breaking validator determinism, causing silent overpayment/underpayment of fees, or enabling dust-truncation exploits on small supplies.
- **How an agent should investigate:** Check the exact integer types, decimal scales, and arithmetic helpers used for NAV, supply, and reserve proofs in `crates/types/src/account_owned_asset_types.rs`, `crates/nav_reserve_protocol`, and `crates/execution/src/nav_vault_asset_execution.rs`.
- **Suggested direction for the fix:** Write out the exact integer-arithmetic specification in an appendix or R4/R5: distinguish `total_reserves_atoms` ($R$), `supply_atoms` ($S$), and `nav_per_share_fp` ($R \cdot 10^k / S$); explicitly name receipt fields by unit (e.g., `mgmt_fee_value_atoms` and `mgmt_fee_shares_minted`); apply management fee first, then performance fee on post-management NAV; update `hwm_after = max(hwm_before, nav_after)` only on crystallisation epochs; initialize `HWM` on the first epoch with `supply > 0`; and specify floor rounding (favoring holders) with exact denominator scaling.

##### P3 — Undefined `Δt` in management fee accrual creates consensus ambiguity and epoch-cadence exploits
- **Severity:** critical
- **Where in the sub-page:** User story step 3 & 5; Technical requirements R1, R3, R4.
- **What is wrong:** `management_bps_per_year` is multiplied by `Δt`, and notice periods are measured in `increase_notice_epochs`, without defining what an epoch's duration is or how `Δt` is sourced deterministically on-chain.
- **Why it matters:** If `Δt` comes from an issuer/attestor-provided timestamp in `NavReservePacket`, a malicious issuer can skew timestamps to over-accrue management fees. If `Δt` is a fixed constant per epoch (e.g., `1/365`) and epoch cadence is not protocol-enforced, an issuer can submit rapid-fire epochs to both drain management fees and fast-forward through `increase_notice_epochs` in minutes. If `Δt` is computed from wall-clock block timestamps without bounds, halted chains or delayed proofs can produce massive single-epoch accruals.
- **How an agent should investigate:** Inspect `NavEpochFinalizeOperation` and `NavReservePacket` in `crates/types/src/transactions_mempool_receipts.rs` and `crates/types/src/account_owned_asset_types.rs` to see what timestamps, block heights, and cadence rules (`min_epoch_interval`, proof validity windows) already exist on PFTL.
- **Suggested direction for the fix:** Define `Δt` strictly from consensus-committed block timestamps (e.g., `clamp(current_block_ts - prev_epoch_finalized_ts, 0, max_accrual_window) / 31_536_000`, using Actual/365) with a protocol-enforced minimum interval between finalized epochs (or zero management accrual if `Δt == 0`). Express fee-increase notice periods in **consensus time or block height** (or minimum wall-clock duration AND minimum finalized epochs) so an issuer cannot shorten holder exit time by spamming epochs.

##### P4 — `nav_epoch_finalize` DoS/liveness hazards from minting fee shares directly to `recipient`
- **Severity:** critical
- **Where in the sub-page:** Technical requirements R4, R5, R6; Dependencies (Pillar 8).
- **What is wrong:** R5 mints fee units directly to `recipient` inside `nav_epoch_finalize`, and R6 asserts that mint-cap and reserve invariants still hold. The spec ignores four fatal execution edge cases:
  1. `supply + fee_shares > mint_cap` causes `nav_epoch_finalize` to revert, freezing the asset's NAV and redemptions permanently.
  2. `supply == 0` (at launch or after 100% redemption) causes division by zero when computing `NAV = reserves / supply`.
  3. Under Pillar 8 (optional trust lines / KYC-gated NAVCoins), if `recipient` lacks a trust line, has an expired KYC credential, or is frozen, direct minting reverts `nav_epoch_finalize`.
  4. If `fee_shares` minting changes total supply inside `nav_epoch_finalize`, subsequent reserve checks must explicitly distinguish pre-fee vs. post-fee supply so the reserve proof does not fail its own invariant check.
- **Why it matters:** `nav_epoch_finalize` is the heartbeat of a NAVCoin. Coupling it to mutable external constraints (`mint_cap`, recipient trust lines/KYC) creates unacceptable protocol liveness risks.
- **How an agent should investigate:** Trace the full execution path of `NavEpochFinalizeOperation` and `mint_cap` / reserve-invariant checks in `crates/execution/src/nav_vault_asset_execution.rs` and `crates/types/src/account_owned_asset_types.rs`.
- **Suggested direction for the fix:**
  1. Short-circuit fee accrual cleanly (`mgmt = 0, perf = 0`) whenever `supply == 0` or `reserves == 0`.
  2. Mint fee shares into an **on-chain protocol-owned fee escrow / claimable balance** keyed by `(asset_id, recipient)` rather than directly into `recipient`'s wallet, and add a `nav_fee_claim` op that enforces trust-line/KYC checks at claim time so `nav_epoch_finalize` can never revert due to recipient state.
  3. Clarify `mint_cap` semantics: either enforce `mint_cap` only on user-initiated `NavMintAtNavOperation` (not protocol fee dilution) or cap total supply with a reserved headroom so fee accrual never bricks epoch finalisation.

##### P5 — Notice-period safeguard (`nav_fee_schedule_set`) is trivially bypassable and lacks access control
- **Severity:** major
- **Where in the sub-page:** User story step 5; Technical requirements R1, R3; Open questions #3.
- **What is wrong:**
  1. No authorization model is defined for `nav_fee_schedule_set` (issuer key, operator, multisig, key rotation).
  2. `increase_notice_epochs` is a field inside `NavFeeScheduleV1` itself, with no protocol floor and no rule preventing an issuer from lowering `increase_notice_epochs` to `0` immediately ("a decrease applies immediately") and raising fees on the next transaction.
  3. Non-scalar field changes are unclassified: shortening `crystallisation` (e.g., quarterly → per-epoch) increases effective performance fees; changing `recipient` or combining an increase and decrease in one call has undefined semantics; overwriting an already-pending schedule is undefined.
- **Why it matters:** Holder protection against rug-pull fee hikes is a core value proposition of transparent on-chain fees. If the notice period can be bypassed in two transactions, the protection is illusory.
- **How an agent should investigate:** Check how `PftlUniswapPrimaryMarketPolicyV2` updates are authorized in `crates/execution/src/nav_vault_asset_execution.rs` and `server/l1/navcoin-routes.js`.
- **Suggested direction for the fix:** Specify the exact authorized signer (NAVCoin issuer/admin authority). Enforce a **protocol-wide minimum notice duration** (creators may only set a *longer* notice period, and *reducing* `increase_notice_epochs` must itself wait the old notice period). Classify shortening `crystallisation` as a fee increase; disallow mixed immediate/pending changes in a single op (or apply the notice period to the entire pending schedule); define single-slot pending replacement/cancellation semantics; and consider an optional `fee_schedule_locked` / creator-chosen `max_fee_cap` flag for credibly immutable low-fee NAVCoins.

##### P6 — Dual source of truth and cross-chain desynchronization with existing primary-market and Uniswap/EVM contracts
- **Severity:** major
- **Where in the sub-page:** What exists today; The NAVCoin primitive; Technical requirements R1, R5.
- **What is wrong:** R1 states that `issue_spread_bps` and `redeem_spread_bps` "unify today's route multipliers", referring to `PftlUniswapPrimaryMarketPolicyV2` (`issue_multiplier_bps`, `redeem_multiplier_bps`, `spread_recipient`) which is live with real money (B24/A666). There is no migration path for existing assets, no reconciliation of `spread_recipient` vs. `recipient` (spreads are collected in quote/wrapped assets into "non-NAV fee custody", whereas mgmt/perf fees are minted in NAVCoin units), and no specification of how `PFTLUniswapPrimaryMarketV2.sol`, `NAVGuardHook.sol`, and `crates/pftl_uniswap_proofs` consume post-fee `nav_after` and updated spreads.
- **Why it matters:** If PFTL dilutes NAV per share via fee minting while `NAVGuardHook.sol` on EVM still enforces prices derived from raw pre-fee reserve proofs (or stale spread multipliers), the Uniswap pool and PFTL primary market will diverge, creating cross-chain arbitrage that drains liquidity providers or the primary market.
- **How an agent should investigate:** Inspect `crates/types/src/market_nav_asset_types.rs`, `crates/pftl_uniswap_proofs`, `crates/ethereum-contracts/src/PFTLUniswapPrimaryMarketV2.sol`, `crates/ethereum-contracts/src/NAVGuardHook.sol`, and `docs/navcoins/primary-market-accounting.md`.
- **Suggested direction for the fix:** Define a clean state migration for existing `NavTrackedAsset` / `PftlUniswapPrimaryMarketPolicyV2` instances (deprecating the old multiplier fields or making `PftlUniswapPrimaryMarketPolicyV2` read from `NavFeeScheduleV1`). Document the separate accounting flows for spread fees (quote-asset non-NAV custody) vs. ongoing fees (minted NAVCoin shares), and specify explicitly that EVM proofs / `NAVGuardHook` attest to post-fee `nav_after` and synchronized spread bounds.

##### P7 — Incomplete fee transparency: `FxFixPacketV1.fee_bps` excluded and explorer requirements miss fee-change and spread transactions
- **Severity:** major
- **Where in the sub-page:** What exists today; Technical requirements R1, R7, R8; Acceptance criteria #3.
- **What is wrong:**
  1. `FxFixPacketV1.fee_bps` is listed as an existing fee mechanism in "What exists today" but is completely absent from `NavFeeScheduleV1` (R1), Task Node's Fees card (R7), and the explorer (R8).
  2. R8 only covers epoch `nav_fee_accrual` items and `nav_fee_history(asset_id)` — it omits explorer views for `NavAssetRegisterOperation` fee schedules, `nav_fee_schedule_set` transactions (including pending vs. active state), and per-transaction spread fees paid on `NavMintAtNavOperation` / `NavRedeemAtNavOperation`.
- **Why it matters:** Directly misses part of the founder's user story (*"set the type of fees etc - and that transaction cannot be represented in any way on the block explorer"*), and leaves a blind spot where an issuer could advertise 0% fees on the Fees card while extracting fees via `FxFixPacketV1.fee_bps`.
- **How an agent should investigate:** Inspect `crates/types/src/fx_fix_types.rs` and the new-chain explorer schema/indexer to see how `Nav*` operations and `FxFixPacketV1` transactions are indexed and rendered.
- **Suggested direction for the fix:** Include a `fix_fee_cap_bps` (or unified `fix_fee_bps`) in `NavFeeScheduleV1` and display it on the Task Node Fees card. Expand R8 to explicitly cover three explorer surfaces: (1) transaction detail views for `NavAssetRegisterOperation` and `nav_fee_schedule_set` (showing old → new schedule and effective epoch), (2) per-swap/mint/redeem spread and fix fee line items, and (3) epoch-level `NavFeeAccrual` records.

##### P8 — "Deducting from reserves" is framed as a simple alternative without reconciling off-chain custody
- **Severity:** major
- **Where in the sub-page:** User story step 3; Architecture diagram; Technical requirements R5; Open questions #2.
- **What is wrong:** The spec casually presents "deducting from NAV per share / deducting from reserves" as an alternative to minting fee units.
- **Why it matters:** PFTL does not hold off-chain or cross-chain NAVCoin reserves (e.g., Hyperliquid positions or custodied assets). The chain cannot transfer those reserves to the fee recipient. Merely reducing `nav_after` on-chain without either minting redeemable shares or maintaining a persistent on-chain `unpaid_fee_liability` reconciled against off-chain reserve withdrawals breaks the reserve-proof invariant on the very next epoch (the proven reserves would still include the "deducted" fee, causing NAV to bounce right back up and double-charging holders!).
- **How an agent should investigate:** Review `docs/navcoins/primary-market-accounting.md`, `crates/nav_reserve_protocol`, and `tools/nav-reserve-proof` to confirm how proven reserve balances relate to on-chain liabilities.
- **Suggested direction for the fix:** Remove "deduct from reserves" as a casual toggle and **commit to minted fee shares (dilution)** as the sole v1 mechanism — explaining *why* it is architecturally superior (the chain can execute it atomically and deterministically without off-chain custody coordination, and the recipient realizes cash via standard `NavRedeemAtNavOperation` or secondary markets). If reserve deduction is ever desired later, frame it accurately as a much larger off-chain liability-reconciliation design.

##### P9 — Broader founder NAVCoin requirements are acknowledged in prose but left unowned
- **Severity:** major
- **Where in the sub-page:** Status banner; The NAVCoin primitive these fees attach to; Technical requirements R9; Dependencies; Open questions #4.
- **What is wrong:** The founder's prompt explicitly prioritizes (1) near-term NAVCoin swaps (tracking a Hyperliquid NAVCoin on Solana), (2) permissionless NAVCoin creation ("a primitive that any user can build"), (3) a NAVCoin SDK in `postfiatl1v2` plus onboarding docs on `postfiatorg`, and (4) a scheduled London-FX-style pfUSDC fix pool. Pillar 7 summarizes these in prose and adds a single line (R9) for `create_navcoin(...)`, while relegating permissionless creation to an external dependency with no pillar owning it and omitting Hyperliquid/Solana swaps and the scheduled fix from requirements and acceptance criteria entirely.
- **Why it matters:** Transparent fees for third-party creators have limited impact if only the operator can create NAVCoins and users lack the SDK, docs, swaps, and fix liquidity the founder explicitly asked for. Across the 10-pillar plan, these core items risk falling through the cracks completely.
- **How an agent should investigate:** Cross-check the other 9 pillar sub-pages in the planning document to confirm whether permissionless NAVCoin creation, the NAVCoin SDK, Hyperliquid-on-Solana swaps, and the scheduled pfUSDC fix are owned anywhere else.
- **Suggested direction for the fix:** Either (a) explicitly map each of those four founder items to a named sibling pillar / roadmap milestone with a clear sequencing contract, or (b) add a dedicated "NAVCoin Primitive & SDK" sub-pillar (or expand R9 + Acceptance Criteria) covering permissionless registration rules, the packaged Python/Rust SDK, the `postfiatorg` quickstart guide, and the near-term Hyperliquid/Solana + FX-fix deliverables.

##### P10 — Phantom dependency on Pillar 2 (`pf.navcoin.fee.*`) with no backing requirement or schema
- **Severity:** minor
- **Where in the sub-page:** Dependencies table.
- **What is wrong:** The Dependencies table lists `[Pillar 2 — Message protocol]` for "`pf.navcoin.fee.*` messages for off-chain disclosure documents", but neither `pf.navcoin.fee.*` nor off-chain disclosure documents appear anywhere else in the User Story, Architecture, Technical Requirements, or Acceptance Criteria.
- **Why it matters:** Creates an artificial cross-pillar blocker and confuses implementers about whether Pillar 7 must block on Pillar 2's protobuf/IPFS schema.
- **How an agent should investigate:** Check `02-message-protocol.md` to see whether `pf.navcoin.fee.*` is defined there and whether fee disclosure documents are actually needed in v1.
- **Suggested direction for the fix:** Either add an explicit optional `disclosure_uri` / message-hash field to `NavFeeScheduleV1` (and mark the Pillar 2 dependency as *optional / phase 2* so Pillar 7 can ship independently), or remove the phantom dependency row.

##### P11 — Acceptance criteria miss critical accounting, governance, SDK, and cross-chain test cases
- **Severity:** minor
- **Where in the sub-page:** Acceptance criteria #1–5.
- **What is wrong:** The 5 acceptance criteria only test a happy-path 3-epoch accrual, basic notice/decrease behavior, basic UI/explorer display, cap rejection, and reserve invariants. They do not test mid-period mints and redemptions, NAV drawdowns and HWM preservation, multi-epoch crystallisation (or provisional accrual), `supply == 0`, `mint_cap` boundary behavior, recipient KYC/trust-line failure isolation, notice-period bypass attempts (e.g., lowering `increase_notice_epochs`), explorer rendering of `nav_fee_schedule_set` and spread fees, EVM `NAVGuardHook` price consistency after fee dilution, or the R9 SDK + worked example.
- **Why it matters:** A naive implementation with all the bugs in P1–P7 would still pass all 5 acceptance criteria as written.
- **How an agent should investigate:** Map every technical requirement (R1–R9) and every failure mode identified in P1–P8 to a concrete, deterministic integration test scenario on devnet.
- **Suggested direction for the fix:** Expand the acceptance criteria into a targeted verification matrix covering: (1) drawdown + recovery HWM test, (2) mint/redeem between epochs with no fee arbitrage or late-entrant taxation, (3) `supply == 0` and at-`mint_cap` epoch finalisation liveness, (4) notice-period downgrade rejection / delay, (5) explorer verification of both `nav_fee_schedule_set` and `NavFeeAccrual` + spread/fix fees, (6) EVM proof / `NAVGuardHook` alignment post-accrual, and (7) end-to-end SDK `create_navcoin(...)` execution matching published worked-example numbers to the exact atom.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 7113 output tokens*

#### Summary judgement

This is a strong, well-grounded sub-page. It takes the founder's short Deliverable 7 story and turns it into a specific plan:

- a fee schedule object on the NAVCoin definition;
- a deterministic accrual step inside epoch finalisation;
- a notice rule for fee increases;
- display on Task Node and the explorer;
- SDK integration.

It is honest about what exists today: spreads only, operator-only creation, no explorer rendering. It ties the work back to the "NAVCoin as a primitive" goal.

The weaknesses sit in the fee mathematics and the economic mechanics, which are the parts that most need to be exactly right for a "deterministic" fee:

- The performance-fee formula has inconsistent units.
- The dilution-mint sizing is ambiguous and likely wrong as written.
- The high-water mark (HWM) is undefined as total vs per-unit, and there is no equalisation story.
- How the time interval Δt is measured is undefined.
- What happens to the live Uniswap wrapper and NAV guard after fee units are minted is not addressed.
- Migrating live spread policies (B24/A666, real money) into the new schedule is unplanned.
- The "proven NAV only" rule collides with the admitted fact that Hyperliquid positions are only attested.

None of these break the plan, but an implementer following the page literally could ship incorrect fee accounting.

#### Correspondence to the founder's intention

The founder asked for four things. All are covered:

| Founder's need | Where the page covers it |
|---|---|
| Set a deterministic performance or management fee | R1, R4 |
| Display a fee | R7, the Fees card |
| Set the type of fee | R1 fields |
| Represent the fee transaction on the block explorer | R5, R8 |

It also correctly places fees inside the broader NAVCoin-primitive vision from the founder's prompt:

- the five core elements;
- the SDK on postfiatl1v2 and postfiatorg;
- "anyone can build".

The notice-period mechanism is a sensible addition that protects holders, in the spirit of "transparent".

Minor gaps in correspondence:

- The founder's phrase "set the type of fees" may imply more types than management, performance and spreads (for example hurdle rates or fix fees).
- The fix-packet `fee_bps` is listed but not folded into the schedule.
- The near-term Hyperliquid-on-Solana NAVCoin is mentioned but not connected to how its fees would work.

#### Technical robustness

**Good:**

- Integer math with documented rounding.
- Accrual computed inside `nav_epoch_finalize`.
- A receipt record with before and after values.
- An RPC history method.
- A cap check at registration.
- Acceptance criteria requiring independent recomputation.

**Weak:**

- Unit definitions for NAV and HWM are inconsistent.
- The mint-for-fee sizing formula is wrong as literally written.
- The source of Δt is unspecified.
- Behaviour on missed or late epochs is unspecified.
- The rule for mixed schedule changes (some fields up, some down) is unspecified.
- The impact of minting on the cross-chain wrapper and the NAV guard hook is unaddressed.
- There is no migration plan for live policies.
- There are no test vectors or reference implementation.

#### Economic coherence

The overall model mirrors standard fund economics, which is coherent: management fee on assets, performance fee over a high-water mark, primary-market spreads, notice before increases.

Open issues:

- **Performance-fee fairness.** A single fund-level per-unit HWM creates free-rider and over-charging effects for holders who enter at different NAVs. There is no equalisation or series accounting.
- **Snapshot manipulation.** Performance fees computed from a single crystallisation snapshot invite NAV manipulation by an issuer who is also an attestor.
- **The notice period's protective value** depends on holders actually being able to redeem. That is constrained by the daily proving window, fix capacity and redeem spreads, which the issuer may also raise.
- **No protocol-level fee.** There is no protocol or validator take, and no discussion of whether one should exist (relevant to validator economics in Deliverable 3).
- **Two treatments of fees.** Spreads go to "non-NAV fee custody" while management fees dilute. Accounting treatment needs reconciling.

#### Problems

**P1 — Performance-fee formula has inconsistent units**
- **Severity:** critical
- **Where:** User story step 3; architecture diagram; R5.
- **What is wrong:** `management_accrual = NAV × rate × Δt` treats NAV as total fund value. `performance_accrual = max(0, NAV − HWM) × rate × supply` treats NAV as per-unit value. It is undefined whether HWM is per-unit or total.
- **Why it matters:** If HWM is total NAV, ordinary subscriptions and redemptions would trigger or suppress performance fees. Fees would be wrong by a factor of supply.
- **How to investigate:** Check how `NavTrackedAsset` and `NavEpochFinalizeOperation` represent NAV (total reserves vs per-unit price). Check what the NAV guard consumes.
- **Fix direction:** Define `nav_total`, `nav_per_unit` and `hwm_per_unit` explicitly. Write both formulas in those terms. Specify whether the performance fee is computed after the management fee.

**P2 — Dilution mint sizing is wrong as written**
- **Severity:** major
- **Where:** R5, "minting NAVCoin units at `nav_after`".
- **What is wrong:** Minting `F / nav_per_unit` units transfers less than F of value, because the mint itself dilutes the unit price. The correct number of units is `F × S / (NAV_total − F)`. The phrase "at nav_after" is circular unless defined.
- **Why it matters:** The recipient is systematically under-paid, or the published NAV after fees disagrees with an independent recomputation. That breaks acceptance criterion 1.
- **How to investigate:** Write a worked numeric example. Compare value transferred against the intended fee.
- **Fix direction:** Specify the closed-form unit count and the rounding direction (round in favour of holders). Publish test vectors.

**P3 — Δt and epoch timing are undefined**
- **Severity:** major
- **Where:** User story step 3; R1; R4.
- **What is wrong:** Fees are set in bps per year, but accrual happens per epoch. Nothing says whether Δt comes from consensus timestamps, block heights or epoch count. Nothing says what happens when an epoch is skipped or delayed (for example, the Solana side only opens in a daily proving window).
- **Why it matters:** Accruals will be non-deterministic, or wrong, across irregular epochs.
- **How to investigate:** Check how the epoch cadence and timestamps are defined in the reserve protocol.
- **Fix direction:**
  - Define Δt from the finalised timestamps of consecutive proven epochs, with a fixed day-count convention (for example actual/365).
  - Cap Δt.
  - Define catch-up behaviour for missed epochs.

**P4 — "Proven NAV only" conflicts with attested reserves**
- **Severity:** major
- **Where:** R4, compared with the "Near term" note that HIP-3 positions are attested, not proven.
- **What is wrong:** The Hyperliquid NAVCoin, which is the founder's near-term priority, would have no proven NAV. Under R4 it could not charge fees at all. The alternative, charging on attested NAV, is unaddressed.
- **Why it matters:** Either the headline NAVCoin cannot use fees, or performance fees are paid on numbers the issuer controls.
- **How to investigate:** Inventory which NAVCoins have fully proven NAV versus attested components.
- **Fix direction:** Either allow management fees on attested NAV but performance fees only on proven NAV, or require an independent attestor quorum. Disclose the proof level on the Fees card.

**P5 — No equalisation and snapshot manipulation risk for performance fees**
- **Severity:** major
- **Where:** R1 (`performance_basis`, `crystallisation`), user story step 3.
- **What is wrong:**
  - A single fund-level HWM charges late entrants for gains made before they joined, or lets them free-ride on losses before they joined.
  - A single snapshot at the crystallisation date can be manipulated.
- **Why it matters:** Holders are treated unfairly, and the issuer has an incentive to game the snapshot. Both undermine "transparent fees" credibility.
- **How to investigate:** Review fund equalisation methods: series accounting, equalisation credits, depreciation deposits.
- **Fix direction:** Document the chosen approach (for example a fund-level HWM with explicit disclosure for v1). Consider time-averaged NAV for crystallisation, or require a proven, non-issuer attestation at crystallisation.

**P6 — Fee minting is not propagated to the Uniswap wrapper and NAV guard**
- **Severity:** major
- **Where:** Architecture section; R6.
- **What is wrong:** Fee units minted on PFTL change the NAV per unit. The page does not say how the Ethereum wrapper (wB24), `NAVGuardHook` and `PFTLUniswapPrimaryMarketV2` learn about the post-fee NAV. It also does not say whether fee units can be exported.
- **Why it matters:** The Uniswap pool could trade against a stale pre-fee NAV, creating arbitrage against holders in a live, real-money pool.
- **How to investigate:** Trace how NAV reaches the hook through `crates/pftl_uniswap_proofs`.
- **Fix direction:** Make `nav_after` the only NAV exported to the proofs, and add a cross-chain acceptance test.

**P7 — No migration plan for live spread policies**
- **Severity:** major
- **Where:** R1 ("unify today's route multipliers").
- **What is wrong:** `PftlUniswapPrimaryMarketPolicyV2` is live with real money: A666 uses 10050/9995, and the B24 pool is live. Replacing it with `NavFeeScheduleV1` fields has no versioning, dual-read or migration step. The multiplier representation also differs from the bps representation.
- **Why it matters:** There is a risk of breaking or changing fees on live assets.
- **How to investigate:** Find every reader of the multiplier fields, on chain, in contracts and in Task Node (`/api/l1/navcoin/fee`).
- **Fix direction:**
  - Define a mapping (multiplier = 10000 ± bps).
  - Run a migration operation.
  - Keep backward-compatible reads.
  - Add an acceptance criterion that existing assets keep identical spreads.

**P8 — Mixed schedule changes and non-rate fields are unhandled**
- **Severity:** major
- **Where:** R3; user story step 5.
- **What is wrong:** The "increase with notice, decrease immediately" rule is undefined when:
  - one field rises while another falls;
  - the crystallisation frequency changes (more frequent crystallisation is effectively an increase);
  - the recipient changes;
  - `increase_notice_epochs` itself is lowered.
- **Why it matters:** Issuers could bypass the notice protection.
- **Fix direction:** Apply the rule field by field. Treat any change that could raise fees (including frequency, basis, recipient and notice-period reductions) as requiring notice. Allow at most one pending schedule at a time.

**P9 — Notice period may not protect holders in practice**
- **Severity:** minor/major
- **Where:** User story step 5.
- **What is wrong:** The notice period assumes holders can redeem in time. Redemption depends on primary-market windows, fix capacity and issuer-controlled redeem spreads. Nothing guarantees redemption remains available during notice.
- **Fix direction:** During a pending increase, freeze the redeem spread at its old value and guarantee minimum redemption capacity. Also set a protocol-wide minimum notice period, which answers one of the open questions.

**P10 — Acceptance criterion 1 is inconsistent with the example schedule**
- **Severity:** minor
- **Where:** Acceptance criteria 1.
- **What is wrong:** The example schedule crystallises quarterly, but the test runs over three epochs. The performance fee may never crystallise, so the test would not exercise it.
- **Fix direction:** Specify per-epoch crystallisation on devnet, plus a NAV path that includes a drawdown and a recovery so the HWM logic is exercised. Require a Python reference implementation and published test vectors.

**P11 — Accounting treatment of the two fee channels is not reconciled**
- **Severity:** minor
- **Where:** "What exists today" (non-NAV fee custody) compared with R5 (dilution).
- **What is wrong:** Spreads go to custody, outside NAV. Management and performance fees dilute. The explorer's NAV-before-to-NAV-after view and `primary-market-accounting.md` need a unified ledger.
- **Fix direction:** Update the accounting doc so it covers all fee channels. Show both channels on the explorer.

**P12 — Fix fee, hurdle and protocol take are left out**
- **Severity:** minor
- **Where:** R1; open questions.
- **What is wrong:**
  - `FxFixPacketV1.fee_bps` is not part of the schedule.
  - There is no hurdle rate.
  - There is no discussion of a protocol or validator share of fees.
- **Why it matters:** Founder intent ("set the type of fees") and the economics of validator and ecosystem incentives.
- **Fix direction:** Add `fix_fee_bps` and an optional hurdle. Add an open question about a protocol fee share.

**P13 — Explorer dependency is vague**
- **Severity:** minor
- **Where:** Dependencies table ("Explorer for the new chain").
- **What is wrong:** Explorer rendering is the founder's explicit requirement, but there is no owning pillar, spec or schema.
- **Fix direction:** Define the receipt schema the explorer consumes and link it to the Pillar 2 message spec and an explorer work item.

**P14 — Regulatory and KYC coupling is underexplored**
- **Severity:** minor
- **Where:** Dependencies (Pillar 8).
- **What is wrong:** Charging performance fees on a pooled vehicle has adviser and regulatory implications, which the page reduces to "some will be KYC-gated".
- **Fix direction:** Add an open question on whether performance fees require identity or KYC gating. Add a disclosure document requirement through `pf.navcoin.fee.*` messages.

**P15 — Code claims are unverified**
- **Severity:** minor
- **Where:** "What exists today" table.
- **What is wrong:** File paths, type names and constants (for example `PFTL_UNISWAP_MAX_PRIMARY_SPREAD_BPS = 500`, the 0–500 bps route) should be checked against the repositories.
- **Fix direction:** An agent should grep the repos and correct any drift before implementation.
