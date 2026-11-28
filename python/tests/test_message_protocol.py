from __future__ import annotations

import hashlib
import hmac
import importlib.util
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from postfiat_rpc import message_protocol as mp


REPO_ROOT = Path(__file__).resolve().parents[2]

# The frozen Contract C1 vectors: the JSON block of docs/specs/message-protocol-v1.md is the one source.
_SPEC_DOC = REPO_ROOT / "docs" / "specs" / "message-protocol-v1.md"
_VECTOR_BLOCK_RE = re.compile(
    r"<!--\s*BEGIN_CANONICAL_TEST_VECTORS_JSON\s*-->\s*```json\s*(\{.*?\})\s*```\s*<!--\s*END_CANONICAL_TEST_VECTORS_JSON\s*-->",
    re.DOTALL,
)
C1_VECTORS = json.loads(_VECTOR_BLOCK_RE.search(_SPEC_DOC.read_text(encoding="utf-8")).group(1))


def _canonical_vector_payloads() -> list[tuple[str, str, bytes]]:
    """Every protobuf payload of the canonical vectors, as listed by scripts/test-message-protocol-vectors.py."""
    spec = importlib.util.spec_from_file_location("c1_vectors", REPO_ROOT / "scripts" / "test-message-protocol-vectors.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.vector_proto_payloads(C1_VECTORS)


# Message types this SDK encodes; the NFT and governance types are outside it.
_SDK_TYPES = {
    "postfiat.message.v1.Pointer": mp.Pointer,
    "postfiat.message.v1.LegacyPointerV4": mp.LegacyPointerV4,
    "postfiat.message.v1.Envelope": mp.Envelope,
    "postfiat.message.v1.MessageHeader": mp.MessageHeader,
    "postfiat.message.v1.TextMessage": mp.TextMessage,
    "postfiat.message.v1.XWingEncryptionHeader": mp.XWingEncryptionHeader,
    "postfiat.message.v1.Manifest": mp.Manifest,
    "postfiat.message.v1.X402PaymentRequired": mp.X402PaymentRequired,
    "postfiat.message.v1.X402PaymentSignature": mp.X402PaymentSignature,
    "postfiat.message.v1.X402PaymentResponse": mp.X402PaymentResponse,
}


def _hmac_test_signer(key: bytes):
    def sign_fn(preimage: bytes) -> bytes:
        return hmac.new(key, preimage, hashlib.sha256).digest()

    def verify_fn(preimage: bytes, sig: bytes, pubkey: bytes) -> bool:
        expected = hmac.new(key, preimage, hashlib.sha256).digest()
        return hmac.compare_digest(expected, sig) and len(pubkey) > 0

    return sign_fn, verify_fn


def _toy_xwing_kem(seed_secret: bytes):
    """Deterministic full-size (1216B pk / 1120B ct) KEM adapter for unit tests."""
    pk = hashlib.shake_256(b"xwing-pk:" + seed_secret).digest(mp.XWING_PUBKEY_BYTES)

    def encap(peer_pk: bytes) -> tuple[bytes, bytes]:
        assert peer_pk == pk
        ct = hashlib.shake_256(b"xwing-ct:" + seed_secret + peer_pk[:32]).digest(mp.XWING_CIPHERTEXT_BYTES)
        ss = hashlib.sha256(b"xwing-ss:" + seed_secret + ct).digest()
        return ct, ss

    def decap(ct: bytes) -> bytes:
        assert len(ct) == mp.XWING_CIPHERTEXT_BYTES
        return hashlib.sha256(b"xwing-ss:" + seed_secret + ct).digest()

    return pk, encap, decap


class MessageProtocolVectorTests(unittest.TestCase):
    def test_unsigned_wire_fields_are_refused(self) -> None:
        extension = bytes.fromhex("f00101")
        for reader in (mp.Envelope, mp.MessageKeyBinding, mp.X402PaymentRequired,
                       mp.X402PaymentSignature, mp.X402PaymentResponse):
            with self.subTest(reader=reader.__name__), self.assertRaisesRegex(mp.MessageProtocolError, "unknown"):
                reader.decode(extension)
        header = mp.MessageHeader.decode(bytes.fromhex("820103") + extension)
        self.assertIsNotNone(header.x402_receipt)

    def test_pointer_reader_contract(self) -> None:
        ptr = mp.Pointer(target=mp.PointerTarget.INLINE, flags=int(mp.PointerFlag.PUBLIC), inline_payload=b"x")
        canonical = ptr.encode()
        self.assertEqual(mp.Pointer.decode(canonical, enforce_canonical_order=True), ptr)
        with self.assertRaises(mp.MessageProtocolError):
            mp.Pointer.decode(canonical + bytes([88, 1]))
        with self.assertRaises(mp.MessageProtocolError):
            mp.Pointer.decode(canonical + canonical[-3:], enforce_canonical_order=True)

    def test_bundled_proto_files_and_protoc_roundtrip(self) -> None:
        proto_files = (
            "postfiat/message/v1/message.proto",
            "postfiat/nft/v1/metadata.proto",
            "postfiat/governance/v1/packet.proto",
        )
        for rel in proto_files:
            bundled = mp.PROTO_ROOT / rel
            self.assertTrue(bundled.is_file(), f"missing bundled proto {bundled}")
            root_proto = REPO_ROOT / "proto" / rel
            if root_proto.is_file():
                self.assertEqual(
                    bundled.read_bytes(),
                    root_proto.read_bytes(),
                    f"proto drift in {rel}; run: cp proto/{rel} python/postfiat_rpc/proto/{rel}",
                )

        protoc = shutil.which("protoc")
        if not protoc:
            return

        with tempfile.TemporaryDirectory(prefix="postfiat-py-c1-") as tmpdir:
            desc_path = Path(tmpdir) / "descriptors.pb"
            subprocess.run(
                [
                    protoc,
                    f"-I{mp.PROTO_ROOT}",
                    "--include_imports",
                    f"--descriptor_set_out={desc_path}",
                    *[str(mp.PROTO_ROOT / rel) for rel in proto_files],
                ],
                check=True,
                capture_output=True,
            )
            checks = [
                ("postfiat.message.v1.Pointer", bytes.fromhex(C1_VECTORS["V1_POINTER_V5_PUBLIC_TEXT"]["memo_data_hex"])),
                ("postfiat.message.v1.Pointer", bytes.fromhex(C1_VECTORS["V2A_POINTER_V5_WORST_CASE_BUDGET"]["memo_data_hex"])),
                ("postfiat.message.v1.Pointer", bytes.fromhex(C1_VECTORS["V2B_POINTER_V5_INLINE_X402"]["memo_data_hex"])),
                ("postfiat.message.v1.LegacyPointerV4", bytes.fromhex(C1_VECTORS["V3_LEGACY_POINTER_V4_DECODE"]["memo_data_hex_upper"])),
                ("postfiat.message.v1.Envelope", bytes.fromhex(C1_VECTORS["V4_ENVELOPE_PUBLIC_TEXT"]["envelope_hex"])),
                ("postfiat.message.v1.Envelope", bytes.fromhex(C1_VECTORS["V5_ENVELOPE_PRIVATE_XWING_REWARD"]["envelope_hex"])),
                ("postfiat.message.v1.Manifest", bytes.fromhex(C1_VECTORS["V6_MANIFEST_MULTICHUNK"]["manifest_hex"])),
                ("postfiat.message.v1.X402PaymentRequired", bytes.fromhex(C1_VECTORS["V7_X402_HEADERS"]["required_hex"])),
                ("postfiat.message.v1.X402PaymentSignature", bytes.fromhex(C1_VECTORS["V7_X402_HEADERS"]["signature_hex"])),
                ("postfiat.message.v1.X402PaymentResponse", bytes.fromhex(C1_VECTORS["V7_X402_HEADERS"]["response_hex"])),
            ]
            for msg_type, raw in checks:
                dec = subprocess.run(
                    [protoc, f"--descriptor_set_in={desc_path}", f"--decode={msg_type}"],
                    input=raw,
                    check=True,
                    capture_output=True,
                )
                reenc = subprocess.run(
                    [protoc, f"--descriptor_set_in={desc_path}", f"--encode={msg_type}"],
                    input=dec.stdout,
                    check=True,
                    capture_output=True,
                )
                self.assertEqual(reenc.stdout, raw, f"protoc roundtrip mismatch for {msg_type}")

    def test_every_canonical_payload_encodes_byte_for_byte(self) -> None:
        checked = 0
        for label, msg_type, raw in _canonical_vector_payloads():
            cls = _SDK_TYPES.get(msg_type)
            if cls is None:
                self.assertTrue(msg_type.startswith(("postfiat.nft.", "postfiat.governance.")), msg_type)
                continue
            # LegacyPointerV4 is a decode-only compatibility type without a canonical-order option.
            strict = {} if cls is mp.LegacyPointerV4 else {"enforce_canonical_order": True}
            decoded = cls.decode(raw, **strict)
            self.assertEqual(decoded.encode(), raw, f"{label} {msg_type} does not re-encode byte for byte")
            checked += 1
        self.assertEqual(checked, 17)

    def test_payment_ref_hashes_are_48_bytes(self) -> None:
        h48, h32 = b"\x11" * mp.CHAIN_HASH_BYTES, b"\x11" * 32
        self.assertEqual(mp.CHAIN_HASH_BYTES, 48)
        ref = mp.FastPayPaymentRef(lane=mp.PaymentLane.FASTPAY, order_hash=h48, certificate_digest=h48)
        self.assertEqual(mp.FastPayPaymentRef.decode(ref.encode()), ref)
        self.assertEqual(ref.dedup_key(), h48)
        consensus = mp.FastPayPaymentRef(lane=mp.PaymentLane.CONSENSUS_PFT, tx_hash=h48)
        self.assertEqual(consensus.dedup_key(), h48)
        sig = mp.X402PaymentSignature(order_hash=h48, tx_hash=h48)
        self.assertEqual(mp.X402PaymentSignature.decode(sig.encode()), sig)
        for field, number in (("order_hash", 3), ("certificate_digest", 4), ("tx_hash", 5)):
            with self.assertRaisesRegex(mp.MessageProtocolError, f"FastPayPaymentRef.{field} must be 48 bytes"):
                mp.FastPayPaymentRef(lane=mp.PaymentLane.FASTPAY, **{field: h32})
            # A 32-byte value on the wire is refused on decode too.
            with self.assertRaises(mp.MessageProtocolError):
                mp.FastPayPaymentRef.decode(mp.pb_bytes(number, h32))
        for field, number in (("order_hash", 7), ("tx_hash", 8)):
            with self.assertRaisesRegex(mp.MessageProtocolError, f"X402PaymentSignature.{field} must be 48 bytes"):
                mp.X402PaymentSignature(**{field: h32})
            with self.assertRaises(mp.MessageProtocolError):
                mp.X402PaymentSignature.decode(mp.pb_bytes(number, h32))

    def test_v1_v2_v3_pointer_vectors(self) -> None:
        v1 = C1_VECTORS["V1_POINTER_V5_PUBLIC_TEXT"]
        ptr1 = mp.Pointer(
            cid_bin=bytes.fromhex(v1["cid_bin_hex"]),
            target=mp.PointerTarget.CONTENT_BLOB,
            kind=mp.ContentKind.CHAT,
            schema=mp.SCHEMA_ID_TEXT_V1,
            thread_id="thread-c1-001",
            flags=int(mp.PointerFlag.PUBLIC),
        )
        memo1 = mp.encode_pointer_v5_memo(ptr1)
        self.assertEqual(memo1["memo_type"], v1["memo_type_hex"])
        self.assertEqual(memo1["memo_format"], v1["memo_format_hex"])
        self.assertEqual(memo1["memo_data"], v1["memo_data_hex"])
        self.assertEqual(len(bytes.fromhex(memo1["memo_data"])), v1["byte_len"])
        self.assertEqual(mp.decode_pointer_v5_memo(memo1, enforce_canonical_order=True), ptr1)
        self.assertEqual(mp.cid_bin_to_multibase(ptr1.cid_bin), v1["cid_multibase"])
        self.assertEqual(mp.multibase_to_cid_bin(v1["cid_multibase"]), ptr1.cid_bin)

        v2a = C1_VECTORS["V2A_POINTER_V5_WORST_CASE_BUDGET"]
        ptr2a = mp.Pointer.decode(bytes.fromhex(v2a["memo_data_hex"]), enforce_canonical_order=True)
        self.assertEqual(ptr2a.encode().hex(), v2a["memo_data_hex"])
        self.assertEqual(len(ptr2a.encode()), v2a["byte_len"])
        self.assertEqual(ptr2a.schema_name, "org.example.custom.packet.v1")

        v2b = C1_VECTORS["V2B_POINTER_V5_INLINE_X402"]
        ptr2b = mp.Pointer(
            target=mp.PointerTarget.INLINE,
            kind=mp.ContentKind.X402,
            schema=mp.SCHEMA_ID_X402_REQUEST_V1,
            flags=int(mp.PointerFlag.PUBLIC | mp.PointerFlag.EPHEMERAL),
            inline_payload=bytes.fromhex(v2b["request_hash_hex"]),
        )
        self.assertEqual(ptr2b.encode().hex(), v2b["memo_data_hex"])
        self.assertEqual(mp.Pointer.decode(bytes.fromhex(v2b["memo_data_hex"])), ptr2b)

        v3 = C1_VECTORS["V3_LEGACY_POINTER_V4_DECODE"]
        legacy = mp.decode_legacy_pointer_v4_memo(
            {
                "memo_type": v3["memo_type_hex"],
                "memo_format": v3["memo_format_hex"],
                "memo_data": v3["memo_data_hex_upper"],
            }
        )
        self.assertEqual(legacy.kind, mp.ContentKind.REWARD)
        self.assertEqual(legacy.task_id, "task-legacy-0042")
        self.assertEqual(legacy.encode().hex().upper(), v3["memo_data_hex_upper"])

    def test_v4_v5_envelope_and_crypto_vectors(self) -> None:
        v4 = C1_VECTORS["V4_ENVELOPE_PUBLIC_TEXT"]
        env4 = mp.Envelope.decode(bytes.fromhex(v4["envelope_hex"]), enforce_canonical_order=True)
        self.assertEqual(env4.encode().hex(), v4["envelope_hex"])
        self.assertEqual(env4.header_bytes.hex(), v4["header_hex"])
        self.assertEqual(env4.body_bytes.hex(), v4["body_hex"])
        self.assertEqual(mp.sha256(env4.signing_preimage()).hex(), v4["signing_preimage_sha256_hex"])
        cid_bin4, cid_b32_4 = mp.cidv1_raw_from_bytes(env4.encode())
        self.assertEqual(cid_bin4.hex(), v4["cid_bin_hex"])
        self.assertEqual(cid_b32_4, v4["cid_multibase"])
        hdr4 = mp.MessageHeader.decode(env4.header_bytes, enforce_canonical_order=True)
        self.assertEqual(hdr4.encode().hex(), v4["header_hex"])
        body4 = mp.TextMessage.decode(env4.body_bytes, enforce_canonical_order=True)
        self.assertEqual(body4.text, "Hello Post Fiat L1v2!")
        self.assertEqual(body4.encode().hex(), v4["body_hex"])

        v5 = C1_VECTORS["V5_ENVELOPE_PRIVATE_XWING_REWARD"]
        env5 = mp.Envelope.decode(bytes.fromhex(v5["envelope_hex"]), enforce_canonical_order=True)
        self.assertEqual(env5.encode().hex(), v5["envelope_hex"])
        self.assertEqual(mp.sha256(env5.signing_preimage()).hex(), v5["signing_preimage_sha256_hex"])
        cid_bin5, cid_b32_5 = mp.cidv1_raw_from_bytes(env5.encode())
        self.assertEqual(cid_bin5.hex(), v5["cid_bin_hex"])
        self.assertEqual(cid_b32_5, v5["cid_multibase"])
        hdr5 = mp.MessageHeader.decode(env5.header_bytes, enforce_canonical_order=True)
        self.assertEqual(hdr5.encode().hex(), v5["header_hex"])
        assert hdr5.payment_ref is not None
        self.assertEqual(hdr5.payment_ref.dedup_key().hex(), v5["order_hash_hex"])
        enc5 = mp.XWingEncryptionHeader.decode(env5.encryption_header_bytes, enforce_canonical_order=True)
        self.assertEqual(enc5.encode().hex(), v5["encryption_header_hex"])
        aad5 = mp.compute_aad_context("postfiat-devnet-2", env5.header_bytes, 0)
        self.assertEqual(aad5.hex(), v5["aad_context_hex"])
        kem_seed = mp.derive_message_kem_seed(bytes.fromhex("10" * 32), "postfiat-devnet-2")
        self.assertEqual(kem_seed.hex(), v5["derived_kem_seed_hex"])
        key_id = mp.compute_recipient_key_id(bytes.fromhex("22" * 32))
        self.assertEqual(key_id.hex(), v5["recipient_key_id_hex"])
        kek = mp.derive_xwing_wrap_kek(bytes.fromhex("33" * 32), aad5, key_id)
        self.assertEqual(kek.hex(), v5["derived_wrap_kek_hex"])
        kb_preimage = mp.compute_key_binding_preimage(
            chain_id="postfiat-devnet-2",
            account="pf1recip0000000000000000000000000000000000",
            xwing_public_key=bytes.fromhex("22" * 32),
            key_id=key_id,
            created_at_unix_ms=1764201600000,
            key_epoch=1,
        )
        self.assertEqual(mp.sha256(kb_preimage).hex(), v5["key_binding_preimage_sha256_hex"])

    def test_v6_manifest_and_v7_x402_vectors(self) -> None:
        v6 = C1_VECTORS["V6_MANIFEST_MULTICHUNK"]
        manifest = mp.Manifest.decode(bytes.fromhex(v6["manifest_hex"]), enforce_canonical_order=True)
        self.assertEqual(manifest.encode().hex(), v6["manifest_hex"])
        self.assertEqual(len(manifest.chunks), 2)

        v7 = C1_VECTORS["V7_X402_HEADERS"]
        req = mp.X402PaymentRequired.decode(bytes.fromhex(v7["required_hex"]), enforce_canonical_order=True)
        self.assertEqual(req.encode().hex(), v7["required_hex"])
        sig = mp.X402PaymentSignature.decode(bytes.fromhex(v7["signature_hex"]), enforce_canonical_order=True)
        self.assertEqual(sig.encode().hex(), v7["signature_hex"])
        resp = mp.X402PaymentResponse.decode(bytes.fromhex(v7["response_hex"]), enforce_canonical_order=True)
        self.assertEqual(resp.encode().hex(), v7["response_hex"])
        assert resp.payment_ref is not None
        self.assertEqual(resp.payment_ref.dedup_key().hex(), v7["dedup_order_hash_hex"])
        b64 = mp.encode_x402_header(req.encode())
        self.assertEqual(mp.decode_x402_header(b64), req.encode())

    def test_seal_open_private_and_sealed_and_manifest_roundtrip(self) -> None:
        sign_fn, verify_fn = _hmac_test_signer(b"deterministic-test-signing-material")
        sender_pk = b"\xaa" * mp.ML_DSA_65_PUBKEY_BYTES
        sender_addr = mp.derive_account_address(sender_pk)
        recip_pk, encap_fn, decap_fn = _toy_xwing_kem(b"recipient-seed-01")
        recip_key_id = mp.compute_recipient_key_id(recip_pk)

        reward_body = mp.RewardNotice(
            reward_id="rew-001",
            task_id="task-100",
            recipient_account="pf1recip0000000000000000000000000000000000",
            amount_atoms=250_000_000,
            asset_id="PFT",
            summary="Milestone completed",
        ).encode()

        # PRIVATE envelope round-trip
        priv_header = mp.MessageHeader(
            protocol_version=1,
            chain_id="postfiat-devnet-2",
            schema_name="pf.reward.v1",
            schema_id=mp.SCHEMA_ID_REWARD_V1,
            kind=mp.ContentKind.REWARD,
            visibility=mp.Visibility.PRIVATE,
            body_encoding=mp.BodyEncoding.PROTOBUF,
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            sender_account=sender_addr,
            created_at_unix_ms=1764201660000,
            task_id="task-100",
        )
        sealed_priv = mp.seal_envelope(
            header=priv_header,
            body_plaintext=reward_body,
            sender_pubkey=sender_pk,
            sign_fn=sign_fn,
            recipients=[mp.RecipientSpec(xwing_public_key=recip_pk, account="pf1recip0000000000000000000000000000000000")],
            xwing_encap_fn=encap_fn,
        )
        self.assertEqual(mp.decode_pointer_v5_memo(sealed_priv.memo).cid_bin, sealed_priv.cid_bin)
        opened = mp.open_envelope(
            sealed_priv.envelope_bytes,
            verify_signature_fn=verify_fn,
            expected_cid_bin=sealed_priv.cid_bin,
            expected_multibase_cid=sealed_priv.multibase_cid,
            expected_chain_id="postfiat-devnet-2",
            expected_tx_sender=sender_addr,
            recipient_key_id=recip_key_id,
            xwing_decap_fn=decap_fn,
            enforce_canonical_order=True,
        )
        self.assertEqual(opened.plaintext_body, reward_body)
        assert opened.encryption_header is not None
        self.assertEqual(opened.encryption_header.recipients[0].recipient_account, "pf1recip0000000000000000000000000000000000")

        # Anchor-binding negative checks enforced inside open_envelope():
        with self.assertRaisesRegex(mp.MessageProtocolError, "chain_id mismatch"):
            mp.open_envelope(
                sealed_priv.envelope_bytes,
                verify_signature_fn=verify_fn,
                expected_chain_id="postfiat-mainnet-1",
                expected_tx_sender=sender_addr,
                recipient_key_id=recip_key_id,
                xwing_decap_fn=decap_fn,
            )
        with self.assertRaisesRegex(mp.MessageProtocolError, "Foreign re-anchor rejected"):
            mp.open_envelope(
                sealed_priv.envelope_bytes,
                verify_signature_fn=verify_fn,
                expected_chain_id="postfiat-devnet-2",
                expected_tx_sender="pf0000000000000000000000000000000000000000",
                recipient_key_id=recip_key_id,
                xwing_decap_fn=decap_fn,
            )
        # Spoofed sender_account (signed by outsider_pk claiming sender_addr) must fail by default:
        outsider_pk = b"\xbb" * mp.ML_DSA_65_PUBKEY_BYTES
        spoof_sealed = mp.seal_envelope(
            header=priv_header,
            body_plaintext=reward_body,
            sender_pubkey=outsider_pk,
            sign_fn=sign_fn,
            recipients=[mp.RecipientSpec(xwing_public_key=recip_pk, account="pf1recip0000000000000000000000000000000000")],
            xwing_encap_fn=encap_fn,
        )
        with self.assertRaisesRegex(mp.MessageProtocolError, "sender_pubkey address"):
            mp.open_envelope(
                spoof_sealed.envelope_bytes,
                verify_signature_fn=verify_fn,
                recipient_key_id=recip_key_id,
                xwing_decap_fn=decap_fn,
            )

        # SEALED envelope must strip recipient_account
        sealed_hdr = mp.MessageHeader(
            protocol_version=1,
            chain_id="postfiat-devnet-2",
            schema_name="pf.message.text.v1",
            schema_id=mp.SCHEMA_ID_TEXT_V1,
            kind=mp.ContentKind.CHAT,
            visibility=mp.Visibility.SEALED,
            body_encoding=mp.BodyEncoding.RAW_UTF8,
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            sender_account=sender_addr,
            created_at_unix_ms=1764201660000,
        )
        sealed_res = mp.seal_envelope(
            header=sealed_hdr,
            body_plaintext=b"sealed payload",
            sender_pubkey=sender_pk,
            sign_fn=sign_fn,
            recipients=[mp.RecipientSpec(xwing_public_key=recip_pk, account="should-be-omitted")],
            xwing_encap_fn=encap_fn,
        )
        opened_sealed = mp.open_envelope(
            sealed_res.envelope_bytes,
            verify_signature_fn=verify_fn,
            expected_cid_bin=sealed_res.cid_bin,
            recipient_key_id=recip_key_id,
            xwing_decap_fn=decap_fn,
        )
        self.assertEqual(opened_sealed.plaintext_body, b"sealed payload")
        assert opened_sealed.encryption_header is not None
        self.assertEqual(opened_sealed.encryption_header.recipients[0].recipient_account, "")

        # Multi-chunk encrypted Manifest round-trip + chunk swap rejection
        cek = os.urandom(32)
        big_payload = b"postfiat-chunk-data-" * 500
        manifest, blocks = mp.split_manifest_chunks(
            big_payload,
            chain_id="postfiat-devnet-2",
            header_bytes=priv_header.encode(),
            media_type="application/octet-stream",
            inner_schema_name="pf.document.bundle.v1",
            cek=cek,
            chunk_size_bytes=4096,
        )
        self.assertGreaterEqual(len(blocks), 3)
        rebuilt = mp.reassemble_manifest_chunks(
            manifest, blocks, chain_id="postfiat-devnet-2", header_bytes=priv_header.encode(), cek=cek
        )
        self.assertEqual(rebuilt, big_payload)

        # Swapping two chunks must fail CID/AAD verification
        swapped = [blocks[1], blocks[0], *blocks[2:]]
        with self.assertRaises(mp.MessageProtocolError):
            mp.reassemble_manifest_chunks(
                manifest, swapped, chain_id="postfiat-devnet-2", header_bytes=priv_header.encode(), cek=cek
            )

    def test_negative_validation_cases(self) -> None:
        # Conflicting visibility flags
        with self.assertRaises(mp.MessageProtocolError):
            mp.validate_pointer_v5_flags(int(mp.PointerFlag.PUBLIC | mp.PointerFlag.PRIVATE))
        with self.assertRaises(mp.MessageProtocolError):
            mp.validate_pointer_v5_flags(0)
        # Uppercase hex rejected in v5 memo
        with self.assertRaises(mp.MessageProtocolError):
            mp.validate_l1_payment_memo("70662E707472", "7635", "00")
        # Tampered raw envelope fails CID check before decode
        v4_bytes = bytes.fromhex(C1_VECTORS["V4_ENVELOPE_PUBLIC_TEXT"]["envelope_hex"])
        tampered = bytearray(v4_bytes)
        tampered[-1] ^= 0xFF
        with self.assertRaises(mp.MessageProtocolError):
            mp.open_envelope(
                bytes(tampered),
                verify_signature_fn=lambda *_: True,
                expected_cid_bin=bytes.fromhex(C1_VECTORS["V4_ENVELOPE_PUBLIC_TEXT"]["cid_bin_hex"]),
            )

    def test_cross_language_js_python_wire_parity(self) -> None:
        js_sdk = REPO_ROOT / "wallet-proxy" / "message-protocol.js"
        if not js_sdk.is_file() or not shutil.which("node"):
            return
        script = (
            f"const mp = require({json.dumps(str(js_sdk))});"
            "const v1 = mp.encodePointerV5Memo({"
            "  cid_bin: Buffer.from('015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d','hex'),"
            "  target: mp.PointerTarget.CONTENT_BLOB,"
            "  kind: mp.ContentKind.CHAT,"
            "  schema: 100,"
            "  thread_id: 'thread-c1-001',"
            "  flags: mp.PointerFlag.PUBLIC,"
            "});"
            "const aad = mp.computeAadContext('postfiat-devnet-2', Buffer.from("
            f"  {json.dumps(C1_VECTORS['V5_ENVELOPE_PRIVATE_XWING_REWARD']['header_hex'])}, 'hex'), 0);"
            "console.log(JSON.stringify({ memo_data: v1.memo_data, aad: aad.toString('hex') }));"
        )
        out = subprocess.check_output(["node", "-e", script], text=True)
        parsed = json.loads(out)
        self.assertEqual(parsed["memo_data"], C1_VECTORS["V1_POINTER_V5_PUBLIC_TEXT"]["memo_data_hex"])
        self.assertEqual(parsed["aad"], C1_VECTORS["V5_ENVELOPE_PRIVATE_XWING_REWARD"]["aad_context_hex"])

    def test_cek_commitment_blinded_sealed_and_epoch_rotation(self) -> None:
        sender_pk = b"\x31" * mp.ML_DSA_65_PUBKEY_BYTES
        sender_addr = mp.derive_account_address(sender_pk)
        bob_pk = b"\x41" * mp.XWING_PUBKEY_BYTES
        bob_static_id = mp.compute_recipient_key_id(bob_pk)
        eve_pk = b"\x51" * mp.XWING_PUBKEY_BYTES

        def fake_encap(pk: bytes) -> tuple[bytes, bytes]:
            ct = hashlib.sha256(b"ct:" + pk).digest() * 35  # 1120 B
            ss = hashlib.sha256(b"ss:" + pk).digest()
            return ct, ss

        def decap_for(owner_pk: bytes):
            expected_ct = hashlib.sha256(b"ct:" + owner_pk).digest() * 35
            expected_ss = hashlib.sha256(b"ss:" + owner_pk).digest()

            def _decap(ct: bytes) -> bytes:
                return expected_ss if ct == expected_ct else hashlib.sha256(b"wrong:" + ct).digest()

            return _decap

        sig_fn = lambda preimage: hashlib.sha256(preimage).digest() + b"\x00" * (mp.ML_DSA_65_SIGNATURE_BYTES - 32)
        ver_fn = lambda preimage, sig, pk: sig == sig_fn(preimage) and pk == sender_pk

        # 1) SEALED envelope uses blinded per-message recipient tag + 32B cek_commitment
        sealed_hdr = mp.MessageHeader(
            protocol_version=1,
            chain_id="postfiat-devnet-2",
            sender_account=sender_addr,
            created_at_unix_ms=1764201600000,
            kind=mp.ContentKind.CHAT,
            visibility=mp.Visibility.SEALED,
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            body_encoding=mp.BodyEncoding.PROTOBUF,
            schema_id=mp.SCHEMA_ID_TEXT_V1,
            schema_name="pf.msg.text.v1",
        )
        sealed = mp.seal_envelope(
            header=sealed_hdr,
            body_plaintext=b"blinded-sealed-hello",
            sender_pubkey=sender_pk,
            sign_fn=sig_fn,
            recipients=[mp.RecipientSpec(xwing_public_key=bob_pk, account="pf_ignored")],
            xwing_encap_fn=fake_encap,
        )
        enc_hdr = mp.XWingEncryptionHeader.decode(sealed.envelope.encryption_header_bytes)
        self.assertEqual(len(enc_hdr.cek_commitment), 32)
        # Static bob_static_id MUST NOT appear on wire in blinded SEALED mode
        for w in enc_hdr.recipients:
            self.assertNotEqual(w.recipient_key_id, bob_static_id)
            self.assertEqual(w.recipient_account, "")

        # Bob opens blinded SEALED envelope without matching static key_id
        opened = mp.open_envelope(
            sealed.envelope_bytes,
            verify_signature_fn=ver_fn,
            expected_cid_bin=sealed.cid_bin,
            expected_chain_id="postfiat-devnet-2",
            expected_tx_sender=sender_addr,
            recipient_key_id=bob_static_id,
            xwing_decap_fn=decap_for(bob_pk),
            require_cek_commitment=True,
        )
        self.assertEqual(opened.plaintext_body, b"blinded-sealed-hello")

        # Eve cannot open blinded SEALED envelope
        with self.assertRaisesRegex(mp.MessageProtocolError, "no matching RecipientWrap"):
            mp.open_envelope(
                sealed.envelope_bytes,
                verify_signature_fn=ver_fn,
                expected_cid_bin=sealed.cid_bin,
                recipient_key_id=mp.compute_recipient_key_id(eve_pk),
                xwing_decap_fn=decap_for(eve_pk),
            )

        # 2) Key-commitment negative control: malicious sender wraps wrong CEK to recipient
        real_cek = b"\xaa" * 32
        evil_cek = b"\xbb" * 32
        aad = mp.compute_aad_context("postfiat-devnet-2", sealed_hdr.encode(), 0)
        body_nonce = b"\x11" * 24
        body_ct = mp.xchacha20_poly1305_encrypt(evil_cek, body_nonce, b"evil-branch", aad)
        bob_ct, bob_ss = fake_encap(bob_pk)
        kek = mp.derive_xwing_wrap_kek(bob_ss, aad, bob_static_id)
        w_nonce = b"\x22" * 24
        evil_wrap = mp.RecipientWrap(
            recipient_key_id=bob_static_id,
            recipient_account="",
            xwing_ciphertext=bob_ct,
            wrap_nonce=w_nonce,
            wrapped_cek=mp.xchacha20_poly1305_encrypt(kek, w_nonce, evil_cek, aad),
        )
        # Signed encryption header commits to real_cek, while bob gets evil_cek
        forged_enc_hdr = mp.XWingEncryptionHeader(
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            aead_nonce=body_nonce,
            aad_context=aad,
            chunk_index=0,
            kdf_info=mp.DOMAIN_XWING_WRAP_INFO.decode("ascii"),
            recipients=(evil_wrap,),
            cek_commitment=mp.compute_cek_commitment(real_cek, aad),
        ).encode()
        hdr_bytes = sealed_hdr.encode()
        preimage = mp.envelope_signing_preimage(1, hdr_bytes, forged_enc_hdr, body_ct)
        forged_env = mp.Envelope(
            version=1,
            header_bytes=hdr_bytes,
            encryption_header_bytes=forged_enc_hdr,
            body_bytes=body_ct,
            signature_algorithm=mp.SignatureAlgorithm.ML_DSA_65,
            sender_pubkey=sender_pk,
            signature=sig_fn(preimage),
        ).encode()
        with self.assertRaisesRegex(mp.MessageProtocolError, "cek_commitment mismatch"):
            mp.open_envelope(
                forged_env,
                verify_signature_fn=ver_fn,
                recipient_key_id=bob_static_id,
                xwing_decap_fn=decap_for(bob_pk),
                require_cek_commitment=True,
            )

        # 3) Forward-secrecy epoch rotation & monotonic anti-downgrade check
        b0 = mp.create_message_key_binding(
            chain_id="postfiat-devnet-2",
            account=sender_addr,
            xwing_public_key=bob_pk,
            created_at_unix_ms=1764201600000,
            key_epoch=0,
            account_pubkey=sender_pk,
            sign_fn=sig_fn,
        )
        b1 = mp.rotate_message_key_binding(
            chain_id="postfiat-devnet-2",
            account=sender_addr,
            new_xwing_public_key=eve_pk,
            previous_key_epoch=0,
            created_at_unix_ms=1764201700000,
            account_pubkey=sender_pk,
            sign_fn=sig_fn,
        )
        self.assertEqual(b1.key_epoch, 1)
        verified_b1 = mp.verify_message_key_binding(
            b1.encode(),
            verify_signature_fn=ver_fn,
            expected_chain_id="postfiat-devnet-2",
            expected_account=sender_addr,
            min_known_epoch=1,
        )
        self.assertEqual(verified_b1.key_epoch, 1)
        with self.assertRaisesRegex(mp.MessageProtocolError, "key_epoch rollback rejected"):
            mp.verify_message_key_binding(
                b0.encode(),
                verify_signature_fn=ver_fn,
                expected_chain_id="postfiat-devnet-2",
                expected_account=sender_addr,
                min_known_epoch=1,
            )

    def test_recipient_caps_and_chain_id_nul_guard(self) -> None:
        # 1) NUL byte in chain_id rejected
        with self.assertRaisesRegex(mp.MessageProtocolError, "NUL"):
            mp.compute_aad_context("postfiat\x00devnet", b"hdr")

        # 2) > MAX_ENVELOPE_RECIPIENTS rejected on decode
        dummy_wrap = mp.RecipientWrap(
            recipient_key_id=b"\x11" * 32,
            recipient_account="",
            xwing_ciphertext=b"\x22" * mp.XWING_CIPHERTEXT_BYTES,
            wrap_nonce=b"\x33" * mp.AEAD_NONCE_BYTES,
            wrapped_cek=b"\x44" * mp.WRAPPED_CEK_BYTES,
        )
        oversized = mp.XWingEncryptionHeader(
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            aead_nonce=b"\x01" * mp.AEAD_NONCE_BYTES,
            aad_context=b"\x02" * 32,
            chunk_index=0,
            kdf_info=mp.DOMAIN_XWING_WRAP_INFO.decode("ascii"),
            recipients=tuple(dummy_wrap for _ in range(mp.MAX_ENVELOPE_RECIPIENTS + 1)),
            cek_commitment=b"\x03" * 32,
        ).encode()
        with self.assertRaisesRegex(mp.MessageProtocolError, "exceeds cap"):
            mp.XWingEncryptionHeader.decode(oversized)

        # 3) SEALED trial-decapsulation enforces max_sealed_wraps before calling decap
        sender_pk = b"\xaa" * mp.ML_DSA_65_PUBKEY_BYTES
        sender_addr = mp.derive_account_address(sender_pk)
        hdr = mp.MessageHeader(
            protocol_version=1,
            chain_id="postfiat-devnet-2",
            schema_name="pf.msg.text.v1",
            schema_id=mp.SCHEMA_ID_TEXT_V1,
            kind=mp.ContentKind.CHAT,
            visibility=mp.Visibility.SEALED,
            body_encoding=mp.BodyEncoding.RAW_UTF8,
            cipher_suite=mp.CipherSuite.XWING_XCHACHA20_POLY1305,
            sender_account=sender_addr,
            created_at_unix_ms=1764201600000,
        )
        sealed = mp.seal_envelope(
            header=hdr,
            body_plaintext=b"cap-check",
            sender_pubkey=sender_pk,
            sign_fn=lambda _: b"\x55" * mp.ML_DSA_65_SIGNATURE_BYTES,
            recipients=[
                mp.RecipientSpec(xwing_public_key=bytes([i + 1]) * mp.XWING_PUBKEY_BYTES)
                for i in range(3)
            ],
            xwing_encap_fn=lambda pk: (b"\x77" * mp.XWING_CIPHERTEXT_BYTES, pk[:32]),
        )
        decap_calls = 0

        def counting_decap(_ct: bytes) -> bytes:
            nonlocal decap_calls
            decap_calls += 1
            return b"\x00" * 32

        with self.assertRaisesRegex(mp.MessageProtocolError, "exceeds scanner cap 2"):
            mp.open_envelope(
                sealed.envelope_bytes,
                verify_signature_fn=lambda *_: True,
                expected_cid_bin=sealed.cid_bin,
                xwing_decap_fn=counting_decap,
                max_sealed_wraps=2,
            )
        self.assertEqual(decap_calls, 0)


if __name__ == "__main__":
    unittest.main()
