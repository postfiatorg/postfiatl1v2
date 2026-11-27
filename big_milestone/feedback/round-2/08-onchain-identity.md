# Feedback — Pillar 8 — On-chain Identity (DID, KYC, Optional Trust Lines)

*Review round 2.*

> **Mean score:** 73/100 · Gemini 4 Neon 74 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/08-onchain-identity.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 17871 output tokens*

#### Summary judgement

This is a strong, code-grounded second-draft specification that directly answers the founder's core questions, uncovers a genuine authorization-bypass bug in `postfiatl1v2`, and maps enforcement across 16 concrete ledger paths. Its adoption of XRPL's DID / Credentials / Permissioned Domains model paired with W3C VC 2.0, `did:pftl`, ML-DSA-65 signing, and strict off-chain PII separation is well-judged.

However, the spec is **not yet ready to ship to engineering without revision**. It contains **two critical flaws** and several major technical and economic gaps:
1. **An internal state-machine bug in R0 + R2:** R0 creates implicit lines as `authorized = true` whenever the gate passes (or the sender is the issuer), which interacts with R2 (`line.authorized || policy_met(...)`) to **permanently cache authorization** and completely bypass future credential expiry and revocation.
2. **A strategic/economic deadlock for regulated NAVCoins:** Under the spec's Step 1 defaults (R11, R12, Q6, Q7), a KYC-gated NAVCoin is barred from the shielded pool (which disables the **London FX Fix**, since `FxFixPacketV1` settles inside the shielded pool), barred from the FastPay owned lane, and forced on Ethereum to choose between zero Uniswap liquidity ("PFTL only") or an unrestricted ERC-20 that defeats KYC. That strips regulated NAVCoins of 3 of the 5 core NAVCoin primitives from the founder's prompt.

Fixing the R0/R2 authorization bug, resolving how gated NAVCoins access the FX fix and liquidity venues, adding issuer key rotation and KYC proof-of-possession, closing DEX/escrow trust-line and orderbook/clawback edge cases, and tightening the fee/reserve economics will move this spec from ~74 to implementation-ready.

---

#### Correspondence to the founder's intention

**What aligns well:**
- **Optional trust lines:** Directly answers the founder's uncertainty ("Do NAVCoins need trust lines? No.") and preserves permissionless-by-default issuance while making `requires_authorization` + `credential_policy` an explicit opt-in.
- **XRPL standards + external KYC & passport systems:** Cleanly maps XRPL DID, Credentials (`credential_create` / `accept` / `delete`), and Permissioned Domains onto `postfiatl1v2`, with explicit adapters for both traditional KYC vendors and ZK passport verifiers.
- **Identity hygiene:** Correctly anchors credentials to PFTL accounts (`did:pftl`) and keeps Pillar 6 (Hive Chat / Nostr) as an optional link, avoiding forcing KYC'd holders to dox their public social identity.
- **Explorer & SDK transparency:** Covers explorer surfacing of gating rules, policy pairs, exit rules, and wrapper settings without leaking personal data.

**Where correspondence slips:**
- **Trust lines are still mandatory on DEX Offers and Escrows:** The spec tells the founder that *"anyone can receive a default asset without setting anything"*, yet "What exists today" (`offer-setup.js`), Row 2 (Offers), and Row 3 (Escrow) confirm that DEX offers and escrows still require pre-existing trust lines (`trust_set`). Leaving native DEX trading dependent on `trust_set` for default permissionless assets contradicts "trust lines are an option, not a default."
- **Regulated NAVCoins lose core founder primitives in Step 1:** The founder's prompt explicitly defines NAVCoins around five core elements, including a Uniswap pool and a PFUSDC liquidity pool settled via a London FX fix. By barring gated assets from the shielded pool (and therefore `FxFixPacketV1`) and deferring compliant wrappers to Q7(c), Step 1 delivers gated assets that cannot use the core NAVCoin liquidity stack.
- **Founder decision overload:** 13 multi-part open questions (many tactical, e.g., Q5, Q8, Q9, Q11, Q13) lack recommended defaults, making it hard for the founder to sign off quickly on the few truly strategic choices.

---

#### Technical robustness

The 16-row enforcement matrix, activation-height strategy (`AssetExecutionCompatibility`), and per-path acceptance tests give this spec strong engineering bones. Still, several load-bearing technical mechanisms break under adversarial or lifecycle stress:

- **Authorization caching bug (R0 + R2):** Creating an implicit line with `authorized = true` on a credential-passing credit or issuer mint permanently overrides future credential expiry/revocation under R2's OR formula.
- **Issuer key lifecycle & immutable policy lock-in (R4, R5, R10):** Deregistering an issuer invalidates *all* historical credentials simultaneously, there is no key rotation, and there is no recovery/governance deregistration path if an issuer loses their key. Coupled with immutable `credential_policy` (Q3a) referencing account addresses, a single vendor key rotation can permanently brick onboarding for a live NAVCoin.
- **Orderbook, Escrow, Clawback, and Renewal edge cases:** Resting DEX offers from expired-credential makers can poison orderbook matching or strand funds under Q1(a); clawback reach over assets parked in escrows/offers (mempool front-running) is unchecked; renewing a credential indexed by `(subject, issuer, type)` before expiry has undefined acceptance semantics; and the spec assumes `trust_set` can clear `authorized = false` without verifying whether `nft_escrow_asset_execution.rs` enforces XRPL's one-way authorization rule.
- **Missing KYC proof-of-possession (account substitution):** `credential_accept` proves the target PFTL key wants the credential, not that the human who passed KYC owns that key. Because PFTL uses ML-DSA-65, standard KYC widgets cannot verify account ownership without a specified challenge-response protocol in the adapter.
- **Underspecified bridge quarantine & FastSwap freeze:** R11 defers how failed return imports stay recoverable (and omits Rows 9 and 10), while R12 allows "short permits" in FastSwap that fail to enforce immediate regulatory freezes.
- **Policy evaluation bounds & boundary conditions:** Nested DNF policies (OR of ANDs) need explicit caps on both dimensions plus multi-leg tx budgets, and `expires_height` needs explicit inclusive/exclusive and non-expiring conventions.

