#!/usr/bin/env python3
"""Deterministic schema and wire-vector verification suite for Post Fiat Contract C1.

Validates:
  1. Protobuf schema integrity for:
       - proto/postfiat/message/v1/message.proto
       - proto/postfiat/nft/v1/metadata.proto
       - proto/postfiat/governance/v1/packet.proto
     Uses `protoc` compilation + wire decode cross-checking whenever `protoc` is
     available on PATH (mandatory when `--require-protoc` is passed), and always
     runs pure-Python stdlib schema + wire-format checks so CI needs zero pip installs:
     the .proto files are parsed, every declared message must match the script's
     field table (number, label, type, name), and every vector byte string is
     decoded strictly against the parsed declarations.
  2. Exact deterministic worked hex vectors published in
     docs/specs/message-protocol-v1.md (single source of truth; detects any drift
     between the specification document and canonical wire encoding).
  3. Cryptographic & protocol invariants:
       - IPFS CIDv1 raw (0x01 0x55 0x12 0x20 || SHA-256) binary & base32lower multibase
       - PaymentMemo L1 constraints (lowercase hex, <= 64/64/256 bytes, total <= 512)
       - FastPay order_hash / certificate_digest and consensus tx_hash are 48-byte SHA3-384 chain hashes
       - Pointer v5 visibility flag mutual exclusion (exact one of PUBLIC/PRIVATE/SEALED)
       - Envelope raw-byte signing preimage (zero re-serialization rule)
       - X-Wing AAD commitment, HKDF-SHA256 CEK wrap key, wallet KEM derivation, key-binding preimage
       - Full production-length sizes (1216B X-Wing pk, 1120B X-Wing ct, 1952B ML-DSA-65 pk,
         3309B ML-DSA-65 sig, 1 MiB single envelope cap, 16 MiB manifest cap)
       - Legacy pf.ptr v4 compatibility decode
       - Pillar 4 NFT one-hash rule (metadata_hash == SHA3-384(raw_envelope)) and bit-63 migrated serial
       - Pillar 1 Governance RFC 8785 Canonical JSON, transcript root SHA-256, and GOVERNANCE (12) memo
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PROTO_ROOT = ROOT / "proto"
SPEC_DOC = ROOT / "docs" / "specs" / "message-protocol-v1.md"

PROTO_FILES = (
    "postfiat/message/v1/message.proto",
    "postfiat/nft/v1/metadata.proto",
    "postfiat/governance/v1/packet.proto",
)

# L1 PaymentMemo constants (must match crates/types/src/core_chain.rs)
MAX_PAYMENT_MEMOS = 4
MAX_PAYMENT_MEMO_TYPE_BYTES = 64
MAX_PAYMENT_MEMO_FORMAT_BYTES = 64
MAX_PAYMENT_MEMO_DATA_BYTES = 256
MAX_PAYMENT_MEMO_TOTAL_BYTES = 512
MAX_NFT_METADATA_HASH_BYTES = 64
MAX_NFT_METADATA_URI_BYTES = 256

# L1 chain hash size: SHA3-384 (hash_bytes in crates/crypto_provider/src/lib.rs) behind FastPay order and
# certificate digests and consensus tx ids, which the RPC returns as 96 lowercase hex characters.
CHAIN_HASH_BYTES = 48

# Envelope / Manifest caps (Pillar 2)
MAX_SINGLE_ENVELOPE_BYTES = 1_048_576  # 1 MiB
MAX_MANIFEST_PLAINTEXT_BYTES = 16_777_216  # 16 MiB

# PointerFlag bitmask (postfiat.message.v1.PointerFlag)
POINTER_FLAG_PRIVATE = 0x01
POINTER_FLAG_PUBLIC = 0x02
POINTER_FLAG_EPHEMERAL = 0x04
POINTER_FLAG_TOMBSTONE = 0x08
POINTER_FLAG_MULTIPART = 0x10
POINTER_FLAG_SEALED = 0x20
VISIBILITY_FLAG_MASK = POINTER_FLAG_PRIVATE | POINTER_FLAG_PUBLIC | POINTER_FLAG_SEALED
ALLOWED_POINTER_FLAG_MASK = (
    POINTER_FLAG_PRIVATE
    | POINTER_FLAG_PUBLIC
    | POINTER_FLAG_EPHEMERAL
    | POINTER_FLAG_TOMBSTONE
    | POINTER_FLAG_MULTIPART
    | POINTER_FLAG_SEALED
)

# Canonical domain separation strings
DOMAIN_ENVELOPE_SIGN = b"postfiat-l1-v2/message-envelope/v1"
DOMAIN_MESSAGE_AAD = b"postfiat-l1-v2/message-aad/v1"
DOMAIN_XWING_WRAP_INFO = b"postfiat-l1-v2/xwing-wrap/v1"
DOMAIN_MESSAGE_KEY_KDF = b"postfiat-l1-v2/message-key/v1"
DOMAIN_MESSAGE_KEY_BIND = b"postfiat-l1-v2/message-key-bind/v1"
DOMAIN_NFT_MIGRATED_SERIAL = b"postfiat.nft.migrated_serial.v1"
DOMAIN_GOV_TRANSCRIPT_ROOT = b"postfiat.governance.transcript_root.v1"


# ---------------------------------------------------------------------------
# Pure-Python canonical protobuf3 wire encoder / decoder
# ---------------------------------------------------------------------------

def encode_varint(value: int) -> bytes:
    if value < 0:
        raise ValueError(f"negative varint not supported for unsigned proto3 field: {value}")
    out = bytearray()
    while value > 0x7F:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def decode_varint(buf: bytes, offset: int) -> tuple[int, int]:
    result = 0
    shift = 0
    while True:
        if offset >= len(buf):
            raise ValueError("truncated varint")
        byte = buf[offset]
        offset += 1
        result |= (byte & 0x7F) << shift
        if not (byte & 0x80):
            return result, offset
        shift += 7
        if shift > 63:
            raise ValueError("varint overflow")


def pb_varint(field_num: int, value: int, *, emit_zero: bool = False) -> bytes:
    if value == 0 and not emit_zero:
        return b""
    tag = encode_varint((field_num << 3) | 0)
    return tag + encode_varint(value)


def pb_bool(field_num: int, value: bool) -> bytes:
    return pb_varint(field_num, 1 if value else 0)


def pb_bytes(field_num: int, value: bytes) -> bytes:
    if not value:
        return b""
    tag = encode_varint((field_num << 3) | 2)
    return tag + encode_varint(len(value)) + value


def pb_string(field_num: int, value: str) -> bytes:
    return pb_bytes(field_num, value.encode("utf-8"))


def pb_msg(field_num: int, payload: bytes) -> bytes:
    return pb_bytes(field_num, payload)


def pb_map_ss(field_num: int, mapping: dict[str, str]) -> bytes:
    """Canonical map<string, string> encoding: entries sorted by UTF-8 key bytes."""
    out = bytearray()
    for k in sorted(mapping.keys()):
        entry = pb_string(1, k) + pb_string(2, mapping[k])
        tag = encode_varint((field_num << 3) | 2)
        out.extend(tag + encode_varint(len(entry)) + entry)
    return bytes(out)


def decode_wire_fields(buf: bytes) -> list[tuple[int, int, Any]]:
    """Decode raw protobuf wire fields as (field_number, wire_type, value)."""
    fields: list[tuple[int, int, Any]] = []
    offset = 0
    while offset < len(buf):
        tag, offset = decode_varint(buf, offset)
        field_num = tag >> 3
        wire_type = tag & 0x07
        if field_num == 0:
            raise ValueError("invalid protobuf field number 0")
        if wire_type == 0:
            val, offset = decode_varint(buf, offset)
            fields.append((field_num, wire_type, val))
        elif wire_type == 2:
            length, offset = decode_varint(buf, offset)
            if offset + length > len(buf):
                raise ValueError("truncated length-delimited field")
            val = buf[offset : offset + length]
            offset += length
            fields.append((field_num, wire_type, val))
        elif wire_type == 1:
            if offset + 8 > len(buf):
                raise ValueError("truncated 64-bit field")
            fields.append((field_num, wire_type, buf[offset : offset + 8]))
            offset += 8
        elif wire_type == 5:
            if offset + 4 > len(buf):
                raise ValueError("truncated 32-bit field")
            fields.append((field_num, wire_type, buf[offset : offset + 4]))
            offset += 4
        else:
            raise ValueError(f"unsupported wire type {wire_type}")
    return fields


def assert_canonical_field_order(buf: bytes, label: str) -> None:
    """Verify field numbers are non-decreasing (strictly ascending except repeated fields)."""
    fields = decode_wire_fields(buf)
    last_fn = 0
    for fn, _, _ in fields:
        if fn < last_fn:
            raise AssertionError(f"{label}: non-canonical field order ({fn} after {last_fn})")
        last_fn = fn


# ---------------------------------------------------------------------------
# Cryptographic & formatting primitives (pure stdlib)
# ---------------------------------------------------------------------------

def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def sha3_384(data: bytes) -> bytes:
    return hashlib.sha3_384(data).digest()


def hkdf_sha256(ikm: bytes, salt: bytes, info: bytes, length: int) -> bytes:
    """RFC 5869 HKDF-SHA256."""
    if not salt:
        salt = b"\x00" * 32
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm = bytearray()
    t = b""
    counter = 1
    while len(okm) < length:
        t = hmac.new(prk, t + info + bytes([counter]), hashlib.sha256).digest()
        okm.extend(t)
        counter += 1
    return bytes(okm[:length])


def cidv1_raw_from_bytes(block: bytes) -> tuple[bytes, str]:
    """Compute CIDv1 raw (codec 0x55, multihash sha2-256) binary (36B) and base32lower multibase."""
    cid_bin = b"\x01\x55\x12\x20" + sha256(block)
    assert len(cid_bin) == 36
    multibase = "b" + base64.b32encode(cid_bin).decode("ascii").lower().rstrip("=")
    return cid_bin, multibase


def canonical_json_bytes(obj: Any) -> bytes:
    """RFC 8785 / JCS canonical JSON for ASCII/BMP JSON objects with integer numbers."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def envelope_signing_preimage(
    version: int,
    header_bytes: bytes,
    encryption_header_bytes: bytes,
    body_bytes: bytes,
) -> bytes:
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


def compute_aad_context(chain_id: str, header_bytes: bytes, chunk_index: int) -> bytes:
    return sha256(
        DOMAIN_MESSAGE_AAD
        + b"\x00"
        + chain_id.encode("utf-8")
        + b"\x00"
        + sha256(header_bytes)
        + struct.pack(">I", chunk_index)
    )


