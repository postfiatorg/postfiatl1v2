"""Post Fiat L1v2 Canonical Message Protocol v1 SDK (Pillar 2 / Contract C1).

Implements deterministic protobuf3 wire encoding/decoding, IPFS CIDv1 raw helpers,
``pf.ptr`` v5 (and read-only legacy v4) transaction memo framing, raw-byte
zero-reserialization envelope signing/verification, post-quantum X-Wing +
HKDF-SHA256 + XChaCha20-Poly1305 encryption framing, multi-chunk Manifest helpers
(up to 16 MiB), and x402 v2 payment header codecs matching the frozen Contract C1
schemas in ``postfiat_rpc/proto/postfiat/message/v1/message.proto``.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import os
import struct
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import Any, Callable, Sequence

try:
    from Crypto.Cipher import ChaCha20_Poly1305 as _PyCryptoChaCha20Poly1305
except ImportError:  # pragma: no cover - requirements-test includes pycryptodome
    _PyCryptoChaCha20Poly1305 = None


PROTO_ROOT = Path(__file__).resolve().parent / "proto"

# L1 PaymentMemo consensus limits (crates/types/src/core_chain.rs)
MAX_PAYMENT_MEMOS = 4
MAX_PAYMENT_MEMO_TYPE_BYTES = 64
MAX_PAYMENT_MEMO_FORMAT_BYTES = 64
MAX_PAYMENT_MEMO_DATA_BYTES = 256
MAX_PAYMENT_MEMO_TOTAL_BYTES = 512

# Envelope & Manifest payload caps
MAX_SINGLE_ENVELOPE_BYTES = 1_048_576  # 1 MiB
MAX_MANIFEST_PLAINTEXT_BYTES = 16_777_216  # 16 MiB

# Normative post-quantum cryptographic byte sizes
ML_DSA_65_PUBKEY_BYTES = 1952
ML_DSA_65_SIGNATURE_BYTES = 3309
XWING_PUBKEY_BYTES = 1216  # 1184B ML-KEM-768 ek || 32B X25519 pk
XWING_CIPHERTEXT_BYTES = 1120  # 1088B ML-KEM-768 ct || 32B X25519 epk
AEAD_KEY_BYTES = 32
AEAD_NONCE_BYTES = 24
AEAD_TAG_BYTES = 16
WRAPPED_CEK_BYTES = AEAD_KEY_BYTES + AEAD_TAG_BYTES  # 48 bytes
CIDV1_RAW_PREFIX = b"\x01\x55\x12\x20"
CIDV1_RAW_BYTES = 36

# Canonical domain separation strings (Contract C1)
DOMAIN_ENVELOPE_SIGN = b"postfiat-l1-v2/message-envelope/v1"
DOMAIN_MESSAGE_AAD = b"postfiat-l1-v2/message-aad/v1"
DOMAIN_XWING_WRAP_INFO = b"postfiat-l1-v2/xwing-wrap/v1"
DOMAIN_MESSAGE_KEY_KDF = b"postfiat-l1-v2/message-key/v1"
DOMAIN_MESSAGE_KEY_BIND = b"postfiat-l1-v2/message-key-bind/v1"
DOMAIN_CEK_COMMIT = b"postfiat-l1-v2/cek-commit/v1"
DOMAIN_SEALED_RECIPIENT_TAG = b"postfiat-l1-v2/sealed-recipient-tag/v1"
DOMAIN_ACCOUNT_ADDRESS = b"postfiat.address.v1\x00"


def compute_cek_commitment(cek: bytes, aad_context: bytes) -> bytes:
    """Compute 32-byte domain-separated key-commitment tag binding CEK and aad_context."""
    if len(cek) != AEAD_KEY_BYTES:
        raise MessageProtocolError(f"cek must be {AEAD_KEY_BYTES} bytes, got {len(cek)}")
    if len(aad_context) != 32:
        raise MessageProtocolError(f"aad_context must be 32 bytes, got {len(aad_context)}")
    return hashlib.sha256(DOMAIN_CEK_COMMIT + b"\x00" + cek + aad_context).digest()


def compute_sealed_recipient_tag(xwing_shared_secret: bytes, aad_context: bytes) -> bytes:
    """Compute unlinkable per-message 32-byte SEALED recipient tag from X-Wing shared secret."""
    if len(xwing_shared_secret) != 32:
        raise MessageProtocolError(f"xwing_shared_secret must be 32 bytes, got {len(xwing_shared_secret)}")
    if len(aad_context) != 32:
        raise MessageProtocolError(f"aad_context must be 32 bytes, got {len(aad_context)}")
    return hashlib.sha256(DOMAIN_SEALED_RECIPIENT_TAG + b"\x00" + xwing_shared_secret + aad_context).digest()


def generate_rotatable_xwing_seed() -> bytes:
    """Generate a CSPRNG 32-byte non-seed-derived X-Wing seed for epoch >= 1 forward secrecy."""
    return os.urandom(32)


def derive_account_address(account_pubkey: bytes) -> str:
    """Derive canonical postfiatl1v2 account address from a 1,952-byte ML-DSA-65 public key."""
    if len(account_pubkey) != ML_DSA_65_PUBKEY_BYTES:
        raise MessageProtocolError(
            f"ML-DSA-65 public key must be {ML_DSA_65_PUBKEY_BYTES} bytes, got {len(account_pubkey)}"
        )
    digest = hashlib.sha3_384(DOMAIN_ACCOUNT_ADDRESS + account_pubkey).digest()
    return "pf" + digest[:20].hex()

MEMO_TYPE_PF_PTR_HEX = b"pf.ptr".hex()  # "70662e707472"
MEMO_FORMAT_V5_HEX = b"v5".hex()  # "7635"
MEMO_FORMAT_V4_HEX = b"v4".hex()  # "7634"

# Schema registry IDs
SCHEMA_ID_CUSTOM = 1
SCHEMA_ID_TEXT_V1 = 100
SCHEMA_ID_GOVERNANCE_PACKET_V1 = 101
SCHEMA_ID_GOVERNANCE_SNAPSHOT_V1 = 102
SCHEMA_ID_GOVERNANCE_TRANSCRIPT_V1 = 103
SCHEMA_ID_GOVERNANCE_MINUTES_V1 = 104
SCHEMA_ID_GOVERNANCE_RULING_V1 = 105
SCHEMA_ID_REWARD_V1 = 201
SCHEMA_ID_NFT_METADATA_V1 = 400
SCHEMA_ID_NFT_CLAIM_V1 = 401
SCHEMA_ID_X402_REQUEST_V1 = 402
SCHEMA_ID_NFT_PROVENANCE_V1 = 403


class MessageProtocolError(ValueError):
    """Raised when a message protocol payload violates Contract C1 wire or crypto rules."""


class ContentKind(IntEnum):
    UNSPECIFIED = 0
    TASK = 1
    TASK_UPDATE = 2
    TASK_SUBMISSION = 3
    CHAT = 4
    CONTEXT = 5
    REWARD = 6
    POLICY = 7
    IDENTITY = 8
    ASSET = 9
    DOCUMENT = 10
    SYSTEM = 11
    GOVERNANCE = 12
    NFT = 13
    X402 = 14
    TEST = 99


class PointerTarget(IntEnum):
    UNSPECIFIED = 0
    CONTENT_BLOB = 1
    MANIFEST = 2
    INLINE = 3


class PointerFlag(IntEnum):
    UNSPECIFIED = 0
    PRIVATE = 0x01
    PUBLIC = 0x02
    EPHEMERAL = 0x04
    TOMBSTONE = 0x08
    MULTIPART = 0x10
    SEALED = 0x20


VISIBILITY_FLAG_MASK = PointerFlag.PRIVATE | PointerFlag.PUBLIC | PointerFlag.SEALED
ALLOWED_POINTER_FLAG_MASK = (
    PointerFlag.PRIVATE
    | PointerFlag.PUBLIC
    | PointerFlag.EPHEMERAL
    | PointerFlag.TOMBSTONE
    | PointerFlag.MULTIPART
    | PointerFlag.SEALED
)


class Visibility(IntEnum):
    UNSPECIFIED = 0
    PUBLIC = 1
    PRIVATE = 2
    SEALED = 3


class CipherSuite(IntEnum):
    UNSPECIFIED = 0
    NONE = 1
    XWING_XCHACHA20_POLY1305 = 2
    LEGACY_X25519_XCHACHA20_POLY1305 = 3


class SignatureAlgorithm(IntEnum):
    UNSPECIFIED = 0
    ML_DSA_65 = 1


class BodyEncoding(IntEnum):
    UNSPECIFIED = 0
    PROTOBUF = 1
    CANONICAL_JSON = 2
    RAW_UTF8 = 3
    RAW_BYTES = 4


class PaymentLane(IntEnum):
    UNSPECIFIED = 0
    FASTPAY = 1
    CONSENSUS_PFT = 2
    CONSENSUS_ISSUED = 3


class X402Scheme(IntEnum):
    UNSPECIFIED = 0
    EXACT = 1
    PF_FASTPAY_SETTLED = 2


# ---------------------------------------------------------------------------
# Pure-Python deterministic protobuf3 wire primitives
# ---------------------------------------------------------------------------

def encode_varint(value: int) -> bytes:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0 or value > 0xFFFF_FFFF_FFFF_FFFF:
        raise MessageProtocolError(f"varint out of uint64 range: {value!r}")
    out = bytearray()
    while value > 0x7F:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def decode_varint(buf: bytes, offset: int) -> tuple[int, int]:
    result = 0
    shift = 0
    start = offset
    while True:
        if offset >= len(buf):
            raise MessageProtocolError("truncated protobuf varint")
        byte = buf[offset]
        offset += 1
        result |= (byte & 0x7F) << shift
        if not (byte & 0x80):
            if offset - start > 1 and byte == 0:
                raise MessageProtocolError("non-minimal protobuf varint")
            if result > 0xFFFF_FFFF_FFFF_FFFF:
                raise MessageProtocolError("protobuf varint overflow")
            return result, offset
        shift += 7
        if shift > 63:
            raise MessageProtocolError("protobuf varint overflow")


def pb_varint(field_num: int, value: int) -> bytes:
    if value == 0:
        return b""
    return encode_varint((field_num << 3) | 0) + encode_varint(int(value))


def pb_bool(field_num: int, value: bool) -> bytes:
    return pb_varint(field_num, 1 if value else 0)


def pb_bytes(field_num: int, value: bytes) -> bytes:
    if not value:
        return b""
    return encode_varint((field_num << 3) | 2) + encode_varint(len(value)) + bytes(value)


def pb_string(field_num: int, value: str) -> bytes:
    if not value:
        return b""
    return pb_bytes(field_num, value.encode("utf-8"))


def decode_wire_fields(buf: bytes, *, enforce_canonical_order: bool = False) -> list[tuple[int, int, Any]]:
    fields: list[tuple[int, int, Any]] = []
    offset = 0
    last_fn = 0
    while offset < len(buf):
        tag, offset = decode_varint(buf, offset)
        field_num = tag >> 3
        wire_type = tag & 0x07
        if field_num == 0:
            raise MessageProtocolError("invalid protobuf field number 0")
        if enforce_canonical_order and field_num < last_fn:
            raise MessageProtocolError(f"non-canonical protobuf field order: {field_num} after {last_fn}")
        last_fn = field_num
        if wire_type == 0:
            val, offset = decode_varint(buf, offset)
            fields.append((field_num, wire_type, val))
        elif wire_type == 2:
            length, offset = decode_varint(buf, offset)
            if offset + length > len(buf):
                raise MessageProtocolError("truncated protobuf length-delimited field")
            val = buf[offset : offset + length]
            offset += length
            fields.append((field_num, wire_type, val))
        elif wire_type == 1:
            if offset + 8 > len(buf):
                raise MessageProtocolError("truncated protobuf 64-bit field")
            fields.append((field_num, wire_type, buf[offset : offset + 8]))
            offset += 8
        elif wire_type == 5:
            if offset + 4 > len(buf):
                raise MessageProtocolError("truncated protobuf 32-bit field")
            fields.append((field_num, wire_type, buf[offset : offset + 4]))
            offset += 4
        else:
            raise MessageProtocolError(f"unsupported protobuf wire type {wire_type}")
    return fields


def _expect_wt(field_num: int, wire_type: int, expected: int) -> None:
    if wire_type != expected:
        raise MessageProtocolError(
            f"unexpected wire type {wire_type} for field {field_num} (expected {expected})"
        )


def _decode_utf8(raw: bytes, field_name: str) -> str:
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise MessageProtocolError(f"invalid UTF-8 in {field_name}") from exc


# ---------------------------------------------------------------------------
# Cryptographic, CIDv1 raw, and Canonical JSON primitives
# ---------------------------------------------------------------------------

def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def sha3_384(data: bytes) -> bytes:
    return hashlib.sha3_384(data).digest()


def hkdf_sha256(ikm: bytes, salt: bytes, info: bytes, length: int = 32) -> bytes:
    """RFC 5869 HKDF-SHA256."""
    if length <= 0 or length > 255 * 32:
        raise MessageProtocolError(f"invalid HKDF output length: {length}")
    effective_salt = salt if salt else b"\x00" * 32
    prk = hmac.new(effective_salt, ikm, hashlib.sha256).digest()
    okm = bytearray()
    t = b""
    counter = 1
    while len(okm) < length:
        t = hmac.new(prk, t + info + bytes([counter]), hashlib.sha256).digest()
        okm.extend(t)
        counter += 1
    return bytes(okm[:length])


def canonical_json_bytes(obj: Any) -> bytes:
    """RFC 8785 / JCS canonical JSON bytes (sorted keys, compact separators, UTF-8)."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def cidv1_raw_from_bytes(block: bytes) -> tuple[bytes, str]:
    """Compute canonical IPFS CIDv1 raw (0x01 0x55 0x12 0x20 || SHA-256) and base32lower multibase."""
    cid_bin = CIDV1_RAW_PREFIX + sha256(block)
    multibase = cid_bin_to_multibase(cid_bin)
    return cid_bin, multibase


