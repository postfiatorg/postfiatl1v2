# Feedback — Pillar 8 — On-chain Identity (DID, KYC, Optional Trust Lines)

> **Mean score:** 64/100 · Gemini 4 Neon 58 · Claude Opus 5.5 70
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../pillars/08-onchain-identity.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 58/100

*Model `google/gemini-4-neon` · 16174 output tokens*

#### Summary judgement

This sub-page is a strong **initial discovery memo** that accurately grounds Deliverable 8 in the `postfiatl1v2` codebase, directly answers the founder's open question about whether trust lines are already optional, and surfaces a genuine critical security bug (`R0`: implicit trust lines are created `authorized = true` even when `requires_authorization` is set). Structurally, the headline model—permissionless by default, opt-in credential-gated assets—matches the founder's strategic direction.

However, as an executable technical and economic specification, it is **not yet build-ready and would produce a leaky, brittle, or non-compliant system if implemented as written**. Major protocol invariants are contradictory or unspecified (cached authorization vs. live revocation/expiry checks, trapped funds when KYC expires, policy spoofing via decoupled issuer/type lists, implicit trust-line state-bloat and reserve economics). Critical compliance bypasses across Post Fiat's own primitives (shielded transactions, FastPay, PFUSDC/fix settlement, and the Ethereum/Uniswap wrapper) are either omitted or hand-waved. Finally, two explicit parts of the founder's request—**DID standards** and **passport-based (e.g., ZK-verified) identity systems**—are largely absent beneath a generic centralized-attestor model, and the economics of credential issuance, bonding, and gated NAVCoin liquidity are not thought through.

---

#### Correspondence to the founder's intention

##### What aligns well
- **Optional trust lines clearly resolved:** The founder explicitly asked whether trust lines need to be set for NAVCoins and stated that everything should be permissionless by default with opt-in trust-line-based assets. The sub-page audited the execution layer, confirmed implicit trust lines already exist, and surfaced the `R0` bypass bug blocking gated assets.
- **NAVCoin-centric framing:** The spec keeps the focus squarely on enabling both permissionless and regulated/KYC'd NAVCoins, linking to Pillar 7 (NAVCoin SDK) and flagging the Ethereum wrapper gap in the create-and-swap spec.
- **Reference to XRPL and W3C ecosystems:** The spec acknowledges XRPL credential/permissioned-domain amendments and W3C Verifiable Credentials as alignment targets.

##### Where it diverges or falls short
- **No actual DID design:** The founder asked for *"DiD type things on the XRPL and best standards"*, and the page is titled *"On-chain Identity (DID, KYC, Optional Trust Lines)"*. Yet the spec defines **no DID method, no DID Document, no verification-method/key-rotation model, and no resolution mechanism** (e.g., XRPL XLS-40 `DIDSet`/`DIDDelete` or `did:pftl:...`). It simply delegates identity to Pillar 6's Hive Chat / NOSTR identity record, which is a social/messaging key derivation rather than a W3C DID or institutional identity primitive.
- **Passport-based systems are not supported by the architecture:** The founder specifically highlighted *"external KYC systems **or passport based systems**"*. Modern crypto-native passport verification systems (e.g., ZKPassport, OpenPassport/Self, Rarimo) verify NFC ePassport CSCA signatures via zero-knowledge proofs so users prove attributes (nationality, age, uniqueness, non-sanctioned) without a centralized KYC vendor storing PII or signing per-user on-chain attestations. The spec only models a centralized registered attestor calling `credential_issue`.
- **Doesn't explain the "why" behind trust lines:** The founder admitted uncertainty (*"I also do not know if Trust Lines need to be set for NAVCOins or not"*). A founder-serving spec should explain *why* XRPL requires opt-in trust lines (ledger reserve ownership, spam/dust protection, counterparty credit limits, legal non-consent to hold unsolicited assets) and explicitly specify how Post Fiat mitigates those risks once trust lines become implicit.
- **Scope mismatch on multi-chain NAVCoins:** The founder's prompt mentions NAVCoins spanning Uniswap (Ethereum), PFTL, PFUSDC, and *"tracking a Hyperliquid Navcoin on Solana"*. R6 only mentions an Ethereum wrapper and leaves even that unresolved via Open Question #3.

---

#### Technical robustness

The specification has strong intuition at the 10,000-foot level, but breaks down under protocol-level scrutiny:

1. **Broken state-machine semantics for gating and revocation (R0, R4, R5):** R4 implies credential checks happen on line creation to set `authorized = true`, whereas R5 requires movements to fail with `credential_revoked` or `credential_expired` when a credential changes later. Caching authorization on the `TrustLine` contradicts live enforcement. Worse, the spec never distinguishes **receiving** vs. **sending** vs. **redeeming back to the issuer**—meaning an expired passport could permanently trap a legitimate holder's NAVCoin balance, and the precedence between manual `TrustSet` authorization and `credential_policy` is undefined.
2. **Vulnerable and inexpressive `credential_policy` schema (R4):** Splitting policy into two flat arrays (`accepted_types[]` and `accepted_issuers[]`) allows any approved basic-KYC issuer to issue high-privilege credentials (like `kyc.accredited_investor`) and satisfy the policy. It also fails to define AND vs. OR semantics (e.g., `kyc.identity AND kyc.non_us`) and imposes no array-length bounds, creating transfer-time compute DoS vectors.
3. **Massive cross-primitive compliance bypasses:** Post Fiat is simultaneously building **shielded transactions** (Deliverable 5), **FastPay** (Deliverable 2), **NAVCoin swaps / PFUSDC / London FX fix**, and **cross-chain wrappers** (Uniswap/Ethereum). If a KYC-gated asset can enter a shielded pool, move over FastPay without live revocation checks, or trade on a Uniswap pool whose ERC-20 token lacks transfer restrictions, the KYC gate is theater. None of these boundaries are specified.
4. **Implicit trust lines without state-bloat / reserve / opt-out rules:** Allowing any sender to auto-create `TrustLine` state on any recipient account without defining who pays the ledger reserve, what default `limit` is set, and how zeroed lines are pruned opens a trivial ledger state-bloat or recipient-reserve-griefing attack.
5. **Missing subject consent and identity/account decoupling:** Unlike XRPL's actual Credentials amendment (XLS-70), there is no `CredentialAccept` or `CredentialDelete` by the subject—allowing unsolicited credential spam. Coupling credentials directly to Pillar 6 (Hive/NOSTR identity) without defining `AccountId -> Identity` resolution, multi-wallet/institutional custody, or key rotation creates both UX friction and execution-layer ambiguity.
6. **Naive privacy and unrealistic off-chain evidence flow:** Publishing `(identity, KYC_provider, credential_type, expiry, revoked)` permanently on-chain and displaying it on the block explorer creates serious GDPR, doxxing, and targeted-attack risks. Meanwhile, the dependency on Pillar 2 to encrypt raw KYC evidence to the *asset issuer* over IPFS contradicts how regulated KYC vendors operate and shifts massive PII liability onto NAVCoin creators.

