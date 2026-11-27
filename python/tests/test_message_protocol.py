from __future__ import annotations

import hashlib
import hmac
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from postfiat_rpc import message_protocol as mp


REPO_ROOT = Path(__file__).resolve().parents[2]

# Exact frozen Contract C1 published hex test vectors (from docs/specs/message-protocol-v1.md)
C1_VECTORS = {
    "V1_POINTER_V5_PUBLIC_TEXT": {
        "memo_type_hex": "70662e707472",
        "memo_format_hex": "7635",
        "memo_data_hex": "0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d100118042064320d7468726561642d63312d3030314002",
        "byte_len": 61,
        "cid_bin_hex": "015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d",
        "cid_multibase": "bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu",
    },
    "V2A_POINTER_V5_WORST_CASE_BUDGET": {
        "memo_data_hex": "0a24015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d1002180c20012a287461736b2d323032362d31312d32372d3030303030303030303030303030303030303030303030313228676f762d73657373696f6e2d323032362d31312d32372d30303030303030303030303030303030313a286374782d736e617073686f742d323032362d31312d32372d303030303030303030303030303030314012521c6f72672e6578616d706c652e637573746f6d2e7061636b65742e7631",
        "byte_len": 202,
    },
    "V2B_POINTER_V5_INLINE_X402": {
        "request_hash_hex": "ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56",
        "memo_data_hex": "1003180e20920340064a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb56",
        "byte_len": 43,
    },
    "V3_LEGACY_POINTER_V4_DECODE": {
        "memo_type_hex": "70662E707472",
        "memo_format_hex": "7634",
        "memo_data_hex_upper": "0A3B6261666B726569676832616B69736361696C646371616273796733646672366368753366677072656769796D73636B376537617161347335327A791001180620012A107461736B2D6C65676163792D303034324001",
        "byte_len": 87,
    },
    "V4_ENVELOPE_PUBLIC_TEXT": {
        "header_hex": "08011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d303031",
        "body_hex": "0a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e",
        "signing_preimage_sha256_hex": "e9d50d27fa79f160fda793f8133fc9ce19f5a507ed539e2a4bbea19d67c53ba7",
        "envelope_hex": "0801127508011211706f7374666961742d6465766e65742d321a1270662e6d6573736167652e746578742e7631206428043001380140014a2a70663173656e6465723030303030303030303030303030303030303030303030303030303030303030305080a8f094ac33620d7468726561642d63312d30303122230a1548656c6c6f20506f73742046696174204c31763221120a746578742f706c61696e28013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
        "cid_bin_hex": "015512202acc209da9af312de8e9d20175c075a22b31b329a3e6d856dec9cf66436d523d",
        "cid_multibase": "bafkreibkzqqj3knpgew6r2osaf24a5ncfmy3gknd43mfnxwjz5teg3kshu",
    },
    "V5_ENVELOPE_PRIVATE_XWING_REWARD": {
        "order_hash_hex": "1fa9e64486fcda633292370aa97c47fa1ffa1463b8bc3804503e4d9a03c94094",
        "header_hex": "08011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317abf0108011211706f7374666961742d6465766e65742d321a201fa9e64486fcda633292370aa97c47fa1ffa1463b8bc3804503e4d9a03c940942220986cb4a5f6ae7ce3b980118f577d54a1a32347ac8d467394479f43d340e04ef832035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee01508208",
        "aad_context_hex": "34df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c70",
        "derived_kem_seed_hex": "430e4662271619e3e43878bddafd267013540a0e72aec34a4eeb1225af12199a",
        "recipient_key_id_hex": "9f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4",
        "derived_wrap_kek_hex": "3f70600a1a973a8b38b1868852cf8db36c2517f58aa389c6b02369678154f9eb",
        "key_binding_preimage_sha256_hex": "6c4a3977bfc8b2f67e028c5b648d375fe21fe2dda86832e83ae6e5d0545560e4",
        "encryption_header_hex": "080212187777777777777777777777777777777777777777777777771a2034df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c702a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a30666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666",
        "signing_preimage_sha256_hex": "32442d9bd94a1edb029e6293d6814ab5eaf58d2eaefc25c6d89fe5b3b09386b4",
        "envelope_hex": "080112b50208011211706f7374666961742d6465766e65742d321a0c70662e7265776172642e763120c90128063002380140024a2a7066317461736b6e6f64653030303030303030303030303030303030303030303030303030303030303050e0fcf394ac335a107461736b2d7265776172642d303030317abf0108011211706f7374666961742d6465766e65742d321a201fa9e64486fcda633292370aa97c47fa1ffa1463b8bc3804503e4d9a03c940942220986cb4a5f6ae7ce3b980118f577d54a1a32347ac8d467394479f43d340e04ef832035046543a2a7066317461736b6e6f646530303030303030303030303030303030303030303030303030303030303030422a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304880cab5ee015082081a9b02080212187777777777777777777777777777777777777777777777771a2034df597e69504a383a97eba1f106a9286b71496de75c7a66af953d4cec167c702a1c706f7374666961742d6c312d76322f7877696e672d777261702f763132bc010a209f72ea0cf49536e3c66c787f705186df9a4378083753ae9536d65b3ad7fcddc4122a7066317265636970303030303030303030303030303030303030303030303030303030303030303030301a20444444444444444444444444444444444444444444444444444444444444444422185555555555555555555555555555555555555555555555552a3066666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666622408888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888828013220a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a13a20b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
        "cid_bin_hex": "0155122073a90001a7ea8b4f5a317c76d97b06049fbd647300aafd4ab0d6ca1e72d100aa",
        "cid_multibase": "bafkreidtveaadj7krnhvuml4o3mxwbqet66wi4yavl6uvmgwziphfuiavi",
    },
    "V6_MANIFEST_MULTICHUNK": {
        "manifest_hex": "0801108080601880804022186170706c69636174696f6e2f6f637465742d73747265616d2a1570662e646f63756d656e742e62756e646c652e76313004380242830112670a24015512200147e682a1ce4cf491afa19c10b08a40f6559f657576398a7788c37c60fcf88a109080401a3b6261666b72656961626937746966696f6f6a74326a646c35627471696c62637361367a6b7a367a6c766f79347975353469796e36676237687972691a18909090909090909090909090909090909090909090909090428501080112670a24015512205b914ae0bd580f1a94824cfc2cff2cf61c2988f46c36e00aa87c5e46ecb584c2109080201a3b6261666b72656963337366666f62706b7962346e6a6a61736d37717770366c6877647175797235646d67337161766b64346c7a646f7a6e6d6579691a18919191919191919191919191919191919191919191919191",
        "byte_len": 333,
    },
    "V7_X402_HEADERS": {
        "required_hex": "080210011a11706f7374666961743a6465766e65742d3220012a0350465430a08d063a2a7066317265636970303030303030303030303030303030303030303030303030303030303030303030304220ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb564a10c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c150e0cf8295ac335a2568747470733a2f2f6170692e706f7374666961742e6f72672f76312f696e666572656e6365621968747470733a2f2f783430322e706f7374666961742e6f7267",
        "signature_hex": "080210011a11706f7374666961743a6465766e65742d3220012a20ee159abe7875173448aac81f2ac10748b46f3f8261f8d78d2b2d98b063a2bb563210c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c13a2083367ad31fc90e2e2fcba5b711ffd80f264f92c5c68aca0241cb89477bc522e24a20d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2",
        "response_hex": "080210011abc0108011211706f7374666961742d6465766e65742d321a2083367ad31fc90e2e2fcba5b711ffd80f264f92c5c68aca0241cb89477bc522e22220c09c70d2488ee1e63c12bad892f3be484f363e34181e8c58d1a6dde8eda315f432035046543a2a70663173656e646572303030303030303030303030303030303030303030303030303030303030303030422a70663172656369703030303030303030303030303030303030303030303030303030303030303030303048a08d06501a20faa9f094ac332a17706f7374666961742d666163696c697461746f722d7631",
        "dedup_order_hash_hex": "83367ad31fc90e2e2fcba5b711ffd80f264f92c5c68aca0241cb89477bc522e2",
    },
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
                self.assertEqual(bundled.read_bytes(), root_proto.read_bytes(), f"proto drift in {rel}")

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


if __name__ == "__main__":
    unittest.main()