def cid_bin_to_multibase(cid_bin: bytes) -> str:
    if len(cid_bin) != CIDV1_RAW_BYTES or not cid_bin.startswith(CIDV1_RAW_PREFIX):
        raise MessageProtocolError(
            "cid_bin must be 36 bytes starting with 0x01 0x55 0x12 0x20 (CIDv1 raw sha2-256)"
        )
    return "b" + base64.b32encode(cid_bin).decode("ascii").lower().rstrip("=")


def multibase_to_cid_bin(multibase_cid: str) -> bytes:
    if not isinstance(multibase_cid, str) or not multibase_cid.startswith("b"):
        raise MessageProtocolError("multibase_cid must use base32lower prefix 'b'")
    body = multibase_cid[1:]
    if not body or body != body.lower():
        raise MessageProtocolError("multibase_cid must be lowercase base32 without padding")
    pad_len = (-len(body)) % 8
    try:
        cid_bin = base64.b32decode(body.upper() + ("=" * pad_len), casefold=False)
    except Exception as exc:
        raise MessageProtocolError("invalid base32lower multibase_cid") from exc
    if len(cid_bin) != CIDV1_RAW_BYTES or not cid_bin.startswith(CIDV1_RAW_PREFIX):
        raise MessageProtocolError("decoded CID is not a 36-byte CIDv1 raw sha2-256 block")
    return cid_bin


def verify_cidv1_raw(
    block: bytes,
    *,
    expected_cid_bin: bytes | None = None,
    expected_multibase_cid: str | None = None,
) -> "IpfsCidRef":
    cid_bin, multibase = cidv1_raw_from_bytes(block)
    if expected_cid_bin is not None:
        if len(expected_cid_bin) != CIDV1_RAW_BYTES or not hmac.compare_digest(cid_bin, expected_cid_bin):
            raise MessageProtocolError("IPFS CIDv1 raw binary digest mismatch")
    if expected_multibase_cid is not None:
        decoded_bin = multibase_to_cid_bin(expected_multibase_cid)
        if not hmac.compare_digest(cid_bin, decoded_bin) or expected_multibase_cid != multibase:
            raise MessageProtocolError("IPFS CIDv1 raw multibase mismatch")
    return IpfsCidRef(cid_bin=cid_bin, byte_length=len(block), multibase_cid=multibase)


def envelope_signing_preimage(
    version: int,
    header_bytes: bytes,
    encryption_header_bytes: bytes,
    body_bytes: bytes,
) -> bytes:
    """Compute exact raw-byte signing preimage (zero re-serialization rule)."""
    return (
        DOMAIN_ENVELOPE_SIGN
        + b"\x00"
        + struct.pack(">I", version)
        + struct.pack(">I", len(header_bytes))
        + header_bytes
        + struct.pack(">I", len(encryption_header_bytes))
        + encryption_header_bytes
        + struct.pack(">I", len(body_bytes))
        + body_bytes
    )


def compute_aad_context(chain_id: str, header_bytes: bytes, chunk_index: int = 0) -> bytes:
    """Compute 32-byte domain-separated AAD commitment for XChaCha20-Poly1305."""
    if not chain_id:
        raise MessageProtocolError("chain_id is required for AAD context")
    if chunk_index < 0 or chunk_index > 0xFFFF_FFFF:
        raise MessageProtocolError(f"chunk_index out of uint32 range: {chunk_index}")
    return sha256(
        DOMAIN_MESSAGE_AAD
        + b"\x00"
        + chain_id.encode("utf-8")
        + b"\x00"
        + sha256(header_bytes)
        + struct.pack(">I", chunk_index)
    )


def derive_message_kem_seed(wallet_master_entropy: bytes, chain_id: str) -> bytes:
    """Deterministically derive 32-byte X-Wing keygen seed from wallet master entropy."""
    if len(wallet_master_entropy) < 32:
        raise MessageProtocolError("wallet_master_entropy must be at least 32 bytes")
    if not chain_id:
        raise MessageProtocolError("chain_id is required")
    return hkdf_sha256(
        wallet_master_entropy,
        chain_id.encode("utf-8"),
        DOMAIN_MESSAGE_KEY_KDF,
        32,
    )


def compute_recipient_key_id(xwing_public_key: bytes) -> bytes:
    if not xwing_public_key:
        raise MessageProtocolError("xwing_public_key must be non-empty")
    return sha256(xwing_public_key)


def derive_xwing_wrap_kek(
    shared_secret: bytes,
    aad_context: bytes,
    recipient_key_id: bytes,
    *,
    kdf_info: str = DOMAIN_XWING_WRAP_INFO.decode("ascii"),
) -> bytes:
    """Derive 32-byte CEK wrap key via HKDF-SHA256 bound to aad_context and recipient_key_id."""
    if len(shared_secret) != 32:
        raise MessageProtocolError("X-Wing shared_secret must be 32 bytes")
    if len(aad_context) != 32:
        raise MessageProtocolError("aad_context must be 32 bytes")
    if len(recipient_key_id) != 32:
        raise MessageProtocolError("recipient_key_id must be 32 bytes")
    info = kdf_info.encode("utf-8") + b"\x00" + recipient_key_id
    return hkdf_sha256(shared_secret, aad_context, info, 32)


def compute_key_binding_preimage(
    *,
    chain_id: str,
    account: str,
    xwing_public_key: bytes,
    key_id: bytes,
    created_at_unix_ms: int,
    key_epoch: int,
) -> bytes:
    chain_b = chain_id.encode("utf-8")
    acct_b = account.encode("utf-8")
    return (
        DOMAIN_MESSAGE_KEY_BIND
        + b"\x00"
        + struct.pack(">I", len(chain_b))
        + chain_b
        + struct.pack(">I", len(acct_b))
        + acct_b
        + xwing_public_key
        + key_id
        + struct.pack(">QI", created_at_unix_ms, key_epoch)
    )


def xchacha20_poly1305_encrypt(key: bytes, nonce: bytes, plaintext: bytes, aad: bytes) -> bytes:
    """Encrypt with XChaCha20-Poly1305, returning ciphertext || 16-byte Poly1305 tag."""
    if len(key) != AEAD_KEY_BYTES:
        raise MessageProtocolError(f"XChaCha20-Poly1305 key must be {AEAD_KEY_BYTES} bytes")
    if len(nonce) != AEAD_NONCE_BYTES:
        raise MessageProtocolError(f"XChaCha20-Poly1305 nonce must be {AEAD_NONCE_BYTES} bytes")
    if _PyCryptoChaCha20Poly1305 is None:  # pragma: no cover
        raise MessageProtocolError("pycryptodome is required for XChaCha20-Poly1305")
    cipher = _PyCryptoChaCha20Poly1305.new(key=key, nonce=nonce)
    if aad:
        cipher.update(aad)
    ciphertext, tag = cipher.encrypt_and_digest(plaintext)
    return ciphertext + tag