---

#### Economic coherence

The economic layer is the least developed part of the sub-page:

1. **Missing state-rent / reserve economics for permissionless assets:** In XRPL, opt-in trust lines exist primarily because ledger state has an ongoing storage cost backed by an owner reserve. Making trust lines implicit without an explicit economic mechanism (sender-paid refundable state deposit, state-creation burn fee, or automatic pruning of zero-balance lines) either subsidizes infinite state bloat or lets attackers lock up victims' reserves via dust airdrops.
2. **Unjustified bonding model for KYC issuers (R1):** Copying `NavAttestor`'s "optionally bonded" pattern onto KYC providers does not make economic sense. Reserve attestors can be slashed against objective or verifiable balance-sheet failures; regulated KYC vendors (e.g., Sumsub, Persona, Jumio) will not lock speculative token bonds subject to subjective governance slashing, and if bonding is optional with no slashing rule defined, it provides zero economic security.
3. **Undefined KYC fee-bearer and portability economics:** Real KYC/KYB costs $1–$500+ per verification plus ongoing sanctions screening and on-chain gas for issuance/revocation. The spec does not clarify who pays, whether credentials are **portable/reusable across multiple NAVCoins** (a major economic moat for Post Fiat if legal reliance is structured properly), or how issuers recover costs.
4. **Unaddressed market-structure & liquidity impact on gated NAVCoins:** Gated assets cannot seamlessly share permissionless liquidity pools or anonymous Uniswap pools without either breaking regulatory compliance or fragmenting liquidity. Furthermore, batch settlement mechanisms like the London FX-style PFUSDC "fix" need rules for participants whose credentials expire mid-batch, and Pillar 7 fee recipients / protocol treasuries need defined exemptions so fee collection doesn't fail compliance checks.
5. **Unanalyzed regulatory liability in Open Question #2:** Punting whether Cobalt governance or individual asset issuers approve KYC vendors ignores a critical economic and legal boundary: protocol-level approval pulls Post Fiat governance/validators toward acting as a regulated compliance gatekeeper, whereas asset-issuer-level whitelisting keeps compliance liability with the NAVCoin creator.

---

#### Problems

##### P1 — Enforcement state-machine contradictions, trapped-asset edge cases, and policy mutability
- **Severity:** critical
- **Where:** Technical requirements (`R0`, `R4`, `R5`), Architecture diagram, Augmented story (steps 4–5).
- **What is wrong:**
  1. `R4` states that the chain *"authorizes a line automatically"* when a recipient has a valid credential, implying it mutates `TrustLine.authorized = true`. Yet `R5` states that a revoked or expired credential makes the line fail at the next movement with `credential_revoked` or `credential_expired`. Checking a cached `authorized` boolean cannot catch subsequent expiry or revocation.
  2. The spec does not distinguish transfer directions when a credential expires or is revoked: blocking **all** movement traps an existing holder's funds and prevents them from redeeming back to the issuer (forcing manual clawbacks for routine passport expiries). Conversely, only checking the recipient (as drawn in the Mermaid diagram) lets a revoked/sanctioned holder liquidate to another credentialed account.
  3. `R4` states that manual `TrustSet` issuer authorization *"still works as an override"*, but never specifies whether a manually authorized line bypasses `R5` credential expiry/revocation checks or is still rejected.
  4. Mutability of `requires_authorization` and `credential_policy` on an already-issued asset is undefined (e.g., whether an issuer can retroactively add a credential policy to a previously permissionless NAVCoin and freeze existing holders).
- **Why it matters:** Ambiguous movement and override rules will produce either compliance bypasses (revoked users still moving assets) or catastrophic user lockouts (holders unable to redeem expired-KYC NAVCoins back to the issuer).
- **How an agent should investigate it:** Inspect `crates/types/src/account_owned_asset_types.rs`, `crates/execution/src/issued_asset_ledger_helpers.rs` (`issued_asset_credit_recipient_line_index`, `ensure_line_can_move`), and `core_asset_execution_tests.rs` to trace every code path that debits, credits, mints, burns, redeems, or freezes issued assets.
- **Suggested direction for the fix:**
  - Separate **line existence** from **live policy evaluation**: for assets with a `credential_policy`, evaluate credentials dynamically during `ensure_line_can_move` (do not rely solely on a cached boolean).
  - Define an explicit truth table for `(sender, recipient)` checks covering: issuer mint, holder-to-holder transfer, **holder-to-issuer redemption (always allowed unless frozen/sanctioned)**, and fee-recipient/treasury credits.
  - Specify explicit precedence between `TrustLine.authorized` (manual issuer override) and `credential_policy` (e.g., `authorized_by_issuer || satisfies_credential_policy`, with issuer able to revoke manual override via `TrustSet`).
  - Lock `requires_authorization` at asset creation (immutable), and define strict rules (e.g., append-only or timelocked/grace-period) for modifying `credential_policy`.

---