def compute_key_binding_preimage(
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


def compute_nft_migrated_serial(source_chain: str, nftoken_id: str) -> int:
    digest = sha3_384(
        DOMAIN_NFT_MIGRATED_SERIAL
        + b"\x00"
        + source_chain.encode("utf-8")
        + b"\x00"
        + nftoken_id.upper().encode("utf-8")
    )
    low63 = int.from_bytes(digest[:8], "big") & 0x7FFF_FFFF_FFFF_FFFF
    return (1 << 63) | low63


def compute_transcript_root(entries: list[tuple[int, str, bytes]]) -> bytes:
    buf = bytearray(DOMAIN_GOV_TRANSCRIPT_ROOT + b"\x00")
    for seq, event_id_hex, raw_sha256 in entries:
        buf.extend(struct.pack(">Q", seq))
        event_id_bytes = bytes.fromhex(event_id_hex)
        assert len(event_id_bytes) == 32
        assert len(raw_sha256) == 32
        buf.extend(event_id_bytes)
        buf.extend(raw_sha256)
    return sha256(bytes(buf))


def validate_pointer_v5_flags(flags: int) -> None:
    if flags & ~ALLOWED_POINTER_FLAG_MASK:
        raise ValueError(f"unsupported pointer flags: 0x{flags:x}")
    vis = flags & VISIBILITY_FLAG_MASK
    if vis not in (POINTER_FLAG_PRIVATE, POINTER_FLAG_PUBLIC, POINTER_FLAG_SEALED):
        raise ValueError(
            f"pointer flags must set exactly one of PRIVATE (0x01), PUBLIC (0x02), SEALED (0x20); got 0x{flags:x}"
        )


def validate_l1_payment_memo(memo_type_hex: str, memo_format_hex: str, memo_data_hex: str) -> None:
    for name, val, limit in (
        ("memo_type", memo_type_hex, MAX_PAYMENT_MEMO_TYPE_BYTES),
        ("memo_format", memo_format_hex, MAX_PAYMENT_MEMO_FORMAT_BYTES),
        ("memo_data", memo_data_hex, MAX_PAYMENT_MEMO_DATA_BYTES),
    ):
        if val != val.lower():
            raise ValueError(f"{name} must be lowercase hex")
        if len(val) % 2 != 0:
            raise ValueError(f"{name} must have even hex length")
        raw = bytes.fromhex(val)
        if len(raw) > limit:
            raise ValueError(f"{name} decoded length {len(raw)} exceeds limit {limit}")
    total = (len(memo_type_hex) + len(memo_format_hex) + len(memo_data_hex)) // 2
    if total == 0 or total > MAX_PAYMENT_MEMO_TOTAL_BYTES:
        raise ValueError(f"total memo byte length {total} outside 1..{MAX_PAYMENT_MEMO_TOTAL_BYTES}")


def validate_chain_hashes(message: str, **hashes: bytes) -> None:
    for name, value in hashes.items():
        if value and len(value) != CHAIN_HASH_BYTES:
            raise ValueError(f"{message}.{name} must be {CHAIN_HASH_BYTES} bytes (SHA3-384), got {len(value)}")


# ---------------------------------------------------------------------------
# Message encoders matching the frozen .proto field numbers
# ---------------------------------------------------------------------------

def enc_ipfs_cid_ref(cid_bin: bytes, byte_length: int, multibase_cid: str = "") -> bytes:
    return b"".join(
        [
            pb_bytes(1, cid_bin),
            pb_varint(2, byte_length),
            pb_string(3, multibase_cid),
        ]
    )


def enc_pointer_v5(
    *,
    cid_bin: bytes = b"",
    target: int = 1,
    kind: int = 0,
    schema: int = 0,
    task_id: str = "",
    thread_id: str = "",
    context_id: str = "",
    flags: int = 0,
    inline_payload: bytes = b"",
    schema_name: str = "",
) -> bytes:
    validate_pointer_v5_flags(flags)
    if target == 3:
        assert not cid_bin and inline_payload, "INLINE target requires inline_payload and empty cid_bin"
    else:
        assert len(cid_bin) == 36 and not inline_payload, "CONTENT_BLOB/MANIFEST requires 36-byte cid_bin"
    encoded = b"".join(
        [
            pb_bytes(1, cid_bin),
            pb_varint(2, target),
            pb_varint(3, kind),
            pb_varint(4, schema),
            pb_string(5, task_id),
            pb_string(6, thread_id),
            pb_string(7, context_id),
            pb_varint(8, flags),
            pb_bytes(9, inline_payload),
            pb_string(10, schema_name),
        ]
    )
    if len(encoded) > MAX_PAYMENT_MEMO_DATA_BYTES:
        raise ValueError(f"Pointer v5 exceeds {MAX_PAYMENT_MEMO_DATA_BYTES} bytes: {len(encoded)}")
    return encoded


def enc_legacy_pointer_v4(
    *,
    cid: str,
    target: int,
    kind: int,
    schema: int,
    task_id: str = "",
    thread_id: str = "",
    context_id: str = "",
    flags: int = 1,
) -> bytes:
    return b"".join(
        [
            pb_string(1, cid),
            pb_varint(2, target),
            pb_varint(3, kind),
            pb_varint(4, schema),
            pb_string(5, task_id),
            pb_string(6, thread_id),
            pb_string(7, context_id),
            pb_varint(8, flags),
        ]
    )


def enc_fastpay_payment_ref(
    *,
    lane: int,
    chain_id: str,
    order_hash: bytes = b"",
    certificate_digest: bytes = b"",
    tx_hash: bytes = b"",
    asset_id: str = "",
    payer_account: str = "",
    payee_account: str = "",
    amount_atoms: int = 0,
    fee_atoms: int = 0,
    finalized_height: int = 0,
) -> bytes:
    validate_chain_hashes(
        "FastPayPaymentRef",
        order_hash=order_hash,
        certificate_digest=certificate_digest,
        tx_hash=tx_hash,
    )
    return b"".join(
        [
            pb_varint(1, lane),
            pb_string(2, chain_id),
            pb_bytes(3, order_hash),
            pb_bytes(4, certificate_digest),
            pb_bytes(5, tx_hash),
            pb_string(6, asset_id),
            pb_string(7, payer_account),
            pb_string(8, payee_account),
            pb_varint(9, amount_atoms),
            pb_varint(10, fee_atoms),
            pb_varint(11, finalized_height),
        ]
    )


def enc_recipient_wrap(
    *,
    recipient_key_id: bytes,
    recipient_account: str,
    xwing_ciphertext: bytes,
    wrap_nonce: bytes,
    wrapped_cek: bytes,
) -> bytes:
    return b"".join(
        [
            pb_bytes(1, recipient_key_id),
            pb_string(2, recipient_account),
            pb_bytes(3, xwing_ciphertext),
            pb_bytes(4, wrap_nonce),
            pb_bytes(5, wrapped_cek),
        ]
    )


def enc_xwing_encryption_header(
    *,
    cipher_suite: int,
    aead_nonce: bytes,
    aad_context: bytes,
    chunk_index: int,
    kdf_info: str,
    recipients: list[bytes],
) -> bytes:
    parts = [
        pb_varint(1, cipher_suite),
        pb_bytes(2, aead_nonce),
        pb_bytes(3, aad_context),
        pb_varint(4, chunk_index),
        pb_string(5, kdf_info),
    ]
    for r in recipients:
        parts.append(pb_msg(6, r))
    return b"".join(parts)


def enc_message_header(
    *,
    protocol_version: int,
    chain_id: str,
    schema_name: str,
    schema_id: int,
    kind: int,
    visibility: int,
    body_encoding: int,
    cipher_suite: int,
    sender_account: str,
    created_at_unix_ms: int,
    task_id: str = "",
    thread_id: str = "",
    context_id: str = "",
    reply_to_cid: bytes = b"",
    payment_ref: bytes = b"",
    x402_receipt: bytes = b"",
) -> bytes:
    return b"".join(
        [
            pb_varint(1, protocol_version),
            pb_string(2, chain_id),
            pb_string(3, schema_name),
            pb_varint(4, schema_id),
            pb_varint(5, kind),
            pb_varint(6, visibility),
            pb_varint(7, body_encoding),
            pb_varint(8, cipher_suite),
            pb_string(9, sender_account),
            pb_varint(10, created_at_unix_ms),
            pb_string(11, task_id),
            pb_string(12, thread_id),
            pb_string(13, context_id),
            pb_msg(14, reply_to_cid),
            pb_msg(15, payment_ref),
            pb_msg(16, x402_receipt),
        ]
    )


def enc_envelope(
    *,
    version: int,
    header_bytes: bytes,
    encryption_header_bytes: bytes,
    body_bytes: bytes,
    signature_algorithm: int,
    sender_pubkey: bytes,
    signature: bytes,
) -> bytes:
    return b"".join(
        [
            pb_varint(1, version),
            pb_bytes(2, header_bytes),
            pb_bytes(3, encryption_header_bytes),
            pb_bytes(4, body_bytes),
            pb_varint(5, signature_algorithm),
            pb_bytes(6, sender_pubkey),
            pb_bytes(7, signature),
        ]
    )


def enc_text_message(text: str, fmt: str = "", attachments: list[bytes] | None = None) -> bytes:
    parts = [pb_string(1, text), pb_string(2, fmt)]
    for att in attachments or []:
        parts.append(pb_msg(3, att))
    return b"".join(parts)


def enc_manifest(
    *,
    version: int,
    total_plaintext_bytes: int,
    chunk_size_bytes: int,
    media_type: str,
    inner_schema_name: str,
    inner_body_encoding: int,
    cipher_suite: int,
    chunks: list[tuple[int, bytes, bytes]],
) -> bytes:
    parts = [
        pb_varint(1, version),
        pb_varint(2, total_plaintext_bytes),
        pb_varint(3, chunk_size_bytes),
        pb_string(4, media_type),
        pb_string(5, inner_schema_name),
        pb_varint(6, inner_body_encoding),
        pb_varint(7, cipher_suite),
    ]
    for idx, cid_ref, nonce in chunks:
        chunk_msg = b"".join([pb_varint(1, idx), pb_msg(2, cid_ref), pb_bytes(3, nonce)])
        parts.append(pb_msg(8, chunk_msg))
    return b"".join(parts)


def enc_x402_required(
    *,
    x402_version: int,
    scheme: int,
    network: str,
    lane: int,
    asset_id: str,
    max_amount_atoms: int,
    pay_to: str,
    request_hash: bytes,
    nonce: bytes,
    expires_at_unix_ms: int,
    resource_uri: str,
    facilitator_url: str,
) -> bytes:
    return b"".join(
        [
            pb_varint(1, x402_version),
            pb_varint(2, scheme),
            pb_string(3, network),
            pb_varint(4, lane),
            pb_string(5, asset_id),
            pb_varint(6, max_amount_atoms),
            pb_string(7, pay_to),
            pb_bytes(8, request_hash),
            pb_bytes(9, nonce),
            pb_varint(10, expires_at_unix_ms),
            pb_string(11, resource_uri),
            pb_string(12, facilitator_url),
        ]
    )


def enc_x402_signature(
    *,
    x402_version: int,
    scheme: int,
    network: str,
    lane: int,
    request_hash: bytes,
    nonce: bytes,
    order_hash: bytes = b"",
    tx_hash: bytes = b"",
    signed_payload: bytes = b"",
    certificate_payload: bytes = b"",
) -> bytes:
    validate_chain_hashes("X402PaymentSignature", order_hash=order_hash, tx_hash=tx_hash)
    return b"".join(
        [
            pb_varint(1, x402_version),
            pb_varint(2, scheme),
            pb_string(3, network),
            pb_varint(4, lane),
            pb_bytes(5, request_hash),
            pb_bytes(6, nonce),
            pb_bytes(7, order_hash),
            pb_bytes(8, tx_hash),
            pb_bytes(9, signed_payload),
            pb_bytes(10, certificate_payload),
        ]
    )


def enc_x402_response(
    *,
    x402_version: int,
    settled: bool,
    payment_ref: bytes,
    settled_at_unix_ms: int,
    facilitator_id: str,
) -> bytes:
    return b"".join(
        [
            pb_varint(1, x402_version),
            pb_bool(2, settled),
            pb_msg(3, payment_ref),
            pb_varint(4, settled_at_unix_ms),
            pb_string(5, facilitator_id),
        ]
    )


def enc_nft_migration_claim(
    *,
    schema_version: int,
    target_chain_id: str,
    source_chain: str,
    nftoken_id: str,
    old_owner: str,
    new_owner_account: str,
    target_collection_id: str,
    migration_issuer_account: str,
    migrated_serial: int,
    retirement_mode: int,
    cutoff_ledger_index: int,
    claimed_at_unix_ms: int,
    allowlist_cid: bytes,
) -> bytes:
    return b"".join(
        [
            pb_varint(1, schema_version),
            pb_string(2, target_chain_id),
            pb_string(3, source_chain),
            pb_string(4, nftoken_id),
            pb_string(5, old_owner),
            pb_string(6, new_owner_account),
            pb_string(7, target_collection_id),
            pb_string(8, migration_issuer_account),
            pb_varint(9, migrated_serial),
            pb_varint(10, retirement_mode),
            pb_varint(11, cutoff_ledger_index),
            pb_varint(12, claimed_at_unix_ms),
            pb_msg(13, allowlist_cid),
        ]
    )


def enc_nft_metadata(
    *,
    schema_version: int,
    name: str,
    description: str,
    image: str,
    image_cid: bytes,
    media: list[bytes],
    attributes: list[bytes],
    collection: bytes,
    content_hashes: list[bytes],
    prompt_digest: bytes,
    license_str: str,
    external_url: str,
    origin: bytes,
    extensions: dict[str, str],
) -> bytes:
    parts = [
        pb_varint(1, schema_version),
        pb_string(2, name),
        pb_string(3, description),
        pb_string(4, image),
        pb_msg(5, image_cid),
    ]
    for m in media:
        parts.append(pb_msg(6, m))
    for a in attributes:
        parts.append(pb_msg(7, a))
    parts.append(pb_msg(8, collection))
    for ch in content_hashes:
        parts.append(pb_msg(9, ch))
    parts.extend(
        [
            pb_bytes(10, prompt_digest),
            pb_string(11, license_str),
            pb_string(12, external_url),
            pb_msg(13, origin),
            pb_map_ss(14, extensions),
        ]
    )
    return b"".join(parts)


# ---------------------------------------------------------------------------
# Deterministic vector builder (produces the canonical expected vectors)
# ---------------------------------------------------------------------------

def build_canonical_vectors() -> dict[str, dict[str, Any]]:
    chain_id = "postfiat-devnet-2"
    sender_acct = "pf1sender000000000000000000000000000000000"
    recipient_acct = "pf1recip0000000000000000000000000000000000"
    tasknode_acct = "pf1tasknode0000000000000000000000000000000"
    founder_acct = "pf1founder00000000000000000000000000000000"
    migr_issuer_acct = "pf1nftmigr00000000000000000000000000000000"

    # Synthetic deterministic 32-byte fixture placeholders for compact doc wire-framing vectors
    # (full 1952B ML-DSA-65 pubkey / 3309B sig / 1120B X-Wing ct are additionally verified below).
    fixture_pk = bytes.fromhex("a1" * 32)
    fixture_sig = bytes.fromhex("b2" * 32)

    # --- V4: Public Envelope (pf.message.text.v1) ---
    v4_body = enc_text_message(
        "Hello Post Fiat L1v2!",
        "text/plain",
    )
    v4_header = enc_message_header(
        protocol_version=1,
        chain_id=chain_id,
        schema_name="pf.message.text.v1",
        schema_id=100,
        kind=4,  # CONTENT_KIND_CHAT
        visibility=1,  # VISIBILITY_PUBLIC
        body_encoding=1,  # BODY_ENCODING_PROTOBUF
        cipher_suite=1,  # CIPHER_SUITE_NONE
        sender_account=sender_acct,
        created_at_unix_ms=1764201600000,
        thread_id="thread-c1-001",
    )
    v4_preimage = envelope_signing_preimage(1, v4_header, b"", v4_body)
    v4_envelope = enc_envelope(
        version=1,
        header_bytes=v4_header,
        encryption_header_bytes=b"",
        body_bytes=v4_body,
        signature_algorithm=1,  # ML-DSA-65
        sender_pubkey=fixture_pk,
        signature=fixture_sig,
    )
    v4_cid_bin, v4_cid_b32 = cidv1_raw_from_bytes(v4_envelope)

    # --- V1: pf.ptr v5 public memo pointing to V4 Envelope ---
    v1_ptr = enc_pointer_v5(
        cid_bin=v4_cid_bin,
        target=1,  # CONTENT_BLOB
        kind=4,  # CONTENT_KIND_CHAT
        schema=100,
        thread_id="thread-c1-001",
        flags=POINTER_FLAG_PUBLIC,
    )
    v1_memo_type_hex = b"pf.ptr".hex()
    v1_memo_format_hex = b"v5".hex()
    v1_memo_data_hex = v1_ptr.hex()
    validate_l1_payment_memo(v1_memo_type_hex, v1_memo_format_hex, v1_memo_data_hex)

    # --- V2a: pf.ptr v5 worst-case 256-byte budget check ---
    v2a_ptr = enc_pointer_v5(
        cid_bin=v4_cid_bin,
        target=2,  # MANIFEST
        kind=12,  # CONTENT_KIND_GOVERNANCE
        schema=1,  # permissionless custom schema
        task_id="task-2026-11-27-000000000000000000000001",
        thread_id="gov-session-2026-11-27-00000000000000001",
        context_id="ctx-snapshot-2026-11-27-0000000000000001",
        flags=POINTER_FLAG_PUBLIC | POINTER_FLAG_MULTIPART,
        schema_name="org.example.custom.packet.v1",
    )
    validate_l1_payment_memo(v1_memo_type_hex, v1_memo_format_hex, v2a_ptr.hex())

    # --- V2b: pf.ptr v5 INLINE payload (x402 request_hash) ---
    x402_req_hash = sha256(b"POST /v1/inference\ncontent-type:application/json\n{\"prompt\":\"ping\"}")
    v2b_ptr = enc_pointer_v5(
        target=3,  # INLINE
        kind=14,  # CONTENT_KIND_X402
        schema=402,
        flags=POINTER_FLAG_PUBLIC | POINTER_FLAG_EPHEMERAL,
        inline_payload=x402_req_hash,
    )
    validate_l1_payment_memo(v1_memo_type_hex, v1_memo_format_hex, v2b_ptr.hex())

    # --- V3: Legacy pf.ptr v4 compatibility decode ---
    v3_ptr_v4 = enc_legacy_pointer_v4(
        cid="bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy",
        target=1,
        kind=6,  # CONTENT_KIND_REWARD
        schema=1,
        task_id="task-legacy-0042",
        flags=0x01,  # encrypted
    )

    # --- V5: Private X-Wing Envelope (Task Node FastPay reward payout) ---
    # FastPay order and certificate digests here and in V7 are SHA3-384 stand-ins over fixed labels:
    # the chain's 48-byte size, not digests of real orders.
    v5_order_hash = sha3_384(b"postfiat.fastpay.order.v3:test-reward-0001")
    v5_cert_digest = sha3_384(b"postfiat.fastpay.cert.v3:test-reward-0001")
    v5_payment_ref = enc_fastpay_payment_ref(
        lane=1,  # PAYMENT_LANE_FASTPAY
        chain_id=chain_id,
        order_hash=v5_order_hash,
        certificate_digest=v5_cert_digest,
        asset_id="PFT",
        payer_account=tasknode_acct,
        payee_account=recipient_acct,
        amount_atoms=500_000_000,  # 500 PFT
        fee_atoms=1_026,  # 25 base + 1 size + 1000 new object (net +1) + 0/1 input example
    )
    v5_header = enc_message_header(
        protocol_version=1,
        chain_id=chain_id,
        schema_name="pf.reward.v1",
        schema_id=201,
        kind=6,  # CONTENT_KIND_REWARD
        visibility=2,  # VISIBILITY_PRIVATE
        body_encoding=1,  # PROTOBUF
        cipher_suite=2,  # CIPHER_SUITE_XWING_XCHACHA20_POLY1305
        sender_account=tasknode_acct,
        created_at_unix_ms=1764201660000,
        task_id="task-reward-0001",
        payment_ref=v5_payment_ref,
    )
    v5_aad = compute_aad_context(chain_id, v5_header, 0)
    # Deterministic KEM derivation & wrap vectors (synthetic test bytes, non-secret)
    wallet_entropy_fixture = bytes.fromhex("10" * 32)
    derived_kem_seed = hkdf_sha256(
        wallet_entropy_fixture,
        chain_id.encode("utf-8"),
        DOMAIN_MESSAGE_KEY_KDF,
        32,
    )
    recip_xwing_pk_fixture = bytes.fromhex("22" * 32)
    recip_key_id = sha256(recip_xwing_pk_fixture)
    xwing_ss_fixture = bytes.fromhex("33" * 32)
    derived_kek = hkdf_sha256(
        xwing_ss_fixture,
        v5_aad,
        DOMAIN_XWING_WRAP_INFO + b"\x00" + recip_key_id,
        32,
    )
    v5_wrap = enc_recipient_wrap(
        recipient_key_id=recip_key_id,
        recipient_account=recipient_acct,
        xwing_ciphertext=bytes.fromhex("44" * 32),  # compact structural fixture; 1120B verified separately
        wrap_nonce=bytes.fromhex("55" * 24),
        wrapped_cek=bytes.fromhex("66" * 48),
    )
    v5_enc_header = enc_xwing_encryption_header(
        cipher_suite=2,
        aead_nonce=bytes.fromhex("77" * 24),
        aad_context=v5_aad,
        chunk_index=0,
        kdf_info=DOMAIN_XWING_WRAP_INFO.decode("ascii"),
        recipients=[v5_wrap],
    )
    v5_ciphertext = bytes.fromhex("88" * 64)  # ciphertext + 16-byte Poly1305 tag
    v5_preimage = envelope_signing_preimage(1, v5_header, v5_enc_header, v5_ciphertext)
    v5_envelope = enc_envelope(
        version=1,
        header_bytes=v5_header,
        encryption_header_bytes=v5_enc_header,
        body_bytes=v5_ciphertext,
        signature_algorithm=1,
        sender_pubkey=fixture_pk,
        signature=fixture_sig,
    )
    v5_cid_bin, v5_cid_b32 = cidv1_raw_from_bytes(v5_envelope)
    v5_key_bind_preimage = compute_key_binding_preimage(
        chain_id=chain_id,
        account=recipient_acct,
        xwing_public_key=recip_xwing_pk_fixture,
        key_id=recip_key_id,
        created_at_unix_ms=1764201600000,
        key_epoch=1,
    )

    # --- V6: Multi-chunk Manifest ---
    chunk0_cid_bin, chunk0_cid_b32 = cidv1_raw_from_bytes(b"postfiat-chunk-0-ciphertext")
    chunk1_cid_bin, chunk1_cid_b32 = cidv1_raw_from_bytes(b"postfiat-chunk-1-ciphertext")
    v6_manifest = enc_manifest(
        version=1,
        total_plaintext_bytes=1_572_864,  # 1.5 MiB
        chunk_size_bytes=1_048_576,  # 1 MiB
        media_type="application/octet-stream",
        inner_schema_name="pf.document.bundle.v1",
        inner_body_encoding=4,  # RAW_BYTES
        cipher_suite=2,  # XWING_XCHACHA20_POLY1305
        chunks=[
            (0, enc_ipfs_cid_ref(chunk0_cid_bin, 1_048_592, chunk0_cid_b32), bytes.fromhex("90" * 24)),
            (1, enc_ipfs_cid_ref(chunk1_cid_bin, 524_304, chunk1_cid_b32), bytes.fromhex("91" * 24)),
        ],
    )

    # --- V7: x402 v2 payment headers (FastPay PFT + PFUSDC consensus fallback) ---
    v7_nonce = bytes.fromhex("c1" * 16)
    v7_required = enc_x402_required(
        x402_version=2,
        scheme=1,  # X402_SCHEME_EXACT
        network="postfiat:devnet-2",
        lane=1,  # PAYMENT_LANE_FASTPAY
        asset_id="PFT",
        max_amount_atoms=100_000,
        pay_to=recipient_acct,
        request_hash=x402_req_hash,
        nonce=v7_nonce,
        expires_at_unix_ms=1764201900000,
        resource_uri="https://api.postfiat.org/v1/inference",
        facilitator_url="https://x402.postfiat.org",
    )
    v7_order_hash = sha3_384(b"postfiat.fastpay.order.v3:x402-0001")
    v7_signature = enc_x402_signature(
        x402_version=2,
        scheme=1,
        network="postfiat:devnet-2",
        lane=1,
        request_hash=x402_req_hash,
        nonce=v7_nonce,
        order_hash=v7_order_hash,
        signed_payload=bytes.fromhex("d2" * 32),
    )
    v7_payment_ref = enc_fastpay_payment_ref(
        lane=1,
        chain_id=chain_id,
        order_hash=v7_order_hash,
        certificate_digest=sha3_384(b"postfiat.fastpay.cert.v3:x402-0001"),
        asset_id="PFT",
        payer_account=sender_acct,
        payee_account=recipient_acct,
        amount_atoms=100_000,
        fee_atoms=26,
    )
    v7_response = enc_x402_response(
        x402_version=2,
        settled=True,
        payment_ref=v7_payment_ref,
        settled_at_unix_ms=1764201600250,
        facilitator_id="postfiat-facilitator-v1",
    )

    # --- V8: Pillar 4 NFT Metadata + Burn-First Migration Claim ---
    old_nftoken_id = "000800006203F49C21D5D6E022CB16DE3538F248662FC73C29ABA6A900000001"
    migrated_serial = compute_nft_migrated_serial("postfiat-legacy-xrpl", old_nftoken_id)
    assert (migrated_serial >> 63) == 1
    allowlist_cid_bin, allowlist_cid_b32 = cidv1_raw_from_bytes(b"postfiat-nft-allowlist-cutoff-1234567")
    v8_claim = enc_nft_migration_claim(
        schema_version=1,
        target_chain_id=chain_id,
        source_chain="postfiat-legacy-xrpl",
        nftoken_id=old_nftoken_id,
        old_owner="rOldOwnerLegacyAddress000000000001",
        new_owner_account=recipient_acct,
        target_collection_id="TASKNODE-PROFILE-V1",
        migration_issuer_account=migr_issuer_acct,
        migrated_serial=migrated_serial,
        retirement_mode=1,  # BURN_FIRST
        cutoff_ledger_index=1_234_567,
        claimed_at_unix_ms=1764201700000,
        allowlist_cid=enc_ipfs_cid_ref(allowlist_cid_bin, 37, allowlist_cid_b32),
    )
    v8_claim_header = enc_message_header(
        protocol_version=1,
        chain_id=chain_id,
        schema_name="pf.nft.claim.v1",
        schema_id=401,
        kind=13,  # CONTENT_KIND_NFT
        visibility=1,  # PUBLIC
        body_encoding=1,  # PROTOBUF
        cipher_suite=1,  # NONE
        sender_account=recipient_acct,
        created_at_unix_ms=1764201700000,
    )
    v8_claim_env = enc_envelope(
        version=1,
        header_bytes=v8_claim_header,
        encryption_header_bytes=b"",
        body_bytes=v8_claim,
        signature_algorithm=1,
        sender_pubkey=fixture_pk,
        signature=fixture_sig,
    )
    v8_claim_cid_bin, v8_claim_cid_b32 = cidv1_raw_from_bytes(v8_claim_env)
    v8_claim_hash = sha256(v8_claim_env)

    img_bytes = b"synthetic-nft-image-bytes-v1"
    img_cid_bin, img_cid_b32 = cidv1_raw_from_bytes(img_bytes)
    img_sha3_384 = sha3_384(img_bytes)
    burn_bundle_cid_bin, burn_bundle_cid_b32 = cidv1_raw_from_bytes(b"founder-signed-burn-proof-bundle-v1")

    v8_media = b"".join(
        [
            pb_string(1, f"ipfs://{img_cid_b32}"),
            pb_msg(2, enc_ipfs_cid_ref(img_cid_bin, len(img_bytes), img_cid_b32)),
            pb_string(3, "image/png"),
            pb_varint(4, 1),  # PRIMARY_IMAGE
            pb_varint(5, 1024),
            pb_varint(6, 1024),
            pb_varint(7, len(img_bytes)),
            pb_bytes(8, img_sha3_384),
        ]
    )
    v8_attr = b"".join([pb_string(1, "tier"), pb_string(2, "Genesis"), pb_string(3, "string")])
    v8_coll = b"".join(
        [
            pb_string(1, "TASKNODE-PROFILE-V1"),
            pb_string(2, migr_issuer_acct),
            pb_string(3, "Post Fiat Profile NFTs"),
        ]
    )
    v8_chash = b"".join(
        [
            pb_string(1, f"ipfs://{img_cid_b32}"),
            pb_varint(2, 1),  # HASH_ALGORITHM_SHA3_384
            pb_bytes(3, img_sha3_384),
            pb_varint(4, len(img_bytes)),
        ]
    )
    v8_origin = b"".join(
        [
            pb_string(1, "postfiat-legacy-xrpl"),
            pb_string(2, old_nftoken_id),
            pb_string(3, "rOldIssuerLegacyAddress00000000001"),
            pb_string(4, "rOldOwnerLegacyAddress000000000001"),
            pb_string(5, f"ipfs://{img_cid_b32}"),
            pb_string(6, "AB" * 32),
            pb_varint(7, 1_100_000),
            pb_varint(8, 1),  # BURN_FIRST
            pb_string(9, "CD" * 32),
            pb_varint(10, 1_234_600),
            pb_msg(11, enc_ipfs_cid_ref(v8_claim_cid_bin, len(v8_claim_env), v8_claim_cid_b32)),
            pb_bytes(12, v8_claim_hash),
            pb_msg(13, enc_ipfs_cid_ref(burn_bundle_cid_bin, 35, burn_bundle_cid_b32)),
            pb_varint(14, migrated_serial),
        ]
    )
    v8_metadata = enc_nft_metadata(
        schema_version=1,
        name="Post Fiat Profile #1",
        description="Migrated post-quantum profile NFT",
        image=f"ipfs://{img_cid_b32}",
        image_cid=enc_ipfs_cid_ref(img_cid_bin, len(img_bytes), img_cid_b32),
        media=[v8_media],
        attributes=[v8_attr],
        collection=v8_coll,
        content_hashes=[v8_chash],
        prompt_digest=sha3_384(b"private-prompt-envelope-v1"),
        license_str="CC-BY-4.0",
        external_url="https://postfiat.org/nft/1",
        origin=v8_origin,
        extensions={"migration_wave": "founder-1"},
    )
    v8_meta_header = enc_message_header(
        protocol_version=1,
        chain_id=chain_id,
        schema_name="pf.nft.metadata.v1",
        schema_id=400,
        kind=13,  # CONTENT_KIND_NFT
        visibility=1,  # PUBLIC
        body_encoding=1,  # PROTOBUF
        cipher_suite=1,  # NONE
        sender_account=migr_issuer_acct,
        created_at_unix_ms=1764201710000,
    )
    v8_meta_env = enc_envelope(
        version=1,
        header_bytes=v8_meta_header,
        encryption_header_bytes=b"",
        body_bytes=v8_metadata,
        signature_algorithm=1,
        sender_pubkey=fixture_pk,
        signature=fixture_sig,
    )
    v8_meta_cid_bin, v8_meta_cid_b32 = cidv1_raw_from_bytes(v8_meta_env)
    v8_metadata_hash_hex = sha3_384(v8_meta_env).hex()
    assert len(v8_metadata_hash_hex) == 96  # 48 bytes = SHA3-384 <= MAX_NFT_METADATA_HASH_BYTES (64)
    v8_metadata_uri = f"ipfs://{v8_meta_cid_b32}"
    assert len(v8_metadata_uri) <= MAX_NFT_METADATA_URI_BYTES

    # --- V9: Pillar 1 Hive Governance Packet, Transcript, Minutes, Ruling ---
    gov_session_id = "gov-2026-11-27-001"
    founder_nostr_pk = "f0" * 32
    snap_raw_cid_bin, snap_raw_cid_b32 = cidv1_raw_from_bytes(b"{\"height\":42000,\"validators\":6}")
    v9_authority = b"".join(
        [
            pb_string(1, "founder-v1"),
            pb_varint(2, 1),
            pb_varint(3, 1),
            pb_string(4, founder_acct),
            pb_string(5, founder_nostr_pk),
            pb_string(6, "founder@tasknode.postfiat.org"),
        ]
    )
    v9_carry = b"".join(
        [
            pb_string(1, "deliv-000-1"),
            pb_string(2, "gov-2026-11-20-001"),
            pb_string(3, "Freeze C1 proto schemas"),
            pb_varint(4, 1),  # DELIVERABLE_STATUS_DONE
            pb_string(5, "Merged with CI vector gate"),
            pb_string(6, "pr:postfiatl1v2#c1"),
        ]
    )
    v9_prop_dec = b"".join(
        [
            pb_string(1, "dec-001"),
            pb_string(2, "Activate Train A height"),
            pb_string(3, "Set Train A activation height at 50000"),
            pb_bool(4, True),  # run_deterministic_ruling = true
        ]
    )
    elig_cid_bin, elig_cid_b32 = cidv1_raw_from_bytes(b"[\"f0f0\"]")
    v9_session_cfg = b"".join(
        [
            pb_string(1, "hive-gov-2026-11-27-001"),
            pb_varint(2, 2),  # POSTING_POLICY_ALLOWLIST
            pb_varint(3, 2),  # FACILITATOR_MODE_AGENDA
            pb_varint(4, 30),
            pb_varint(5, 15),
            pb_varint(6, 10),
            pb_varint(7, 25),
            pb_varint(8, 15),
            pb_msg(9, enc_ipfs_cid_ref(elig_cid_bin, 8, elig_cid_b32)),
            pb_bytes(10, sha256(b"[\"f0f0\"]")),
        ]
    )
    v9_packet_proto = b"".join(
        [
            pb_varint(1, 1),
            pb_string(2, gov_session_id),
            pb_string(3, chain_id),
            pb_varint(4, 1),
            pb_string(5, "Governance Session #1"),
            pb_string(6, "Review C1 freeze and Train A activation."),
            pb_msg(7, v9_authority),
            pb_msg(8, enc_ipfs_cid_ref(snap_raw_cid_bin, 31, snap_raw_cid_b32)),
            pb_bytes(9, sha256(b"{\"height\":42000,\"validators\":6}")),
            pb_msg(10, v9_carry),
            pb_string(11, "Are all 6 validators ready for Train A?"),
            pb_msg(12, v9_prop_dec),
            pb_msg(14, v9_session_cfg),
            pb_varint(15, 1764201800000),
        ]
    )
    # Canonical JSON representation (RFC 8785) wrapped in a Pillar 2 Envelope
    v9_packet_jcs_obj = {
        "authority": {
            "authorized_accounts": [founder_acct],
            "founder_nostr_pubkey": founder_nostr_pk,
            "signer_set_id": "founder-v1",
            "threshold": 1,
            "total_signers": 1,
        },
        "chain_id": chain_id,
        "created_at_unix_ms": 1764201800000,
        "previous_deliverables_status": [
            {
                "deliverable_id": "deliv-000-1",
                "origin_session_id": "gov-2026-11-20-001",
                "status": "DELIVERABLE_STATUS_DONE",
                "title": "Freeze C1 proto schemas",
            }
        ],
        "schema_version": 1,
        "session_id": gov_session_id,
        "session_number": 1,
        "title": "Governance Session #1",
    }
    v9_packet_jcs = canonical_json_bytes(v9_packet_jcs_obj)
    v9_env_header = enc_message_header(
        protocol_version=1,
        chain_id=chain_id,
        schema_name="pf.governance.packet.v1",
        schema_id=101,
        kind=12,  # CONTENT_KIND_GOVERNANCE
        visibility=1,  # PUBLIC
        body_encoding=2,  # BODY_ENCODING_CANONICAL_JSON
        cipher_suite=1,  # NONE
        sender_account=founder_acct,
        created_at_unix_ms=1764201800000,
        thread_id=gov_session_id,
    )
    v9_env = enc_envelope(
        version=1,
        header_bytes=v9_env_header,
        encryption_header_bytes=b"",
        body_bytes=v9_packet_jcs,
        signature_algorithm=1,
        sender_pubkey=fixture_pk,
        signature=fixture_sig,
    )
    v9_env_cid_bin, v9_env_cid_b32 = cidv1_raw_from_bytes(v9_env)
    v9_ptr = enc_pointer_v5(
        cid_bin=v9_env_cid_bin,
        target=1,
        kind=12,  # CONTENT_KIND_GOVERNANCE
        schema=101,
        thread_id=gov_session_id,
        flags=POINTER_FLAG_PUBLIC,
    )
    validate_l1_payment_memo(v1_memo_type_hex, v1_memo_format_hex, v9_ptr.hex())

    # Transcript root + SessionTranscript
    ev1_id = "11" * 32
    ev1_raw_sha = sha256(b"raw-nostr-event-1")
    ev1_cid_bin, ev1_cid_b32 = cidv1_raw_from_bytes(b"raw-nostr-event-1")
    v9_transcript_root = compute_transcript_root([(1, ev1_id, ev1_raw_sha)])
    md_cid_bin, md_cid_b32 = cidv1_raw_from_bytes(b"# Governance Session #1 Transcript\n")
    v9_entry = b"".join(
        [
            pb_varint(1, 1),
            pb_string(2, ev1_id),
            pb_string(3, founder_nostr_pk),
            pb_string(4, "founder"),
            pb_varint(5, 1764201810),
            pb_varint(6, 1764201810500),
            pb_bytes(7, ev1_raw_sha),
            pb_msg(8, enc_ipfs_cid_ref(ev1_cid_bin, 17, ev1_cid_b32)),
        ]
    )
    v9_excluded = b"".join(
        [
            pb_string(1, "22" * 32),
            pb_string(2, "33" * 32),
            pb_varint(3, 5),  # EXCLUSION_REASON_RATE_LIMITED
            pb_string(4, "exceeded 1 msg / 15s limit"),
        ]
    )
    v9_transcript = b"".join(
        [
            pb_varint(1, 1),
            pb_string(2, gov_session_id),
            pb_string(3, "hive-gov-2026-11-27-001"),
            pb_msg(4, enc_ipfs_cid_ref(v9_env_cid_bin, len(v9_env), v9_env_cid_b32)),
            pb_msg(5, enc_ipfs_cid_ref(elig_cid_bin, 8, elig_cid_b32)),
            pb_varint(6, 1764201800000),
            pb_varint(7, 1764203600000),
            pb_msg(8, v9_entry),
            pb_msg(9, v9_excluded),
            pb_msg(10, enc_ipfs_cid_ref(md_cid_bin, 35, md_cid_b32)),
            pb_bytes(11, v9_transcript_root),
        ]
    )
    v9_transcript_cid_bin, v9_transcript_cid_b32 = cidv1_raw_from_bytes(v9_transcript)

    # GovernanceMinutes
    v9_decision_rec = b"".join(
        [
            pb_string(1, "dec-001"),
            pb_string(2, "Activate Train A height"),
            pb_string(3, "Approved activation at height 50000"),
            pb_string(4, ev1_id),
            pb_bool(5, True),
        ]
    )
    v9_next_deliv = b"".join(
        [
            pb_string(1, "deliv-001-1"),
            pb_string(2, gov_session_id),
            pb_string(3, "Ship Train A rollout"),
            pb_string(4, "Roll out Train A to 6 validators"),
            pb_string(5, founder_acct),
            pb_string(6, "2026-12-04"),
            pb_string(7, ev1_id),
        ]
    )
    v9_comm_input = b"".join(
        [
            pb_string(1, "Validator readiness"),
            pb_string(2, "Two validators confirmed staging upgrades."),
            pb_string(3, "44" * 32),
        ]
    )
    v9_approval = b"".join(
        [
            pb_string(1, founder_acct),
            pb_varint(2, 1),
            pb_bytes(3, fixture_pk),
            pb_bytes(4, fixture_sig),
            pb_varint(5, 1764203700000),
        ]
    )
    v9_minutes = b"".join(
        [
            pb_varint(1, 1),
            pb_string(2, gov_session_id),
            pb_varint(3, 1),
            pb_varint(4, 6),  # SESSION_STATE_PUBLISHED
            pb_msg(5, v9_authority),
            pb_msg(6, enc_ipfs_cid_ref(v9_env_cid_bin, len(v9_env), v9_env_cid_b32)),
            pb_msg(7, enc_ipfs_cid_ref(v9_transcript_cid_bin, len(v9_transcript), v9_transcript_cid_b32)),
            pb_bytes(8, v9_transcript_root),
            pb_string(9, "Approved Train A activation and scheduled rollout."),
            pb_msg(10, v9_carry),
            pb_msg(11, v9_decision_rec),
            pb_msg(12, v9_next_deliv),
            pb_msg(13, v9_comm_input),
            pb_msg(14, v9_approval),
            pb_varint(15, 1764203700000),
        ]
    )
    v9_minutes_cid_bin, v9_minutes_cid_b32 = cidv1_raw_from_bytes(v9_minutes)

    # RulingAnchor
    ruling_art_cid_bin, ruling_art_cid_b32 = cidv1_raw_from_bytes(b"{\"verdict\":\"APPROVE\"}")
    v9_ruling = b"".join(
        [
            pb_varint(1, 1),
            pb_string(2, gov_session_id),
            pb_string(3, "dec-001"),
            pb_string(4, "ruling-2026-11-27-001"),
            pb_varint(5, 1),  # RULING_COMPUTE_BACKEND_GOOGLE
            pb_msg(6, enc_ipfs_cid_ref(snap_raw_cid_bin, 31, snap_raw_cid_b32)),
            pb_msg(7, enc_ipfs_cid_ref(v9_env_cid_bin, len(v9_env), v9_env_cid_b32)),
            pb_msg(8, enc_ipfs_cid_ref(v9_minutes_cid_bin, len(v9_minutes), v9_minutes_cid_b32)),
            pb_msg(9, enc_ipfs_cid_ref(ruling_art_cid_bin, 21, ruling_art_cid_b32)),
            pb_bytes(10, sha256(b"ruling-input-bundle-v1")),
            pb_bytes(11, sha256(b"{\"verdict\":\"APPROVE\"}")),
            pb_bytes(12, sha256(b"ruling-replay-trace-v1")),
            pb_string(13, "sha256:deterministic-model-v1"),
            pb_string(14, "constitution-v1"),
            pb_string(15, "APPROVE"),
            pb_msg(16, v9_authority),
            pb_msg(17, v9_approval),
            pb_varint(18, 1764204000000),
        ]
    )

    return {
        "V1_POINTER_V5_PUBLIC_TEXT": {
            "proto_type": "postfiat.message.v1.Pointer",
            "memo_type_hex": v1_memo_type_hex,
            "memo_format_hex": v1_memo_format_hex,
            "memo_data_hex": v1_memo_data_hex,
            "byte_len": len(v1_ptr),
            "cid_bin_hex": v4_cid_bin.hex(),
            "cid_multibase": v4_cid_b32,
        },
        "V2A_POINTER_V5_WORST_CASE_BUDGET": {
            "proto_type": "postfiat.message.v1.Pointer",
            "memo_data_hex": v2a_ptr.hex(),
            "byte_len": len(v2a_ptr),
        },
        "V2B_POINTER_V5_INLINE_X402": {
            "proto_type": "postfiat.message.v1.Pointer",
            "request_hash_hex": x402_req_hash.hex(),
            "memo_data_hex": v2b_ptr.hex(),
            "byte_len": len(v2b_ptr),
        },
        "V3_LEGACY_POINTER_V4_DECODE": {
            "proto_type": "postfiat.message.v1.LegacyPointerV4",
            "memo_type_hex": "70662E707472",
            "memo_format_hex": "7634",
            "memo_data_hex_upper": v3_ptr_v4.hex().upper(),
            "byte_len": len(v3_ptr_v4),
        },
        "V4_ENVELOPE_PUBLIC_TEXT": {
            "proto_type": "postfiat.message.v1.Envelope",
            "header_hex": v4_header.hex(),
            "body_hex": v4_body.hex(),
            "signing_preimage_sha256_hex": sha256(v4_preimage).hex(),
            "envelope_hex": v4_envelope.hex(),
            "cid_bin_hex": v4_cid_bin.hex(),
            "cid_multibase": v4_cid_b32,
        },
        "V5_ENVELOPE_PRIVATE_XWING_REWARD": {
            "proto_type": "postfiat.message.v1.Envelope",
            "order_hash_hex": v5_order_hash.hex(),
            "header_hex": v5_header.hex(),
            "aad_context_hex": v5_aad.hex(),
            "derived_kem_seed_hex": derived_kem_seed.hex(),
            "recipient_key_id_hex": recip_key_id.hex(),
            "derived_wrap_kek_hex": derived_kek.hex(),
            "key_binding_preimage_sha256_hex": sha256(v5_key_bind_preimage).hex(),
            "encryption_header_hex": v5_enc_header.hex(),
            "signing_preimage_sha256_hex": sha256(v5_preimage).hex(),
            "envelope_hex": v5_envelope.hex(),
            "cid_bin_hex": v5_cid_bin.hex(),
            "cid_multibase": v5_cid_b32,
        },
        "V6_MANIFEST_MULTICHUNK": {
            "proto_type": "postfiat.message.v1.Manifest",
            "manifest_hex": v6_manifest.hex(),
            "byte_len": len(v6_manifest),
        },
        "V7_X402_HEADERS": {
            "required_proto_type": "postfiat.message.v1.X402PaymentRequired",
            "required_hex": v7_required.hex(),
            "signature_proto_type": "postfiat.message.v1.X402PaymentSignature",
            "signature_hex": v7_signature.hex(),
            "response_proto_type": "postfiat.message.v1.X402PaymentResponse",
            "response_hex": v7_response.hex(),
            "dedup_order_hash_hex": v7_order_hash.hex(),
        },
        "V8_NFT_METADATA_AND_MIGRATION": {
            "claim_proto_type": "postfiat.nft.v1.MigrationClaim",
            "claim_hex": v8_claim.hex(),
            "claim_envelope_cid_multibase": v8_claim_cid_b32,
            "claim_hash_hex": v8_claim_hash.hex(),
            "migrated_serial_uint64": migrated_serial,
            "metadata_proto_type": "postfiat.nft.v1.NftMetadata",
            "metadata_hex": v8_metadata.hex(),
            "metadata_envelope_hex": v8_meta_env.hex(),
            "onchain_metadata_uri": v8_metadata_uri,
            "onchain_metadata_hash_sha3_384_hex": v8_metadata_hash_hex,
        },
        "V9_GOVERNANCE_PACKET_AND_LIFECYCLE": {
            "packet_proto_type": "postfiat.governance.v1.GovernancePacket",
            "packet_proto_hex": v9_packet_proto.hex(),
            "packet_canonical_json_sha256_hex": sha256(v9_packet_jcs).hex(),
            "packet_envelope_hex": v9_env.hex(),
            "packet_envelope_cid_multibase": v9_env_cid_b32,
            "anchor_pointer_v5_hex": v9_ptr.hex(),
            "anchor_pointer_v5_len": len(v9_ptr),
            "transcript_proto_type": "postfiat.governance.v1.SessionTranscript",
            "transcript_root_sha256_hex": v9_transcript_root.hex(),
            "transcript_hex": v9_transcript.hex(),
            "minutes_proto_type": "postfiat.governance.v1.GovernanceMinutes",
            "minutes_hex": v9_minutes.hex(),
            "ruling_proto_type": "postfiat.governance.v1.RulingAnchor",
            "ruling_hex": v9_ruling.hex(),
        },
    }


# ---------------------------------------------------------------------------
# .proto field tables: the schema the vectors are encoded against (pure stdlib, no protoc)
# ---------------------------------------------------------------------------

PROTO_VARINT_SCALARS = {"bool", "uint32", "uint64"}
PROTO_LENGTH_SCALARS = {"bytes", "string"}
PROTO_MAP_STRING_STRING = "map<string,string>"
PROTO_REPEATED = "repeated "
PROTO_UNSUPPORTED_STATEMENTS = {"message", "enum", "oneof", "option", "optional", "extend", "extensions", "service"}
PROTO_TOKEN_RE = re.compile(r'\s*(?:([A-Za-z_][A-Za-z0-9_.]*)|(\d+)|("[^"\n]*")|([{};=<>,]))')

# Every message declared in the .proto files, field by field as "<number> <type> <name>" in the spelling of the
# .proto files. verify_proto_field_tables() fails when a .proto file declares a different number, label, type or
# name for any of them, or declares a message that has no table here, and decodes every vector byte string
# against the parsed .proto files.
PROTO_MESSAGE_FIELDS: dict[str, tuple[str, ...]] = {
    "postfiat.message.v1.IpfsCidRef": ("1 bytes cid_bin", "2 uint64 byte_length", "3 string multibase_cid"),
    "postfiat.message.v1.Pointer": (
        "1 bytes cid_bin", "2 PointerTarget target", "3 ContentKind kind", "4 uint32 schema", "5 string task_id",
        "6 string thread_id", "7 string context_id", "8 uint32 flags", "9 bytes inline_payload",
        "10 string schema_name",
    ),
    "postfiat.message.v1.LegacyPointerV4": (
        "1 string cid", "2 uint32 target", "3 ContentKind kind", "4 uint32 schema", "5 string task_id",
        "6 string thread_id", "7 string context_id", "8 uint32 flags",
    ),
    "postfiat.message.v1.RecipientWrap": (
        "1 bytes recipient_key_id", "2 string recipient_account", "3 bytes xwing_ciphertext", "4 bytes wrap_nonce",
        "5 bytes wrapped_cek",
    ),
    "postfiat.message.v1.XWingEncryptionHeader": (
        "1 CipherSuite cipher_suite", "2 bytes aead_nonce", "3 bytes aad_context", "4 uint32 chunk_index",
        "5 string kdf_info", "6 repeated RecipientWrap recipients",
    ),
    "postfiat.message.v1.MessageKeyBinding": (
        "1 string chain_id", "2 string account", "3 bytes xwing_public_key", "4 bytes key_id",
        "5 uint64 created_at_unix_ms", "6 uint32 key_epoch", "7 SignatureAlgorithm signature_algorithm",
        "8 bytes account_pubkey", "9 bytes signature",
    ),
    "postfiat.message.v1.FastPayPaymentRef": (
        "1 PaymentLane lane", "2 string chain_id", "3 bytes order_hash", "4 bytes certificate_digest",
        "5 bytes tx_hash", "6 string asset_id", "7 string payer_account", "8 string payee_account",
        "9 uint64 amount_atoms", "10 uint64 fee_atoms", "11 uint64 finalized_height",
    ),
    "postfiat.message.v1.X402PaymentRequired": (
        "1 uint32 x402_version", "2 X402Scheme scheme", "3 string network", "4 PaymentLane lane", "5 string asset_id",
        "6 uint64 max_amount_atoms", "7 string pay_to", "8 bytes request_hash", "9 bytes nonce",
        "10 uint64 expires_at_unix_ms", "11 string resource_uri", "12 string facilitator_url",
    ),
    "postfiat.message.v1.X402PaymentSignature": (
        "1 uint32 x402_version", "2 X402Scheme scheme", "3 string network", "4 PaymentLane lane",
        "5 bytes request_hash", "6 bytes nonce", "7 bytes order_hash", "8 bytes tx_hash", "9 bytes signed_payload",
        "10 bytes certificate_payload",
    ),
    "postfiat.message.v1.X402PaymentResponse": (
        "1 uint32 x402_version", "2 bool settled", "3 FastPayPaymentRef payment_ref", "4 uint64 settled_at_unix_ms",
        "5 string facilitator_id",
    ),
    "postfiat.message.v1.MessageHeader": (
        "1 uint32 protocol_version", "2 string chain_id", "3 string schema_name", "4 uint32 schema_id",
        "5 ContentKind kind", "6 Visibility visibility", "7 BodyEncoding body_encoding", "8 CipherSuite cipher_suite",
        "9 string sender_account", "10 uint64 created_at_unix_ms", "11 string task_id", "12 string thread_id",
        "13 string context_id", "14 IpfsCidRef reply_to_cid", "15 FastPayPaymentRef payment_ref",
        "16 X402PaymentResponse x402_receipt",
    ),
    "postfiat.message.v1.Envelope": (
        "1 uint32 version", "2 bytes header_bytes", "3 bytes encryption_header_bytes", "4 bytes body_bytes",
        "5 SignatureAlgorithm signature_algorithm", "6 bytes sender_pubkey", "7 bytes signature",
    ),
    "postfiat.message.v1.ManifestChunk": ("1 uint32 chunk_index", "2 IpfsCidRef cid", "3 bytes aead_nonce"),
    "postfiat.message.v1.Manifest": (
        "1 uint32 version", "2 uint64 total_plaintext_bytes", "3 uint32 chunk_size_bytes", "4 string media_type",
        "5 string inner_schema_name", "6 BodyEncoding inner_body_encoding", "7 CipherSuite cipher_suite",
        "8 repeated ManifestChunk chunks",
    ),
    "postfiat.message.v1.TextMessage": ("1 string text", "2 string format", "3 repeated IpfsCidRef attachments"),
    "postfiat.message.v1.RewardNotice": (
        "1 string reward_id", "2 string task_id", "3 string recipient_account", "4 uint64 amount_atoms",
        "5 string asset_id", "6 string summary", "7 FastPayPaymentRef payment_ref",
    ),
    "postfiat.nft.v1.ContentHash": (
        "1 string uri", "2 HashAlgorithm algorithm", "3 bytes digest", "4 uint64 byte_length",
    ),
    "postfiat.nft.v1.MediaAttachment": (
        "1 string uri", "2 postfiat.message.v1.IpfsCidRef cid", "3 string mime_type", "4 MediaRole role",
        "5 uint32 width", "6 uint32 height", "7 uint64 byte_length", "8 bytes sha3_384_digest", "9 bool encrypted",
    ),
    "postfiat.nft.v1.Attribute": ("1 string trait_type", "2 string value", "3 string display_type"),
    "postfiat.nft.v1.CollectionRef": (
        "1 string collection_id", "2 string issuer_account", "3 string name", "4 string description",
        "5 string external_url",
    ),
    "postfiat.nft.v1.OriginProvenance": (
        "1 string source_chain", "2 string nftoken_id", "3 string original_issuer", "4 string original_owner",
        "5 string original_uri", "6 string mint_tx", "7 uint64 mint_ledger_index", "8 RetirementMode retirement_mode",
        "9 string burn_tx", "10 uint64 burn_ledger_index", "11 postfiat.message.v1.IpfsCidRef claim_cid",
        "12 bytes claim_hash", "13 postfiat.message.v1.IpfsCidRef burn_proof_bundle_cid", "14 uint64 migrated_serial",
    ),
    "postfiat.nft.v1.NftMetadata": (
        "1 uint32 schema_version", "2 string name", "3 string description", "4 string image",
        "5 postfiat.message.v1.IpfsCidRef image_cid", "6 repeated MediaAttachment media",
        "7 repeated Attribute attributes", "8 CollectionRef collection", "9 repeated ContentHash content_hashes",
        "10 bytes prompt_digest", "11 string license", "12 string external_url", "13 OriginProvenance origin",
        "14 map<string,string> extensions",
    ),
    "postfiat.nft.v1.MigrationClaim": (
        "1 uint32 schema_version", "2 string target_chain_id", "3 string source_chain", "4 string nftoken_id",
        "5 string old_owner", "6 string new_owner_account", "7 string target_collection_id",
        "8 string migration_issuer_account", "9 uint64 migrated_serial", "10 RetirementMode retirement_mode",
        "11 uint64 cutoff_ledger_index", "12 uint64 claimed_at_unix_ms",
        "13 postfiat.message.v1.IpfsCidRef allowlist_cid",
    ),
    "postfiat.nft.v1.PinOnlyProvenance": (
        "1 uint32 schema_version", "2 string target_chain_id", "3 string claimant_account", "4 OriginProvenance origin",
        "5 string old_wallet_signature_hex", "6 uint64 recorded_at_unix_ms",
    ),
    "postfiat.governance.v1.AuthorityBinding": (
        "1 string signer_set_id", "2 uint32 threshold", "3 uint32 total_signers",
        "4 repeated string authorized_accounts", "5 string founder_nostr_pubkey", "6 string identity_handle",
    ),
    "postfiat.governance.v1.DeliverableItem": (
        "1 string deliverable_id", "2 string origin_session_id", "3 string title", "4 string description",
        "5 string owner", "6 string target_date", "7 repeated string cited_event_ids",
    ),
    "postfiat.governance.v1.DeliverableCarryOver": (
        "1 string deliverable_id", "2 string origin_session_id", "3 string title", "4 DeliverableStatus status",
        "5 string evidence_note", "6 repeated string evidence_refs",
    ),
    "postfiat.governance.v1.SnapshotSourceRecord": (
        "1 string source_id", "2 string endpoint", "3 uint64 block_height", "4 uint64 captured_at_unix_ms",
        "5 bytes raw_response_sha256", "6 postfiat.message.v1.IpfsCidRef raw_response_cid",
    ),
    "postfiat.governance.v1.ProtocolSnapshot": (
        "1 uint32 schema_version", "2 string session_id", "3 string chain_id", "4 uint64 finalized_block_height",
        "5 bytes finalized_block_hash", "6 uint64 captured_at_unix_ms", "7 uint32 active_validator_count",
        "8 string bridge_status_summary", "9 string navcoin_supply_summary", "10 string cobalt_verdict_summary",
        "11 string tasknode_health_summary", "12 string previous_session_id",
        "13 postfiat.message.v1.IpfsCidRef previous_minutes_cid", "14 repeated DeliverableItem carried_deliverables",
        "15 repeated SnapshotSourceRecord sources",
    ),
    "postfiat.governance.v1.ProposedDecision": (
        "1 string decision_id", "2 string title", "3 string summary", "4 bool run_deterministic_ruling",
    ),
    "postfiat.governance.v1.SessionConfig": (
        "1 string channel_id", "2 PostingPolicy posting_policy", "3 FacilitatorMode facilitator_mode",
        "4 uint32 duration_minutes", "5 uint32 rate_limit_seconds", "6 uint32 max_non_founder_messages",
        "7 uint32 max_facilitator_turns", "8 uint32 reserved_founder_turns",
        "9 postfiat.message.v1.IpfsCidRef eligible_pubkeys_cid", "10 bytes eligible_pubkeys_sha256",
    ),
    "postfiat.governance.v1.GovernancePacket": (
        "1 uint32 schema_version", "2 string session_id", "3 string chain_id", "4 uint64 session_number",
        "5 string title", "6 string summary", "7 AuthorityBinding authority",
        "8 postfiat.message.v1.IpfsCidRef snapshot_cid", "9 bytes snapshot_sha256",
        "10 repeated DeliverableCarryOver previous_deliverables_status", "11 repeated string open_questions",
        "12 repeated ProposedDecision proposed_decisions", "13 repeated postfiat.message.v1.IpfsCidRef attachments",
        "14 SessionConfig session_config", "15 uint64 created_at_unix_ms",
    ),
    "postfiat.governance.v1.TranscriptEntry": (
        "1 uint64 receipt_seq", "2 string nostr_event_id", "3 string author_pubkey", "4 string author_role",
        "5 uint64 event_created_at_unix_s", "6 uint64 server_received_at_unix_ms", "7 bytes raw_event_sha256",
        "8 postfiat.message.v1.IpfsCidRef raw_event_cid",
    ),
    "postfiat.governance.v1.ExcludedEvent": (
        "1 string nostr_event_id", "2 string author_pubkey", "3 ExclusionReason reason", "4 string detail",
    ),
    "postfiat.governance.v1.SessionTranscript": (
        "1 uint32 schema_version", "2 string session_id", "3 string channel_id",
        "4 postfiat.message.v1.IpfsCidRef opening_packet_cid", "5 postfiat.message.v1.IpfsCidRef eligible_pubkeys_cid",
        "6 uint64 opened_at_unix_ms", "7 uint64 frozen_at_unix_ms", "8 repeated TranscriptEntry included_entries",
        "9 repeated ExcludedEvent excluded_events", "10 postfiat.message.v1.IpfsCidRef markdown_transcript_cid",
        "11 bytes transcript_root_sha256",
    ),
    "postfiat.governance.v1.DecisionRecord": (
        "1 string decision_id", "2 string title", "3 string resolution", "4 repeated string cited_event_ids",
        "5 bool run_deterministic_ruling",
    ),
    "postfiat.governance.v1.CommunityInputSummary": (
        "1 string topic", "2 string summary", "3 repeated string cited_event_ids",
    ),
    "postfiat.governance.v1.ApprovalSignature": (
        "1 string signer_account", "2 postfiat.message.v1.SignatureAlgorithm signature_algorithm",
        "3 bytes signer_pubkey", "4 bytes signature", "5 uint64 signed_at_unix_ms",
    ),
    "postfiat.governance.v1.GovernanceMinutes": (
        "1 uint32 schema_version", "2 string session_id", "3 uint32 revision", "4 SessionState state",
        "5 AuthorityBinding authority", "6 postfiat.message.v1.IpfsCidRef opening_packet_cid",
        "7 postfiat.message.v1.IpfsCidRef transcript_cid", "8 bytes transcript_root_sha256", "9 string summary",
        "10 repeated DeliverableCarryOver previous_deliverables_review", "11 repeated DecisionRecord decisions",
        "12 repeated DeliverableItem next_deliverables", "13 repeated CommunityInputSummary community_input",
        "14 repeated ApprovalSignature approvals", "15 uint64 finalized_at_unix_ms",
    ),
    "postfiat.governance.v1.RulingAnchor": (
        "1 uint32 schema_version", "2 string session_id", "3 string decision_id", "4 string ruling_id",
        "5 RulingComputeBackend compute_backend", "6 postfiat.message.v1.IpfsCidRef snapshot_cid",
        "7 postfiat.message.v1.IpfsCidRef opening_packet_cid", "8 postfiat.message.v1.IpfsCidRef minutes_cid",
        "9 postfiat.message.v1.IpfsCidRef ruling_artifact_cid", "10 bytes input_bundle_sha256",
        "11 bytes output_ruling_sha256", "12 bytes replay_trace_sha256", "13 string model_digest",
        "14 string constitution_version", "15 string verdict", "16 AuthorityBinding authority",
        "17 repeated ApprovalSignature approvals", "18 uint64 anchored_at_unix_ms",
    ),
}


class ProtoTokens:
    def __init__(self, rel: str, src: str) -> None:
        self.rel = rel
        self.tokens: list[str] = []
        src = re.sub(r"//[^\n]*", "", src)
        pos = 0
        while src[pos:].strip():
            match = PROTO_TOKEN_RE.match(src, pos)
            if not match:
                raise AssertionError(f"{rel}: cannot parse near {src[pos:pos + 40].strip()!r}")
            self.tokens.append(match.group(match.lastindex))
            pos = match.end()
        self.pos = 0

    def more(self) -> bool:
        return self.pos < len(self.tokens)

    def take(self, expected: str | None = None) -> str:
        if not self.more():
            raise AssertionError(f"{self.rel}: unexpected end of file")
        token = self.tokens[self.pos]
        self.pos += 1
        if expected is not None and token != expected:
            raise AssertionError(f"{self.rel}: expected {expected!r}, found {token!r}")
        return token

    def take_number(self) -> int:
        token = self.take()
        if not token.isdigit():
            raise AssertionError(f"{self.rel}: expected a number, found {token!r}")
        return int(token)

    def skip_statement(self) -> None:
        while self.take() != ";":
            pass


def parse_proto_message(tok: ProtoTokens, full_name: str) -> dict[int, tuple[str, str]]:
    fields: dict[int, tuple[str, str]] = {}
    tok.take("{")
    while (token := tok.take()) != "}":
        if token == "reserved":
            tok.skip_statement()
            continue
        if token in PROTO_UNSUPPORTED_STATEMENTS:
            raise AssertionError(f"{tok.rel}: `{token}` inside {full_name} is not supported by this check")
        if token == "map":
            tok.take("<")
            key_type = tok.take()
            tok.take(",")
            value_type = tok.take()
            tok.take(">")
            field_type = f"map<{key_type},{value_type}>"
        elif token == "repeated":
            field_type = PROTO_REPEATED + tok.take()
        else:
            field_type = token
        name = tok.take()
        tok.take("=")
        number = tok.take_number()
        tok.take(";")
        if number < 1 or number in fields or name in {n for _, n in fields.values()}:
            raise AssertionError(f"{tok.rel}: {full_name} reuses or misnumbers field {number} `{name}`")
        fields[number] = (field_type, name)
    return fields


def parse_proto_enum(tok: ProtoTokens, full_name: str) -> dict[str, int]:
    values: dict[str, int] = {}
    tok.take("{")
    while (token := tok.take()) != "}":
        if token == "reserved":
            tok.skip_statement()
            continue
        tok.take("=")
        number = tok.take_number()
        tok.take(";")
        if token in values or number in values.values():
            raise AssertionError(f"{tok.rel}: {full_name} reuses enum value {number} `{token}`")
        values[token] = number
    return values


def resolve_proto_type(field_type: str, package: str, known_types: set[str], where: str) -> str:
    label = PROTO_REPEATED if field_type.startswith(PROTO_REPEATED) else ""
    base = field_type.removeprefix(PROTO_REPEATED)
    if base in PROTO_VARINT_SCALARS or base in PROTO_LENGTH_SCALARS or base == PROTO_MAP_STRING_STRING:
        return label + base
    full = base if "." in base else f"{package}.{base}"
    if full not in known_types:
        raise AssertionError(f"{where}: unknown or unsupported field type `{base}`")
    return label + full


def parse_proto_sources() -> tuple[dict[str, dict[int, tuple[str, str]]], dict[str, dict[str, int]]]:
    """Parse the three .proto files into {message: {number: (resolved type, name)}} and {enum: {name: value}}."""
    messages: dict[str, dict[int, tuple[str, str]]] = {}
    enums: dict[str, dict[str, int]] = {}
    packages: dict[str, str] = {}
    for rel in PROTO_FILES:
        tok = ProtoTokens(rel, (PROTO_ROOT / rel).read_text(encoding="utf-8"))
        package = ""
        while tok.more():
            token = tok.take()
            if token == "syntax":
                tok.take("=")
                tok.take('"proto3"')
                tok.take(";")
            elif token == "package":
                package = tok.take()
                tok.take(";")
            elif token == "import":
                tok.skip_statement()
            elif token in ("message", "enum"):
                if not package:
                    raise AssertionError(f"{rel}: `{token}` declared before the package statement")
                full_name = f"{package}.{tok.take()}"
                if full_name in messages or full_name in enums:
                    raise AssertionError(f"{rel}: {full_name} is declared twice")
                if token == "message":
                    messages[full_name] = parse_proto_message(tok, full_name)
                    packages[full_name] = package
                else:
                    enums[full_name] = parse_proto_enum(tok, full_name)
            else:
                raise AssertionError(f"{rel}: unexpected top-level token {token!r}")
    known_types = set(messages) | set(enums)
    for full_name, fields in messages.items():
        for number, (field_type, name) in fields.items():
            where = f"{full_name}.{name}"
            fields[number] = (resolve_proto_type(field_type, packages[full_name], known_types, where), name)
    return messages, enums


def require_utf8(value: bytes, where: str) -> None:
    try:
        value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise AssertionError(f"{where} is not valid UTF-8") from exc


def decode_against_proto(
    buf: bytes,
    message: str,
    messages: dict[str, dict[int, tuple[str, str]]],
    enums: dict[str, dict[str, int]],
    visited: set[str],
    label: str,
) -> None:
    """Strictly decode one vector byte string against the parsed .proto declaration of `message`."""
    visited.add(message)
    fields = messages[message]
    seen: set[int] = set()
    for number, wire_type, value in decode_wire_fields(buf):
        if number not in fields:
            raise AssertionError(f"{label}: field {number} is not declared in {message}")
        field_type, name = fields[number]
        where = f"{label}: {message}.{name} (field {number})"
        base = field_type.removeprefix(PROTO_REPEATED)
        repeated = base != field_type or base == PROTO_MAP_STRING_STRING
        if number in seen and not repeated:
            raise AssertionError(f"{where} appears twice but is not repeated")
        seen.add(number)
        expected_wire_type = 0 if base in PROTO_VARINT_SCALARS or base in enums else 2
        if wire_type != expected_wire_type:
            raise AssertionError(f"{where} has wire type {wire_type}, but the .proto type `{base}` needs {expected_wire_type}")
        if base in enums and value not in enums[base].values():
            raise AssertionError(f"{where} holds {value}, which {base} does not define")
        if base in messages:
            decode_against_proto(value, base, messages, enums, visited, label)
        elif base == "string":
            require_utf8(value, where)
        elif base == PROTO_MAP_STRING_STRING:
            for entry_number, entry_wire_type, entry_value in decode_wire_fields(value):
                if entry_number not in (1, 2) or entry_wire_type != 2:
                    raise AssertionError(f"{where} has a malformed map entry")
                require_utf8(entry_value, where)


def vector_proto_payloads(vectors: dict[str, dict[str, Any]]) -> list[tuple[str, str, bytes]]:
    """Every protobuf byte string published in the vectors, with the message type it encodes."""
    return [
        ("V1", "postfiat.message.v1.Pointer", bytes.fromhex(vectors["V1_POINTER_V5_PUBLIC_TEXT"]["memo_data_hex"])),
        ("V2A", "postfiat.message.v1.Pointer", bytes.fromhex(vectors["V2A_POINTER_V5_WORST_CASE_BUDGET"]["memo_data_hex"])),
        ("V2B", "postfiat.message.v1.Pointer", bytes.fromhex(vectors["V2B_POINTER_V5_INLINE_X402"]["memo_data_hex"])),
        ("V3", "postfiat.message.v1.LegacyPointerV4", bytes.fromhex(vectors["V3_LEGACY_POINTER_V4_DECODE"]["memo_data_hex_upper"])),
        ("V4.env", "postfiat.message.v1.Envelope", bytes.fromhex(vectors["V4_ENVELOPE_PUBLIC_TEXT"]["envelope_hex"])),
        ("V4.hdr", "postfiat.message.v1.MessageHeader", bytes.fromhex(vectors["V4_ENVELOPE_PUBLIC_TEXT"]["header_hex"])),
        ("V4.body", "postfiat.message.v1.TextMessage", bytes.fromhex(vectors["V4_ENVELOPE_PUBLIC_TEXT"]["body_hex"])),
        ("V5.env", "postfiat.message.v1.Envelope", bytes.fromhex(vectors["V5_ENVELOPE_PRIVATE_XWING_REWARD"]["envelope_hex"])),
        ("V5.hdr", "postfiat.message.v1.MessageHeader", bytes.fromhex(vectors["V5_ENVELOPE_PRIVATE_XWING_REWARD"]["header_hex"])),
        ("V5.xwing", "postfiat.message.v1.XWingEncryptionHeader", bytes.fromhex(vectors["V5_ENVELOPE_PRIVATE_XWING_REWARD"]["encryption_header_hex"])),
        ("V6", "postfiat.message.v1.Manifest", bytes.fromhex(vectors["V6_MANIFEST_MULTICHUNK"]["manifest_hex"])),
        ("V7.req", "postfiat.message.v1.X402PaymentRequired", bytes.fromhex(vectors["V7_X402_HEADERS"]["required_hex"])),
        ("V7.sig", "postfiat.message.v1.X402PaymentSignature", bytes.fromhex(vectors["V7_X402_HEADERS"]["signature_hex"])),
        ("V7.resp", "postfiat.message.v1.X402PaymentResponse", bytes.fromhex(vectors["V7_X402_HEADERS"]["response_hex"])),
        ("V8.claim", "postfiat.nft.v1.MigrationClaim", bytes.fromhex(vectors["V8_NFT_METADATA_AND_MIGRATION"]["claim_hex"])),
        ("V8.meta", "postfiat.nft.v1.NftMetadata", bytes.fromhex(vectors["V8_NFT_METADATA_AND_MIGRATION"]["metadata_hex"])),
        ("V8.env", "postfiat.message.v1.Envelope", bytes.fromhex(vectors["V8_NFT_METADATA_AND_MIGRATION"]["metadata_envelope_hex"])),
        ("V9.pkt", "postfiat.governance.v1.GovernancePacket", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["packet_proto_hex"])),
        ("V9.env", "postfiat.message.v1.Envelope", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["packet_envelope_hex"])),
        ("V9.ptr", "postfiat.message.v1.Pointer", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["anchor_pointer_v5_hex"])),
        ("V9.tr", "postfiat.governance.v1.SessionTranscript", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["transcript_hex"])),
        ("V9.min", "postfiat.governance.v1.GovernanceMinutes", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["minutes_hex"])),
        ("V9.rul", "postfiat.governance.v1.RulingAnchor", bytes.fromhex(vectors["V9_GOVERNANCE_PACKET_AND_LIFECYCLE"]["ruling_hex"])),
    ]