def xchacha20_poly1305_decrypt(key: bytes, nonce: bytes, ciphertext_with_tag: bytes, aad: bytes) -> bytes:
    """Decrypt XChaCha20-Poly1305 ciphertext || 16-byte Poly1305 tag."""
    if len(key) != AEAD_KEY_BYTES:
        raise MessageProtocolError(f"XChaCha20-Poly1305 key must be {AEAD_KEY_BYTES} bytes")
    if len(nonce) != AEAD_NONCE_BYTES:
        raise MessageProtocolError(f"XChaCha20-Poly1305 nonce must be {AEAD_NONCE_BYTES} bytes")
    if len(ciphertext_with_tag) < AEAD_TAG_BYTES:
        raise MessageProtocolError("XChaCha20-Poly1305 ciphertext shorter than 16-byte tag")
    if _PyCryptoChaCha20Poly1305 is None:  # pragma: no cover
        raise MessageProtocolError("pycryptodome is required for XChaCha20-Poly1305")
    ciphertext = ciphertext_with_tag[:-AEAD_TAG_BYTES]
    tag = ciphertext_with_tag[-AEAD_TAG_BYTES:]
    cipher = _PyCryptoChaCha20Poly1305.new(key=key, nonce=nonce)
    if aad:
        cipher.update(aad)
    try:
        return cipher.decrypt_and_verify(ciphertext, tag)
    except ValueError as exc:
        raise MessageProtocolError("XChaCha20-Poly1305 authentication failed") from exc


def validate_pointer_v5_flags(flags: int) -> None:
    if flags & ~ALLOWED_POINTER_FLAG_MASK:
        raise MessageProtocolError(f"unsupported pointer flags: 0x{flags:x}")
    vis = flags & VISIBILITY_FLAG_MASK
    if vis not in (PointerFlag.PRIVATE, PointerFlag.PUBLIC, PointerFlag.SEALED):
        raise MessageProtocolError(
            f"pointer flags must set exactly one of PRIVATE (0x01), PUBLIC (0x02), SEALED (0x20); got 0x{flags:x}"
        )


def validate_l1_payment_memo(memo_type_hex: str, memo_format_hex: str, memo_data_hex: str) -> None:
    for name, val, limit in (
        ("memo_type", memo_type_hex, MAX_PAYMENT_MEMO_TYPE_BYTES),
        ("memo_format", memo_format_hex, MAX_PAYMENT_MEMO_FORMAT_BYTES),
        ("memo_data", memo_data_hex, MAX_PAYMENT_MEMO_DATA_BYTES),
    ):
        if not isinstance(val, str):
            raise MessageProtocolError(f"{name} must be a hex string")
        if val != val.lower():
            raise MessageProtocolError(f"{name} must be lowercase hex")
        if len(val) % 2 != 0:
            raise MessageProtocolError(f"{name} must have even hex length")
        try:
            raw = bytes.fromhex(val)
        except ValueError as exc:
            raise MessageProtocolError(f"{name} is not valid hex") from exc
        if len(raw) > limit:
            raise MessageProtocolError(f"{name} decoded length {len(raw)} exceeds limit {limit}")
    total = (len(memo_type_hex) + len(memo_format_hex) + len(memo_data_hex)) // 2
    if total == 0 or total > MAX_PAYMENT_MEMO_TOTAL_BYTES:
        raise MessageProtocolError(f"total memo byte length {total} outside 1..{MAX_PAYMENT_MEMO_TOTAL_BYTES}")


