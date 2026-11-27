# Feedback — Pillar 6 — Hive Chat Identity on Chain

*Execution-hole audit · Round 4.*

> **Mean score:** 71/100 · Gemini 4 Neon 72 · Claude Opus 5.5 70
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/06-hive-identity.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 72/100

*Model `google/gemini-4-neon` · 16026 output tokens · `tech=15/30 econ=11/20 align=19/20 ship=13/15 read=14/15`*

#### Summary judgement

Pillar 6 has a strong core architecture and unusually good product clarity: it keeps existing v1 Nostr keys intact, solves ML-DSA signature non-determinism cleanly with a shared v2 seed KDF, puts the `signedCollaborationProof` fix first, and stays completely faithful to the locked founder decisions with readable prose and concrete milestones.

However, it is not safe to implement as written because several consensus state-machine, key-separation, and off-chain/on-chain boundary holes will lead to handle hijacking, domain squatting, account-griefing, and relay DoS on day one. In particular, the spec collapses routine handle issuance and privileged `handle_reassign` into one hot registrar key, mixes off-chain custom domains into a globally unique consensus handle index, leaves `rotate_account` and linked-key `unlink` open to griefing/theft, and leaves fee units and validator link-hopping rules undefined. Closing these 10 holes requires small, concrete consensus and operational rules rather than any architectural redesign.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (15/30):** Strong cryptographic framing (v1/v2 derivation, honest quantum boundary, native object), but 3 critical and 6 major consensus/security/ops holes remain in registrar key separation, custom-domain uniqueness, `rotate_account`, `handle_reserve`, self-`unlink`, kill-switch behavior, and relay write-gating.
- **Economic & Incentive Coherence (11/20):** Burned fees from starter PFT and capped links are sensible, but `"20 units"` has no defined denomination, tombstone/field bloat lacks hard caps, and validator anti-farming has a Sybil and node-hopping gap across Pillar 3 and Pillar 6.
- **Founder Decision Alignment & Zero Open Questions (19/20):** Faithfully implements every locked decision (rotatable Task Node registrar, custom NIP-05, validator-only gating, first-party relay, zero open questions); loses 1 point only for leaving fee denominations underspecified instead of locking concrete engineering defaults.
- **Actionable Milestones & 4 Ship Gates (13/15):** Well-ordered M1–M4 build milestones, real code references, and all four mandatory ship gates with strong founder-test steps; needs explicit acceptance checks for registrar key separation, relay abuse limits, and state-machine edge cases.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Crisp, scannable, plain English with a clear diagram and zero requirement-code bloat.

#### Execution holes

##### H1 — Single hot registrar key can hijack any `@tasknode.postfiat.org` handle
- **Severity:** `critical`
- **Where:** Locked-in design decisions; Detailed specification §2–§3
- **Hole:** One rotatable ML-DSA-65 registrar key operated by Task Node appears to both attest routine handle claims and execute `handle_reassign` on active identities. The spec also omits who authorizes rotating that registrar key on chain and provides no timelock or veto on `handle_reassign`.
- **Why it breaks execution:** Task Node must keep an online hot key to sign handle attestations for everyday user signups. If that server is compromised, an attacker immediately calls `handle_reassign` to steal `goodalexander@tasknode.postfiat.org` or validator handles on chain with zero delay. Conversely, if the registrar key is lost without an on-chain governance rotation path, the namespace permanently deadlocks.
- **Minimal plain-English fix:** Split the registrar into two on-chain keys: a hot **issuance key** (permitted only to attest unclaimed handles) and a cold/multisig **admin key** (rotated via L1 governance and alone permitted to rotate the issuance key or call `handle_reassign`). Require a 7-day pending timelock on `lost-key` reassignments during which the current account owner can veto with a signed cancel transaction.

