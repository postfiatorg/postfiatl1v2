# Feedback — Pillar 7 — Transparent Fees for NAVCoins

*Review round 3.*

> **Mean score:** 72/100 · Gemini 4 Neon 64 · Claude Opus 5.5 80
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/07-navcoin-fees.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 64/100

*Model `google/gemini-4-neon` · 42557 output tokens*

#### Summary judgement

This sub-page is unusually well-grounded in the `postfiatl1v2` and `tasknode` codebases, clearly maps the founder's five NAVCoin primitives to fee surfaces, and gets several subtle institutional-finance mechanics right (why ongoing fees must be paid by minting units rather than cutting NAV, why $\Delta S = F \cdot S / (V - F)$ avoids dilution underpayment, why management fees need remainder carry across frequent finalisations, and why per-holder equalisation breaks ERC-20/Uniswap/shielded fungibility).

However, **it is not ready to implement as written**. Underneath the polished presentation are multiple **critical economic exploits and mathematical bugs**:
1. **Option B is economically broken**: an issuer (or any arbitrageur) can force-crystallise ~99% of the provisional performance fee $P$ at any peak via zero-spread mint/redeem churn, defeating Option B's sole purpose. Option B also contains a double-deduction order-of-operations bug at crystallisation, retroactively applies performance-fee increases to prior unrealised gains, and omits the math for reconciling submission-to-finalisation dealing.
2. **R10's supply-headroom rule (`S >= issued_asset_supply`) undermines R8 and breaks Option A's HWM**: because `circulating_supply` in a packet can exceed `issued_asset_supply`, an issuer can inflate $S$ on Epoch 1 to set an artificially low high-water mark $H$, then drop $S$ back to actual supply on Epoch 2 to extract a fake performance fee on zero actual return—and can depress $N$ on any epoch as a hidden fee.
3. **The notice-period exit guarantee (R4 + R5) can be bypassed**: R5 blocks route exits after a spread *decrease* until the issuer re-pins, and R4's "live block" clock keeps ticking even when routes are unpinned, past the 100-block pricing window, out of capacity, or when the issuer stops settling `nav_redeem_at_nav`.
4. **Acceptance Criterion 6 is mathematically false**: splitting an interval into more finalisations does *not* leave minted management fee units within one atom per finalisation—compounding creates divergences of thousands to millions of atoms, so every faithful implementation will fail AC6.
5. **Consensus & cross-chain gaps**: `NavFeeAccrualV1` has unspecified edge-case rules (`clipped_value_atoms`, `mgmt_remainder` resets, live-block counter state transitions), `nav_fee_claim` lacks signer authority and per-recipient accounting, R11 does not verify whether Ethereum's `NAVGuardHook.sol` / `PFTLUniswapPrimaryMarketV2.sol` read `NavTrackedAsset.nav_per_unit` or `packet.nav_per_unit`, and storing per-epoch accruals in state/unpaginated RPCs will blow up at 100k–300k finalisations/year.

Fixing these issues—and pruning the 12 open questions down to an opinionated v1 default—would bring this spec to production quality.

---

#### Correspondence to the founder's intention

##### What aligns well
- **Deliverable 7 core user story is squarely hit**: Deterministic management fees, performance fees with a persistent high-water mark, transparent display in Task Node, issuer controls, and representation of schedule changes and accruals on the block explorer are all explicitly specified.
- **NAVCoin 5-element primitive framing**: The table mapping the five founder-specified elements (Uniswap wrapper, PFTL asset, USDC→pfUSDC bridge, wrapper→actual exit, scheduled London-FX-style fix) to current code and fee surfaces is directly faithful to the original prompt.
- **SDK and docs**: R17–R19 explicitly scope a Python/Rust NAVCoin SDK and a "Create a NAVCoin" guide on `postfiatorg.github.io`, matching the founder's instruction that NAVCoins be a primitive any user can ramp up on.
- **Honest scope boundaries**: Open Questions 11 and 12 transparently flag that operator-only creation, the Hyperliquid-on-Solana NAVCoin, and scheduled fix settlement are not yet built or owned.

##### Where correspondence falls short
- **Unowned near-term founder priorities**: The founder framed NAVCoins around two immediate goals—*"getting NAVCoin swaps available. This likely means tracking a Hyperliquid Navcoin on Solana"* and *"a primitive that any user can build"*—yet this spec delivers neither permissionless creation nor Solana route support, and pushes ownership of Hyperliquid-on-Solana and scheduled fix settlement to Open Question 12. Even R17's `create_navcoin(...)` only wraps PFTL registration + fee schedule; it does not cover the rest of the 5-element primitive, and remains unusable by third-party creators while the operator-only release rule stands.
- **Ethereum-only spread enforcement (R5)**: R5 binds schedule spreads exclusively to `PftlUniswapPrimaryMarketPolicyV2`. If the near-term Hyperliquid NAVCoin launches on Solana rather than (or alongside) Ethereum/Uniswap, R5 has no route-agnostic spread enforcement mechanism.
- **Decision overload instead of opinionated shipping defaults**: The founder is asking what set of activities maximizes Post Fiat's likelihood of success in the near term. Returning 12 multi-part open questions (~25 distinct decisions)—including a fundamental architectural fork (Option A vs Option B) with no clear recommendation—creates planning drag rather than a crisp executable plan.

---

#### Technical robustness

##### Strengths
- Accurate inventory of existing Rust types, execution handlers, Solidity contracts, Task Node routes, and Python builders.
- Correct recognition that `BlockHeader` has no timestamp and that existing `u128` mul-div helpers cannot safely hold products of three `u64` inputs (requiring `U256` intermediates).
- Careful backwards-compatibility plan for live assets (A666) preserving `policy_hash` and existing Ethereum mint packet formats.
- Good defensive invariants in principle: accrual never fails `nav_epoch_finalize`, unclaimed fees go into a compile-time-classified custody lane counted by `issued_asset_supply`, and Option A's worked example arithmetic is exact to the atom.

##### Weaknesses
- **Option B is underspecified and buggy in four distinct places**: double-deduction of `P` in `N` on crystallisation, missing formulas for submission-to-finalisation reconciliation, missing `N == 0` guard / `B` `u64` overflow / `p > 50%` supply-expansion-on-exit checks, and undefined mid-interval `p` changes.
- **Consensus-critical determinism gaps**: `clipped_value_atoms` has no formula; `mgmt_remainder` behavior on `S = 0`, `V = 0`, `F >= V`, `m = 0`, and clamping is unspecified; the live-block counter lacks an explicit state transition formula across halt/unhalt/deadman/finalisation; `nav_fee_claim` signer authority and partial-claim semantics are missing.
- **False acceptance criterion (AC6)**: Asserts atom-level invariance of minted units under interval splitting, contradicting the compounding behavior of the spec's own Option A formula.
- **Unverified Ethereum/PFTL pricing sync (R11) and SP1 deduction compatibility (R8)**: Keeping `packet.nav_per_unit` at `N_before` while setting `NavTrackedAsset.nav_per_unit` to `N_after` risks pricing divergence if Ethereum contracts read `packet.nav_per_unit`. Adding a typed `deduction` field in R8 is not reconciled with SP1 prover constraints or attestor signatures.
- **State bloat and unpaginated RPCs (R12, R15)**: Route NAVCoins finalise at least every 100 blocks (~100k–300k/year). Putting full `NavFeeAccrualV1` history into the state commitment or returning unbounded arrays from `nav_fee_history(asset_id)` will degrade node and explorer performance.

