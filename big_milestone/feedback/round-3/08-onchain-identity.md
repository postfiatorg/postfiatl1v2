# Feedback — Pillar 8 — On-chain Identity (DID, KYC, Optional Trust Lines)

*Review round 3.*

> **Mean score:** 78/100 · Gemini 4 Neon 76 · Claude Opus 5.5 79
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/08-onchain-identity.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 76/100

*Model `google/gemini-4-neon` · 20810 output tokens*

#### Summary judgement

This is a strong, unusually well-grounded engineering specification that directly answers the founder's core questions, audits the real `postfiatl1v2` codebase down to specific Rust functions and tests, and catches a genuine prerequisite security bug (`issued_asset_credit_recipient_line_index` silently bypassing `requires_authorization`). Its core architectural choices—permissionless implicit lines by default everywhere, opt-in credential gating via one central `asset_gate`, XRPL Credentials / Permissioned Domains alignment, W3C VC 2.0 off-chain mapping with salted commitments, and post-quantum honesty—are sound and well-matched to Post Fiat.

It stops short of being execution-ready (keeping the score at 76) for three clusters of reasons:
1. **Protocol and consensus edge cases** that would ship live DoS, griefing, or security loopholes if built as written (DEX book poisoning and rollback loops, unbounded single-transaction `asset_clawback`, credential renewal vs. compromise cut-off loopholes, unrotatable issuer admin keys, bridge hold-spam, and `holder_limit`/`refuse_flag` re-introducing Solana batch jamming on default assets).
2. **Economic and UX incoherence** in state pricing (10 burned atoms for permanent, undeletable ledger state), broken 0-PFT onboarding for `credential_accept`, and an under-thought KYC vendor / cross-issuer legal reliance model.
3. **Excessive decision deferral** across 13 multi-part open questions without recommended defaults—most critically leaving two of the founder's five core NAVCoin primitives (the Uniswap wrapper and the pfUSDC FX fix) either non-functional or non-compliant for gated NAVCoins.

#### Correspondence to the founder's intention