##### H2 — Global handle uniqueness allows permanent custom-domain squatting, and missing normalization enables spoofing
- **Severity:** `critical`
- **Where:** Locked-in design decisions; Detailed specification §2
- **Hole:** Consensus enforces global uniqueness on `handle` ("1 active identity per handle") while allowing users to bind custom domains (`you@yourdomain.com`) that are only verified off chain via `nostr.json` or DNS TXT. Consensus also omits strict character-set and case-normalization rules.
- **Why it breaks execution:** Consensus cannot query HTTP or DNS. If custom-domain handles sit in the globally unique on-chain handle index without on-chain verification, any griefer can register `founder@company.com` first for a tiny fee, permanently blocking the real domain owner and poisoning `identity_by_handle`. Separately, without canonical lowercase ASCII normalization, `GoodAlexander` or Unicode homoglyphs pass consensus uniqueness and impersonate real users in Hive and Explorer.
- **Minimal plain-English fix:** Enforce strict lowercase ASCII normalization (`[a-z0-9._-]`, 3–32 chars) in consensus and limit the globally unique `handle` index strictly to registrar-attested namespaces (`@tasknode.postfiat.org`). Store custom domains in a separate non-unique `external_handle` field marked *client-verified* (displayed and indexed only after clients verify the live domain's `nostr.json` or DNS TXT back to that `npub`).

##### H3 — `rotate_account` lacks destination co-signature and link re-authorization
- **Severity:** `critical`
- **Where:** Detailed specification §2 (`Operations`)
- **Hole:** `rotate_account` moves an identity, its handle, and all `links[]` to a "new unused account", but the spec does not require the destination account to co-sign, does not state whether linked keys must re-consent, and does not define `sequence` carry-over.
- **Why it breaks execution:** Without a destination ML-DSA co-signature, an attacker can dump a spam or compromised identity onto a victim's freshly generated unused address before they mint, permanently locking them out under "1 active identity per account". Worse, if an attacker steals an account key and calls `rotate_account` to an attacker-controlled address while automatically carrying over `links[]`, they retain the victim's co-signed validator and old-chain links on a clean account without those keys ever consenting to the new account.
- **Minimal plain-English fix:** Require `rotate_account` to carry a domain-separated ML-DSA co-signature from the destination account (proving consent) and carry forward `sequence + 1` to block replay. Drop third-party `links[]` on account rotation unless fresh co-signatures binding the new account are included in the transaction.

##### H4 — `handle_reserve` bound only to unsalted `hash(npub)` enables pre-mint handle theft and blocks safe v2 migration
- **Severity:** `major`
- **Where:** Detailed specification §1 & §3 (`Existing handle reservations`)
- **Hole:** Existing Task Node handles are pre-reserved on chain using unsalted `(handle, hash(npub))` with no expiry and no binding to the user's old-chain wallet or Task Node session.
- **Why it breaks execution:** Because the existing `signedCollaborationProof` bug lets a compromised collaboration backend derive a user's v1 Nostr private key without ever touching their old-chain private key, an attacker who already holds a leaked v1 `nsec` can front-run the victim and claim their reserved `@tasknode.postfiat.org` handle onto an attacker-controlled L1v2 account. In addition, unsalted `hash(npub)` is trivially linkable against public Nostr relays (defeating the stated privacy goal), unexpiring reservations lock dormant handles forever, and users who want to migrate straight to a clean v2 key cannot claim their reserved handle without exposing/using v1 first.
- **Minimal plain-English fix:** Do not let `hash(npub)` alone authorize handle claims. Either salt the on-chain reservation with a registrar-held secret or issue short-lived registrar claim attestations `(handle, target_l1_account, expiry)` only after the user proves control of their existing Task Node account/old-chain key—letting them mint directly with either their v1 or a fresh v2 `npub`, and expiring unclaimed reservations after 180 days.

##### H5 — Self-`unlink` by a linked key is front-runnable, and client-verified EVM/Solana links conflict with global uniqueness
- **Severity:** `major`
- **Where:** Detailed specification §2 (`Operations`, `Consensus checks & RPCs`)
- **Hole:** Consensus domain-separates signatures by `(op, chain_id, account, sequence)` while permitting "self-unlink by linked key", and stores EVM/Solana links as *client-verified* without specifying how uniqueness or self-unlink applies to them.
- **Why it breaks execution:** If a linked validator key or controller account signs an unlink proof referencing the identity owner's mutable `sequence`, a hostile identity owner can permanently censor the unlink by front-running with cheap `update` transactions that bump `sequence` (and the spec omits which L1 account pays the fee for a non-L1 key's unlink). Meanwhile, because consensus does not check EVM/Solana signatures, putting them in the unique `identity_by_link` index lets anyone squat third-party EVM/Solana addresses permanently, and those owners cannot self-unlink on chain.
- **Minimal plain-English fix:** Allow any fee-paying L1 account to submit a `self_unlink` carrying a linked-key signature domain-separated by `(op="self_unlink", chain_id, target_account, link_id)`—independent of the identity's mutable `sequence`—and explicitly exclude client-verified EVM/Solana links from consensus uniqueness indexes.

