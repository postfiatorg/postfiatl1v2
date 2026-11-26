# Changelog — Pillar 2 — Message Protocol Spec

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/02-message-protocol.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/02-message-protocol.md).

| Item | Change |
|---|---|
| Gemini P1 | R7 KEM/HKDF/AEAD steps and associated data. R8 `key_id` and sender as reader. R9 key directory. R10 no keys on the explorer. Open question 1. |
| Gemini P2 | R13 async pinning, R3 size cap. Open question 4. |
| Gemini P3 | R4 binds chain and genesis. R19 *From* vs *Forwarded*. AC 4. |
| Gemini P4 | R2 field table, hex case, budget, format dispatch. R3 CIDv1 raw, `envelope_hash` dropped. R5 body encoding. Names unified. |
| Gemini P5 | R1 core split, R6 open ids, R18 generic renderer, R9 breaks the Pillar 6 cycle. The "60 schemas" figure was wrong: 6 lifecycle schemas, about 30 names. |
| Gemini P6 | R21 v2 headers, `exact`, both modes, replay and idempotency. Open question 8. |
| Gemini P7 | R22–R25. AC 6 and 9. |
| Gemini P8 | R17 (verified: FastPay is not in `account_tx`). R20. Open question 11. |
| Gemini P9 | R11 privacy levels. Partly disputed: `target` is an enum, not an address. The leaks are the ids, `kind` and v4's clear `content_hash`. |
| Gemini P10 | R16 Rust. |
| Gemini P11 | Verified: zero amounts are rejected on both lanes. Economics section. Open questions 2 and 3. |
| Opus P1, P2 | R2 table and budget (about 165/256 bytes). `envelope_hash` dropped. |
| Opus P3 | R5. |
| Opus P4 | R11 Sealed mode. |
| Opus P5 | R9. Open question 5. |
| Opus P6 | R24. AC 5. |
| Opus P7 | R13. Latency now cites 183 ms p50 from the index. |
| Opus P8 | Economics section. Open questions 2–4. |
| Opus P9, P10 | R21. Open question 8. FastPay is PFT-only (verified). |
| Opus P11 | R6. Open question 7. |
| Opus P12 | R22–R23, AC 6. Open question 10. |
| Opus P13 | R10. |
| Opus P14 | R2 defines the flags as advisory. |
| Opus P15 | R6. |
| Opus P16 | R3 `Manifest`. R8 sizes and reader limit. |
| Opus P17 | R7 makes post-quantum mandatory. Open question 1 narrowed to hybrid vs pure. |
| Opus P18 | R25, AC 9. |
| Opus P19 | Governance-packet worked example. |
| Opus P20 | R15. |
| Cross-pillar (Pillars 1, 4, 5, 6) | Field 4 is the schema id and `GOVERNANCE = 12`. SHA3-384 for chain-side commitments. Memo-carrying kinds listed (no NFT or PFSH03 memos). R10 wallet envelope signing. R9 uses the Pillar 6 `message_key`. R14 inline profile. Randomised ML-DSA noted. Open question 12. Chain names defined. |
| Own corrections | FastPay memos are not in history. The postfiat.org explorer is new-chain only and decodes no pointers. A mismatched `pf_ptr_v1.proto` exists. The IPFS cap is 16 MiB, not 1 MB. Task Node's upper-case hex is rejected by the L1. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/02-message-protocol.md).