##### P2 — Implicit trust lines lack state-bloat protection, reserve accounting, and recipient opt-out mechanics
- **Severity:** critical
- **Where:** In one paragraph, What exists today (permissionless by default), Technical requirements (`R0`, `R7`).
- **What is wrong:** The spec celebrates that `issued_asset_credit_recipient_line_index` already creates missing `TrustLine` records on credit, making trust lines optional—but never addresses how ledger state growth, owner reserves, default `limit` values, dust spam, or recipient opt-out work when any sender can force a new `TrustLine` object onto any recipient account.
- **Why it matters:** On XRPL-derived architectures, trust lines are the primary defense against unbounded ledger state bloat and reserve griefing. Without clear reserve/fee economics, an attacker can cheaply spam millions of 1-unit garbage-token transfers to bloat validator storage, lock up victims' PFTL reserves, or force unwanted/sanctioned tokens onto accounts.
- **How an agent should investigate it:** Inspect `issued_asset_ledger_helpers.rs`, account reserve / fee calculation code in `crates/execution` and `crates/types`, and `docs/status/xrpl-feature-parity-burndown.md` to determine whether `TrustLine` objects currently consume account reserve, what `limit` is initialized to on implicit creation, and whether zero-balance lines are pruned.
- **Suggested direction for the fix:**
  - Specify the state-creation economics explicitly: e.g., when a transfer implicitly creates a recipient `TrustLine`, the **sender** pays a state-creation fee (or refundable deposit attached to the line), NOT the recipient's reserve; and zero-balance implicit lines are automatically deleted (garbage-collected) on full debit.
  - Define the implicit `TrustLine.limit` semantics (e.g., unlimited/sentinel vs. explicit cap) and provide an optional account-level flag (like XRPL `DisallowIncoming` / opt-out blocklist) for institutional or compliance-sensitive accounts that refuse unsolicited assets.

---

##### P3 — Unaddressed compliance bypasses across shielded transactions, FastPay, PFUSDC/fix settlement, and fee collection
- **Severity:** critical
- **Where:** Architecture, Technical requirements (`R4`–`R6`), Dependencies.
- **What is wrong:** The spec treats asset transfers as simple transparent L1 peer-to-peer credits. It completely omits how `requires_authorization` / `credential_policy` interacts with Post Fiat's other core protocol layers described in the founder's prompt:
  1. **Shielded transactions (Deliverable 5):** Can gated assets be shielded? If yes without ZK-credential proofs, anyone can shield a gated NAVCoin and unshield to an un-KYC'd address.
  2. **FastPay (Deliverable 2):** Does FastPay support issued assets, and if so, how are live credential expiry/revocation enforced in fast-path transactions?
  3. **NAVCoin Swaps / PFUSDC liquidity pools / London FX fix:** Do pool/escrow/fix contracts hold gated assets as exempt protocol accounts, or must they hold credentials? What happens if a participant's credential expires between submitting a fix order and fix settlement?
  4. **NAVCoin fees (Pillar 7):** Can management/performance fees be swept to a fee-recipient or protocol treasury address without failing credential checks?
- **Why it matters:** A compliance gate enforced on one transfer path but bypassed via shielding, FastPay, or DEX/pool contracts is completely ineffective legally and technically—and failing to exempt protocol/fee/redemption flows will break NAVCoin operations in production.
- **How an agent should investigate it:** Review the specs and code for Pillars 2 (FastPay/Message protocol), 5 (Task Node Local / shielded transactions), and 7 (NAVCoin fees / `docs/navcoins/navcoin-create-and-swap-spec.md`), plus any DEX/AMM/escrow execution paths in `crates/execution`.
- **Suggested direction for the fix:** Add an explicit **Cross-primitive enforcement matrix** to the spec:
  - **V1 hard rule:** Reject shielding and FastPay for any asset with `requires_authorization = true` at the execution layer until ZK-credential proofs are designed.
  - Define how system/pool/fix/fee-collector accounts interact with gated assets (e.g., explicit issuer-whitelisted settlement contracts, plus graceful per-order failure/refund if a user's KYC expires before fix execution so the whole batch doesn't revert).

---