def verify_proto_field_tables(vectors: dict[str, dict[str, Any]]) -> tuple[int, int]:
    """Fail when the .proto files disagree with the script's message tables or with the vector bytes."""
    messages, enums = parse_proto_sources()
    known_types = set(messages) | set(enums)
    for kind, declared_names, tables in (
        ("message", messages, PROTO_MESSAGE_FIELDS),
    ):
        untabled = sorted(set(declared_names) - set(tables))
        if untabled:
            raise AssertionError(f"{kind} declared in the .proto files without a table in this script: {', '.join(untabled)}")
        undeclared = sorted(set(tables) - set(declared_names))
        if undeclared:
            raise AssertionError(f"{kind} in this script's tables but not declared in the .proto files: {', '.join(undeclared)}")

    for message, entries in PROTO_MESSAGE_FIELDS.items():
        package = message.rsplit(".", 1)[0]
        expected: dict[int, tuple[str, str]] = {}
        for entry in entries:
            number, *field_type, name = entry.split()
            expected[int(number)] = (resolve_proto_type(" ".join(field_type), package, known_types, message), name)
        declared = messages[message]
        if declared != expected:
            differences = [
                f"field {number}: .proto {declared.get(number)} vs script {expected.get(number)}"
                for number in sorted(set(declared) | set(expected))
                if declared.get(number) != expected.get(number)
            ]
            raise AssertionError(f"{message} disagrees with the script's field table: " + "; ".join(differences))

    visited: set[str] = set()
    for label, message, raw_bytes in vector_proto_payloads(vectors):
        decode_against_proto(raw_bytes, message, messages, enums, visited, label)
    return len(PROTO_MESSAGE_FIELDS), len(visited)


