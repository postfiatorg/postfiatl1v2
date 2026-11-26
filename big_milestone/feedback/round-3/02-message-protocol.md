# Feedback — Pillar 2 — Message Protocol Spec

*Review round 3.*

> **Mean score:** 77/100 · Gemini 4 Neon 76 · Claude Opus 5.5 78
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/02-message-protocol.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 76/100

*Model `google/gemini-4-neon` · 27440 output tokens*

#### Summary judgement

This is a well-researched, concrete engineering specification that maps closely to the founder's intentions and shows genuine familiarity with the actual Post Fiat repositories (`tasknode`, `postfiatl1v2`, `explorer`, and `postfiatorg.github.io`). It gets the core architecture right: decoupling message durability from FastPay latency via offline CIDv1 computation, fixing the serious cryptographic weaknesses of `pf.ptr` v4 (missing associated data, missing sender signature, plaintext hash leak, non-PQ encryption), specifying safe explorer rendering without ever exposing keys to the browser, and surfacing the real L1 constraints (256-byte decoded memo limit, 100,000 owned-object cap, 64 FastPay effects/block, zero FastPay fee floor, missing FastPay `account_tx` rows).

However, the spec is not yet ready to hand straight to implementation without revision. It contains **four critical technical/operational flaws** that will break builds or block launch if implemented as written: (1) a direct mathematical contradiction between `Pointer.inline` (≤240 bytes) and R14 requiring inline payloads to use the R4–R6 signed `Envelope` (≥5.3 KB); (2) chunked `Manifest` envelopes using IPFS `raw` blocks that will silently lose their chunks because `raw` blocks have no IPLD links and encrypted manifests cannot be traversed by `pinByHash`; (3) an x402 `exact` scheme whose >13–15 KB HTTP headers fail behind standard cloud gateways/CDNs, omits ML-DSA public key handling in `payTo`, and allows free-compute replay attacks on paid POST endpoints; and (4) a new-user onboarding deadlock where first-time Task Node users lack PFT to publish their `message_key` on chain and may be unable to unwrap FastPay PFT if unwrapping requires consensus-lane fees. Economically, the spec diagnoses the right risks (spam, owned-object cap, pinning costs, PFT-only FastPay vs PFUSDC) but stops short of coherent mechanisms—in particular, a pure burn fee neither stops dust-griefing of a fixed 100k owned-object cap nor incentivizes recipients to merge payouts.

Fixing these specific defects will turn an already strong foundation into an executable, production-grade specification.

#### Correspondence to the founder's intention

Correspondence to the founder's prompt is **high**. The spec directly addresses every requirement tied to Pillar 2 across the prompt:

- **Deliverable 2 (Message Protocol Spec):** Standardizes protobuf-encoded, encrypted, IPFS-pinned messages with exact `.proto` packages, multi-language code generation (JS, Python, Rust), worked byte examples for FastPay and consensus carriers, block explorer rendering, and an x402 profile.
- **Chain readiness gates 2 & 3:** Explicitly owns both *"works with existing memo type private payments"* (dual-read v4/v5, legacy X25519 decryption via imported old seed, `pf.reward.v1` migration) and *"default task node payouts should be using fast pay for maximum speed and latency"* (FastPay default payouts with `payment_v2` fallback, pooled owned objects, wallet consolidation and unwrapping, testnet latency AC).
- **Cross-pillar alignment:** Accommodates Deliverable 1's governance packets (including `GOVERNANCE = 12`, strict pinning, and the blog post requirement), Deliverable 4's NFT metadata contiguity (`metadata_uri`/`metadata_hash` and `CANONICAL_JSON` twin), Deliverable 5's local wallet signing and shielded-payment memo question, and Deliverable 6's identity/key directory.

Two minor intention gaps stand out:
1. **Protobuf vs `CANONICAL_JSON`:** The founder specifically asked for *"IPFS pinning protobuf representations"*. Allowing `CANONICAL_JSON` for ~24 legacy Task Node schemas and Pillar 1/4 payloads is a pragmatic migration choice, and the spec appropriately flags it in Open Question 13, but it should state a clear default migration target so `CANONICAL_JSON` does not quietly become the permanent default.
2. **"x402 or other integration points":** x402 is fleshed out in detail, while other machine-native integration points (e.g., MCP payment hooks, A2A/agent frameworks, Nostr/Hive direct-delivery envelopes) are deferred to Open Question 9 with minimal concrete design.

#### Technical robustness

The core cryptographic envelope (R4–R8) and explorer security model (R18–R20) are well designed: signing raw serialized `header` and `readers` bytes eliminates protobuf re-serialization malleability; binding `H ‖ RH` into AEAD associated data and ML-DSA signatures prevents reader-list tampering; verifying `sender_pubkey` against the carrier transaction distinguishes genuine senders from *Forwarded* memos; and escaping/capping/sandboxed rendering protects the explorer.

Nevertheless, several severe technical flaws undermine implementability and reliability:

- **Impossible inline wire format (P1):** R14 places `Pointer.inline` under R4–R6, but an R4 `Envelope` is ≥5.3 KB due to ML-DSA-65 keys/signatures while `Pointer.inline` is ≤240 bytes.
- **Broken large-message pinning (P2):** Chunked `Manifest` payloads (1–16 MiB) use CIDv1 `raw` blocks, which carry no IPLD DAG links, and encrypted manifests hide chunk CIDs from pinners—meaning `pinByHash(manifest_cid)` leaves every chunk unpinned and subject to garbage collection.
- **Fragile x402 design (P3):** Putting a >10 KB raw (>13–15 KB base64) ML-DSA FastPay order in an HTTP request header breaks behind standard proxies/CDNs/AWS API Gateway; serving retries "without charge" without an idempotent response cache creates a free-compute replay exploit; and `payTo` ignores that FastPay requires a 1,952-byte ML-DSA public key rather than a hashed address.
- **First-time user bootstrapping deadlock (P4):** New users have 0 consensus-lane PFT to run `identity_set` (which publishes their `message_key` needed for their first encrypted payout), and R26 does not confirm whether `owned_unwrap_apply_v3` works with a 0 consensus-lane balance.
- **Unresolved operational & protocol contradictions (P6, P7, P8, P10, P11):** Sealed envelopes have no outer sender or schema yet R13 expects sender/schema pinner policies; `wrap_owned` is disabled yet "FastLane deposits" and dual-lane fallback reserves are left undefined; R5/R16 and R6 contradict each other on unknown protobuf fields; chunk nonce/AD construction lacks domain separation and framing rules; `account_tx` anchoring lag vs 183 ms FastPay finality creates explorer visibility gaps; and Task Node's systemic service KEM key has no rotation/KMS requirement.

