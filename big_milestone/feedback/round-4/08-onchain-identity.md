# Feedback — Pillar 8 — On-chain Identity (DID, KYC & Optional Trust Lines)

*Execution-hole audit · Round 4.*

> **Mean score:** 76/100 · Gemini 4 Neon 76 · Claude Opus 5.5 76
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/08-onchain-identity.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 76/100

*Model `google/gemini-4-neon` · 14884 output tokens · `tech=17/30 econ=13/20 align=19/20 ship=13/15 read=14/15`*

#### Summary judgement

Pillar 8 is well-aligned with every locked founder decision, cleanly written in plain English, and anchored in real codebase reality—especially by prioritizing the active 6-path `issued_asset_credit_recipient_line_index` bypass before layering on credentials. Its high-level architecture (permissionless by default, creator-chosen zero-PII credentials, Return-only default exit, and claimable holds for cross-chain imports) is sound and ready for Wave 1 / Wave 3 execution once its state-machine edge cases are tightened.

However, before engineers start coding, the spec has three critical and five major execution holes lurking at the boundaries between `asset_gate(...)`, Claimable Holds, Pillar 7 epoch finalization, the DEX matching engine, and holder spam controls. Left unresolved, these will re-introduce Solana bridge batch jamming, allow a silent `either`-mode authorization bypass, deadlock NAVCoin epoch finalization when a fee recipient's credential lapses, and open cheap DEX and dust-asset griefing vectors.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (17/30):** Strong diagnosis of existing L1 bugs and clean single-gate intent, but 3 critical and 5 major state-machine, consensus-determinism, and cross-pillar holes remain in holds, `asset_gate`, DEX matching, and Pillar 7 fee accrual.
- **Economic & Incentive Coherence (13/20):** Sound creator-liability and Return-only incentives, but 10-atom auto-lines allow permanent frozen-dust pollution, un-coalesced holds enable free state bloat, takers subsidize dead-maker pruning, and credential deposit refund ownership is unspecified.
- **Founder Decision Alignment & Zero Open Questions (19/20):** Faithfully implements all locked decisions with zero open questions; minor tension only where "on-chain verified domain" implicitly requires off-chain verification to avoid re-introducing a central gatekeeper.
- **Actionable Milestones & 4 Ship Gates (13/15):** Concrete M1–M5 milestones match README Wave 1/Wave 3 sequencing with exact file pointers and all 4 mandatory ship gates; just needs edge-case tests added once the holes below are patched.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Crisp, concise, zero claudese, clear Mermaid flow, and immediately understandable to both founder and engineers.

#### Execution holes

##### H1 — `refuse_unsolicited_assets` and `holder_limit` can still jam Solana/Ethereum bridge batches
- **Severity:** `critical`
- **Where:** *Detailed specification → 1. Prerequisite fix & single `asset_gate(...)` & 2. Permissionless UX & holder controls*
- **Hole:** Claimable Holds are only specified for *gated-asset* ineligibility, while `refuse_unsolicited_assets` and binding `holder_limit` apply to *default permissionless assets* too (like `pfUSDC` or unwrapped NAVCoins).
- **Why it breaks execution:** An attacker can bridge 1 atom of a permissionless asset from Solana to an institutional `postfiatl1v2` wallet that has `refuse_unsolicited_assets = true` or a maxed-out `holder_limit`. If the credit rejects, it fails and jams the entire Solana bridge proof batch (re-creating the exact DoS Claimable Holds were meant to prevent); if it bypasses the flags, it breaks institutional wallet guarantees.
- **Minimal plain-English fix:** Route *every* cross-chain final bridge credit that fails any recipient check (`asset_gate`, `refuse_unsolicited_assets`, or `holder_limit`) into a Claimable Hold—never fail the bridge batch on a recipient-side rule.