The sub-page tracks Deliverable 8 very closely on its stated requirements:
- **Answers the trust-line question decisively:** Directly answers "Do NAVCoins need trust lines? No," keeps assets permissionless by default, and extends implicit line creation to DEX offers and escrows (and removes the Task Node wallet's forced `trust_set` step) so trust lines are truly an option, not a default.
- **XRPL & external standards alignment:** Faithfully adapts XRPL DID, Credentials, and Permissioned Domains alongside W3C DID/VC 2.0 data models, covering both traditional KYC vendors and zero-knowledge passport apps.
- **Protects near-term priorities:** Explicitly isolates default assets from the R0 bug fix and sequences credentials alongside the NAVCoin SDK, honoring the founder's near-term focus on shipping NAVCoin swaps without unnecessary drag.
- **Cross-pillar awareness:** Thoughtfully integrates with Pillar 6 (optional Nostr identity inside `did:pftl` plus a privacy warning against linking KYC to public social identity), Pillar 7 (NAVCoin SDK and fee claims), Pillar 5 (PFSH03 shielded pools), and flags the existing shadow-mode validator KYC pipeline.

**Where correspondence weakens:**
- **Core NAVCoin primitive gap:** The founder explicitly defined five core NAVCoin elements—including a Uniswap asset, swapping between the Uniswap pool asset and the PFTL asset, and a pfUSDC liquidity pool settling at a periodic "fix." Under this spec's concrete build plan (Steps 1–2), a gated NAVCoin loses the FX fix completely (Q6a/R12) and either has no Uniswap pool (Q7a) or leaks compliance completely via a free-moving Ethereum wrapper (Q7b). A founder reading this expects a recommended path to a *working* regulated NAVCoin, not a deferral of the hardest product intersections to "Later."
- **Decision overload:** Handing the founder 13 multi-part open questions (many of which are protocol-engineering or standard-practice defaults, like expired-record deletion bounties, empty-line garbage collection, or `kyc.*` type-name governance) works against the founder's goal of a crisp execution plan. The spec should propose opinionated defaults for each and ask only for sign-off on genuine strategic/legal trade-offs.

#### Technical robustness

The specification's biggest technical strength is its **path-by-path enforcement audit (R1)** across all 16 issued-asset execution paths and its **discovery of the R0 gating-bypass bug** enshrined in `asset_transactions_create_trust_pay_burn_and_reject_frozen_lines`. Sequencing R0 behind `AssetExecutionCompatibility` for deterministic replay, using holds (R11) where cross-chain value is already final, leveraging the trial ledger on shielded exits, and separating post-quantum on-chain records from classical upstream KYC signatures (R16) are all marks of rigorous L1 engineering.

However, several concrete state-machine, consensus, and security defects remain:
- **Liveness and DoS vulnerabilities in R3:** Counting skipped ineligible makers toward `MAX_DEX_CROSSES_PER_TRANSACTION` (64) enables cheap order-book poisoning (especially when a failing taker transaction rolls back the cancellations). Meanwhile, allowing a single `asset_clawback` to cancel all of an owner's open offers and escrows introduces unbounded O(N) execution in one transaction and breaks recipient finality on escrows.
- **Key-management and lifecycle gaps in R5–R6:** Renewal semantics do not specify whether `issued_height`, `signing_account`, or `status = revoked` are overwritten—creating a bypass around the compromise cut-off. Worse, R5 delegates signing-key rotation to an `admin` account while acknowledging that PFTL accounts cannot rotate keys, leaving the `admin` key as an unrecoverable single point of failure.
- **Cross-feature regressions in R11 & R14:** Keying bridge holds by `(asset, recipient, source_reference)` permits unbounded hold creation per user; and introducing binding `holder_limit` and `refuse_flag` (R14) without hold-fallback on default assets re-introduces the exact Solana batch-proof jamming bug that R11 solved for gated assets.
- **Fast-lane and owned-object ambiguity in R12:** The interplay between `RevokeHolderPermit` and L1 finalized-height admission checks is underspecified (who can call it, and why both exist), permit expiry is undefined when `approval_mode = either` or credentials have no expiry, and the owned-object sentence contradicts itself ("checks the gate at wrap and unwrap and keeps gated assets out").
- **Subtle L1 assumptions in R6/R7/R10/R14:** `did_resolve` assumes the account's ML-DSA-65 public key is already in ledger state (often false for unfunded/never-signed accounts); block-height expiries drift across multi-year KYC validity windows; DNF error reporting masks the real reason a user's credential failed; and creating an escrow recipient's line at *creation* rather than *release* creates permanent orphan lines.

#### Economic coherence

The economic design has three serious structural weaknesses:
1. **Permanent state is severely underpriced and inconsistent:** R13 burns 10 atoms (0.00001 PFT—less than half a transfer fee) to create a `TrustLine` or `Hold` that is **never deleted**. Claiming "byte fees and R14 limit spam" conflates UI spam filtering with ledger state-growth economics: an attacker can still bloat validator/RPC state with millions of permanent zero-balance lines or dust holds for trivial cost, and nobody has an economic incentive to prune empty lines. Meanwhile, R6 uses a refundable deposit for `Credential` objects, creating two contradictory state-accounting philosophies in the same spec.
2. **Broken onboarding economics for regulated users:** Requiring a newly KYC'd user with 0 PFT to sign and submit an on-chain `credential_accept` transaction creates a fatal gasless chicken-and-egg problem for institutional and mainstream users (relegated to a bullet inside Open Question 8). Delegated acceptance (signed during proof-of-control and submitted atomically by the issuer) eliminates both the fee barrier and an extra UX step.
3. **Unexamined KYC vendor and reliance incentives:** Traditional KYC vendors (Sumsub, Persona, Jumio) rarely operate bespoke L1 signing keys—especially for ML-DSA-65—so someone (the NAVCoin issuer or a middleware aggregator) must fund and operate the adapter, pay per-check KYC fees, and bear ongoing AML re-screening costs. Moreover, allowing any NAVCoin policy to accept `(IssuerA, kyc.basic)` without Issuer A's consent creates a classic free-rider problem and collides with financial regulations that require formal KYC reliance agreements and regulator-auditable access to underlying evidence.

Finally, economically speaking, a gated NAVCoin without an FX fix (Q6a) and without a compliant liquidity venue (Q7a) will suffer severe liquidity fragmentation and struggle to justify issuer adoption unless a pragmatic path—such as a transparent gated fix (Q6d) and a clear compliance stance on wrappers—is designed upfront.

#### Problems

##### P1 — DEX order-book poisoning and rollback loop on skipped ineligible makers
- **Severity:** critical
- **Where in the sub-page:** `Technical requirements → Movement rules → R3` ("Resting offers")
- **What is wrong:** When matching crosses an ineligible maker, the engine skips and cancels the offer, and *counts each skip toward `MAX_DEX_CROSSES_PER_TRANSACTION` (64)*. If 64+ resting dust offers at the top of the book belong to expired/revoked accounts (either organically after a batch expiry/issuer deregistration or maliciously via Sybils), a taker's transaction hits the 64-cross limit before reaching valid liquidity. Worse, if the taker's order is Fill-or-Kill, fails a minimum-output/slippage check, or otherwise aborts, the entire transaction rolls back—reverting the cancellations so the book remains permanently poisoned.
- **Why it matters:** Any regulated DEX market can be cheaply halted or degraded by dead offers at the best price, forcing honest takers to pay transaction fees to garbage-collect someone else's stale orders.
- **How an agent should investigate:** Inspect `apply_offer_operation` and `apply_offer_fills` in `crates/execution/src/fees_offer_planning.rs` to verify how `MAX_DEX_CROSSES_PER_TRANSACTION`, partial fills, and transaction rollbacks behave when offers are auto-cancelled during matching (and compare with XRPL's `tecKILLED` / unfunded-offer removal rules).
- **Suggested direction for the fix:**
  1. Add a permissionless `offer_prune` (or `offer_cancel_ineligible`) operation that anyone (keepers, market makers, issuers) can call to cancel ineligible resting offers without crossing them, optionally funded by a small refundable offer reserve.
  2. Cap skipped offers separately from filled crosses, or ensure that minimum offer sizes / refundable offer reserves make 64-offer book-stuffing uneconomical.
  3. Explicitly verify whether auto-cancellations persist on partial fill vs. revert on failed taker tx, and document the anti-poisoning invariant in acceptance criteria.

##### P2 — Core NAVCoin primitives (FX fix and Uniswap liquidity) left broken or non-compliant for gated NAVCoins
- **Severity:** critical
- **Where in the sub-page:** `What a gated NAVCoin can do`, `R11`, `R12`, `Build order`, `Open questions 6 & 7`
- **What is wrong:** Two of the founder's five core NAVCoin elements—the pfUSDC FX fix and the Uniswap wrapper/pool—are effectively dropped or left in an untenable contradiction for gated NAVCoins. Under Steps 1–2, gated assets are barred from shielded pools (meaning **no FX fix at all**), and wrappers are either disabled ("PFTL only," killing Uniswap liquidity) or free-moving on Ethereum ("PFTL and Ethereum," allowing any un-KYC'd/sanctioned address on Ethereum to hold and trade the NAVCoin with zero gating). Both decisions are deferred to "Later" with no recommended architecture.
- **Why it matters:** If regulated NAVCoins cannot use the FX fix and cannot safely interoperate with Uniswap, Pillar 8 fails its stated outcome ("*NAVCoins anyone can build, including regulated ones*") and makes gated NAVCoins economically second-class compared to ungated ones.
- **How an agent should investigate:** Inspect `FxFixPacketV1` and `consume_fx_fix_swap_plan` in `crates/node/src/execution_actions.rs` and `NAVGuardHook.sol` in `crates/ethereum-contracts/src/` to estimate the concrete engineering scope of (a) a transparent fix execution path (Q6d) vs. a PFSH03 credential-proof circuit (Q6c), and (b) an ERC-3643/allowlist-restricted wrapper (Q7c) vs. PFTL-only regulated NAVCoins.
- **Suggested direction for the fix:** Promote a concrete default recommendation into the main spec instead of punting:
  - Recommend **Q6(d) (transparent fix path for gated assets)** for Step 2/3 so regulated NAVCoins get the London-style pfUSDC fix without waiting for audited ZK credential circuits (keeping Q6c as a future privacy upgrade).
  - Be explicit about **Q7**: recommend **PFTL-only by default for regulated/gated NAVCoins** (with a clear compliance warning if an issuer enables a free-moving ERC-20 wrapper, since free ERC-20 wrappers do *not* constitute KYC-gating on Ethereum), and scope an ERC-3643 allowlist-bridge wrapper cleanly as a dedicated follow-on milestone if regulated Ethereum liquidity is required.

##### P3 — Unbounded O(N) computation in `asset_clawback` and broken escrow finality
- **Severity:** major
- **Where in the sub-page:** `Technical requirements → Movement rules → R3` ("Clawback reach")
- **What is wrong:** R3 specifies that a single `asset_clawback` transaction "can cancel the owner's open offers and owner-funded escrows in that asset and claw the units in one transaction." There is no bound on how many open offers or escrows an account may hold, nor any index/pagination mechanism. Additionally, clawing back an owner-funded escrow unilaterally strips units that a counterparty (the escrow recipient) may be relying on for settlement or collateral.
- **Why it matters:** A malicious holder can immunize their balance against clawback by splitting it across thousands of tiny offers or escrows, causing `asset_clawback` to exceed per-transaction execution limits or stall block validation across consensus nodes. Separately, silent confiscation of locked escrows alters the trust model of PFTL escrows for all recipients.
- **How an agent should investigate:** Inspect `AssetClawback`, offer storage, and escrow storage in `crates/execution/src/nft_escrow_asset_execution.rs` and `crates/execution/src/fees_offer_planning.rs` to check whether offers/escrows are indexed by `(owner, asset)`, what per-transaction compute/write caps exist, and what escrow guarantees are documented elsewhere in `postfiatl1v2`.
- **Suggested direction for the fix:**
  1. Make `asset_clawback` bounded and explicit: either accept an explicit, bounded list of `offer_ids` / `escrow_ids` (e.g., max 64 per tx) or process up to `MAX_CLAWBACK_UNLOCKS` per call so issuers can iterate safely across transactions without risking block DoS.
  2. Explicitly document and flag on-chain/in-wallet that escrows of `clawback_enabled` assets are clawback-revocable (or restrict clawback to open offers and expired/unclaimed escrows unless the escrow explicitly opts into clawbackable custody).

##### P4 — Credential renewal bypasses compromise cut-off, unspecified revocation transitions, and unrotatable issuer `admin` keys
- **Severity:** major
- **Where in the sub-page:** `What exists today` ("Reserve attestors"), `Technical requirements → Credentials and DID → R5 & R6`
- **What is wrong:** Three related security holes in credential lifecycle and key management:
  1. R6 states that renewal "updates expiry and commitment and keeps the acceptance," without specifying whether it also updates `issued_height` and `signing_account`. If it does not, a compromised signing key can renew an *old* credential (extending its expiry indefinitely) while keeping `issued_height` *before* the compromise cut-off in R5, completely defeating cut-off invalidation.
  2. R6 does not state whether a `credential_create` (renewal) on a `status = revoked` record reactivates it or is rejected, nor whether a compromised signing key can maliciously revoke/delete legitimate credentials before rotation.
  3. R5 uses an `admin` account to rotate compromised `signing_accounts`, yet "What exists today" notes that **PFTL accounts cannot rotate keys**. If an issuer's `admin` key is lost or compromised, there is no recovery or admin-transfer mechanism—permanently hijacking or bricking that `issuer_id` (and every immutable asset policy referencing it).
- **Why it matters:** Key compromise is a certainty over a chain's lifetime. A compromise-mitigation design that can be bypassed via renewal or permanently defeated by losing the admin key is not production-safe for regulated assets.
- **How an agent should investigate:** Check `NavAttestor` and account key/multisig structures in `crates/types/src/core_chain.rs` and `crates/types/src/account_owned_asset_types.rs` to see whether multisig/threshold accounts or admin-transfer operations already exist.
- **Suggested direction for the fix:**
  1. Require renewal to overwrite `issued_height`, `signing_account`, and `status = active` (or disallow renewing revoked credentials unless signed by a non-compromised key after explicit un-revocation, and document that clearly).
  2. Add an `issuer_admin_transfer` (two-step propose + accept) transaction and recommend/require M-of-N multisig (or cold governance-recoverable admin) for issuer `admin` accounts.
  3. Clarify whether signing keys can bulk-delete vs. only revoke, and how an issuer restores wrongly revoked credentials after a signing-key compromise.

##### P5 — Bridge hold-spam vector (`source_reference` keying) and R14 re-introducing Solana batch jamming on default assets
- **Severity:** major
- **Where in the sub-page:** `Technical requirements → Wrappers, holds, shielded pools and fast lanes → R11`, `Economics and spam → R13 & R14`
- **What is wrong:** Two flaws in the bridge/hold/recipient-control interaction:
  1. R11 keys holds by `(asset, recipient, source_reference)`. Unlike trust lines (bounded to one per `(asset, recipient)`), an attacker on Ethereum or Solana can send thousands of dust transfers to the same uncredentialed PFTL address, creating unbounded individual `Hold` objects in L1 state and draining the bridge relayer's fee balance (R13). There is also no expiry or consolidation rule for unclaimed holds (and under Strict exit rule without clawback, an ineligible recipient's hold is permanently stuck).
  2. R14 adds binding `holder_limit` and `refuse_flag` on *every credit path*, including default assets. However, R11 only routes **gate failures** to holds. If a recipient sets `holder_limit = 0` or `refuse_flag = true` on a *default* (non-gated) Solana-bridged asset and receives a bridge deposit, `credit_verified` will reject that receipt—**re-introducing the exact whole-batch Solana proof jamming bug** that R11 claims to fix!
