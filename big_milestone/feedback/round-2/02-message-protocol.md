# Feedback — Pillar 2 — Message Protocol Spec

*Review round 2.*

> **Mean score:** 73/100 · Gemini 4 Neon 74 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/02-message-protocol.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 22971 output tokens*

#### Summary judgement

This is a well-researched, tightly written round-2 specification that aligns closely with the founder's goals and is unusually well grounded in the actual `postfiatl1v2`, `tasknode`, and `postfiatorg.github.io` codebases. It catches real implementation traps (upper-case vs lower-case hex, `pf_ptr_v1.proto` drift, FastPay missing from `account_tx`, PFT-only FastPay, disabled `wrap_owned`, randomised ML-DSA signing) and cleanly defines how Pillars 1, 4, 5, 6, 7, and 8 plug into a shared core.

However, it is not yet ready to hand directly to engineers without another tightening pass. It still contains four **critical** protocol/wire-format flaws that will cause security regressions, cross-language interoperability failures, or runtime breakage in production:
1. `readers[]` is excluded from both the ML-DSA signature and the AEAD associated data, directly contradicting R7's security claim.
2. The x402 Direct mode stuffs a post-quantum FastPay quorum certificate (multiple 3.3 KB ML-DSA-65 signatures, totaling 15–70+ KB Base64) into an HTTP `PAYMENT-SIGNATURE` header, which standard HTTP servers, proxies, and CDNs will reject with `431` / `400`, alongside missing server-nonce recovery and epoch-rotation handling.
3. R2 declares `cid_bin` "Required" while R2/R14/R21 allow CID-less `inline` payloads, and the 256-byte memo budget does not verify whether L1 enforces 256 *decoded bytes* or 256 *hex characters* (128 bytes).
4. The encryption wire format omits all XChaCha20-Poly1305 nonces, leaves HKDF parameters incomplete, and does not define the inner envelope format or blinded-reader HKDF inputs for Sealed mode.

It also leaves several **major** operational and economic gaps—most notably an apparent zero-fee FastPay memo spam vector left as an open question rather than a launch gate, unverified FastPay certificate indexing assumptions, missing multi-chunk `Manifest` encryption/size bounds, and incomplete FastPay-by-default wallet UX (concurrent payout UTXO pooling, recipient consolidation, and exiting FastPay to pay consensus fees). Fixing these will move the spec into the 90+ range.

#### Correspondence to the founder's intention

The sub-page faithfully covers almost everything the founder asked for in Deliverable 2, the readiness gates, and the cross-pillar references:
- **Standardized protobuf + IPFS + encryption:** Directly addressed via versioned `.proto` packages (`R1`–`R11`), multi-language generated reference clients (`R16`), and mandatory post-quantum encryption (`R7`) with dual-suite backward compatibility for old-chain payloads (`R24`).
- **Block explorer consumption:** Well covered via the Messages lens, generic `FileDescriptorSet` renderer, five custom cards, *Verified* / *Forwarded* / *Unverified* attribution, and XSS/DoS hardening (`R17`–`R20`).
- **FastPay integration & default Task Node payouts:** Explicitly owned (`R12`, `R13`, `R15`, `R22`, `R23`, `AC 2`, `AC 6`), with async pinning decoupled from payment quorum so FastPay latency is preserved.
- **x402 conformance:** Concrete x402 v2 profile (`R21`, `AC 7`), with other agent protocols (e.g., MCP payment hooks) surfaced in Open Question 9.
- **Docs, blog post, and cross-pillar contiguity:** Covered in `R25`, `AC 9`, the Pillar 1 governance-packet worked example, and explicit hook points for Pillar 4 NFTs and Pillar 6 identity.

The main correspondence gap is around the founder's phrasing *"make fast pay wallets default for massive latency on task payouts"*. The spec addresses the **sender** side (Task Node paying out via FastPay by default in `R22`), but glosses over the **recipient** user's default FastPay wallet experience: how a user who receives all payouts as FastPay owned objects consolidates them, sees a unified balance, or unwraps/exits to consensus PFT to pay fees for consensus-only features (identity, NFTs, PFUSDC, NAVCoins).

#### Technical robustness

The high-level architecture (signed envelope -> offline CIDv1 raw -> compact protobuf pointer in L1 memo -> async IPFS pin -> verified explorer rendering -> local wallet decryption) is sound. The problems are concentrated at the wire-format, cryptographic, HTTP-transport, and L1-state boundaries:

- **Cryptography & canonicalization (`R4`, `R5`, `R7`, `R8`, `R11`):**
  - `readers[]` sits outside `Header`, and `R5` signs only `header + SHA3-384(body)`, while `R7` uses only `header` as AEAD AAD. Thus `readers[]` is neither signed nor authenticated—contradicting `R7`'s claim that readers cannot be swapped.
  - `R5` requires verifying "the stored header bytes" without re-serialization, yet `R1`/`R4` model `Header` as a normal protobuf submessage rather than an opaque `bytes header_bytes` field; standard `prost`/JS/Python protobuf decoders discard raw submessage wire bytes during parsing.
  - Nonces for both key-wrapping AEAD and body AEAD are unspecified, HKDF inputs omit the KEM shared secret and salt/info layout, and Sealed mode (`R11`) leaves its outer-vs-inner header layout and blinded-reader HKDF inputs undefined.
- **Wire budget & schema consistency (`R2`, `R3`, `R14`):**
  - `cid_bin` is marked "Required" even though `inline` payloads have no IPFS object.
  - The L1 memo limit ("data ≤ 256 bytes, 512 total, in lower-case hex only") needs code verification to ensure 256 is *decoded bytes* (512 hex chars) rather than 256 *hex characters* (128 decoded bytes), which would break the 165-byte v5 pointer budget.
  - Multi-chunk `Manifest` (`R3`) defines no encryption, chunk-nonce, or total-size-cap rules, leaving >1 MiB messages (common in Task Node today, which allows 8–16 MiB) unimplementable across languages and vulnerable to reassembly DoS in the explorer.
- **x402 over post-quantum FastPay (`R21`):**
  - Carrying a full ML-DSA-65 FastPay quorum certificate in the `PAYMENT-SIGNATURE` HTTP header produces 15–70+ KB headers that break standard HTTP stacks.
  - The server cannot recompute `extra.request_hash` on retry without a specified mechanism to recover and expire the server nonce, and pinning a static `fastpay_recovery_committee` root breaks on epoch rotation.