---

#### Economic coherence

The spec improved materially over round 1 by questioning `NavAttestor` bonds (since no slashing exists) and flagging liquidity splitting on Ethereum wrappers. Nevertheless, the economic model has three significant gaps:

1. **Unviable Step 1 economics for regulated NAVCoins:** If a regulated NAVCoin issuer cannot use the London FX Fix (barred with shielded pools), cannot use FastPay, and cannot access Uniswap without either splitting liquidity or abandoning KYC compliance (Q7), there is no coherent liquidity path for the very product this pillar exists to unlock.
2. **Unverified state-expansion fee economics (R13 & Q8):** R13 mandates charging the sender `TRUSTLINE_STATE_EXPANSION_FEE (10)` on every implicit line creation, without verifying the unit or value of `10` (drops vs. whole PFT). If tiny, it provides zero spam resistance while R14's refuse flag defaults to off; if large, it penalizes permissionless onboarding and creates repeat-fee griefing against automated senders (like payout engines or NAV redemptions) whenever recipients empty and delete lines (Q8).
3. **Missing reserve-handoff incentives for credentials (R6 & Q8):** Unlike XRPL Credentials—where the issuer locks an owner reserve on `CredentialCreate`, which transfers to the subject on `CredentialAccept` and refunds on `CredentialDelete`—this spec treats credential fees as an open question (Q8). Waiving fees or omitting refundable reserves leaves the chain exposed to unaccepted-credential state spam from compromised/malicious issuers and removes any incentive for third parties to prune expired credentials.

---

#### Problems

##### P1 — R0 + R2 logical contradiction permanently bypasses credential expiry and revocation
- **Severity:** critical
- **Where:** Technical requirements — R0 ("Prerequisite") and R2 ("Movement rules").
- **What is wrong:** R0 states: *"On a gated asset, an implicit line is created **authorized** only in two cases: the gate passes (R2), or the transaction's source is the asset's issuer."* Meanwhile R2 defines live evaluation as: `can_move = !frozen && (!requires_authorization || line.authorized || policy_met(account, height))`. If an implicit line created via `policy_met` sets `line.authorized = true`, then `line.authorized` stays `true` forever—even after the holder's credential expires or is revoked. The same bypass occurs on issuer-sourced credits (e.g., `NavMintAtNav` or issuer mints): any holder who receives tokens from the issuer gets `line.authorized = true` permanently and is thereafter immune to credential expiry/revocation.
- **Why it matters:** This directly breaks live credential enforcement (R2), revocation/expiry (User Story 5, Acceptance Criteria 3, 4, and 5), and compliance guarantees for every holder onboarded via implicit lines or primary mints.
- **How to investigate:** Inspect `issued_asset_credit_recipient_line_index` in `crates/execution/src/issued_asset_ledger_helpers.rs` and every caller in rows 1, 4, 7, 9, 10, and 13 to see how `authorized` is initialized and whether `NavMintAtNav` or route credits run with `source == issuer`.
- **Suggested direction for fix:**
  - When an implicit line is created because `policy_met(account, height)` passed, initialize it with `authorized = false` (never setting the manual issuer override flag).
  - Do **not** auto-set `authorized = true` on issuer-sourced credits by default; either require the recipient to pass `asset_gate` (via credential or pre-existing manual `trust_set`), or restrict auto-authorization strictly to an explicit issuer flag / `trust_set` transaction so primary mints never silently confer permanent KYC exemption.

---

##### P2 — Step 1 defaults strip KYC-gated NAVCoins of the London FX Fix, Uniswap compliance, and FastPay
- **Severity:** critical
- **Where:** Enforcement table Row 12 & Row 14, R11, R12, Open questions Q6 & Q7.
- **What is wrong:** Under Step 1 defaults, gated assets are barred from shielded pools (R12)—which also bars them from the **London FX Fix**, because Row 12 notes `FxFixPacketV1` settles inside the shielded pool. They are also barred from FastPay owned lanes (R12), and on Ethereum they must either forgo Uniswap entirely ("PFTL only") or use an unrestricted ERC-20 that bypasses KYC ("PFTL and Ethereum"). The spec fragments these trade-offs across R11, R12, Q6, and Q7 without addressing their combined impact.
- **Why it matters:** The founder's prompt treats the London FX Fix, Uniswap pools, and fast settlement as core NAVCoin primitives. Shipping a KYC pillar under which a regulated NAVCoin cannot participate in the FX fix or compliant secondary liquidity risks producing a technically compliant asset that no issuer will actually use.
- **How to investigate:** Inspect `fx_fix_types.rs` and `orchard_state_application.rs` to determine how tightly `FxFixPacketV1` is coupled to Asset-Orchard/PFSH03, and check `NAVGuardHook.sol` and the Ethereum wrapper contract to assess the effort for a transfer-restricted ERC-20 (e.g., ERC-3643 / Merkle-root allowlist) vs. a transparent PFTL-native FX fix.
- **Suggested direction for fix:**
  - Call out this trade-off prominently in the Architecture section so the founder sees the full picture.
  - Evaluate adding a **transparent (non-shielded) FX Fix settlement path** on PFTL for gated NAVCoins (calling `asset_gate` directly on transparent accounts), which unblocks the London FX Fix without waiting for ZK credential circuits inside PFSH03.
  - Define a clear two-phase liquidity roadmap for regulated NAVCoins (Phase 1: PFTL native DEX + transparent FX Fix; Phase 2: credential-gated wrapper or PFSH03 circuit proofs).