- **Why it matters:** A single adversarial recipient can DoS the Solana bridge relayer for everyone in the same proof batch, or bloat L1 state with unbounded per-reference holds.
- **How an agent should investigate:** Inspect `credit_verified` in `crates/execution/src/solana_bridge.rs`, `nav_vault_asset_execution.rs`, and `credit_issued_asset_balance[_from_custody]` to verify how batch receipts fail, who pays creation fees on relayer-submitted proofs, and whether holds need per-source granularity or can be aggregated.
- **Suggested direction for the fix:**
  1. Route *all* non-fatal recipient credit rejections on finalized cross-chain imports (gate failure, `holder_limit` exceeded, or `refuse_flag` set) into holds so no recipient setting can ever jam a Solana or Ethereum bridge batch.
  2. Aggregate held balances by `(asset, recipient)` (emitting `source_reference` in transaction receipts/events rather than creating a distinct ledger state object per bridge transfer), eliminating unbounded per-user hold state bloat.
  3. Define a clear resolution path for permanently ineligible holds on non-clawbackable Strict assets (e.g., always allow return-to-issuer or require `clawback_enabled` whenever `exit_rule = strict`).

##### P6 — Underpriced permanent ledger state and contradictory state-rent models
- **Severity:** major
- **Where in the sub-page:** `Technical requirements → Economics and spam → R13`, `Credentials and DID → R6`, `Open question 8`
- **What is wrong:** R13 sets the fee for creating a `TrustLine` or `Hold` at **10 burned atoms (0.00001 PFT)**—less than half of a typical 22-atom transfer fee—for ledger objects that are **never deleted**. R13 claims "byte fees and R14 limit spam," but R14 (`refuse_flag`, off by default, plus explorer labels) only protects UX, not chain state size. An attacker can create 1,000,000 permanent `TrustLine` records across fresh addresses for 10 PFT plus modest byte fees. Simultaneously, R6 uses a **refundable deposit** for `Credential` records (returned on deletion), creating two incompatible state-rent philosophies across the same pillar.
- **Why it matters:** Unbounded, cheaply created, non-deletable state causes long-term validator and RPC storage bloat, degrades sync times, and creates painful migration debt if PFTL later has to retrofit owner reserves onto millions of legacy fee-free/burned-fee lines.
- **How an agent should investigate:** Review `docs/status/xrpl-feature-parity-burndown.md` (the state-fee rule), `crates/execution/src/entrypoints.rs`, and `crates/execution/src/fees_offer_planning.rs` to check current reserve/burn mechanics for offers, escrows, NFTs, and trust lines, as well as typical byte fees and target PFT unit economics.
- **Suggested direction for the fix:**
  1. Align `TrustLine`, `Hold`, and `Credential` under one coherent state-accounting model—ideally a **meaningful refundable state deposit** (comparable to offer reserves) coupled with **automatic pruning of zero-balance, non-frozen, default-limit lines** (returning the deposit to the funder/owner), or a higher anti-bloat fee with explicit pruning.
  2. If keeping a low fee during early bootstrap to preserve frictionless implicit lines, explicitly size it against worst-case state-growth math (GB/PFT) and commit to auto-pruning empty implicit lines from day one so the ledger doesn't accumulate permanent dust.