##### H2 — Auto-creating gated lines risks a permanent `either`-mode bypass, and `asset_gate(..., direction, ...)` cannot express Return-only or escrow freeze rules
- **Severity:** `critical`
- **Where:** *Locked-in design decisions & Detailed specification → 1. Prerequisite fix & 2. Permissionless UX*
- **Hole:** Two gaps in the single gate design: (1) auto-creating a missing `TrustLine` on a gated asset when a credential passes does not forbid setting `TrustLine.authorized = true`; (2) a binary `direction` parameter cannot distinguish third-party transfers from Return-only exits (burn, issuer return, async NAV redeem request + settlement) or distinguish third-party credits from self-refunds (`offer_cancel`, `escrow_cancel`), nor does it specify whether a frozen escrow *creator* blocks `escrow_finish`.
- **Why it breaks execution:** Under `approval_mode = either` (manual approval OR credential), if auto-creation sets `authorized = true`, the manual-approval bit permanently caches the old credential—so revoking or expiring the credential never takes effect. Separately, a holder frozen after creating an escrow can smuggle frozen tokens out via `escrow_finish` if only the beneficiary is checked, while a Return-only holder may be blocked from settling an async Pillar 7 NAV redemption.
- **Minimal plain-English fix:** Mandate that all auto-created lines on gated assets initialize with `authorized = false` (only explicit issuer `trust_authorize` sets `true`). Replace `direction` with an explicit operation type (`credit_third_party`, `credit_self_refund`, `debit_third_party`, `debit_return_or_nav_redeem`, `clawback`), enforce strict freeze on *both* creator and beneficiary at `escrow_finish`, and explicitly allow Return-only holders to both submit and settle NAV redemptions.

##### H3 — Routing Pillar 7 atomic fee accrual through `asset_gate` can deadlock NAVCoin epoch finalization
- **Severity:** `critical`
- **Where:** *Detailed specification → 1. Prerequisite fix & single `asset_gate(...)`*
- **Hole:** Pillar 7 accrues deterministic NAVCoin fees atomically inside epoch finalization, while Pillar 8 routes "Pillar 7 fee claims" through live `asset_gate(...)`.
- **Why it breaks execution:** If a gated NAVCoin's fee-recipient account loses its credential or is frozen before epoch close, a failing `asset_gate` check inside epoch finalization reverts the epoch transaction—permanently wedging NAV updates, mints, and redemptions for every legitimate holder in the vault.
- **Minimal plain-English fix:** Accrue Pillar 7 fees atomically into an internal vault fee-reserve balance (or a Claimable Hold) during epoch finalization with no per-account `asset_gate` failure path, and run `asset_gate` only when the fee recipient executes a separate `fee_claim` withdrawal transaction.

##### H4 — Claimable Holds are unreachable by clawback, lack direct NAV-exit mechanics, and allow unbounded state bloat
- **Severity:** `major`
- **Where:** *Detailed specification → 1. Prerequisite fix & single `asset_gate(...)`*
- **Hole:** Holds are keyed per `(asset, recipient, source_ref)` with no deposit or coalescing; `asset_clawback` only mentions open offers and escrows (omitting Holds); and Return-only exit from a Hold is asserted without defining how a non-credentialed user redeems at NAV directly from a Hold.
- **Why it breaks execution:** First, a sanctioned actor can bridge a gated asset to an unverified address so units sit in a Hold permanently immune to `asset_clawback`. Second, anyone can bridge tiny dust transfers across thousands of unique `source_ref`s to create permanent un-prunable ledger rows at near-zero cost. Third, an unverified holder cannot call normal `nav_redeem` if it only debits `TrustLine` balances.
- **Minimal plain-English fix:** Coalesce Holds by `(asset, recipient)` (log `source_ref` in events rather than separate state rows), extend `asset_clawback` to sweep Holds, and expose explicit `hold_claim`, `hold_return_to_issuer`, and `hold_redeem_nav` paths operating directly on the Hold balance.