##### H6 — Validator anti-farming claim has a Sybil and node-hopping loophole across Pillar 3 and Pillar 6
- **Severity:** `major`
- **Where:** Locked-in design decisions (`Required vs optional gating`); Detailed specification §2–§3
- **Hole:** The spec states minting an on-chain identity prevents multi-node validator reward farming, yet `identity_set` is permissionless (one operator with two validators can mint two separate identities and link one node to each) and allows unrestricted `unlink` → `re-link` of validator `node_id`s.
- **Why it breaks execution:** Permissionless `identity_set` alone does not stop an operator running two nodes from creating two identities to claim 2× `$20/month` credits and duplicate Finance Hub/API keys. Worse, without an on-chain re-link cooldown or epoch record, a validator node can collect rewards under Identity A, unlink, re-link to Identity B, and bypass Pillar 3's 14-day grace/revocation window or double-claim in the same billing period.
- **Minimal plain-English fix:** Clarify the cross-pillar contract: Pillar 6 permits one identity to link multiple validator `node_id`s (still earning one Pillar 3 grant), enforces an on-chain re-link cooldown (e.g., 30 days) on any previously linked `node_id` so nodes cannot hop between identities to dodge revocation, and explicitly relies on Pillar 3's G0 / Admission Policy V1 operator ratification to bind each approved operator to a single identity.