##### P7 — 0-PFT onboarding deadlock for `credential_accept` and unexamined KYC vendor / reliance economics
- **Severity:** major
- **Where in the sub-page:** `Technical requirements → Credentials and DID → R6 & R9`, `Dependencies`, `Open question 8`
- **What is wrong:** Two practical product/economic gaps:
  1. R6 requires the subject to submit an on-chain `credential_accept` transaction before a credential counts. A new user or institution onboarding to buy a KYC-gated NAVCoin typically starts with **0 PFT** and cannot pay gas for `credential_accept`. The spec buries this as the last bullet of Open Question 8 ("Whether to sponsor fees for new users with no PFT") rather than designing the protocol to avoid it.
  2. R9 assumes KYC vendors will operate an adapter signing ML-DSA-65 `credential_create` transactions, and the spec implies any NAVCoin creator can freely reuse another issuer's `kyc.basic` credentials. In practice, mainstream KYC vendors do not run custom L1 signing nodes, and regulated issuers face both free-riding (paying $2–$10/user for KYC while competitors reuse the on-chain attestation for free) and regulatory barriers against relying on third-party KYC without a formal reliance agreement and audit access to underlying PII.
- **Why it matters:** Without solving 0-PFT acceptance, new regulated users hit a dead end during onboarding. Without a realistic operator and reliance model for credential issuers, no real KYC provider or regulated NAVCoin issuer will deploy on the standard.
- **How an agent should investigate:** Check how Post Fiat's Task Node wallet signs off-chain messages (`crypto_provider/src/lib.rs`) and review `navcoin-create-and-swap-spec.md` (section B7.2) to understand Post Fiat's legal/operational assumptions around KYC vendors and reliance.
- **Suggested direction for the fix:**
  1. Support **delegated atomic acceptance**: during R9 proof-of-control, the user signs a domain-separated ML-DSA-65 acceptance payload `(chain_id, subject, issuer_id, credential_type, expires_height_bound, nonce)`, allowing the issuer/adapter to submit `credential_create_and_accept` in a single issuer-funded transaction (eliminating both the 0-PFT barrier and a UX round-trip while keeping subject consent). Keep standalone `credential_accept` as a fallback.
  2. Clarify the expected operator model (NAVCoin issuers or specialized identity middleware running the open-source reference adapter against vendor webhooks, rather than vendors directly) and consider optional **reliance scoping** (e.g., an issuer-controlled flag or issuer-specific credential type namespace so issuers can choose whether their KYC attestations are public-good or scoped to partners under reliance agreements).

##### P8 — Fast-lane permit revocation ambiguity, missing expiry bounds, and contradictory owned-object rule
- **Severity:** major
- **Where in the sub-page:** `Technical requirements → Wrappers, holds, shielded pools and fast lanes → R12`
- **What is wrong:** Three internal inconsistencies in R12:
  1. R12 requires both a new `RevokeHolderPermit` control action *and* fast-lane admission checking the L1 gate at the lane's finalized height. It does not explain **who is authorized** to invoke `RevokeHolderPermit` (asset issuer? credential issuer? validator/sequencer?), nor why `RevokeHolderPermit` is needed if admission already checks L1 freeze and gate status (presumably to close the finality-lag window, which should be stated).
  2. "Permits end by the credential's expiry" is undefined when `approval_mode = either` and eligibility comes from `line.authorized` (no credential), when a credential has `expires_height = None`, or when multiple credentials satisfy an OR-policy with different expiries.
  3. The owned-object bullet contradicts itself in one sentence: *"Any issued-asset owned lane checks the gate at wrap and unwrap and keeps gated assets out."* Either gated assets are rejected outright at wrap, or they are permitted with gate checks at wrap/unwrap—not both.
- **Why it matters:** Fast-lane and owned-object lanes are high-speed execution paths; ambiguous gating rules here easily turn into compliance bypasses or stuck funds during fast-lane redemption.
- **How an agent should investigate:** Inspect `crates/types/src/fastswap_types.rs`, `crates/execution/src/fastswap.rs`, `crates/execution/src/fastswap_control.rs`, and `crates/execution/src/owned_transfer.rs` to verify how permits are issued, validated, and finalized relative to L1 height.
- **Suggested direction for the fix:**
  1. Explicitly state the two-layer fast-lane defense: L1 finalized-height gate check catches all revocations/freezes automatically after finality delay, while `RevokeHolderPermit` (callable by the asset issuer, or automatically triggered by fast-lane watchers on L1 revocation/freeze events) provides immediate pre-finality cutoff.
  2. Cap every `FastHolderPermitV1` at `min(requested_expiry, max_permit_ttl_blocks, earliest_applicable_credential_expiry)` so permits always expire boundedly even when `expires_height` is empty or manual authorization is used.
  3. Fix the owned-object sentence to state cleanly: *"Gated assets (`requires_authorization = true`) are rejected from owned lanes at `wrap_to_owned`."*