---

##### P3 — Issuer key compromise, key rotation, and immutable policies can permanently brick gated NAVCoins
- **Severity:** major
- **Where:** Technical requirements — R4, R5, R10; Open questions Q3.
- **What is wrong:** R5 ties issuer identity directly to a single account and handles key compromise exclusively by deregistering the issuer, which invalidates *all* credentials that issuer ever created. There is no key rotation mechanism, no separation between "stop trusting new issuances from key K" and "invalidate prior valid credentials," and no defined authority (e.g., backup key or governance) to deregister an issuer whose key is lost. Furthermore, R10 binds `credential_policy` to `(issuer, type)` pairs; if `issuer` is a single key-bound account and `credential_policy` is immutable (Q3a), rotating or deregistering a KYC vendor permanently bricks onboarding for every NAVCoin referencing that vendor.
- **Why it matters:** Routine key rotation is mandatory for regulated KYC vendors, and key loss/compromise is an operational certainty. Mass-invalidating all legitimate holders on rotation or bricking live NAVCoins is unacceptable in production.
- **How to investigate:** Review how `NavAttestor` in `postfiatl1v2` represents issuer identities and updates keys/domains, and check how governance transactions interact with protocol registries.
- **Suggested direction for fix:**
  - Introduce a stable `issuer_id` (derived at registration) decoupled from an active signing key (or key set), supporting key rotation with an optional `revoked_before_height` cutoff for compromise vs. routine rotation.
  - Specify an emergency freeze/deregistration path (issuer recovery key + governance backstop) for lost or actively abused keys.
  - Recommend Q3(b) or Q3(c) (notice-period policy updates) as the default instead of immutable policies (Q3a).

---

##### P4 — DEX Offers and Escrows still require explicit `trust_set` for permissionless assets, and `TrustLine` lacks an explicit-limit flag
- **Severity:** major
- **Where:** "In one paragraph", "What exists today" (`offer-setup.js`), Enforcement table Rows 2 & 3, R14.
- **What is wrong:** First, the spec promises that anyone can receive a default NAVCoin without setting a trust line, yet Row 2 (Offers), Row 3 (Escrow), and `offer-setup.js` require pre-existing trust lines even for permissionless default assets. Second, R14 states that *"a limit the holder set explicitly binds on every path,"* whereas implicit lines currently store `limit = amount` and auto-raise on payments/mints—there is no specified field on `TrustLine` distinguishing an explicit user-chosen limit from an implicitly populated one (and `reserve_paid == 0` is fragile if fees are waived or lines are later modified).
- **Why it matters:** Forcing users (and wallets like Task Node) to send a `trust_set` transaction before placing a DEX buy offer on a permissionless NAVCoin breaks the founder's "trust lines are an option, not a default" UX goal. Without a schema change on `TrustLine`, R14 cannot be implemented deterministically.
- **How to investigate:** Inspect `TrustLine` in `crates/types/src/account_owned_asset_types.rs`, offer execution in `crates/execution/src/issued_asset_ledger_helpers.rs`, escrow release in `nft_escrow_asset_execution.rs`, and `src/features/wallet/l1/market/offer-setup.js`.
- **Suggested direction for fix:**
  - Extend implicit line creation (subject to `asset_gate`, R13 fees, and R14 refuse flag) to Offer settlement and Escrow release so default NAVCoins never require `trust_set` on the native DEX—and remove the mandatory `trust_set` step from `offer-setup.js` for default assets (or, if deferred, explicitly qualify the top-level claim).
  - Update `TrustLine` to store `explicit_limit: Option<Amount>` (where `None` means uncapped implicit line and `Some(cap)` enforces a binding ceiling on every credit path).

---

##### P5 — Unaddressed execution edge cases: resting DEX offers, clawback evasion, credential renewal, and `trust_set` de-authorization
- **Severity:** major
- **Where:** Enforcement table Rows 2, 3, 6; R0, R2, R3, R6; Acceptance Criterion 5.
- **What is wrong:** Four concrete state-machine gaps are unverified or unspecified:
  1. **Resting offers on expiry/revocation:** If a maker's credential expires while an offer sits on the orderbook, it is unspecified whether takers fail on matching (orderbook poisoning) or the engine auto-skips/prunes the offer, and under Q1(a) Strict the maker cannot even cancel and recover locked units.
  2. **Clawback evasion:** Row 6 notes `AssetClawback` bypasses movement checks on free balance, but does not verify whether a holder can evade clawback by locking gated assets inside an Escrow or far-out-of-the-money DEX Offer (potentially front-running revocation/clawback in the mempool).
  3. **Credential renewal before expiry:** R6 indexes records by `(subject, issuer, type)`. If an issuer renews a user's KYC before `expires_height`, overwriting the record with `accepted = false` would lock the user out until they sign `credential_accept`, whereas rejecting duplicate creation forces expiry/deletion gaps.
  4. **Clearing `line.authorized`:** R0, R2, and Acceptance 5 assume the issuer can clear `line.authorized = false` via `trust_set`, but XRPL famously forbids clearing authorization once set; the spec never verified whether `nft_escrow_asset_execution.rs` actually permits de-authorization today.