##### H5 — DEX skip-and-cancel of ineligible resting makers enables zero-fill order-book griefing
- **Severity:** `major`
- **Where:** *Detailed specification → 1. Prerequisite fix & 3. On-chain Credentials*
- **Hole:** Takers must evaluate `asset_gate` and skip/cancel ineligible resting makers during matching, within a bounded cross budget (spec benchmarks 64 makers).
- **Why it breaks execution:** An attacker can stack the top of a gated asset's order book with 64+ dust offers from accounts whose credentials soon expire or are revoked by a self-registered issuer. Every legitimate taker then pays transaction fees purely to prune dead offers and hits the cross budget with zero fill—halting DEX liquidity.
- **Minimal plain-English fix:** Add a permissionless `offer_prune_ineligible` transaction so anyone can sweep dead offers, require a refundable offer reserve (or minimum offer size) that pays a small bounty to the pruner/taker when an offer is cancelled for ineligibility, and do not count skipped dead offers against the taker's filled-maker limit (use a separate prune cap).

##### H6 — On-chain "verified domain" contradicts consensus determinism and creates a domain-spoofing or gatekeeper trap
- **Severity:** `major`
- **Where:** *Locked-in design decisions & Detailed specification → 3. On-chain Credentials, `did:pftl` & zero-PII adapters*
- **Hole:** The spec describes issuer registration as "an on-chain record with a verified domain" and namespaces credential types under `<domain>/...`, while locking that Post Fiat governance never approves or polices KYC issuers.
- **Why it breaks execution:** L1 validators cannot deterministically verify DNS/HTTPS domains in consensus. Allowing arbitrary unverified `domain` strings as unique namespaces lets attackers squat `self.xyz` or real KYC vendor domains and spoof issuers on the explorer; conversely, adding an off-chain verifier re-introduces the central compliance gatekeeper the founder explicitly rejected.
- **Minimal plain-English fix:** Key issuers solely by an unspoofable on-chain `issuer_id` (derived from creator account + nonce) and key credential types by `(issuer_id, type_name)`. Store `domain` purely as an unverified claim on-chain that wallets and the explorer verify bidirectionally off-chain via `https://<domain>/.well-known/pftl-issuer.json` or a DNS TXT record referencing `issuer_id`.

##### H7 — 10-atom auto-line creation enables permanent un-deletable frozen-dust account pollution
- **Severity:** `major`
- **Where:** *Locked-in design decisions & Detailed specification → 2. Permissionless UX & holder controls*
- **Hole:** With `refuse_unsolicited_assets` off by default, anyone can auto-create lines in victim accounts for 10 atoms, and line deletion is only permitted for *empty* lines.
- **Why it breaks execution:** An attacker can issue a junk asset with `freeze_enabled = true`, send 1 atom to thousands of user accounts (paying ~10 atoms per line), and immediately freeze the asset. Victims cannot transfer or empty the frozen 1-atom balance, so they can never delete the line—permanently polluting accounts and potentially exhausting the 100k owned-object cap referenced in Pillar 2.
- **Minimal plain-English fix:** Allow holders to unconditionally delete any unsolicited or frozen line by burning/forfeiting its balance back to the issuer (or zeroing it on burn), and align the new-line creation fee/reserve with Pillar 2's anti-spam new-object fee rather than a near-zero 10-atom burn.

##### H8 — Under-specified policy tightening rules, missing issuer-compromise escape hatch, and deposit/admin-key gaps
- **Severity:** `major`
- **Where:** *Locked-in design decisions & Detailed specification → 3. On-chain Credentials*
- **Hole:** Four small gaps in the credential/policy state machine: (1) no deterministic rule defines "loosen vs. tighten" for arbitrary OR-of-AND boolean policies, and waiting 7 days to drop a newly compromised KYC issuer leaves gated assets exposed for a week; (2) compromise cut-off height (`H_cut`) cannot invalidate bad renewals unless each credential records `(last_signed_by, last_signed_height)`; (3) the issuer `admin_account` has no rotation/multisig path (single point of failure for every dependent asset); (4) `credential_delete` (callable by subject or anyone post-expiry) does not specify who receives the refunded deposit or enforce `(issuer_id, subject, credential_type)` uniqueness.
- **Why it breaks execution:** Swapping `(A) OR (B)` to `(A) OR (C)` is ambiguous in consensus; a stolen issuer signing key that keeps minting fake credentials can drain/infiltrate gated NAVCoins during the 7-day notice window; losing an issuer `admin_account` permanently bricks key rotation; and returning a deleted credential's deposit to the *caller* instead of the *payer* lets subjects or bots steal issuer deposits.
- **Minimal plain-English fix:** Define loosening strictly as adding an OR-branch or removing an AND-term (everything else uses a cancelable 7-day pending policy, except an issuer-triggered `suspend_signing_key` / asset-level emergency pause on new mints); store `(signer_key, signed_height)` on credentials; support `admin_account` rotation/multisig; enforce unique `(issuer_id, subject, credential_type)` keys; and always refund deposits to the original payer.