#### Economic coherence

The Economics section is the thinnest part of the spec. It deserves credit for honestly flagging four real economic risks—zero-fee FastPay spam, the 100,000 owned-object cap, centralized Task Node pinning costs, and PFT-only FastPay vs PFUSDC for x402—but the proposed mitigations are economically incomplete or misaligned:

1. **Owned-object cap and UTXO consolidation incentives are inverted (P5):** R28 suggests protecting the 100,000 chain-wide owned-object cap with a fee (burn) per new object, while R27 prices FastPay orders by byte size (`ceil(order_bytes / 512)`). A pure burn fee does not protect a fixed 100k cap against cheap dust-griefing to unspendable public keys, and byte-proportional fees with no storage rebate make merging many small payout objects *expensive* for users—actively discouraging users from freeing global state slots. Refundable object storage deposits (like Sui storage rebates or XRPL reserves) plus a much higher cap are needed to align incentives.
2. **Pinning economics underestimate worst-case costs by 3+ orders of magnitude (P9):** The back-of-the-envelope storage figure uses 9 KB messages ("1M = 9 GB"), ignoring 1 MiB envelopes and 16 MiB manifests (1,800× larger), Pinata per-pin counts (17 pins per 16 MiB manifest), gateway egress bandwidth, and takedown/abuse costs.
3. **x402 currency mismatch (P9):** Real-world APIs price requests in USD/stablecoins, not volatile L1 native tokens. Running x402 on FastPay in volatile PFT introduces quote-expiry/FX slippage risk (especially under 64-effect/block backpressure), while running PFUSDC on the consensus lane sacrifices FastPay's latency advantage.
4. **Dual-lane Task Node float economics (P7, P9):** Running concurrent FastPay payouts without object locking requires tying up a split PFT float in a hot wallet, refilling it via an unspecified "FastLane deposit" path, *and* holding a separate consensus-lane PFT reserve for quorum-failure fallback, while paying R27 burn fees on every micro-payout and daily airdrop.

#### Problems

##### P1 — `Pointer.inline` (≤240 bytes) directly contradicts R14 ("both under R4–R6", ≥5.3 KB)
- **Severity:** critical
- **Where:** Technical requirements — R2 (Pointer v5), R4 (Wire form), R8 (Reader/Envelope overhead), R14 (Storage profiles), R21 (x402 `request_hash` inline).
- **What is wrong:** R14 states that both Pinned and Inline storage profiles operate *"under R4–R6"*, which defines a signed `Envelope` containing an ML-DSA-65 `sender_pubkey` and `signature`. R8 explicitly calculates that this signature and sender key alone add ~5.3 KB. However, R2 limits `Pointer.inline` to ~125 bytes (with three IDs) or ~240 bytes (with none), bounded by the L1's 256-byte decoded memo data limit. An R4 `Envelope` cannot physically fit in `Pointer.inline`. Meanwhile, the actual wire encoding of `Pointer.inline` (raw protobuf message body, unsigned mini-header, or raw 32-byte x402 hash) is never specified.
- **Why it matters:** JS, Python, Rust, and explorer implementations cannot interoperate on inline messages, x402 request-hash memos, or short receipts without inventing incompatible formats.
- **How an agent should investigate:** Check `transactions_validation_helpers.rs` for exact memo byte limits, check Pillar 6 R10 to verify whether `identity_set` uses a memo or a dedicated transaction field (so R14 does not conflate dedicated tx fields with `Pointer.inline`), and inspect what fields the explorer generic decoder (`FileDescriptorSet`) needs to decode an inline payload without an `Envelope` wrapper.
- **Suggested direction for fix:** Define an explicit **Inline** wire rule separate from R4 `Envelope`: `Pointer.inline` holds raw deterministic protobuf (or canonical JSON) **body bytes only**, relying on the carrier transaction's ML-DSA signature for sender authenticity and `Pointer.schema` (registry ID > 1) plus a flag or schema rule for type/encoding dispatch. Specify a dedicated tiny protobuf message (e.g., `pf.x402.binding.v1 { bytes request_hash = 1; }`) for x402 inline memos, and clarify that `identity_set` uses its own transaction payload rather than `Pointer.inline` if that is what Pillar 6 specifies.

##### P2 — Chunked `Manifest` envelopes (1–16 MiB) break IPFS/Pinata transitive pinning
- **Severity:** critical
- **Where:** Technical requirements — R3 (Envelope & Manifest), R13 (Fast path and availability); Economics.
- **What is wrong:** R3 specifies CIDv1 with the `raw` codec (`0x55`) for both the 1 MiB `Manifest` envelope and its ≤1 MiB chunks, and R13 pins the envelope via `block/put` and replicates via Pinata `pinByHash`. Because `raw` blocks contain no IPLD links—and encrypted manifests hide chunk CIDs in ciphertext—pinning a `Manifest` CID on an IPFS node or calling Pinata `pinByHash(manifest_cid)` pins **only the manifest block**, never any of its chunks.
- **Why it matters:** Every Task Node payload or attachment over 1 MiB (Task Node currently supports up to 8 MiB files and 16 MiB JSON) will silently lose its chunk blocks to IPFS garbage collection or fail to replicate to Pinata, causing permanent data loss.
- **How an agent should investigate:** Audit `server/context-ipfs.js` in `tasknode` to see how files >1 MiB are currently pinned and replicated (UnixFS DAGs work today because DAG-PB carries IPLD links), check Pinata API limits on pin counts vs CAR imports (`pinFileToIPFS` / DAG pinning), and measure the actual payload size distribution of Task Node messages.
- **Suggested direction for fix:** Either (a) wrap multi-chunk payloads in an **IPFS DAG/CAR or UnixFS/DAG-CBOR container** (for public chunks, or a shallow DAG-CBOR link node pointing to encrypted raw chunk CIDs) so standard `pinByHash` transitively pins all chunks—while preserving offline deterministic CID calculation—or (b) explicitly require pinners/uploaders to upload a single CAR containing the manifest and all chunks and track all N+1 chunk CIDs under one logical pin quota/lifecycle. Option (a) using a deterministic DAG-CBOR wrapper for chunked payloads is far cleaner for third-party pinners like Pinata.