##### H7 — Undefined fee denomination (`"20 units"`) and missing inline field caps create state-bloat and onboarding risk
- **Severity:** `major`
- **Where:** Locked-in design decisions (`Fees & limits`); Detailed specification §2
- **Hole:** State-expansion fees are written as `"20 units for mint, 10 per link, flat handle price"` with no unit definition (atoms vs. PFT), no hard byte limits on inline fields (`relays[]`, `handle`, external link payloads), and no specification of whether `revoke` allows re-minting or leaves unbounded permanent `compromised` `npub` tombstones.
- **Why it breaks execution:** If "units" means atoms (matching Pillar 2's ~25-atom FastPay fee), attackers can cheaply bloat consensus state and the permanent `compromised_npub` tombstone table forever. If "units" means whole PFT, Task Node's starter PFT stipend may fail to cover basic onboarding. Without per-field byte caps, oversized relay URLs or external payloads can bloat inline state objects.
- **Minimal plain-English fix:** Lock explicit governance-adjustable PFT/atom amounts calibrated to starter PFT (e.g., `0.5 PFT` mint, `0.25 PFT` per link, `1 PFT` handle, `0.5 PFT` compromised rotation), add strict per-field byte caps (e.g., 128 bytes per relay URL, 64 bytes per external link), and state whether a revoked account can re-mint (paying full `mint` fee) while tombstoning only `compromised` `npub`s.

##### H8 — Activating the BIP-340 governance kill-switch can deadlock existing identities
- **Severity:** `major`
- **Where:** Locked-in design decisions; Detailed specification §2 (`Honest quantum boundary`)
- **Hole:** Governance can disable new BIP-340 proofs if secp256k1 weakens, but the spec does not distinguish which `identity_set` operations require BIP-340 versus ML-DSA-only, nor whether `nostr_pubkey` becomes optional once disabled.
- **Why it breaks execution:** If `update`, `unlink`, `rotate_account`, or `revoke` require a BIP-340 co-signature alongside ML-DSA, flipping the emergency kill-switch freezes every live identity on chain—preventing users from revoking compromised identities or updating their `pq_receive_address`. If `nostr_pubkey` stays mandatory on `mint`, all new identity minting halts permanently when the kill-switch fires.
- **Minimal plain-English fix:** Require a BIP-340 proof *only* when setting or rotating `nostr_pubkey` (`mint` with `nostr_pubkey`, `rotate`), allow ML-DSA-only authorization for `update`, `link`, `unlink`, `revoke`, and `rotate_account`, and make `nostr_pubkey` optional in the schema so post-quantum-only identities (`ML-DSA` + `message_key` + `pfpq1…`) remain usable if BIP-340 is disabled.

##### H9 — First-party relay (`relay.postfiat.org`) has no write-admission gate or inbound mirroring filter
- **Severity:** `major`
- **Where:** Locked-in design decisions; Detailed specification §3 (`First-party relay`)
- **Hole:** `relay.postfiat.org` is specified as a lightweight relay mirrored to public relays, with no write-authentication policy, per-pubkey rate limits, or mirror direction/filtering rules.
- **Why it breaks execution:** Any open Nostr relay is rapidly flooded by automated spam bots, exhausting disk and bandwidth and degrading Hive Chat. Pulling events from public relays without an allow-list imports the global Nostr firehose and abusive content onto Post Fiat infrastructure.
- **Minimal plain-English fix:** Require NIP-42 AUTH / pubkey allow-listing for writes to `relay.postfiat.org` (minted on-chain `npub`s, active Task Node users, and Hive group pubkeys) with per-pubkey event-rate and size caps, and scope mirroring to outbound-only for public Hive/profile events plus allow-listed inbound sync solely for subscribed Hive pubkeys and channels.

##### H10 — `avatar_nft_id` lacks ownership verification at write and read time
- **Severity:** `minor`
- **Where:** Locked-in design decisions (`Validator link & avatar`); Detailed specification §2
- **Hole:** `identity_set` lets any account attach an `avatar_nft_id` referencing a Pillar 4 NFT without stating whether consensus checks NFT ownership or how transfers affect displayed avatars.
- **Why it breaks execution:** Without an ownership check, any account can attach the founder's or a high-status user's Profile NFT as their `avatar_nft_id` to impersonate them in Hive and Explorer; if checked only at `update` time, a sold or transferred NFT remains displayed on the former owner's identity indefinitely.
- **Minimal plain-English fix:** Enforce `owner(avatar_nft_id) == account` in consensus on `mint`/`update`, and require Explorer, Hive, and `resolveIdentity` to verify live NFT ownership on read (hiding the avatar if the NFT has since been transferred or burned).

## Claude Opus 5.5 — 70/100

*Model `anthropic/claude-opus-5.5` · 9866 output tokens · `tech=17/30 econ=11/20 align=18/20 ship=12/15 read=12/15`*

#### Summary judgement

This is a well-structured pillar. It follows the locked decisions: Task Node registrar with lost-key/abuse-only reassignment, custom NIP-05 domains, identity required only for validator rewards, and a first-party relay. It ships the signing-guard fix first, keeps v1 `npub`s, and has clean M1–M4 milestones plus all four ship gates.

The problems are in the trust and anti-abuse mechanics.
- **Anti-farming does not work.** The spec says the founder-mandated "identity required for per-operator rewards" rule prevents farming. It doesn't: one operator can mint any number of identities from fresh accounts.
- **The registrar is an unchecked single key.** It can silently reassign any handle, including `goodalexander`. Since there is no recovery key, it is also the only recovery path.
- **Some "verified" claims cannot be verified by consensus.** Custom-domain NIP-05 and EVM/Solana links are stored on chain, yet appear to sit in the uniqueness indexes. That makes them squattable.
- **Compromise handling is unsafe.**
  - An attacker holding the account key can instantly `rotate_account` away with the validator link and rewards attached.
  - Badge stripping depends on Nostr timestamps, which the signer sets and can backdate.
- **Cross-pillar gap with Pillar 3.** Deduplicating grants across old and new chains needs an old-chain *validator-key* link, not just an old-chain wallet link.

None of these is hard to fix, but each would be exploited or would block real users on day one.

#### Rubric breakdown

- **Tech (17/30):** The core design is sound: ML-DSA plus BIP-340 proofs, domain-separated signatures, and uniqueness indexes. However:
  - Registrar compromise, account-key compromise, and timestamp backdating are unhandled.
  - Unverifiable claims sit in uniqueness indexes.
  - Consensus-versus-FastPay ordering for the shared uniqueness index is unstated.
- **Econ (11/20):** The validator-reward anti-farming claim is false as specified. Starter-PFT-funded mints are an open faucet. Some fee numbers are given in unnamed "units" and the handle price is undefined.
- **Align (18/20):** Every locked decision is honored and there are no open-questions sections. Small deductions:
  - The "flat handle price" and the "hold-back" window are left without concrete defaults.
  - The link to Pillar 3's `IdentityReceiptV1` is undefined.
- **Ship (12/15):** Milestones are concrete and all four gates are present. Gate 2 does not test:
  - the signing guard,
  - a compromised rotation,
  - registrar reassignment,
  - a uniqueness collision.
- **Read (12/15):** The plain-English intro is good. The detailed spec gets dense with inline parameter strings, and some terms are never explained: "unanchored old-key events", "hold-back", "units".

#### Execution holes

**H1**
- **Severity:** critical
- **Where:** Locked-in design decisions → Required vs optional gating; §2 uniqueness rules
- **Hole:** Uniqueness is enforced per account, `npub`, handle, `node_id` and old-chain account. Nothing ties an identity to a single *operator*. Anyone can create N fresh accounts, derive N v2 `npub`s (new seeds or `key_index`), link one validator `node_id` to each, and pay about 20 units per mint.
- **Why it breaks execution:** One person running 5 validators mints 5 identities and collects 5 × ($20/mo + Finance Hub + API keys). This is exactly the farming the founder required identity to prevent. The spec presents identity as the control, so Pillar 3 may build no other dedupe.
- **Minimal plain-English fix:** State plainly that an identity proves "one identity per node", not "one person per identity". Reward eligibility should require one of two things:
  - a registrar-attested `@tasknode.postfiat.org` handle, where the registrar issues one reward-eligible handle per verified operator; or
  - the operator's Pillar 3 admission record (Admission Policy V1 / validator ratification), which names the identity account.

  Pay rewards per identity, never per node.

**H2**
- **Severity:** critical
- **Where:** Locked-in design decisions → Handle registrar; Revoke & rotate safety ("no separate recovery key")
- **Hole:** One rotatable registrar key can immediately `handle_reassign` any handle. Two things are undefined:
  - who authorizes rotating the registrar key itself, and
  - whether there is any delay or chance for the owner to contest.

  The registrar is also the only lost-key recovery path, so it is a single point of failure for both theft and recovery.
- **Why it breaks execution:**
  - A leaked registrar key reassigns `goodalexander` to an attacker. NIP-05, Hive badges and the Explorer card all follow the change.
  - Pillar 1 governance verifiers that resolve the founder by handle would accept attacker-signed packets.
  - The attacker can also rotate the registrar key to lock the Foundation out.
- **Minimal plain-English fix:**
  - Put a 7-day public delay on every `handle_reassign`. The current owner can cancel it by signing with their account key; the abuse reason code is the only exception.
  - Allow registrar key rotation only through a Foundation multisig or a validator governance transaction, never by the registrar key itself.
  - Require Pillar 1 and the SDK to pin the founder by **account**, not by handle.

**H3**
- **Severity:** major
- **Where:** §2 Operations (`rotate compromised`, `rotate_account`); v2 derivation (§1)
- **Hole:** v2 Nostr keys and the ML-DSA account come from the same master seed. If the seed or account key leaks, the attacker can:
  1. declare the current `npub` compromised (it is then permanently un-rebindable for the real owner),
  2. rotate to their own `npub`, and
  3. `rotate_account` the whole identity, including the validator link and handle, to a fresh account in a single block.

  Registrar reassignment only moves the *handle*, not the links.
- **Why it breaks execution:** The validator link, and the reward and Finance Hub eligibility tied to it, follow the attacker permanently. The real operator cannot reclaim their `node_id` because uniqueness blocks re-linking it.
- **Minimal plain-English fix:**
  - Make `rotate_account` and `compromised` rotation take effect after 72 hours, and show them as pending on the Explorer.
  - During that window, the linked validator node key or the registrar may cancel.
  - Extend registrar lost-key/abuse reassignment so it can move the *whole identity* (handle plus links) to the verified owner's new account, not just the handle.

**H4**
- **Severity:** major
- **Where:** Locked-in design decisions → custom domains; §2 consensus checks (EVM/Solana "client-verified")
- **Hole:** Consensus cannot fetch DNS or `nostr.json`, so a custom-domain handle on chain is just a claim. EVM and Solana links are also only "client-verified". If these claims enter the global uniqueness index, anyone can squat them first.
- **Why it breaks execution:** An attacker mints `alice@alice.com`, or claims a famous EVM treasury address. The real owner is then blocked by uniqueness, and the Explorer shows a plausible-looking impersonation.
- **Minimal plain-English fix:**
  - Leave custom domains and client-verified links **out** of uniqueness indexes.
  - Have readers (SDK, Explorer, Hive) re-check `nostr.json` or DNS live and show "verified now" or "unverified".
  - Better still, verify EVM signatures (secp256k1/keccak via the existing `k256`) and Solana signatures (ed25519) in consensus, so they become real verified links.

**H5**
- **Severity:** major
- **Where:** §2 old-chain links (strict low-S DER ECDSA); cross-pillar with Pillar 3 "one grant per operator across old/new chains"
- **Hole:** Old-chain links only cover secp256k1 *wallet* keys whose public key hashes to the address. Three cases are missing:
  - XRPL-style **ed25519** accounts,
  - accounts that use a **regular key** or have the master key disabled, which new-chain consensus cannot see,
  - old-chain **validator master keys**, which is what Pillar 3's G0 / old-chain Dynamic UNL / `IdentityReceiptV1` actually identifies.
- **Why it breaks execution:**
  - Some legitimate users cannot link their old-chain account.
  - Old-chain and new-chain reward grants cannot be deduplicated per operator, because the old-chain validator identity has no link type.
  - After Stage 4, classical old-chain keys keep minting links indefinitely, even though they are quantum-vulnerable.
- **Minimal plain-English fix:**
  - Accept ed25519 old-chain signatures.
  - Add an `old_chain_validator` link type checked against `IdentityReceiptV1`.
  - Check old-chain links only against the master-key-derived address.
  - Stop accepting new old-chain links once Stage 4 publishes the final ledger hash.

**H6**
- **Severity:** major
- **Where:** §3 Chain-backed NIP-05 & Hive badges ("strip immediately from unanchored old-key events")
- **Hole:** A Nostr event's `created_at` is set by whoever signs it. After a compromised rotation, the attacker can backdate events signed with the old key to before the rotation. "Unanchored" is never defined.
- **Why it breaks execution:** Backdated attacker messages keep *Minted* or *Validator* badges in Hive and appear to be the legitimate pre-compromise owner's, which is ideal for governance-chat impersonation.
- **Minimal plain-English fix:** Define an event as "anchored" if either:
  - `relay.postfiat.org` received it, by its own receive timestamp, before the rotation block's time; or
  - it carries the ML-DSA companion signature.

  Strip badges from every other old-key event.

**H7**
- **Severity:** major
- **Where:** §2 consensus checks / uniqueness (interaction with Pillar 2 FastPay)
- **Hole:** The spec never says whether `identity_set` must go through ordered consensus. The handle, `npub` and `node_id` indexes are global shared state, while FastPay handles owned objects with no global ordering.
- **Why it breaks execution:** If any client sends identity ops through the FastPay path, two certificates claiming the same handle or `npub` can both finalize. Validators then disagree about uniqueness and state diverges.
- **Minimal plain-English fix:** State that every `identity_set` operation is consensus-ordered only, rejected on the FastPay path, and resolved by in-block ordering when it collides with another claim.

**H8**
- **Severity:** minor
- **Where:** Fees & limits ("paid from Task Node starter PFT")
- **Hole:** If Task Node funds mints from starter PFT, anyone can spin up accounts to drain it. They can also spam identity state, up to 16 links and 5 relays each, without paying.
- **Why it breaks execution:** The starter-PFT treasury drains, and chain state fills with junk identities that the Explorer and indexes must carry.
- **Minimal plain-English fix:**
  - Sponsor at most one mint per existing Task Node user, only for accounts holding a Postgres handle reservation or meeting the leaderboard minimum.
  - Everyone else pays their own fee.
  - Give the fees in PFT atoms, e.g. 20 PFT per mint and 10 PFT per link, and fix the handle price at a concrete number.

**H9**
- **Severity:** minor
- **Where:** §3 Existing handle reservations (`handle_reserve` with `hash(npub)`)
- **Hole:** v1 `npub`s are already public on Primal, nos.lol and Damus, so `hash(npub)` hides nothing: anyone can hash every known `npub` and match it. Reservations also have no expiry, unlike the current 365-day Postgres expiry.
- **Why it breaks execution:** The stated privacy goal is not met. Handles reserved for users who never mint stay locked forever.
- **Minimal plain-English fix:**
  - Commit with `hash(npub ‖ random salt)` and deliver the salt to the user privately, or simply admit the reservation is public.
  - Expire unclaimed reservations after 365 days, matching today's rule.
  - Set the retired-handle hold-back to a concrete 90 days.

**H10**
- **Severity:** minor
- **Where:** M1 / §1 (shared derivation scope); cross-pillar with Pillar 5
- **Hole:** M1 shares only **v2** derivation across web, Mac app and CLI. Minting with a v1 `npub` requires the v1 secret, which is derived from an old-chain wallet signature. That flow exists only in the browser.
- **Why it breaks execution:** Existing users on the native `tasknode-apple` app cannot mint while keeping their v1 `npub`. Gate 2's "same `npub` & history" test passes on web but fails in the locked native app.
- **Minimal plain-English fix:** Add the v1 derivation, behind the signing guard, to the shared Rust module with its own test vectors, so the Mac app can mint v1 natively.

**H11**
- **Severity:** minor
- **Where:** §3 First-party relay
- **Hole:** No write policy or rate limits are defined for `relay.postfiat.org`.
- **Why it breaks execution:** An open, Foundation-hosted relay becomes a spam sink and a storage-cost DoS target. Hive's fallback then fails exactly when third-party relays throttle.
- **Minimal plain-English fix:**
  - Accept writes only from Hive members or minted `npub`s, with per-key rate and size limits.
  - Serve reads to everyone.

**H12**
- **Severity:** minor
- **Where:** Ship gate 2
- **Hole:** The user test never exercises the security-critical paths:
  - signing-guard rejection,
  - a uniqueness collision,
  - a compromised rotation with badge stripping,
  - registrar reassignment with the delay and cancel.
- **Why it breaks execution:** The gate can pass while the main impersonation defenses are untested.
- **Minimal plain-English fix:** Add to gate 2:
  - one rejected malicious challenge,
  - one duplicate-handle rejection,
  - one compromised rotation showing stripped badges, and
  - one registrar reassignment that the founder cancels during the delay window.
