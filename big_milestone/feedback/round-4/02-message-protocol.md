# Feedback — Pillar 2 — Message Protocol Spec

*Execution-hole audit · Round 4.*

> **Mean score:** 75/100 · Gemini 4 Neon 74 · Claude Opus 5.5 76
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/02-message-protocol.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 19580 output tokens · `tech=16/30 econ=13/20 align=18/20 ship=13/15 read=14/15`*

#### Summary judgement

Pillar 2 is a well-written, founder-aligned specification with a clean architectural direction: canonical protobufs, X-Wing post-quantum encryption, FastPay payouts with Sui-inspired fee discipline, a key-free Explorer Message panel, and x402 (+ MCP) agent payments. It respects every locked founder decision, includes zero open questions, and grounds its build plan in real repository paths with concrete `[ ]` milestones and all four mandatory ship gates.

However, if engineering builds this spec as written, it will hit **three critical execution blockers** and **four major operational/economic holes**: (1) keying x402 replay protection and `tx` lookups on a malleable quorum `certificate_digest` lets attackers replay FastPay payments for free; (2) zero-balance new Task Node users cannot publish an X-Wing public KEM key under Pillar 6's optional identity model or via a paid self-payment, deadlocking their first encrypted payout; (3) signing over re-serialized protobuf submessages (`header`, `readers`) instead of opaque wire bytes will break cross-language signature verification across JS, Python, and Rust. Fixing these alongside IPFS outbox durability, hot-wallet pool equivocation safety, concrete FastPay object-cap/merge fee parameters, and PFUSDC x402 finality handling will make the spec production-ready.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (16/30):** Strong high-level architecture grounded in the live codebase, but held back by 3 critical and 4 major concrete protocol, concurrency, and key-bootstrap bugs.
- **Economic & Incentive Coherence (13/20):** Clean Sui-inspired fee direction and pinning responsibility split, but missing concrete `per_new_object_fee`/dust-floor/cap defaults, a global-cap griefing vector, cheap 2,048-input merge spam, and facilitator DoS amplification.
- **Founder Decision Alignment & Zero Open Questions (18/20):** Faithfully locks every Nov 26 founder decision with zero open questions; minor deduction for leaving concrete fee/cap engineering defaults and `sealed` wire semantics unspecified, and missing the interaction with Pillar 6's optional identity rule.
- **Actionable Milestones & 4 Ship Gates (13/15):** Clear M1–M6 build sequence with concrete latency targets, repo links, and all four ship gates; just needs test gates added for quorum-subset replay, zero-balance KEM onboarding, and hot-wallet crash recovery.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Concise, plain-English prose with helpful brief definitions of unavoidable crypto terms, clear diagrams, and zero `R1..R31` jargon bloat.

#### Execution holes

##### H1 — Malleable `certificate_digest` breaks x402 replay protection and canonical `tx` indexing
- **Severity:** `critical`
- **Where:** Sequence diagram (`PAYMENT-RESPONSE (certificate digest)`), Section 3 (`account_tx` / `tx`), Section 4 (x402 `pf-fastpay-settled` & spent-digest store)
- **Hole:** The spec identifies FastPay payments using `certificate_digest` (which covers the validator quorum signatures) and relies on a spent-digest store to prevent x402 replay, without specifying a server-signed challenge expiry or canonical order identifier.
- **Why it breaks execution:** In a quorum-certificate protocol (`N = 3f + 1`), multiple distinct subsets of `2f + 1` validators can sign the exact same `OwnedTransferOrderV3`, producing different `certificate_digest` values for a single payment. An attacker using `pf-fastpay-settled` can pay once, swap one validator signature to generate a new `certificate_digest`, and bypass the spent-digest store to get unlimited free API responses. Similarly, wallets, facilitators, and blocks can compute different hashes for the same transfer, breaking Explorer links and `tx` lookups; and without a signed challenge TTL shorter than spent-store retention, pruned/restarted stores allow replay of old payments.
- **Minimal plain-English fix:** Key all `tx`/`account_tx` lookups, Explorer URLs, x402 receipts, and replay stores on the canonical **`order_digest`** (hash of the client-signed `OwnedTransferOrderV3`, excluding validator signatures). Require servers to issue an HMAC-signed `402` challenge (`payee`, `amount`, `asset`, `request_hash`, `nonce`, `expires_at`) embedded in the memo, verify active-epoch validator quorum on the certificate, and retain spent `order_digest`s strictly longer than `expires_at`.