##### P3 — x402 `exact` fails behind standard HTTP gateways (>13–15 KB headers), allows free-compute replay, and omits ML-DSA `payTo` key resolution
- **Severity:** critical
- **Where:** Technical requirements — R21 (x402 profile), Open Question 8.
- **What is wrong:** R21 has three serious defects:
  1. **Header size cliff:** A signed `SignedOwnedTransferOrderV3` is >10 KB before base64 (~13–15+ KB in `PAYMENT-SIGNATURE`). Telling servers to "raise their header limit" fails behind managed infrastructure with hard, non-configurable limits (e.g., AWS API Gateway's 10,240-byte header cap, Cloudflare/CDN/proxy limits, default Nginx/Envoy 8 KB buffers).
  2. **Free-compute replay bug:** R21 states *"A retry with the same order or certificate and the same hash is served again without charge"* while also claiming *"the server keeps no per-request state."* Without storing and returning a **cached response** keyed by `(certificate_digest, request_hash)`, a client can replay the same paid `POST` (e.g., an LLM inference call or agent action) dozens of times before nonce expiry and force the server to re-execute expensive compute for free.
  3. **Missing ML-DSA `payTo` public key:** FastPay outputs are owned by a 1,952-byte ML-DSA public key (not a hashed address), yet R21 never specifies how `payTo` carries or resolves to an ML-DSA public key so clients can construct the order.
  4. *(Secondary)* In `exact`, an unauthenticated client can trigger validator quorum fan-out (`owned_sign_v3`) on invalid or already-spent orders unless the server/facilitator pre-validates signature, amount, payee, and live object state locally first.
- **Why it matters:** Post Fiat's x402 integration will fail to work on standard cloud deployments, leak unpaid compute on AI/agent APIs, and fail interoperability with x402 clients.
- **How an agent should investigate:** Measure the exact serialized byte size of a 1-input, 2-output `SignedOwnedTransferOrderV3` in both JSON-hex and compact binary/protobuf encodings in `postfiatl1v2`; check x402 v2 spec conventions for `payTo`, `extra`, and alternative header/transport schemes; and check `owned_sign_v3` / `owned_certificate` RPC behavior on already-applied certificates.
- **Suggested direction for fix:**
  1. Make `pf-fastpay-settled` (~100-byte header carrying certificate digest + epoch) a **first-class primary scheme** (not an optional afterthought) so Post Fiat x402 works through every proxy/CDN out of the box, and check whether compact binary encoding brings `exact` under 10 KB or if `exact` should pass the order via a facilitator upload / body/chunked header extension.
  2. Explicitly require an **idempotent response cache** (keyed by `digest + request_hash`, TTL ≥ nonce expiry) for paid non-GET endpoints—or document that replay returns the cached result rather than re-executing—and mandate local pre-validation before calling `owned_sign_v3`.
  3. Specify that `PAYMENT-REQUIRED` includes the payee's ML-DSA public key (e.g., `extra.payToPubkey`) verified to hash to `payTo`.

##### P4 — Onboarding deadlock for first-time Task Node users (`message_key` discovery and FastPay-only unwrapping)
- **Severity:** critical
- **Where:** Technical requirements — R9 (Keys), R22 (Task Node payout), R26 (Recipient wallet).
- **What is wrong:** Two interlocking bootstrapping deadlocks affect brand-new users with 0 PFT:
  1. **`message_key` chicken-and-egg:** R9 reads a recipient's PQ `message_key` from the Pillar 6 on-chain identity (`identity_set`). Submitting `identity_set` requires consensus-lane PFT for fees. R22 provides an off-chain wallet-registration fallback for the user's ML-DSA public key, but **R9 provides no equivalent registration path for `message_key`**. Consequently, Task Node cannot encrypt the user's very first `pf.reward.v1` payout before the user already has PFT.
  2. **Zero consensus-lane PFT unwrap risk:** Once paid via FastPay, a new user holds PFT *only* in FastPay owned objects and 0 PFT on the consensus lane. R26 relies on `owned_unwrap_sign_v3` / `owned_unwrap_apply_v3` so the user can pay for `identity_set`, NFTs, and PFUSDC, but the spec never verifies whether `owned_unwrap_apply_v3` requires a consensus-lane fee (which would deadlock the user) or deducts its fee from the unwrapped FastPay object, nor whether the wallet triggers unwrapping automatically.
- **Why it matters:** If new users cannot receive their first encrypted reward or cannot move their FastPay reward to the consensus lane to mint an identity/NFT, the default FastPay payout readiness gate fails in production.
- **How an agent should investigate:** Inspect `owned_unwrap_sign_v3` and `owned_unwrap_apply_v3` in `postfiatl1v2` (`crates/execution/src/owned_transfer.rs` and `docs/rpc/methods.md`) to confirm how unwrap fees are paid when the target account has 0 consensus-lane balance; check Task Node wallet registration endpoints to see what keys are submitted today.
- **Suggested direction for fix:** Allow Task Node to record a **self-signed `{ml_dsa_pubkey, message_key}` registration proof** at wallet signup (verified by signature under `ml_dsa_pubkey` so the server cannot spoof keys) for encrypting payouts before `identity_set` exists on chain. Verify that `owned_unwrap` pays any fee from the FastPay object itself (or add that as an L1 requirement in Step 2), and specify automatic just-in-time unwrap in the wallet UX when a user initiates a consensus-lane transaction with insufficient consensus balance.

##### P5 — Owned-object cap (100k global) cannot be secured by a burn fee alone, and byte-based fees penalize payout consolidation
- **Severity:** major
- **Where:** Technical requirements — R26, R27, R28; Economics; Open Question 3.
- **What is wrong:** FastPay has a hard chain-wide cap of 100,000 owned objects. R28 proposes protecting it via *"a charge per new owned object, a higher cap, or both"*, while R27 burns `base + per_quantum × ceil(order bytes / 512)`. This creates two economic failures:
  1. **Permanent dust-griefing:** A modest burn fee does not stop an attacker from sending 1-atom outputs to 100,000 random (unspendable) public keys, permanently exhausting the global cap and bricking FastPay for the entire network.
  2. **Consolidation disincentive:** Because R27 charges per 512 bytes of order size and burns the fee without rebate, merging 50–2,048 small Task Node payouts (R26) costs the recipient a *large* fee, whereas leaving them unmerged costs the recipient nothing while consuming scarce global object slots.
- **Why it matters:** Either a cheap griefing attack or normal organic accumulation of unmerged daily rewards/airdrops will hit the 100,000-object wall and halt FastPay payouts and x402.
- **How an agent should investigate:** Inspect `crates/execution/src/owned_transfer.rs` to find where the 100,000-object cap is enforced, why that constant was chosen (memory/state-proof/checkpoint constraints), and whether owned objects can carry a refundable storage deposit field or whether fees can be negative/rebated on net object destruction.
- **Suggested direction for fix:** Raise the owned-object cap substantially (after benchmarking memory/checkpoint cost) **and** replace or supplement the pure burn fee with a **refundable per-object storage deposit** (locked when creating an owned object, refunded when the object is consumed/merged). That makes creating unspendable dust objects capital-expensive and makes consolidating/merging payouts fee-neutral or net-positive for users.

##### P6 — Sealed envelopes conflict with R13 pinner acceptance policies and overstate privacy in AC 3
- **Severity:** major
- **Where:** Technical requirements — R11 (Privacy levels), R13 (Fast path and availability); Acceptance criteria — AC 3; Open Question 6.
- **What is wrong:** R13 specifies pluggable pinner acceptance policies based on *"senders, schemas, quota"*. However, R11's Sealed outer envelopes intentionally contain **no sender, no outer signature, and no inner schema** (only `pf.msg.sealed.v1`). During fast-path parallel pinning (before tx confirmation), a pinner cannot determine who sent a Sealed envelope or what schema it contains—forcing it either to reject Sealed envelopes, require authenticated HTTP uploads (which links uploader identity to the Sealed CID at the pinner, undermining sealing), or wait for carrier confirmation (breaking parallel fast-path). Additionally, AC 3 claims *"A non-reader learns only CID and flags from a sealed message,"* omitting that the carrier payment still publicly exposes payer, payee, amount, timestamp, and envelope byte length—so a Sealed Task Node reward payout is still trivially identifiable as a Task Node payout.
- **Why it matters:** Sealed messages will either fail to pin under anti-spam policies or give users a false sense of anonymity while leaking sender/recipient/timing at the payment and pinner layers.
- **How an agent should investigate:** Trace the pinning flow for both fast-path and strict-mode Sealed envelopes, evaluate what authentication/payment mechanism the first-party pinner (`context-ipfs.js`) can use without logging identity-to-CID mappings, and clarify how Sealed interacts with Open Question 6 (payout privacy) and Pillar 5 shielded payments.
- **Suggested direction for fix:** Explicitly define how Sealed envelopes pass pinner admission (e.g., anonymous x402 payment / blind quota token / size-capped anonymous rate limit, or verifying the carrier tx post-submission with a short unpinned staging buffer), and tighten AC 3 and R11 wording so users understand that Sealed hides *envelope metadata* (`kind`, `schema`, `task_id`, `in_reply_to`, and multi-reader list) while *payment-graph metadata* remains public unless carried off-chain or inside a shielded note.

##### P7 — FastPay float replenishment ("FastLane deposits") and consensus-lane fallback reserves are underspecified
- **Severity:** major
- **Where:** Technical requirements — R22 (Task Node payout); Economics; Open Question 14.
- **What is wrong:** R22 notes that `wrap_owned` is disabled and states that the Task Node payout float is *"funded by FastLane deposits"*, without explaining what FastLane deposits are, whether they are permissionless or validator/governance-gated, what their settlement latency and minimum/maximum amounts are, or how PFT cycles back into FastPay after recipients unwrap it to the consensus lane (R26). In addition, R22 falls back to `payment_v2` when FastPay lacks quorum—which requires maintaining a *separate* funded consensus-lane float, because FastPay objects cannot be unwrapped when FastPay has no quorum.
- **Why it matters:** An automated payout pipeline can easily run dry if float replenishment is manual, slow, or permissioned, or fail to fall back during a FastPay quorum outage if no separate consensus-lane reserve is maintained.
- **How an agent should investigate:** Search `postfiatl1v2` for `wrap_owned` (why it was disabled and whether re-enabling is planned) and for FastLane deposit entrypoints/docs (`docs/rpc/methods.md`, `crates/execution/`). Check whether Task Node can programmatically trigger FastLane deposits.
- **Suggested direction for fix:** Document the exact on-chain/RPC refill path for Task Node's FastPay float (or add re-enabling `wrap_owned` as an explicit Step 2 L1 dependency if FastLane deposits are not suitable for automated treasury refills), specify dual-lane float sizing (FastPay hot pool + consensus-lane fallback reserve), and add a refill test to AC 6 or AC 11.

##### P8 — Contradictory unknown-field rules (R5 vs R6 vs R16) and underspecified chunk encryption framing (R3)
- **Severity:** major
- **Where:** Technical requirements — R3 (Manifest chunks), R5 (Encoding), R6 (Registry and evolution), R16 (Shared vectors).
- **What is wrong:**
  1. **Unknown fields contradiction:** R5 specifies *"no unknown fields"* and R16 includes *"unknown fields"* in a list of error/rejection test vectors, while R6 states *"readers ignore unknown fields"* so schemas can evolve without breaking older clients. If decoders reject unknown fields, any additive schema change becomes a breaking change.
  2. **Chunk crypto hygiene & framing:** R3 encrypts chunks with the same `content_key` used for the Manifest body, using `nonce = first 16 bytes of body_nonce ‖ chunk_index (u64)` and AD = *"body's associated data (R7) plus index and count"*. It does not specify `chunk_index` endianness, 0-based vs 1-based indexing, or the exact byte framing of `index` and `count` in the AD, nor does it domain-separate the chunk key/nonce from the Manifest body encryption.
- **Why it matters:** Contradictory unknown-field semantics will break either forward compatibility or CI vector agreement across JS, Python, and Rust. Unspecified endianness/framing causes silent cross-language decryption failures, and reusing `content_key` across manifest body and chunks without clean domain separation is avoidable cryptographic fragility.
- **How an agent should investigate:** Review `prost`, `protobufjs`, and Python `protobuf` behavior on unknown fields and deterministic serialization, and check PRIV-013/014 (`docs/status/privacy-production-burndown.md`) for existing HKDF/AEAD conventions in `postfiatl1v2`.
- **Suggested direction for fix:** Clarify the split: **encoders** emit no unknown fields, **outer `Pointer` / `Envelope` / `Header`** either reject or ignore unknown fields explicitly (with R6 matching R5/R16), and **decoded message bodies** ignore unknown fields per R6. For chunks, derive a dedicated `chunk_key = HKDF-SHA256(content_key, salt=H, info="pf.msg.v1/chunk")` (or a 1-byte domain prefix in nonce/AD), fix `chunk_index` as 0-based big-endian `u64`, and define exact byte concatenation for chunk AD (`H ‖ RH ‖ "chunk" ‖ u64_be(index) ‖ u64_be(count)`).

##### P9 — Economics omits worst-case IPFS costs (16 MiB manifests, egress, pin counts), x402 volatile-PFT friction, and payout unit economics
- **Severity:** major
- **Where:** Economics; R13; R21; R22.
- **What is wrong:**
  - Storage math only models 9 KB messages ("1M = 9 GB"), ignoring that envelopes can be 1 MiB and manifests 16 MiB (1,800× larger), that raw-chunk manifests multiply pin counts by up to 17×, and that gateway egress bandwidth and abuse/takedown operations typically dominate IPFS costs.
  - x402's dependence on volatile PFT (for FastPay speed) vs slow PFUSDC (on the consensus lane) is noted in a single sentence without assessing quote-expiry risk, oracle pricing requirements, or whether PFT-only x402 is commercially viable for API providers.
  - Unit economics of R27 burn fees against small Task Node micro-rewards and daily airdrops (R23) are not bounded.
- **Why it matters:** Without realistic cost bounds and a clear path to fast stablecoin payments (or explicit PFT quoting rules), Post Fiat risks runaway pinning bills or an x402 standard that APIs decline to adopt.
- **How an agent should investigate:** Pull current Pinata/first-party IPFS storage, pin-count, and egress costs from `tasknode` ops; check average/p99 payload sizes on the old chain; and check `postfiatl1v2` roadmap/issues for FastPay issued-asset (PFUSDC) support.
- **Suggested direction for fix:** Expand the Economics section with a simple sensitivity table (median vs p99 vs adversarial 16 MiB message pinning/egress costs, payout burn fee as % of median reward, and hot-wallet float exposure), state explicit per-account/per-day pinning quotas as the default policy until paid pinning exists, and document either a short-TTL PFT quoting rule for x402 v1 or the dependency on FastPay PFUSDC support.

##### P10 — Lag between FastPay quorum finality (183 ms) and `account_tx` / explorer visibility, plus multi-memo ambiguity
- **Severity:** minor
- **Where:** What exists today (L1 memo, FastPay in history); R2; R17; R18; AC 2.
- **What is wrong:** FastPay reaches quorum in ~183 ms, but R17 derives `account_tx` and `tx` rows from certified blocks (`fastpay_pre_state_effects`, capped at 64/block). During bursts (such as R23's daily airdrop), a finalized FastPay payment may wait multiple blocks before appearing in `account_tx` and the explorer—yet AC 2 assumes the explorer panel transitions immediately from *pending* to *resolved* right after quorum. Separately, the L1 allows up to 4 memos and FastPay allows up to 8 outputs, but neither R2 nor R18 specifies how multiple `pf.ptr` memos on one transaction are rendered or bound to specific outputs.
- **Why it matters:** Users and agents clicking an explorer link immediately after a 183 ms FastPay transfer can hit "transaction not found" during traffic spikes, and multi-memo transactions may render inconsistently across clients.
- **How an agent should investigate:** Check `fastpay-effect-anchoring-fix-20260928.md` and `postfiatl1v2` block times to measure worst-case anchoring lag at 64 effects/block, and check whether `owned_certificate` RPC can be queried by the explorer for unanchored-but-certified transfers.
- **Suggested direction for fix:** Allow the explorer's transaction view to fall back to `owned_certificate` (showing an *"Anchored pending — FastPay quorum verified"* badge) before `account_tx` indexing catches up. Explicitly specify the multi-memo rule in R2/R18 (e.g., at most one `pf.ptr` memo per carrier transaction in v1, or render all valid `pf.ptr` memos in order as transaction-level messages).

##### P11 — Systemic Task Node service KEM key risk, master-seed rotation caveat, and ambiguous governance signing custody
- **Severity:** minor
- **Where:** R9 (Keys), R11 (Reader policy), R24 (Dual read), Worked example (governance packet).
- **What is wrong:**
  1. Task Node service is a default reader on all task lifecycle and reward messages (R11) and can re-encrypt lost payloads (R24), making its KEM private key a master decryption key for all users' historical task data on public IPFS—yet no KMS/HSM isolation or periodic service-key rotation policy is specified.
  2. R9 states that rotating a user's `message_key` bumps `key_index`, but because all `key_index` keys derive deterministically from the same `master_seed`, rotation provides no protection if `master_seed` leaks; this limitation should be stated alongside the forward-secrecy caveat.
  3. The governance packet worked example writes *"Task Node builds a public `pf.governance.packet.v1` envelope (CANONICAL_JSON, signed by his key)... It sends a `payment_v2` self-payment"*, ambiguously blurring client-side wallet signing (`wallet_wasm`) with server-side Task Node execution.
- **Why it matters:** Compromising a single long-lived Task Node service KEM key exposes every user's historical encrypted task/reward archive permanently, and imprecise custody wording risks leading implementers to store founder/user signing keys on the server.
- **How an agent should investigate:** Check how Task Node currently stores its service X25519 key (`server/task-payloads.js`) and how Pillar 1 specifies signing governance packets from Hive Chat.
- **Suggested direction for fix:** Require KMS/HSM protection and scheduled rotation (e.g., quarterly epoch keys) for the Task Node service KEM key; explicitly note in R9 that recovery from a leaked `master_seed` requires migrating to a new seed/account; and reword the worked example to clarify that *goodalexander's local wallet* signs both the envelope and the `payment_v2` self-payment.

##### P12 — 14 open questions lack recommended defaults, blocking Step 1, and L1 transaction byte inflation warrants verification
- **Severity:** minor
- **Where:** Build order, Open Questions 1–14, What exists today (L1 fees).
- **What is wrong:** Step 1 is explicitly blocked on Open Question 1 (X-Wing vs pure ML-KEM-768), and none of the 14 open questions presents a concrete engineering recommendation for the founder to approve or override—mixing true founder product/economic decisions (OQ 3, 4, 6, 8, 11, 14) with low-level engineering choices (OQ 1, 7, 13). Additionally, the spec reports ~23 atoms (~11.7 KB) for a `payment_v2` and >10 KB for a FastPay order even though a raw ML-DSA-65 pubkey + signature is ~5.3 KB, suggesting JSON-hex serialization overhead that directly worsens both fees and x402 header sizes.
- **Why it matters:** Presenting 14 unprioritized questions without defaults stalls Step 1 while waiting on founder sign-off for engineering details, and unexamined JSON-hex bloat compounds P3's x402 header-size problem.
- **How an agent should investigate:** Inspect `crates/execution/src/fees_offer_planning.rs` and `crates/types/src/fastpay_recovery_types.rs` to verify how transaction byte length is calculated (JSON-hex vs binary) and confirm whether PRIV-013/014 already commits to pure ML-KEM-768 or X-Wing.
- **Suggested direction for fix:** Add a concise **Recommended default** to every open question (e.g., match PRIV-013's KEM choice for OQ 1 so Step 1 can proceed immediately; keep `body_encoding` supporting both Protobuf and `CANONICAL_JSON` with a Stage 3 conversion plan for OQ 13; default to repo-managed core IDs for OQ 7), split true founder-gate questions from engineering defaults, and note the exact serialization basis behind the 23-atom / >10 KB figures.

## Claude Opus 5.5 — 78/100

*Model `anthropic/claude-opus-5.5` · 14169 output tokens*

#### Summary judgement

This is a strong sub-page. It follows Deliverable 2 closely and is grounded in the real code. It inventories what exists with file-level references, notices real constraints, and turns them into numbered requirements. The constraints it catches include:

- the 256-byte memo data limit;
- lower-case hex on the new chain;
- the absence of FastPay rows in `account_tx`;
- the zero-fee FastPay hole;
- the 100,000 owned-object cap;
- the 64-effects-per-block anchoring limit;
- the lack of ML-KEM.

The cryptographic envelope is well thought through. The sender key sits inside the signed header, and that header is bound into both key-wrapping and the body's associated data (AD). The design covers all of the founder's explicit asks: protobuf, IPFS, encryption, explorer rendering, FastPay examples, x402, the blog post, and the two readiness gates (memo-type private payments and FastPay as the default payout).

The score is held back by four kinds of issue:

- **Internal contradictions.** Inline payloads are said to be "under R4–R6" yet cannot hold a signed envelope. R5 says "no unknown fields" while R6 says "readers ignore unknown fields".
- **Under-specified operational paths with real fund-safety risk.** Payout idempotency interacts badly with randomised signing and FastPay equivocation locks.
- **A chicken-and-egg problem in key discovery for new payees.**
- **Chain-level blockers it identifies but does not escalate.** The owned-object cap can be exhausted at zero fee today. Anchoring throughput is chain-wide.

The page is also very dense. A founder skimming it will find it hard to see which decisions actually block shipping.

#### Correspondence to the founder's intention

**Strong matches:**
- "Standardized message protocol spec … IPFS pinning protobuf representations which can then be encrypted properly." This is covered by R1–R9: versioned `.proto` files, an envelope, CIDv1, a post-quantum (PQ) suite, and shared test vectors.
- "Easily consumed on the block explorer." Covered by R18–R20: a Messages lens, verified sender, decryption that never happens on the explorer page, and hardening.
- "Default for machine-native communication." Covered by the registry (R6), reference clients in three languages, the CLI (R16), and R23 moving every `pf.ptr` flow to v5.
- "Clear examples of how it integrates with FastPay … appropriately represented on the Block Explorer." Covered by R12(a) byte examples and R17 (FastPay rows in history). R17 correctly spots that FastPay is currently invisible in account history, which the founder may not have known.
- "Conformant with X402 or other integration points." Covered by R21 (x402 v2 headers, an `exact` scheme plus an extension). Open question 9 raises MCP and other integrations.
- Readiness gates. "Works with existing memo type private payments" and "fast pay default for task payouts" are covered by R22–R24 and R26–R28, with acceptance criteria 6 and 11.
- Deliverable 1's "I can read about the message protocol schema on the blog" is covered by R25.
- Deliverable 4's "natively contiguous with the message protocol" is covered by the NFT carrier note in R12 and by Pillar 4 registering packages.

**Partial or drift:**
- **Protobuf.** The founder said "protobuf representations". The spec allows `CANONICAL_JSON` for governance packets, NFT metadata twins and about 24 legacy schemas. This is flagged honestly (open question 13). Still, the flagship worked example (the governance packet) is JSON, which slightly undercuts the founder's framing.
- **Privacy versus FastPay.** The founder links "private payments" to FastPay. The spec shows that FastPay payments are fully public (parties, amount, time) and that only the memo body is private. This is correct and stated, but the founder should be told plainly that "memo-type private" means private content, not private payments.
- **Narrative.** There is little "post-human governance" framing. That is fine for a technical pillar; the governance example ties it in.

Overall correspondence is high, at roughly 85–90 out of 100.

#### Technical robustness

**Strengths:**
- Binary CIDv1 raw at 36 bytes is correct (version, codec 0x55, multihash 0x12 0x20 followed by 32 bytes).
- Pointer budgets add up: about 160–165 bytes with three ids, and 40 bytes sealed.
- ML-DSA-65 sizes add up: 3,309-byte signature plus 1,952-byte public key is about 5.3 KB.
- The signature covers header, reader hash and body separately, so verifiers never re-serialize.
- The key-wrap KDF uses the header hash as salt with suite, chain and `key_id` in the info string. Body AD = H ‖ RH.
- Sealed messages bind the outer reader set through `outer_readers_hash`.
- The x402 nonce is stateless (HMAC plus expiry), and a spent store is shared across replicas.
- Committee-epoch-aware certificate verification.
- Hardening is specified for the explorer.
- The test-vector list is strong.

**Weaknesses:**
- Several requirements contradict each other (inline versus envelope, unknown fields, where the request hash lives).
- Fund-safety concerns in the payout path are not addressed: idempotency, equivocation locks, and durable storage of envelopes before submission.
- How the payout float gets funded is unclear ("FastLane deposits" while `wrap_owned` is disabled).
- New payees need a message key before they can read their first reward, which creates a bootstrap problem.
- Chunk nonces reuse a body-nonce prefix under the same key. This is safe with overwhelming probability but is a design smell.
- Step 1 is gated on ML-KEM code that does not exist, so the whole pillar waits on PQ crypto.

#### Economic coherence

**Coherent:**
- The spam analysis is honest: about 23 atoms per `payment_v2` and zero for FastPay.
- The storage estimate is realistic: about 9 KB per short message to three readers, so about 9 GB per million messages.
- The costs of a pinning policy are framed as founder choices.
- The trade-off between float size and hot-wallet risk is named.
- The fee rule (R27) mirrors `payment_v2` and keeps burns.

**Gaps:**
1. **Object-cap exhaustion.** The 100,000 owned-object cap combined with zero-fee 8-output splits is a live, chain-wide denial-of-service vector. It is not just a payout-scaling concern, and it is placed only as a pre-condition for this pillar.
2. **Anchoring is a shared chain-wide budget.** At most 64 FastPay effects per block must cover payouts, x402 and any pure-message self-transfers (open question 2). Nothing sizes demand against it.
3. **PQ envelope overhead.** About 5.3 KB, plus about 1.1 KB per reader, makes each message expensive relative to its payload. Nobody is assigned to pay for it beyond "Task Node pays today".
4. **x402 is PFT-only on the fast lane.** API sellers usually want stable-value pricing, so the fast x402 path's economic appeal is weak until FastPay supports issued assets. This is noted, but not weighed against effort.
5. **A non-zero R27 fee needs a payer.** The fee is "a burn on every payout", but no one decides whether Task Node absorbs it or deducts it from rewards.

#### Problems

**P1 — Inline storage profile cannot satisfy R4–R6 (critical)**
- **Where:** R2 (field 10 `inline`), R14 ("Storage profiles, both under R4–R6"), R21 (request hash "inline in its memo").
- **What's wrong:**
  - A signed envelope needs at least about 5.3 KB (ML-DSA signature plus public key). The `inline` field has at most about 240 bytes.
  - So inline payloads cannot be R4 envelopes. They are unsigned bytes whose only authentication is the carrier transaction.
  - R2 says schema id `1` means "read the name from the envelope", but an inline pointer has no envelope. Unregistered inline payloads are therefore undecodable.
- **Why it matters:** x402 receipts, identity records and short receipts all depend on inline. Implementers in JS, Python and Rust will diverge.
- **How to investigate:** Enumerate every inline use (x402 `request_hash`, `pf.identity.*` in `identity_set`, short receipts). Check what authentication and schema information each one needs.
- **Fix direction:**
  - Define an explicit inline encoding: registered schema id required, no envelope, authenticity taken from the carrier signature, and R19's *From* derived from the carrier.
  - Forbid schema `1` with inline.
  - Rewrite R14 accordingly.

**P2 — Payout retries can equivocate FastPay objects and lock funds, or double-pay (critical)**
- **Where:** R5 (randomised signing: each CID is new), R13 (submit first, then pin), R22 (object pool, row-lock leasing).
- **What's wrong:**
  - Signing is randomised, so a worker that crashes after submitting but before recording will rebuild a different envelope, CID, memo and order on retry.
  - If the retry reuses the same leased object, validators see two conflicting orders on one input. FastPay typically locks the object, leaving it stuck until recovery.
  - If the retry uses a different object, the user is paid twice.
- **Why it matters:** This is fund safety and accounting integrity for the default payout path. It is the core of readiness gate 3.
- **How to investigate:**
  - Read `owned_transfer.rs` and the recovery spec. Establish what a validator does on a conflicting order for a locked input, and whether the vote digest includes memos.
  - Check whether `task-review-publication.js` currently persists the signed transaction before submitting.
- **Fix direction:**
  - Require Task Node to persist the exact envelope bytes and the signed order (keyed by payout id) before first submission, and to resubmit identical bytes on retry.
  - Make the payout id, not the CID, the idempotency key.
  - Add an acceptance criterion: kill the worker mid-payout, then confirm one payment and no locked object.

**P3 — No durable persistence of envelopes before submission on the fast path (major)**
- **Where:** R13 ("submits the payment at once and pins in parallel").
- **What's wrong:**
  - The payment is final immediately, but content survives only if pinning succeeds.
  - Pinata `pinByHash` has to discover the first-party node, which is asynchronous and can fail.
  - No retry queue or local retention is required, so a crash leaves a permanent pointer to a dead CID.
- **Why it matters:** Reward records and private context become unreadable forever, and the "Content unavailable" state becomes normal.
- **How to investigate:** Check the pinning code in `context-ipfs.js` and the research note on async IPFS for queueing semantics.
- **Fix direction:**
  - Require the envelope to be written to durable storage before submission.
  - Run a pin job with retries until both pinners confirm.
  - Report pin-failure rate in acceptance criterion 11.

**P4 — How the payout float is funded is unclear (major)**
- **Where:** R22 ("funded by FastLane deposits because `wrap_owned` is disabled").
- **What's wrong:** "FastLane deposits" is never defined. If wrapping is disabled, it is unclear how Task Node moves treasury PFT into owned objects at all. Signed unwrap exists, but its reverse apparently does not.
- **Why it matters:** Without a funding path, FastPay-by-default payouts cannot start.
- **How to investigate:** Check `docs/rpc/methods.md` and the L1 code for `wrap_owned` status, any deposit or genesis allocation path into owned objects, and plans to re-enable wrapping.
- **Fix direction:**
  - Name the exact mechanism.
  - If wrapping needs enabling, add it to the R26–R28 L1 work in Step 2.
  - Add the funding path to the Dependencies table.

**P5 — New payees cannot read their first reward (major)**
- **Where:** R9 (reader key from the Pillar 6 `message_key` set by `identity_set`), R22, R26.
- **What's wrong:**
  - A brand-new user paid only over FastPay has no consensus-lane PFT to pay for `identity_set`.
  - Without `identity_set` they have no published message key, so the encrypted `pf.reward.v1` cannot list them as a reader.
  - R26's unwrap only helps after they have been paid.
- **Why it matters:** Onboarding UX suffers, and gate 2 ("works with memo-type private payments") fails for exactly the users the gate is meant for.
- **How to investigate:** Trace the wallet-registration flow in Task Node and Pillar 6's `identity_set` fee requirements.
- **Fix direction:**
  - Register the message key at Task Node wallet registration, using a key derived per R9 and recorded off chain.
  - Alternatively, have Task Node sponsor the first `identity_set`, or wait until the identity is set before encrypting.
  - Make the policy explicit.

**P6 — Owned-object cap is a live chain-wide denial-of-service vector, under-escalated (major)**
- **Where:** What exists today (FastPay row: fee 0 accepted, 8 outputs, 100,000 objects chain-wide), R28, Economics.
- **What's wrong:** About 12,500 zero-fee split transfers could fill the cap and halt FastPay for everyone. The page frames the cap only as a pre-condition for opening payouts.
- **Why it matters:** This threatens Outcome B (production-ready chain) regardless of this pillar's timing.
- **How to investigate:** Confirm in `account_owned_asset_types.rs` and the validator code whether the cap is enforced chain-wide and what happens when it is hit. Test on devnet.
- **Fix direction:** Escalate to the chain-readiness gates in the index. Make R27 and R28 a Step 0 security fix rather than Step 2.

**P7 — FastPay anchoring throughput not sized against demand (major)**
- **Where:** R22 (Throughput), open question 2 (pure messages as FastPay self-transfers), R21.
- **What's wrong:**
  - "At most 64 FastPay effects per block" is a chain-wide budget shared by payouts, x402 traffic and any message-only self-transfers.
  - Block time and demand are not quantified. Nobody states whether quorum latency (183 ms) or history visibility degrades under backpressure.
- **Why it matters:** The founder's goal is maximum speed for task payouts, and a shared bottleneck could negate it.
- **How to investigate:** Check the block interval and the anchoring-fix doc. Measure quorum latency versus anchoring latency under load.
- **Fix direction:**
  - Add a capacity table: effects per second at the current block time, against projected payouts and messages.
  - Steer pure messages to `payment_v2` if FastPay capacity is scarce.
  - Make acceptance criterion 11 test backpressure.

**P8 — Contradiction on unknown fields (minor)**
- **Where:** R5 ("Deterministic: … no unknown fields"), R6 ("readers ignore unknown fields"), R16 (vector "unknown fields").
- **What's wrong:** It is unclear whether unknown fields cause rejection or are ignored. The expected outcome of the R16 vector is undefined.
- **Why it matters:** Cross-language clients will disagree, and forward compatibility breaks.
- **Fix direction:** State that writers must not emit unknown fields, and that verifiers accept and ignore them (signatures cover raw bytes). Define the expected vector result.

**P9 — `request_hash` defined in two places (minor)**
- **Where:** R4 (optional Header field `request_hash`), R21 (`request_hash` inline in the order memo).
- **What's wrong:** In x402 there is no envelope, so the Header field is unused there. It is unclear which source is authoritative.
- **Fix direction:** Specify that x402 uses the inline memo form only (tied to the P1 fix). Keep the Header field only for envelope-based replies, or remove it.

**P10 — Chunk nonces derived from a body-nonce prefix (minor)**
- **Where:** R3 (chunk nonce = first 16 bytes of `body_nonce` ‖ chunk index, under the same content key as the manifest body).
- **What's wrong:** A collision with the manifest body's own nonce is negligibly likely but structurally possible. It is also unclear whether manifest bodies (listing chunk CIDs) are themselves encrypted in private mode.
- **Fix direction:**
  - Derive a separate chunk key via HKDF(content key, "pf.msg.v1/chunk").
  - State that private manifests encrypt their chunk list.

**P11 — Lost-seed re-encryption lacks an authorization path (minor)**
- **Where:** R24 ("A user who lost the seed can ask Task Node to re-encrypt payloads it was a reader of").
- **What's wrong:** Proving ownership of the old identity normally requires the old key, which is the thing that was lost. Pillar 6's link is said to "only show which history is the user's".
- **Why it matters:** Re-encryption becomes either impossible or a social-engineering target.
- **Fix direction:** Define the authentication: an existing Task Node account session, a previously recorded link signed while the user still had the seed, or another method. Note the privacy implication that Task Node can read all historical payloads.

**P12 — Step 1 blocked on ML-KEM code that does not exist (minor to major)**
- **Where:** Build order (Step 1 blocked by open question 1 and ML-KEM code), R7 (v1 writes only a PQ suite).
- **What's wrong:** The public envelope, pointer v5, CID, explorer and pinning do not need a KEM, yet the whole first step waits on PRIV-013/014.
- **Fix direction:**
  - Split Step 1 into 1a (public and signed messages, pointer v5, vectors, clients) and 1b (PQ encryption).
  - Let the governance packet (public) ship on 1a.

**P13 — x402 conformance overstated (minor)**
- **Where:** R21.
- **What's wrong:**
  - The payload exceeds 10 KB before base64, about 13.5 KB after, which is near the 16 KB total header default in many servers and CDNs.
  - Standard x402 facilitators and SDKs will not understand ML-DSA FastPay orders.
  - "Conformant" really means "uses x402 headers with a custom network binding".
- **Fix direction:**
  - Say this explicitly.
  - Verify the v2 header names against the current x402 spec.
  - Test with stock proxies (nginx, Node, Cloudflare) in acceptance criterion 7.
  - Consider letting the client upload the payload through a body or a reference.

**P14 — Founder ask for protobuf diluted by the JSON default (minor)**
- **Where:** R1, R5, open question 13, worked example.
- **What's wrong:** The flagship governance example and about 24 schemas use `CANONICAL_JSON`.
- **Fix direction:** Recommend a default to the founder, for example: protobuf for all new schemas and the reward record, with JSON only for a named legacy list and a deadline. Consider converting the governance packet to protobuf to match "protobuf representations".

**P15 — Unclear verification rule for multi-input FastPay orders (minor)**
- **Where:** R19 (*From* when `sender_pubkey` equals the FastPay `owner_pubkey_hex`), R26 (merging up to 2,048 inputs).
- **What's wrong:** The page does not confirm that all inputs of an order share one owner key. If they can differ, the verification rule is ambiguous.
- **How to investigate:** Check `OwnedTransferOrderV3` in `owned_transfer.rs`.
- **Fix direction:** State the single-owner invariant or define the rule for multiple owners.

**P16 — Unverified figures cited as facts (minor)**
- **Where:** p50 183 ms, "epoch-2 committee since height 1123", "about 23 atoms", the "10 KB" order size.
- **What's wrong:** These are cited as facts, and some come from the index rather than reproducible measurements.
- **Fix direction:** Link each one to a test or measurement script, or recompute it, so later revisions stay honest.

**P17 — Density and decision clarity for the founder (minor)**
- **Where:** Whole page; 14 open questions.
- **What's wrong:** The page does not mark which open questions block Step 1 versus later steps, or give a recommended default for each. That makes founder review slow.
- **Fix direction:** Add a short "decisions needed now" box: open question 1 (suite), 3 (spam price, especially given P6), 6 (payout privacy) and 14 (float), each with a recommendation. Move the rest to later steps.