| Item | Change |
|---|---|
| Gemini P1, Opus P1 | R4 carries header and readers as serialized bytes. R5 signs SHA3-384(header) ‖ readers hash ‖ SHA3-384(body). R7 body associated data adds the readers hash. Reader add/remove/reorder vectors (R16), AC 3. |
| Gemini P2 | R21 rebuilt: `exact` carries a signed, unapplied order that the server settles (who applies is now explicit); a compact `pf-fastpay-settled` scheme sends only the certificate digest. Header size stated (over 10 KB before base64). Stateless HMAC nonce with expiry. Votes checked against the certificate's own epoch. Sequence diagram redrawn. |
| Gemini P3, Opus P3 | Disputed after code check: limits are decoded bytes (`validate_optional_lower_hex_max` allows 2× hex characters), so 256 data bytes and the 165-byte pointer hold. The 512 total covers type, format and data of all memos, now stated. AC 1 tests against the real validator. |
| Gemini P3, Opus P4 | R2: `cid_bin` and `inline` are a `oneof`, exactly one set. Inline budget recomputed. |
| Gemini P4 | R7 specifies content key, HKDF input/salt/info, derived wrap nonce, wrap associated data, random `body_nonce` in the header. R11 defines the Sealed outer header and inner envelope. |
| Gemini P5, Opus P18 | R4 wire form `Envelope { bytes header; repeated bytes readers; bytes body; bytes signature }`. |
| Gemini P6 | Partly disputed: certified blocks do carry applied certificates with memos (`BlockRecord.fastpay_pre_state_effects`, confirmed fences keep the certificate). R17 is now firm: L1 `account_tx`/`tx` rows for sender and recipient, built from that source. AC 8. |
| Gemini P7 | R3 Manifest: chunk size, count, total, per-chunk nonce and associated data, `max_message_bytes` starting at Task Node's existing 16 MiB. R20 no auto-reassembly. AC 10. |
| Gemini P8, Opus P8 | Verified fee 0 accepted on FastPay; `payment_v2` full-memo cost computed (about 23 atoms, 1 PFT = 1,000,000 atoms). R27 governed FastPay fee rule; values are open question 3 (designed to work under any value, zero included). AC 9. |
| Gemini P9, Opus P12 | R22 payee key, leased object pool (row lock, as publication jobs already use), 64-effects-per-block throughput. R26 recipient wallet: balances, merge (2,048 inputs), signed unwrap (verified `owned_unwrap_*_v3`). R28 owned-object cap (100,000, verified). Float and sizing: open question 14. AC 6. |
| Gemini P10 | R13: payment finality vs availability, first-party gateway first, direct delivery checked against the CID, grace window with no negative caching. R20. |
| Gemini P11 | R24: decryption needs the imported old seed; Pillar 6 link is only for discovery. R10 import slot. |
| Gemini P12 | R1 package directories and CANONICAL_JSON for unconverted names; R2 flag rules; R19 adds `issued_payment_v2` and `address_from_public_key`. |
| Opus P2 | R11 Sealed: outer header hides schema, sender and time; sign-then-encrypt with `outer_readers_hash`; empty `key_id`, trial decryption, padding. |
| Opus P5 | R13 pins the exact block with `block/put` (raw codec) and replicates with Pinata `pinByHash`; CID mismatch is an error. |
| Opus P6 | Verified: `payment_v2` self-payments and FastPay self-transfers are accepted; issued self-payments are rejected. Stated in What exists and R12; open question 2 updated. |
| Opus P7 | R17 now one firm option with AC 8. |
| Opus P9 | Economics gives a storage estimate per envelope; R13 pluggable pinning policy; open question 4 (designed to work under each option). |
| Opus P10 | R21 `exact` follows the authorise-then-settle split; the settle-first mode is a separate scheme name. Open question 8 asks which to ship and the CAIP-2 id. |
| Opus P11 | R21 epoch-aware committee, shared spent store, stateless nonce, salted request hash. AC 7 across replicas and rotation. |
| Opus P13 | AC 11 testnet load run; targets are open question 14 (no thresholds invented). |
| Opus P14 | R11: sealed messages are found through the carrier or an out-of-band notice; no scanning in v1. |
| Opus P15 | Open question 12 now asks whether gate 2 covers shielded payments, decided with Pillar 5 open question 6. |
| Opus P16 | R11 reader policy: Task Node service only on schemas it processes; R6 registry lists default readers. |
| Opus P17 | R9 derivation path and no-forward-secrecy trade-off; R24 lost-seed path; R19 notes keys cannot rotate (address derives from the key). |
| Opus P19 | Build order table, Step 1/2/3 with blockers. |
| Opus P20 | Already done: revision notes live only in this changelog. |
| Opus (intent: CANONICAL_JSON) | Open question 13 (`body_encoding` works under either answer). |
| Opus (economics: x402 asset) | Economics and AC 7 measure both lanes for open question 8. |
| Own edits | Failure-modes table folded into R13, R21, R22 and R28. Explorer row trimmed. New code references: `transactions_validation_helpers.rs`, `entrypoints.rs`, `exchange-guide.md`, the anchoring status note. |