##### H9 — Fast-lane permit revocation lacks caller authorization and a height-freshness bound
- **Severity:** `minor`
- **Where:** *Detailed specification → 1. Prerequisite fix & single `asset_gate(...)`*
- **Hole:** `RevokeHolderPermit` does not state who is authorized to invoke it, and evaluating `asset_gate` at "the lane's finalized height" does not bound how stale that height may be. Also, if any gated-asset units entered a shielded pool prior to activation height, barring shielded *exit* without routing ineligible exits to a Hold could strand pre-activation balances.
- **Why it breaks execution:** A revoked or frozen user could continue submitting fast-lane intents referencing an older finalized height before revocation took effect, or unauthorized parties could grief-revoke permits.
- **Minimal plain-English fix:** Restrict `RevokeHolderPermit` to the asset issuer and the permit holder, enforce a strict `max_height_lag` (or L1 inclusion-height check) on gated fast-lane intents, and bar gated assets on shielded *entry* while routing any pre-existing shielded *exit* through `asset_gate` (falling back to a Claimable Hold if ineligible).

## Claude Opus 5.5 — 76/100

*Model `anthropic/claude-opus-5.5` · 6436 output tokens · `tech=19/30 econ=13/20 align=18/20 ship=13/15 read=13/15`*

#### Summary judgement

This is one of the stronger pillars. It correctly answers the founder's trust-line question ("no, permissionless by default"). It identifies a real, live gating-bypass bug and fixes it first. It routes every movement path through one gate function. It honors every locked decision: creator-chosen issuers, Return-only default exit, the two wrapper modes, a W3C VC adapter plus a ZK-passport verifier, and no issuer bonds. The milestone and ship-gate structure is clean.

The holes sit in the state machine around the gate rather than in the gate itself:

- **Mode changes after launch.** The notice rule does not cover switching the exit rule, freeze, or clawback.
- **Claimable holds.** There is no cost, expiry, or bound, so they become free state bloat.
- **Fast-lane revocation timing.** The spec never says which height the gate is evaluated at for FastPay, so revocation races are undefined.
- **"Return-only" exit.** It promises an exit "at NAV" without guaranteeing a redemption counterparty.
- **Missing concrete defaults.** Deposit size, policy-rule bound, and the time base for credential expiry are deferred, which violates the "concrete defaults" standard.

#### Rubric breakdown

- **Tech (19/30):** Good bug triage, a single enforcement point, and DEX book-poisoning and clawback-reach fixes. However:
  - Hold lifecycle is undefined.
  - Fast-lane race semantics are undefined.
  - Expiry time base is undefined.
  - The issuer-admin single key is a single point of failure.
  - Bug-created lines and existing gated shielded notes are left in limbo.
- **Econ (13/20):** The 10-atom line fee is a solid anti-spam default. But holds are free to create from the source chain, the credential deposit is unspecified, Pillar 7 fee interaction with held or lapsed units is unaddressed, and "send back to issuer" is not an economic exit.
- **Align (18/20):** Every locked decision is reflected, and there is no open-questions section. Points lost only for a few vague parameters ("small deposit", "bounded" policy) that should be concrete defaults.
- **Ship (13/15):** M1–M5 are concrete and the four gates are present and specific. The founder test covers revocation, Return-only exit, and hold recovery. Missing: a test for the activation-height replay and for hold-spam limits.
- **Read (13/15):** Mostly plain English with a clear diagram. Some dense function-name lists in M3 could use one-line glosses.

