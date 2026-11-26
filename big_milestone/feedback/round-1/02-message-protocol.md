# Feedback — Pillar 2 — Message Protocol Spec

*Review round 1.*

> **Mean score:** 64/100 · Gemini 4 Neon 58 · Claude Opus 5.5 71
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/02-message-protocol.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 58/100

*Model `google/gemini-4-neon` · 18676 output tokens*

#### Summary judgement

This sub-page is a well-structured, codebase-grounded starting point that accurately captures the *spirit* of Deliverable 2: formalizing Task Node's existing `pf.ptr` pointer-and-IPFS pattern into a versioned protobuf message protocol that rides on `postfiatl1v2` / FastPay, renders cleanly in the block explorer, integrates with x402, and acts as the shared substrate for governance, NFTs, identity, and NAVCoin fees.

However, if handed to engineers as written, **it will not produce something technically robust or economically coherent**. It has three critical technical/architectural flaws—(1) an incomplete and subtly insecure post-quantum cryptography and signing design, (2) a synchronous two-gateway IPFS pinning rule that destroys FastPay's ~200 ms latency and x402 usability, and (3) missing cryptographic binding between on-chain transactions and off-chain envelopes (enabling memo replay/spoofing)—alongside major wire-format contradictions, a circular dependency and 60-schema scope deadlock, a misaligned x402 profile, and almost no economic model for IPFS storage/pinning costs, spam resistance, or micropayment unit economics.

With a focused revision pass (separating Layer 0 envelope/pointer specs from downstream pillar schemas, fixing the crypto/canonicalization/binding spec, decoupling IPFS latency from FastPay, and adding a pinning + fee economic model), this can become a strong, shippable cornerstone spec.

---

#### Correspondence to the founder's intention

**What aligns well:**
- **Core primitive:** Correctly captures protobuf payloads pinned to IPFS, encrypted for private readers, referenced by memo on chain, integrated with FastPay, and decoded on the block explorer.
- **Cross-pillar substrate:** Properly connects Pillar 2 to Deliverable 1 (governance packets), Deliverable 4 (quantum-resistant NFTs contiguous with the message protocol), Deliverable 6 (identity), and Deliverable 7 (NAVCoin fees).
- **Codebase grounding:** Accurately inventories existing Task Node machinery (`pf.ptr` v4, `CONTENT_KIND`, `ENC_X25519_XCHACHA20P1305`, `PaymentMemo` limits, `OwnedTransferOrder.memos`).
- **x402 posture:** Correctly treats x402 as an explicit integration target without letting x402 dictate the core protocol.

**Where it drifts from or omits the founder's intention:**
1. **No migration plan for live Task Node private payments and FastPay task payouts.** The founder's prompt emphasizes that the new chain must *"work with existing memo type private payments"* and *"make fast pay wallets default for massive latency on task payouts."* The sub-page treats Task Node's current pointers only as a historical replay path ("Old-chain v4 pointers stay valid and decodable for replay") rather than specifying how live Task Node task payouts and private messages cut over onto `pf.ptr` v5 + FastPay.
2. **Missing public blog post / readable schema documentation.** Deliverable 1 explicitly states *"I can read about the message protocol schema on the blog,"* and Deliverable 4 similarly expects readable documentation and blog coverage contiguous with the message spec. The sub-page only specifies `docs/specs/message-protocol-v1.md` and `proto/` inside `postfiatl1v2`, omitting any public blog post / documentation deliverable on `postfiatorg.github.io`.
3. **Browser-based key decryption conflicts with local-first privacy.** Augmented Story #4 proposes decrypting private messages in the web block explorer ("a reader with the key can decrypt it in the browser"), whereas Deliverable 5 ("The Task Node Local Experience") emphasizes keeping sensitive privacy operations local rather than trusting web servers/browsers with private keys. Without an explicit wallet-bridge / local-signer architecture, browser decryption encourages unsafe key handling.
4. **Over-coupling other pillars into Pillar 2's delivery gate.** Putting all ~60 existing Task Node event schemas plus unfinished schemas from Pillars 1, 4, 6, and 7 into R1 and Acceptance Criterion #1 blocks Pillar 2 from shipping independently, whereas the founder wants staged, rapid migration.

---

#### Technical robustness

The technical design is currently **under-specified in security-critical places and internally contradictory in several wire-level details**:

1. **Cryptography is incomplete and vulnerable to subtle attacks.**
   - `ENC_MLKEM768_XCHACHA20P1305` treats ML-KEM-768 as if it directly wraps a symmetric file key. ML-KEM-768 is a Key Encapsulation Mechanism (KEM), not a key-wrap algorithm; a complete spec must define KEM encapsulation → HKDF (with domain separation and context binding) → AEAD key-wrap of the Content Encryption Key (CEK).
   - Choosing pure ML-KEM-768 without evaluating hybrid encryption (e.g., X25519 + ML-KEM-768 / X-Wing) risks dropping confidentiality to zero if a lattice implementation bug occurs, while the spec also omits cipher-suite agility (`CipherSuite` enum) despite asking in Open Questions whether classical crypto should ship first.
   - **Protobuf is not canonical.** Signing an `Envelope` protobuf with ML-DSA-65 without specifying deterministic canonicalization (or signing an immutable `bytes payload` + fixed header transcript) will break verification across JS, Python, and Rust encoders.
   - **Encrypt-then-Sign without inner sender authentication or AAD binding** allows surreptitious forwarding / re-signing attacks (an attacker strips the outer ML-DSA-65 signature on a ciphertext and re-signs it as their own).
   - **Key management gaps:** ML-DSA-65 signing keys cannot be used for ML-KEM-768 encryption. Every user needs a separate 1,184-byte ML-KEM-768 encapsulation public key on chain (not verified against L1 account state limits), `Reader` entries need key fingerprints (`key_id`) to survive key rotation, and sender self-encryption (so senders can read their own sent messages) is unspecified.