---

#### Economic coherence

##### Strengths
- **Option A dilution and HWM math are sound**: Computing management fee before performance fee prevents charging performance fees on management fees; minting $\lfloor F \cdot S / (V - F) \rfloor$ accounts for post-mint dilution; updating $H' = \max(H, N_{\text{after}})$ even when $p = 0$ closes the "toggle $p$ to 0 to reset HWM" exploit.
- **Clean channel separation**: Separating settlement-asset primary-market spreads (outside holder value) from minted-unit ongoing fees (dilutive inside holder value) is coherent and avoids impossible cross-chain reserve transfers.
- **Good instinct on transition intervals**: Charging the lower of old and new rates during an interval where a rate changes protects holders on both increases and decreases (under Option A).

##### Weaknesses
- **Fatal flash-churn exploit in Option B**: Zero-spread mint/redeem (`nav_mint_at_nav` / `nav_redeem_at_nav`) allows the issuer to force-crystallise arbitrarily close to 100% of provisional liability `P` on any intraday peak before crystallisation, destroying Option B's economic premise.
- **Fatal supply-headroom exploit (R10 vs R8)**: Allowing `packet.circulating_supply > issued_asset_supply` lets an issuer depress $N$ without a declared deduction (hidden fee) and game Epoch 1's HWM to harvest performance fees on zero return.
- **Bypassable notice-period exit protection (R4 + R5)**: Holders cannot reliably exit during a pending fee increase if the issuer (a) triggers a 1 bps spread decrease and refuses to re-pin the route (which R5 turns into an exit freeze), (b) lets the 100-block route pricing window lapse while staying inside a longer `max_epoch_gap_blocks` deadman, (c) zeroes route cash-in capacity, or (d) refuses to settle `nav_redeem_at_nav`.
- **Single-lane recipient confiscation risk (R9 + Q7)**: Because unclaimed fees sit in one shared lane per asset with no per-recipient balance tracking, changing `recipient` redirects already-earned fees away from the previous recipient.
- **Unaddressed volatility cost of Option A on near-term crypto NAVCoins**: Because route NAVCoins must finalise at least every 100 blocks (every few minutes), Option A crystallises performance fees on every transient peak. On a volatile Hyperliquid crypto NAVCoin, a +20% spike followed by a -20% pullback within a week charges holders a full performance fee on a net-losing position.
- **Minor economic flaws**: Ratcheting $H'$ when $p > 0$ and $F_p$ floors to `0` leaks performance fees on small-supply/high-NAV assets; Open Question 4's idea of exempting unclaimed fee units from management fees would break fungibility or incentivize never claiming; the Option B comparison table misstates late-entrant pooled-mark risk (entering *above* the mark when $P > 0$ also exposes late entrants to performance fees on a dip-and-recovery back to their entry price).

---

#### Problems

##### P1 — Option B has a critical flash-mint/redeem exploit that force-crystallises provisional performance fees
- **Severity:** critical
- **Where:** *Fee math and economics → Entering and exiting between finalisations* (Option B table and discussion)
- **What is wrong:** Option B's exit rule crystallises a pro-rata share $c = \lfloor P \cdot y / S_B \rfloor$ of the provisional liability $P$ into permanent minted fee units whenever $y$ atoms exit. Because an entry of $x$ atoms adds $x$ to $S_B$ without changing $P$, anyone who enters $x$ atoms and immediately exits $y = x$ atoms crystallises $P \cdot x / (S_B + x)$ of the *existing holders'* provisional liability into permanent fee units. Worse, the issuer can execute `nav_mint_at_nav` and `nav_redeem_at_nav` with **zero spread**. Example: with $S_B = 100, V_B = 120, B = 100, P = 2, N = 1.18$, minting $x = 10{,}000$ and redeeming $y = 10{,}000$ sets $c = \lfloor 2 \times 10{,}000 / 10{,}100 \rfloor = 1.980198$ (99% of $P$) and mints $1.678134$ permanent fee units immediately, even if reserves crash back to $1.00$ the next block. Even without malice, routine market-maker/arbitrage creation and redemption churn continuously drains $P$ into permanent fee units.
- **Why it matters:** Option B's entire justification—and its heavy engineering cost (R13, touching every supply path)—is that losses net off before crystallisation and that the issuer cannot harvest transient peaks. This exploit completely invalidates that guarantee.
- **How an agent should investigate:** Simulate a mint-then-redeem cycle in Python against the Option B equations (starting from $S_B = 100, V_B = 120, B = 100, P = 2$, followed by a reserve drop to $V = 100$) and verify that the original holder's net value falls below $100$ while the fee recipient retains permanent units. Check whether any cooldown, fee, or spread restricts `nav_mint_at_nav` + `nav_redeem_at_nav` in `crates/execution/src/nft_escrow_asset_execution.rs`.
- **Suggested direction for the fix:** Either (a) drop Option B and adopt Option A with an explicit crystallisation/watermark schedule (e.g., performance fee evaluated only on periodic crystallisation epochs, accepting the small entry/exit timing distortion, or using a trailing-minimum NAV / non-issuer-attested periodic checkpoint), or (b) if retaining Option B, do not mint permanent fee units on every exit—instead retain $c$ inside a refundable/escrowed reserve or require a crystallisation-window lock / minimum holding period—noting that fungible pools cannot cleanly distinguish churn from genuine exits without breaking fungibility.