# ---------------------------------------------------------------------------
# Spec-doc extraction & verification
# ---------------------------------------------------------------------------

VECTOR_BLOCK_RE = re.compile(
    r"<!--\s*BEGIN_CANONICAL_TEST_VECTORS_JSON\s*-->\s*```json\s*(\{.*?\})\s*```\s*<!--\s*END_CANONICAL_TEST_VECTORS_JSON\s*-->",
    re.DOTALL,
)


def extract_doc_vectors(doc_path: Path) -> dict[str, dict[str, Any]]:
    text = doc_path.read_text(encoding="utf-8")
    match = VECTOR_BLOCK_RE.search(text)
    if not match:
        raise AssertionError(
            f"{doc_path}: missing canonical test vectors JSON block between "
            "BEGIN_CANONICAL_TEST_VECTORS_JSON and END_CANONICAL_TEST_VECTORS_JSON"
        )
    return json.loads(match.group(1))


def verify_proto_sources_pure_python() -> None:
    """Verify proto3 syntax, package declarations, and critical enum values without external tools."""
    expected_packages = {
        "postfiat/message/v1/message.proto": "postfiat.message.v1",
        "postfiat/nft/v1/metadata.proto": "postfiat.nft.v1",
        "postfiat/governance/v1/packet.proto": "postfiat.governance.v1",
    }
    for rel, pkg in expected_packages.items():
        path = PROTO_ROOT / rel
        if not path.is_file():
            raise AssertionError(f"missing proto file: {path}")
        src = path.read_text(encoding="utf-8")
        if 'syntax = "proto3";' not in src:
            raise AssertionError(f"{rel}: missing proto3 syntax declaration")
        if f"package {pkg};" not in src:
            raise AssertionError(f"{rel}: missing expected package `{pkg}`")

    msg_src = (PROTO_ROOT / "postfiat/message/v1/message.proto").read_text(encoding="utf-8")
    for enum_entry in (
        "CONTENT_KIND_TASK = 1;",
        "CONTENT_KIND_REWARD = 6;",
        "CONTENT_KIND_SYSTEM = 11;",
        "CONTENT_KIND_GOVERNANCE = 12;",
        "CONTENT_KIND_NFT = 13;",
        "CONTENT_KIND_X402 = 14;",
        "CONTENT_KIND_TEST = 99;",
        "POINTER_FLAG_PRIVATE = 1;",
        "POINTER_FLAG_PUBLIC = 2;",
        "POINTER_FLAG_SEALED = 32;",
        "CIPHER_SUITE_XWING_XCHACHA20_POLY1305 = 2;",
        "SIGNATURE_ALGORITHM_ML_DSA_65 = 1;",
    ):
        if enum_entry not in msg_src:
            raise AssertionError(f"message.proto missing locked enum constant: {enum_entry}")


