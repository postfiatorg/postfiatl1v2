# Post Fiat Message Protocol v1 Specification (Contract C1)

Status: frozen Phase 0 wire contract (Contract C1)
Scope: Pillar 2 (`postfiat.message.v1`), Pillar 4 (`postfiat.nft.v1`), and Pillar 1 (`postfiat.governance.v1`)

This specification freezes the canonical protobuf3 schemas, wire framing,
cryptographic domain separation, and deterministic worked hex test vectors for
Post Fiat L1v2 messaging, post-quantum payload encryption, x402 agent payments,
signed NFT metadata & burn-first migration provenance, and Hive governance
packets.

All downstream SDKs (JavaScript/TypeScript, Python, Rust `prost`), Task Node,
wallets (`wallet_wasm`, `tasknode-apple`), x402 facilitators, and the block
explorer MUST conform to these exact wire rules and pass
[`scripts/test-message-protocol-vectors.py`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/test-message-protocol-vectors.py).

## 1. Canonical `.proto` Sources

| Package | File | Primary Schemas |
|---|---|---|
| `postfiat.message.v1` | [`proto/postfiat/message/v1/message.proto`](https://github.com/postfiatorg/postfiatl1v2/blob/main/proto/postfiat/message/v1/message.proto) | `Pointer` (`pf.ptr` v5), `LegacyPointerV4`, `Envelope` (`pf.message.v1` / `pf.msg.v1`), `MessageHeader`, `XWingEncryptionHeader`, `RecipientWrap`, `MessageKeyBinding`, `FastPayPaymentRef`, `X402PaymentRequired`, `X402PaymentSignature`, `X402PaymentResponse`, `Manifest`, `TextMessage`, `RewardNotice` |
| `postfiat.nft.v1` | [`proto/postfiat/nft/v1/metadata.proto`](https://github.com/postfiatorg/postfiatl1v2/blob/main/proto/postfiat/nft/v1/metadata.proto) | `NftMetadata` (`pf.nft.metadata.v1`), `OriginProvenance`, `MigrationClaim` (`pf.nft.claim.v1`), `PinOnlyProvenance` (`pf.nft.provenance.v1`) |
| `postfiat.governance.v1` | [`proto/postfiat/governance/v1/packet.proto`](https://github.com/postfiatorg/postfiatl1v2/blob/main/proto/postfiat/governance/v1/packet.proto) | `ProtocolSnapshot` (`pf.governance.snapshot.v1`), `GovernancePacket` (`pf.governance.packet.v1`), `SessionTranscript` (`pf.governance.transcript.v1`), `GovernanceMinutes` (`pf.governance.minutes.v1`), `RulingAnchor` (`pf.governance.ruling.v1`) |

### Schema Registry Identifiers

Core schemas carry both a canonical `schema_name` string and a numeric
`schema_id`. Permissionless third-party schemas MUST set `schema = 1`
(`schema_id = 1`) and populate a reverse-DNS `schema_name` (e.g.,
`org.example.custom.packet.v1`).

| `schema_id` | `schema_name` | Default `ContentKind` |
|---:|---|---|
| `1` | *(permissionless custom `schema_name`)* | caller-selected |
| `100` | `pf.message.text.v1` | `CONTENT_KIND_CHAT (4)` |
| `101` | `pf.governance.packet.v1` | `CONTENT_KIND_GOVERNANCE (12)` |
| `102` | `pf.governance.snapshot.v1` | `CONTENT_KIND_GOVERNANCE (12)` |
| `103` | `pf.governance.transcript.v1` | `CONTENT_KIND_GOVERNANCE (12)` |
| `104` | `pf.governance.minutes.v1` | `CONTENT_KIND_GOVERNANCE (12)` |
| `105` | `pf.governance.ruling.v1` | `CONTENT_KIND_GOVERNANCE (12)` |
| `201` | `pf.reward.v1` | `CONTENT_KIND_REWARD (6)` |
| `400` | `pf.nft.metadata.v1` | `CONTENT_KIND_NFT (13)` |
| `401` | `pf.nft.claim.v1` | `CONTENT_KIND_NFT (13)` |
| `402` | `pf.x402.request.v1` | `CONTENT_KIND_X402 (14)` |
| `403` | `pf.nft.provenance.v1` | `CONTENT_KIND_NFT (13)` |

## 2. Canonical Serialization & Zero Re-Serialization Rules

### 2.1 Deterministic Protobuf3 Producer Rules

When producing new messages, encoders MUST emit deterministic protobuf3 bytes:

1. **Ascending field numbers:** Fields are written in strictly ascending field
   number order (`1, 2, 3, ...`), preserving element order for `repeated` fields.
2. **Omit default scalar values:** Zero integers, `false` booleans, empty
   strings, empty `bytes`, and `0` enum values are omitted on the wire.
3. **Minimal varints:** Unsigned varints use minimal LEB128 encoding with no
   redundant leading zero groups.
4. **Canonical `map<string, string>` ordering:** Map entries are emitted in
   lexicographic order of UTF-8 key bytes.
5. **No unknown fields:** Producers emit no unknown fields.

### 2.2 Zero Re-Serialization Rule for Verifiers

Cross-language protobuf libraries can differ on edge cases if re-serialized.
Therefore `postfiat.message.v1.Envelope` wraps `header_bytes`,
`encryption_header_bytes`, and `body_bytes` as opaque `bytes` fields:

- Verifiers MUST first check the IPFS CIDv1 `raw` SHA-256 digest over the exact
  received `Envelope` bytes.
- Verifiers MUST verify the ML-DSA-65 signature over the exact raw byte fields
  `header_bytes`, `encryption_header_bytes`, and `body_bytes` BEFORE decoding
  any nested protobuf or JSON message.
- Verifiers MUST NEVER re-serialize a decoded protobuf message in order to
  recompute a signature preimage, CID, or on-chain `metadata_hash`.

### 2.3 Canonical JSON (RFC 8785 / JCS)

When `MessageHeader.body_encoding == BODY_ENCODING_CANONICAL_JSON` (used by
Pillar 1 governance packets and optional human-readable artifacts), producers
serialize UTF-8 JSON per RFC 8785 (keys sorted recursively, no insignificant
whitespace, deterministic number formatting). As with protobuf bodies,
signatures and CIDs cover the exact transmitted `body_bytes`.

## 3. On-Chain `pf.ptr` v5 Memo Wire Rules

`postfiatl1v2` supports memos on `payment_v2`, `issued_payment_v2`, and FastPay
(`OwnedTransferOrderV3`) with consensus-enforced `PaymentMemo` limits (see
`crates/types/src/core_chain.rs`):

- `MAX_PAYMENT_MEMOS = 4`
- `MAX_PAYMENT_MEMO_TYPE_BYTES = 64`
- `MAX_PAYMENT_MEMO_FORMAT_BYTES = 64`
- `MAX_PAYMENT_MEMO_DATA_BYTES = 256`
- `MAX_PAYMENT_MEMO_TOTAL_BYTES = 512`
- **Lowercase hex only:** `memo_type`, `memo_format`, and `memo_data` MUST be
  even-length lowercase ASCII hex (`[0-9a-f]*`). Uppercase hex is rejected by
  `postfiatl1v2`.

A `pf.ptr` v5 memo encodes as:

| `PaymentMemo` field | Logical value | Exact lowercase hex |
|---|---|---|
| `memo_type` | `"pf.ptr"` | `70662e707472` (6 bytes) |
| `memo_format` | `"v5"` | `7635` (2 bytes) |
| `memo_data` | `postfiat.message.v1.Pointer` protobuf bytes | lowercase hex, decoded length `<= 256` bytes |

Pure messages without third-party value transfer use a **FastPay self-transfer**
or a **`payment_v2` self-payment** (no new L1 transaction type is required).

### 3.1 IPFS CIDv1 `raw` Binary Format

Every single-block `Envelope` (up to **1 MiB = 1,048,576 bytes**) is stored as
an IPFS **CIDv1 `raw`** block (codec `0x55`, multihash `sha2-256` `0x12 0x20`):

```text
cid_bin (36 bytes) = 0x01 || 0x55 || 0x12 || 0x20 || SHA-256(raw_block_bytes)
multibase_cid      = "b" || base32lower_nopad(cid_bin)
```

Clients compute `cid_bin` offline before pinning to IPFS so FastPay payouts and
on-chain anchors never block on IPFS network I/O.

### 3.2 Mutually Exclusive Visibility Flags

`Pointer.flags` is a bitmask of `PointerFlag`:

| Bit | Name | Role |
|---:|---|---|
| `0x01` | `POINTER_FLAG_PRIVATE` | Encrypted body; explorer may show verified badge/metadata |
| `0x02` | `POINTER_FLAG_PUBLIC` | Plaintext public body |
| `0x04` | `POINTER_FLAG_EPHEMERAL` | Ephemeral modifier |
| `0x08` | `POINTER_FLAG_TOMBSTONE` | Tombstone modifier |
| `0x10` | `POINTER_FLAG_MULTIPART` | Multi-chunk `Manifest` modifier |
| `0x20` | `POINTER_FLAG_SEALED` | Encrypted body with recipient-anonymous header |

Validators and decoders MUST enforce that **exactly one** visibility bit from
`{0x01, 0x02, 0x20}` is set and no unsupported bits outside `0x3f` are set.
`Pointer.target == POINTER_TARGET_INLINE (3)` MUST set `inline_payload` and
leave `cid_bin` empty; `CONTENT_BLOB (1)` and `MANIFEST (2)` MUST set a 36-byte
`cid_bin` and leave `inline_payload` empty.

### 3.3 Legacy `pf.ptr` v4 Read-Only Compatibility

Wallets and indexers decode historical old-chain memos (`memo_format = "v4"`,
uppercase hex on the legacy XRPL fork) using `postfiat.message.v1.LegacyPointerV4`.
`ContentKind` values `0..11` and `99` preserve exact v4 numbering, with new v5
kinds starting at `CONTENT_KIND_GOVERNANCE = 12`, `CONTENT_KIND_NFT = 13`, and
`CONTENT_KIND_X402 = 14`.

## 4. `pf.message.v1` Envelope Signing & Post-Quantum Encryption

### 4.1 Raw-Byte ML-DSA-65 Signing Preimage

`Envelope.signature` is an ML-DSA-65 (FIPS 204) signature produced by the
sender's `postfiatl1v2` account key over the exact length-prefixed raw bytes:

```text
signing_preimage =
    "postfiat-l1-v2/message-envelope/v1\x00"
 || u32_be(version)
 || u32_be(len(header_bytes))            || header_bytes
 || u32_be(len(encryption_header_bytes)) || encryption_header_bytes
 || u32_be(len(body_bytes))              || body_bytes
```

Normative production cryptographic sizes:

| Artifact | Algorithm | Exact byte length |
|---|---|---:|
| `Envelope.sender_pubkey` | ML-DSA-65 public key | `1952` |
| `Envelope.signature` | ML-DSA-65 signature | `3309` |
| `MessageKeyBinding.xwing_public_key` | X-Wing (ML-KEM-768 `1184` \|\| X25519 `32`) | `1216` |
| `RecipientWrap.xwing_ciphertext` | X-Wing (ML-KEM-768 `1088` \|\| X25519 `32`) | `1120` |
| `XWingEncryptionHeader.aead_nonce` / `wrap_nonce` | XChaCha20-Poly1305 nonce | `24` |
| `RecipientWrap.wrapped_cek` | 32B CEK + 16B Poly1305 tag | `48` |

Explorer display rules:
- Show **Verified sender** ONLY when `Envelope.signature` is valid AND the
  account derived from `Envelope.sender_pubkey` matches the on-chain transaction
  signer.
- Show the **`"Task reward"` badge** ONLY when the verified `sender_pubkey`
  matches the pinned official Task Node payout key (`pf.reward.v1`,
  `VISIBILITY_PRIVATE`). Unrecognized senders claiming `pf.reward.v1` render as
  generic *Private message*.

### 4.2 X-Wing + HKDF-SHA256 + XChaCha20-Poly1305

For `VISIBILITY_PRIVATE` and `VISIBILITY_SEALED` messages:

1. Generate a fresh random 32-byte Content Encryption Key (`CEK`).
2. Compute the 32-byte domain-separated AAD commitment:
   ```text
   aad_context = SHA-256(
       "postfiat-l1-v2/message-aad/v1\x00"
    || chain_id_utf8 || "\x00"
    || SHA-256(header_bytes)
    || u32_be(chunk_index)
   )
   ```
3. Encrypt the body bytes using `XChaCha20-Poly1305(key=CEK, nonce=aead_nonce,
   aad=aad_context, plaintext=body_plaintext)`. **No plaintext digest is ever
   published in the header or envelope.**
4. For each authorized recipient (sorted lexicographically by `recipient_key_id`):
   - Compute `recipient_key_id = SHA-256(xwing_public_key)` (32 bytes).
   - Run X-Wing encapsulation `(xwing_ciphertext, shared_secret) = XWing.Encap(xwing_public_key)`.
   - Derive the 32-byte wrap key via RFC 5869 HKDF-SHA256:
     ```text
     KEK = HKDF-SHA256(
         ikm  = shared_secret,
         salt = aad_context,
         info = "postfiat-l1-v2/xwing-wrap/v1\x00" || recipient_key_id,
         L    = 32
     )
     ```
   - Wrap the CEK: `wrapped_cek = XChaCha20-Poly1305(key=KEK, nonce=wrap_nonce, aad=aad_context, plaintext=CEK)`.
   - In `VISIBILITY_SEALED` mode, `RecipientWrap.recipient_account` MUST be empty.
5. **Strict reader policy:** Task Node is included as a reader ONLY on Task Node
   lifecycle/reward schemas that the server must process — NEVER on
   user-to-user messages or shielded (`PFSH03`) payments.

### 4.3 Zero-Balance Wallet KEM Key Bootstrap

Wallets deterministically derive their 32-byte X-Wing key-generation seed from
master wallet entropy via HKDF-SHA256:

```text
kem_seed = HKDF-SHA256(
    ikm  = wallet_master_entropy,
    salt = chain_id_utf8,
    info = "postfiat-l1-v2/message-key/v1",
    L    = 32
)
```

Before a new zero-balance user mints a Pillar 6 on-chain identity, their wallet
signs a `MessageKeyBinding` with its ML-DSA-65 account key over:

```text
key_bind_preimage =
    "postfiat-l1-v2/message-key-bind/v1\x00"
 || u32_be(len(chain_id)) || chain_id
 || u32_be(len(account))  || account
 || xwing_public_key
 || key_id
 || u64_be(created_at_unix_ms)
 || u32_be(key_epoch)
```

Once a Pillar 6 on-chain identity is minted, its on-chain `message_key` becomes
authoritative and clients cross-check it.

### 4.4 Multi-Chunk `Manifest` (Up to 16 MiB)

Payloads exceeding `1,048,576` bytes (up to `16,777,216` bytes) split into
ordered chunks `<= 1 MiB` each. Each chunk uses a distinct 24-byte `aead_nonce`
and binds its strictly ascending 0-based `chunk_index` into `aad_context` to
prevent chunk reordering or cross-message chunk substitution. The `Manifest` is
stored inside a signed `Envelope` referenced by a `Pointer` with
`target == POINTER_TARGET_MANIFEST (2)` and `POINTER_FLAG_MULTIPART (0x10)`.

## 5. FastPay References & x402 v2 Agent Payment Headers

### 5.1 Immutable Deduplication Keys (`order_hash` / `tx_hash`)

FastPay `account_tx` history indexing and x402 spent-payment deduplication
across replicas MUST key on:

- **`order_hash`** (48-byte immutable SHA3-384 digest of the signed
  `OwnedTransferOrderV3`, computed by `fastpay_transfer_order_digest_v3`) for
  `PAYMENT_LANE_FASTPAY`, NEVER `certificate_digest`
  (which varies with validator signature arrival order).
- **`tx_hash`** (48-byte SHA3-384 transaction id, computed by
  `payment_v2_tx_id` or `asset_transaction_tx_id`) for
  `PAYMENT_LANE_CONSENSUS_PFT` (`payment_v2`) and
  `PAYMENT_LANE_CONSENSUS_ISSUED` (`issued_payment_v2`, e.g. PFUSDC).

`order_hash`, `tx_hash`, and the optional `certificate_digest` (computed by
`fastpay_transfer_certificate_digest_v3`) are all the chain hash
(`hash_bytes` in `crates/crypto_provider/src/lib.rs`, SHA3-384). They are
carried as the raw 48 bytes of the 96-lowercase-hex strings the L1 RPC returns.

### 5.2 x402 v2 HTTP Header Transport

HTTP headers carry base64url-encoded (no padding) canonical protobuf bytes (or
canonical JSON projection with identical field names):

1. Server `402 Payment Required` -> `PAYMENT-REQUIRED`: `X402PaymentRequired`
   (`x402_version = 2`, `scheme = exact | pf-fastpay-settled`, `request_hash`,
   `nonce`, `pay_to`, `max_amount_atoms`, `expires_at_unix_ms`).
2. Client retry -> `PAYMENT-SIGNATURE`: `X402PaymentSignature` carrying the
   signed FastPay transfer (with `request_hash` anchored in its `pf.ptr` v5
   `INLINE` memo) or consensus-lane `issued_payment_v2`, plus immutable
   `order_hash` / `tx_hash`.
3. Server `200 OK` -> `PAYMENT-RESPONSE`: `X402PaymentResponse` carrying
   `settled = true` and `FastPayPaymentRef`.

## 6. Pillar 4 NFT Metadata & Burn-First Migration Rules

### 6.1 One Byte Stream, One Hash (`pf.nft.metadata.v1`)

1. Serialize `postfiat.nft.v1.NftMetadata` and wrap it in a `VISIBILITY_PUBLIC`
   Pillar 2 `Envelope` signed by the on-chain `nft.issuer` ML-DSA-65 key.
2. Pin the raw `Envelope` bytes to IPFS as a CIDv1 `raw` block.
3. On-chain `nft_mint` records:
   - `metadata_uri = "ipfs://" + multibase_cid` (`<= 256` bytes)
   - `metadata_hash = lowercase_hex(SHA3-384(raw_envelope_bytes))` (96 lowercase
     hex chars = 48 bytes, within `MAX_NFT_METADATA_HASH_BYTES = 64`).
4. Verifiers MUST check ALL three invariants:
   - `SHA3-384(raw_envelope_bytes) == nft.metadata_hash`
   - `account(Envelope.sender_pubkey) == nft.issuer`
   - `Envelope.signature` verifies under ML-DSA-65 over the raw-byte preimage.

### 6.2 Burn-First Migration Claim & Partitioned Serials

Before prompting any irreversible old-chain `NFTokenBurn`:

1. Claimant signs `MigrationClaim` (`pf.nft.claim.v1`) inside a PUBLIC `Envelope`
   with their new `postfiatl1v2` ML-DSA-65 key.
2. Migration service durably persists and pins the signed claim envelope to IPFS,
   yielding `claim_cid` (stored in `OriginProvenance.claim_cid` for offline
   verification) and `claim_hash = SHA-256(raw_claim_envelope_bytes)` (placed in
   the old-chain `NFTokenBurn` memo).
3. Serial numbers partition cleanly between fresh native mints (`bit 63 = 0`)
   and deterministic migrated mints (`bit 63 = 1`):
   ```text
   digest          = SHA3-384("postfiat.nft.migrated_serial.v1\x00"
                              || source_chain_utf8 || "\x00"
                              || uppercase(nftoken_id))
   migrated_serial = (1 << 63) | (u64_be(digest[0..8]) & 0x7fff_ffff_ffff_ffff)
   ```
4. Optional `RETIREMENT_MODE_PIN_ONLY` uses `PinOnlyProvenance`
   (`pf.nft.provenance.v1`) anchored via a Pillar 2 self-payment and MUST render
   strictly as *Recorded Provenance*, never as a migrated live NFT.

## 7. Pillar 1 Hive Governance Packet & Anchor Rules

1. **Authority binding:** `AuthorityBinding` pins `authorized_accounts` (1-of-1
   founder at launch; `signer_set_id` and `threshold` support future multi-sig
   stewards without schema changes) and embeds `founder_nostr_pubkey`. Explorers
   MUST render `CONTENT_KIND_GOVERNANCE (12)` timelines ONLY when anchored by a
   pinned authorized account.
2. **Single-block snapshot:** Every `SnapshotSourceRecord` inside
   `ProtocolSnapshot` (`pf.governance.snapshot.v1`) pins L1 reads to one
   `finalized_block_height` and stores `raw_response_sha256` + `raw_response_cid`.
3. **Carry-over accountability:** Every `GovernancePacket`
   (`pf.governance.packet.v1`) explicitly scores every deliverable carried over
   from the prior session (`DONE`, `IN_PROGRESS`, `NOT_DONE`, or `DROPPED`) —
   with zero automatic PFT bounties.
4. **Canonical transcript root:** `SessionTranscript` orders included events
   strictly by server `receipt_seq` (preventing client timestamp backdating) and
   commits to:
   ```text
   transcript_root_sha256 = SHA-256(
       "postfiat.governance.transcript_root.v1\x00"
    || concat_i(u64_be(receipt_seq_i) || bytes32(nostr_event_id_i) || raw_event_sha256_i)
   )
   ```
5. **Two-pass prompt-injection isolation:** `GovernanceMinutes` separates Pass 1
   (`previous_deliverables_review`, `decisions`, `next_deliverables`, citing
   only founder/agenda events) from Pass 2 (`community_input`, quoted data).
6. **Custody-safe deterministic rulings:** `RulingAnchor`
   (`pf.governance.ruling.v1`) is emitted ONLY when `run_deterministic_ruling`
   is explicitly flagged by the founder. The Google/Vast runner is read-only
   compute and never holds a founder signing key; the founder signs
   `approvals[]` on review.
7. **On-chain anchoring:** Governance artifacts are wrapped in PUBLIC Pillar 2
   `Envelope`s (`body_encoding = BODY_ENCODING_CANONICAL_JSON` or `PROTOBUF`)
   and anchored on `postfiatl1v2` via `payment_v2` `pf.ptr` v5 memos with
   `kind = CONTENT_KIND_GOVERNANCE (12)` and `thread_id = session_id`.

## 8. Deterministic Worked Hex Test Vectors

The JSON block below is the machine-verified single source of truth checked in
CI by `python3 scripts/test-message-protocol-vectors.py` (including `protoc`
decode/re-encode byte-identity round-trip checks when `protoc` is present).

Fixture note: To keep the published wire vectors concise and human-inspectable,
the `Envelope` structural vectors (`V4`, `V5`, `V8`, `V9`) use deterministic
32-byte placeholder `sender_pubkey` (`a1 * 32`), `signature` (`b2 * 32`), and
compact `xwing_ciphertext` (`44 * 32`). Full normative production lengths
(1952B ML-DSA-65 pubkey, 3309B ML-DSA-65 signature, 1216B X-Wing pubkey,
1120B X-Wing ciphertext) are additionally verified in
`scripts/test-message-protocol-vectors.py`.

<!-- BEGIN_CANONICAL_TEST_VECTORS_JSON -->
```json
{
  "V1_POINTER_V5_PUBLIC_TEXT": {
    "proto_type": "postfiat.message.v1.Pointer",
    "memo_type_hex": "70662e707472",
    "memo_format_hex": "7635",
    "memo_data_hex": "0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d100118042064320d7468726561642d63312d3030314002",
    "byte_len": 61,
    "cid_bin_hex": "015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d",
    "cid_multibase": "bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu"
  },
  "V2A_POINTER_V5_WORST_CASE_BUDGET": {
    "proto_type": "postfiat.message.v1.Pointer",
    "memo_data_hex": "0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d1002180c20012a287461736b2d323032362d31312d32372d3030303030303030303030303030303030303030303030313228676f762d73657373696f6e2d323032362d31312d32372d30303030303030303030303030303030313a286374782d736e617073686f742d323032362d31312d32372d303030303030303030303030303030314012521c6f72672e6578616d706c652e637573746f6d2e7061636b65742e7631",
    "byte_len": 202
  },
  "V2B_POINTER_V5_INLINE_X402": {
    "proto_type": "postfiat.message.v1.Pointer",
    "request_hash_hex": "ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56",
    "memo_data_hex": "1003180e20920340064a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56",
    "byte_len": 43
  },
  "V3_LEGACY_POINTER_V4_DECODE": {
    "proto_type": "postfiat.message.v1.LegacyPointerV4",
    "memo_type_hex": "70662E707472",
    "memo_format_hex": "7634",
    "memo_data_hex_upper": "0A3B6261666B726569676832616B69736361696C646371616273796733646672366368753366677072656769796D73636B376537617161347335327A791001180620012A107461736B2D6C65676163792D303034324001",
    "byte_len": 87
  },
  "V4_ENVELOPE_PUBLIC_TEXT": {
    "proto_type": "postfiat.message.v1.Envelope",
    "header_hex": "08011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d303031",
    "body_hex": "0a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e",
    "signing_preimage_sha256_hex": "e9d50d27fa79f160fda793f8133fc9ce19f5a507ed539e2a4bbea19d67c53ba7",
    "envelope_hex": "0801127508011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d30303122230a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e28013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    "cid_bin_hex": "015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d",
    "cid_multibase": "bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu"
  },
  "V5_ENVELOPE_PRIVATE_XWING_REWARD": {
    "proto_type": "postfiat.message.v1.Envelope",
    "order_hash_hex": "36480e7b3e38fec90ea5af2b15296f17da4d38b0077e4a67328a9a5ff655e8490720a9c51be74ed2e609d6c385ffa4af",
    "header_hex": "08011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317adf0108011211706f7374666961742d6465766e65742d321a3036480e7b3e38fec90ea5af2b15296f17da4d38b0077e4a67328a9a5ff655e8490720a9c51be74ed2e609d6c385ffa4af2230de06e9927f18fb6fe9f75ffd54b23cacd56d28635a61af2d85dbbc51e6fcf69510eab54bbb147ad64d87a878c8ba15df32035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee01508208",
    "aad_context_hex": "b57d20818e367f1b68e8707871f1c0c880305f1a2314e91e89a5b76322f8b140",
    "derived_kem_seed_hex": "430e4662271619e3e43878bddafd267013540a0e72aec34a4eeb1225af12199a",
    "recipient_key_id_hex": "9f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4",
    "derived_wrap_kek_hex": "45276bc1f6737eded01434e1d9ab971820316dca764ac588b37c22e27061ca0a",
    "key_binding_preimage_sha256_hex": "6c4a3977bfc8b2f67e028c5b648d375fe21fe2dda86832e83ae6e5d0545560e4",
    "encryption_header_hex": "080212187777777777777777777777777777777777777777777777771a20b57d20818e367f1b68e8707871f1c0c880305f1a2314e91e89a5b76322f8b1402a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a30666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666",
    "signing_preimage_sha256_hex": "c95c8e29207b30d5a11fbe3881f0c86e011aef71a225ca751c3956f3a97a68f5",
    "envelope_hex": "080112d50208011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317adf0108011211706f7374666961742d6465766e65742d321a3036480e7b3e38fec90ea5af2b15296f17da4d38b0077e4a67328a9a5ff655e8490720a9c51be74ed2e609d6c385ffa4af2230de06e9927f18fb6fe9f75ffd54b23cacd56d28635a61af2d85dbbc51e6fcf69510eab54bbb147ad64d87a878c8ba15df32035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee015082081a9b02080212187777777777777777777777777777777777777777777777771a20b57d20818e367f1b68e8707871f1c0c880305f1a2314e91e89a5b76322f8b1402a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a3066666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666622408888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888828013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    "cid_bin_hex": "015512203bf9c430b3423d7503d30da0284e874248c2b6134e67834d14f8c4647551840f",
    "cid_multibase": "bafkreib37hcdbm2chv2qhuynuaue5b2cjdblme2om6bu2fhyyrshkumeb4"
  },
  "V6_MANIFEST_MULTICHUNK": {
    "proto_type": "postfiat.message.v1.Manifest",
    "manifest_hex": "0801108080601880804022186170706c69636174696f6e2f6f637465742d73747265616d2a1570662e646f63756d656e742e62756e646c652e76313004380242830112670a24015512200147e682a1ce4cf491afa19c10b08a40f6559f657576398a7788c37c60fcf88a109080401a3b6261666b72656961626937746966696f6f6a74326a646c35627471696c62637361367a6b7a367a6c766f79347975353469796e36676237687972691a18909090909090909090909090909090909090909090909090428501080112670a24015512205b914ae0bd580f1a94824cfc2cff2cf61c2988f46c36e00aa87c5e46ecb584c2109080201a3b6261666b72656963337366666f62706b7962346e6a6a61736d37717770366c6877647175797235646d67337161766b64346c7a646f7a6e6d6579691a18919191919191919191919191919191919191919191919191",
    "byte_len": 333
  },
  "V7_X402_HEADERS": {
    "required_proto_type": "postfiat.message.v1.X402PaymentRequired",
    "required_hex": "080210011a11706f7374666961743a6465766e65742d3220012a0350465430a08d063a2a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304220ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb564a10c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c150e0cf8295ac335a2568747470733a2f2f6170692e706f7374666961742e6f72672f76312f696e666572656e6365621968747470733a2f2f783430322e706f7374666961742e6f7267",
    "signature_proto_type": "postfiat.message.v1.X402PaymentSignature",
    "signature_hex": "080210011a11706f7374666961743a6465766e65742d3220012a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb563210c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c13a30dbaa9afe5a060e7b3d3fa0a5d59bccdf2922f1bc179335d1a97884b99a9570ca258e7a6bfd8bef7c36f5e269a897bdac4a20d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2",
    "response_proto_type": "postfiat.message.v1.X402PaymentResponse",
    "response_hex": "080210011adc0108011211706f7374666961742d6465766e65742d321a30dbaa9afe5a060e7b3d3fa0a5d59bccdf2922f1bc179335d1a97884b99a9570ca258e7a6bfd8bef7c36f5e269a897bdac22304916711407090bb42c52ef59dac8aa31a97554a224b5e9283196f50ccc4c6451d7e80713ecec9d05515d0086f732f19932035046543a2a70663173656e646572303030303030303030303030303030303030303030303030303030303030303030422a70663172656369703030303030303030303030303030303030303030303030303030303030303030303048a08d06501a20faa9f094ac332a17706f7374666961742d666163696c697461746f722d7631",
    "dedup_order_hash_hex": "dbaa9afe5a060e7b3d3fa0a5d59bccdf2922f1bc179335d1a97884b99a9570ca258e7a6bfd8bef7c36f5e269a897bdac"
  },
  "V8_NFT_METADATA_AND_MIGRATION": {
    "claim_proto_type": "postfiat.nft.v1.MigrationClaim",
    "claim_hex": "08011211706f7374666961742d6465766e65742d321a14706f7374666961742d6c65676163792d7872706c2240303030383030303036323033463439433231443544364530323243423136444533353338463234383636324643373343323941424136413930303030303030312a22724f6c644f776e65724c656761637941646472657373303030303030303030303031322a7066317265636970303030303030303030303030303030303030303030303030303030303030303030303a135441534b4e4f44452d50524f46494c452d5631422a7066316e66746d696772303030303030303030303030303030303030303030303030303030303030303048baaffbe095f5ec9fe40150015887ad4b60a0b5f694ac336a650a24015512208e55e24fdf10441efc4371a93f29bf99eb0210d3be95719c3afd36c4dd47305210251a3b6261666b726569656f6b787265377879716971707079713372766537737470347a356d6262627535367376797a796f78356733636e32727a716b69",
    "claim_envelope_cid_multibase": "bafkreid72tro6pxpyto6x6oeqdnrwortn2gqhszmn3odovh7d4y6zv7oma",
    "claim_hash_hex": "7fd4e2ef3eefc4ddebf9c480db1b3a336e8d03cb2c6edc3754ff1f31ecd7ee60",
    "migrated_serial_uint64": 16447061904156710842,
    "metadata_proto_type": "postfiat.nft.v1.NftMetadata",
    "metadata_hex": "08011214506f737420466961742050726f66696c652023311a214d6967726174656420706f73742d7175616e74756d2070726f66696c65204e46542242697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a72712a650a2401551220e4a307eb3fa590779ab780a48e91a7797d3120656472d60bfadbd4f6ad7d598c101c1a3b6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727132f2010a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727112650a2401551220e4a307eb3fa590779ab780a48e91a7797d3120656472d60bfadbd4f6ad7d598c101c1a3b6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a72711a09696d6167652f706e672001288008308008381c42301cf2f78f77f73b57a872a81634b406a1e4e23f394ba921eac5f43613d44e9f8815ff1ab572a753c30c9d7ce00a26354b3a170a0474696572120747656e657369731a06737472696e6742590a135441534b4e4f44452d50524f46494c452d5631122a7066316e66746d69677230303030303030303030303030303030303030303030303030303030303030301a16506f737420466961742050726f66696c65204e4654734a7a0a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727110011a301cf2f78f77f73b57a872a81634b406a1e4e23f394ba921eac5f43613d44e9f8815ff1ab572a753c30c9d7ce00a26354b201c5230158d8c492d5b612b047ec92cc7425d447b49f14541a81c83967c42c6515e4ae764b1cf63affa4777059ffad5917b88935a0943432d42592d342e30621a68747470733a2f2f706f7374666961742e6f72672f6e66742f316aee040a14706f7374666961742d6c65676163792d7872706c1240303030383030303036323033463439433231443544364530323243423136444533353338463234383636324643373343323941424136413930303030303030311a22724f6c644973737565724c65676163794164647265737330303030303030303030312222724f6c644f776e65724c6567616379416464726573733030303030303030303030312a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727132404142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414238e0914340014a404344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434450a8ad4b5a660a24015512207fd4e2ef3eefc4ddebf9c480db1b3a336e8d03cb2c6edc3754ff1f31ecd7ee6010ae041a3b6261666b72656964373274726f3670787079746f3678366f6571646e72776f72746e32677168737a6d6e336f646f766837643479367a76376f6d6162207fd4e2ef3eefc4ddebf9c480db1b3a336e8d03cb2c6edc3754ff1f31ecd7ee606a650a240155122067cca5fef9729b0de598e232e3bd79771b21caa9d8ebbbfd7d5c5598b7055acf10231a3b6261666b72656964687a73733735366c73746d67366c676863676c723332366c78646d7134766b6f79356f353732376b346b776d6c6f626b327a3470baaffbe095f5ec9fe401721b0a0e6d6967726174696f6e5f776176651209666f756e6465722d31",
    "metadata_envelope_hex": "0801126708011211706f7374666961742d6465766e65742d321a1270662e6e66742e6d657461646174612e7631209003280d3001380140014a2a7066316e66746d696772303030303030303030303030303030303030303030303030303030303030303050b083f794ac3322b20b08011214506f737420466961742050726f66696c652023311a214d6967726174656420706f73742d7175616e74756d2070726f66696c65204e46542242697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a72712a650a2401551220e4a307eb3fa590779ab780a48e91a7797d3120656472d60bfadbd4f6ad7d598c101c1a3b6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727132f2010a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727112650a2401551220e4a307eb3fa590779ab780a48e91a7797d3120656472d60bfadbd4f6ad7d598c101c1a3b6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a72711a09696d6167652f706e672001288008308008381c42301cf2f78f77f73b57a872a81634b406a1e4e23f394ba921eac5f43613d44e9f8815ff1ab572a753c30c9d7ce00a26354b3a170a0474696572120747656e657369731a06737472696e6742590a135441534b4e4f44452d50524f46494c452d5631122a7066316e66746d69677230303030303030303030303030303030303030303030303030303030303030301a16506f737420466961742050726f66696c65204e4654734a7a0a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727110011a301cf2f78f77f73b57a872a81634b406a1e4e23f394ba921eac5f43613d44e9f8815ff1ab572a753c30c9d7ce00a26354b201c5230158d8c492d5b612b047ec92cc7425d447b49f14541a81c83967c42c6515e4ae764b1cf63affa4777059ffad5917b88935a0943432d42592d342e30621a68747470733a2f2f706f7374666961742e6f72672f6e66742f316aee040a14706f7374666961742d6c65676163792d7872706c1240303030383030303036323033463439433231443544364530323243423136444533353338463234383636324643373343323941424136413930303030303030311a22724f6c644973737565724c65676163794164647265737330303030303030303030312222724f6c644f776e65724c6567616379416464726573733030303030303030303030312a42697066733a2f2f6261666b7265696865756d6436777035667362337a766e34617573686a646a337a70757973617a6c656f6c6c61783677333274336b32376b7a727132404142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414241424142414238e0914340014a404344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434443444344434450a8ad4b5a660a24015512207fd4e2ef3eefc4ddebf9c480db1b3a336e8d03cb2c6edc3754ff1f31ecd7ee6010ae041a3b6261666b72656964373274726f3670787079746f3678366f6571646e72776f72746e32677168737a6d6e336f646f766837643479367a76376f6d6162207fd4e2ef3eefc4ddebf9c480db1b3a336e8d03cb2c6edc3754ff1f31ecd7ee606a650a240155122067cca5fef9729b0de598e232e3bd79771b21caa9d8ebbbfd7d5c5598b7055acf10231a3b6261666b72656964687a73733735366c73746d67366c676863676c723332366c78646d7134766b6f79356f353732376b346b776d6c6f626b327a3470baaffbe095f5ec9fe401721b0a0e6d6967726174696f6e5f776176651209666f756e6465722d3128013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    "onchain_metadata_uri": "ipfs://bafkreiczlekh55wwngmvkywfi3igkp4gqm5tssvvhh34wxg4xf55kripji",
    "onchain_metadata_hash_sha3_384_hex": "58348df460ecc12e87eb48cd3e97604344b24734bca83955cee2e69ad78455aca54243f55fae787f15143244a2f7296b"
  },
  "V9_GOVERNANCE_PACKET_AND_LIFECYCLE": {
    "packet_proto_type": "postfiat.governance.v1.GovernancePacket",
    "packet_proto_hex": "08011212676f762d323032362d31312d32372d3030311a11706f7374666961742d6465766e65742d3220012a15476f7665726e616e63652053657373696f6e202331322852657669657720433120667265657a6520616e6420547261696e20412061637469766174696f6e2e3a9d010a0a666f756e6465722d763110011801222a706631666f756e64657230303030303030303030303030303030303030303030303030303030303030302a4066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630321d666f756e646572407461736b6e6f64652e706f7374666961742e6f726742650a24015512200d1fab1ee1f5dfaba030702451ef3054f7d40f387baddc70b2f0f48d906943be101f1a3b6261666b726569616e643676723579707633367632616d647165726936366d637536376b61366f643376786f68626d78713673677a61326b6478794a200d1fab1ee1f5dfaba030702451ef3054f7d40f387baddc70b2f0f48d906943be526c0a0b64656c69762d3030302d311212676f762d323032362d31312d32302d3030311a17467265657a652043312070726f746f20736368656d617320012a1a4d6572676564207769746820434920766563746f722067617465321270723a706f7374666961746c3176322363315a2741726520616c6c20362076616c696461746f727320726561647920666f7220547261696e20413f624c0a076465632d3030311217416374697661746520547261696e2041206865696768741a2653657420547261696e20412061637469766174696f6e20686569676874206174203530303030200172b0010a17686976652d676f762d323032362d31312d32372d30303110021802201e280f300a3819400f4a650a2401551220c5d35f2d54a65a818dc75d8987985b3e474be012f7140d7d4245d4dc7cab5e9310081a3b6261666b7265696766326e7073327666676c6b6179337232357267647a71777a366935663661657878637167783271736632746f687a6b3236736d5220c5d35f2d54a65a818dc75d8987985b3e474be012f7140d7d4245d4dc7cab5e9378c0c2fc94ac33",
    "packet_canonical_json_sha256_hex": "db5d97cc3a6980147fc30bdc058568732a9f4af9d68a7041a7fe347f6954b4f7",
    "packet_envelope_hex": "0801127f08011211706f7374666961742d6465766e65742d321a1770662e676f7665726e616e63652e7061636b65742e76312065280c3001380240014a2a706631666f756e646572303030303030303030303030303030303030303030303030303030303030303050c0c2fc94ac336212676f762d323032362d31312d32372d30303122c5047b22617574686f72697479223a7b22617574686f72697a65645f6163636f756e7473223a5b22706631666f756e6465723030303030303030303030303030303030303030303030303030303030303030225d2c22666f756e6465725f6e6f7374725f7075626b6579223a2266306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630222c227369676e65725f7365745f6964223a22666f756e6465722d7631222c227468726573686f6c64223a312c22746f74616c5f7369676e657273223a317d2c22636861696e5f6964223a22706f7374666961742d6465766e65742d32222c22637265617465645f61745f756e69785f6d73223a313736343230313830303030302c2270726576696f75735f64656c6976657261626c65735f737461747573223a5b7b2264656c6976657261626c655f6964223a2264656c69762d3030302d31222c226f726967696e5f73657373696f6e5f6964223a22676f762d323032362d31312d32302d303031222c22737461747573223a2244454c4956455241424c455f5354415455535f444f4e45222c227469746c65223a22467265657a652043312070726f746f20736368656d6173227d5d2c22736368656d615f76657273696f6e223a312c2273657373696f6e5f6964223a22676f762d323032362d31312d32372d303031222c2273657373696f6e5f6e756d626572223a312c227469746c65223a22476f7665726e616e63652053657373696f6e202331227d28013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    "packet_envelope_cid_multibase": "bafkreigqgygl2g7pgmd3bnnva564mselpxohmwxey4lb3xelo43ceqduj4",
    "anchor_pointer_v5_hex": "0a2401551220d0360cbd1bef3307b0b5b5077dc6488b7ddc765ae4c7161ddc8b77362240744f1001180c20653212676f762d323032362d31312d32372d3030314002",
    "anchor_pointer_v5_len": 66,
    "transcript_proto_type": "postfiat.governance.v1.SessionTranscript",
    "transcript_root_sha256_hex": "8f09fe8eba7966cedcbe9269c2e692ea63a5ae7d7f6b0b9c5b1672dda3a1747a",
    "transcript_hex": "08011212676f762d323032362d31312d32372d3030311a17686976652d676f762d323032362d31312d32372d30303122660a2401551220d0360cbd1bef3307b0b5b5077dc6488b7ddc765ae4c7161ddc8b77362240744f1091061a3b6261666b72656967716779676c32673770676d6433626e6e76613536346d73656c70786f686d77786579346c623378656c6f343363657164756a342a650a2401551220c5d35f2d54a65a818dc75d8987985b3e474be012f7140d7d4245d4dc7cab5e9310081a3b6261666b7265696766326e7073327666676c6b6179337232357267647a71777a366935663661657878637167783271736632746f687a6b3236736d30c0c2fc94ac333880b1ea95ac3342a50208011240313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131311a40663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066302207666f756e64657228d2aa9ec90630c494fd94ac333a2048a91f06e3a9dbe7bc655b9132de8f2ce3e16f68ae4f41b886ced6fb56062fdc42650a240155122048a91f06e3a9dbe7bc655b9132de8f2ce3e16f68ae4f41b886ced6fb56062fdc10111a3b6261666b7265696369766570716e79356a33707433797a6b3373657a6e35647a6d347071773632666f6a3561337262776f323335766d62727033714aa2010a40323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232321240333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333331805221a65786365656465642031206d7367202f20313573206c696d697452650a240155122088c5963e926bd34a910a4973f856958449352d65e917c0e0479b676278b13b4010231a3b6261666b726569656979776c643565746c326e666a6363736a6f7034666e666d656a653273327a706a6337616f617234336d357268726d6a3369615a208f09fe8eba7966cedcbe9269c2e692ea63a5ae7d7f6b0b9c5b1672dda3a1747a",
    "minutes_proto_type": "postfiat.governance.v1.GovernanceMinutes",
    "minutes_hex": "08011212676f762d323032362d31312d32372d303031180120062a9d010a0a666f756e6465722d763110011801222a706631666f756e64657230303030303030303030303030303030303030303030303030303030303030302a4066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630321d666f756e646572407461736b6e6f64652e706f7374666961742e6f726732660a2401551220d0360cbd1bef3307b0b5b5077dc6488b7ddc765ae4c7161ddc8b77362240744f1091061a3b6261666b72656967716779676c32673770676d6433626e6e76613536346d73656c70786f686d77786579346c623378656c6f343363657164756a343a660a2401551220eb6b021ae4a15986a3656ad18badafddb0a35e1c63701b19fb93018308eb378810e2061a3b6261666b726569686c6e6d6262767a66626c67646b677a6c6b32676632336c363577637276346864646f616e72743634746167627172327a78726142208f09fe8eba7966cedcbe9269c2e692ea63a5ae7d7f6b0b9c5b1672dda3a1747a4a32417070726f76656420547261696e20412061637469766174696f6e20616e64207363686564756c656420726f6c6c6f75742e526c0a0b64656c69762d3030302d311212676f762d323032362d31312d32302d3030311a17467265657a652043312070726f746f20736368656d617320012a1a4d6572676564207769746820434920766563746f722067617465321270723a706f7374666961746c3176322363315a8b010a076465632d3030311217416374697661746520547261696e2041206865696768741a23417070726f7665642061637469766174696f6e20617420686569676874203530303030224031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131280162d3010a0b64656c69762d3030312d311212676f762d323032362d31312d32372d3030311a145368697020547261696e204120726f6c6c6f75742220526f6c6c206f757420547261696e204120746f20362076616c696461746f72732a2a706631666f756e6465723030303030303030303030303030303030303030303030303030303030303030320a323032362d31322d30343a40313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131316a83010a1356616c696461746f722072656164696e657373122a54776f2076616c696461746f727320636f6e6669726d65642073746167696e672075706772616465732e1a403434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343434343472790a2a706631666f756e646572303030303030303030303030303030303030303030303030303030303030303010011a20a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a12220b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b228a0bef095ac3378a0bef095ac33",
    "ruling_proto_type": "postfiat.governance.v1.RulingAnchor",
    "ruling_hex": "08011212676f762d323032362d31312d32372d3030311a076465632d303031221572756c696e672d323032362d31312d32372d303031280132650a24015512200d1fab1ee1f5dfaba030702451ef3054f7d40f387baddc70b2f0f48d906943be101f1a3b6261666b726569616e643676723579707633367632616d647165726936366d637536376b61366f643376786f68626d78713673677a61326b6478793a660a2401551220d0360cbd1bef3307b0b5b5077dc6488b7ddc765ae4c7161ddc8b77362240744f1091061a3b6261666b72656967716779676c32673770676d6433626e6e76613536346d73656c70786f686d77786579346c623378656c6f343363657164756a3442660a240155122060518d56e2ab02fa7fa6b5058fe492acd12130f3ecc04e81f1b444fffba191ab10ba091a3b6261666b72656964616b6767766e79766c616c3568376a7676617768366a65766d326571746234376d7962686964346e756974373778696d72766d4a650a240155122068fa34826f2050f8c24fe0ccd84daf43594e5e621c521da4d27844faa627d35910151a3b6261666b72656964693769326965337a616b64346d657437617a746d65336c32646c666866347971346b696f326a7574796974356b6d6a36746c655220ea62c627d836c44a7cac60494c9d39eb2edde28cb2ca24af2714d0b704e6ef0d5a2068fa34826f2050f8c24fe0ccd84daf43594e5e621c521da4d27844faa627d3596220f718d44876058c6ce80f8e84d931616cfe7bad6bf7a81a7c0c604a7647ac07076a1d7368613235363a64657465726d696e69737469632d6d6f64656c2d7631720f636f6e737469747574696f6e2d76317a07415050524f564582019d010a0a666f756e6465722d763110011801222a706631666f756e64657230303030303030303030303030303030303030303030303030303030303030302a4066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630663066306630321d666f756e646572407461736b6e6f64652e706f7374666961742e6f72678a01790a2a706631666f756e646572303030303030303030303030303030303030303030303030303030303030303010011a20a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a12220b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b228a0bef095ac33900180e68296ac33"
  }
}
```
<!-- END_CANONICAL_TEST_VECTORS_JSON -->

## 9. Verification

Run locally or in CI:

```bash
python3 scripts/test-message-protocol-vectors.py
# Strict mode requiring protoc compile + decode/re-encode byte-identity round-trip:
python3 scripts/test-message-protocol-vectors.py --require-protoc
```