##### P2 — Supply headroom (`circulating_supply > issued_asset_supply`) bypasses R8 and allows Epoch-1 HWM manipulation
- **Severity:** critical
- **Where:** *Technical requirements → R8 and R10*
- **What is wrong:** R8 derives $N = \lfloor V \cdot U / S \rfloor$ to prevent issuers from stating NAV below reserves as a hidden fee. However, R10 only rejects packets where `packet.circulating_supply < issued_asset_supply`, permitting `S > issued_asset_supply` ("proof-backed headroom"). An issuer can inflate $S$ above actual supply to depress $N$ arbitrarily without declaring a deduction. Worse, on the first finalisation after attach (`H` uninitialised), an issuer can submit $S = 2 \times \text{issued\_asset\_supply}$ so $H$ is initialised at $0.5 \times \text{true NAV}$, then submit $S = \text{issued\_asset\_supply}$ on the next epoch so $N$ doubles back to true NAV—harvesting a massive performance fee on zero real gains.
- **Why it matters:** Destroys both the anti-hidden-fee guarantee (R8) and the integrity of the high-water mark, allowing direct theft of holder value on any NAVCoin where the issuer can choose headroom.
- **How an agent should investigate:** Inspect `crates/execution/src/nft_escrow_asset_execution.rs` and `crates/nav_reserve_protocol` to determine how `packet.circulating_supply` is proved/validated today, whether SP1 binds `packet.circulating_supply` strictly to on-chain `issued_asset_supply`, and whether `NavMintAtNavOperation` genuinely requires `packet.circulating_supply > issued_asset_supply`.
- **Suggested direction for the fix:** Separate the **mint cap** (`mint_headroom_atoms`) from **circulating supply** (`S`) used for NAV and fee calculations. Require `S == issued_asset_supply` (or exact on-chain supply at the packet's observation height, proved in consensus) for all NAVCoins with a fee schedule, and represent any pre-funded mint capacity in an explicit separate field that does not dilute $N = \lfloor V \cdot U / S \rfloor$.

##### P3 — R4 + R5 allow an issuer to trap holders from exiting during a fee-increase notice period
- **Severity:** critical
- **Where:** *Technical requirements → R4 and R5*, *Open questions → Q6*
- **What is wrong:** Three interacting mechanisms let an issuer advance the R4 "live block" notice clock to completion while preventing holders from exiting on the old terms:
  1. R5 states: *"After a decrease, a route on higher multipliers cannot issue or exit until it re-pins."* An issuer can submit a 1 bps spread decrease (applies at next finalisation), refuse to re-pin the route (blocking all route exits), then submit a large fee increase and keep finalising—live blocks tick to completion while route exits remain frozen.
  2. Live blocks check only `!halted && !deadman_tripped`. Routes stop allowing exits after `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS = 100` blocks without finalisation, or when a route is unpinned after a finalisation, or when route cash-in capacity is zero—all while `max_epoch_gap_blocks` may be much larger and `!halted` remains true.
  3. Direct `nav_redeem_at_nav` requires issuer settlement ("which the issuer settles"), so it is not a unilateral exit fallback.
- **Why it matters:** The notice period's sole economic purpose is giving holders a real exit window on the old fee terms before higher fees take effect. Without exit liveness, the notice period is security theater.
- **How an agent should investigate:** Check `crates/execution/src/nav_vault_asset_execution.rs` to verify how route exits (`PftlUniswapPrimaryRedeem`) check multipliers, `policy_hash`, pricing age, and capacities, and check `nft_escrow_asset_execution.rs` for `NavRedeemAtNavOperation` settlement mechanics.
- **Suggested direction for the fix:**
  1. Never block *exits* on a spread decrease: for PFTL-side route exits, read the active `min(route_redeem_spread, schedule.redeem_spread_bps)` directly from `NavFeeScheduleV1` (only block *issues* if route multipliers exceed the schedule, or decouple PFTL redeem spread from Ethereum `policy_hash`).
  2. Tighten the live-block definition (or add an exit-liveness condition) for route NAVCoins: blocks count as live during a pending increase only while at least one exit route is pinned, within `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS`, and has non-zero cash-in capacity (and bar reducing cash-in capacity while an increase is pending), or pause the notice clock whenever `nav_redeem_at_nav` requests sit unsettled past a timeout.

##### P4 — Multiple mathematical and specification bugs inside Option B
- **Severity:** major
- **Where:** *Fee math and economics → Entering and exiting between finalisations* (Option B table) and *R10*
- **What is wrong:** Beyond the P1 exploit, Option B has four concrete math/spec errors:
  1. **Double-deduction at crystallisation:** The Crystallisation row says *"As above, but one mint of `floor((F_m + P) × S_B / (V_B − F_m − P))` units pays both. If `P > 0`, `B = V_B`. Then `P = 0`."* Because "As above" computes `N = floor((V_B - P) * U / S_B)`, computing `N` before setting `P = 0` deducts `P` twice (in the worked example $S_B = 100, V_B = 120, P = 2$, it gives $118 / 101.694915 = 1.16033$ instead of $120 / 101.694915 = 1.18$).
  2. **Retroactive performance-fee increase:** If a pending increase raises `p` (e.g., 10% → 20%) mid-crystallisation-interval, the next finalisation applies 20% to the entire `V_B - B`, retroactively charging the higher rate on unrealised gains earned before the increase took effect.
  3. **Missing reconciliation formulas for submission-to-finalisation dealing:** R10 says *"Finalisation computes against that snapshot, then carries forward pro rata the units that exited and the value that entered since"*, but provides no formulas for reconciling fee units already minted by exits during the submission-to-finalisation window (at old `N` and old `P`) with the newly computed `P` and `N`.
  4. **Missing boundary guards & integer widths:** The exit rule divides by `N` (`floor(c * U / N)`) without a guard for `N == 0`; `B` can overflow `u64` when `S_B` is near `u64::MAX` and `N > U`; and if `p > 50%`, `c / N` can exceed `y`, causing an exit to *increase* total supply (violating supply/cap assumptions).
- **Why it matters:** Any engineer implementing Option B from this spec will either write buggy consensus math or have to invent consensus rules ad hoc.
- **How an agent should investigate:** Write a reference Python prototype of Option B covering crystallisation, mid-interval `p` changes, submission-to-finalisation dealing, `p = 60%`, and `N = 0` edge cases, and assert conservation of holder value + `P` + fee units = `V_B`.
- **Suggested direction for the fix:** If Option B is kept at all (see P1), explicitly reorder crystallisation so `P = 0` *before* computing `N = floor(V_B * U / S_B)`; mandate forced crystallisation at the old rate whenever `p` changes; write exact integer formulas for submission-to-finalisation reconciliation; cap `p <= 5,000` bps; guard `c > 0 && N > 0`; and specify `u128` for `B`, `V_B`, and `P`. Better yet, make Option A (or periodic-checkpoint Option A) the sole recommended design and remove Option B from v1.

##### P5 — Acceptance Criterion 6 is mathematically false due to management-fee compounding
- **Severity:** major
- **Where:** *Acceptance criteria → Item 6*
- **What is wrong:** AC6 states: *"Splitting the same blocks into more finalisations leaves total management fee value unchanged and units within one atom per finalisation."* While total `F_m` (sum of value atoms) is preserved by the remainder `r`, total minted units $\Delta S$ is **not** preserved within 1 atom per finalisation because each finalisation compounds on the previously minted fee units. Using the spec's own parameters ($V = 10^{12}, S = 10^{12}, m = 100\text{ bps}, Y = 10^6, \Delta h = 10{,}000$):
  - 1 finalisation yields $\Delta S = \lfloor 10^8 \times 10^{12} / (10^{12} - 10^8) \rfloor = 100{,}010{,}001$ atoms.
  - 2 finalisations of $\Delta h = 5{,}000$ yield $\Delta S_1 = 50{,}002{,}500$ and $\Delta S_2 = 50{,}005{,}000$, summing to $100{,}007{,}500$ atoms—a **2,501-atom discrepancy** (and over a full year at 200 bps, a **20+ bps** discrepancy of millions of atoms).
- **Why it matters:** A provably false acceptance criterion guarantees test failures or forces engineers to hack around a broken spec requirement. It also masks a minor economic incentive: finalising *less* frequently mints slightly more fee units ($1/(1-x) > (1-x/k)^{-k}$).
- **How an agent should investigate:** Run the two-line integer check above in Python for $k = 1, 2, 10, 100$ finalisations over $\Delta h = 10{,}000$ and $\Delta h = Y$.
- **Suggested direction for the fix:** Rewrite AC6 to assert exact invariance of $\sum F_m$ (within 1 valuation atom) and assert that total minted units match the closed-form compounding bound $S \cdot ((1 - F_m/(kV))^{-k} - 1)$ within $k$ atoms, OR—if strict unit-invariance under splitting is desired—note the exact second-order compounding tolerance ($\approx \frac{1}{2}(m \Delta h / Y)^2 S$).

##### P6 — `nav_fee_claim` signer authority is unspecified, and single-lane accounting allows fee confiscation on recipient change
- **Severity:** major
- **Where:** *User story #6*, *Architecture*, *Technical requirements → R1, R9, R14*, *Open questions → Q7*
- **What is wrong:** R9 introduces `nav_fee_claim` but never specifies **who may sign it** (the issuer, the `recipient`, or either), nor whether partial claims are supported. Meanwhile, the architecture uses a single unclaimed fee lane per asset rather than per-recipient balances. If the issuer changes `recipient` (via `nav_fee_schedule_set`) while unclaimed fee units exist, the previous recipient's already-accrued fees are silently redirected to the new recipient.
- **Why it matters:** Breaks third-party manager/partner fee splits (and any future protocol/validator share from Q8), and leaves a critical authorization rule unspecified for execution implementation.
- **How an agent should investigate:** Check `crates/execution/src/issued_asset_ledger_helpers.rs` and `crates/types/src/account_owned_asset_types.rs` to see how custody lanes and claim/withdraw operations are keyed and authorized in `postfiatl1v2`.
- **Suggested direction for the fix:** Explicitly specify `nav_fee_claim(asset_id, amount_or_all)` signer authority (allow `recipient` to sign claims directly, and optionally allow the issuer to push-claim *only to the credited recipient*). Either key unclaimed fee balances by `(asset_id, recipient_account)` or disallow changing `recipient` while the unclaimed balance is non-zero (or auto-snapshot/lock the prior balance to the prior recipient).

##### P7 — Unverified cross-chain pricing consistency (Ethereum `NAVGuardHook.sol` / `PFTLUniswapPrimaryMarketV2.sol`) and SP1 deduction compatibility
- **Severity:** major
- **Where:** *Technical requirements → R8 and R11*
- **What is wrong:**
  1. R11 states that `NavTrackedAsset.nav_per_unit` becomes `N_after`, while `packet.nav_per_unit` remains `N_before` so the packet hash is unchanged. It asserts *"Every route and every mint and exit at NAV already reads the tracked-asset value"*, which covers PFTL execution, but does **not** verify what Ethereum's `NAVGuardHook.sol`, `PFTLUniswapPrimaryMarketV2.sol`, and `crates/pftl_uniswap_proofs` read. If Ethereum verifies `NavReservePacket` / `NavReservePublicValues` directly, Ethereum will enforce `N_before` while PFTL enforces `N_after`—causing arbitrage or rejected Uniswap swaps immediately after any fee accrual.
  2. R8 adds a typed `deduction` field with a `reason_hash` to reserve packets, without specifying whether `deduction` is attested/proved inside `NavReservePublicValues` (requiring SP1 guest/prover changes in `crates/nav_reserve_protocol` and `tools/nav-reserve-proof`) or merely submitted by the issuer (which would let the issuer inject arbitrary unverified deductions to suppress NAV or manipulate HWM).
- **Why it matters:** A PFTL↔Ethereum NAV mismatch breaks the live B24/Uniswap integration, and an unverified `deduction` field defeats R8's anti-hidden-fee goal.
- **How an agent should investigate:** Read `crates/ethereum-contracts/src/NAVGuardHook.sol`, `PFTLUniswapPrimaryMarketV2.sol`, `crates/pftl_uniswap_proofs`, and `crates/types/src/nav_reserve_public_values.rs` to trace the exact source of `nav_per_unit` on Ethereum and the fields covered by SP1/attestor signatures.
- **Suggested direction for the fix:** Confirm in the spec (and add an acceptance criterion) that Ethereum pricing commitments bind to `NavTrackedAsset.nav_per_unit` (`N_after`), updating `pftl_uniswap_proofs` if they currently bind to `packet.nav_per_unit`. Require R8's `deduction` field to be signed by the proof/attestor quorum (not solely the issuer) and add `crates/nav_reserve_protocol` + `tools/nav-reserve-proof` to the build order and dependencies.

##### P8 — State-commitment bloat and unpaginated RPC (`nav_fee_history`) at 100k–300k finalisations/year
- **Severity:** major
- **Where:** *Technical requirements → R12 and R15*
- **What is wrong:** Route NAVCoins must finalise at least every 100 blocks (`MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS = 100`), producing ~100,000–300,000 `NavFeeAccrualV1` records per asset per year. R12 says *"A `NavFeeAccrualV1` record goes into the finalisation receipt and the state commitment"* without clarifying whether state holds only the *latest* accrual (plus a rolling accumulator) or an append-only history. Meanwhile, R15 defines `nav_fee_history(asset_id)` with no `from_epoch`, `to_epoch`, `limit`, or `cursor` parameters.
- **Why it matters:** Storing unbounded per-epoch records in consensus state bloats the state trie permanently, and an unpaginated `nav_fee_history(asset_id)` RPC will time out or OOM nodes and the block explorer within months of launch.
- **How an agent should investigate:** Check existing RPC pagination conventions in `crates/rpc_sdk` and receipt/state commitment layouts in `crates/types/src/transactions_mempool_receipts.rs` and `account_owned_asset_types.rs`.
- **Suggested direction for the fix:** Specify that consensus state stores only the *latest* `NavFeeAccrualV1` (or cumulative totals + rolling hash) while full per-epoch `NavFeeAccrualV1` records live in execution receipts and explorer/indexer storage. Update R15 to specify paginated RPC signatures: `nav_fee_history(asset_id, after_epoch, limit, kind_filter)` with a hard page-size cap.

##### P9 — Missing deterministic consensus specifications for edge cases (`clipped_value_atoms`, `mgmt_remainder`, live-block counter, `disclosure_ref`)
- **Severity:** major
- **Where:** *Fee math and economics → Every finalisation*, *Technical requirements → R4, R12*
- **What is wrong:** Several fields committed to consensus state and receipts lack exact integer rules:
  1. `clipped_value_atoms` in R12 has no formula when `ΔS` is clamped at `max_supply` or `u64::MAX`.
  2. `mgmt_remainder` (`r`) behavior is unspecified when `S = 0`, `V = 0`, `F >= V`, `m` changes to `0`, or `ΔS` is clamped, and its integer width (`u64` vs `u128`) is unstated.
  3. The live-block counter in R4 has no explicit state fields or interval-overlap formula for handling halt-before-deadman, deadman-before-halt, `nav_epoch_finalize` clearing a halt, pending-set while halted/past-deadman, and clearing a pending increase when a pure decrease is submitted.
  4. R4 does not classify whether changing `disclosure_ref` alone is a pure decrease/immediate event or an increase requiring notice.
- **Why it matters:** Every one of these is part of consensus state transitions; ambiguity causes Rust/Python reference mismatches (failing AC1) or consensus bugs.
- **How an agent should investigate:** Trace every branch of `nav_epoch_finalize`, `nav_halt`, and `nav_fee_schedule_set` against `NavFeeAccrualV1` fields to enumerate all state transitions.
- **Suggested direction for the fix:** Add a concise deterministic state-transition table specifying: exact `clipped_value_atoms` formula (e.g., `F - floor(ΔS_clamped * N_after / U)` or `F - floor(ΔS_clamped * (V - F) / S)`), `r` reset rules (reset `r = 0` when `m == 0` or `S == 0` or `V == 0`; keep `r` unchanged when `F >= V`; type `u128`), exact live-block counter fields (`pending_live_blocks`, `live_counted_through_height`) and overlap formula with `[last_finalize_height, last_finalize_height + max_epoch_gap_blocks]`, and explicit R4 classification of `disclosure_ref` and pure-decrease-cancels-pending-increase.

##### P10 — Excessive unresolved open questions (12 questions / ~25 sub-decisions) and missing route abstraction for Solana/non-Uniswap NAVCoins
- **Severity:** major
- **Where:** *The NAVCoin primitive these fees attach to*, *Technical requirements → R5, R17*, *Open questions for the founder*
- **What is wrong:**
  1. The spec punts 12 multi-part open questions to the founder without recommending concrete v1 defaults for core choices (Q1 method, Q2 caps/notice values, Q3 block-height clock & live-only `Δh`, Q4 fee base, Q5 peak protection).
  2. R5 hard-codes spread enforcement to `PftlUniswapPrimaryMarketPolicyV2`, providing no abstraction for the founder's immediate near-term target: a Hyperliquid NAVCoin on Solana.
  3. The SDK (R17) promises `create_navcoin(...)`, yet lifting operator-only creation and connecting the remaining 4 elements of the NAVCoin primitive (wrapper/pool, pfUSDC bridge, route, fix) are left unowned (Q11, Q12).
- **Why it matters:** Blocks execution waiting for founder decisions on low-level parameters, risks building Ethereum-only spread plumbing right before a Solana NAVCoin launches, and leaves the founder's "any user can build a NAVCoin" vision without a clear owner.
- **How an agent should investigate:** Review sibling pillars (especially README/Outcome C and Pillar 8) to determine where Hyperliquid-on-Solana, scheduled fix settlement, and permissionless NAVCoin creation belong, and check current PFTL block times to propose concrete constants for Q2 and Q3.
- **Suggested direction for the fix:** Convert Open Questions into a **"Proposed v1 defaults (override if needed)"** table with crisp recommendations (e.g., Option A with periodic performance-fee epochs or Step-1 `p = 0`; concrete caps like `m <= 500`, `p <= 2,500`, spreads `<= 500`, fix cap `<= 100`, notice `>= 7 days` in live blocks; live-only `Δh`; charge on `V` with proven/attested split disclosed). Generalise R5 from "PFTL-Uniswap route" to a trait/interface covering any primary-market route (Ethereum Uniswap and future Solana routes), and explicitly assign ownership for opening NAVCoin creation, Hyperliquid-on-Solana, and scheduled fix settlement in the parent planning doc.

##### P11 — Option A performance-fee rounding leak ratchets `H` upward when `F_p` floors to zero
- **Severity:** minor
- **Where:** *Fee math and economics → Every finalisation (Option A)* (Steps 3 and 5)
- **What is wrong:** In Step 3, $F_p = \lfloor g \cdot S \cdot p / (10{,}000 \cdot U) \rfloor$. When $p > 0$ and $g > 0$ is small enough that $g \cdot S \cdot p < 10{,}000 \cdot U$ (e.g., a high-unit-price or early-stage NAVCoin with $S < 10{,}000 \cdot U / p$, such as $S < 10$ whole units at $p = 1{,}000$ bps), $F_p$ floors to `0`, yet Step 5 still updates $H' = \max(H, N_{\text{after}}) = N_m > H$. Over many small 100-block finalisations, $N$ and $H$ can climb arbitrarily while collecting zero performance fee.
- **Why it matters:** Subtle revenue leak for small-supply / high-unit-value NAVCoins; inconsistent with the care taken to preserve management-fee precision via remainder `r`.
- **How an agent should investigate:** Test Option A in Python with $U = 10^6, S = 5 \times 10^6$ (5 units), $p = 1{,}000$, incrementing $V$ by $5$ atoms ($g = 1$ atom) across 1,000 finalisations.
- **Suggested direction for the fix:** When `p > 0`, only advance `H'` if `F_p > 0` (leaving `H` unchanged when `g > 0` produced `F_p == 0`, so tiny gains accumulate until `g` is large enough to produce at least 1 valuation atom of fee). Keep the unconditional `H' = max(H, N_after)` update when `p == 0` so gains while `p == 0` remain uncharged.

##### P12 — Economic inaccuracies in Open Question 4 (exempting unclaimed units) and Option B late-entrant description
- **Severity:** minor
- **Where:** *Fee math and economics → Option A vs B table* and *Open questions → Q4*
- **What is wrong:**
  1. Q4 asks: *"Should unclaimed fee units be exempt from the management fee, so the recipient is not charged on its own fees?"* Because management fees are paid by minting fungible units (diluting *all* units equally), you cannot exempt unclaimed fee units simply by excluding them from the fee base `V`—doing so still dilutes the unclaimed units while under-charging holders. Conversely, if you somehow shielded unclaimed units from dilution, you would break fungibility and create a perverse incentive for recipients to *never claim* fee units (leaving them in the unclaimed lane to dodge management fees forever).
  2. The Option A vs B comparison table says under Option B: *"entering below the mark lowers the pooled mark, so holders below their own entry share the charge on its recovery."* It omits the symmetric distortion: entering *above* the mark (when $P > 0$) raises the pooled mark so that if NAV dips slightly and recovers back to the late entrant's own entry price, the late entrant pays performance fee on zero personal gain.
- **Why it matters:** Misleading framing in Q4 and the comparison table could steer the founder toward a broken choice.
- **How an agent should investigate:** Work through a 2-unit numerical example for unclaimed-unit exemption (1 holder unit + 1 unclaimed fee unit, 10% management fee) and a late-entrant-above-mark example ($N = 1.18$, enter at $1.18$, dip to $1.12$, recover to $1.18$).
- **Suggested direction for the fix:** Remove the "exempt unclaimed units" sub-question from Q4 and note directly in the spec why unclaimed fee units must dilute identically to claimed units (to preserve fungibility and avoid an incentive to park fees unclaimed). Update the Option B table row to note both below-mark and above-mark pooled-benchmark distortions.

## Claude Opus 5.5 — 80/100

*Model `anthropic/claude-opus-5.5` · 11245 output tokens*

#### Summary judgement

This is a strong, code-grounded spec. It answers the founder's complaint directly. Today a creator cannot set a deterministic management or performance fee, cannot display a fee, cannot pick a fee type, and nothing reaches the explorer. The page responds with:

- an on-chain `NavFeeScheduleV1`;
- deterministic accrual inside `nav_epoch_finalize`;
- a per-epoch `NavFeeAccrualV1` record;
- a Fees card in Task Node;
- explorer and RPC surfaces;
- an SDK `create_navcoin` that requires a schedule.

The page ties fees to all five NAVCoin elements the founder listed. It correctly reads what exists today: spreads capped at 500 bps, no fee fields, no timestamp, `u128` mul-div only. The fee math is checkable, and I recomputed both worked examples and they hold:

- Option A gives F_m = 110, F_p = 9,989, ΔS ≈ 9,265.979 and N_after = 1.089901.
- Option B gives P = 2, mints 1.694915 units, and keeps N = 1.18.

It handles the classic fund-accounting traps:

- minting F/N would underpay;
- no performance fee is charged on the management fee;
- the high-water mark (HWM) cannot be reset by toggling p;
- rounding favours holders;
- fees are paid in units because off-chain reserves cannot be moved.

What keeps it from the high 80s or 90s:

- **Unresolved decisions.** The hardest economic choices are deferred to open questions: the performance method, manipulated peaks, the fee base on attested reserves, and the clock. On the near-term Hyperliquid NAVCoin, the issuer both runs the reserves and finalises. That makes the performance fee structurally gameable until those questions are answered.
- **Unverified claims.** A few technical claims need checking. Examples are whether private routes expose amounts to consensus, and how R8 interacts with existing non-SP1 packets.
- **Dense presentation.** The page is very dense, with 12 open questions and two full fee regimes. That is a usability problem for a founder who wants to say "here is how anyone builds a NAVCoin."
- **Sequencing.** The founder's near-term priority is NAVCoin swaps and permissionless creation. This pillar depends on opening creation but does not drive it.

#### Correspondence to the founder's intention

**Directly addressed:**

- **Deterministic management and performance fees.** These are covered by R1 and R7, with integer-atom math and exact 256-bit intermediates.
- **Setting the fee type.** This is covered by `performance_method`, the spreads, the fix cap and the fee-base fields.
- **Displaying fees.** The Fees card (R14) shows active and pending schedules, recipients, the proven/attested split and the realised annual rate.
- **Explorer representation.** R15 shows attach, set, cancel and claim with old → new values, per-epoch accrual, and fix fee rates.

**Contextually aligned:**

- The page treats a NAVCoin as a primitive anyone can build. This matches the founder's statement that NAVCoins should be "a primitive that any user can build."
- Fees are a required argument of the SDK create call, and a "Create a NAVCoin" guide is planned (R17–R19).
- It maps fees onto the founder's five elements: the Uniswap asset, the PFTL asset, USDC→pfUSDC, wrapper→actual asset, and the fix.
- It links to Pillar 2 (`disclosure_ref` messages) and Pillar 8 (credential-gated claims), which matches the founder's wish that primitives stay "natively contiguous."

**Gaps relative to intent:**

- **Hyperliquid swaps are not owned.** The founder calls Hyperliquid NAVCoin swaps on Solana "very near term." The page only notes that such an asset could launch at Step 1 with p = 0. OQ12 admits no pillar owns this.
- **Creation is still operator-only.** The founder's framing implies third-party creators. The page lists "opening creation" as a dependency and asks OQ11, but gives no path.
- **The fix is not built.** Its fee is a cap on `FxFixPacketV1.fee_bps`, but where fix fees go is unspecified.
- **Possible over-specification.** The founder asked for transparency and determinism. The page delivers an institutional-grade fund-administration engine: two crystallisation regimes, live-block notice clocks and pending-schedule slots. Most of this is justified, but the founder may want a simpler v1, essentially Step 1.

#### Technical robustness

**Strengths:**

- **Atomicity.** Accrual runs inside finalisation and never fails it, using clamped or zero fees with reason codes.
- **Supply accounting.** The fee lane is classified under the existing compile-time lane check, and R10 rejects packets whose supply omits fee units.
- **Rounding.** Every step rounds down, and a management remainder is carried so frequent finalisation neither gains nor loses fee value. Acceptance criterion 6 tests this.
- **Binding.** Spreads are bound to route multipliers (R5), with A666 migrated without changing `policy_hash`, so Ethereum packet formats are unchanged.
- **Notice clock.** It counts only live blocks, so halting or withholding packets cannot run down notice.
- **Build order.** It is staged so Step 1 is independent of the performance-method decision.
- **Testing.** A Python reference implementation with shared test vectors gives independent recomputation.

**Weaknesses:** detailed in P2, P5–P7, P9, P10 and P13–P15 below. In brief:

- the R8 consensus change may break existing packet producers;
- the semantics of S as cap versus issued supply are muddy;
- the claim that private routes expose amounts is unverified;
- the precision/`u64` interaction is impractical at precision 18;
- the 256-bit arithmetic is new code;
- Option B touches every supply path, including shielded and bridge paths;
- the block-height clock is an approximation with no stated tolerance.

#### Economic coherence

**Strengths:**

- Paying fees in units is the right choice for off-chain reserves.
- The dilution-correct mint formula is right.
- The HWM is per unit and permanent, and is set after the performance mint, so the fee is not charged on itself.
- Notice periods protect exit.
- Rejecting NAV stated below reserves (R8) closes a hidden-fee channel.
- Per-holder equalisation is rejected for good reason: fungibility across Ethereum, Uniswap and the shielded pool.

**Weaknesses:**

- **Option A charges at issuer-chosen heights.** On route NAVCoins this means at least every 100 blocks. The issuer also controls or attests the reserve values. A performance fee charged on new per-unit highs, sampled frequently, on attested values controlled by the fee recipient's principal, is a weak protection. The rise-then-fall cost is acknowledged but not quantified.
- **Late-entrant management unfairness.** Management is charged on the full interval's V at finalisation. A holder who entered just before submission pays for blocks they did not hold, up to `accrual_cap_blocks`. If that cap is large, the effect is material.
- **Fee base on headroom and unclaimed units** is ambiguous (P6, P8).
- **Fees are disclosed only on PFTL.** Wrapper holders on Ethereum/Uniswap bear the fees but see disclosure only on PFTL.
- **Only the issuer is fee-earning.** There is no hurdle and no protocol share in v1. That may be fine, but the fee-revenue implications for Post Fiat (validators, PFT) are absent, so "economic coherence for the project" is only partly addressed.

#### Problems

**P1 — Performance fee is gameable on issuer-controlled and attested reserves; deferred without a safe default**

- **Severity:** critical
- **Where:** Option A table; OQ1, OQ4, OQ5; the Near-term paragraph on the Hyperliquid NAVCoin.
- **What is wrong:** The issuer chooses finalisation heights, submits or attests packet values, and receives the fee. Under Option A, any transient upward mark on attested reserves is crystallised permanently into fee units. The page identifies this but leaves every mitigation open: the lower of two NAVs, a non-issuer attestation, or a per-charge cap.
- **Why it matters:** The first real fee-bearing NAVCoin is likely the Hyperliquid one, with mostly `attested_value`. A gameable performance fee would undermine the "transparent fees" story at launch.
- **How to investigate:**
  - Simulate Option A versus Option B on realistic Hyperliquid NAV paths (volatile, mean-reverting) with adversarial finalisation timing.
  - Measure excess fee against a monthly-crystallised benchmark.
  - Check which reserve profiles allow issuer-only attestation.
- **Suggested fix:** Make a protocol default normative now rather than an open question. For example, require a proven share or a non-issuer attestor quorum for `p > 0`, and apply the performance gain to `min(N_this, N_prev)`. Keep Step 2 gated on it.

**P2 — R8's consensus NAV derivation may break existing packets and profiles**

- **Severity:** major
- **Where:** R8, and the "NAV arithmetic" row of What exists today.
- **What is wrong:** Today only the SP1 profile enforces NAV = floor(V×U/S). Under R8, every scheduled asset gets the check, and all assets get a schedule through attach. That changes acceptance rules for live producers such as A666 and B24 tooling. The new typed deduction field is a packet-format change, which conflicts with "packet hash unchanged" (R11) for packets that need deductions.
- **Why it matters:** Live assets and proof tools (`tools/nav-reserve-proof`, `crates/nav_reserve_protocol`) could start failing at finalisation.
- **How to investigate:**
  - Audit recent A666 and B24 packets for the relation of `nav_per_unit` to the floor of reserves over supply.
  - Check whether the packet struct can take an optional field without hash changes.
- **Suggested fix:** Version the packet (V2 with deduction) and apply R8 only to V2 packets or newly attached assets. Add a migration acceptance test against historical packets.

**P3 — Management fee charges late entrants for time they did not hold**

- **Severity:** major
- **Where:** "Entering and exiting between finalisations", Management bullet; acceptance criterion 3.
- **What is wrong:** A unit entering just before packet submission pays `m × Δh / Y` for the whole interval. The page bounds this by `accrual_cap_blocks`, but leaves that cap's maximum open (OQ2). With a long cap on a non-route NAVCoin, the overcharge is material, and the mirror case lets an early exiter dodge fees.
- **Why it matters:** This undermines the fairness claims in the Option A table.
- **How to investigate:** Compute the worst case for plausible `accrual_cap_blocks` and deadman gaps on non-route assets.
- **Suggested fix:** Either set a small protocol maximum for `accrual_cap_blocks`, tied to `max_epoch_gap_blocks`, or adopt time-weighted V by averaging this packet and the last (already in OQ4) as the default.

**P4 — Muddy semantics of S as mint cap versus issued supply ("headroom")**

- **Severity:** major
- **Where:** NAV state row ("that supply is the mint cap"); R10; Option B state ("units minted into proof-backed headroom").
- **What is wrong:** `circulating_supply` is both the NAV denominator and the mint cap. If S exceeds issued units, fees are charged on V covering unissued headroom, and dilution is spread over headroom units. It is unclear who economically owns the headroom value and whether holders' effective fee rate equals m. R10 sets the cap to S+ΔS while the fee lane consumes issued supply. The page does not state the invariant relating issued supply, cap and fee lane after mints and redemptions that occur between submission and finalisation.
- **Why it matters:** This risks mis-stated NAV, wrong fee incidence, or headroom erosion.
- **How to investigate:** Trace `NavTrackedAsset.circulating_supply` usage in `nav_vault_asset_execution.rs` and `issued_asset_supply`, and write the invariant explicitly.
- **Suggested fix:** Add an explicit invariant section covering issued supply, cap and fee lane. Clarify whether V must cover only issued units or the full cap, and state the fee incidence on headroom.

**P5 — Option B depends on every supply path exposing amounts, including private routes**

- **Severity:** major
- **Where:** R13 ("the private primary routes, which already expose amounts to consensus"); R10's Option B snapshot.
- **What is wrong:** This claim is unverified. If shielded issuance or exit hides amounts, the B and S_B updates are impossible there. Vault-bridge mints and fix fills add more paths. A missed path silently corrupts the pooled mark.
- **Why it matters:** Option B's correctness rests on complete coverage of supply paths.
- **How to investigate:**
  - Grep for all supply-mutating operations in the execution crates.
  - Confirm amount visibility in the Orchard primary route proofs.
- **Suggested fix:** Enumerate supply paths mechanically, like the compile-time lane check. Make Option B rejection conditional on that coverage, or prefer Option A with protective defaults.

**P6 — Unclaimed fee units and the fee base**

- **Severity:** minor
- **Where:** R9; OQ4.
- **What is wrong:** Unclaimed units share NAV and are charged the management fee, so the recipient pays fees to itself. That is harmless in value but distorts the realised rate shown. More importantly, it creates an incentive to leave units unclaimed when credentials lapse.
- **Why it matters:** The displayed realised rate becomes misleading.
- **How to investigate:** Run a quick numerical check.
- **Suggested fix:** Exclude fee-lane units from the management base by default, and display fee incidence on holder units only.

**P7 — The block-height clock has no drift tolerance or governance**

- **Severity:** major
- **Where:** Notation ("Why heights"); OQ3.
- **What is wrong:** `Y` is a protocol constant, but block rate varies with consensus settings and load. FastPay defaults and Cobalt changes may shift block cadence. No tolerance or update mechanism is stated, so a change to `Y` is itself a fee increase that needs notice semantics.
- **Why it matters:** Holders could pay materially more or less than the advertised 100 bps a year.
- **How to investigate:** Measure historical block-rate variance on devnet and testnet, and check consensus roadmap changes.
- **Suggested fix:** Either add a header timestamp with bounded drift or specify `Y` governance. Changes to `Y` should be treated as schedule increases with notice, and the realised rate should be shown with a deviation alert.

**P8 — Fix fee destination and scope undefined**

- **Severity:** minor
- **Where:** R6; the fix row.
- **What is wrong:** The schedule caps `fee_bps` but does not say who receives fix fees, how they are custodied, or whether they appear on the explorer as amounts (only rates, given privacy). The scheduled fix settlement itself is unowned (OQ12).
- **Why it matters:** The founder's fix concept is a core NAVCoin element.
- **How to investigate:** Read the pNOK fix demo spec for fee routing.
- **Suggested fix:** Add a recipient field or route fix fees to `spread_recipient`. Assign fix settlement to a named pillar.

**P9 — Precision 18 with `u64` supply is impractical**

- **Severity:** minor
- **Where:** Notation (U up to 10^18); acceptance criterion 1.
- **What is wrong:** At precision 18, `u64` supply caps at about 18.4 whole units. Testing "precision 18, supply near the `u64` limit" passes vacuously for real products.
- **Why it matters:** An SDK that lets creators pick precision 18 produces unusable assets.
- **How to investigate:** Check the precision bounds in the types.
- **Suggested fix:** Recommend or enforce precision ≤ 9 in the SDK. Document supply limits.

**P10 — Missing 256-bit arithmetic is a correctness risk in consensus code**

- **Severity:** minor
- **Where:** Notation; R7.
- **What is wrong:** No 256-bit helper exists, so new consensus arithmetic must be written. Bugs here fork the chain or misprice fees.
- **Why it matters:** Errors in this code are consensus failures, not display bugs.
- **How to investigate:** Identify a vetted crate, for example the ruint/U256 used elsewhere, and check determinism across platforms.
- **Suggested fix:** Mandate an audited crate, add property tests against the Python reference, and add overflow tests on every formula.

**P11 — Ethereum/Uniswap wrapper holders lack disclosure**

- **Severity:** minor
- **Where:** R14 and R15; the Uniswap asset row.
- **What is wrong:** Wrapper holders bear management and performance fees through NAV, but disclosure lives only on PFTL surfaces. NAVGuardHook prices are NAV-after-fees, yet nothing on Ethereum shows the schedule hash.
- **Why it matters:** Transparency is incomplete for the venue most outside users touch.
- **How to investigate:** Check whether mint packets or the hook can commit to `schedule_hash` cheaply.
- **Suggested fix:** Include `schedule_hash` in exported metadata or in a view function, and link to it from the guide.

**P12 — Near-term Hyperliquid swaps and permissionless creation not driven by this pillar**

- **Severity:** major (for founder intent)
- **Where:** the Near-term paragraph; OQ11 and OQ12; Dependencies.
- **What is wrong:** The founder's nearest-term NAVCoin item is Hyperliquid swaps via Solana, and the stated goal is a primitive anyone can mint. This pillar acknowledges both but owns neither. Fees therefore risk being built ahead of the thing they price.
- **Why it matters:** Effort could be misallocated relative to the founder's sequencing.
- **How to investigate:** Cross-check the other pillar pages and the README for ownership.
- **Suggested fix:** Recommend a decision: either a NAVCoin-primitive pillar owns creation, Hyperliquid tracking and fix settlement, with this pillar as a module, or this pillar expands. Align the build order with that pillar's milestones.

**P13 — Notice protection is incomplete for gated holders and for capacity cuts**

- **Severity:** minor
- **Where:** R4; OQ6.
- **What is wrong:** During a pending increase, the issuer can cut route cash-in capacity, and gated holders who have lost authorisation cannot exit. Both defeat "exit on the old terms."
- **Why it matters:** The core holder protection becomes hollow in adversarial cases.
- **How to investigate:** Review the route epoch advance parameters.
- **Suggested fix:** Bar capacity reductions during a pending increase. Allow redeem-at-NAV for de-authorised holders while an increase is pending, or state plainly that they are unprotected.

**P14 — Option B rules are complex and under-tested for path-dependence**

- **Severity:** minor
- **Where:** the Option B state table; acceptance criterion 10.
- **What is wrong:** The pooled mark means holders below their own entry share charges on recovery, which is acknowledged. The snapshot-carry-forward in R10 is only sketched.
- **Why it matters:** The rules are hard to audit and hard for users to understand.
- **How to investigate:** Write a Python simulation with interleaved entries and exits between submission and finalisation, and check the invariants.
- **Suggested fix:** Give a precise carry-forward formula and add invariant-based fuzz tests. Or recommend Option A with protective defaults to reduce scope.

**P15 — Readability and decision load for the founder**

- **Severity:** minor
- **Where:** whole page; the 12 open questions.
- **What is wrong:** The page is very dense and assumes deep context. The founder must make many decisions before mainnet, and recommended defaults are missing.
- **Why it matters:** The decisions may stall, and the user-facing story of "build a NAVCoin with transparent fees" gets lost.
- **How to investigate:** Check which open questions block Step 1: only OQ2 and OQ3.
- **Suggested fix:** Add a recommended answer to each open question. Add a short plain-language summary of the default fee model, for example "Option A, proven-base performance fee, 2%/20% caps, 30-day notice."

**P16 — Fee revenue and protocol economics absent**

- **Severity:** minor
- **Where:** OQ8; Dependencies on Pillar 3.
- **What is wrong:** There is no analysis of whether a protocol or validator share would fund validators or PFT demand. That ties into the founder's validator program.
- **Why it matters:** It touches project-level economic coherence.
- **How to investigate:** Check the validator rewards design in Pillar 3.
- **Suggested fix:** Provide a recommended position, even "none in v1, reserved field," with reasoning.