def verify_with_protoc(vectors: dict[str, dict[str, Any]], require_protoc: bool) -> bool:
    protoc = shutil.which("protoc")
    if not protoc:
        if require_protoc:
            raise AssertionError("--require-protoc set, but `protoc` was not found on PATH")
        return False

    with tempfile.TemporaryDirectory(prefix="postfiat-c1-protoc-") as tmpdir:
        desc_path = Path(tmpdir) / "descriptors.pb"
        cmd = [
            protoc,
            f"-I{PROTO_ROOT}",
            "--include_imports",
            f"--descriptor_set_out={desc_path}",
            *[str(PROTO_ROOT / rel) for rel in PROTO_FILES],
        ]
        subprocess.run(cmd, check=True, capture_output=True, text=True)

        # Cross-decode every protobuf hex vector via `protoc --decode`
        for label, message_type, raw_bytes in vector_proto_payloads(vectors):
            dec = subprocess.run(
                [protoc, f"--descriptor_set_in={desc_path}", f"--decode={message_type}"],
                input=raw_bytes,
                check=True,
                capture_output=True,
            )
            # Re-encode text format via protoc --encode and assert byte-for-byte identity!
            reenc = subprocess.run(
                [protoc, f"--descriptor_set_in={desc_path}", f"--encode={message_type}"],
                input=dec.stdout,
                check=True,
                capture_output=True,
            )
            if reenc.stdout != raw_bytes:
                raise AssertionError(
                    f"protoc round-trip mismatch for {label} ({message_type}): "
                    f"expected {raw_bytes.hex()} got {reenc.stdout.hex()}"
                )
    return True