2. **Transaction-to-envelope binding is missing.** Nothing stops an attacker from copying a valid `pf.ptr` memo (pointing to Alice's validly signed IPFS envelope) and attaching it to Bob's FastPay payment. An explorer that checks only the IPFS envelope signature would display Alice's message on Bob's transaction.
3. **Synchronous IPFS verification breaks FastPay latency.** Requiring every CID to resolve from at least two IPFS gateways *before* submitting a transaction (R7) injects 2–30+ seconds of synchronous pinning and propagation delay in front of a ~200 ms FastPay payment and makes x402 API calls unusable.
4. **Wire-format contradictions and unverified byte budgets.**
   - R1 and R3 say `Pointer` v5 preserves v4 fields 1–8 and adds `envelope_hash`, yet the Architecture diagram shows only 7 fields with `envelope_hash` at field 7.
   - If IPFS uses CIDv1 with the `raw` codec, the CID *already is* the SHA-256 hash of the envelope bytes—making `envelope_hash` redundant unless UnixFS/dag-pb wrapping is permitted (which itself causes cross-pinner CID divergence if not banned).
   - The spec never maps `Pointer` onto `PaymentMemo`'s three fields (`type`, `format`, `data`), never verifies whether `data` is raw bytes vs hex/base64 under the 256-byte limit, leaves v4 flags (`ephemeral`, `tombstone`, `multipart`) undefined, and doesn't explain how the explorer distinguishes legacy JSON blobs from v1 Protobuf envelopes.
   - Naming drifts between `pf.ptr`, `pf.msg` (Augmented Story #5), and `pf.event` (What exists today).
5. **Explorer indexing & DoS surface ignored.** FastPay certificates achieve finality via validator quorums without necessarily appearing in consensus blocks immediately—the spec doesn't specify how the explorer discovers/indexes FastPay certificates. Nor does it bound IPFS fetch sizes, timeouts, malformed protobufs, XSS rendering, or abusive public content.
6. **Missing Rust reference implementation.** The L1 chain, FastPay types, crypto provider, and likely local Task Node/explorer indexers are in Rust, yet R8 only specifies JS and Python reference clients.

---

#### Economic coherence

Economic considerations are largely absent from the sub-page, leaving four significant sustainability and incentive risks:

1. **IPFS storage and bandwidth free-rider / DoS problem.** R7 relies on Post Fiat's Pinata account plus a first-party gateway for a protocol intended to be *"the default for machine-native communication on the Post Fiat Chain"* for *"anyone hoping to use the Post Fiat blockchain."* Without pinning quotas, user-funded pinning, retention SLAs, or garbage-collection rules, Post Fiat either subsidizes unbounded third-party storage/egress (a direct financial griefing vector) or restricts pinning to Task Node (defeating open machine-native communication).
2. **Unanalyzed carrier fee vs. spam economics.** Open Question #2 asks whether to add a zero-value message transaction vs. riding on payments, without evaluating the economic trade-off: zero-value / zero-fee messages invite chain + explorer + IPFS spam, whereas mandatory payment transfers impose dust accounting distortion on pure communication (chat, governance, identity). A dedicated fee-bearing message tx or explicit dust/fee schedule is needed.
3. **x402 micropayment unit economics.** For x402 to drive real agentic usage on Post Fiat, the cost and latency of paying for an API call (often $0.001–$0.05) must be tiny. Requiring a two-gateway IPFS pin per x402 receipt adds both API latency and Pinata storage/request overhead that can easily exceed the value of the API call itself. Furthermore, the spec does not specify whether x402 payments settle in volatile PFT or stable PFUSDC (a key primitive in the founder's prompt).
4. **Engineering ROI / scope bloat.** Building ~60+ bespoke `.proto` files (R1) and ~60+ custom explorer renderers (R10) before shipping v1 consumes massive engineering effort on rapidly changing off-chain Task Node events that could instead be handled by a generic protobuf-descriptor explorer renderer plus bespoke renderers for 5–8 core on-chain message types.

---

#### Problems

##### P1 — Incomplete and insecure post-quantum encryption, signing, and key-custody design
- **Severity:** critical
- **Where it occurs:** `Architecture` (both diagrams), `Technical requirements` (R4, R5), `Augmented story` (#4), `Open questions` (#1).
- **What is wrong:**
  1. `ENC_MLKEM768_XCHACHA20P1305` describes ML-KEM-768 as directly wrapping a file key, omitting the required KEM → KDF (e.g., HKDF-SHA256 with domain separation) → AEAD key-wrap construction.
  2. No hybrid PQ option (X25519 + ML-KEM-768) and no explicit `CipherSuite` enum for algorithm agility, even though Open Question #1 asks about shipping classical first.
  3. ML-DSA-65 signing over an `Envelope` protobuf ignores that protobuf wire encoding is non-canonical across languages/versions.
  4. The flow is Encrypt-then-Sign without binding sender identity or recipient list into the AEAD Associated Data (AAD) or inner signed plaintext, enabling surreptitious forwarding/re-signing attacks.
  5. No `key_id` / public-key fingerprint per reader (breaking decryption on key rotation), no rule for sender self-encryption, no check that L1 account state can store a 1,184-byte ML-KEM-768 public key, and unsafe browser-side private-key decryption on a static explorer.
- **Why it matters:** Cryptographic bugs in a messaging standard cause silent cross-client verification failures, permanent loss of access to historical encrypted messages, spoofing/re-signing vulnerabilities, or catastrophic key theft in the explorer.
- **How an agent should investigate:**
  - Inspect `tasknode/server/task-payloads.js` and `docs/wiki/architecture/encryption.md` to see how X25519 envelopes, AAD, sender copies, and `AccountSet.MessageKey` work today.
  - Inspect `postfiatl1v2/crates/crypto_provider/src/lib.rs` for existing ML-DSA-65 domain separation contexts and check whether an ML-KEM or X-Wing crate is already in `Cargo.toml`.
  - Check `postfiatl1v2` account state structs to verify maximum field sizes for an account-level `MessageKey` (1,184 bytes for ML-KEM-768, or 1,216 bytes for X-Wing).
- **Suggested direction for fix:**
  - Specify a versioned `CipherSuite` enum supporting `X25519_XCHACHA20P1305` (legacy/migration), `XWING_XCHACHA20P1305` (hybrid X25519 + ML-KEM-768 recommended over pure ML-KEM), and/or `MLKEM768_HKDF_SHA256_XCHACHA20P1305`.
  - Write out exact KEM encapsulation + HKDF-SHA256 info string + AEAD wrapping of the 32-byte CEK, include a 16-byte `key_id` per `Reader`, and mandate including the sender as a reader unless flagged otherwise.
  - Sign a domain-separated hash over an immutable `bytes payload` + explicit fixed-order header fields (never re-serialized protobufs), and bind sender/schema/recipients in AEAD AAD (or use Sign-then-Encrypt-then-Sign).
  - Replace direct browser private-key entry with a local wallet/Task Node signer RPC or WebCrypto/extension bridge.

##### P2 — Synchronous two-gateway IPFS pinning destroys FastPay latency and lacks an economic/sustainability model
- **Severity:** critical
- **Where it occurs:** `Technical requirements` (R7), `Augmented story` (#3, #5), `What exists today` (IPFS pinning & FastDB row).
- **What is wrong:** R7 mandates that *"every CID in a memo must resolve from at least two gateways before the transaction is submitted,"* while relying on Task Node's Pinata account + first-party gateway. There is no inline-memo fast path for small payloads, no async pinning path, no storage/bandwidth cost owner, no maximum envelope size, and no pin retention SLA.
- **Why it matters:** Synchronous multi-gateway IPFS propagation takes seconds (and fails during gateway hiccups), completely negating FastPay's ~200 ms finality and making x402 API calls unusable. Economically, opening Post Fiat's Pinata account/gateway to arbitrary chain users creates an unbounded billing/DoS liability, while leaving pinning to third parties without SLAs creates dead links for governance records and NFTs.
- **How an agent should investigate:**
  - Read `postfiatorg.github.io/content/research/fastpay-fastdb-async-ipfs-architecture.md` (already cited in the spec as unbuilt research) and `tasknode/server/context-ipfs.js` to measure current Pinata latency, replication queue behavior, and cost/size limits.
  - Check actual typical message sizes across Task Node events, x402 receipts, and governance packets.
- **Suggested direction for fix:**
  - Define two tiers: (a) **inline payloads** inside the 256-byte memo (for small receipts, hashes, x402 challenges, short structured updates—0 ms IPFS overhead), and (b) **IPFS-backed envelopes** with deterministic local CID computation (raw CIDv1) so clients can compute the CID locally in <1 ms, pin to their own/local node or FastDB immediately, submit FastPay without waiting for remote gateway replication, and asynchronously replicate to durable pinners.
  - Specify explicit protocol caps (e.g., max 1 MB envelope), clearly separate **who pays for pinning** (e.g., Task Node pins Task Node/governance/verified-validator traffic under quota; external apps self-pin or pay a pinning gateway via x402), and define archival pinning SLAs for governance and NFT CIDs.

##### P3 — Missing cryptographic binding between on-chain transactions and off-chain envelopes (memo replay / spoofing)
- **Severity:** critical
- **Where it occurs:** `Architecture`, `Technical requirements` (R1, R3, R9, R11).
- **What is wrong:** The spec signs the IPFS `Envelope` with the sender's ML-DSA-65 key and places the CID in a transaction memo, but does not require the `Envelope` (or verifier rules) to bind the message to the on-chain transaction sender, recipient, chain ID, or unique nonce/context.
- **Why it matters:** Any third party can copy a valid `pf.ptr` memo from Alice's transaction and attach it to their own FastPay or `payment_v2` transaction. An explorer or downstream agent checking only the IPFS envelope signature will misattribute Alice's signed message (e.g., a governance packet, task offer, or payment receipt) to a different transaction. For x402, unbound receipts can be replayed across endpoints or requests.
- **How an agent should investigate:**
  - Check `tasknode/server/pftl-pointer.js` and `task-payloads.js` to see whether v4 currently binds sender account/tx metadata when validating pointers.
  - Check `postfiatl1v2` transaction structures (`OwnedTransferOrder`, `payment_v2`) to see which deterministic fields (sender address, sequence/nonce, chain ID, recipient) are known *before* signing the transaction and can be committed inside the `Envelope`.
- **Suggested direction for fix:**
  - Mandate explorer/verifier checks: `Envelope.sender` MUST match the on-chain transaction signer/account (unless explicitly marked as a delegated/forwarded envelope with verifiable delegation), and include `chain_id`, `tx_sender`, and optional `tx_nonce` / `recipient` / `challenge_hash` inside the signed `Envelope` header.
  - Document explicit verification rules in the spec for both direct-sender messages and x402 receipts.

##### P4 — Wire-format contradictions, redundant `envelope_hash`, and unverified 256-byte memo budget
- **Severity:** major
- **Where it occurs:** `Architecture` (diagram #2), `What exists today`, `Technical requirements` (R1, R3), `Augmented story` (#5).
- **What is wrong:**
  1. R1 and R3 state v5 keeps wire compatibility with v4 fields 1–8 and adds `envelope_hash`, but Architecture diagram #2 shows fields 1–7 with `envelope_hash` as field 7.
  2. `envelope_hash` duplicates what a `raw`-codec CIDv1 already proves, yet the spec never mandates an IPFS codec (`raw` vs `dag-pb` / UnixFS) or CID version (v0 vs v1).
  3. The spec never defines what goes into `PaymentMemo.type`, `PaymentMemo.format`, and `PaymentMemo.data`, nor whether `data` is raw bytes or hex/base64 (hex would halve the 256-byte budget to 128 bytes of protobuf).
  4. Naming is inconsistent (`pf.ptr` vs `pf.msg` in Augmented Story #5 vs `pf.event` in the inventory).
  5. Existing v4 flags (`ephemeral=4`, `tombstone=8`, `multipart=16`) and legacy JSON-vs-protobuf payload discrimination (needed for Acceptance Criterion #4) are completely omitted.
- **Why it matters:** Implementers building JS, Python, Rust, and explorer codecs from this spec will produce mutually incompatible wire formats or silently exceed the L1 256-byte memo limit.
- **How an agent should investigate:**
  - Read `tasknode/server/pftl-pointer.js` and `pointers.py` to extract the exact field numbers, types, and semantics of all 8 v4 fields and all 5 flags.
  - Inspect `postfiatl1v2/crates/types/src/transactions_mempool_receipts.rs`, `core_chain.rs`, and `account_owned_asset_types.rs` to verify exact types (`Vec<u8>` vs `String`) and limits for `PaymentMemo` on both `payment_v2` and FastPay `OwnedTransferOrder`.
- **Suggested direction for fix:**
  - Include an explicit field table for `Pointer` v4 (fields 1–8) and v5 (fields 9+), with worst-case byte-size math proving it fits well inside the L1/FastPay memo limit (using binary CID bytes rather than base32 strings if needed).
  - Mandate CIDv1 with `raw` codec + SHA-256 (eliminating `envelope_hash` unless a concrete UnixFS use case is proven).
  - Standardize `PaymentMemo.type` (`pf.ptr`), `PaymentMemo.format` (`v5/proto` vs `v4/proto`), and `Envelope` payload encoding (distinguishing legacy JSON vs protobuf), and explicitly specify or deprecate `ephemeral`, `tombstone`, and `multipart`.

##### P5 — Scope explosion, centralized schema registry bottleneck, and circular dependency with Pillar 6
- **Severity:** major
- **Where it occurs:** `Technical requirements` (R1, R2, R10), `Dependencies`, `Acceptance criteria` (#1).
- **What is wrong:**
  1. R1 + AC #1 require publishing `.proto` files for ~60+ existing off-chain Task Node JSON schemas *plus* unfinished schemas from Pillars 1, 4, 6, and 7 before Pillar 2 is complete. R10 further requires a custom explorer renderer for *each* schema.
  2. Dependencies state Pillar 2 *depends on* Pillar 6 (On-chain identity) for reader keys, while R1 and R5 place `pf.identity.*` and `pf.identity.message_key.v1` *inside* Pillar 2—a circular dependency.
  3. R2 keeps the numeric `schema_id` registry strictly inside the `postfiatl1v2` repo, creating a centralized PR gate for third-party developers trying to use a "machine-native" protocol.
- **Why it matters:** Pillar 2 becomes a scheduling bottleneck that cannot finish until four other pillars finalize their specs and ~60 renderers are hand-coded, stalling migration to the new chain. Centralizing `schema_id` discourages permissionless agent/developer adoption.
- **How an agent should investigate:**
  - Audit `tasknode/server/task-event-meaning.js` to categorize which of the ~60 schemas actually go on chain vs. stay off-chain in Postgres.
  - Cross-check Pillar 1, 4, 6, and 7 sub-pages to verify schema ownership boundaries and break the Pillar 2 ↔ Pillar 6 cycle.
- **Suggested direction for fix:**
  - Split Pillar 2 into **Core Protocol v1** (`Pointer`, `Envelope`, `Reader`, `ContentKind`, plus 3–5 core/example schemas like `pf.msg.text.v1`, `pf.task.pointer.v1`, `pf.x402.receipt.v1`) and let Pillars 1, 4, 6, and 7 own and register their own `.proto` packages independently.
  - Break the Pillar 6 cycle by defining Layer 0 unencrypted public messages / L1 account `MessageKey` fields as independent of encrypted Pillar 2 messaging.
  - Partition `schema_id`: `0` = custom/unregistered (read full `schema` string from Envelope), `1–65535` = Post Fiat registry, or use a deterministic 4-byte/8-byte hash prefix of the fully qualified schema name.
  - Build a **generic protobuf-descriptor explorer renderer** (using compiled `FileDescriptorSet`) so *any* public schema renders human-readably automatically, supplemented by bespoke UI cards for the top ~5–8 high-value schemas.

##### P6 — x402 profile is technically misaligned with x402 and omits settlement & asset economics
- **Severity:** major
- **Where it occurs:** `Augmented story` (#5), `Technical requirements` (R11), `Acceptance criteria` (#5).
- **What is wrong:** The spec sketches a custom "pay first via FastPay with a `pf.msg` IPFS memo, then retry" flow without aligning to x402's actual protocol structure (`PAYMENT-REQUIRED` / `PAYMENT-SIGNATURE` / `PAYMENT-RESPONSE` headers, CAIP-2 network identifiers, scheme names like `exact`, and optional facilitator `/verify` + `/settle` endpoints). It also ignores what asset is paid (PFT vs. PFUSDC), how servers prevent certificate replay across requests, what happens if the server fails after client prepayment, and whether micropayment fees + IPFS pinning costs are economically viable.
- **Why it matters:** Calling a bespoke 402 flow "x402" won't interoperate with existing x402 SDKs/facilitators/agents, and forcing IPFS pinning on every API payment destroys both latency and unit economics for agentic micropayments.
- **How an agent should investigate:**
  - Review the current Coinbase x402 specification (v1/v2 headers, `PaymentRequirements`, `PaymentPayload`, facilitator API, CAIP-2 network IDs).
  - Check `postfiatl1v2` FastPay capabilities: does `OwnedTransferOrder` currently support only native PFT or also issued assets like PFUSDC? What are current FastPay transaction fees?
- **Suggested direction for fix:**
  - Specify a concrete **x402 `pftl-fastpay` scheme** that maps cleanly to x402's standard headers and supports both (a) **direct certificate mode** (client obtains FastPay certificate with an *inline* 32-byte `request_hash` memo—no IPFS required—and submits it in `PAYMENT-SIGNATURE`) and (b) **facilitator mode** (client sends a signed `OwnedTransferOrder` for the server/facilitator to co-sign/settle atomically with resource delivery).
  - Specify server-side certificate verification (checking validator quorum signatures locally, verifying recipient/asset/amount/expiry, and recording spent certificate digests to prevent replay) and explicitly address PFT vs. PFUSDC support and fee floors.

##### P7 — Missing Task Node live migration path and missing public blog post deliverable
- **Severity:** major
- **Where it occurs:** `In one paragraph`, `What exists today`, `Technical requirements`, `Acceptance criteria`.
- **What is wrong:** The sub-page omits (1) how Task Node migrates live memo-type private payments and FastPay task payouts from the old chain (`pf.ptr` v4 + X25519) to the new chain (`pf.ptr` v5 + FastPay), and (2) the public blog post explaining the message protocol schema on `postfiatorg.github.io` (explicitly expected by the founder in Deliverables 1 and 4).
- **Why it matters:** Without an explicit Task Node cutover requirement, Pillar 2 risks shipping as a standalone spec/devnet demo while Task Node continues running legacy pointers on the old chain—failing the founder's primary migration objective. Without the blog post, external developers and users cannot discover or understand the standard.
- **How an agent should investigate:**
  - Check Task Node's payout and memo-sending paths (`server/pftl-pointer.js`, `server/task-payloads.js`, payout worker code) to list every touchpoint required to switch default task payouts to FastPay + `pf.ptr` v5.
  - Check `postfiatorg.github.io/content/` structure to see where protocol specs and blog posts live.
- **Suggested direction for fix:**
  - Add a requirement and acceptance criterion for **Task Node dual-read / v5-write migration** (sending default task payouts via FastPay with `pf.ptr` v5 memos while continuing to read legacy v4 pointers).
  - Add a concrete deliverable for a **public blog post + docs page** on `postfiatorg.github.io` walking through the schema, worked byte examples, FastPay integration, and how Pillars 1 and 4 build on it.

##### P8 — Explorer FastPay ingestion and DoS/security hardening are unspecified
- **Severity:** major
- **Where it occurs:** `Architecture` (diagram #1), `Technical requirements` (R9, R10), `Dependencies`.
- **What is wrong:** Diagram #1 assumes FastPay orders automatically appear in `PFTL history` and get indexed by the explorer, without specifying how FastPay quorum certificates (which bypass full consensus blocks) propagate to the explorer indexer. Furthermore, R9/R10 require the explorer to fetch and render arbitrary untrusted IPFS envelopes with zero mention of size caps, fetch timeouts, circuit breakers for dead CIDs, protobuf nesting limits, XSS sanitization, or abusive/illegal public content handling.
- **Why it matters:** If FastPay certificates aren't reliably streamed/archived to the indexer, FastPay messages won't appear on the explorer at all. Without DoS and content hardening, a trivial attacker can hang the indexer with dead/huge CIDs, inject XSS into explorer users' browsers, or force Post Fiat's explorer to publicly host illegal content.
- **How an agent should investigate:**
  - Inspect `postfiatl1v2/docs/rpc/account-history.md`, `fastpay-payment-recovery-v1.md`, and `owned_apply_v3` / `owned_certificate` RPCs to see how FastPay certificates are persisted and queried across validators/RPC nodes.
  - Inspect `postfiatorg.github.io/static/explorer/explorer.js` and `docs/explorer-spec.md` to see how the current explorer fetches IPFS content and whether it runs client-side or through an indexer backend.
- **Suggested direction for fix:**
  - Specify the exact FastPay-to-explorer data pipeline (e.g., RPC certificate subscription / archival index on `owned_apply_v3` + fallback on-demand certificate lookup).
  - Add a **Security & Resilience** subsection: strict max envelope size (e.g., 1 MB), strict IPFS fetch timeout + negative-cache backoff for missing CIDs, protobuf decode depth/allocation limits, strict HTML escaping / non-executable rendering for public fields, and a denylist / opt-in click-to-reveal policy for arbitrary public blobs.

##### P9 — Unresolved metadata privacy leakage on "private" messages
- **Severity:** major
- **Where it occurs:** `Architecture` (diagram #2), `Technical requirements` (R1, R4).
- **What is wrong:** Even when a message is encrypted (`Private? -> yes`), the on-chain memo exposes `target`, `kind`, `schema_id`, and `thread_id` in plaintext, and the IPFS envelope exposes `schema`, `sender`, and `readers` in plaintext.
- **Why it matters:** Anyone scraping the chain and IPFS can reconstruct the complete social graph, conversation threads, and message types of every "private" message—undermining the privacy expectations of Task Node users, governance participants, and financial/hive-mind contributors.
- **How an agent should investigate:**
  - Check `tasknode/docs/wiki/architecture/encryption.md` and `server/pftl-pointer.js` to see what metadata Task Node currently leaks vs. hides in v4, and whether Task Node relies on plaintext `thread_id`/`kind` for indexing without decrypting.
- **Suggested direction for fix:**
  - Explicitly document the **privacy threat model** (what is public vs. confidential).
  - For private messages, allow omitting or hashing/salting `target`, `thread_id`, and `schema_id` on chain (moving the real `schema`, `thread_id`, and `kind` *inside* the encrypted inner payload), and consider anonymous/trial-decryption or keyed-hash `key_id` tags for `readers` so third parties cannot trivially enumerate recipient accounts from IPFS.

##### P10 — Missing Rust reference crate alongside JS and Python clients
- **Severity:** minor
- **Where it occurs:** `Technical requirements` (R8), `Acceptance criteria` (#1).
- **What is wrong:** R8 specifies generated reference clients and CI conformance vectors only for JS and Python, omitting Rust.
- **Why it matters:** `postfiatl1v2`, FastPay, the ML-DSA-65 crypto provider, and likely the local Task Node privacy engine and explorer indexer are written in Rust. Without a canonical Rust crate (`pf-msg-proto` / `prost` types) verified against the exact same conformance vectors, Rust and JS/Python implementations will drift.
- **How an agent should investigate:**
  - Check `postfiatl1v2/Cargo.toml` workspace crates to see whether `prost` or protobuf codegen is already used in the repo.
- **Suggested direction for fix:**
  - Add a Rust crate (`crates/pf-msg` or `crates/message-protocol`) to R8 and AC #1, tested in CI against the same golden byte-level conformance vectors as JS and Python.

##### P11 — Unanalyzed fee and spam economics for zero-value message transactions vs. payment carriers
- **Severity:** minor
- **Where it occurs:** `Technical requirements` (R6(c)), `Open questions for the founder` (#2).
- **What is wrong:** The spec mentions a potential zero-value message transaction and asks the founder whether to add one, without analyzing the economic implications (fee market, anti-spam minimum fees, state bloat, vs. dust friction when messages must carry a PFT transfer).
- **Why it matters:** Asking the founder a binary product question without surfacing the fee/spam/accounting trade-offs makes it harder to make a sound decision, and risks shipping either a spam-vulnerable zero-fee tx or an awkward UX where every chat/identity update requires moving PFT dust.
- **How an agent should investigate:**
  - Check `postfiatl1v2` fee rules (`core_chain.rs` / FastPay fee model) and whether `payment_v2` or `OwnedTransferOrder` already permits `amount == 0` (paying only the tx fee).
- **Suggested direction for fix:**
  - Verify whether `amount == 0` with standard tx fee is already valid or trivial to support on `payment_v2` / FastPay, and present the founder with a concrete recommendation (e.g., fee-bearing `amount == 0` memo tx or dedicated fee-bearing `MessageTx` so pure messages pay anti-spam fees without moving PFT principal).

## Claude Opus 5.5 — 71/100

*Model `anthropic/claude-opus-5.5` · 10821 output tokens*

### Summary judgement

This is one of the stronger kinds of sub-page. It covers what exists today accurately, with code pointers. It stays close to the founder's words: protobuf, IPFS, encryption, explorer, FastPay examples, x402. It also turns the deliverable into testable requirements (R1–R11) and acceptance criteria. The architecture is sensible: a small on-chain pointer, a large off-chain envelope, a schema registry and conformance vectors.

It loses points for four things:

- **Unexamined technical details that will bite in implementation:** whether the pointer fits in 256 bytes, deterministic encoding for signatures, metadata leakage, a key-discovery bootstrap cycle and IPFS latency versus FastPay latency.
- **A thin x402 section** that does not engage with how x402 actually works (payment schemes, facilitators, the `X-PAYMENT` header).
- **Almost no economic design:** who pays for pinning, spam, fees and the asset for x402.
- **An unresolved tension.** It wants this to be "the default for machine-native communication," but Task Node has moved events into Postgres.

The spec is likely to produce something that works as a developer-facing format. It is less certain to produce a protocol that is private, fast and self-sustaining.

### Correspondence to the founder's intention

**Strong matches:**

- **Standardized spec with protobuf and IPFS pinning.** R1 and R2 add real `.proto` files and a registry.
- **"Encrypted properly."** R4 adds a post-quantum suite, which fits the founder's quantum-resistance concern in Deliverable 4.
- **"Easily consumed on the block explorer."** R9 and R10 add a Message panel and per-schema renderers.
- **"Clear examples of how it integrates with FastPay … represented on the Block Explorer."** R6, acceptance criterion 2 and step 3 of the augmented story cover this.
- **"Conformant with X402 or other integration points."** R11 covers this, and an open question about MCP was added.
- **The founder's wider framing.** The "bubbles up to" line positions this as the common substrate for governance packets (D1), NFTs (D4), identity (D6) and NAVCoin fees (D7). That matches "natively contiguous with the MESSAGE PROTOCOL SPEC" in D4 and "tagged on an IPFS blob the same way that our existing message protocol works" in D1.

**Weaker spots:**

- **"Default for machine-native communication."** The founder wants this to be the default. The page itself notes that Task Node events now live off chain in Postgres, with pointers only a replay path. The page never says how this spec becomes the default: what Task Node must change, and which flows must go on chain. As written, it could produce a nice spec that the flagship app does not use.
- **"I can read about the message protocol schema on the blog"** (from D1). The founder expects blog-level public explanation. The page targets only `docs/specs/...`. There is no blog or website deliverable, even though the founder repeatedly wants public narrative ("post human governance is a new primitive").
- **"Works with existing memo type private payments"** (a performance gate in the founder's list). Old encrypted payloads use X25519 keys derived from the old chain. The page only commits to *decoding* v4 pointers, not to keeping old ciphertexts readable or migrating reader keys.
- **The founder's push for maximum speed and lowest latency on task payouts via FastPay.** The page adds pre-submit IPFS resolution checks (R7) that work against this, without acknowledging the trade-off.

### Technical robustness

**Good:**

- The pointer/envelope split respects the real 256-byte memo data limit.
- Keeping v4 wire compatibility is correct.
- Conformance vectors in CI are the right discipline.
- Choosing ML-KEM-768 with XChaCha20-Poly1305 and ML-DSA-65 aligns with the chain's existing crypto provider.
- Listing several carriers (FastPay, `payment_v2`, a possible message transaction) is sensible.

**Concerns:**

- **Byte budget never computed.** The pointer byte budget is asserted, not computed.
- **Pointer field list is inconsistent.** The diagram shows 7 fields, while R3 says "fields 1–8 plus envelope_hash."
- **`envelope_hash` looks redundant.** It appears to duplicate the CID's own content hash.
- **No deterministic serialization rule.** Protobuf has none, yet the design signs envelopes and promises "exact bytes."
- **The envelope signature's scope is undefined:** domain separation, chain id, binding to the carrying transaction.
- **On-chain metadata (kind, schema_id, target, thread_id, reader count) leaks heavily for "private" messages.**
- **Key handling is incomplete.** Browser-side decryption implies handling secret keys in a web page, and key rotation and revocation are missing.
- **Circular dependency.** Pillar 6 identity is itself a message type, yet reader-key discovery depends on Pillar 6.
- **Tombstone and ephemeral flags** conflict with IPFS immutability.
- **The x402 mapping lacks a scheme definition, a facilitator or verifier model, and committee-certificate verification** for servers.

### Economic coherence

Largely absent:

- No statement of who pays for IPFS pinning and replication, which today is Pinata plus a first-party gateway. There is no pin lifetime and no garbage-collection policy.
- No spam or anti-abuse model for messages. This matters especially if a zero-value message transaction is added, since cheap messages with free pinning are a denial-of-service vector.
- No link between message volume and PFT demand or fee capture.
- The x402 section does not say which asset is paid: PFT, or PFUSDC, which the founder introduces for NAVCoins and which is the natural analogue of x402's USDC default.
- Per-request x402 payments that each require an IPFS pin would be uneconomic and slow. Receipts should be inline.
- A schema registry kept in the repo makes the core team a gatekeeper. That conflicts with the founder's "primitive that any user can build" and "permissionless by default" philosophy.

### Problems

**P1 — Pointer byte budget not computed; field list inconsistent**
- **Severity:** major
- **Where:** R2 and R3; the second mermaid diagram ("≤256 bytes data", 7 fields); the What-exists table ("fields 1–8").
- **What's wrong:** The diagram shows 7 fields: cid, target, kind, schema_id, thread_id, flags, envelope_hash. R3 says v4 has fields 1–8 and v5 adds `envelope_hash`, which would make 9. The page never states the byte cost. Estimated sizes:
  - a binary CIDv1 is about 36 bytes;
  - a target address could be 32 bytes or more, depending on the new-chain address format;
  - thread_id could be 16–32 bytes;
  - envelope_hash is 32 bytes;
  - plus tags and varints.
  This probably fits, but not if any field is text-encoded (for example a base32 CID of about 59 characters, or hex addresses), or if multipart adds a second CID.
- **Why it matters:** The 256-byte limit is a hard protocol constant. Getting it wrong means a v6 bump or a chain change.
- **Investigate:** Read `buildPftPointerMemo` to list the actual 8 v4 fields and their wire types. Check the new-chain address length in `postfiatl1v2`. Write a worst-case encoding.
- **Fix direction:** Add a table of field number, name, wire type and max bytes, with a worst-case total. Reconcile the diagram with R3. Require binary CIDs.

**P2 — `envelope_hash` purpose undefined and likely redundant**
- **Severity:** minor/major
- **Where:** R3; the diagram.
- **What's wrong:** A CID is already a cryptographic hash of the envelope bytes. A second hash adds 32 bytes to a tight memo, with no stated purpose: binding plaintext, choosing an alternative hash, or guarding against gateway substitution.
- **Why it matters:** It wastes memo space and creates confusion about which hash is authoritative.
- **Investigate:** Check whether the FastPay/IPFS research doc proposes it for a reason, such as async pinning where the CID is not known yet.
- **Fix direction:** Either drop it, or define it precisely (for example, a hash of the plaintext body for receivers to verify after decryption) and justify the cost.

**P3 — No deterministic serialization or signature binding rules**
- **Severity:** major
- **Where:** R1, R4 and R8; the augmented story ("exact bytes").
- **What's wrong:** Protobuf encoding is not canonical. Field order, unknown fields and default values can all vary. The envelope is ML-DSA-signed, but the page does not say:
  - which bytes are signed;
  - which ML-DSA context string is used (the crypto provider has contexts);
  - whether the signature binds chain id, schema id or the carrying transaction.
- **Why it matters:** Two conformant clients may produce different bytes and therefore different CIDs and signatures. Without binding, the signature permits cross-context replay: the same envelope attached to a different payment or chain.
- **Investigate:** Look at the ML-DSA contexts in `crates/crypto_provider/src/lib.rs`. Look at how L1 transactions achieve canonical encoding.
- **Fix direction:** Specify canonical encoding: deterministic protobuf with ascending field order, no unknown fields, explicit rules for defaults. Alternatively, sign a defined preimage such as `domain || chain_id || schema_id || sha256(body)`. Add a dedicated ML-DSA context for message signatures, and add negative test vectors.

**P4 — Metadata leakage undermines "private"**
- **Severity:** major
- **Where:** The memo fields (target, kind, schema_id, thread_id); R9's "Encrypted to N readers" badge; the envelope reader list.
- **What's wrong:** For an encrypted message, the public chain still shows:
  - the content kind and schema (for example `pf.reward.v1`);
  - the target;
  - the thread linkage;
  - the number of readers.
  The envelope may also list reader key ids publicly. The founder links privacy to production readiness ("privacy working", shielded transactions in D5).
- **Why it matters:** Traffic analysis reveals who rewards whom and for what. That defeats much of the point of encryption and conflicts with the shielded Task Node vision.
- **Investigate:** Check what v4 exposes today. Check how Pillar 5 (shielded transactions) expects messages to work.
- **Fix direction:** Define a "sealed" flag mode in which kind and schema_id move inside the ciphertext. In that mode, use anonymous reader slots (trial decryption, or blinded key ids), allow an optional padded reader count, and consider omitting `target`. Document the privacy levels explicitly.

**P5 — Key discovery bootstrap is circular, and key lifecycle is missing**
- **Severity:** major
- **Where:** R5; the Dependencies table.
- **What's wrong:**
  - Reader ML-KEM keys are found either via an account field or via a `pf.identity.message_key.v1` *message*, which is itself carried by this protocol and depends on Pillar 6.
  - Nothing covers key rotation, revocation, multiple devices, or what happens to old ciphertexts after rotation.
  - Nothing says how ML-KEM keys relate to the ML-DSA account key: whether they are derived from the same seed, or how they are attested.
- **Why it matters:** Encryption cannot ship without a decided key directory. Message-based key publication also needs an authentication rule (signed by the account key, latest wins, and so on).
- **Investigate:** Check whether postfiatl1v2 has an account-field mechanism comparable to XRPL `AccountSet`. Check the Pillar 6 draft.
- **Fix direction:** Pick one primary mechanism for v1. The preferred option is a native account field or a minimal key-registration transaction, so the protocol does not depend on itself. Define:
  - attestation, with the ML-KEM key signed by the ML-DSA key;
  - rotation semantics;
  - multi-device keys;
  - whether seed-deterministic derivation is used, consistent with the founder's deterministic-identity theme in D6.

**P6 — Old-chain encrypted payload compatibility is decode-only**
- **Severity:** major
- **Where:** R3, R4 and acceptance criterion 4.
- **What's wrong:** "Old-chain v4 pointers still decode" only covers pointers. Readers of old X25519 payloads need their old keys, which live in XRPL-derived wallets and `AccountSet.MessageKey`. The founder's performance gate says the new chain must "work with existing memo type private payments."
- **Why it matters:** The migration could strand historical private context, for example Task Node context history restore.
- **Investigate:** Read `context-history-restore.md` and `encryption.md` to see how keys are derived. Check whether users will hold both key sets.
- **Fix direction:** Add a migration requirement: a dual-suite client, an optional re-encryption tool, and a mapping from old key to new account, linked to Pillar 6 identity linking.

**P7 — IPFS pre-submit resolution conflicts with FastPay latency; pinning is centralized**
- **Severity:** major
- **Where:** R7; augmented story step 3 ("final in about 200 ms").
- **What's wrong:** Pinning and confirming resolution from two gateways typically takes seconds or more, which dwarfs FastPay's sub-second finality. Both gateways (Pinata and the first-party gateway) are under the team's control or contract, so "two gateways" gives little real redundancy. The "about 200 ms" claim is also asserted without a citation.
- **Why it matters:** The founder's explicit goal is maximum speed for task payouts. The design either slows payouts or silently accepts dangling CIDs.
- **Investigate:**
  - Measure the current pin-to-resolve latency in `context-ipfs.js`.
  - Read the async IPFS research doc, which apparently proposes async pinning.
  - Check FastPay latency measurements on devnet.
- **Fix direction:**
  - Allow an async mode: submit the payment immediately, and pin with a retry SLA.
  - Use the CID computed locally, which is deterministic and needs no network.
  - Let the explorer show "content pending."
  - Make strict pre-resolution optional for high-value messages.
  - Pursue truly independent pinning: validators or RPC operators pin, tied to the Pillar 3 validator rewards.

**P8 — No economics for storage, spam or fees**
- **Severity:** major
- **Where:** Entire page; R6(c) on zero-value messages; R7.
- **What's wrong:** Nothing covers:
  - who pays for pinning, and for how long;
  - limits on message size;
  - spam prevention, especially for zero-value message transactions;
  - whether messages pay PFT fees that accrue to validators or are burned.
- **Why it matters:** An unbounded free storage service run by the core team is not sustainable. It is also an abuse vector. It misses a natural demand driver for PFT.
- **Investigate:** Check the current Pinata costs and usage. Check the L1 fee model for `payment_v2` and FastPay.
- **Fix direction:** Add an "Economics" section covering:
  - a minimum fee or stake per message;
  - pin-lifetime tiers paid in PFT;
  - an optional validator pinning market with rewards;
  - size caps per kind;
  - rate limits;
  - a stated policy for unpinned content.

**P9 — x402 profile is underspecified relative to the actual x402 design**
- **Severity:** major
- **Where:** R11; augmented story step 5; acceptance criterion 5; the open question about x402.
- **What's wrong:** x402 is built around:
  - `PaymentRequirements` in the 402 response (scheme, network, asset, amount, payTo);
  - an `X-PAYMENT` header carrying a payment payload;
  - an optional facilitator with `/verify` and `/settle`;
  - an `X-PAYMENT-RESPONSE` header.

  The page says "the exact headers" will be shown but does not commit to a Post Fiat scheme or network identifier, a facilitator role, or how a server verifies a FastPay certificate. Verifying a certificate requires the server to know the current committee's public keys, which change with epochs.

  The page also says a `pf.msg` receipt memo, while the memo type elsewhere is `pf.ptr`. This naming is inconsistent.
- **Why it matters:** Without these, "x402 conformance" is aspirational. Agents integrating with existing x402 tooling will not interoperate.
- **Investigate:** Read the current x402 spec and SDKs (Coinbase x402 repo) for the scheme extension points. Check FastPay `owned_certificate` RPC and how committee keys are fetched per epoch.
- **Fix direction:**
  - Define a Post Fiat x402 scheme (for example `exact` on `postfiat:mainnet`).
  - Specify the payload as a serialized FastPay certificate.
  - Provide a reference facilitator service that verifies against the committee.
  - Make the memo carry `sha256(request)` inline, with no IPFS, for replay binding.
  - Specify a nonce or expiry.
  - Fix the `pf.msg`/`pf.ptr` naming.

**P10 — x402 asset choice is not addressed**
- **Severity:** minor/major
- **Where:** R11.
- **What's wrong:** x402's ecosystem centers on stablecoins. The founder introduces PFUSDC. The page does not say whether x402 payments are in PFT, PFUSDC or any asset.
- **Why it matters:** Agent APIs price in dollars. Paying in a volatile token hurts adoption, while paying in PFUSDC ties this pillar to the NAVCoin/PFUSDC work.
- **Investigate:** Check whether FastPay supports non-native or issued assets in `OwnedTransferOrder`.
- **Fix direction:** Specify multi-asset support, with PFUSDC as the default x402 asset once it is available, and PFT as a fallback.

**P11 — Schema registry is centralized and conflicts with permissionless primitives**
- **Severity:** major
- **Where:** R2.
- **What's wrong:** Numeric `schema_id`s live in the core repo, so third parties (NAVCoin builders, external agents) need a PR to add a type. There is no reserved range for experimental or private use and no collision policy.
- **Why it matters:** The founder wants primitives "any user can build" and permissionless-by-default design. A gatekept registry slows ecosystem use as the "default machine-native" layer.
- **Investigate:** Count today's roughly 60 schema strings. Decide whether a 2-byte or 4-byte id fits the budget.
- **Fix direction:**
  - Reserve ranges: core, governed, and an open range where the id is a truncated hash of the fully qualified name.
  - Optionally publish an on-chain registry message so schemas are self-describing.
  - Let the explorer fetch `.proto` descriptors from IPFS for unknown ids.

**P12 — "Default" adoption path for Task Node is missing**
- **Severity:** major
- **Where:** "What exists today" (events in Postgres); the "In one paragraph" section.
- **What's wrong:** The page does not say which Task Node flows will emit v5 messages on the new chain, or how adoption will be measured.
- **Why it matters:** The founder's intent is that this becomes the default. Without an adoption plan, the spec stays theoretical, and the explorer has little real traffic to render.
- **Investigate:** Read `task-lifecycle.md` for which events still emit pointers.
- **Fix direction:** Add a requirement listing which Task Node events will be emitted on chain (rewards, submissions, governance packets, NFT mints), with a cutover milestone and an acceptance criterion.

**P13 — Browser decryption implies handling secret keys in a web page**
- **Severity:** major
- **Where:** Augmented story step 4; R9.
- **What's wrong:** "A reader with the key can decrypt it in the browser" on the explorer means pasting or loading ML-KEM secret keys into a website. Doing so trains users to expose keys to a web origin.
- **Why it matters:** This is a security and phishing risk. It also contradicts D5's local-first shielded model.
- **Investigate:** Check how the current explorer handles decryption, if it does at all.
- **Fix direction:** Do decryption in the local Task Node app or a wallet extension, through a postMessage or signing-request API. The explorer should only show metadata and offer "open in local app."

**P14 — Tombstone and ephemeral flags have no semantics on immutable IPFS and chain**
- **Severity:** minor
- **Where:** The Flags row (tombstone=8, ephemeral=4); R3.
- **What's wrong:** The flags are carried over without defined meaning on the new chain. Content cannot be deleted from IPFS caches or chain history.
- **Why it matters:** Users may assume deletion is possible. Legal and privacy expectations would be wrong.
- **Fix direction:** Define the flags as advisory: unpin, renderer hides, keys destroyed for encrypted content. Document the limits.

**P15 — Schema evolution rules absent**
- **Severity:** minor
- **Where:** R1 and R3.
- **What's wrong:** Nothing covers reserved field numbers, how unknown fields are handled, the version bump policy (`v1` to `v2` per schema), or how the explorer renders unknown versions.
- **Why it matters:** Sixty-plus schemas will evolve, and without rules clients will break.
- **Fix direction:** Add an evolution section: reserved fields, a mandatory policy for ignoring unknown fields, and the criteria for a major version.

**P16 — Size limits and multipart undefined**
- **Severity:** minor
- **Where:** The IPFS row (1 MB JSON cap); the multipart flag; R7.
- **What's wrong:** The page has no envelope size limit for protobuf. Per-reader ML-KEM ciphertexts (about 1088 bytes each), ML-DSA signatures (3309 bytes) and public keys (1952 bytes) add overhead. Multipart manifest structure is not defined.
- **Why it matters:** Governance packets (D1) and NFT media (D4) may be large, and many-reader messages grow linearly.
- **Fix direction:** Define a max envelope size, a manifest schema for multipart (a DAG of CIDs) and a reader-count cap, or group keys for large audiences.

**P17 — Open question on post-quantum timing contradicts committed requirements**
- **Severity:** minor
- **Where:** The open questions versus R4 and acceptance criterion 3.
- **What's wrong:** R4 and acceptance criterion 3 already require ML-KEM, yet the page asks whether post-quantum should come first. The page also does not consider hybrid X25519+ML-KEM, which is the current best practice.
- **Why it matters:** Ambiguity for implementers.
- **Fix direction:** Recommend a hybrid suite as the v1 default, given the founder's quantum-resistance emphasis. Turn the question into a confirmation request.

**P18 — No public/blog explainer deliverable**
- **Severity:** minor
- **Where:** The user story and acceptance criteria.
- **What's wrong:** D1 expects the schema to be readable "on the blog." The founder's strategy relies on public narrative.
- **Fix direction:** Add a blog post or website page with diagrams and worked examples as an acceptance criterion.

**P19 — Pillar 1 governance packet flow not shown end to end**
- **Severity:** minor
- **Where:** R1 and R10.
- **What's wrong:** The founder's first, highest-priority use is goodalexander uploading a protocol-state packet in Hive Chat. That packet lives on Nostr, so the Nostr-to-chain bridge is not addressed. No worked example covers this case.
- **Fix direction:** Add a worked example: the `pf.governance.packet.v1` envelope, the memo, its explorer rendering and how a Hive Chat (Nostr) event references the CID.

**P20 — FastPay concurrency and the owned-object model for agent message rates**
- **Severity:** minor
- **Where:** R6(a).
- **What's wrong:** FastPay owned transfers are sequenced per account. Agents sending many messages concurrently may hit sequence contention, and recovery flows (v3) may apply.
- **Fix direction:** Document the expected throughput per account and the client-side sequencing guidance, referencing the FastPay recovery spec.