# ---------------------------------------------------------------------------
# Protobuf3 message dataclasses (postfiat.message.v1)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class IpfsCidRef:
    cid_bin: bytes = b""
    byte_length: int = 0
    multibase_cid: str = ""

    def encode(self) -> bytes:
        if self.cid_bin and self.multibase_cid:
            expected = cid_bin_to_multibase(self.cid_bin)
            if self.multibase_cid != expected:
                raise MessageProtocolError("IpfsCidRef multibase_cid does not match cid_bin")
        return b"".join(
            [
                pb_bytes(1, self.cid_bin),
                pb_varint(2, self.byte_length),
                pb_string(3, self.multibase_cid),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "IpfsCidRef":
        cid_bin = b""
        byte_length = 0
        multibase_cid = ""
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                cid_bin = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                byte_length = val
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                multibase_cid = _decode_utf8(val, "IpfsCidRef.multibase_cid")
        if cid_bin and multibase_cid and cid_bin_to_multibase(cid_bin) != multibase_cid:
            raise MessageProtocolError("IpfsCidRef multibase_cid does not match cid_bin")
        return cls(cid_bin=cid_bin, byte_length=byte_length, multibase_cid=multibase_cid)


@dataclass(frozen=True)
class Pointer:
    cid_bin: bytes = b""
    target: PointerTarget = PointerTarget.UNSPECIFIED
    kind: ContentKind = ContentKind.UNSPECIFIED
    schema: int = 0
    task_id: str = ""
    thread_id: str = ""
    context_id: str = ""
    flags: int = 0
    inline_payload: bytes = b""
    schema_name: str = ""

    def validate(self) -> None:
        validate_pointer_v5_flags(self.flags)
        if self.target == PointerTarget.INLINE:
            if self.cid_bin or not self.inline_payload:
                raise MessageProtocolError("POINTER_TARGET_INLINE requires non-empty inline_payload and empty cid_bin")
        elif self.target in (PointerTarget.CONTENT_BLOB, PointerTarget.MANIFEST):
            if len(self.cid_bin) != CIDV1_RAW_BYTES or not self.cid_bin.startswith(CIDV1_RAW_PREFIX):
                raise MessageProtocolError("CONTENT_BLOB/MANIFEST pointer requires 36-byte CIDv1 raw cid_bin")
            if self.inline_payload:
                raise MessageProtocolError("CONTENT_BLOB/MANIFEST pointer must leave inline_payload empty")
        else:
            raise MessageProtocolError(f"unsupported PointerTarget: {self.target!r}")

    def encode(self) -> bytes:
        self.validate()
        encoded = b"".join(
            [
                pb_bytes(1, self.cid_bin),
                pb_varint(2, int(self.target)),
                pb_varint(3, int(self.kind)),
                pb_varint(4, self.schema),
                pb_string(5, self.task_id),
                pb_string(6, self.thread_id),
                pb_string(7, self.context_id),
                pb_varint(8, self.flags),
                pb_bytes(9, self.inline_payload),
                pb_string(10, self.schema_name),
            ]
        )
        if len(encoded) > MAX_PAYMENT_MEMO_DATA_BYTES:
            raise MessageProtocolError(
                f"Pointer v5 serialized length {len(encoded)} exceeds {MAX_PAYMENT_MEMO_DATA_BYTES} bytes"
            )
        return encoded

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "Pointer":
        if len(data) > MAX_PAYMENT_MEMO_DATA_BYTES:
            raise MessageProtocolError(
                f"Pointer v5 byte length {len(data)} exceeds {MAX_PAYMENT_MEMO_DATA_BYTES}"
            )
        vals: dict[str, Any] = {
            "cid_bin": b"",
            "target": PointerTarget.UNSPECIFIED,
            "kind": ContentKind.UNSPECIFIED,
            "schema": 0,
            "task_id": "",
            "thread_id": "",
            "context_id": "",
            "flags": 0,
            "inline_payload": b"",
            "schema_name": "",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                vals["cid_bin"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                vals["target"] = PointerTarget(val)
            elif fn == 3:
                _expect_wt(fn, wt, 0)
                vals["kind"] = ContentKind(val)
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["schema"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["task_id"] = _decode_utf8(val, "Pointer.task_id")
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["thread_id"] = _decode_utf8(val, "Pointer.thread_id")
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["context_id"] = _decode_utf8(val, "Pointer.context_id")
            elif fn == 8:
                _expect_wt(fn, wt, 0)
                vals["flags"] = val
            elif fn == 9:
                _expect_wt(fn, wt, 2)
                vals["inline_payload"] = val
            elif fn == 10:
                _expect_wt(fn, wt, 2)
                vals["schema_name"] = _decode_utf8(val, "Pointer.schema_name")
            else:
                raise MessageProtocolError(f"unknown Pointer field {fn}")
        ptr = cls(**vals)
        ptr.validate()
        if enforce_canonical_order and ptr.encode() != data:
            raise MessageProtocolError("non-canonical Pointer encoding")
        return ptr


@dataclass(frozen=True)
class LegacyPointerV4:
    cid: str = ""
    target: int = 0
    kind: ContentKind = ContentKind.UNSPECIFIED
    schema: int = 0
    task_id: str = ""
    thread_id: str = ""
    context_id: str = ""
    flags: int = 0

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_string(1, self.cid),
                pb_varint(2, self.target),
                pb_varint(3, int(self.kind)),
                pb_varint(4, self.schema),
                pb_string(5, self.task_id),
                pb_string(6, self.thread_id),
                pb_string(7, self.context_id),
                pb_varint(8, self.flags),
            ]
        )

    @classmethod
    def decode(cls, data: bytes) -> "LegacyPointerV4":
        vals: dict[str, Any] = {
            "cid": "",
            "target": 0,
            "kind": ContentKind.UNSPECIFIED,
            "schema": 0,
            "task_id": "",
            "thread_id": "",
            "context_id": "",
            "flags": 0,
        }
        for fn, wt, val in decode_wire_fields(data):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                vals["cid"] = _decode_utf8(val, "LegacyPointerV4.cid")
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                vals["target"] = val
            elif fn == 3:
                _expect_wt(fn, wt, 0)
                vals["kind"] = ContentKind(val)
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["schema"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["task_id"] = _decode_utf8(val, "LegacyPointerV4.task_id")
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["thread_id"] = _decode_utf8(val, "LegacyPointerV4.thread_id")
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["context_id"] = _decode_utf8(val, "LegacyPointerV4.context_id")
            elif fn == 8:
                _expect_wt(fn, wt, 0)
                vals["flags"] = val
        return cls(**vals)


@dataclass(frozen=True)
class RecipientWrap:
    recipient_key_id: bytes = b""
    recipient_account: str = ""
    xwing_ciphertext: bytes = b""
    wrap_nonce: bytes = b""
    wrapped_cek: bytes = b""

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_bytes(1, self.recipient_key_id),
                pb_string(2, self.recipient_account),
                pb_bytes(3, self.xwing_ciphertext),
                pb_bytes(4, self.wrap_nonce),
                pb_bytes(5, self.wrapped_cek),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "RecipientWrap":
        vals: dict[str, Any] = {
            "recipient_key_id": b"",
            "recipient_account": "",
            "xwing_ciphertext": b"",
            "wrap_nonce": b"",
            "wrapped_cek": b"",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                vals["recipient_key_id"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["recipient_account"] = _decode_utf8(val, "RecipientWrap.recipient_account")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["xwing_ciphertext"] = val
            elif fn == 4:
                _expect_wt(fn, wt, 2)
                vals["wrap_nonce"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["wrapped_cek"] = val
        return cls(**vals)


@dataclass(frozen=True)
class XWingEncryptionHeader:
    cipher_suite: CipherSuite = CipherSuite.XWING_XCHACHA20_POLY1305
    aead_nonce: bytes = b""
    aad_context: bytes = b""
    chunk_index: int = 0
    kdf_info: str = DOMAIN_XWING_WRAP_INFO.decode("ascii")
    recipients: tuple[RecipientWrap, ...] = ()
    cek_commitment: bytes = b""

    def encode(self) -> bytes:
        parts = [
            pb_varint(1, int(self.cipher_suite)),
            pb_bytes(2, self.aead_nonce),
            pb_bytes(3, self.aad_context),
            pb_varint(4, self.chunk_index),
            pb_string(5, self.kdf_info),
        ]
        for r in self.recipients:
            parts.append(pb_bytes(6, r.encode()))
        if self.cek_commitment:
            parts.append(pb_bytes(7, self.cek_commitment))
        return b"".join(parts)

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "XWingEncryptionHeader":
        cipher_suite = CipherSuite.UNSPECIFIED
        aead_nonce = b""
        aad_context = b""
        chunk_index = 0
        kdf_info = ""
        recipients: list[RecipientWrap] = []
        cek_commitment = b""
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                cipher_suite = CipherSuite(val)
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                aead_nonce = val
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                aad_context = val
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                chunk_index = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                kdf_info = _decode_utf8(val, "XWingEncryptionHeader.kdf_info")
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                recipients.append(RecipientWrap.decode(val, enforce_canonical_order=enforce_canonical_order))
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                cek_commitment = val
        return cls(
            cipher_suite=cipher_suite,
            aead_nonce=aead_nonce,
            aad_context=aad_context,
            chunk_index=chunk_index,
            kdf_info=kdf_info,
            recipients=tuple(recipients),
            cek_commitment=cek_commitment,
        )


@dataclass(frozen=True)
class MessageKeyBinding:
    chain_id: str = ""
    account: str = ""
    xwing_public_key: bytes = b""
    key_id: bytes = b""
    created_at_unix_ms: int = 0
    key_epoch: int = 0
    signature_algorithm: SignatureAlgorithm = SignatureAlgorithm.ML_DSA_65
    account_pubkey: bytes = b""
    signature: bytes = b""

    def signing_preimage(self) -> bytes:
        return compute_key_binding_preimage(
            chain_id=self.chain_id,
            account=self.account,
            xwing_public_key=self.xwing_public_key,
            key_id=self.key_id,
            created_at_unix_ms=self.created_at_unix_ms,
            key_epoch=self.key_epoch,
        )

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_string(1, self.chain_id),
                pb_string(2, self.account),
                pb_bytes(3, self.xwing_public_key),
                pb_bytes(4, self.key_id),
                pb_varint(5, self.created_at_unix_ms),
                pb_varint(6, self.key_epoch),
                pb_varint(7, int(self.signature_algorithm)),
                pb_bytes(8, self.account_pubkey),
                pb_bytes(9, self.signature),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "MessageKeyBinding":
        vals: dict[str, Any] = {
            "chain_id": "",
            "account": "",
            "xwing_public_key": b"",
            "key_id": b"",
            "created_at_unix_ms": 0,
            "key_epoch": 0,
            "signature_algorithm": SignatureAlgorithm.UNSPECIFIED,
            "account_pubkey": b"",
            "signature": b"",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                vals["chain_id"] = _decode_utf8(val, "MessageKeyBinding.chain_id")
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["account"] = _decode_utf8(val, "MessageKeyBinding.account")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["xwing_public_key"] = val
            elif fn == 4:
                _expect_wt(fn, wt, 2)
                vals["key_id"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 0)
                vals["created_at_unix_ms"] = val
            elif fn == 6:
                _expect_wt(fn, wt, 0)
                vals["key_epoch"] = val
            elif fn == 7:
                _expect_wt(fn, wt, 0)
                vals["signature_algorithm"] = SignatureAlgorithm(val)
            elif fn == 8:
                _expect_wt(fn, wt, 2)
                vals["account_pubkey"] = val
            elif fn == 9:
                _expect_wt(fn, wt, 2)
                vals["signature"] = val
            else:
                raise MessageProtocolError(f"unknown MessageKeyBinding field {fn}")
        return cls(**vals)


@dataclass(frozen=True)
class FastPayPaymentRef:
    lane: PaymentLane = PaymentLane.UNSPECIFIED
    chain_id: str = ""
    order_hash: bytes = b""
    certificate_digest: bytes = b""
    tx_hash: bytes = b""
    asset_id: str = ""
    payer_account: str = ""
    payee_account: str = ""
    amount_atoms: int = 0
    fee_atoms: int = 0
    finalized_height: int = 0

    def dedup_key(self) -> bytes:
        """Canonical immutable deduplication key (order_hash for FastPay, tx_hash for consensus)."""
        if self.lane == PaymentLane.FASTPAY:
            if len(self.order_hash) != 32:
                raise MessageProtocolError("FastPayPaymentRef(FASTPAY) requires 32-byte order_hash")
            return self.order_hash
        if self.lane in (PaymentLane.CONSENSUS_PFT, PaymentLane.CONSENSUS_ISSUED):
            if len(self.tx_hash) != 32:
                raise MessageProtocolError("FastPayPaymentRef(CONSENSUS) requires 32-byte tx_hash")
            return self.tx_hash
        raise MessageProtocolError(f"unsupported PaymentLane for dedup key: {self.lane!r}")

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, int(self.lane)),
                pb_string(2, self.chain_id),
                pb_bytes(3, self.order_hash),
                pb_bytes(4, self.certificate_digest),
                pb_bytes(5, self.tx_hash),
                pb_string(6, self.asset_id),
                pb_string(7, self.payer_account),
                pb_string(8, self.payee_account),
                pb_varint(9, self.amount_atoms),
                pb_varint(10, self.fee_atoms),
                pb_varint(11, self.finalized_height),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "FastPayPaymentRef":
        vals: dict[str, Any] = {
            "lane": PaymentLane.UNSPECIFIED,
            "chain_id": "",
            "order_hash": b"",
            "certificate_digest": b"",
            "tx_hash": b"",
            "asset_id": "",
            "payer_account": "",
            "payee_account": "",
            "amount_atoms": 0,
            "fee_atoms": 0,
            "finalized_height": 0,
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                vals["lane"] = PaymentLane(val)
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["chain_id"] = _decode_utf8(val, "FastPayPaymentRef.chain_id")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["order_hash"] = val
            elif fn == 4:
                _expect_wt(fn, wt, 2)
                vals["certificate_digest"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["tx_hash"] = val
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["asset_id"] = _decode_utf8(val, "FastPayPaymentRef.asset_id")
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["payer_account"] = _decode_utf8(val, "FastPayPaymentRef.payer_account")
            elif fn == 8:
                _expect_wt(fn, wt, 2)
                vals["payee_account"] = _decode_utf8(val, "FastPayPaymentRef.payee_account")
            elif fn == 9:
                _expect_wt(fn, wt, 0)
                vals["amount_atoms"] = val
            elif fn == 10:
                _expect_wt(fn, wt, 0)
                vals["fee_atoms"] = val
            elif fn == 11:
                _expect_wt(fn, wt, 0)
                vals["finalized_height"] = val
        return cls(**vals)


@dataclass(frozen=True)
class X402PaymentRequired:
    x402_version: int = 2
    scheme: X402Scheme = X402Scheme.EXACT
    network: str = ""
    lane: PaymentLane = PaymentLane.FASTPAY
    asset_id: str = "PFT"
    max_amount_atoms: int = 0
    pay_to: str = ""
    request_hash: bytes = b""
    nonce: bytes = b""
    expires_at_unix_ms: int = 0
    resource_uri: str = ""
    facilitator_url: str = ""

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, self.x402_version),
                pb_varint(2, int(self.scheme)),
                pb_string(3, self.network),
                pb_varint(4, int(self.lane)),
                pb_string(5, self.asset_id),
                pb_varint(6, self.max_amount_atoms),
                pb_string(7, self.pay_to),
                pb_bytes(8, self.request_hash),
                pb_bytes(9, self.nonce),
                pb_varint(10, self.expires_at_unix_ms),
                pb_string(11, self.resource_uri),
                pb_string(12, self.facilitator_url),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "X402PaymentRequired":
        vals: dict[str, Any] = {
            "x402_version": 0,
            "scheme": X402Scheme.UNSPECIFIED,
            "network": "",
            "lane": PaymentLane.UNSPECIFIED,
            "asset_id": "",
            "max_amount_atoms": 0,
            "pay_to": "",
            "request_hash": b"",
            "nonce": b"",
            "expires_at_unix_ms": 0,
            "resource_uri": "",
            "facilitator_url": "",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                vals["x402_version"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                vals["scheme"] = X402Scheme(val)
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["network"] = _decode_utf8(val, "X402PaymentRequired.network")
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["lane"] = PaymentLane(val)
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["asset_id"] = _decode_utf8(val, "X402PaymentRequired.asset_id")
            elif fn == 6:
                _expect_wt(fn, wt, 0)
                vals["max_amount_atoms"] = val
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["pay_to"] = _decode_utf8(val, "X402PaymentRequired.pay_to")
            elif fn == 8:
                _expect_wt(fn, wt, 2)
                vals["request_hash"] = val
            elif fn == 9:
                _expect_wt(fn, wt, 2)
                vals["nonce"] = val
            elif fn == 10:
                _expect_wt(fn, wt, 0)
                vals["expires_at_unix_ms"] = val
            elif fn == 11:
                _expect_wt(fn, wt, 2)
                vals["resource_uri"] = _decode_utf8(val, "X402PaymentRequired.resource_uri")
            elif fn == 12:
                _expect_wt(fn, wt, 2)
                vals["facilitator_url"] = _decode_utf8(val, "X402PaymentRequired.facilitator_url")
            else:
                raise MessageProtocolError(f"unknown X402PaymentRequired field {fn}")
        return cls(**vals)


@dataclass(frozen=True)
class X402PaymentSignature:
    x402_version: int = 2
    scheme: X402Scheme = X402Scheme.EXACT
    network: str = ""
    lane: PaymentLane = PaymentLane.FASTPAY
    request_hash: bytes = b""
    nonce: bytes = b""
    order_hash: bytes = b""
    tx_hash: bytes = b""
    signed_payload: bytes = b""
    certificate_payload: bytes = b""

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, self.x402_version),
                pb_varint(2, int(self.scheme)),
                pb_string(3, self.network),
                pb_varint(4, int(self.lane)),
                pb_bytes(5, self.request_hash),
                pb_bytes(6, self.nonce),
                pb_bytes(7, self.order_hash),
                pb_bytes(8, self.tx_hash),
                pb_bytes(9, self.signed_payload),
                pb_bytes(10, self.certificate_payload),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "X402PaymentSignature":
        vals: dict[str, Any] = {
            "x402_version": 0,
            "scheme": X402Scheme.UNSPECIFIED,
            "network": "",
            "lane": PaymentLane.UNSPECIFIED,
            "request_hash": b"",
            "nonce": b"",
            "order_hash": b"",
            "tx_hash": b"",
            "signed_payload": b"",
            "certificate_payload": b"",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                vals["x402_version"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                vals["scheme"] = X402Scheme(val)
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["network"] = _decode_utf8(val, "X402PaymentSignature.network")
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["lane"] = PaymentLane(val)
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["request_hash"] = val
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["nonce"] = val
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["order_hash"] = val
            elif fn == 8:
                _expect_wt(fn, wt, 2)
                vals["tx_hash"] = val
            elif fn == 9:
                _expect_wt(fn, wt, 2)
                vals["signed_payload"] = val
            elif fn == 10:
                _expect_wt(fn, wt, 2)
                vals["certificate_payload"] = val
            else:
                raise MessageProtocolError(f"unknown X402PaymentSignature field {fn}")
        return cls(**vals)


@dataclass(frozen=True)
class X402PaymentResponse:
    x402_version: int = 2
    settled: bool = False
    payment_ref: FastPayPaymentRef | None = None
    settled_at_unix_ms: int = 0
    facilitator_id: str = ""

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, self.x402_version),
                pb_bool(2, self.settled),
                pb_bytes(3, self.payment_ref.encode() if self.payment_ref else b""),
                pb_varint(4, self.settled_at_unix_ms),
                pb_string(5, self.facilitator_id),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False, _inside_signed_header: bool = False) -> "X402PaymentResponse":
        x402_version = 0
        settled = False
        payment_ref = None
        settled_at_unix_ms = 0
        facilitator_id = ""
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                x402_version = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                settled = bool(val)
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                payment_ref = FastPayPaymentRef.decode(val, enforce_canonical_order=enforce_canonical_order)
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                settled_at_unix_ms = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                facilitator_id = _decode_utf8(val, "X402PaymentResponse.facilitator_id")
            elif not _inside_signed_header:
                raise MessageProtocolError(f"unknown X402PaymentResponse field {fn}")
        return cls(
            x402_version=x402_version,
            settled=settled,
            payment_ref=payment_ref,
            settled_at_unix_ms=settled_at_unix_ms,
            facilitator_id=facilitator_id,
        )


@dataclass(frozen=True)
class MessageHeader:
    protocol_version: int = 1
    chain_id: str = ""
    schema_name: str = ""
    schema_id: int = 0
    kind: ContentKind = ContentKind.UNSPECIFIED
    visibility: Visibility = Visibility.UNSPECIFIED
    body_encoding: BodyEncoding = BodyEncoding.PROTOBUF
    cipher_suite: CipherSuite = CipherSuite.NONE
    sender_account: str = ""
    created_at_unix_ms: int = 0
    task_id: str = ""
    thread_id: str = ""
    context_id: str = ""
    reply_to_cid: IpfsCidRef | None = None
    payment_ref: FastPayPaymentRef | None = None
    x402_receipt: X402PaymentResponse | None = None

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, self.protocol_version),
                pb_string(2, self.chain_id),
                pb_string(3, self.schema_name),
                pb_varint(4, self.schema_id),
                pb_varint(5, int(self.kind)),
                pb_varint(6, int(self.visibility)),
                pb_varint(7, int(self.body_encoding)),
                pb_varint(8, int(self.cipher_suite)),
                pb_string(9, self.sender_account),
                pb_varint(10, self.created_at_unix_ms),
                pb_string(11, self.task_id),
                pb_string(12, self.thread_id),
                pb_string(13, self.context_id),
                pb_bytes(14, self.reply_to_cid.encode() if self.reply_to_cid else b""),
                pb_bytes(15, self.payment_ref.encode() if self.payment_ref else b""),
                pb_bytes(16, self.x402_receipt.encode() if self.x402_receipt else b""),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "MessageHeader":
        vals: dict[str, Any] = {
            "protocol_version": 0,
            "chain_id": "",
            "schema_name": "",
            "schema_id": 0,
            "kind": ContentKind.UNSPECIFIED,
            "visibility": Visibility.UNSPECIFIED,
            "body_encoding": BodyEncoding.UNSPECIFIED,
            "cipher_suite": CipherSuite.UNSPECIFIED,
            "sender_account": "",
            "created_at_unix_ms": 0,
            "task_id": "",
            "thread_id": "",
            "context_id": "",
            "reply_to_cid": None,
            "payment_ref": None,
            "x402_receipt": None,
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                vals["protocol_version"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["chain_id"] = _decode_utf8(val, "MessageHeader.chain_id")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["schema_name"] = _decode_utf8(val, "MessageHeader.schema_name")
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["schema_id"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 0)
                vals["kind"] = ContentKind(val)
            elif fn == 6:
                _expect_wt(fn, wt, 0)
                vals["visibility"] = Visibility(val)
            elif fn == 7:
                _expect_wt(fn, wt, 0)
                vals["body_encoding"] = BodyEncoding(val)
            elif fn == 8:
                _expect_wt(fn, wt, 0)
                vals["cipher_suite"] = CipherSuite(val)
            elif fn == 9:
                _expect_wt(fn, wt, 2)
                vals["sender_account"] = _decode_utf8(val, "MessageHeader.sender_account")
            elif fn == 10:
                _expect_wt(fn, wt, 0)
                vals["created_at_unix_ms"] = val
            elif fn == 11:
                _expect_wt(fn, wt, 2)
                vals["task_id"] = _decode_utf8(val, "MessageHeader.task_id")
            elif fn == 12:
                _expect_wt(fn, wt, 2)
                vals["thread_id"] = _decode_utf8(val, "MessageHeader.thread_id")
            elif fn == 13:
                _expect_wt(fn, wt, 2)
                vals["context_id"] = _decode_utf8(val, "MessageHeader.context_id")
            elif fn == 14:
                _expect_wt(fn, wt, 2)
                vals["reply_to_cid"] = IpfsCidRef.decode(val, enforce_canonical_order=enforce_canonical_order)
            elif fn == 15:
                _expect_wt(fn, wt, 2)
                vals["payment_ref"] = FastPayPaymentRef.decode(val, enforce_canonical_order=enforce_canonical_order)
            elif fn == 16:
                _expect_wt(fn, wt, 2)
                vals["x402_receipt"] = X402PaymentResponse.decode(
                    val, enforce_canonical_order=enforce_canonical_order, _inside_signed_header=True
                )
        return cls(**vals)


@dataclass(frozen=True)
class Envelope:
    version: int = 1
    header_bytes: bytes = b""
    encryption_header_bytes: bytes = b""
    body_bytes: bytes = b""
    signature_algorithm: SignatureAlgorithm = SignatureAlgorithm.ML_DSA_65
    sender_pubkey: bytes = b""
    signature: bytes = b""

    def signing_preimage(self) -> bytes:
        """Exact raw-byte preimage (zero re-serialization rule)."""
        return envelope_signing_preimage(
            self.version,
            self.header_bytes,
            self.encryption_header_bytes,
            self.body_bytes,
        )

    def encode(self) -> bytes:
        encoded = b"".join(
            [
                pb_varint(1, self.version),
                pb_bytes(2, self.header_bytes),
                pb_bytes(3, self.encryption_header_bytes),
                pb_bytes(4, self.body_bytes),
                pb_varint(5, int(self.signature_algorithm)),
                pb_bytes(6, self.sender_pubkey),
                pb_bytes(7, self.signature),
            ]
        )
        if len(encoded) > MAX_SINGLE_ENVELOPE_BYTES:
            raise MessageProtocolError(
                f"Envelope size {len(encoded)} exceeds {MAX_SINGLE_ENVELOPE_BYTES} bytes (use Manifest)"
            )
        return encoded

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "Envelope":
        if len(data) > MAX_SINGLE_ENVELOPE_BYTES:
            raise MessageProtocolError(
                f"Envelope size {len(data)} exceeds {MAX_SINGLE_ENVELOPE_BYTES} bytes"
            )
        vals: dict[str, Any] = {
            "version": 0,
            "header_bytes": b"",
            "encryption_header_bytes": b"",
            "body_bytes": b"",
            "signature_algorithm": SignatureAlgorithm.UNSPECIFIED,
            "sender_pubkey": b"",
            "signature": b"",
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                vals["version"] = val
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["header_bytes"] = val
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["encryption_header_bytes"] = val
            elif fn == 4:
                _expect_wt(fn, wt, 2)
                vals["body_bytes"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 0)
                vals["signature_algorithm"] = SignatureAlgorithm(val)
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["sender_pubkey"] = val
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["signature"] = val
            else:
                raise MessageProtocolError(f"unknown Envelope field {fn}")
        return cls(**vals)


@dataclass(frozen=True)
class ManifestChunk:
    chunk_index: int = 0
    cid: IpfsCidRef = field(default_factory=IpfsCidRef)
    aead_nonce: bytes = b""

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_varint(1, self.chunk_index),
                pb_bytes(2, self.cid.encode()),
                pb_bytes(3, self.aead_nonce),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "ManifestChunk":
        chunk_index = 0
        cid = IpfsCidRef()
        aead_nonce = b""
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                chunk_index = val
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                cid = IpfsCidRef.decode(val, enforce_canonical_order=enforce_canonical_order)
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                aead_nonce = val
        return cls(chunk_index=chunk_index, cid=cid, aead_nonce=aead_nonce)


@dataclass(frozen=True)
class Manifest:
    version: int = 1
    total_plaintext_bytes: int = 0
    chunk_size_bytes: int = MAX_SINGLE_ENVELOPE_BYTES
    media_type: str = ""
    inner_schema_name: str = ""
    inner_body_encoding: BodyEncoding = BodyEncoding.RAW_BYTES
    cipher_suite: CipherSuite = CipherSuite.XWING_XCHACHA20_POLY1305
    chunks: tuple[ManifestChunk, ...] = ()

    def validate(self) -> None:
        if self.total_plaintext_bytes > MAX_MANIFEST_PLAINTEXT_BYTES:
            raise MessageProtocolError(
                f"Manifest total_plaintext_bytes {self.total_plaintext_bytes} exceeds 16 MiB cap"
            )
        if self.chunk_size_bytes == 0 or self.chunk_size_bytes > MAX_SINGLE_ENVELOPE_BYTES:
            raise MessageProtocolError(
                f"Manifest chunk_size_bytes {self.chunk_size_bytes} outside 1..{MAX_SINGLE_ENVELOPE_BYTES}"
            )
        for expected_idx, chunk in enumerate(self.chunks):
            if chunk.chunk_index != expected_idx:
                raise MessageProtocolError(
                    f"Manifest chunks must be strictly ascending 0..N-1; expected {expected_idx}, got {chunk.chunk_index}"
                )

    def encode(self) -> bytes:
        self.validate()
        parts = [
            pb_varint(1, self.version),
            pb_varint(2, self.total_plaintext_bytes),
            pb_varint(3, self.chunk_size_bytes),
            pb_string(4, self.media_type),
            pb_string(5, self.inner_schema_name),
            pb_varint(6, int(self.inner_body_encoding)),
            pb_varint(7, int(self.cipher_suite)),
        ]
        for chunk in self.chunks:
            parts.append(pb_bytes(8, chunk.encode()))
        return b"".join(parts)

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "Manifest":
        version = 0
        total_plaintext_bytes = 0
        chunk_size_bytes = 0
        media_type = ""
        inner_schema_name = ""
        inner_body_encoding = BodyEncoding.UNSPECIFIED
        cipher_suite = CipherSuite.UNSPECIFIED
        chunks: list[ManifestChunk] = []
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 0)
                version = val
            elif fn == 2:
                _expect_wt(fn, wt, 0)
                total_plaintext_bytes = val
            elif fn == 3:
                _expect_wt(fn, wt, 0)
                chunk_size_bytes = val
            elif fn == 4:
                _expect_wt(fn, wt, 2)
                media_type = _decode_utf8(val, "Manifest.media_type")
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                inner_schema_name = _decode_utf8(val, "Manifest.inner_schema_name")
            elif fn == 6:
                _expect_wt(fn, wt, 0)
                inner_body_encoding = BodyEncoding(val)
            elif fn == 7:
                _expect_wt(fn, wt, 0)
                cipher_suite = CipherSuite(val)
            elif fn == 8:
                _expect_wt(fn, wt, 2)
                chunks.append(ManifestChunk.decode(val, enforce_canonical_order=enforce_canonical_order))
        manifest = cls(
            version=version,
            total_plaintext_bytes=total_plaintext_bytes,
            chunk_size_bytes=chunk_size_bytes,
            media_type=media_type,
            inner_schema_name=inner_schema_name,
            inner_body_encoding=inner_body_encoding,
            cipher_suite=cipher_suite,
            chunks=tuple(chunks),
        )
        manifest.validate()
        return manifest


@dataclass(frozen=True)
class TextMessage:
    text: str = ""
    format: str = ""
    attachments: tuple[IpfsCidRef, ...] = ()

    def encode(self) -> bytes:
        parts = [pb_string(1, self.text), pb_string(2, self.format)]
        for att in self.attachments:
            parts.append(pb_bytes(3, att.encode()))
        return b"".join(parts)

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "TextMessage":
        text = ""
        fmt = ""
        attachments: list[IpfsCidRef] = []
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                text = _decode_utf8(val, "TextMessage.text")
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                fmt = _decode_utf8(val, "TextMessage.format")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                attachments.append(IpfsCidRef.decode(val, enforce_canonical_order=enforce_canonical_order))
        return cls(text=text, format=fmt, attachments=tuple(attachments))


@dataclass(frozen=True)
class RewardNotice:
    reward_id: str = ""
    task_id: str = ""
    recipient_account: str = ""
    amount_atoms: int = 0
    asset_id: str = "PFT"
    summary: str = ""
    payment_ref: FastPayPaymentRef | None = None

    def encode(self) -> bytes:
        return b"".join(
            [
                pb_string(1, self.reward_id),
                pb_string(2, self.task_id),
                pb_string(3, self.recipient_account),
                pb_varint(4, self.amount_atoms),
                pb_string(5, self.asset_id),
                pb_string(6, self.summary),
                pb_bytes(7, self.payment_ref.encode() if self.payment_ref else b""),
            ]
        )

    @classmethod
    def decode(cls, data: bytes, *, enforce_canonical_order: bool = False) -> "RewardNotice":
        vals: dict[str, Any] = {
            "reward_id": "",
            "task_id": "",
            "recipient_account": "",
            "amount_atoms": 0,
            "asset_id": "",
            "summary": "",
            "payment_ref": None,
        }
        for fn, wt, val in decode_wire_fields(data, enforce_canonical_order=enforce_canonical_order):
            if fn == 1:
                _expect_wt(fn, wt, 2)
                vals["reward_id"] = _decode_utf8(val, "RewardNotice.reward_id")
            elif fn == 2:
                _expect_wt(fn, wt, 2)
                vals["task_id"] = _decode_utf8(val, "RewardNotice.task_id")
            elif fn == 3:
                _expect_wt(fn, wt, 2)
                vals["recipient_account"] = _decode_utf8(val, "RewardNotice.recipient_account")
            elif fn == 4:
                _expect_wt(fn, wt, 0)
                vals["amount_atoms"] = val
            elif fn == 5:
                _expect_wt(fn, wt, 2)
                vals["asset_id"] = _decode_utf8(val, "RewardNotice.asset_id")
            elif fn == 6:
                _expect_wt(fn, wt, 2)
                vals["summary"] = _decode_utf8(val, "RewardNotice.summary")
            elif fn == 7:
                _expect_wt(fn, wt, 2)
                vals["payment_ref"] = FastPayPaymentRef.decode(val, enforce_canonical_order=enforce_canonical_order)
        return cls(**vals)


# ---------------------------------------------------------------------------
# Memo & x402 HTTP header helpers
# ---------------------------------------------------------------------------

def encode_pointer_v5_memo(pointer: Pointer) -> dict[str, str]:
    """Encode a Pointer into an L1 PaymentMemo dict (strictly lowercase hex, <= 256B)."""
    memo_data_hex = pointer.encode().hex()
    validate_l1_payment_memo(MEMO_TYPE_PF_PTR_HEX, MEMO_FORMAT_V5_HEX, memo_data_hex)
    return {
        "memo_type": MEMO_TYPE_PF_PTR_HEX,
        "memo_format": MEMO_FORMAT_V5_HEX,
        "memo_data": memo_data_hex,
    }


def decode_pointer_v5_memo(memo: dict[str, str], *, enforce_canonical_order: bool = False) -> Pointer:
    memo_type = memo.get("memo_type", "")
    memo_format = memo.get("memo_format", "")
    memo_data = memo.get("memo_data", "")
    validate_l1_payment_memo(memo_type, memo_format, memo_data)
    if memo_type != MEMO_TYPE_PF_PTR_HEX or memo_format != MEMO_FORMAT_V5_HEX:
        raise MessageProtocolError(f"not a pf.ptr v5 memo: type={memo_type} format={memo_format}")
    return Pointer.decode(bytes.fromhex(memo_data), enforce_canonical_order=enforce_canonical_order)


def decode_legacy_pointer_v4_memo(memo: dict[str, str]) -> LegacyPointerV4:
    """Decode historical old-chain pf.ptr v4 memo (accepts uppercase or lowercase hex)."""
    memo_type = memo.get("memo_type", "").lower()
    memo_format = memo.get("memo_format", "").lower()
    memo_data = memo.get("memo_data", "")
    if memo_type != MEMO_TYPE_PF_PTR_HEX or memo_format != MEMO_FORMAT_V4_HEX:
        raise MessageProtocolError(f"not a pf.ptr v4 memo: type={memo_type} format={memo_format}")
    try:
        raw = bytes.fromhex(memo_data)
    except ValueError as exc:
        raise MessageProtocolError("invalid hex in legacy pf.ptr v4 memo_data") from exc
    return LegacyPointerV4.decode(raw)


def encode_x402_header(raw_protobuf_bytes: bytes) -> str:
    """Encode canonical x402 protobuf bytes into unpadded base64url HTTP header text."""
    return base64.urlsafe_b64encode(raw_protobuf_bytes).decode("ascii").rstrip("=")


def decode_x402_header(header_value: str) -> bytes:
    """Decode unpadded base64url HTTP x402 header text into raw bytes."""
    if not isinstance(header_value, str) or not header_value.strip():
        raise MessageProtocolError("x402 header value must be non-empty")
    token = header_value.strip()
    pad = "=" * ((-len(token)) % 4)
    try:
        return base64.urlsafe_b64decode(token + pad)
    except Exception as exc:
        raise MessageProtocolError("invalid base64url x402 header") from exc


# ---------------------------------------------------------------------------
# High-level PQ envelope framing (X-Wing + XChaCha20-Poly1305 + ML-DSA-65)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class RecipientSpec:
    xwing_public_key: bytes
    account: str = ""


@dataclass(frozen=True)
class SealedEnvelopeResult:
    envelope: Envelope
    envelope_bytes: bytes
    cid_bin: bytes
    multibase_cid: str
    pointer: Pointer
    memo: dict[str, str]


@dataclass(frozen=True)
class OpenedEnvelopeResult:
    envelope: Envelope
    header: MessageHeader
    encryption_header: XWingEncryptionHeader | None
    plaintext_body: bytes
    cid_ref: IpfsCidRef


def create_message_key_binding(
    *,
    chain_id: str,
    account: str,
    xwing_public_key: bytes,
    created_at_unix_ms: int,
    key_epoch: int,
    account_pubkey: bytes,
    sign_fn: Callable[[bytes], bytes],
) -> MessageKeyBinding:
    """Create and sign a MessageKeyBinding (epoch 0 = Day-0 seed bootstrap; epoch >= 1 = rotatable forward-secret key)."""
    if len(xwing_public_key) != XWING_PUBKEY_BYTES:
        raise MessageProtocolError(f"xwing_public_key must be {XWING_PUBKEY_BYTES} bytes")
    if account != derive_account_address(account_pubkey):
        raise MessageProtocolError("account does not match derive_account_address(account_pubkey)")
    key_id = compute_recipient_key_id(xwing_public_key)
    preimage = compute_key_binding_preimage(
        chain_id=chain_id,
        account=account,
        xwing_public_key=xwing_public_key,
        key_id=key_id,
        created_at_unix_ms=created_at_unix_ms,
        key_epoch=key_epoch,
    )
    signature = sign_fn(preimage)
    return MessageKeyBinding(
        chain_id=chain_id,
        account=account,
        xwing_public_key=xwing_public_key,
        key_id=key_id,
        created_at_unix_ms=created_at_unix_ms,
        key_epoch=key_epoch,
        signature_algorithm=SignatureAlgorithm.ML_DSA_65,
        account_pubkey=account_pubkey,
        signature=signature,
    )


def rotate_message_key_binding(
    *,
    chain_id: str,
    account: str,
    new_xwing_public_key: bytes,
    previous_key_epoch: int,
    created_at_unix_ms: int,
    account_pubkey: bytes,
    sign_fn: Callable[[bytes], bytes],
    new_key_epoch: int | None = None,
) -> MessageKeyBinding:
    """Rotate to a strictly higher epoch (>= 1) non-seed-derived X-Wing key for epoch forward secrecy."""
    target_epoch = (previous_key_epoch + 1) if new_key_epoch is None else new_key_epoch
    if target_epoch < 1 or target_epoch <= previous_key_epoch:
        raise MessageProtocolError(
            f"rotated key_epoch ({target_epoch}) must be >= 1 and strictly greater than previous_key_epoch ({previous_key_epoch})"
        )
    return create_message_key_binding(
        chain_id=chain_id,
        account=account,
        xwing_public_key=new_xwing_public_key,
        created_at_unix_ms=created_at_unix_ms,
        key_epoch=target_epoch,
        account_pubkey=account_pubkey,
        sign_fn=sign_fn,
    )


def verify_message_key_binding(
    binding_or_bytes: MessageKeyBinding | bytes,
    *,
    verify_signature_fn: Callable[[bytes, bytes, bytes], bool],
    expected_chain_id: str | None = None,
    expected_account: str | None = None,
    min_known_epoch: int = 0,
    enforce_canonical_order: bool = False,
) -> MessageKeyBinding:
    """Verify MessageKeyBinding signature, address derivation, chain_id, and monotonic key_epoch anti-downgrade."""
    binding = (
        MessageKeyBinding.decode(binding_or_bytes, enforce_canonical_order=enforce_canonical_order)
        if isinstance(binding_or_bytes, (bytes, bytearray))
        else binding_or_bytes
    )
    if binding.signature_algorithm != SignatureAlgorithm.ML_DSA_65:
        raise MessageProtocolError("MessageKeyBinding requires ML-DSA-65")
    if len(binding.xwing_public_key) != XWING_PUBKEY_BYTES:
        raise MessageProtocolError(f"MessageKeyBinding xwing_public_key must be {XWING_PUBKEY_BYTES} bytes")
    expected_key_id = compute_recipient_key_id(binding.xwing_public_key)
    if len(binding.key_id) != 32 or not hmac.compare_digest(binding.key_id, expected_key_id):
        raise MessageProtocolError("MessageKeyBinding key_id does not match SHA-256(xwing_public_key)")
    derived = derive_account_address(binding.account_pubkey)
    if binding.account != derived:
        raise MessageProtocolError(
            f"MessageKeyBinding account_pubkey address {derived} != binding.account {binding.account}"
        )
    if expected_chain_id is not None and binding.chain_id != expected_chain_id:
        raise MessageProtocolError(
            f"MessageKeyBinding chain_id mismatch: got {binding.chain_id!r}, expected {expected_chain_id!r}"
        )
    if expected_account is not None and binding.account != expected_account:
        raise MessageProtocolError(
            f"MessageKeyBinding account mismatch: got {binding.account!r}, expected {expected_account!r}"
        )
    if binding.key_epoch < min_known_epoch:
        raise MessageProtocolError(
            f"MessageKeyBinding key_epoch rollback rejected: got {binding.key_epoch} < min_known_epoch {min_known_epoch}"
        )
    if not verify_signature_fn(binding.signing_preimage(), binding.signature, binding.account_pubkey):
        raise MessageProtocolError("MessageKeyBinding ML-DSA-65 signature invalid")
    return binding


def seal_envelope(
    *,
    header: MessageHeader,
    body_plaintext: bytes,
    sender_pubkey: bytes,
    sign_fn: Callable[[bytes], bytes],
    recipients: Sequence[RecipientSpec] = (),
    xwing_encap_fn: Callable[[bytes], tuple[bytes, bytes]] | None = None,
    cek: bytes | None = None,
    aead_nonce: bytes | None = None,
    wrap_nonce_fn: Callable[[], bytes] | None = None,
    chunk_index: int = 0,
    pointer_target: PointerTarget = PointerTarget.CONTENT_BLOB,
    extra_pointer_flags: int = 0,
    blind_sealed_recipients: bool = True,
    emit_cek_commitment: bool = True,
) -> SealedEnvelopeResult:
    """Frame, optionally X-Wing encrypt, and ML-DSA-65 sign a canonical pf.message.v1 Envelope.

    ``sign_fn`` receives the exact raw-byte ``envelope_signing_preimage`` and returns
    the ML-DSA-65 signature bytes. For ``PRIVATE`` and ``SEALED`` visibility,
    ``xwing_encap_fn(xwing_public_key)`` must return ``(xwing_ciphertext, shared_secret_32b)``.
    """
    if header.protocol_version != 1:
        raise MessageProtocolError("MessageHeader.protocol_version must be 1")
    if header.visibility not in (Visibility.PUBLIC, Visibility.PRIVATE, Visibility.SEALED):
        raise MessageProtocolError(f"invalid visibility: {header.visibility!r}")

    header_bytes = header.encode()
    if header.visibility == Visibility.PUBLIC:
        if header.cipher_suite != CipherSuite.NONE:
            raise MessageProtocolError("VISIBILITY_PUBLIC requires CIPHER_SUITE_NONE")
        if recipients:
            raise MessageProtocolError("VISIBILITY_PUBLIC cannot include encrypted recipients")
        encryption_header_bytes = b""
        body_bytes = body_plaintext
        vis_flag = int(PointerFlag.PUBLIC)
    else:
        if header.cipher_suite != CipherSuite.XWING_XCHACHA20_POLY1305:
            raise MessageProtocolError("PRIVATE/SEALED requires CIPHER_SUITE_XWING_XCHACHA20_POLY1305")
        if not recipients:
            raise MessageProtocolError("PRIVATE/SEALED envelope requires at least one recipient")
        if xwing_encap_fn is None:
            raise MessageProtocolError("xwing_encap_fn is required for PRIVATE/SEALED envelopes")

        content_key = cek if cek is not None else os.urandom(AEAD_KEY_BYTES)
        body_nonce = aead_nonce if aead_nonce is not None else os.urandom(AEAD_NONCE_BYTES)
        aad_context = compute_aad_context(header.chain_id, header_bytes, chunk_index)
        cek_commitment = compute_cek_commitment(content_key, aad_context) if emit_cek_commitment else b""
        body_bytes = xchacha20_poly1305_encrypt(content_key, body_nonce, body_plaintext, aad_context)

        wraps_with_id: list[tuple[bytes, RecipientWrap]] = []
        seen_static_ids: set[bytes] = set()
        for recipient in recipients:
            static_key_id = compute_recipient_key_id(recipient.xwing_public_key)
            if static_key_id in seen_static_ids:
                raise MessageProtocolError("duplicate recipient_key_id in envelope")
            seen_static_ids.add(static_key_id)
            xwing_ct, shared_secret = xwing_encap_fn(recipient.xwing_public_key)
            wire_tag = (
                compute_sealed_recipient_tag(shared_secret, aad_context)
                if (header.visibility == Visibility.SEALED and blind_sealed_recipients)
                else static_key_id
            )
            kek = derive_xwing_wrap_kek(shared_secret, aad_context, wire_tag)
            w_nonce = wrap_nonce_fn() if wrap_nonce_fn is not None else os.urandom(AEAD_NONCE_BYTES)
            wrapped_cek = xchacha20_poly1305_encrypt(kek, w_nonce, content_key, aad_context)
            recip_account = "" if header.visibility == Visibility.SEALED else recipient.account
            wraps_with_id.append(
                (
                    wire_tag,
                    RecipientWrap(
                        recipient_key_id=wire_tag,
                        recipient_account=recip_account,
                        xwing_ciphertext=xwing_ct,
                        wrap_nonce=w_nonce,
                        wrapped_cek=wrapped_cek,
                    ),
                )
            )
        wraps_with_id.sort(key=lambda pair: pair[0])
        enc_header = XWingEncryptionHeader(
            cipher_suite=CipherSuite.XWING_XCHACHA20_POLY1305,
            aead_nonce=body_nonce,
            aad_context=aad_context,
            chunk_index=chunk_index,
            kdf_info=DOMAIN_XWING_WRAP_INFO.decode("ascii"),
            recipients=tuple(w for _, w in wraps_with_id),
            cek_commitment=cek_commitment,
        )
        encryption_header_bytes = enc_header.encode()
        vis_flag = int(PointerFlag.SEALED if header.visibility == Visibility.SEALED else PointerFlag.PRIVATE)

    preimage = envelope_signing_preimage(1, header_bytes, encryption_header_bytes, body_bytes)
    signature = sign_fn(preimage)
    envelope = Envelope(
        version=1,
        header_bytes=header_bytes,
        encryption_header_bytes=encryption_header_bytes,
        body_bytes=body_bytes,
        signature_algorithm=SignatureAlgorithm.ML_DSA_65,
        sender_pubkey=sender_pubkey,
        signature=signature,
    )
    envelope_bytes = envelope.encode()
    cid_bin, multibase_cid = cidv1_raw_from_bytes(envelope_bytes)

    ptr_flags = vis_flag | extra_pointer_flags
    if pointer_target == PointerTarget.MANIFEST:
        ptr_flags |= int(PointerFlag.MULTIPART)
    pointer = Pointer(
        cid_bin=cid_bin,
        target=pointer_target,
        kind=header.kind,
        schema=header.schema_id,
        task_id=header.task_id,
        thread_id=header.thread_id,
        context_id=header.context_id,
        flags=ptr_flags,
        schema_name=header.schema_name if header.schema_id == SCHEMA_ID_CUSTOM else "",
    )
    memo = encode_pointer_v5_memo(pointer)
    return SealedEnvelopeResult(
        envelope=envelope,
        envelope_bytes=envelope_bytes,
        cid_bin=cid_bin,
        multibase_cid=multibase_cid,
        pointer=pointer,
        memo=memo,
    )


def open_envelope(
    envelope_bytes: bytes,
    *,
    verify_signature_fn: Callable[[bytes, bytes, bytes], bool],
    expected_cid_bin: bytes | None = None,
    expected_multibase_cid: str | None = None,
    expected_chain_id: str | None = None,
    expected_tx_sender: str | None = None,
    verify_sender_address: bool = True,
    recipient_key_id: bytes | None = None,
    xwing_decap_fn: Callable[[bytes], bytes] | None = None,
    enforce_canonical_order: bool = False,
    require_cek_commitment: bool = False,
) -> OpenedEnvelopeResult:
    """Verify CID + raw-byte ML-DSA-65 signature BEFORE decoding nested fields, enforce anchor/sender bindings, then decrypt if private/sealed."""
    # Step 1: Verify CIDv1 raw over exact transmitted bytes (zero re-serialization).
    cid_ref = verify_cidv1_raw(
        envelope_bytes,
        expected_cid_bin=expected_cid_bin,
        expected_multibase_cid=expected_multibase_cid,
    )
    # Step 2: Decode outer Envelope only and verify raw-byte signature BEFORE decoding header/body.
    envelope = Envelope.decode(envelope_bytes, enforce_canonical_order=enforce_canonical_order)
    if envelope.version != 1:
        raise MessageProtocolError(f"unsupported Envelope version: {envelope.version}")
    if envelope.signature_algorithm != SignatureAlgorithm.ML_DSA_65:
        raise MessageProtocolError(f"unsupported signature_algorithm: {envelope.signature_algorithm!r}")
    preimage = envelope.signing_preimage()
    if not verify_signature_fn(preimage, envelope.signature, envelope.sender_pubkey):
        raise MessageProtocolError("Envelope raw-byte signature verification failed")

    # Step 3: Decode nested MessageHeader only after signature verification succeeded.
    header = MessageHeader.decode(envelope.header_bytes, enforce_canonical_order=enforce_canonical_order)
    if header.protocol_version != 1:
        raise MessageProtocolError(f"unsupported MessageHeader.protocol_version: {header.protocol_version}")

    # Step 4: Enforce sender-key derivation, chain_id, and anchoring tx.from bindings.
    if verify_sender_address:
        derived_sender = derive_account_address(envelope.sender_pubkey)
        if derived_sender != header.sender_account:
            raise MessageProtocolError(
                f"Envelope sender_pubkey address {derived_sender} != header.sender_account {header.sender_account}"
            )
    if expected_chain_id is not None and header.chain_id != expected_chain_id:
        raise MessageProtocolError(
            f"Envelope chain_id mismatch: got {header.chain_id!r}, expected {expected_chain_id!r}"
        )
    if expected_tx_sender is not None and header.sender_account != expected_tx_sender:
        raise MessageProtocolError(
            f"Foreign re-anchor rejected: header.sender_account {header.sender_account} != anchoring tx.from {expected_tx_sender}"
        )

    if header.visibility == Visibility.PUBLIC:
        if envelope.encryption_header_bytes:
            raise MessageProtocolError("VISIBILITY_PUBLIC envelope must have empty encryption_header_bytes")
        return OpenedEnvelopeResult(
            envelope=envelope,
            header=header,
            encryption_header=None,
            plaintext_body=envelope.body_bytes,
            cid_ref=cid_ref,
        )

    if not envelope.encryption_header_bytes:
        raise MessageProtocolError("PRIVATE/SEALED envelope missing encryption_header_bytes")
    enc_header = XWingEncryptionHeader.decode(
        envelope.encryption_header_bytes,
        enforce_canonical_order=enforce_canonical_order,
    )
    if header.visibility == Visibility.SEALED:
        for wrap in enc_header.recipients:
            if wrap.recipient_account:
                raise MessageProtocolError("VISIBILITY_SEALED must omit RecipientWrap.recipient_account")

    expected_aad = compute_aad_context(header.chain_id, envelope.header_bytes, enc_header.chunk_index)
    if not hmac.compare_digest(enc_header.aad_context, expected_aad):
        raise MessageProtocolError("XWingEncryptionHeader.aad_context mismatch")

    if xwing_decap_fn is None:
        raise MessageProtocolError("xwing_decap_fn is required to decrypt PRIVATE/SEALED")

    kdf_info = enc_header.kdf_info or DOMAIN_XWING_WRAP_INFO.decode("ascii")
    matching_wrap = None
    shared_secret: bytes | None = None

    if recipient_key_id is not None:
        matching_wrap = next(
            (w for w in enc_header.recipients if hmac.compare_digest(w.recipient_key_id, recipient_key_id)),
            None,
        )

    if matching_wrap is not None:
        shared_secret = xwing_decap_fn(matching_wrap.xwing_ciphertext)
    elif header.visibility == Visibility.SEALED:
        # Blinded SEALED mode: trial-decapsulate wraps and match per-message unlinkable tag.
        for candidate in enc_header.recipients:
            if len(candidate.recipient_key_id) != 32:
                continue
            try:
                ss = xwing_decap_fn(candidate.xwing_ciphertext)
            except Exception:
                continue
            if len(ss) == 32 and hmac.compare_digest(
                compute_sealed_recipient_tag(ss, expected_aad),
                candidate.recipient_key_id,
            ):
                matching_wrap = candidate
                shared_secret = ss
                break
        if matching_wrap is None or shared_secret is None:
            raise MessageProtocolError("no matching RecipientWrap for recipient_key_id")
    else:
        if recipient_key_id is None:
            raise MessageProtocolError("recipient_key_id and xwing_decap_fn are required to decrypt PRIVATE/SEALED")
        raise MessageProtocolError("no matching RecipientWrap for recipient_key_id")

    kek = derive_xwing_wrap_kek(
        shared_secret,
        expected_aad,
        matching_wrap.recipient_key_id,
        kdf_info=kdf_info,
    )
    cek = xchacha20_poly1305_decrypt(kek, matching_wrap.wrap_nonce, matching_wrap.wrapped_cek, expected_aad)
    if enc_header.cek_commitment:
        expected_commit = compute_cek_commitment(cek, expected_aad)
        if len(enc_header.cek_commitment) != 32 or not hmac.compare_digest(enc_header.cek_commitment, expected_commit):
            raise MessageProtocolError("XWingEncryptionHeader.cek_commitment mismatch")
    elif require_cek_commitment:
        raise MessageProtocolError("XWingEncryptionHeader.cek_commitment missing")
    plaintext_body = xchacha20_poly1305_decrypt(cek, enc_header.aead_nonce, envelope.body_bytes, expected_aad)
    return OpenedEnvelopeResult(
        envelope=envelope,
        header=header,
        encryption_header=enc_header,
        plaintext_body=plaintext_body,
        cid_ref=cid_ref,
    )


def split_manifest_chunks(
    payload: bytes,
    *,
    chain_id: str,
    header_bytes: bytes,
    media_type: str,
    inner_schema_name: str,
    inner_body_encoding: BodyEncoding = BodyEncoding.RAW_BYTES,
    cipher_suite: CipherSuite = CipherSuite.XWING_XCHACHA20_POLY1305,
    cek: bytes | None = None,
    chunk_size_bytes: int = MAX_SINGLE_ENVELOPE_BYTES - AEAD_TAG_BYTES,
    nonce_fn: Callable[[int], bytes] | None = None,
) -> tuple[Manifest, list[bytes]]:
    """Split payloads (up to 16 MiB) into chunk-index-bound blocks + canonical Manifest."""
    if len(payload) > MAX_MANIFEST_PLAINTEXT_BYTES:
        raise MessageProtocolError(f"payload size {len(payload)} exceeds 16 MiB Manifest limit")
    encrypted = cipher_suite == CipherSuite.XWING_XCHACHA20_POLY1305
    max_pt_chunk = (MAX_SINGLE_ENVELOPE_BYTES - AEAD_TAG_BYTES) if encrypted else MAX_SINGLE_ENVELOPE_BYTES
    if chunk_size_bytes <= 0 or chunk_size_bytes > max_pt_chunk:
        raise MessageProtocolError(f"chunk_size_bytes {chunk_size_bytes} exceeds per-block limit {max_pt_chunk}")
    if encrypted and (cek is None or len(cek) != AEAD_KEY_BYTES):
        raise MessageProtocolError("32-byte cek is required for encrypted Manifest chunks")

    blocks: list[bytes] = []
    chunks: list[ManifestChunk] = []
    idx = 0
    for offset in range(0, max(len(payload), 1), chunk_size_bytes):
        piece = payload[offset : offset + chunk_size_bytes]
        if encrypted:
            assert cek is not None
            nonce = nonce_fn(idx) if nonce_fn is not None else os.urandom(AEAD_NONCE_BYTES)
            aad = compute_aad_context(chain_id, header_bytes, idx)
            block = xchacha20_poly1305_encrypt(cek, nonce, piece, aad)
        else:
            nonce = b""
            block = piece
        cid_bin, multibase = cidv1_raw_from_bytes(block)
        blocks.append(block)
        chunks.append(
            ManifestChunk(
                chunk_index=idx,
                cid=IpfsCidRef(cid_bin=cid_bin, byte_length=len(block), multibase_cid=multibase),
                aead_nonce=nonce,
            )
        )
        idx += 1
        if len(payload) == 0:
            break

    manifest = Manifest(
        version=1,
        total_plaintext_bytes=len(payload),
        chunk_size_bytes=chunk_size_bytes,
        media_type=media_type,
        inner_schema_name=inner_schema_name,
        inner_body_encoding=inner_body_encoding,
        cipher_suite=cipher_suite,
        chunks=tuple(chunks),
    )
    manifest.validate()
    return manifest, blocks


def reassemble_manifest_chunks(
    manifest: Manifest,
    blocks: Sequence[bytes],
    *,
    chain_id: str,
    header_bytes: bytes,
    cek: bytes | None = None,
) -> bytes:
    """Verify every chunk CID and chunk-index-bound AAD, then reassemble original plaintext."""
    manifest.validate()
    if len(blocks) != len(manifest.chunks):
        raise MessageProtocolError("block count does not match manifest.chunks length")
    encrypted = manifest.cipher_suite == CipherSuite.XWING_XCHACHA20_POLY1305
    if encrypted and (cek is None or len(cek) != AEAD_KEY_BYTES):
        raise MessageProtocolError("32-byte cek is required to decrypt Manifest chunks")

    out = bytearray()
    for chunk, block in zip(manifest.chunks, blocks):
        verify_cidv1_raw(
            block,
            expected_cid_bin=chunk.cid.cid_bin,
            expected_multibase_cid=chunk.cid.multibase_cid or None,
        )
        if len(block) != chunk.cid.byte_length:
            raise MessageProtocolError(f"chunk {chunk.chunk_index} byte_length mismatch")
        if encrypted:
            assert cek is not None
            aad = compute_aad_context(chain_id, header_bytes, chunk.chunk_index)
            out.extend(xchacha20_poly1305_decrypt(cek, chunk.aead_nonce, block, aad))
        else:
            out.extend(block)
    if len(out) != manifest.total_plaintext_bytes:
        raise MessageProtocolError(
            f"reassembled length {len(out)} != manifest.total_plaintext_bytes {manifest.total_plaintext_bytes}"
        )
    return bytes(out)