##### P9 — Excessive decision deferral (13 multi-part open questions) with hidden Step-1 dependencies
- **Severity:** major
- **Where in the sub-page:** `Build order` and `Open questions for the founder` (Q1–Q13)
- **What is wrong:** The spec defers 13 multi-part questions to the founder without providing recommended defaults, mixing genuine strategic/legal choices (Q4 governance liability, Q6/Q7 regulated product scope, Q10 vendor selection) with low-level engineering defaults (Q8 expired-record deletion bounties and empty-line GC, Q9 refuse flag, Q13 type-name namespacing). Furthermore, the claim *"None blocks build step 1"* is inaccurate: Build step 1 ships the R13 10-atom burned line fee on payments and NAV mints before Q8 decides whether line fees should instead be refundable reserves, creating avoidable state-migration debt.
- **Why it matters:** Presenting 13 un-prioritized decision trees stalls founder sign-off and forces implementers to either block or guess on choices that affect storage schemas and migrations.
- **How an agent should investigate:** Review Pillars 5, 6, 7, and `README.md` to ensure recommendations on fees, governance, and NAVCoin lifecycle are consistent across the planning document.
- **Suggested direction for the fix:** Collapse the list into **4–5 true founder decisions** (governance liability for issuer registry Q4/Q5, regulated NAVCoin wrapper/fix product stance Q6/Q7, exit-rule floor Q1, and initial KYC/passport vendor budget Q10/Q8-vendor-payer), and convert the rest into **recommended engineering defaults** inside the Technical Requirements that the founder can override by exception. Resolve the Q8 line-fee-vs-reserve decision *before* Build step 1 ships.

##### P10 — Fragile R0 migration relies on manual issuer cleanup and ambiguous `reserve_paid == 0` heuristic
- **Severity:** minor
- **Where in the sub-page:** `Technical requirements → Prerequisite → R0`
- **What is wrong:** R0 proposes listing lines on gated assets with `authorized = true` and `reserve_paid = 0` before activation and having issuers manually clear them with `trust_set`. However, `reserve_paid == 0` cannot reliably distinguish a line created by the bug from a bug-created line that the issuer *subsequently* intended to keep authorized, and relying on every issuer to manually execute cleanup transactions leaves stale `authorized = true` lines that permanently bypass future `approval_mode = either` credential revocation.
- **Why it matters:** Even after R0 activates, legacy buggy lines on live gated assets (like A666) could silently undermine compliance unless verified and cleanly reconciled.
- **How an agent should investigate:** Query current testnet/mainnet state (or Genesis/A666 history referenced at `h341`) to count how many gated assets and `authorized = true, reserve_paid = 0` lines actually exist today.
- **Suggested direction for the fix:** Audit existing live gated assets now. If only test/internal assets like A666 exist, perform a deterministic state audit and coordinate explicit issuer review (or an automated one-time migration report verified by each issuer) before activation, rather than leaving cleanup as an informal off-chain chore.

##### P11 — `did_resolve` fails on never-signed accounts, and block-height expiries drift over multi-year KYC periods
- **Severity:** minor
- **Where in the sub-page:** `Technical requirements → Credentials and DID → R6, R7 & R9`
- **What is wrong:**
  1. R7 derives the `did:pftl` verification method from ledger state's ML-DSA-65 public key. On hash-addressed chains (`address_from_public_key`), an account that has never signed an on-chain transaction typically has no public key stored in ledger state yet—so `did_resolve` will fail or omit the verification method precisely when a new user is trying to complete KYC!
  2. R6 measures KYC credential expiry in `expires_height` (block height). While heights work well for short-lived permits and fixes, real-world KYC/passport expiries are calendar-based (e.g., 1–5 years). Consensus tuning or Cobalt block-time changes will shift actual expiry dates by weeks or months.
- **Why it matters:** External W3C DID/VC verifiers calling `did_resolve` during onboarding will break on fresh accounts, and compliance teams expecting date-based KYC renewal windows will face unpredictable height drift.
- **How an agent should investigate:** Check `crates/crypto_provider/src/lib.rs` and account creation/storage structs in `crates/types/src/core_chain.rs` to confirm when an account's ML-DSA-65 public key is persisted on chain, and check whether consensus block headers expose deterministic consensus timestamps (like XRPL ledger close time).
- **Suggested direction for the fix:**
  1. Explicitly handle unrevealed public keys in `did:pftl` (e.g., return an address-bound DID document with `alsoKnownAs` / `verificationMethod` populated once the key is revealed, or register the key via `did_set` / atomic `credential_create_and_accept`, and note that R9 proof-of-control passes the raw public key directly).
  2. Evaluate using deterministic consensus ledger timestamps (seconds since epoch, matching XRPL `Expiration` and W3C `validUntil`) for `Credential` expiry, or document the block-time assumption and renewal buffer explicitly if keeping heights.

##### P12 — Secondary edge cases: escrow line timing, DNF error masking, `nav_fee_claim` self-blocking, and lost-key passport nullifiers
- **Severity:** minor
- **Where in the sub-page:** `R1` (row 16), `R6` (Codes), `R9` (Passport proof), `R10`, `R14`
- **What is wrong:** Four small but sharp implementation traps:
  1. **Escrow line creation (R14):** Creating the *recipient's* line at escrow *creation* leaves permanent empty lines whenever an escrow is canceled or refunded; creating it on *release* is cleaner and avoids spam.
  2. **DNF error masking (R6/R10):** Returning "the first failing pair of the first rule" means a user whose Rule 2 credential *expired* gets a misleading `missing_credential` error for Rule 1.
  3. **`nav_fee_claim` self-blocking (R1 row 16):** Gating `nav_fee_claim` means an issuer/treasury on an `approval_mode = both` asset could lock itself out of claiming its own management/performance fees unless the treasury account itself maintains an external KYC credential.
  4. **Off-chain passport nullifier lock-in (R9):** Enforcing "one live credential per nullifier" off chain provides no cross-verifier uniqueness and no documented recovery flow if a user loses their PFTL key (since PFTL accounts cannot rotate keys).