- **Why it matters:** Any one of these four gaps causes live fund-stranding, compliance bypasses, or orderbook liveness failures on mainnet.
- **How to investigate:** Read `nft_escrow_asset_execution.rs` (for `TrustSet` de-authorization and `AssetClawback` balance scope) and the offer-matching loop in `crates/execution/` (for how failed maker checks behave during book walking).
- **Suggested direction for fix:**
  - Verify (and if needed implement) `authorized = false` clearing in `TrustSet`.
  - Define orderbook auto-pruning/skipping for makers failing `asset_gate`, and always allow offer cancellation / self-refund of own locked assets (or explicitly handle issuer-side cancellation).
  - Specify whether `AssetClawback` can force-cancel open offers or reach escrowed balances for sanctioned accounts.
  - Define renewal semantics in R6: calling `credential_create` (or `credential_renew`) on an existing **accepted** `(subject, issuer, type)` record should update `expires_height` and `commitment` while preserving `accepted = true` (with an option for the subject to delete at any time).

---

##### P6 — Bridge return-import recovery (R11) and FastSwap freeze enforcement (R12) are underspecified
- **Severity:** major
- **Where:** Enforcement table Rows 7, 9, 10, 15; R11, R12.
- **What is wrong:** R11 states that a return import from Ethereum (Row 7) to an account failing the gate *"must leave the units recoverable. This is defined before any gated asset is exported,"* without specifying how recovery works—and without extending the same requirement to Row 9 (vault bridge) and Row 10 (Solana bridge). Meanwhile, R12 allows FastSwap fast-lane permits to omit early revocation if permits are kept "short," which means a **frozen** (sanctioned) account can keep trading on the fast lane until its permit expires.
- **Why it matters:** Cross-chain return imports happen *after* tokens are burned/locked on the external chain; a plain ledger revert either permanently strands user funds or stalls sequential bridge queues. And permitting a frozen account to trade during a permit window violates the R2 invariant ("Freeze overrides everything").
- **How to investigate:** Inspect `nav_vault_asset_execution.rs`, `solana_bridge.rs`, and `fastswap.rs` / `fastswap_control.rs` to see how failed bridge credits are currently handled and how `FastHolderPermitV1` interacts with account/line freeze checks.
- **Suggested direction for fix:**
  - Specify a dedicated **quarantine / claimable-escrow ledger object** for failed inbound bridge credits across Rows 7, 9, and 10: tokens land in a non-transferable quarantine bucket keyed by `(asset, recipient)` that can be claimed once `asset_gate` passes, returned under the asset's exit rule, or clawed back by the issuer.
  - Require FastSwap fast-lane execution to check line freeze state (or add an immediate permit-revocation / freeze-epoch check) so freeze overrides fast-lane permits immediately.

---

##### P7 — Missing proof-of-possession (account binding) in KYC/passport flows and unclear Step 1 passport verifier operator
- **Severity:** major
- **Where:** User Story Step 3, R7, R9, R15.
- **What is wrong:** In R6/R9, the vendor issues `credential_create` for a user-supplied PFTL address and the user signs `credential_accept`. This does **not** prove to the vendor that the human who passed KYC owns that PFTL address—Alice can pass KYC and submit Bob's address, and Bob simply calls `credential_accept` (trivial KYC-renting / credential-selling attack). Because PFTL uses ML-DSA-65, off-the-shelf KYC widgets cannot verify wallet ownership natively. Additionally, R9 assumes *"In Step 1 a registered verifier checks the proof off chain and issues the credential"* without specifying who runs this passport verifier, how it is audited/secured, or how R15's "fresh account for gated assets" interacts with passport uniqueness/sybil nullifiers.
- **Why it matters:** Without wallet proof-of-possession during KYC onboarding, gated NAVCoins are trivially bypassed by third-party KYC-for-hire. And an unspecified "registered verifier" for passport proofs leaves a major trust and operational hole in one of the founder's two explicit pathways.
- **How to investigate:** Review the `did:pftl` ML-DSA-65 signing primitives in `postfiatl1v2` and Task Node wallet signing APIs, and evaluate concrete passport ZK SDKs (e.g., Self / OpenPassport / ZkPassport) for how they bind a user address / public signal inside the ZK proof.
- **Suggested direction for fix:**
  - Specify a standard `did:pftl` ML-DSA-65 challenge-response proof-of-possession flow inside the vendor adapter (R9): the adapter issues a session nonce, the wallet signs it, and the adapter verifies it against `did_resolve` before calling `credential_create`.
  - For ZK passport proofs, bind the target `did:pftl` address directly as a **public input inside the ZK proof** so the proof cannot be replayed for another address, and specify the concrete Step 1 verifier operator model (e.g., open-source stateless verifier service with reproducible builds, plus a clear path to on-chain verification).
  - Clarify how a vendor handles fresh-account re-issuance (R15) vs. uniqueness nullifiers and cross-account sanctions revocation.

---