#### Execution holes

##### H1
- **Severity:** major
- **Where:** Locked-in design decisions → "Policy changes after launch"; Lapsed-eligibility exit rule
- **Hole:** The one-way rule covers only `requires_authorization` and credential-policy tightening. It does not say whether an issuer can later:
  - switch the exit rule from Return-only to strict freeze/clawback;
  - turn on `clawback_enabled` or `freeze_enabled` on an existing asset;
  - change the wrapper mode.
- **Why it breaks execution:** A creator launches a "Return-only, no clawback" NAVCoin and attracts holders. They then flip to strict clawback in one block and claw lapsed holders' units. The "never stranded" promise shown on the explorer becomes false retroactively.
- **Minimal plain-English fix:** Treat any change that reduces holder rights as a tightening that requires the 7-day block notice. This covers:
  - turning freeze or clawback on;
  - Return-only → strict;
  - adding a wrapper restriction.

  Additionally, make `clawback_enabled` false → true impossible on assets with existing non-issuer holders. Show pending changes on the explorer.

##### H2
- **Severity:** major
- **Where:** Detailed spec §1 → Claimable holds
- **Hole:** Holds are created by bridge credits that pay no L1 fee. They have no expiry, no count limit, and no defined behavior when the recipient account does not exist.
- **Why it breaks execution:** An attacker bridges thousands of dust amounts of a gated wrapper token from Ethereum or Solana to random unverified addresses. This creates unbounded permanent ledger entries that count in supply. It bloats state and the NAV denominator, and nobody can clean them up.
- **Minimal plain-English fix:**
  - Charge the hold-creation cost to the bridge relayer or source-side fee.
  - Set a minimum hold amount.
  - Auto-return a hold to the issuer (under Return-only) after a fixed default window, for example 90 days.
  - Allow anyone to trigger that return once the window passes.
  - Define that credits to non-existent accounts go to a hold keyed by address and follow the same expiry.

##### H3
- **Severity:** major
- **Where:** §1 → Fast-lane revocation; §3 → Credential lifecycle
- **Hole:** The spec does not fix the height or time at which the gate is evaluated, which creates two problems:
  - **Fast-lane race:** "Fails the gate at the lane's finalized height" is ambiguous for a FastPay certificate signed while the credential was valid but settled after a `credential_revoke` lands.
  - **Expiry time base:** It is unspecified whether `expiry` is a block height or a wall-clock time. Wall-clock time cannot be evaluated deterministically by validators.
- **Why it breaks execution:** Validators can disagree on whether a certified transfer settles, which is a replay or divergence risk. Alternatively, certified FastPay payments get silently reverted, breaking FastPay's finality promise from Pillar 2.
- **Minimal plain-English fix:**
  - State that `expiry` is measured against the finalized block timestamp or height, never local clocks.
  - State that fast-lane gate checks use the certificate's reference height.
  - State that revocations and freezes take effect for the fast lane only from the next finalized epoch, with a documented one-epoch lag.

##### H4
- **Severity:** major
- **Where:** Lapsed-eligibility exit rule; §1 asset_gate directions
- **Hole:** Return-only promises an exit "at NAV", but nothing guarantees a NAV-redeem path exists for a lapsed holder. If the issuer provides no redemption liquidity, the only guaranteed options are burning or "send back to issuer", both of which mean getting nothing.
- **Why it breaks execution:** An issuer tightens policy, holders lapse, and their only practical exits are giving units away. "Never stranded" holds only in name and becomes a griefing or expropriation path.
- **Minimal plain-English fix:**
  - Specify that `asset_gate` always allows the "redeem at NAV" direction for lapsed holders on Return-only assets.
  - Require gated NAVCoins with Return-only to keep a redeem route (vault redeem or the next FX fix).
  - Make "return to issuer" a NAV-credited return, not a gift.