##### P4 — R6 (Ethereum / Uniswap wrapper gating) is contradictory, underspecified, and misses multi-chain NAVCoins
- **Severity:** critical
- **Where:** Technical requirements (`R6`), What exists today, Open questions for the founder (#3).
- **What is wrong:**
  1. `R6` states *"the hook or the primary market checks the holder list, closing the gap in the create-and-swap spec."* Checking only the primary market leaves secondary Uniswap swaps and ERC-20 transfers completely ungated; checking only a Uniswap v4 hook leaves direct ERC-20 `transfer()` and alternative pools ungated. Only token-level transfer hooks (e.g., ERC-3643 / transfer-restricted ERC-20) actually gate the wrapped asset.
  2. `R6` specifies nothing about **how PFTL credentials sync to Ethereum** (relayer/bridge trust model, gas cost per KYC update, revocation latency, Merkle root vs. signed short-lived EIP-712 attestations).
  3. `R6` is listed as a flat technical requirement, yet Open Question #3 asks whether to defer wrapper gating beyond day one.
  4. The founder also mentioned *"tracking a Hyperliquid Navcoin on Solana"*, but non-Ethereum wrappers (e.g., Solana Token-2022 Transfer Hooks) are ignored.
- **Why it matters:** Because NAVCoins are explicitly designed to bridge between PFTL and Uniswap (and potentially Solana), an ungated wrapper renders PFTL-side KYC legally meaningless for regulated assets, while a naive on-chain holder-list push to Ethereum will be prohibitively expensive in gas and leak holder lists publicly.
- **How an agent should investigate it:** Read `docs/navcoins/navcoin-create-and-swap-spec.md` (including legal gate B7.2) and any Ethereum/Solana bridge/wrapper contracts in the repo to map exact mint, burn, swap, and transfer flows.
- **Suggested direction for the fix:**
  - Clearly phase `R6`: either **Phase 1 (PFTL-only gated NAVCoins; wrappers disabled for `requires_authorization = true` assets at the bridge)** and **Phase 2 (wrapped compliance)**, removing the contradiction with Open Question #3.
  - For Phase 2, specify a concrete, gas-efficient architecture: an ERC-3643-style restricted ERC-20 (and/or Uniswap v4 hook) verifying off-chain signed/Merkle-proven credentials or synced registry roots from PFTL, plus Solana Token-2022 transfer-hook parity if Solana NAVCoins are in scope.

---

##### P5 — `credential_policy` schema allows cross-issuer credential spoofing, lacks AND/OR expressiveness, and has no DoS bounds
- **Severity:** major
- **Where:** Technical requirements (`R4`), Architecture diagram.
- **What is wrong:** `R4` defines `AssetDefinition.credential_policy {accepted_types[], accepted_issuers[], require_unexpired}`. Decoupling `accepted_types` and `accepted_issuers` into two flat lists means any issuer in `accepted_issuers` can issue any type in `accepted_types` (e.g., a lightweight social/passport issuer approved for `kyc.basic` could mint `kyc.us_accredited`). Furthermore, the spec does not say whether multiple entries are evaluated as OR or AND (preventing rules like *"requires BOTH identity KYC AND non-sanctioned/accredited status"*), nor does it cap array sizes or credentials-per-account scanned per transfer.
- **Why it matters:** This is a textbook authorization vulnerability (confused-deputy / privilege escalation across issuers) and makes real-world multi-criteria compliance impossible to express, while unbounded scans introduce consensus execution DoS vectors.
- **How an agent should investigate it:** Review XRPL XLS-80 (Permissioned Domains) design—which uses explicit `(Issuer, CredentialType)` pairs with a hard cap (e.g., 10 pairs)—and check existing collection-size limits and gas/weight metering in `crates/types` and `crates/execution`.
- **Suggested direction for the fix:**
  - Replace the decoupled arrays with explicit `(issuer, credential_type)` tuples, grouped in bounded disjunctive/conjunctive clauses (e.g., a bounded list of up to `N` policy rules where each rule is a small AND-set of `(issuer, type)` pairs, and satisfying any rule passes—or follow XRPL XLS-80's exact bounded `AcceptedCredentials` structure plus optional composite domain support).
  - Index credentials by `(subject, issuer, type)` so verification is O(policy_size) constant-time lookups rather than scanning all credentials on an account.

---

##### P6 — Missing DID specification and missing passport / ZK-proof verification path
- **Severity:** major
- **Where:** Title, In one paragraph, User story, Architecture, Technical requirements (`R1`–`R3`).
- **What is wrong:** Despite the title *"On-chain Identity (DID, KYC, Optional Trust Lines)"* and the founder's explicit request for *"DiD type things on the XRPL"* and *"passport based systems"*, the spec omits:
  1. Any DID primitive (no `DIDSet`/`DIDDelete` equivalent to XRPL XLS-40, no `did:pftl` method spec, no DID Document or verification methods).
  2. Any design for **cryptographic passport-based systems** (e.g., NFC ePassport ZK proofs like ZKPassport / OpenPassport / Self / Rarimo), where a user proves nationality, age, non-OFAC status, and Sybil-resistant nullifier directly via a ZK proof verified against ICAO CSCA root certificates rather than relying on a centralized third-party KYC vendor attesting on-chain.
- **Why it matters:** Relying exclusively on centralized attestors misses one of the founder's two explicit identity tracks, leaves Post Fiat without a true DID standard for external interoperability, and forfeits privacy-preserving passport verification (which is uniquely well-suited to Post Fiat's privacy and global permissionless ethos).
- **How an agent should investigate it:** Survey XRPL XLS-40 (`DID`), XLS-70 (`Credentials`), and XLS-80 (`PermissionedDomains`); evaluate current ePassport ZK frameworks (ZKPassport, Self/OpenPassport, Rarimo) and check what cryptographic precompiles/verifiers (e.g., Groth16/Plonk/Halo2/STARKs) already exist in `postfiatl1v2` (especially from Pillar 5's shielded transaction work).
- **Suggested direction for the fix:**
  - Explicitly define a lightweight on-chain DID record (aligned with XRPL XLS-40 and W3C DID Core) or clearly document how Pillar 6's identity record maps to a valid `did:pftl:<id>` DID Document.
  - Support two credential issuance pathways into the same on-chain `Credential` / policy layer: (a) **Trusted attestor issuance** (for regulated KYC vendors like Sumsub/Persona), and (b) **Verifier-issued / ZK-proof credentials** (where submitting a valid ePassport ZK proof + nullifier against a governed CSCA root registry mints a `passport.zk` credential directly without a centralized vendor seeing PII).

---

##### P7 — Tight, underspecified coupling to Pillar 6 (Hive Chat / NOSTR identity)
- **Severity:** major
- **Where:** In one paragraph, Architecture, Technical requirements (`R2`), Dependencies.
- **What is wrong:** The spec attaches credentials to `subject_identity` (Pillar 6's Hive Chat / NOSTR identity record), but never explains:
  - Whether a regulated NAVCoin investor or market maker must mint a Hive Chat / NOSTR identity just to hold a KYC'd asset.
  - How execution-layer asset transfers (which operate on `AccountId` / ledger addresses) resolve `AccountId -> subject_identity`, and whether an identity can link multiple accounts (hot wallet, cold storage, multisig, institutional custody).
  - What happens to credentials if a user rotates keys, unlinks an account from a Pillar 6 identity, or operates a corporate/multisig account.
- **Why it matters:** Forcing institutional LPs, custodians, and ordinary NAVCoin users to onboard through a social/chat identity module adds unnecessary friction and couples financial compliance state to social-layer upgrades. Undefined account-to-identity resolution will block implementation of `R4`.
- **How an agent should investigate it:** Read the Pillar 6 sub-page (`06-hive-identity.md`) and inspect account/key structures in `crates/types` to see whether Pillar 6 identities are 1:1 with `AccountId`, support multiple linked accounts, or support key rotation.
- **Suggested direction for the fix:** Make credentials attachable directly to an on-chain **Account / DID** (so any Post Fiat account—individual, multisig, or institution—can hold credentials without needing a Hive/NOSTR profile), while optionally allowing Pillar 6's identity record to bind/aggregate linked accounts with explicit inheritance and key-rotation semantics.

---

##### P8 — Missing subject consent (`CredentialAccept`/`CredentialDelete`) and credential lifecycle mechanics
- **Severity:** major
- **Where:** Technical requirements (`R1`, `R2`, `R3`).
- **What is wrong:** `R2` only defines `credential_issue` and `credential_revoke` by the issuer. Unlike XRPL XLS-70—which `R3` claims to align with—there is no `credential_accept` (subject opt-in) and no `credential_delete` (cleanup of expired/unwanted credentials by subject or anyone after expiry). Additionally, the spec omits:
  - How `expires_at` is measured (block height vs. consensus ledger timestamp).
  - How a credential issuer rotates a compromised signing key without invalidating all active user credentials.
  - Any canonical schema/namespacing for `credential_type` strings (`kyc.basic` is ad-hoc).
- **Why it matters:** Without subject acceptance, any malicious account that registers via `R1` can spam arbitrary credentials onto high-profile identities (polluting their explorer profiles, consuming ledger state, or attaching hostile labels). Without issuer key rotation, a single key compromise breaks every gated NAVCoin relying on that KYC vendor.
- **How an agent should investigate it:** Compare `R2` directly against XRPL XLS-70 (`CredentialCreate`, `CredentialAccept`, `CredentialDelete`) and inspect how ledger timestamps and account-key rotation work in `postfiatl1v2`.
- **Suggested direction for the fix:** Adopt XRPL XLS-70's full lifecycle (`CredentialCreate` -> `CredentialAccept` -> `CredentialDelete`/`CredentialRevoke`), allow permissionless pruning of expired credentials to reclaim storage reserve, specify consensus ledger close time (Unix epoch seconds) for `expires_at`, separate issuer **identity/account** from rotatable **signing keys**, and define a versioned `credential_type` namespace registry.

---

##### P9 — Privacy / GDPR metadata exposure and unrealistic Pillar 2 dependency for KYC evidence
- **Severity:** major
- **Where:** Augmented story (steps 3, 6), Technical requirements (`R2`, `R8`), Dependencies.
- **What is wrong:**
  1. Claiming *"No personal data on chain"* and *"never shows anyone's personal data"* overlooks that publicly linking a permanent wallet/NOSTR identity to a named KYC vendor, jurisdiction/accreditation tier (e.g., `kyc.accredited`), issuance timestamp, and revocation event on a public block explorer (`R8`) is sensitive financial/regulatory metadata (and legally treated as pseudonymous personal data under GDPR when combinable with the KYC vendor's database). Unsalted `evidence_hash` values are also vulnerable to correlation attacks.
  2. The Dependencies table states Pillar 2 (Message protocol) will carry *"Off-chain credential evidence envelopes, encrypted to the issuer."* Regulated KYC vendors (Persona, Sumsub, Jumio) run hosted compliance flows and will not ingest raw passport uploads over a custom IPFS protobuf messaging protocol—and encrypting user PII to the *NAVCoin asset issuer* would expose arbitrary NAVCoin creators to severe GDPR/data-breach liability.
- **Why it matters:** Exposing accredited-investor status and KYC revocations on a public explorer creates safety risks (targeted physical/phishing attacks) and regulatory friction, while a broken Pillar 2 assumption creates a phantom dependency that stalls implementation.
- **How an agent should investigate it:** Review Pillar 2 (`02-message-protocol.md`) and `docs/specs/identity-flag-pipeline-spec-20261023.md` to see how existing validator KYC metadata is handled privately, and assess GDPR/right-to-erasure implications of public credential records.
- **Suggested direction for the fix:**
  - Require salted commitments (`H(evidence || random_salt)`) for `evidence_hash` (or omit `evidence_hash` on-chain entirely and keep an opaque issuer-side reference ID).
  - Decouple Pillar 2 from external KYC document submission (document submission happens via the KYC provider's standard SDK/API; only the resulting attestation/VC bridges to PFTL). Optionally keep Pillar 2 only for bilateral issuer-to-holder compliance notices.
  - Evaluate privacy-preserving options for `R8` and on-chain storage (e.g., allow `CredentialDelete` on revocation/opt-out, avoid exposing sensitive sub-classifications on the explorer by default, and consider hashed/blinded credential claims or ZK proofs in Phase 2).

---

##### P10 — Incoherent economic model for credential issuers, KYC costs, gated liquidity, and governance liability
- **Severity:** major
- **Where:** Technical requirements (`R1`), Open questions for the founder (#2), Dependencies.
- **What is wrong:**
  1. `R1` copies `NavAttestor`'s *"optionally bonded"* registration model without defining why a KYC provider would bond capital, what objective condition triggers slashing, or who adjudicates disputes.
  2. The spec ignores who pays KYC verification fees ($1–$500+) and on-chain issuance/revocation transaction fees, and misses the opportunity to design **portable/reusable credentials** (with reliance/fee-sharing mechanics across NAVCoins).
  3. There is no analysis of how gating impacts NAVCoin liquidity pools, market makers, and the London FX-style PFUSDC fix (where permissionless LPs cannot participate in a gated pool).
  4. Open Question #2 (*"governance vs. each asset's issuer"* approving KYC issuers) is punted without analyzing the profound legal/economic consequence: making Cobalt governance the gatekeeper of approved KYC providers concentrates regulatory liability onto Post Fiat governance/validators.
- **Why it matters:** Even a bug-free credential system will fail to get adopted if no real KYC vendor will integrate with its bonding/registration model, if onboarding costs kill NAVCoin UX, if gated NAVCoins have zero liquidity, or if protocol governance takes on avoidable regulatory liability.
- **How an agent should investigate it:** Inspect `NavAttestor` in `crates/types` / `crates/execution` to verify how bonding and slashing actually work there; review `docs/navcoins/navcoin-create-and-swap-spec.md` and Pillar 7 (`07-navcoin-fees.md`) for fee flows, pool mechanics, and legal gate B7.2.
- **Suggested direction for the fix:**
  - Drop bonding from `R1` unless a cryptographically verifiable slashing condition exists; make credential issuer registration a lightweight metadata/domain-verification record (DNS `/.well-known/pftl-issuer.json` or XRPL-style `Domain` field), and recommend **asset-issuer-selected `accepted_issuers`** (not protocol-governance whitelisting) as the default so compliance liability stays with each NAVCoin issuer.
  - Explicitly define the fee flow (who pays tx gas for `CredentialCreate`/`CredentialAccept`—including sponsored/fee-delegated transactions so new KYC'd users don't need PFTL before onboarding) and document portable credential reuse.
  - Add a short market-structure section specifying how liquidity providers, market makers, and fix settlement operate for gated NAVCoins.

---

##### P11 — W3C VC vs. XRPL alignment is hand-waved, and acceptance criteria only test a mock issuer
- **Severity:** minor
- **Where:** Technical requirements (`R3`), Acceptance criteria (#2), Open questions for the founder (#1).
- **What is wrong:** `R3` tells implementers to align with *both* the W3C Verifiable Credentials data model (off-chain JSON-LD/JWT/SD-JWT documents with DID URIs and Bitstring Status Lists) and XRPL Credentials/Permissioned Domains (compact binary on-chain ledger objects) *"where practical"*, without specifying the mapping between them. Meanwhile, Acceptance Criterion #2 only validates a mocked local test issuer rather than proving integration with at least one real external KYC or passport verification flow.
- **Why it matters:** Without a concrete field mapping and an adapter pattern (e.g., W3C VC / webhook from KYC provider -> signed on-chain `Credential` object), implementers will either bloat on-chain ledger objects with JSON-LD blobs or build a proprietary format incompatible with every external provider.
- **How an agent should investigate it:** Compare W3C VC Data Model 2.0 core claims (`issuer`, `credentialSubject`, `type`, `validFrom`, `validUntil`, `credentialStatus`) side-by-side with XRPL XLS-70 (`Issuer`, `Subject`, `CredentialType`, `Expiration`, `URI`) and inspect 2–3 concrete provider APIs (e.g., Sumsub/Persona webhooks + Civic/Privado/ZKPassport).
- **Suggested direction for the fix:** Provide a concrete 1-table field mapping (compact binary fields on-chain following XLS-70; full W3C VC JSON/SD-JWT held off-chain by the user/issuer and referenced via optional URI/hash) and add an acceptance criterion for one end-to-end sandbox integration (either a real KYC vendor sandbox webhook adapter or an ePassport ZK-proof verifier demo).

## Claude Opus 5.5 — 70/100

*Model `anthropic/claude-opus-5.5` · 7769 output tokens*

#### Summary judgement

This is one of the stronger kinds of sub-page. It is grounded in the repository and names real files, functions, flags and tests. It answers the founder's direct question ("do trust lines need to be set for NAVCoins?") with a code-backed "no, they are already optional." It also flags a concrete prerequisite: implicit lines are created authorized even on gated assets. The overall shape of the design is sensible:

- assets are permissionless by default;
- issuers can opt in to gating with a credential policy;
- KYC providers issue off-chain-evidenced, on-chain-attested credentials;
- the explorer shows policy but never personal data.

The weaknesses are in the parts that make a KYC layer actually hold:

- **Enforcement across every movement path.** FastPay, shielded transfers, the DEX/AMM and escrow are not covered.
- **Privacy.** Credentials are linked to an identity that Pillar 6 binds to NOSTR and validator keys. This leaks who is KYC'd, which conflicts with the founder's privacy goals.
- **The external wrapper.** R6 assumes Ethereum, while the founder points to a Hyperliquid/Solana track.
- **Economics.** The spec does not say who pays for implicit lines, credentials and issuer bonds. Bonds are not tied to any slashing condition.
- **DID itself.** The founder explicitly referenced XRPL DID standards. The spec offers only "DID-style" credentials with no DID method, document or resolver.

These gaps are fixable. They are, however, the difference between a demo and a regulated-NAVCoin-ready system.

#### Correspondence to the founder's intention

What matches well:

- **"Trust lines are an option, not a default."** The page handles this explicitly and confirms it against code (`issued_asset_credit_recipient_line_index`).
- **"Integrate with various external KYC systems or passport based systems."** R1–R3 cover this through registered credential issuers, evidence hashes, and alignment with W3C VC and XRPL Credentials.
- **"Used to KYC NAVCoins."** R4 and R6 cover this, along with the gated-NAVCoin user story.
- **Continuity with other deliverables.** The page links Pillar 6 (identity), Pillar 2 (message envelopes for evidence) and Pillar 7 (SDK). This respects the founder's habit of wanting primitives to be "natively contiguous."

Partial misses:

- **DID is under-delivered.** The founder said "there are a number of DiD type things on the XRPL." XRPL has a DID amendment (XLS-40), Credentials (XLS-70) and Permissioned Domains (XLS-80). The page mentions credentials and permissioned domains. It never defines a DID method, a DID document or resolution, which is what the founder literally named.
- **The privacy-first framing is missing.** It is a founder-wide theme: shielded transactions, local task node, "production ready needs privacy." The page treats privacy only as "no PII on chain." It does not address linkability or shielded flows.
- **Cross-pillar reuse is missed.** Validator and Finance Hub access gating (Deliverables 3 and 9: "validators or top 30 users") is an obvious second consumer of a credential layer. The page does not mention it.
- **The wrapper venue is wrong.** The founder's near-term NAVCoin plan references Hyperliquid/Solana and Uniswap. R6 speaks of an "Ethereum wrapper."

#### Technical robustness

- The R0 diagnosis is plausible and valuable. It still needs verification that it is a bug rather than intended issuer-credit behaviour, and the fix's ordering semantics are unspecified (see P3).
- Enforcement is described only at the `ensure_line_can_move` / implicit-line helper. A KYC gate is only as strong as its weakest movement path:
  - FastPay, which the founder wants as the default for task payouts;
  - shielded pool entry and exit;
  - DEX offers and AMM;
  - escrow and payment channels;
  - clawback paths;
  - bridge and wrapper mint and burn.
- Some revocation semantics are underspecified:
  - what happens to balances held by revoked holders;
  - whether they can redeem back to the issuer;
  - whether expiry uses deterministic ledger time.
- Credential issuance has no subject-acceptance step, unlike XRPL `CredentialAccept`. This allows credential spam and unwanted attestations.
- Some cryptographic details are missing:
  - whether the evidence hash is salted (a bare hash of passport data is brute-forceable or linkable);
  - which signature scheme is used, given the chain's quantum-resistance posture;
  - how a compromised issuer key is handled.
- The acceptance criteria test only the happy path and the simple reject path on a basic transfer.

#### Economic coherence

- Permissionless-by-default with auto-created lines means anyone can push ledger state onto anyone else's account. The page does not cover reserves or fees for implicit lines, dust or spam assets, or a recipient opt-out such as DepositAuth.
- Issuer bonds are "optional" and have no slashing trigger. A bond without a slashable fault condition does nothing economically.
- The page does not say who pays KYC providers, or whether credential issuance and revocation carry fees.
- Gated NAVCoins on external AMMs fragment liquidity. A Uniswap pool that must check a PFTL-sourced holder list needs an oracle or bridge of credential state. That cost, its latency, and its effect on the PFUSDC "fix" pool are ignored.
- Freezing revoked holders' balances creates stranded-asset risk. For a NAV fund this can be both legally and economically unacceptable. Typically redemption-to-issuer must remain possible.

#### Problems

**P1 — Gating not enforced across all asset-movement paths**
- **Severity:** critical
- **Where:** Technical requirements R0/R4/R5; Acceptance criteria 1–3.
- **What is wrong:** Enforcement is specified only for implicit-line credit and `ensure_line_can_move`. FastPay transfers, shielded pool deposits and withdrawals, DEX offers and AMM, escrow, payment channels, and bridge mints are not mentioned.
- **Why it matters:** Any single unchecked path lets a gated asset reach an unverified holder. That defeats KYC entirely, and FastPay is slated to be the default for payouts.
- **How to investigate:** Grep postfiatl1v2 for every call site that credits an issued-asset balance or creates a `TrustLine`. Check whether the FastPay certificate path and the shielded-note path go through `issued_asset_credit_recipient_line_index` / `ensure_line_can_move`.
- **Suggested fix:**
  - Add a requirement that names a single enforcement chokepoint, or enumerate every path.
  - Add per-path regression tests.
  - Explicitly decide whether gated assets may enter the shielded pool at all (e.g. disallow, or require a ZK credential proof).

**P2 — Privacy and linkability of on-chain credentials**
- **Severity:** major
- **Where:** User story step 3/6; R2; R8; Architecture (credential attached to the Pillar 6 identity).
- **What is wrong:** Credentials are public records attached to an identity that Pillar 6 links to NOSTR, task node and validator keys. The explorer publicly shows "identity X has `kyc.basic` from provider Y." This reveals KYC status, likely jurisdiction and provider relationships.
- **Why it matters:** It conflicts with the founder's privacy requirement and the shielded task node (Deliverable 5). It may also create regulatory and data-protection exposure.
- **How to investigate:** Check what Pillar 6 binds to the identity record. Review the XRPL Credentials privacy notes. Review ZK-credential approaches (selective disclosure, BBS+, Semaphore-style membership proofs).
- **Suggested fix:**
  - Allow credentials to attach to per-asset or pseudonymous sub-identities.
  - Support ZK proofs of credential possession for transfers.
  - Make explorer display of an identity's credentials opt-in.
  - Salt the evidence hashes.

**P3 — R0 fix semantics and flow unspecified, and the "bug" unverified**
- **Severity:** major
- **Where:** "What exists today" bug row; R0.
- **What is wrong:** The page asserts a bug, but an existing test asserts the behaviour. That could be intentional for issuer-originated credit. After the fix it is unclear:
  - how a recipient of a gated asset without a policy ever gets a line (does the recipient create it, or does the issuer pre-authorize a non-existent line?);
  - whether issuer-to-holder credit differs from holder-to-holder.
- **Why it matters:** A wrong fix either keeps the bypass or breaks issuance and existing NAVCoin flows.
- **How to investigate:** Read `core_asset_execution_tests.rs` and the git blame and commit message for the assertion. Compare with XRPL RequireAuth plus preauthorization semantics.
- **Suggested fix:** Specify a state machine covering:
  - line creation by recipient or issuer, and preauthorization;
  - policy-based auto-authorization;
  - issuer-to-holder versus holder-to-holder rules;
  - a test matrix for each.

**P4 — DID layer missing despite the founder's explicit reference**
- **Severity:** major
- **Where:** Title and "In one paragraph" ("DID-style"); R3.
- **What is wrong:** No DID method (e.g. `did:pftl`), DID document, resolution or key rotation is defined. XRPL's DID amendment is not mentioned.
- **Why it matters:** External KYC/VC ecosystems expect a resolvable DID as the credential subject and issuer. Without one, the "little adaptation" claim in R3 fails.
- **How to investigate:** Review XRPL XLS-40 (DID), XLS-70 (Credentials), XLS-80 (Permissioned Domains), W3C DID Core and VC 2.0. Check what Pillar 6 already defines.
- **Suggested fix:** Add requirements for:
  - a DID method spec mapping Pillar 6 identities to DID documents;
  - resolver behaviour;
  - key rotation;
  - a mapping table to the XLS-40/70/80 fields.

**P5 — No subject acceptance and credential spam**
- **Severity:** minor/major
- **Where:** R2.
- **What is wrong:** Issuers can unilaterally attach credentials to any identity. XRPL requires the subject to accept a credential.
- **Why it matters:** Bad actors could attach unwanted or defamatory attestations, and spam bloats state.
- **How to investigate:** Read the XLS-70 `CredentialCreate` / `CredentialAccept` flow.
- **Suggested fix:** Add a `credential_accept` step, with fee or reserve per credential.

**P6 — Revocation leaves balances stranded and the semantics are underspecified**
- **Severity:** major
- **Where:** User story step 5; R5.
- **What is wrong:** "The line stops moving at the next transfer" does not say:
  - whether revoked holders can redeem to the issuer or the primary market;
  - how expiry time is measured (it must use deterministic ledger close time);
  - what grace periods apply;
  - whether incoming transfers to revoked holders are blocked.
- **Why it matters:** For NAV products, locked funds with no redemption path is a legal and economic hazard. Non-deterministic time breaks consensus.
- **How to investigate:** Look at how the NAVCoin create-and-swap spec handles redemption, and at the source of time in execution.
- **Suggested fix:**
  - Always allow transfers back to the issuer and redemption.
  - Define expiry against ledger time.
  - Specify an inbound/outbound rules table.

**P7 — Wrapper venue mismatch and unrealistic external enforcement**
- **Severity:** major
- **Where:** R6; Open question 3; "Private NAVCoin Access panel" row.
- **What is wrong:** R6 targets an "Ethereum wrapper" via a hook or the primary market. The founder's near-term plan is Hyperliquid/Solana tracking plus Uniswap pools. Enforcing PFTL credential state on an external AMM needs a credential oracle or bridge, and its latency and trust model are unspecified.
- **Why it matters:** Gated assets can leak through secondary markets. Liquidity is fragmented, and the PFUSDC fix pool may be incompatible with gating.
- **How to investigate:**
  - Check Pillar 7 and the NAVCoin spec for the actual wrapper chain(s).
  - Assess whether Uniswap v4 hooks can do allowlists.
  - Check Solana Token-2022 transfer hooks.
- **Suggested fix:**
  - Scope v1 to PFTL-only gating.
  - Define an explicit credential-root bridge (e.g. a Merkle root of valid credential holders posted periodically) for external venues.
  - Align the venue list with the founder's Solana/Hyperliquid plan.

**P8 — Economics of permissionless implicit lines (spam and state bloat)**
- **Severity:** major
- **Where:** "Permissionless by default" row; User story step 1; R7.
- **What is wrong:** Auto-created lines let anyone create state on any account. The page does not cover:
  - who pays a reserve or fee for the line;
  - a recipient opt-out such as DepositAuth;
  - spam-asset filtering in wallet and explorer.
- **Why it matters:** These are denial-of-service, state-bloat and phishing-token vectors, and are economically incoherent if lines are free.
- **How to investigate:** Check the fee and reserve model in execution for line creation, and whether any deposit-authorization flag exists.
- **Suggested fix:**
  - The sender pays a line-creation fee.
  - Add an account-level "accept only known assets" flag.
  - Add explorer and wallet spam labelling.

**P9 — Issuer bond without a slashing condition, and no fee model**
- **Severity:** major
- **Where:** R1; Open question 2.
- **What is wrong:** The bond is optional and has no fault definition, adjudicator or slashing. The page has no fee schedule for issuing or revoking credentials and no KYC-provider incentive model.
- **Why it matters:** The economic security of KYC claims is nil, and providers have no reason to integrate.
- **How to investigate:** Review the `NavAttestor` bond semantics, if any exist, and Cobalt governance capabilities.
- **Suggested fix:**
  - Either drop the bond or define slashable faults (e.g. governance-adjudicated false attestation).
  - Specify fees.
  - Clarify whether governance (Cobalt) or each asset issuer approves credential issuers. Recommend per-asset lists plus an optional governance-curated registry.

**P10 — Missed reuse for validator and Finance Hub gating**
- **Severity:** minor
- **Where:** Dependencies; KYC references row.
- **What is wrong:** Validator KYC exists separately (`validator.private.kyc_status`, the identity-flag pipeline). Finance Hub access (Deliverable 9) needs eligibility credentials. The page keeps them separate.
- **Why it matters:** Parallel identity systems cause duplication and inconsistency.
- **How to investigate:** Read `identity-flag-pipeline-spec-20261023.md` and the Pillar 3/9 specs.
- **Suggested fix:** Make validator status, "top-30 task node user" and Finance Hub access credential types in the same layer, or explicitly state why they stay separate.

**P11 — Cryptographic details missing**
- **Severity:** minor
- **Where:** R2/R3.
- **What is wrong:** No signature scheme for credentials (the chain's quantum-resistance posture applies), no evidence-hash salting, and no procedure for issuer key compromise or mass revocation.
- **Why it matters:** These are interop and security gaps.
- **How to investigate:** Check the chain's account signature algorithms and Pillar 4's QR-NFT choices.
- **Suggested fix:**
  - Specify the PQ signature scheme.
  - Specify salted commitments.
  - Specify an issuer-revocation cascade: credentials from a deregistered issuer become invalid.

**P12 — Acceptance criteria too thin**
- **Severity:** minor
- **Where:** Acceptance criteria.
- **What is wrong:** The criteria do not cover:
  - multiple movement paths, including FastPay;
  - expiry;
  - the manual `TrustSet` override;
  - redemption by revoked holders;
  - the external wrapper;
  - performance (the cost of a credential lookup per transfer).
- **Why it matters:** The pillar could pass while remaining bypassable.
- **Suggested fix:** Expand the criteria to a test matrix tied to P1, P3 and P6, and add a benchmark for credential-check overhead.

**P13 — Unverified references**
- **Severity:** minor
- **Where:** Code references; KYC references row (a spec dated `20261023`); `NavAttestor` description.
- **What is wrong:** The dates and file names look suspicious or may be hallucinated.
- **Why it matters:** They erode trust in the "what exists" table.
- **How to investigate:** Confirm each path and symbol exists on main.
- **Suggested fix:** Correct or remove any reference that cannot be found.