##### P8 — Unanalyzed fee/reserve economics for implicit lines (R13) and credentials (R6, Q8)
- **Severity:** major
- **Where:** "What exists today", R6, R13, Open question Q8, Acceptance Criterion 6.
- **What is wrong:**
  1. R13 charges the sender `TRUSTLINE_STATE_EXPANSION_FEE (10)` on implicit-line creation without stating the unit (`10` drops vs. `10` PFT) or analyzing its economic effect. If `10` is negligible, it fails to prevent dust/spam lines (while R14's refuse flag is off by default); if `10` is meaningful, deleting empty lines (Q8) lets recipients force senders (including automated NAV payout/redemption contracts) to pay the fee repeatedly, and breaks fee estimation for wallets that don't know if a recipient has a line.
  2. R6 creates on-chain objects on `credential_create` before acceptance, yet Q8 leaves fees open ("Charge or waive fees") without considering XRPL's refundable **owner-reserve handoff** (issuer locks reserve on create → transfers to subject on accept → refunds on delete).
- **Why it matters:** Broken state-rent economics either invite unbounded ledger bloat (unaccepted credential spam, asset dusting) or create high-friction UX penalties on permissionless NAVCoins.
- **How to investigate:** Check `TRUSTLINE_STATE_EXPANSION_FEE` and native token decimal precision in `crates/execution/src/fees_offer_planning.rs` and `xrpl-feature-parity-burndown.md`, and verify how owner reserves vs. burned fees are implemented elsewhere in `postfiatl1v2`.
- **Suggested direction for fix:**
  - Document the exact unit and PFT/USD magnitude of `TRUSTLINE_STATE_EXPANSION_FEE`, and analyze whether a **refundable reserve** (returned on empty-line deletion) or a low burned fee + rate-limited dust protection is coherent with R14.
  - Adopt XRPL's reserve-handoff model (or a refundable creation deposit) for `credential_create` / `credential_accept` / `credential_delete`, plus a small bounty/refund to whoever cleans up expired credentials, so state cleanup is self-incentivized and unaccepted spam costs the spammer capital.

---

##### P9 — Missing formal bounds on DNF policies and height-boundary conventions
- **Severity:** minor
- **Where:** Technical requirements — R2, R10.
- **What is wrong:** R10 allows a `credential_policy` to be an OR-list of AND-sets of `(issuer, type)` pairs (Disjunctive Normal Form), but refers only to a single bound ("fixes the bound"). Bounding only outer rules allows arbitrarily large inner AND-sets, and multi-leg transactions (e.g., an offer crossing 20 resting offers) multiply `asset_gate` evaluations per transaction. Additionally, R2 specifies `expires_height` without defining inclusive vs. exclusive boundary semantics (`height <= expires_height` vs. `<`) or how non-expiring credentials are represented (`Option<u64>` vs. `0`).
- **Why it matters:** Unbounded nested policies risk block-execution DoS, and ambiguous height boundaries cause off-by-one consensus discrepancies across `Credential`, `FastHolderPermitV1`, and `FxFixPacketV1`.
- **How to investigate:** Check `FastHolderPermitV1` (`fastswap_types.rs`) and `FxFixPacketV1` (`fx_fix_types.rs`) for existing height-comparison conventions, and check max-offer-crossing limits in `crates/execution/`.
- **Suggested direction for fix:** Define explicit protocol constants (`MAX_POLICY_RULES`, `MAX_PAIRS_PER_RULE`, and `MAX_TOTAL_POLICY_PAIRS`, e.g., ≤ 16 total pairs so worst-case lookup cost is fixed) and match the existing codebase's exact `Option<Height>` and inclusive/exclusive expiry rule.

---

##### P10 — Founder decision overload (13 unprioritized open questions) and minor PQ boundary overstatement in R16
- **Severity:** minor
- **Where:** "Open questions for the founder" (Q1–Q13) and R16.
- **What is wrong:** Presenting 13 unranked, multi-part questions without recommended defaults blocks implementation on low-level choices (e.g., Q5 bonds when slashing doesn't exist, Q8 fee minutiae, Q9 refuse flag, Q11 on-chain passport proofs, Q13 namespace governance). Separately, R16 asserts post-quantum security for credentials without noting that underlying e-passport CSCA signatures (RSA/ECDSA), classical ZK passport proofs, and TLS vendor webhooks remain classical—only the on-chain credential attestation is ML-DSA-65.
- **Why it matters:** Engineering velocity stalls when tactical decisions are escalated alongside strategic ones; and an imprecise PQ claim could mislead auditors or institutional issuers about the cryptographic trust boundary.
- **How to investigate:** Review Q1–Q13 against existing specs (`07-navcoin-fees.md`, `navcoin-create-and-swap-spec.md`, `identity-flag-pipeline-spec-20261023.md`) to separate true founder/legal policy calls from engineering defaults.
- **Suggested direction for fix:** Trim the founder decision list to the ~5 strategic/legal calls (Q1 exit rule, Q4 compliance liability/registry model, Q6 shielded/FX-fix path, Q7 wrapper compliance vs. liquidity, Q10 initial KYC/passport partners), and give every other item a crisp **recommended default** that proceeds unless overridden. Add one sentence to R16 clarifying that on-chain attestations are PQ (ML-DSA-65) while upstream passport/vendor verification channels inherit classical cryptography.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 9925 output tokens*

#### Summary judgement

This is a strong, code-grounded sub-page. It answers the founder's direct question: NAVCoins do not need trust lines, and trust lines are already optional. It proposes a credible opt-in design modelled on XRPL Credentials, Permissioned Domains and W3C VC. It also surfaces a real prerequisite bug: implicit lines are created authorized even on gated assets.

The 16-path enforcement table is the most valuable part. It is the kind of exhaustive inventory that stops a compliance gate from being bypassed in a corner.

The core movement model has internal flaws:

- **Credential-created lines persist as issuer-authorized.** As written, R0 plus R2 make credential revocation ineffective.
- **Rejecting credits on bridge and shielded-exit paths can strand or destroy value.** The design does not specify refunds.
- **Revocation by deletion cannot produce `credential_revoked`.** Deleting the record removes the information needed for that error code.
- **The off-chain VC interop story is unclear.** It assumes vendors sign as `did:pftl` with ML-DSA-65.

Economically, the page is honest about the liquidity split. It does not state the product consequence plainly: a gated NAVCoin, under the defaults chosen, loses most of the founder's listed NAVCoin core elements.

The page also lacks sequencing against the founder's near-term priority, which is NAVCoin swaps. A large DID/VC/passport programme risks crowding out the urgent pieces: R0 and the fee fix.

#### Correspondence to the founder's intention

**The founder's Deliverable 8 asks five things, and the page addresses each:**

1. *DID-type things on the XRPL.* R7 defines `did:pftl`, modelled on the XRPL DID object, and R6 adopts the XRPL Credentials lifecycle.
2. *Best standards for external KYC or passport systems.* R8 maps fields to W3C VC 2.0. R9 defines a vendor adapter and a ZK passport pathway.
3. *Use these to KYC NAVCoins.* `credential_policy` sits on `AssetDefinition`, and Pillar 7 is linked.
4. *Do NAVCoins need trust lines?* The page answers "No" in its first sentence, with code evidence (implicit lines).
5. *Trust lines an option, not a default; permissionless by default.* The page confirms this already holds. R17 makes it explicit in docs. The XRPL comparison table explains what replaces each XRPL reason for trust lines.

**Where correspondence weakens:**

- **NAVCoin core elements.** The founder lists them as a Uniswap asset, an on-chain PFTL asset, USDC↔PFUSDC swaps, and a PFUSDC pool ready at a "fix". The page's defaults bar gated assets from the shielded pool, and the fix settles as a private swap. The Ethereum wrapper either moves freely (option b) or splits liquidity. So a "KYC'd NAVCoin" either loses the fix and possibly Uniswap, or its gating is cosmetic. The founder should be told this in one sentence near the top, not discover it across R11, R12, Q6 and Q7.
- **Near-term urgency.** The founder flags NAVCoin swaps, a Hyperliquid NAVCoin tracked on Solana, as the "very near term thing". The page offers no phasing that separates "fix now" (R0, R13) from "later" (DID method, passport ZK, issuer registry).
- **Hyperliquid/Solana.** This is deferred to "later venues follow the same choice". That is reasonable, but it is thin given the founder's explicit Solana/Hyperliquid interest.
- **Revision notes.** The table is reviewer-facing noise for the founder. It is not harmful.

Overall the correspondence is high, about 85%. The gaps are about consequences and priority, not missing topics.

#### Technical robustness

**Strengths:**

- A single gate function (R1) with a reviewed path inventory.
- Live evaluation with no caching (R2).
- Height-based expiry, consistent with existing permits and fix packets.
- Activation through `AssetExecutionCompatibility` for replay safety.
- A pre-activation audit of bug-created lines.
- An indexed (subject, issuer, type) lookup and a bounded policy, plus a benchmark.
- An acceptance matrix with six account kinds per path.

**Weaknesses (detailed in Problems):**

- The `line.authorized` semantics are overloaded (P1).
- Rejection semantics on non-atomic, cross-domain credit paths are undefined (P2).
- Revocation-status persistence contradicts the error codes (P3).
- The VC issuer and DID model is likely incompatible with real vendor tooling (P4).
- The issuer-source exception in R0 is under-specified for paths where the issuer is not the transaction source (P5).
- The permit revocation gap is acknowledged but unresolved (P9).
- Mass invalidation when an issuer is deregistered has no exit-rule interaction defined (P11).

#### Economic coherence

**Coherent:**

- Charging the sender the existing state-expansion fee for implicit lines (R13) closes a real free-state hole. It matches the atomic-swap legs and the parity tracker rule.
- The refuse flag and binding limits (R14) address spam.
- Bonds without slashing are correctly called worthless.
- The point that "regulated vendors rarely post token bonds" is realistic.

**Incoherent or under-examined:**

- **No unit or magnitude for the fee.** The state-expansion fee (10) has no stated unit or size relative to spam cost or to mass distributions. Examples are task payouts, airdrops and fee mints to many holders.
- **No payer on bridge and shielded credits.** It is unclear who pays the implicit-line fee on bridge deposits, Solana credits and shielded exits, where there is no ordinary "sender".
- **Gated NAVCoin unit economics are missing.** Vendor KYC costs per user (Q8), fragmented liquidity (Q7) and loss of the fix (Q6) together may make gated NAVCoins non-viable at small scale. This is not modelled.
- **Option 4(b) carries legal and economic risk.** It makes Post Fiat governance a compliance gatekeeper. The page mentions this but does not connect it to B7.2 liability in a recommendation.

#### Problems

**P1 — Credential-based lines become permanently authorized, so revocation does nothing**
- **Severity:** critical.
- **Where:** R0 ("an implicit line is created authorized only in two cases: the gate passes (R2), or…"), together with the R2 formula `!frozen && (!requires_authorization || line.authorized || policy_met)`.
- **What is wrong:** suppose the line is created because `policy_met` was true, and R0 sets `authorized = true`. After revocation or expiry, `line.authorized` is still true, so `can_move` stays true. Revocation and expiry never take effect for any holder who first received the asset via credential. This also contradicts R2's claim that `line.authorized` is "the issuer's manual flag".
- **Why it matters:** it defeats the entire KYC gating purpose. Acceptance tests 3 and 4 would fail, or would be written around the bug.
- **How to investigate:** trace R0's line-creation branch against R2's formula for a revoked-credential holder. Check whether `TrustLine` has room for a separate flag.
- **Suggested fix:**
  - Create credential-path lines with `authorized = false`, and rely on live `policy_met`.
  - Set `authorized = true` only on issuer approval, meaning an issuer-signed `trust_set` or an issuer-sourced credit.
  - Optionally add a `authorization_source` enum.
  - Add an explicit acceptance test: receive via credential → revoke → send fails.

**P2 — Rejecting credits on bridge, vault, shielded-exit and return-import paths can strand or destroy funds**
- **Severity:** critical.
- **Where:** R0 ("Otherwise the credit is rejected"), applied to rows 7, 9, 10 and 13. R11 defines recoverability only for row 7.
- **What is wrong:** these credits finalise value that has already left another domain. Examples are a Solana deposit, an Ethereum burn and import, a vault receipt, and a spent shielded note. A plain rejection on PFTL has no counterpart refund. The units become unclaimable, or the transaction becomes non-includable and blocks the bridge queue.
- **Why it matters:** this means loss of funds and liveness risk on bridges. It also creates a possible griefing vector: deposit to an ineligible account to jam processing.
- **How to investigate:** for each of `credit_verified`, the vault deposit claim and the mint from receipts, `credit_issued_asset_balance_from_custody`, and `credit_issued_asset_from_shielded_pool`, determine:
  - whether the source-domain action is already final;
  - whether failure aborts the whole transaction or only the credit.

  Also check the shielded exit for nullifier spend ordering.
- **Suggested fix:**
  - Define a uniform "quarantine/escrow-to-claim" outcome for gated credits that fail the gate. Credit an issuer-controlled or recipient-claimable holding that releases on authorization or returns via the exit rule.
  - Alternatively, require the gate to be checked before the source-domain action where possible, for example by a pre-check at export or shielded spend.
  - Make R11's recoverability requirement apply to all of rows 7, 9, 10 and 13.

**P3 — Revocation-by-deletion cannot produce `credential_revoked`**
- **Severity:** major.
- **Where:** R6 (`credential_delete` by the issuer as revocation, by anyone after expiry), user story step 5, and acceptance 3.
- **What is wrong:** if issuer revocation deletes the record, the gate sees no record and returns `missing_credential`. It cannot distinguish revoked from never-issued. Deletion by anyone after expiry also makes `credential_expired` unreachable in practice. The `status` field exists, but the lifecycle never sets it.
- **Why it matters:** the error codes and explorer semantics are wrong. Compliance audit trails lose the revocation event, and only history remains.
- **How to investigate:** check whether the XRPL Credentials amendment keeps any tombstone; it does not. Decide whether the error codes are needed.
- **Suggested fix:**
  - Either make issuer revocation set `status = revoked` and keep a tombstone (prunable after expiry, with a fee refund);
  - or drop the distinct codes and document that revocation is visible only through transaction history.

**P4 — The W3C VC issuer and DID model is likely incompatible with KYC vendor tooling**
- **Severity:** major.
- **Where:** R7, R8 (issuer/subject both `did:pftl`) and R16.
- **What is wrong:** a W3C VC whose `issuer` is `did:pftl` must be proof-signed by that DID's verification method, which is ML-DSA-65. Mainstream VC stacks and KYC vendors sign with Ed25519 or ECDSA under `did:web`, `did:key` or their own DIDs, and no standard cryptosuite exists for ML-DSA-65 yet.
  - So either the off-chain VC is vendor-signed under a different DID, and the mapping table is wrong;
  - or the adapter re-issues the VC, and then Post Fiat infrastructure is the VC issuer, with liability implications.

  The `commitment` is mapped to XRPL `URI`, which is semantically different.
- **Why it matters:** "interop with W3C" may be nominal. Q10 ("do they already issue W3C credentials?") is the right question, but the design presumes the answer.
- **How to investigate:**
  - Survey 2–3 target vendors and ZK passport apps for output formats.
  - Check W3C Data Integrity cryptosuite status for ML-DSA.
- **Suggested fix:**
  - Treat the on-chain record as the authoritative PQ credential.
  - Let the off-chain VC keep the vendor's own DID and signature, with `commitment` = salted hash.
  - Map `issuer` to "registered PFTL issuer account whose registry entry lists the vendor DID".
  - Document a `did:pftl` ↔ vendor DID linkage in the R5 registry.

**P5 — The R0 issuer-source exception is under-specified and partly inapplicable**
- **Severity:** major.
- **Where:** R0 ("the transaction's source is the asset's issuer").
- **What is wrong:**
  - For `NavMintAtNav` (row 4), the source is the subscriber, not the issuer.
  - For fee mints (row 16), the source may be a protocol action.
  - For bridge credits, the source is a relayer.

  It is unclear whether issuer-signed payments to arbitrary accounts silently authorize lines, bypassing the credential policy. An issuer mistake or a compromised hot key would then create permanently authorized holders.
- **Why it matters:** this produces surprising authorizations and ambiguity in compliance.
- **How to investigate:** enumerate the source account per row of the table.
- **Suggested fix:**
  - Specify per-row which actor counts as "issuer approval".
  - Consider requiring an explicit flag on the issuer payment (`authorize_recipient`) rather than implicit authorization.

**P6 — Gated NAVCoins conflict with the founder's NAVCoin core elements; the product consequence is buried**
- **Severity:** major.
- **Where:** R11, R12, Q6 and Q7. The executive paragraph omits it.
- **What is wrong:** under the Step 1 defaults, a gated NAVCoin cannot use the shielded fix, which is the PFUSDC pool "fix". Its Ethereum wrapper is either free, so gating is theatre, or absent, so there is no Uniswap. The founder's list makes the Uniswap asset and the fix central.
- **Why it matters:** the founder may approve "KYC NAVCoins" expecting full NAVCoin functionality.
- **How to investigate:** produce a matrix of each Q6×Q7 combination against the founder's five NAVCoin elements.
- **Suggested fix:**
  - Add a short "What a gated NAVCoin can and cannot do in Step 1" box near the top.
  - Recommend a default, for example (a)+(b) disclosed for Step 1.
  - Name the path to restore the fix, for example a non-shielded fix variant for gated assets.

**P7 — No phasing against the founder's near-term priorities**
- **Severity:** major.
- **Where:** the page overall. Only "Step 1" is mentioned, in R9 and R12.
- **What is wrong:**
  - Some items fix live risk and are small: R0, R13 and the A666 pool check.
  - The rest is a large programme: the DID method, the registry, credentials, passport ZK, explorer and CLI.

  None of it is sequenced or sized.
- **Why it matters:** the founder's urgent item is NAVCoin swaps, so the bug fix risks being delayed by the bundle.
- **How to investigate:** estimate effort per R-item, and identify which are prerequisites for Pillar 7 and the swap launch.
- **Suggested fix:** define three phases:
  - Phase 0: R0, R13, the shielded entry bar and the A666 audit.
  - Phase 1: registry, credentials, gate and CLI.
  - Phase 2: `did:pftl`, passport, explorer polish and the restricted wrapper.

**P8 — The implicit-line fee economics are under-specified**
- **Severity:** major.
- **Where:** R13 and Q8. The "Line state fees" row lists `TRUSTLINE_STATE_EXPANSION_FEE (10)`.
- **What is wrong:**
  - There is no unit or magnitude relative to spam cost.
  - There is no payer for credits without a natural sender: bridges, shielded exits, fee mints, vault receipts.
  - The impact on mass distributions (task payouts, airdrops) is not considered.

  Never-deleted lines remain a state-growth leak even with the fee.
- **Why it matters:** the fee may be too small to deter spam or too large for distributions. A missing-payer bug could also block bridge credits.
- **How to investigate:**
  - Read the fee constant's unit in `fees_offer_planning.rs`.
  - Model spam cost per GB of state.
  - List every implicit-credit path's fee payer.
- **Suggested fix:**
  - Specify the payer per path; for bridges, deduct from the credited amount or the relayer fee.
  - State the fee in PFT with a rationale.
  - Pair it with line deletion and refund on empty lines.

**P9 — The fast-lane permit revocation gap leaves a compliance window**
- **Severity:** minor/major.
- **Where:** R12 ("Either add early revocation or keep permits short") and row 15.
- **What is wrong:** a revoked holder can keep receiving gated assets via FastSwap until the permit expires. The page does not choose an option.
- **Why it matters:** regulated issuers need bounded revocation latency.
- **How to investigate:** check `fastswap.rs` admission for whether it can read live credential state cheaply.
- **Suggested fix:** check `asset_gate` at FastSwap admission in addition to the permit. Otherwise mandate a maximum permit lifetime in blocks, stated in docs as the revocation SLA.

**P10 — Privacy linkage through the DID document**
- **Severity:** minor/major.
- **Where:** R7 (the DID document includes the Pillar 6 Nostr key and handle) and R15.
- **What is wrong:** an account holding `kyc.basic` credentials becomes publicly linked to a social identity whenever it minted a Hive identity. "Explorer opt-in" is cosmetic because the data is public.
- **Why it matters:** KYC status is a sensitive attribute, and it links to public Hive chat activity.
- **How to investigate:** consider the default wallet flows, for example whether the Task Node uses one account for both.
- **Suggested fix:** have wallets default to a separate account for gated assets, and warn when credentialing an account with a linked Nostr identity.

**P11 — Issuer deregistration cascades without exit-rule interaction**
- **Severity:** minor.
- **Where:** R5.
- **What is wrong:** deregistering one compromised issuer instantly invalidates every holder's eligibility. Under exit rule (a), all holders are stranded at once. It is also unspecified who can deregister: the issuer itself, governance, or the NAVCoin creator.
- **Why it matters:** this is a mass liquidity event or a griefing lever.
- **How to investigate:** combine with Q1 and Q4.
- **Suggested fix:** specify the deregistration authority, an optional grace height, and a recommendation that policies list more than one issuer.

**P12 — `requires_authorization` immutability prevents adding KYC to existing assets**
- **Severity:** minor.
- **Where:** R4.
- **What is wrong:** default NAVCoins can never become gated, and a gated asset can never be opened up. That is sound, but it is not surfaced as a design consequence.
- **Why it matters:** issuers may need migration paths if regulation changes.
- **How to investigate:** consider whether one-way loosening is safe.
- **Suggested fix:** document this, or allow a one-way `requires_authorization → false` change.

**P13 — Acceptance criteria miss an end-to-end product test**
- **Severity:** minor.
- **Where:** Acceptance criteria.
- **What is wrong:** there is no criterion like "a gated NAVCoin is created via the Pillar 7 SDK, a credentialed user subscribes, swaps and redeems, and an uncredentialed user is rejected on every venue". There is also no Solana or Hyperliquid venue criterion.
- **Why it matters:** the unit tests can pass while the product flow fails.
- **Suggested fix:** add one integrated scenario test tied to Outcome C.

**P14 — Small spec ambiguities**
- **Severity:** minor.
- **Where:**
  - Row 6: clawback has no movement check; that is intended, but say so.
  - R10: the policy bound is "fixed by benchmark", with no target.
  - R2: the "never cached" claim interacts with permits in row 15.
  - The future-dated filename `identity-flag-pipeline-spec-20261023.md`: verify it.
- **Why it matters:** these are small sources of implementer confusion.
- **Suggested fix:** clarify each one, and set a provisional policy bound, for example 10 pairs to match XRPL.

**P15 — The revision-notes table adds noise for the founder**
- **Severity:** minor.
- **Where:** the end of the page.
- **What is wrong:** reviewer bookkeeping is mixed into the founder-facing spec.
- **Suggested fix:** move it to a changelog or appendix.