##### H5
- **Severity:** major
- **Where:** §1 → Close the 6-path gating bypass
- **Hole:** The fix is behind a governed activation height, and the spec only says to "audit and list" lines the bug created with `authorized = true`. It does not say:
  - whether those lines remain authorized after activation;
  - how issuers de-authorize them without stranding the funds;
  - what happens to gated-asset notes already inside the shielded pool before the bar.
- **Why it breaks execution:**
  - Anyone exploiting the bypass before activation keeps permanent authorization.
  - Pre-existing gated shielded notes either cannot exit, stranding them, or exit unchecked, which is another bypass.
- **Minimal plain-English fix:**
  - At activation, mark all bug-created gated lines as "unverified". These behave like a lapsed credential under the asset's exit rule until the issuer explicitly re-authorizes them.
  - Allow unshield of pre-existing gated notes only through `asset_gate` with Return-only semantics.
  - Schedule activation as soon as possible and freeze new gated-asset creation until then.

##### H6
- **Severity:** minor
- **Where:** §3 → Issuer registry
- **Hole:** The issuer has a single admin account that can rotate signing keys and set the compromise cut-off. There is no delay or recovery mechanism.
- **Why it breaks execution:** A compromised admin key can rotate in attacker keys and mint credentials for any subject. It can also set a cut-off that mass-invalidates legitimate credentials, freezing every holder of every asset that trusts that issuer.
- **Minimal plain-English fix:**
  - Allow an optional M-of-N admin.
  - Put a 7-day delay on admin and signing-key rotations, shown on the explorer.
  - Let asset creators drop a compromised issuer from their policy immediately, since loosening needs no notice.

##### H7
- **Severity:** minor
- **Where:** Fees & anti-spam; §3 Policy rules
- **Hole:** The credential deposit is described only as "small", the policy size only as "bounded", and the recipient of a deposit refund on anyone-can-delete after expiry is unspecified. These are deferred parameters.
- **Why it breaks execution:**
  - Engineers will pick inconsistent values.
  - An unbounded or loosely bounded OR-of-AND policy multiplied by a 64-maker DEX cross under live, never-cached evaluation is a block-time DoS.
- **Minimal plain-English fix:** Lock the defaults:
  - **Credential deposit:** for example 1,000 atoms, refunded to whoever paid it.
  - **Policy size:** at most 4 OR-rules, each with at most 4 pairs.
  - **Gate checks:** a per-transaction cap enforced in M3's benchmark.

##### H8
- **Severity:** minor
- **Where:** §1 asset_gate covers "Pillar 7 fee claims"; Claimable holds "counting in supply"
- **Hole:** The interaction with Pillar 7 is undefined:
  - Whether fees accrue on units sitting in holds or on lapsed balances.
  - What happens to accrued fees when the fee recipient's own claim fails the gate.
- **Why it breaks execution:** Fee claims revert forever and accrued units are stranded. Or held units silently pay fees their holder never agreed to while being unable to move.
- **Minimal plain-English fix:**
  - State that held units accrue fees like any other units, because they are in supply (keeping Pillar 7's NAV math exact).
  - State that the asset creator's fee recipient is always gate-exempt for fee claims.
  - Cross-reference this rule in Pillar 7.

##### H9
- **Severity:** minor
- **Where:** §2 → `refuse_unsolicited_assets`, `holder_limit`; §1 holds
- **Hole:** The spec does not define what happens when a bridge credit or NAV mint targets an account that has `refuse_unsolicited_assets` set or would exceed its `holder_limit`.
- **Why it breaks execution:** If these cases reject the credit, the same problem as an ineligible receipt recurs: a final cross-chain credit fails and jams the Solana proof batch.
- **Minimal plain-English fix:** Route cross-chain credits that hit a refuse flag or a holder limit into the same claimable hold, under the same expiry and return rules as H2. Local transfers in these cases simply reject with a typed error code.