- **Why it matters:** Each of these will cause avoidable UX bugs, confused wallet error messages, or stuck protocol operations once implemented.
- **How an agent should investigate:** Cross-check `lock_issued_asset_for_escrow` / `release_issued_asset_escrow_to_recipient` in `crates/execution/src/nft_escrow_asset_execution.rs` and `nav_fee_claim` in Pillar 7 (`07-navcoin-fees.md`).
- **Suggested direction for the fix:**
  1. Defer recipient line creation to escrow *release* (while checking the gate at release regardless).
  2. In gate error selection, prioritize the "closest-to-passing" failure across rules (e.g., `credential_expired` or `credential_revoked` over `missing_credential`) or return a structured failure summary in RPC simulation.
  3. Clarify whether the asset's designated fee recipient/issuer account is exempt on `nav_fee_claim` (or explicitly documented as requiring authorization/credentialing).
  4. Specify that a passport verifier must allow a user to re-prove the same passport nullifier to a *new* PFTL account by simultaneously revoking/deleting the prior credential on the lost account.

## Claude Opus 5.5 — 79/100

*Model `anthropic/claude-opus-5.5` · 10516 output tokens*

#### Summary judgement

This is a strong sub-page. It answers the founder's direct question first: NAVCoins do not need trust lines, because the chain creates the holding line on first credit. It then turns "trust lines are an option, not a default" into a concrete, enforceable design: one gate function, an optional credential policy, and XRPL-style DID and Credentials mapped to W3C VC 2.0.

It also does real diagnostic work. It finds a plausible gating-bypass bug, where implicit lines are always created as authorized. It inventories 16 movement paths and is honest about where value is already final on another chain. Its build order keeps the near-term goal, permissionless NAVCoin swaps, unblocked.

The main weaknesses are:

- **Density and scope.** There are 18 requirements, 16 enforcement rows and 13 open questions, for what the founder framed as one exploratory question.
- **Tensions it names but does not resolve:**
  - Gated NAVCoins lose the London-style fix, a founder-listed core NAVCoin element.
  - A freely moving Ethereum wrapper undermines PFTL-side KYC.
  - Public credentials sit awkwardly beside the privacy pillar.
- **Engineering gaps:**
  - No account or DID key rotation.
  - Holds have no expiry or return path.
  - Binding holder limits and the refuse flag conflict with holds on bridge credits.
  - Gate coverage relies on code-review discipline rather than structure.
- **Economics.** State rent for lines is trivially cheap. Who pays for KYC is left open.
- **Unverified references.** Code citations are specific, but this review could not check them. Agents must confirm them before trusting the bug claim.

#### Correspondence to the founder's intention

**What the founder asked for, and how the page answers:**

- **Survey XRPL DID-type standards.** Done. The page uses the XRPL DID, Credentials and Permissioned Domains amendments, and maps them field by field to W3C VC 2.0 (R8).
- **Integration with external KYC or passport systems, usable to KYC NAVCoins.** Done. There are two pathways (R9):
  - a vendor adapter;
  - a zero-knowledge passport-proof verifier with nullifiers.
- **"I don't know if trust lines need to be set."** Answered in the first sentence of the paragraph, with an honest caveat in the existing-state table: today the DEX does require lines, and the Task Node wallet sends `trust_set` first. R14 removes that requirement.
- **"Trust lines an option, not a default; permissionless by default but trust-line assets possible."** Directly encoded:
  - `requires_authorization` is opt-in and immutable.
  - Default assets are untouched by R0.
  - R17 makes "permissionless by default" a docs commitment.

**Alignment beyond the literal ask:**

- **Pillar 6 (Nostr identity).** The Nostr key and handle appear in the `did:pftl` document.
- **Pillar 7 (NAVCoin SDK).** The SDK sample shows both gated and default modes.
- **NAVCoin core elements.** A table checks gated NAVCoins against each element the founder listed.
- **Explorer.** Explorer visibility (R18) matches the founder's recurring wish that everything be "represented on the block explorer".

**Where correspondence is weaker:**

- **The gated-NAVCoin table shows two core elements broken or undecided.**
  - The pfUSDC pool at a fix is effectively removed for gated assets in step 1.
  - The Uniswap asset is undecided (Q7).
  
  The founder listed these as *core* NAVCoin elements. The page leaves "regulated NAVCoins work as NAVCoins" to open questions.
- **Scale of the answer.** The founder framed this as "there are a number of DiD type things… I do not know if…". The page answers with a full protocol spec and a 13-question decision load. That burden may slow decisions that matter more near term, such as NAVCoin swaps and the chain migration.
- **Cross-pillar identity.** Reuse of identity for validators, top-30 users and Finance Hub access (founder deliverables 3 and 9) is relegated to Q12. A unified credential primitive is arguably the "post-human" identity story the founder wants. It deserves a recommendation, not just a question.
- **Hyperliquid and Solana.** The founder's near-term item, "tracking a Hyperliquid NAVCoin on Solana", gets one line ("later venues follow the same wrapper choice").

#### Technical robustness

**Strengths:**

- **Single decision function.** `asset_gate(asset, account, direction, height)` is evaluated live, never cached, with an explicit enumeration of every path.
- **Clear semantics.** The R2 approval-mode logic is clear.
- **R3 is careful.** It distinguishes unlocking from moving, auto-cancels makers that fail the gate, and lets clawback reach offers and escrows.
- **Correct reasoning about finality on other chains.** Rows 7, 9 and 10 go to holds. Row 13 is rejected because the note stays unspent.
- **Safe rollout.** Activation-height gating via `AssetExecutionCompatibility` gives replay safety, plus a pre-activation audit of wrongly authorized lines.
- **Issuer rotation.** Rotation via stable `issuer_id` with compromise cut-offs is well thought out.
- **Honest about post-quantum limits.** The on-chain record is post-quantum (ML-DSA-65), while upstream stays classical (R16).
- **Measurable acceptance criteria.** For example, the 6-account matrix and the 64-cross benchmark.

**Weaknesses (detailed in Problems):**

- Gate coverage is enforced by review, not by types or APIs.
- Accounts cannot rotate keys, which leaves DIDs and credentials brittle.
- Holds have no lifecycle bound.
- Binding limits and the refuse flag interact badly with holds.
- Renewal can change a commitment without the subject's consent.
- Height-based expiry ignores real-world KYC validity dates.
- The fast-lane revocation delay is only documented, not bounded.
- The many specific code claims, which the whole bug narrative rests on, need verification.

#### Economic coherence

**Coherent:**

- Line fees are charged to the transaction source, following the parity tracker's rule that every state-expanding object pays.
- Credential deposits are refundable, and anyone may delete a record after expiry.
- The page is honest that the 10-atom fee is state accounting, not a spam deterrent.
- It acknowledges that bonds without slashing secure nothing.
- It flags the risk that governance becomes a compliance gatekeeper (Q4b, B7.2).

**Less coherent:**