##### H2 — Zero-balance new users cannot publish an X-Wing public KEM key before their first encrypted payout
- **Severity:** `critical`
- **Where:** Section 2 (Deterministic restore / public KEM key discovery) & Section 3 (Task Node default payout cutover)
- **Hole:** Pillar 2 assumes users publish their X-Wing public KEM key via Pillar 6's on-chain identity record or a "signed self-payment fallback." However, Pillar 6 is locked as **optional** for non-validators (and ships after Pillar 2), and Pillar 2 enforces **non-zero fees** on both FastPay and `payment_v2`. Furthermore, the HKDF input seed is not explicitly pinned to Pillar 5/6's unified BIP-39 KDF.
- **Why it breaks execution:** A brand-new Task Node user has `0 PFT` on `postfiatl1v2` and no Pillar 6 identity. They cannot pay for an `identity_set` transaction or a self-payment before receiving their first reward, so Task Node has no verified public KEM key to encrypt their first `pf.reward.v1` payout—deadlocking onboarding at M4. Separately, if `wallet_wasm`, `tasknode-apple`, and Python SDKs feed different BIP-39 encodings into HKDF, restoring a seed on another device breaks decryption.
- **Minimal plain-English fix:** Define a small ML-DSA-65-signed `KemKeyAnnouncement` (`account_address`, `chain_id`, `kem_pubkey`, `key_version`, `created_at`) that wallets submit off-chain to Task Node during zero-balance onboarding (verified against the user's account address) and optionally anchor on-chain once funded or via Pillar 6. Pin the exact derivation across Pillars 2, 5, and 6: 32-byte HKDF-SHA256 output from the unified BIP-39 root seed (`info = "postfiat-l1-v2/message-key/v1" || account_index || key_version`) fed into standard `X-Wing GenerateKeyPairDerand`.

##### H3 — Non-canonical protobuf re-serialization breaks cross-language `Envelope` signatures and CIDs
- **Severity:** `critical`
- **Where:** Section 1 (Canonical schemas, `Envelope`, Signature & integrity)
- **Hole:** The spec states protobuf ensures JS, Python, and Rust produce identical bytes and defines the ML-DSA-65 signature over `SHA3-384(header) || SHA3-384(readers) || SHA3-384(body)`. Standard protobuf serialization is **not canonical** across runtimes (field order, unknown fields, packed repeated encodings, default values), and a `repeated Reader` field has no single contiguous byte slice in protobuf wire format.
- **Why it breaks execution:** A client in Rust or Python that signs an `Envelope` can fail signature verification in JS or the Explorer if the verifier decodes and re-encodes `header` or `readers` before hashing. Worse, wire-level malleability lets an intermediary alter the serialized `Envelope` (changing its IPFS CID) without invalidating the signature.
- **Minimal plain-English fix:** Define `Envelope` with opaque length-delimited `bytes` fields—`header_bytes`, `readers_bytes` (wrapping a `ReaderList` message), and `body_bytes`—and compute `SHA3-384` directly over those exact received wire bytes without ever re-serializing before verification. Compute the IPFS CIDv1 raw hash over the exact serialized `Envelope` bytes.

##### H4 — Async IPFS pinning lacks a durable outbox and a browser/NAT upload path, risking permanent dead CIDs
- **Severity:** `major`
- **Where:** Flow diagram, Section 1 (`Envelope`), Section 3 (Task Node payouts), M3
- **Hole:** The spec broadcasts FastPay transactions (~200ms finality) in parallel with async IPFS pinning, without requiring durable local persistence prior to broadcast or specifying how browser (`wallet_wasm`) and home-NAT Mac clients push blocks into IPFS. Browsers and NAT'd laptops cannot serve IPFS blocks to the public DHT.
- **Why it breaks execution:** If Task Node crashes, a browser tab closes, or a pin request fails after the 200ms transaction finalizes, the immutable on-chain `pf.ptr` v5 permanently points to an unretrievable CID (*Content unavailable*). For governance packets (Pillar 1) and NFT metadata (Pillar 4), async-before-pin risks permanently breaking authoritative records. Conversely, exposing an unauthenticated Task Node pin endpoint allows 16 MiB chunked-manifest storage DoS.
- **Minimal plain-English fix:** Persist signed `Envelope` bytes in a durable local/DB pin outbox *before* broadcasting FastPay payouts/messages, with background retry until pin confirmation; require synchronous pin confirmation *before* on-chain anchoring for Pillar 1 governance packets and Pillar 4 NFT metadata; and specify an authenticated HTTP pin-upload endpoint on Task Node (rate-limited and restricted to Task Node/governance/NFT schemas) plus x402-paid pinning for third-party traffic.

##### H5 — Hot-wallet 20-object FastPay pool has no crash-safe idempotency, risking object equivocation or double payouts
- **Severity:** `major`
- **Where:** Section 3 (Task Node default payout cutover), M4
- **Hole:** Leasing ~20 owned objects via DB row locks does not specify recovery when a payout worker crashes or times out *after* sending a signed FastPay order to validators but *before* receiving/recording the quorum certificate, nor does it specify balance replenishment for depleted slots.
- **Why it breaks execution:** If the DB lock expires on crash/timeout and another worker signs a new payout spending the same object version, the hot wallet equivocates—locking that owned object in FastPay and steadily shrinking the pool until payouts stall. If the worker instead retries the payout using a different pool object while the first order actually reached quorum, the user is double-paid. Over time, large payouts also starve depleted slots without automatic re-splitting.
- **Minimal plain-English fix:** Persist the exact signed `OwnedTransferOrderV3` and its `order_digest` tied 1:1 to a unique `payout_id` in the DB *before* broadcasting. On timeout or restart, reconcile by re-submitting that exact stored order (never minting a conflicting order for the same object version) until finality or epoch reconciliation confirms its fate, and run a background treasury rebalancer to top up/re-split low-balance pool objects.

##### H6 — Unspecified FastPay object fees, global cap griefing, and cheap 2,048-input merges create DoS vectors
- **Severity:** `major`
- **Where:** Locked-in design decisions, Section 3 (FastPay fee floor & Wallet UX), M2
- **Hole:** Three concrete economic/resource parameters are missing or conflicting: (1) `per_new_object_fee`, minimum object balance (dust floor), raised global cap, and fee sink routing inside `fastpay_pre_state_effects` are left unspecified; (2) a chain-wide owned-object cap with no v1 eviction lets an attacker split dust to exhaust the cap and halt FastPay network-wide; (3) allowing up to 2,048 inputs in an auto-merge while stating "Merging payouts pays only the tiny base fee" lets attackers force 2,048 state reads/locks per validator vote for ~25 atoms.
- **Why it breaks execution:** A low-budget griefer can either freeze all FastPay payouts and x402 traffic by hitting the global owned-object cap, or CPU/IO-DoS validators with giant 2,048-input merge transactions. Conversely, arbitrarily cranking up `per_new_object_fee` without a dust floor or tiered curve penalizes legitimate Task Node payouts (which create 1 net-new object per payout).
- **Minimal plain-English fix:** Lock concrete v1 consensus defaults in the spec: `base_fee = 25 atoms`, `byte_fee = 25 atoms / 512 B`, a `per_input_fee` (e.g., `10 atoms / input`) with a hard cap of `64`–`128` inputs per merge tx (wallets chain multiple merges automatically), a minimum output dust floor (e.g., `1,000 atoms`), a concrete `per_new_object_fee` (e.g., `5,000`–`10,000 atoms`) with steep dynamic escalation above 70% of a raised `10,000,000` global cap, and explicit fee deduction/burn routing matching `payment_v2`.

##### H7 — x402 PFUSDC consensus fallback lacks finality/retry/sequence rules, and facilitator lacks DoS defenses
- **Severity:** `major`
- **Where:** Section 4 (x402 v2 binding + MCP wrapper), M6
- **Hole:** Consensus-lane PFUSDC (`issued_payment_v2`) is placed behind the same synchronous HTTP `402 → PAYMENT-SIGNATURE → 200` flow as ~200ms FastPay without specifying finalized-block vs mempool verification, HTTP-timeout retry idempotency, or account sequence-number management for concurrent agent calls. Meanwhile, the public Post Fiat facilitator forwards client-supplied payment payloads to validators with no stated pre-validation or rate-limit boundary.
- **Why it breaks execution:** Returning `200 OK` on mempool-only PFUSDC allows trivial double-spends via sequence replacement; waiting for block finality without an idempotent `challenge_id` retry lookup causes agents that retry after an HTTP timeout to double-pay; concurrent MCP tool calls from one agent collide on account sequence numbers and fail; and an unthrottled facilitator becomes a free DoS amplifier against validator FastPay voting endpoints.
- **Minimal plain-English fix:** Require finalized-block confirmation for PFUSDC x402 (supporting idempotent retry/polling by `challenge_id` so retries return the cached response or wait without re-charging), add a local sequence queue to the MCP payment helper, and require the facilitator to verify client signatures, local object/balance state, and per-IP/account rate limits before contacting validators.

##### H8 — `sealed` wire format is undefined, `"Task reward"` badge is spoofable, and v4 downgrade rules are unstated
- **Severity:** `minor`
- **Where:** Section 1 (`pf.ptr` v5 flags), Section 2 (Dual-read backwards compatibility), Section 4 (Explorer Message panel)
- **Hole:** (1) `pf.ptr` v5 requires a `sealed` flag bit alongside `public` and `private`, yet never defines what `sealed` encrypts or hides on the wire; (2) the Explorer displays a `"Task reward"` card for `pf.reward.v1` envelopes whose signer matches the transaction signer without checking whether the sender is an official Task Node payout address; (3) dual-read v4/v5 support does not forbid legacy unsigned X25519 v4 pointers on `postfiatl1v2`.
- **Why it breaks execution:** M1 engineers cannot write interoperable `sealed` test vectors without inventing a format; any spammer can send 1-atom FastPay transfers with `pf.reward.v1` headers to spoof official `"Task reward"` cards on the Explorer; and accepting v4 pointers on the new chain enables unsigned/classical-crypto downgrade spoofing.
- **Minimal plain-English fix:** Either specify `sealed` concretely (encrypt serialized `Header + body` inside an anonymous padded outer wrapper) or reserve the flag bit as unsupported in v1; gate the official `"Task reward"` Explorer badge on configured Task Node payout addresses (rendering third-party `pf.reward.v1` senders with a neutral/unverified label); and restrict v4 decoding strictly to old-chain historical records.

## Claude Opus 5.5 — 76/100

*Model `anthropic/claude-opus-5.5` · 6968 output tokens · `tech=19/30 econ=12/20 align=19/20 ship=14/15 read=12/15`*

#### Summary judgement

This spec is well aligned with the locked decisions. It covers the Sui-inspired FastPay fee plus a new-object fee, raises the 100k cap, and defers storage rebates to v2. It assigns pinning to Task Node, makes payouts Private with a "Task reward" badge, keeps the `PFSH03` circuit untouched, and ships x402 over FastPay PFT with a PFUSDC fallback, a Post Fiat facilitator and an MCP helper. The cryptographic core is sound: protobuf envelopes, X-Wing hybrid encryption, ML-DSA signatures, and header/reader binding. Milestones and the four ship gates are clean.

The real risk is the payout and fee machinery around FastPay. The spec does not say how the payout path handles the defining hazard of owned-object systems: a crash-retry that signs two different orders for the same coin. It also allows a fallback path that can pay a user twice. The "Task reward" badge can be spoofed, and Task Node cannot encrypt to users who have not yet published a message key. The spam defense has no actual numbers. That matters most because the cap is chain-wide, so filling it would knock out FastPay for every user.

#### Rubric breakdown

- **Tech (19/30):** The envelope, encryption and signature design is solid. Several holes sit in the payout and liveness path: equivocation, double-pay on fallback, badge spoofing, and a missing message key for recipients. These would surface on day one of the M4 cutover.
- **Econ (12/20):** The fee shape is correct, but the per-object fee and the new cap are unquantified. Fee destination is undefined. A single chain-wide cap with no per-account limit leaves the cheap-attack math open, along with dust griefing of victims.
- **Align (19/20):** Every locked Pillar 2 decision is reflected faithfully, and there are no open questions. The only gap is a minor cross-pillar issue: the dependency on Pillar 6 to publish message keys is not reflected in the Pillar 6 deliverables.
- **Ship (14/15):** M1–M6 are concrete and testable, and all four gates are present and specific.
- **Read (12/15):** Mostly plain English with good inline glosses. Sections 1 and 4 drift into dense, acronym-stacked bullets (SHA3-384 concatenations, scheme names) that need a one-line "why" each.

#### Execution holes

**H1 — FastPay crash-retry equivocation locks payout objects**
- **Severity:** critical
- **Where:** §3, Task Node default payout cutover (hot-wallet pool of ~20 objects leased via DB row locks)
- **Hole:** In FastPay, if two *different* signed orders spend the same object at the same version, validators lock that object, typically until the epoch ends. The spec leases objects through row locks but never requires the signed order to be saved before it is broadcast.
  - Suppose Task Node crashes, a lease times out, or a second replica takes over mid-payout.
  - The retry then builds a fresh order (new memo, new amount, new recipient) on the same object version.
- **Why it breaks execution:** Each crash or timeout can permanently lock one of the ~20 pool objects for the epoch. A few incidents under load will drain the pool, push the fallback rate far above 1%, and miss the p50/p95 targets. A bug or replica split-brain could freeze all payouts.
- **Minimal plain-English fix:**
  - Write the exact signed order to the database *before* sending it.
  - On any retry, resend that same stored order byte-for-byte. Never re-sign a different order for an object version that already has one.
  - Only one Task Node process may hold signing rights to a given pool object.
  - Add a "crash mid-payout, restart, retry" test to M4.

**H2 — `payment_v2` fallback can double-pay**
- **Severity:** major
- **Where:** §3, "falling back to `payment_v2` only if FastPay quorum is unavailable"
- **Hole:** "Quorum unavailable" as seen by Task Node does not prove the FastPay order failed. Votes can still be gathered later, and anyone holding the signed order can finish certifying it. Falling back while the order is still pending can settle both payments.
- **Why it breaks execution:** Users get paid twice during exactly the network conditions (partial validator outages) when fallback fires. Since the 1% fallback rate is a design target, losses are systematic, not rare.
- **Minimal plain-English fix:**
  - Give every reward a unique payout ID.
  - Before falling back, first spend the same FastPay object version on the consensus lane, or wait until it is provably locked or expired. This guarantees the pending order can never execute.
  - Put the payout ID in both memos so the indexer can flag duplicates.

**H3 — "Task reward" badge is spoofable**
- **Severity:** major
- **Where:** §4, Explorer Message panel; locked decision on payout visibility
- **Hole:** Reward bodies are encrypted, so the explorer cannot verify the content. The spec does not say what entitles a transaction to the badge. Anyone can send a `pf.ptr` v5 private pointer with the reward content kind for ~25 atoms.
- **Why it breaks execution:**
  - Scammers can fake "Task reward" activity to impersonate Task Node payouts or pad metrics.
  - This undercuts the stated goal that the badge shows "real network activity."
- **Minimal plain-English fix:**
  - Show the badge only when the transaction signer is on a published, signed list of Task Node payout accounts. That list lives in a well-known on-chain record and is rotatable.
  - Show anything else as a generic "Private message."

**H4 — No encryption target for recipients without a published message key**
- **Severity:** major
- **Where:** §2, deterministic message keys; M4 payout cutover
- **Hole:** A recipient's X-Wing public key cannot be derived from their ML-DSA address. It must be published, either on the Pillar 6 identity record or through a signed self-payment fallback. The cutover assumes every payee has done this. Existing users, and users who have not opened an upgraded wallet, will not have.
- **Why it breaks execution:**
  - M4 has no defined behavior for keyless recipients. It must either block their payouts, or silently send rewards the recipient can never decrypt (encrypted to Task Node only).
  - The Pillar 6 README deliverables do not list a message key field, so the dependency can slip unnoticed.
- **Minimal plain-English fix:**
  - The web and Mac wallets automatically publish the message key on first login after upgrade.
  - Until a key exists, the payout still goes out over FastPay. The envelope is encrypted to Task Node only, and the explorer shows a "Recipient key pending" status. The body is re-encrypted to the user once their key appears.
  - Add a message public key field explicitly to the Pillar 6 `identity_set` schema.
  - Define the precedence rule: the latest of identity record or self-payment wins, and the identity record wins on ties.

**H5 — Spam economics are unquantified, and a chain-wide cap is a global kill switch**
- **Severity:** major
- **Where:** Locked-in design decisions and §3, fee floor and owned-object cap
- **Hole:** The per-new-object fee has no value, and the new cap has no number. Fee destination (burn vs. validators) is undefined. The cap is chain-wide only, with no per-account limit. There is also no rule against creating objects inside *other people's* accounts.
- **Why it breaks execution:**
  - An attacker pays cap × per-object fee once to fill the chain-wide cap. FastPay then refuses new objects for every user, which halts Task Node payouts and x402.
  - The attacker can also send dust objects to victims, inflating their object counts and forcing them to pay for merges.
  - Without numbers, M2 cannot be built or load-tested.
- **Minimal plain-English fix:**
  - Set concrete defaults. Example: per-new-object fee of 1,000 atoms, burned. Chain-wide cap of 10M objects. Per-account cap of 10k owned objects.
  - Transfers that would exceed the *recipient's* cap are rejected, and the sender pays the fee anyway.
  - Merging and unwrapping stay always allowed and cheap, even when at a cap.
  - Size the fee so filling the global cap costs well above a stated budget, for example ≥ $100k.

**H6 — Async pin failure permanently orphans private rewards**
- **Severity:** minor
- **Where:** Flow diagram ("async pin"); §4 "Content pending" grace window
- **Hole:** The payment is final before the envelope is pinned. If the Task Node pin fails or the pinning node loses the block, the recipient holds an on-chain pointer to undecryptable, unavailable content. The grace window length is unspecified.
- **Why it breaks execution:** Users see "Content unavailable" on real rewards. For x402 receipts and governance packets, the audit trail silently breaks.
- **Minimal plain-English fix:**
  - Task Node keeps the envelope in its own database until at least two independent pins confirm the CID, retrying with backoff.
  - Set a fixed grace window (for example 15 minutes) and alert when it is exceeded.
  - Add a nightly job that re-pins and checks every CID referenced on-chain by Task Node accounts.

**H7 — x402 signed orders have no expiry, letting a server hold the client's coin**
- **Severity:** minor
- **Where:** §4 and x402 sequence diagram (`exact` scheme)
- **Hole:** In the `exact` scheme, the server holds the client's signed FastPay order and settles it at its leisure. Until then, the client's object is effectively reserved: signing anything else on it risks the H1-style lock. The 402 challenge has a nonce but no stated expiry.
- **Why it breaks execution:**
  - A slow or malicious server can strand agent funds across many requests.
  - The "replay-proof spent-digest store" also has no stated retention rule.
- **Minimal plain-English fix:**
  - Put an expiry in the 402 challenge and bind the request hash, payee, amount and expiry into the memo.
  - Servers must settle within 30 seconds or reject the payment.
  - Clients pay from a dedicated small "x402 spending" object and never reuse an object while an order on it is outstanding.
  - Keep spent digests at least as long as the longest allowed expiry.

**H8 — Envelope signatures are not bound to a chain, so they replay across networks**
- **Severity:** minor
- **Where:** §1, signature context `postfiat-l1-v2/message-envelope/v1`
- **Hole:** Key wrapping is bound to chain ID, but the ML-DSA signature context is not. A signed public envelope from devnet (`postfiat-wan-devnet-2`) can be re-pointed on mainnet and still show as validly signed.
- **Why it breaks execution:** Signed public artifacts such as governance packets, NFT metadata and identity attestations created during testing can be replayed on mainnet. Pillars 1 and 4 rely on these signatures.
- **Minimal plain-English fix:** Include the chain ID in the signed header, and have the explorer and SDKs reject envelopes whose chain ID does not match the chain they are read from.