- **L1/IPFS integration (`R13`, `R17`, `R22`):**
  - `R17` allows FastPay discovery to live solely in the explorer indexer ("reading applied certificates from certified blocks"), which (a) may be impossible if applied FastPay certificates are not embedded in consensus blocks, and (b) leaves wallets, the Python CLI, and local Task Node unable to discover incoming FastPay messages via L1 RPC.
  - Async IPFS pinning (`R13`) omits how recipients/explorers achieve low-latency content discovery (public IPFS DHT lookup takes seconds to tens of seconds, undermining FastPay's 183 ms latency).

#### Economic coherence

The round-1 revision improved economic honesty by separating code facts from founder decisions, but three concrete economic vulnerabilities remain unresolved:

1. **Unpriced FastPay memo spam (launch blocker):** The spec notes it found **no FastPay fee floor**, and `payment_v2` charges only 1 atom per 512 bytes (with no PFT/USD conversion showing whether 1 atom provides any real spam resistance). Leaving FastPay memo fees purely as Open Question 3 without establishing a mandatory pre-launch anti-spam floor allows zero-cost flooding of FastPay validators, RPC history, the explorer indexer, and IPFS fetch queues.
2. **Unbounded pinning liability and missing MVP boundary:** Today Task Node pays for all pinning, yet Pillar 2 positions this as the chain-wide machine-native messaging layer. Open Question 4 lists four options without recommending a safe launch default (e.g., authenticated pinning for Task Node + Pillar 1 + Pillar 4 only; self-pinning or x402-paid pinning for third parties) or a durability budget beyond a single commercial Pinata account.
3. **Unquantified x402 lane trade-off:** FastPay is PFT-only while real-world x402 API billing strongly favors PFUSDC (consensus lane only). Without stating consensus finality latency and fees for `issued_payment_v2`—or how an x402 facilitator covers its operating costs—the founder lacks the numbers needed to decide Open Question 8. Similarly, the capital/fee overhead of Task Node's pre-split FastPay hot wallet and user FastPay-to-consensus exits is unanalyzed.

#### Problems

##### P1 — `readers[]` is neither signed nor authenticated in AEAD AAD (contradicts R7)
- **Severity:** critical
- **Where it occurs:** Technical requirements — R4, R5, R7.
- **What is wrong:** R4 places `readers[]` outside `Header`. R5 states the ML-DSA signature covers only "the stored header bytes plus the SHA3-384 of the stored body". R7 states "The body AEAD uses the header as associated data, so readers, sender and schema cannot be swapped". Because `readers[]` is not in `Header`, neither the signature nor the body AEAD binds `readers[]`.
- **Why it matters:** A malicious recipient (who knows the content encryption key) or intermediary can mutate `readers[]`—adding an unauthorized reader, stripping a legitimate reader, or padding junk entries—without invalidating the sender's ML-DSA signature or the body ciphertext. While changing `readers[]` changes the IPFS CID (preventing silent in-place mutation of an already-pinned CID), an attacker can pin the altered envelope and present it (on-chain as a forwarded/new tx or off-chain via Hive/Nostr) as genuinely signed by the original sender for a different recipient list. It also violates the spec's own stated security invariant in R7.
- **How an agent should investigate:** Review the exact envelope signing and AEAD construction in R4, R5, R7, and check PRIV-013/014 in `postfiatl1v2/docs/status/privacy-production-burndown.md` to ensure shared KEM/DEM code aligns.
- **Suggested direction for the fix:** Generate the random 32-byte CEK first, wrap it for all readers, serialize `readers[]` into `bytes readers_bytes` (or compute `readers_hash = SHA3-384(readers_bytes)`), then encrypt the body with AAD = domain-separated `(header_bytes, readers_hash)` and sign `domain_sep || SHA3-384(header_bytes) || readers_hash || SHA3-384(body)`. There is no circularity because KEK derivation only depends on `header_hash`, not `readers_hash`.

##### P2 — x402 Direct mode exceeds standard HTTP header limits with PQ FastPay certificates, and omits nonce recovery & epoch rotation
- **Severity:** critical
- **Where it occurs:** Architecture (sequence diagram) and Technical requirements — R21.
- **What is wrong:**
  1. Direct mode puts a full FastPay v3 quorum certificate in the `PAYMENT-SIGNATURE` HTTP header. Because Post Fiat uses ML-DSA-65 (3,309-byte signatures, 1,952-byte pubkeys), a certificate with 2f+1 validator signatures is ~10–50+ KB binary (~13–70+ KB Base64). Even a single signed order in Facilitator mode is ~7–8 KB Base64.
  2. R21 computes `extra.request_hash` over `(method, URL, body hash, server nonce)`, but does not specify how the server recovers or validates the server nonce and `maxTimeoutSeconds` expiry on the retried HTTP request.
  3. R21 checks quorum against a "pinned `fastpay_recovery_committee` registry root", which breaks whenever validator epochs rotate.
  4. R21 does not clarify who calls `owned_apply_v3` so the payee can actually spend the received object (if the client stops after collecting votes and sends the unapplied certificate).
- **Why it matters:** Nginx (8 KB default), Apache (8 KB), Node.js (16 KB), Cloudflare (16–32 KB), and AWS ALB (16 KB) will immediately reject `PAYMENT-SIGNATURE` headers carrying PQ FastPay certificates with `431 Request Header Fields Too Large` or `400 Bad Request`, breaking x402 in real deployments. Without nonce recovery/validation, the server cannot recompute `request_hash` on retry; without epoch refresh, x402 breaks at every epoch boundary.
- **How an agent should investigate:** Measure exact serialized byte sizes of `OwnedTransferOrderV3` and `OwnedTransferCertificateV3` in `crates/types/src/fastpay_recovery_types.rs` for realistic committee sizes (4, 7, 13, 21 validators). Check x402 v2 transport spec for header vs body/reference conventions and inspect `fastpay_recovery_committee` and `owned_apply_v3` in `docs/rpc/methods.md`.
- **Suggested direction for the fix:** In Direct mode, require the client to apply the certificate (`owned_apply_v3`) and send only a compact `PAYMENT-SIGNATURE` payload containing `certificate_digest` (48 bytes), `server_nonce` (or a server-issued signed challenge token with expiry), and `epoch`; the server fetches/verifies the certificate via local RPC (`owned_certificate`) or caches it. For Facilitator mode, measure exact order size and document required header-buffer sizing (or pass large PQ payloads via a compact reference/facilitator endpoint). Replace the static pinned committee root with an epoch-aware committee cache refreshed from L1 RPC.

##### P3 — `cid_bin` "Required" contradicts `inline` payloads, and L1 memo byte-vs-hex-character limit is unverified
- **Severity:** critical
- **Where it occurs:** What exists today (L1 memo), Technical requirements — R2, R14, R21.
- **What is wrong:**
  1. R2 marks field 9 `cid_bin` as "Required", yet R2 field 10 (`inline`), R14, and R21 define inline payloads with **no IPFS object** (and therefore no CID).
  2. "What exists today" says L1 memos allow `type and format ≤ 64 bytes, data ≤ 256 bytes, 512 total, in lower-case hex only`, while R2 budgets ~165 protobuf bytes (330 hex characters) against a 256-byte cap. It is ambiguous whether L1 checks decoded byte length (`<= 256` raw bytes = 512 hex chars) or hex string length (`<= 256` hex chars = 128 raw bytes), and what `512 total` constrains (all fields in one memo, or all 4 memos combined).
- **Why it matters:** If L1 validates `data_hex.len() <= 256` (128 decoded bytes), the v5 pointer design (165 bytes with 3 UUID strings) will be rejected by consensus on every full pointer. Meanwhile, marking `cid_bin` required breaks all inline x402 and receipt pointers.
- **How an agent should investigate:** Read the exact validation predicates in `crates/types/src/transactions_mempool_receipts.rs` and `crates/types/src/core_chain.rs` for `payment_v2`, `issued_payment_v2`, and `OwnedTransferOrderV3.memos` (checking whether limits apply pre- or post-hex-decode, and whether `512 total` is per-tx or per-memo).
- **Suggested direction for the fix:** Change R2 fields 9 and 10 to a mutually exclusive `oneof payload { bytes cid_bin = 9; bytes inline = 10; }` (or explicitly make `cid_bin` optional when `inline` is populated). Explicitly state the verified L1 limits in both decoded bytes and hex characters; if L1 only allows 128 decoded bytes in `data`, switch `task_id`/`thread_id`/`context_id` to 16-byte binary UUIDs (saving 60 bytes) or split across multiple memos / increase the L1 memo cap.

##### P4 — Unspecified AEAD nonces, incomplete HKDF inputs, and undefined Sealed-mode wire format
- **Severity:** critical
- **Where it occurs:** Technical requirements — R4, R7, R8, R11.
- **What is wrong:**
  1. R7 mandates XChaCha20-Poly1305 for both key wrapping and body encryption, but never specifies where the 24-byte nonces live (stored in `Header`/`Reader`, prepended to ciphertext, HKDF-derived, or fixed zero) or what AAD is used during key wrapping.
  2. R7 specifies "HKDF-SHA256 over the suite id, `chain_id`, the header hash and the reader's `key_id`" without stating the IKM (the KEM shared secret), salt, or `info` encoding, nor how X-Wing's built-in SHA3-256 combiner interacts with HKDF-SHA256.
  3. R11 (Sealed mode) says "Kind, schema and ids move inside the ciphertext, reader ids are blinded (readers trial-decrypt) and the reader count can be padded", but R4 puts `schema` and `kind` in the plaintext `Header`—no outer sentinel header, inner encrypted payload schema, blinded-reader HKDF `key_id` rule, or dummy-reader indistinguishability rule is defined.
- **Why it matters:** Three independent implementations (JS, Python, Rust in R16) cannot interoperate if nonces, HKDF parameters, and Sealed inner envelopes are left to implementer guesswork—and ad-hoc nonce handling risks catastrophic nonce reuse.
- **How an agent should investigate:** Check `postfiatl1v2/docs/status/privacy-production-burndown.md` (PRIV-013/014) and any KEM/DEM PRs or drafts in `postfiatl1v2` to align exact nonce derivation, HKDF salt/info formatting, and X-Wing vs pure ML-KEM-768 conventions.
- **Suggested direction for the fix:** Specify exact fields and byte layouts: e.g., derive `(kek[32], wrap_nonce[24]) = HKDF-SHA256(ikm=kem_ss, salt=header_hash, info="pf.msg.v1/wrap" || suite_id || chain_id || reader_key_id)` with empty or `header_hash` AAD; store a random 24-byte `body_nonce` in `Header` (or derive uniquely per-chunk when using `Manifest`). For Sealed mode, explicitly define an outer `Header` with `schema = "pf.msg.sealed.v1"`, `kind = SEALED`, and an inner protobuf `SealedPayload { string schema; ContentKind kind; string task_id; string thread_id; string context_id; bytes body; }`, clarify whether `sender_pubkey` stays in the outer header, and specify that blinded readers omit `key_id` on the wire while still binding the reader's true `key_id` (or fixed empty bytes) in HKDF during trial decryption.

##### P5 — "Verifiers never re-serialize" conflicts with modeling `Header` as a protobuf submessage
- **Severity:** major
- **Where it occurs:** Technical requirements — R1, R4, R5.
- **What is wrong:** R5 mandates that signatures cover "the stored header bytes" and that "verifiers never re-serialize", while R6 mandates that "readers ignore unknown fields". However, R1/R4 describe `Envelope` as containing a structured `Header` submessage. Standard protobuf libraries (`prost`, `protobufjs`, Python `protobuf`) parse submessages into structs and discard raw wire bytes (and in `prost`, unknown fields are discarded completely), making verbatim byte verification impossible without custom wire parsers.
- **Why it matters:** Either engineers will secretly re-serialize `Header` to verify signatures (which breaks forward compatibility as soon as a newer producer adds a field, violating R6), or they will have to write fragile hand-rolled protobuf wire slicers in three languages.
- **How an agent should investigate:** Check `prost` version/behavior in `postfiatl1v2` and standard protobuf signed-envelope patterns (e.g., Cosmos `TxRaw` storing `bytes body_bytes`).
- **Suggested direction for the fix:** Define `Envelope` with `bytes header_bytes = 1; bytes readers_bytes = 2; bytes body = 3; bytes signature = 4;` (or `repeated bytes readers = 2` hashed in order). Verifiers verify the ML-DSA signature and AEAD AAD directly over the raw `bytes` fields first, then decode `header_bytes` into `Header` and `readers_bytes` into `ReaderList`.

##### P6 — FastPay certificate discovery (`R17`) may rely on non-existent block data and strands wallets/CLI behind the explorer indexer
- **Severity:** major
- **Where it occurs:** What exists today (FastPay in history), Technical requirements — R17, R16, R22.
- **What is wrong:** R17 offers an "either/or": either L1 adds FastPay certificates and memos to `account_tx`/`tx`, *or* "the indexer reads applied certificates from certified blocks". Two problems:
  1. FastPay is an out-of-consensus lane; it is not verified in the spec whether applied FastPay certificates (with their memos) are actually included inside certified consensus blocks at all.
  2. Even if an explorer indexer can scrape them, taking the indexer-only option means L1 RPC (`account_tx` or an `owned_history` RPC) still cannot tell a recipient wallet, the Python CLI (AC 9), or a local Task Node (Pillar 5) what FastPay messages it has received.
- **Why it matters:** Without L1 RPC discovery of incoming FastPay certificates/memos by account, "FastPay as the default for Task Node payouts" and local/self-sovereign wallets (Pillar 5) become hard-dependent on a centralized explorer indexer just to discover that they received a payment or encrypted memo.
- **How an agent should investigate:** Inspect `postfiatl1v2` RPC and consensus block structures (`crates/types/src/fastpay_recovery_types.rs`, `docs/rpc/account-history.md`, `docs/rpc/methods.md`, and block/execution crates) to determine where applied FastPay certificates are persisted and whether they appear in consensus blocks or only in node/validator FastPay stores.
- **Suggested direction for the fix:** Make L1 RPC indexing of applied FastPay certificates/memos by sender and recipient account (in `account_tx` or a dedicated `account_owned_tx` RPC) a firm requirement rather than an optional fallback to an explorer-only scraper, so both the explorer and self-sovereign wallets/CLIs share one canonical discovery interface.

##### P7 — Multi-chunk `Manifest` (`R3`) lacks an encryption/chunking wire spec and DoS size caps
- **Severity:** major
- **Where it occurs:** Technical requirements — R1, R3, R20.
- **What is wrong:** R3 caps single IPFS envelopes at 1 MiB and delegates larger payloads (note that Task Node today allows up to 16 MiB JSON and 8 MiB files) to "a signed `Manifest` of chunk CIDs". However, the spec does not define: (a) whether chunks are plaintext ciphertext slices encrypted with the Manifest's CEK or independent envelopes, (b) how per-chunk nonces and AADs prevent chunk reordering, duplication, or truncation, (c) chunk size, or (d) maximum chunk count / total reassembled size.
- **Why it matters:** Existing Task Node payloads >1 MiB will immediately hit `Manifest` during migration (R23). Without a defined chunk-encryption scheme, JS/Python/Rust clients won't interoperate and naive chunking leaks/breaks AEAD security. Without a max chunk/byte cap, a small `Manifest` referencing 10,000 1-MiB chunks becomes a 10 GB reassembly DoS vector against the explorer (R20) and clients.
- **How an agent should investigate:** Check actual Task Node payload size distributions (`server/context-ipfs.js`, `server/task-payloads.js`) to see how often >1 MiB messages occur and how `multipart=16` is currently handled (if at all).
- **Suggested direction for the fix:** Either (a) specify deterministic streaming/chunked AEAD for `Manifest` (fixed max chunk size ≤ 1 MiB, per-chunk nonce derived from `base_nonce XOR chunk_index`, AAD binding `header_hash || chunk_index || is_final`, chunk CIDs listed in the encrypted or signed `Manifest`, and hard caps like max 32 chunks / 16 MiB total; explorer refuses auto-reassembly above 1 MiB and offers explicit download), or (b) explicitly defer `Manifest` to v1.1 if Task Node payloads can stay ≤ 1 MiB at launch.

##### P8 — Unpriced FastPay memo spam and unanalyzed L1 byte-fee economics
- **Severity:** major
- **Where it occurs:** What exists today (L1 memo, FastPay memos), Economics, Open questions 2–4 and 8.
- **What is wrong:**
  1. The spec states "I found no FastPay fee floor (this needs a test)" and leaves FastPay memo pricing to Open Question 3 rather than making a non-zero fee floor a mandatory pre-launch requirement.
  2. `payment_v2` charges "at least 1 atom, 1 atom per 512 bytes", with no PFT/USD valuation of 1 atom to assess whether it deters spam at all.
  3. Open Question 4 leaves third-party IPFS pinning wide open without recommending a safe Stage-2 launch default, and Open Question 8 asks whether to run x402 on PFUSDC (`issued_payment_v2`) without stating consensus lane finality latency or facilitator fee economics.
- **Why it matters:** Launching FastPay memos with a 0-atom fee floor exposes validators, RPC nodes, explorer indexers, and IPFS fetch workers to trivial infinite-rate DoS. Launching an open pinning endpoint without an explicit launch boundary invites immediate storage abuse. Leaving x402 consensus latency unquantified prevents the founder from making an informed decision on PFT vs PFUSDC.
- **How an agent should investigate:** Inspect `crates/execution/src/owned_transfer.rs` and `crates/execution/src/fees_offer_planning.rs` to verify FastPay's exact fee validation and the atom-to-PFT denomination (`1 PFT = ? atoms`). Check devnet/testnet consensus block finality times in `postfiatl1v2` docs/benchmarks for `issued_payment_v2`.
- **Suggested direction for the fix:** Add a mandatory acceptance criterion / readiness requirement that FastPay transfers (at minimum those carrying memos) enforce a non-zero burned fee floor before public launch. Convert `1 atom` to PFT and show the cost of 1 MB of memo spam on both lanes. Recommend a concrete Stage-2 default for pinning (Task Node pins authenticated Task Node + Pillar 1 + Pillar 4 traffic only; third-party messages self-pin or pay via x402; Pillar 1/4 records replicated to at least two independent pinners) and add consensus lane latency/fee numbers to Open Question 8.

##### P9 — Default FastPay payouts (`R22`) omit payout UTXO pool concurrency and recipient wallet exit/consolidation UX
- **Severity:** major
- **Where it occurs:** User story, Technical requirements — R15, R22, R23.
- **What is wrong:**
  1. On the **sender** side, R22 makes FastPay the default for Task Node payouts (plus daily airdrops in R23) funded via consensus FastLane deposit, and R15 mentions splitting owned objects for high-rate agents—but R22 does not specify the Task Node payout service's owned-object pool manager (pre-splitting, per-object lease/locking to avoid concurrent self-equivocation, automatic refilling, and recovery queue).
  2. On the **recipient** side, a Task Node user who receives FastPay payouts by default accumulates many small owned objects and has 0 consensus PFT if they never funded the consensus lane. The spec never explains how the default Task Node wallet displays a unified balance, consolidates owned objects (and what `OwnedTransferOrderV3`'s max input count is), or moves FastPay PFT back to the consensus lane so users can pay fees for consensus transactions (`identity_set`, NFTs, `issued_payment_v2`, NAVCoins).
- **Why it matters:** Without a payout pool manager, concurrent Task Node rewards will lock/equivocate the hot wallet's owned objects and fall back to `payment_v2` (defeating the FastPay default). Without a clean recipient consolidation and FastPay-to-consensus exit flow, making FastPay wallets default will trap users with spendable FastPay PFT who cannot execute consensus-lane transactions.
- **How an agent should investigate:** Check `crates/types/src/fastpay_recovery_types.rs` and `crates/execution/src/owned_transfer.rs` for max inputs/outputs per `OwnedTransferOrderV3` and the exact mechanism to unwrap/withdraw owned FastPay PFT back to a consensus balance (especially since `wrap_owned` is disabled). Inspect `tasknode`'s L1 wallet code to see what FastPay UI/balances currently exist.
- **Suggested direction for the fix:** Add concrete requirements under R22 for (a) Task Node's server-side owned-object pool manager (N parallel objects, mutex/lease checkout, automated FastLane refill), and (b) the user-facing default wallet experience (unified PFT balance showing FastPay + consensus, automatic background/one-click owned-object consolidation, and an explicit FastPay-to-consensus exit path for consensus fees).

##### P10 — Async IPFS discovery latency undermines FastPay's 183 ms messaging promise
- **Severity:** major
- **Where it occurs:** In one paragraph, User story (step 3), Technical requirements — R13, Failure modes.
- **What is wrong:** R13 decouples pinning from FastPay submission ("submits the payment at once and pins in parallel") and cites 183 ms p50 FastPay latency. However, it does not specify how a recipient agent or the explorer actually *locates and fetches* a brand-new CID quickly. Public IPFS DHT advertisement and lookup typically take 2–30+ seconds, and if the async pin has not yet completed when the FastPay certificate arrives 183 ms later, immediate fetches will 404 (and could even trigger R20's "negative caching of dead CIDs"!).
- **Why it matters:** If a recipient or explorer hits a 404 at t = 200 ms and negatively caches the CID—or waits 15 seconds for IPFS DHT routing—FastPay's speed advantage is lost for machine-native messaging, and transient async-pin lag turns into false *Content unavailable* badges.
- **How an agent should investigate:** Review `content/research/fastpay-fastdb-async-ipfs-architecture.md` and `server/context-ipfs.js` to see how Post Fiat's gateway racing, Bitswap peering, and negative cache TTLs are designed.
- **Suggested direction for the fix:** Distinguish clearly between *payment/commitment finality* (~183 ms) and *blob availability*, specify a short retry/backoff window (e.g., 5–30 s) before transitioning from *Content pending* to *Content unavailable* (and never negatively cache a CID younger than that grace window), and define fast content routing (e.g., direct upload to Post Fiat's first-party gateway / shared Bitswap peer list, or optional inline delivery for small time-critical agent messages).

##### P11 — Old-chain X25519 decryption (`R24`) conflates Pillar 6's public key link with secret-key retention
- **Severity:** minor
- **Where it occurs:** Technical requirements — R24, Acceptance criteria — AC 5.
- **What is wrong:** R24 states: "Old X25519 payloads decrypt with the user's old seed, which Pillar 6's old-chain key link ties to the new account." While technically true that the public link correlates the two accounts, the wording implies Pillar 6's on-chain link enables decryption, omitting that the user's **old XRPL/X25519 private key** must be imported or retained locally in the new wallet (and securely wiped only after optional re-encryption).
- **Why it matters:** Implementers building the new wallet (R10 / Pillar 5) need an explicit key-import / local vault storage flow for legacy X25519 secret keys, not just a Pillar 6 RPC lookup.
- **How an agent should investigate:** Check `tasknode/docs/wiki/architecture/encryption.md` and `context-history-restore.md` to confirm how the old X25519 secret key is derived from the XRPL seed and whether users hold separate encryption keys.
- **Suggested direction for the fix:** Clarify in R24 that Pillar 6's key link correlates old-chain history to the new account for discovery/UI, while local decryption requires importing the legacy XRPL seed (or exported X25519 private key) into the wallet's local key store until the user runs the optional re-encryption migration tool.

##### P12 — Minor schema & wire-format edge cases: unmigrated Task Node schemas, `flags` semantics, and proto package layout
- **Severity:** minor
- **Where it occurs:** Technical requirements — R1, R2, R19, R23.
- **What is wrong:**
  1. R1 publishes `.proto` files only for `pf.msg.text.v1` and the 6 lifecycle schemas, yet R23 migrates *every* current `pf.ptr` flow (including context publish and daily airdrop—part of the ~30 `pf.*.v1` names) to v5 without stating whether those non-lifecycle schemas get `.proto` definitions or use `CANONICAL_JSON` inside v5 envelopes.
  2. R2 inherits v4's `flags` bitmask (`encrypted=1 default, public=2, ...`) plus `sealed=32`, without specifying what `flags == 0` (proto3 default!) means or rejecting contradictory flag combinations (e.g., `encrypted | public`, or `sealed` without `encrypted`).
  3. R1 puts `pf.reward.v1` and `pf.task.*.v1` under `proto/pf/msg/v1/`, which breaks standard Protobuf/Buf package-to-directory conventions.
  4. R19 mentions matching `sender_pubkey` against FastPay `owner_pubkey_hex` and `payment_v2` `from`, omitting `issued_payment_v2` and not specifying address derivation (`address(sender_pubkey) == tx.from`) for account-based transactions.
- **Why it matters:** Ambiguous `flags == 0` handling can cause security confusion (treating an unflagged private message as public or vice versa), and unspecified schema encodings for context/airdrop flows will stall Stage-2 migration engineers.
- **How an agent should investigate:** Enumerate all ~30 `pf.*.v1` schemas in `tasknode/server/task-event-meaning.js` and `task-review-publication.js`, and check `postfiatl1v2` address derivation from ML-DSA-65 pubkeys.
- **Suggested direction for the fix:** State in R23 that the remaining ~24 legacy Task Node schemas migrate to v5 using `CANONICAL_JSON` until individually converted to `.proto`; define exact valid `flags` combinations in R2 (reject `flags == 0` or make `public`/`encrypted`/`sealed` mutually exclusive and required); split Task Node schemas into `proto/pf/task/v1/` (keeping `proto/pf/msg/v1/` for core envelope/pointer/text types); and update R19 to include `issued_payment_v2` and explicit `address(sender_pubkey) == tx.from` verification.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 12270 output tokens*

#### Summary judgement

This is a strong, code-grounded spec. It covers almost everything the founder asked for in Deliverable 2:

- protobuf bodies on IPFS;
- encryption;
- explorer rendering;
- FastPay integration;
- an x402 profile;
- the two readiness gates it owns, memo-type private payments and FastPay-default payouts.

Its inventory of the current code is unusually honest. It records that FastPay certificates are not in `account_tx`, that FastPay carries PFT only, that the L1 accepts only lower-case hex, that no ML-KEM exists yet and that a mismatched `pf_ptr_v1.proto` is in the repo. It also corrects its own earlier mistakes. Correspondence to the founder's intent is high.

The score falls short of the high 80s for four reasons:

1. **Cryptographic binding claims.** Some are stated more strongly than the construction supports. Readers are not signed or bound. "Sealed" does not hide what it claims to hide, because the envelope header sits in clear on IPFS.
2. **Unverified byte-level assumptions.** The memo budget's hex-versus-bytes semantics, raw-CID pinning on Pinata and self-payment validity are all unchecked, and each could break the design.
3. **Undecided dependencies.** The explorer ask hinges on R17, which is left as either/or. Pure messages, spam pricing and third-party pinning are deferred to open questions with no recommended default.
4. **x402 conformance.** "Direct mode" with a pre-settled certificate is probably not conformant with the x402 `exact` scheme.

The economics are mostly acknowledged rather than resolved.

#### Correspondence to the founder's intention

**Covered well:**

- **Standardized spec.** Versioned `.proto` files, a registry, reference clients in JS, Python and Rust, and shared test vectors (R1, R6, R16).
- **IPFS pinning of protobuf, encrypted properly.** R3, R7–R11. The page also goes beyond the ask by making encryption post-quantum, which is consistent with the founder's quantum-resistance theme in Deliverable 4.
- **Easy to consume on the explorer.** R17–R20, including a generic `FileDescriptorSet` renderer and verified-sender semantics.
- **Default for machine-native communication.** Pillars 1, 4, 6, 7 and 8 register types on the core protocol, so the core ships on its own.
- **Clear examples with FastPay transactions, represented on the explorer.** R12 gives byte examples per carrier, and R17 is correctly identified as the blocker for explorer rendering.
- **x402 conformance.** R21 plus a sequence diagram.
- **Readiness gates.** Gate 2 (memo-type private payments) and gate 3 (FastPay-default payouts) are covered by R22–R24 and AC 6.
- **"Read about the schema on the blog"** (from Deliverable 1). Covered by R25.

**Gaps relative to intent:**

- The founder wants this to *become the default* for machine-native communication. The page does not say who pays for third-party pinning or what an outside agent should do on day one. Both are left open (Q4). A default needs a defaulted path.
- "Represented appropriately on the Block Explorer" for FastPay is conditional on L1 work that the spec does not commit to (R17 says "either…or").
- Gate 2 says the chain must "work with existing memo type private payments". The page maps this onto encrypted-memo transparent payments. But the new chain's real private payments, PFSH03 shielded transfers, carry no memo, and that is pushed to Q12. That is defensible, but the founder may read "private payments" to include shielded ones, which Deliverable 5 explicitly wants.
- The founder asked for protobuf representations. The page allows `CANONICAL_JSON` for governance packets and NFT metadata. This is justified, but it is a deviation the founder should approve explicitly.
- There is no sequencing or milestone plan inside the pillar, such as "M1 spec + vectors, M2 devnet payout, M3 explorer". The founder's framing question is about *which activities* maximize success.

#### Technical robustness

**Strengths:**

- Dispatching on memo format rather than re-reading v4 fields is correct.
- The CIDv1 raw, SHA-256, offline-computable CID is the right choice.
- Verifiers never re-serialize the signed bytes.
- A domain-separated ML-DSA context is used.
- Randomised signing is acknowledged, along with its consequence that CIDs are not deterministic.
- AEAD binds the header as associated data.
- The clear plaintext hash from v4 is removed.
- Explorer hardening is listed.
- *Verified* vs *Forwarded* semantics are defined.
- Equivocation and recovery are referenced for FastPay.
- The x402 replay and idempotency rules are explicit.

**Weaknesses (detailed in Problems):**

- The reader list is not authenticated.
- Sealed mode leaks via the public IPFS envelope header.
- The R2 budget may be off by a factor of two if the 256-byte limit counts hex characters.
- The inline profile conflicts with "cid_bin required".
- Raw-block pinning on Pinata is unverified.
- Self-payment validity for pure messages and governance packets is unverified.
- Task Node's multi-instance signing could equivocate on FastPay objects.
- The x402 server's committee root goes stale across epochs.
- Spent-certificate state must be shared across server instances.
- Sealed-mode discovery has no recipient hint.
- All acceptance criteria are devnet-only.

#### Economic coherence

- **Spam is underpriced on the fast path.** FastPay has no fee floor, as the page itself found. That means up to 512 bytes of memo per order at FastPay speed for roughly nothing beyond the smallest transfer. The page parks this in Q3 rather than proposing a floor. Since FastPay is the *default* carrier, this is the main economic hole.
- **Pinning cost is unbounded under "default for machine-native communication".** Task Node pays for everything today, and the page offers four options with no recommendation and no cost estimate. A rough figure is easy: ~7.5 KB per two-reader PQ envelope × expected messages/day × Pinata $/GB-month.
- **Liquidity on the payout wallet.** Because `wrap_owned` is disabled, the payout wallet needs capital locked through the consensus FastLane deposit, plus object-splitting management. There is no sizing.
- **x402 asset mismatch.** The x402 ecosystem is USDC-denominated, but FastPay is PFT-only. PFUSDC would have to go over the slower consensus lane, which weakens the "agent payments at FastPay speed" story. This is acknowledged but not resolved.
- **Self-payments and dust rows.** Self-payment carriers burn fees and add rows, and their incentives are unexamined.

Overall, the page correctly separates what the code fixes from what the founder must decide. It does not give the founder costed recommendations.

#### Problems

**P1 — Reader list not authenticated; "readers cannot be swapped" is overstated**
- **Severity:** major
- **Where:** R4, R5, R7, R8.
- **What is wrong:** R4 places `readers[]` *after* the header. The R5 signature covers only the header bytes plus the body hash. R7's HKDF and AEAD bind the header and the reader's own `key_id`, not the reader set. As a result, any legitimate reader, who holds the content key, can append new `Reader` entries or strip existing ones. The envelope still verifies as signed by the original sender, under a new CID. R7's claim that "readers… cannot be swapped" is therefore not supported.
- **Why it matters:** A sender's authenticated message can be re-shared to parties the sender never chose, and it will display *verified*. It also undermines R19's Forwarded logic, which only checks the carrier.
- **Investigate:** Write the envelope byte layout explicitly, then attempt a reader-append test vector.
- **Fix:** Either include a hash of the canonical `readers[]` bytes in the signed material (header field `readers_hash`), or move readers into the header. Add a negative vector for "reader appended" and "reader removed".

**P2 — Sealed mode leaks via the public envelope header**
- **Severity:** major
- **Where:** R4 ("full schema name (always)", `sender_pubkey`, `created_at_ms`), R11 Sealed.
- **What is wrong:** Sealed moves kind, schema and ids off the chain. But the envelope is publicly fetchable on IPFS by CID, and its clear header carries the schema name, sender public key, cipher suite and timestamp. Reader `key_id`s must also be blinded, but how is not specified.
- **Why it matters:** Sealed payouts (Q6) would still reveal the "pf.reward.v1" schema and the sender to anyone who fetches the CID from the chain.
- **Investigate:** Enumerate every clear-text field of a Sealed envelope.
- **Fix:** Define a Sealed header profile in which schema, kind, ids and `sender_pubkey` live inside the ciphertext, and the signature is placed inside the ciphertext too (sign-then-encrypt). Define the blinding of `key_id`, for example `HMAC(kem_ss, "pf-reader-tag")`.

**P3 — Memo data budget may assume bytes when the limit is hex characters**
- **Severity:** critical
- **Where:** R2 budget (~165 bytes, 89 left for `inline`, "refuse over 256"); the L1 memo row ("data ≤ 256 bytes… lower-case hex only").
- **What is wrong:** If `transactions_mempool_receipts.rs` limits the *hex string* to 256 characters, which is common with XRPL-style hex memos, then only 128 raw bytes fit. A 165-byte pointer would be rejected.
- **Why it matters:** Every non-sealed v5 pointer, and therefore the whole payout cutover, would fail validation.
- **Investigate:** Read the memo validation code. Check whether the length is applied before or after hex decode, then build the worst-case vector from AC 1 against the real validator.
- **Fix:** If the limit is 128 bytes, change `task_id`/`thread_id`/`context_id` from 36-character UUID strings to 16-byte binary fields. That brings the pointer to about 95 bytes. Alternatively, push the ids into the envelope.

**P4 — Inline profile contradicts "cid_bin required"**
- **Severity:** minor
- **Where:** R2 table (field 9 "Required"), R14 (inline payloads "never put on IPFS"), R21 (x402 memo holds `request_hash` inline).
- **What is wrong:** A pointer can't both require a CID and have no IPFS object.
- **Why it matters:** Implementations will diverge, and the vectors will contradict each other.
- **Investigate:** Cross-check R2 against R14 and R21.
- **Fix:** Make `cid_bin` required unless `inline` is present, or add an `inline` flag. Add vectors for both.

**P5 — Raw-codec CIDv1 vs the pinning service's behaviour unverified**
- **Severity:** major
- **Where:** R3, R13; the IPFS row (Pinata plus CIDv0 UnixFS today).
- **What is wrong:** Pinata's standard upload path builds a UnixFS DAG and returns a different CID from a locally computed raw-block CID. Preserving the client's CID requires CAR uploads or block-put semantics.
- **Why it matters:** If the service re-chunks or re-wraps, the on-chain CID never resolves, and every message shows *Content unavailable*.
- **Investigate:** Test a Pinata CAR or raw upload of a 900 KiB block and confirm the returned CID. Also test gateway retrieval of raw-codec CIDs.
- **Fix:** Specify the upload method (CAR v1 containing a single raw block), have the client verify the returned CID, and require pinners to reject mismatches.

**P6 — Self-payment validity unverified for pure messages and governance packets**
- **Severity:** major
- **Where:** Q2 options, the governance worked example (`payment_v2` self-payment), Q5.
- **What is wrong:** XRPL rejects self-payments of the native asset, and FastPay may reject transfers whose recipient is the owner. The page verified that zero amounts are rejected, but not that self-payments are accepted.
- **Why it matters:** The governance packet flow (Pillar 1) and the no-chain-change options both depend on it.
- **Investigate:** Check the `payment_v2` and `owned_transfer` validation for source == destination.
- **Fix:** If rejected, either pay a designated sink or burn address, or prioritize the `message` transaction type.

**P7 — Explorer rendering of FastPay depends on an uncommitted L1 choice**
- **Severity:** major
- **Where:** R17, Dependencies.
- **What is wrong:** The founder's explicit ask, FastPay messages represented on the explorer, is blocked. R17 lists two options with no owner, decision or estimate.
- **Why it matters:** This is the most visible success signal for the deliverable.
- **Investigate:** Assess whether certified blocks expose applied certificates to an indexer today, and the cost of adding certificates to `account_tx`.
- **Fix:** Recommend one option, most likely L1 history rows since other wallets need them too, and make it a gate with an acceptance criterion.

**P8 — FastPay spam is free on the default carrier**
- **Severity:** major (economic)
- **Where:** Economics, Q3, the FastPay row ("no FastPay fee floor").
- **What is wrong:** No proposal is made, even though FastPay is the default carrier.
- **Why it matters:** Making FastPay the default invites unpriced memo spam and indexer and pinning load.
- **Investigate:** Confirm the fee path in `owned_transfer.rs` and measure validator cost per memo order.
- **Fix:** Propose a default, for example a minimum fee plus a per-byte fee for memo-bearing orders mirroring `payment_v2`'s 1 atom per 512 bytes. Make it a chain change tracked under the readiness gates.

**P9 — Pinning economics unresolved and uncosted**
- **Severity:** major (economic)
- **Where:** Economics, Q4.
- **What is wrong:** Four options are offered with no recommendation and no estimate of volume or cost. Retention periods for governance packets and NFT metadata are undefined.
- **Why it matters:** A protocol positioned as "default for machine-native communication" needs a sustainable pinning answer before third parties adopt it.
- **Investigate:** Estimate envelope sizes (PQ overhead of ~5.3 KB plus ~1.1 KB per reader) × expected message rate × provider pricing.
- **Fix:** Recommend a default: Task Node pins its own traffic and a quota of public messages, and others self-pin or pay over x402. Validator pinning (Pillar 3) can be a later option.

**P10 — x402 "direct mode" likely not conformant with the `exact` scheme**
- **Severity:** major
- **Where:** R21, the sequence diagram.
- **What is wrong:** In x402, the client sends a signed *authorization* (`PAYMENT-SIGNATURE`), and the server or facilitator verifies and *settles* it. In direct mode, the client settles first and then presents a certificate. That is a different trust and refund model, and calling it `exact` may break off-the-shelf x402 clients and facilitators.
- **Why it matters:** The founder wants "conformant with X402".
- **Investigate:** Compare R21 against the x402 v2 scheme and network spec, especially the verify/settle split and the payload schema for non-EVM chains.
- **Fix:**
  - Make facilitator mode, where a signed but unapplied order is settled by the server or facilitator, the conformant `exact` binding.
  - Register direct mode as a separate scheme name.
  - Pin the CAIP-2 id.

**P11 — x402 operational gaps**
- **Severity:** minor
- **Where:** R21.
- **What is wrong:** Several operational details are missing:
  - A "pinned" `fastpay_recovery_committee` root goes stale at epoch rotation, and there is no rotation procedure.
  - The spent-digest set must be shared across server replicas.
  - The `request_hash` sits inline and public on chain, which creates API usage linkability.
  - The server nonce implies server state.
- **Why it matters:** Each of these breaks or weakens real multi-instance deployments.
- **Investigate:** Review committee epoch semantics in the recovery spec.
- **Fix:** Specify committee root refresh, a shared spent store, and optionally a salted or blinded request hash.

**P12 — Task Node FastPay payout equivocation and liquidity risk**
- **Severity:** major
- **Where:** R15, R22.
- **What is wrong:** FastPay locks an owned object on conflicting orders. If multiple Task Node workers or instances sign from the same object, funds lock until recovery. Payout-wallet funding through the consensus FastLane deposit, required because `wrap_owned` is disabled, has no sizing or top-up policy.
- **Why it matters:** Default payouts could stall, and gate 3 would fail in production.
- **Investigate:** Check how Task Node schedules payouts and whether it is multi-instance, and test throughput under parallel payout load.
- **Fix:**
  - Require a single signer per owned object (object-to-worker partitioning).
  - Define object-splitting and top-up thresholds.
  - Add an acceptance criterion for N concurrent payouts with zero locks.

**P13 — Acceptance criteria are devnet-only and lack performance targets**
- **Severity:** major
- **Where:** Acceptance criteria.
- **What is wrong:** The readiness gates are about the *production* chain being performant, but every criterion runs on devnet. There is no throughput, latency SLO or failure-rate target for payouts or message rendering.
- **Why it matters:** The criteria cannot certify Outcome B.
- **Investigate:** Align with the README's readiness gates.
- **Fix:** Add testnet or mainnet criteria, for example "X payouts/min at p50 under 300 ms, under 0.1% fallback to `payment_v2`", plus a pin-resolution SLO.

**P14 — Sealed-mode discovery undefined**
- **Severity:** minor
- **Where:** R11.
- **What is wrong:** With only a CID and flags on chain and blinded readers, a recipient who is not the payment counterparty cannot find messages without fetching and trial-decrypting every envelope.
- **Why it matters:** Sealed is impractical beyond payouts.
- **Investigate:** Consider how many sealed messages would not be addressed to the payee.
- **Fix:** Restrict Sealed to the counterparty-is-reader case, or add an encrypted recipient tag scheme.

**P15 — Shielded payments excluded from the "private payments" gate**
- **Severity:** major (intent)
- **Where:** R12, Q12, gate 2.
- **What is wrong:** PFSH03 carries no memo, so private messaging cannot accompany shielded transfers. Deliverable 5 requires shielded transactions processed locally.
- **Why it matters:** The founder may consider gate 2 unmet if private payments mean shielded ones.
- **Investigate:** Coordinate with Pillar 5 and check the PFSH note format for extension room.
- **Fix:** Have the founder confirm gate 2's meaning. Recommend Q12 option 2 (memo in the note ciphertext) as the target and option 3 as the interim.

**P16 — Task Node as a default reader conflicts with the privacy narrative**
- **Severity:** minor
- **Where:** The encryption row, R8, R22.
- **What is wrong:** The Task Node service reads all user private payloads today. The page does not say whether v5 keeps it as a mandatory reader.
- **Why it matters:** It weakens the privacy claims and conflicts with the spirit of Deliverable 5.
- **Investigate:** Determine which Task Node functions actually need server-side decryption.
- **Fix:** Make the service a reader only where it is needed, and document this.

**P17 — Key-management details**
- **Severity:** minor
- **Where:** R9, R10, R24.
- **What is wrong:** Several details are missing:
  - The message key derives from the wallet seed, so there is no forward secrecy: a seed compromise exposes all history.
  - The ML-KEM derivation path and seed format (d, z) are unspecified.
  - Users who have lost their old-chain seed lose their v4 history.
  - The `payment_v2` "key behind from" mapping must handle key rotation and multisig for R19.
- **Investigate:** Review the wallet seed handling in `wallet_wasm`.
- **Fix:** Specify the derivation path and test vectors, document the security trade-off, and define the R19 key lookup at the transaction's ledger height.

**P18 — Header serialization form not pinned**
- **Severity:** minor
- **Where:** R4, R5 ("stored header bytes").
- **What is wrong:** To avoid re-serialization, the envelope must carry the header as an opaque `bytes` field. The page doesn't say so.
- **Why it matters:** Implementations that nest the header as a message will produce signatures that fail to verify across languages.
- **Fix:** Define `Envelope { bytes header = 1; repeated Reader readers = 2; bytes body = 3; bytes signature = 4; }` explicitly in R1.

**P19 — No internal sequencing or ownership**
- **Severity:** minor
- **Where:** The whole page.
- **What is wrong:** There are 25 requirements with no milestone order, owners or estimates. Several depend on other pillars or on L1 work: R17, PRIV-013/014 and the Pillar 6 `message_key`.
- **Why it matters:** The founder's framing is "what activities maximize success". Without ordering, the critical path is unclear.
- **Fix:** Add a phased plan:
  - **M1:** proto files, vectors, v5 pointer over classical crypto behind a flag.
  - **M2:** ML-KEM plus FastPay payouts.
  - **M3:** explorer.
  - **M4:** x402.

  Mark each with its blocking dependency.

**P20 — Revision notes add bulk without aiding the reader**
- **Severity:** minor
- **Where:** Revision notes.
- **What is wrong:** They are a long change log keyed to reviewer IDs.
- **Why it matters:** They add length without helping the founder decide anything.
- **Fix:** Move them to a changelog file, and keep the "Own corrections" items in the inventory.