def run_negative_and_size_invariant_tests() -> None:
    """Verify rejection of malformed flags/memos and round-trip of full production PQ key/ct sizes."""
    # 1. Visibility flag mutual exclusion
    for bad_flags in (
        0,  # no visibility bit
        POINTER_FLAG_PUBLIC | POINTER_FLAG_PRIVATE,
        POINTER_FLAG_PUBLIC | POINTER_FLAG_SEALED,
        POINTER_FLAG_PRIVATE | POINTER_FLAG_SEALED,
        POINTER_FLAG_PUBLIC | POINTER_FLAG_PRIVATE | POINTER_FLAG_SEALED,
        0x40,  # unknown bit
    ):
        try:
            validate_pointer_v5_flags(bad_flags)
        except ValueError:
            pass
        else:
            raise AssertionError(f"expected invalid pointer flags 0x{bad_flags:x} to be rejected")

    # 2. Uppercase hex rejection on postfiatl1v2 PaymentMemo
    try:
        validate_l1_payment_memo("70662E707472", "7635", "0a")
    except ValueError:
        pass
    else:
        raise AssertionError("expected uppercase PaymentMemo hex to be rejected")

    # 3. Oversized memo_data (> 256 bytes) rejection
    try:
        validate_l1_payment_memo(b"pf.ptr".hex(), b"v5".hex(), ("ab" * 257))
    except ValueError:
        pass
    else:
        raise AssertionError("expected >256-byte PaymentMemo memo_data to be rejected")

    # 4. 32-byte payment reference hash rejection (chain hashes are 48-byte SHA3-384)
    for bad_ref in (
        {"order_hash": sha256(b"32-byte-order-hash")},
        {"certificate_digest": sha256(b"32-byte-certificate-digest")},
        {"tx_hash": sha256(b"32-byte-tx-hash")},
    ):
        try:
            enc_fastpay_payment_ref(lane=1, chain_id="postfiat-devnet-2", **bad_ref)
        except ValueError:
            pass
        else:
            raise AssertionError(f"expected 32-byte FastPayPaymentRef {next(iter(bad_ref))} to be rejected")

    # 5. Production PQ sizes: X-Wing pk=1216B, ct=1120B; ML-DSA-65 pk=1952B, sig=3309B
    prod_xwing_pk = bytes([0x1A]) * 1216
    prod_xwing_ct = bytes([0x2B]) * 1120
    prod_mldsa_pk = bytes([0x3C]) * 1952
    prod_mldsa_sig = bytes([0x4D]) * 3309
    prod_key_id = sha256(prod_xwing_pk)
    prod_wrap = enc_recipient_wrap(
        recipient_key_id=prod_key_id,
        recipient_account="",  # SEALED mode omits recipient_account
        xwing_ciphertext=prod_xwing_ct,
        wrap_nonce=bytes([0x5E]) * 24,
        wrapped_cek=bytes([0x6F]) * 48,
    )
    prod_hdr = enc_message_header(
        protocol_version=1,
        chain_id="postfiat-mainnet",
        schema_name="pf.message.text.v1",
        schema_id=100,
        kind=4,
        visibility=3,  # SEALED
        body_encoding=1,
        cipher_suite=2,
        sender_account="pf1sender000000000000000000000000000000000",
        created_at_unix_ms=1764201999000,
    )
    prod_enc_hdr = enc_xwing_encryption_header(
        cipher_suite=2,
        aead_nonce=bytes([0x7A]) * 24,
        aad_context=compute_aad_context("postfiat-mainnet", prod_hdr, 0),
        chunk_index=0,
        kdf_info=DOMAIN_XWING_WRAP_INFO.decode("ascii"),
        recipients=[prod_wrap],
    )
    prod_env = enc_envelope(
        version=1,
        header_bytes=prod_hdr,
        encryption_header_bytes=prod_enc_hdr,
        body_bytes=bytes([0x8B]) * 512,
        signature_algorithm=1,
        sender_pubkey=prod_mldsa_pk,
        signature=prod_mldsa_sig,
    )
    assert len(prod_env) < MAX_SINGLE_ENVELOPE_BYTES
    assert_canonical_field_order(prod_env, "production-pq-envelope")


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Contract C1 protobuf schemas and deterministic test vectors.")
    parser.add_argument(
        "--emit-vectors",
        action="store_true",
        help="Print the canonical JSON test vectors block and exit.",
    )
    parser.add_argument(
        "--require-protoc",
        action="store_true",
        help="Fail if `protoc` is not available on PATH.",
    )
    args = parser.parse_args()

    expected = build_canonical_vectors()
    if args.emit_vectors:
        print(json.dumps(expected, indent=2, sort_keys=False))
        return 0

    verify_proto_sources_pure_python()
    proto_messages_checked, proto_messages_in_vectors = verify_proto_field_tables(expected)

    # Check canonical field ordering on every hex protobuf payload
    for vec_name, fields in expected.items():
        for k, v in fields.items():
            if k.endswith("_hex") and any(
                tok in k
                for tok in (
                    "memo_data",
                    "header",
                    "body",
                    "envelope",
                    "manifest",
                    "required",
                    "signature",
                    "response",
                    "claim_hex",
                    "metadata_hex",
                    "packet_proto",
                    "pointer_v5",
                    "transcript_hex",
                    "minutes_hex",
                    "ruling_hex",
                )
            ):
                if "sha256" in k or "sha3_384" in k or "hash" in k:
                    continue
                assert_canonical_field_order(bytes.fromhex(v), f"{vec_name}.{k}")

    # Verify exact match with docs/specs/message-protocol-v1.md
    if not SPEC_DOC.is_file():
        raise AssertionError(f"missing spec document: {SPEC_DOC}")
    published = extract_doc_vectors(SPEC_DOC)
    if published != expected:
        for key in sorted(set(expected) | set(published)):
            if expected.get(key) != published.get(key):
                print(f"VECTOR MISMATCH in {key}:", file=sys.stderr)
                print(f"  expected : {json.dumps(expected.get(key), sort_keys=True)}", file=sys.stderr)
                print(f"  published: {json.dumps(published.get(key), sort_keys=True)}", file=sys.stderr)
        return 1

    run_negative_and_size_invariant_tests()
    protoc_used = verify_with_protoc(expected, require_protoc=args.require_protoc)

    print(
        f"message-protocol-v1 vectors PASSED "
        f"(vectors={len(expected)}, proto_messages_checked={proto_messages_checked}, "
        f"proto_messages_in_vectors={proto_messages_in_vectors}, "
        f"protoc_roundtrip={'yes' if protoc_used else 'skipped-no-protoc'})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