- **Lines are permanent but priced at 10 atoms.** That is well under a single transfer fee. Anyone can create a line in anyone's account (for example, airdrop spam), and lines are never deleted. A burned fee with no deletion does not pay for permanent state.
- **KYC vendor cost is not modelled.** Vendors typically charge per check. The page does not say who pays, whether credentials are reused across NAVCoins (deferred to B7.2), or what it means for NAVCoin creators' unit economics. Since "anyone can build NAVCoins" is the outcome, the per-holder onboarding cost of a gated NAVCoin matters.
- **Free-moving wrapper.** Option 7b, today's A666 behaviour, means KYC on PFTL may have no compliance value. Issuers pay credential costs for a gate that Uniswap holders bypass.
- **Losing the fix.** Gated NAVCoins lose the fix, which removes a liquidity and pricing mechanism. That weakens the regulated-NAVCoin value proposition the pillar exists to enable.

#### Problems

**P1. Gated NAVCoins lose the fix and possibly the Uniswap leg in the shipped plan**
- **Severity:** major.
- **Where:** "What a gated NAVCoin can do" table; R12; Q6; Q7; build step 1.
- **What is wrong:** Step 1 bars gated assets from shielded pools. The fix only settles as a shielded swap. So gated NAVCoins have no fix. The Uniswap leg is undecided.
- **Why it matters:** Two of the founder's five core NAVCoin elements are missing for regulated NAVCoins. The pillar promises "NAVCoins anyone can build, including regulated ones", but they would be second-class.
- **How to investigate:**
  - Read `consume_fx_fix_swap_plan` and `FxFixPacketV1`.
  - Estimate the effort for option 6d (a transparent fix path) against 6c (circuit work).
- **Suggested fix:** Recommend a default, likely 6d as a near-term transparent fix path for gated assets. Put it in build step 2 rather than leaving it fully open.

**P2. Gate coverage depends on code review, not structure**
- **Severity:** major.
- **Where:** R1 ("Review rejects any new path that skips it").
- **What is wrong:** The bug being fixed is exactly a path that bypassed checks. A review convention will regress.
- **Why it matters:** One missed path silently defeats KYC on every gated asset.
- **How to investigate:** List every function that mutates `TrustLine.balance` or creates lines. Check whether they can all be routed through one module-private API.
- **Suggested fix:**
  - Make line credit and debit private behind a single gated API, for example via a type that is only obtainable from `asset_gate`.
  - Add a test or lint that enumerates callers.
  - Add a property test: no gated balance increases without a passing gate at that height.

**P3. No key rotation for accounts, yet accounts are DIDs and credential subjects**
- **Severity:** major.
- **Where:** R7; "What exists today" (the "PFTL accounts can't rotate keys either" note); R9.
- **What is wrong:**
  - A compromised account key means a compromised DID and its KYC credentials, with no recovery.
  - Holders cannot migrate their credentials.
  - `did:pftl` documents have a single, fixed verification method.
- **Why it matters:**
  - DIDs without rotation fall short of W3C DID best practice.
  - Regulated holders lose access permanently, and issuers face fraud risk.
- **How to investigate:** Check `crypto_provider` and `address_from_public_key`. Check whether an account-level regular-key or master-key change (as in XRPL `SetRegularKey`) is feasible.
- **Suggested fix:** Add account key rotation, or an explicit credential-migration flow, as a dependency or requirement. Document compromise handling.

**P4. Holds have no expiry, return path or bound**
- **Severity:** major.
- **Where:** R11 (holds).
- **What is wrong:** Value that fails the gate sits in a hold indefinitely. "Held units count as supply." There is no return-to-origin option on Ethereum or Solana and no timeout. Only the return-only exit rule offers an exit.
- **Why it matters:**
  - Permanently stranded value and supply distortion for NAV accounting.
  - State bloat.
  - A user who buys on Uniswap and cannot KYC (for example, a sanctioned jurisdiction) is stuck forever.
- **How to investigate:** Model the life of a hold under each exit rule. Check whether NAV computation treats held units differently.
- **Suggested fix:**
  - Add a hold timeout after which units go back to the source, burn at NAV to the recipient, or pass to the issuer, per the exit rule.
  - Expose holds in NAV and supply reporting.
  - Make hold creation pay into state rent.

**P5. Binding holder limits and the refuse flag conflict with bridge finality**
- **Severity:** major.
- **Where:** R14 (binding limits on "every credit path", refuse flag) versus R11 (holds for credits already final elsewhere).
- **What is wrong:** Nothing specifies what happens when a bridge import or deposit claim exceeds a holder limit or hits a refuse-flagged account. Rejecting it would lose final value, or jam a Solana proof batch, which R11 explicitly fixes for gate failures.
- **Why it matters:** It reintroduces the batch-jam and lost-value failure modes the page solves elsewhere.
- **How to investigate:** Trace `credit_verified` and vault claim paths for limit checks.
- **Suggested fix:** Route any credit-side failure on rows 7, 9 and 10 (gate, limit or refuse) to a hold. Make this a single rule rather than a gate-only rule.

**P6. Line state rent is economically trivial and lines are never deleted**
- **Severity:** major (economic).
- **Where:** R13; Q8; "Why XRPL has trust lines" table.
- **What is wrong:**
  - The fee is 10 atoms, burned, and empty lines persist.
  - Implicit lines on default assets let anyone grow anyone's state.
  - The refuse flag is optional and may not ship.
- **Why it matters:**
  - Unbounded state growth.
  - Spam assets in wallets, which labels only mitigate.
  - It contradicts the parity tracker's intent that state-expanding objects pay a reserve.
- **How to investigate:** Estimate the bytes per line and the cost to grow state by, for example, 10 GB at current fees. Compare with the XRPL reserve and with the byte fees mentioned.
- **Suggested fix:**
  - Recommend a refundable reserve, or a fee scaled to state bytes.
  - Allow deletion of empty implicit lines.
  - Consider making the refuse flag ship, off by default.

**P7. KYC on PFTL may be compliance theatre under a free-moving wrapper**
- **Severity:** major.
- **Where:** Q7(b); R11; A666 note.
- **What is wrong:** If the Ethereum wrapper moves freely, gated holders can exit to anyone via Uniswap. The PFTL gate only controls who re-imports.
- **Why it matters:** Issuers pay for credentials that do not meet a regulator's transfer-restriction expectations. B7.2 legal review may reject the whole design.
- **How to investigate:** Get B7.2's view on whether 7b is acceptable for any regulated product. Assess the ERC-3643 bridge cost.
- **Suggested fix:**
  - State plainly that 7b makes the asset "KYC-at-PFTL-edge only".
  - Label it so on the explorer.
  - Recommend 7a for regulated products until 7c exists.

**P8. Code references and the bug claim are unverified**
- **Severity:** major (process).
- **Where:** "What exists today" table; Code references; the bug row citing test `asset_transactions_create_trust_pay_burn_and_reject_frozen_lines`.
- **What is wrong:** The page cites many specific functions and behaviours: the implicit line authorized, limits raised, fee exemptions, permit semantics, and "A666 at h341". If any is inaccurate, R0's urgency and scope change.
- **Why it matters:** Build step 1 is driven entirely by these claims.
- **How to investigate:**
  - Grep postfiatl1v2 for each symbol.
  - Run the named test.
  - Confirm A666's flags and its use of the route and the pool.
- **Suggested fix:** Add verified commit hashes or line anchors. Correct any mismatches.

**P9. Live gate cost at matching scale is unbounded until benchmarked**
- **Severity:** minor/major.
- **Where:** R2 ("never cached"); R10; acceptance criterion 11.
- **What is wrong:** A 64-cross taker against makers is checked for both assets under a 10-pair policy. That could mean on the order of thousands of credential lookups per transaction. Validators, Cobalt and fast-pay latency goals may suffer.
- **Why it matters:** The founder emphasises latency (fast-pay defaults).
- **How to investigate:** Run the benchmark early, in step 1, using a mock credential store.
- **Suggested fix:** Lower the initial bounds. Cache gate results within a single transaction (not across transactions). Index credentials per subject.

**P10. Block-height expiry does not match real-world KYC validity**
- **Severity:** minor.
- **Where:** R6 expiry; R8 mapping of `validUntil`.
- **What is wrong:** VC `validUntil` is a wall-clock date. Height-based expiry drifts with block time.
- **Why it matters:** A credential may be honoured past its legal expiry.
- **How to investigate:** Check whether the chain has a consensus timestamp.
- **Suggested fix:** Use a consensus-time expiry, or require issuers to set a conservative height with a documented mapping.

**P11. Renewal can change the commitment without subject consent**
- **Severity:** minor.
- **Where:** R6 renewal.
- **What is wrong:** A create on an existing record updates expiry and commitment while keeping acceptance. The issuer can therefore bind the subject to a different VC unilaterally.
- **Why it matters:** It weakens consent semantics and the auditability of what was attested.
- **How to investigate:** Compare with XRPL Credentials semantics.
- **Suggested fix:** Keep acceptance for expiry-only renewals. Require re-acceptance when the commitment changes.

**P12. The fast-lane revocation window is documented but not bounded**
- **Severity:** minor.
- **Where:** R12 fast lane; acceptance criterion 8.
- **What is wrong:** Admission is checked at the lane's finalized height. A revoked or frozen holder can trade during the lag.
- **Why it matters:** It creates a compliance gap and could enable sanctioned transfers.
- **How to investigate:** Measure the typical lag between finalized height and execution.
- **Suggested fix:** State a maximum lag. Optionally re-check the gate at settlement on L1.

**P13. Credentials are publicly linkable, which sits uneasily with the privacy pillar**
- **Severity:** minor/major.
- **Where:** R15.
- **What is wrong:**
  - Records are public. "This account is KYC'd by vendor A for kyc.basic" is visible to all.
  - Opt-in explorer display is cosmetic, because the data is on chain anyway.
- **Why it matters:** The founder treats privacy as a production-readiness criterion. Public KYC flags plus the Pillar 6 linkage risk deanonymising users.
- **How to investigate:** Assess whether a credential could be stored as a commitment, with the gate verifying a proof (related to Q6c and Q11).
- **Suggested fix:**
  - Keep the public design for v1, but make separate-account defaults mandatory in wallets.
  - Note a v2 path to private credentials.

**P14. Cross-pillar identity reuse is under-recommended**
- **Severity:** minor.
- **Where:** Q12.
- **What is wrong:** Validator status, top-30 status and Finance Hub access are natural credential types. The founder's validator, Finance Hub and Hive identity deliverables all want linked identity.
- **Why it matters:** It is a missed chance for one coherent identity primitive, and it duplicates pipelines.
- **How to investigate:** Read `identity-flag-pipeline-spec`. Check whether validator KYC could be emitted as credentials.
- **Suggested fix:** Recommend a direction, for example credentials for access gating with the governance pipeline as the issuer.

**P15. The headline "No trust lines needed" overstates current reality**
- **Severity:** minor.
- **Where:** "In one paragraph" versus the existing-state Offers row.
- **What is wrong:** Today the DEX requires lines even on default assets. That only changes with R14 in build step 2.
- **Why it matters:** The founder may tell users something not yet true.
- **How to investigate:** Confirm the current behaviour in `ensure_offer_asset_participation`.
- **Suggested fix:**
  - Say "No for payments today; no for the DEX and escrow after R14."
  - Move R14's DEX line creation into step 1, since it serves near-term swaps.

**P16. Too dense for founder decision-making**
- **Severity:** minor.
- **Where:** The whole page; 13 open questions.
- **What is wrong:** Every question is open with no recommended answer, and dense row-number cross-referencing makes the page hard to read.
- **Why it matters:** It slows decisions. Some questions (Q2, Q5, Q13) could be defaulted.
- **How to investigate:** Rank the questions by build-step dependency.
- **Suggested fix:**
  - Give a recommended default for each question.
  - Mark which questions block step 2.
  - Add a short plain-language summary for the founder.

**P17. Credential portability is weak, enabling credential resale**
- **Severity:** minor.
- **Where:** R9 proof of control.
- **What is wrong:**
  - Credentials are bound to an account that can be sold or handed over.
  - KYC vendors are not limited to one credential per person, unlike the passport nullifier.
  - One KYC could credential many accounts.
- **Why it matters:** It gives a sybil and laundering vector for gated NAVCoins.
- **How to investigate:** Check vendor capabilities for per-person uniqueness.
- **Suggested fix:**
  - Let policies require uniqueness-scoped credentials.
  - Recommend that vendor adapters enforce one live credential per person per scope where possible.

**P18. Hyperliquid and Solana tracking is barely covered**
- **Severity:** minor.
- **Where:** R11 last bullet.
- **What is wrong:** The founder's stated near-term item gets one sentence, with no hold or wrapper semantics specific to Solana series.
- **Why it matters:** These are near-term integration risks.
- **How to investigate:** Check `solana_bridge.rs`, where series copy the family's flags, and check Hyperliquid wrapper plans.
- **Suggested fix:** Add a row and acceptance criterion for gated Solana series.
