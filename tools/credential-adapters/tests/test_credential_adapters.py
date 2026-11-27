"""Automated tests for Pillar 8 zero-PII W3C VC & ZK-Passport adapters."""

from __future__ import annotations

import hashlib
import hmac
import json
import sys
import threading
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

import pytest
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from credential_adapters import (  # noqa: E402
    CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND,
    CREDENTIAL_REVOKE_TRANSACTION_KIND,
    ML_DSA_65_PUBLIC_KEY_BYTES,
    ML_DSA_65_SIGNATURE_BYTES,
    PiiLeakageError,
    SubjectDelegatedAcceptance,
    TrustedVendorDid,
    W3cVcAdapterConfig,
    W3cVcIssuerAdapter,
    ZeroPiiCredentialStore,
    ZkPassportAdapterConfig,
    ZkPassportSandboxAdapter,
    assert_zero_pii_structure,
    build_credential_delete_op,
    build_signed_sandbox_zkp_envelope,
    cli,
    delegated_accept_preimage,
    derive_address_from_pubkey_hex,
    derive_credential_id,
    derive_issuer_id,
    format_did_pftl,
    parse_did_pftl,
    sign_w3c_vc_ed25519,
)
from credential_adapters.server import CredentialAdapterService, create_http_server  # noqa: E402

CHAIN_ID = "postfiat-wan-devnet-2"
GENESIS_HASH = "ce" * 48
ADMIN_ACCOUNT = "pf1000000000000000000000000000000000000001"
SIGNING_ACCOUNT = "pf2000000000000000000000000000000000000002"
ISSUER_DOMAIN = "kyc.example"
ISSUER_ID = derive_issuer_id(CHAIN_ID, ADMIN_ACCOUNT, 1)

# Synthetic ML-DSA-65 pubkey & signature fixtures with exact FIPS-204 byte lengths.
SUBJECT_PUBKEY_HEX = (hashlib.sha3_512(b"subject-1").digest() * 31)[:ML_DSA_65_PUBLIC_KEY_BYTES].hex()
SUBJECT_ACCOUNT = derive_address_from_pubkey_hex(SUBJECT_PUBKEY_HEX)
SUBJECT_2_PUBKEY_HEX = (hashlib.sha3_512(b"subject-2").digest() * 31)[:ML_DSA_65_PUBLIC_KEY_BYTES].hex()
SUBJECT_2_ACCOUNT = derive_address_from_pubkey_hex(SUBJECT_2_PUBKEY_HEX)


def _mock_mldsa_sign(pubkey_hex: str, preimage: bytes) -> str:
    """Deterministic full-length (3309-byte) mock ML-DSA-65 signature bound to preimage."""
    tag = hashlib.sha3_384(bytes.fromhex(pubkey_hex) + preimage).digest()
    raw = (tag * ((ML_DSA_65_SIGNATURE_BYTES // len(tag)) + 1))[:ML_DSA_65_SIGNATURE_BYTES]
    return raw.hex()


def _mock_mldsa_verify(pubkey: bytes, preimage: bytes, sig: bytes) -> bool:
    expected = _mock_mldsa_sign(pubkey.hex(), preimage)
    return sig.hex() == expected


def _ed25519_pair(label: bytes) -> tuple[bytes, str]:
    seed = hashlib.sha256(label).digest()
    pub_hex = Ed25519PrivateKey.from_private_bytes(seed).public_key().public_bytes_raw().hex()
    return seed, pub_hex


def _make_w3c_setup(tmp_path: Path):
    vendor_seed, vendor_pub = _ed25519_pair(b"w3c-vendor-ed25519-test")
    vendor_did = "did:web:vendor.example"
    vm = f"{vendor_did}#key-1"
    hmac_material = hashlib.sha256(b"webhook-auth-material").digest()
    cfg = W3cVcAdapterConfig(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        issuer_id=ISSUER_ID,
        issuer_domain=ISSUER_DOMAIN,
        signing_account=SIGNING_ACCOUNT,
        trusted_vendors=(
            TrustedVendorDid(
                vendor_did=vendor_did,
                verification_method=vm,
                ed25519_public_key_hex=vendor_pub,
            ),
        ),
        vc_type_mapping={"KycBasicCredential": f"{ISSUER_DOMAIN}/kyc.basic"},
        allowed_credential_types=(f"{ISSUER_DOMAIN}/kyc.basic",),
        webhook_hmac_bytes=hmac_material,
    )
    store = ZeroPiiCredentialStore(
        tmp_path / "zero_pii_store.json",
        pepper=b"unit-test-pepper",
    )
    adapter = W3cVcIssuerAdapter(cfg, store, mldsa_verifier=_mock_mldsa_verify)
    return adapter, store, vendor_seed, vendor_did, vm, hmac_material


def _make_zkp_setup(tmp_path: Path, store: ZeroPiiCredentialStore | None = None):
    zk_seed, zk_pub = _ed25519_pair(b"zkp-sandbox-verifier-test")
    cfg = ZkPassportAdapterConfig(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        issuer_id=ISSUER_ID,
        issuer_domain=ISSUER_DOMAIN,
        signing_account=SIGNING_ACCOUNT,
        credential_type=f"{ISSUER_DOMAIN}/passport.over18",
        expected_scope=f"postfiat:{CHAIN_ID}:nav-gate",
        trusted_icao_roots=("0x" + "11" * 32,),
        trusted_ofac_roots=("0x" + "22" * 32,),
        sandbox_verifier_pubkey_hex=zk_pub,
        required_minimum_age=18,
    )
    store = store or ZeroPiiCredentialStore(
        tmp_path / "zkp_store.json",
        pepper=b"unit-test-pepper",
    )
    adapter = ZkPassportSandboxAdapter(cfg, store, mldsa_verifier=_mock_mldsa_verify)
    return adapter, store, zk_seed


def test_c4_types_did_and_delegated_accept_preimage_exact_format() -> None:
    did = format_did_pftl(CHAIN_ID, SUBJECT_ACCOUNT)
    assert did == f"did:pftl:{CHAIN_ID}:{SUBJECT_ACCOUNT}"
    assert parse_did_pftl(did) == (CHAIN_ID, SUBJECT_ACCOUNT)

    commitment = "aa" * 48
    preimage = delegated_accept_preimage(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        subject=SUBJECT_ACCOUNT,
        issuer_id=ISSUER_ID,
        credential_type=f"{ISSUER_DOMAIN}/kyc.basic",
        commitment_hash=commitment,
        expires_at_height=500_000,
        subject_accept_nonce=7,
    ).decode("utf-8")
    expected_lines = [
        "postfiat.credential.delegated_accept.v1",
        f"chain_id={CHAIN_ID}",
        f"genesis_hash={GENESIS_HASH}",
        f"subject={SUBJECT_ACCOUNT}",
        f"issuer_id={ISSUER_ID}",
        f"credential_type={ISSUER_DOMAIN}/kyc.basic",
        f"commitment_hash={commitment}",
        "expires_at_height=500000",
        "subject_accept_nonce=7",
        "",
    ]
    assert preimage == "\n".join(expected_lines)

    del_op = build_credential_delete_op(
        caller=SUBJECT_ACCOUNT,
        credential_id=derive_credential_id(SUBJECT_ACCOUNT, ISSUER_ID, f"{ISSUER_DOMAIN}/kyc.basic"),
    )
    assert del_op["operation"] == "credential_delete"


def test_w3c_vc_issuance_revocation_and_zero_pii_persistence(tmp_path: Path) -> None:
    adapter, store, vendor_seed, vendor_did, vm, _ = _make_w3c_setup(tmp_path)
    now_utc = datetime(2026, 11, 27, 12, 0, 0, tzinfo=timezone.utc)
    salt = hashlib.sha256(b"ephemeral-salt-vc-test").digest()

    # Inject rich synthetic PII into the external VC to prove ZERO bytes persist.
    pii_markers = [
        "Alice Example",
        "P99887766",
        "1990-01-01",
        "alice.example@invalid.test",
        "742 Evergreen Terrace",
    ]
    vc_unsigned = {
        "@context": ["https://www.w3.org/ns/credentials/v2"],
        "id": "urn:uuid:vc-external-ref-001",
        "type": ["VerifiableCredential", "KycBasicCredential"],
        "issuer": vendor_did,
        "validFrom": "2026-11-01T00:00:00Z",
        "validUntil": "2027-11-01T00:00:00Z",
        "credentialSubject": {
            "id": format_did_pftl(CHAIN_ID, SUBJECT_ACCOUNT),
            "fullName": pii_markers[0],
            "passportNumber": pii_markers[1],
            "dateOfBirth": pii_markers[2],
            "email": pii_markers[3],
            "streetAddress": pii_markers[4],
        },
    }
    signed_vc = sign_w3c_vc_ed25519(
        vc_unsigned,
        key_seed_32=vendor_seed,
        verification_method=vm,
        created_iso="2026-11-27T11:55:00Z",
    )

    verified = adapter.verify_vc_and_derive_commitment(signed_vc, now_utc=now_utc, salt_bytes=salt)
    expires_at_height = 250_000
    preimage = delegated_accept_preimage(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        subject=SUBJECT_ACCOUNT,
        issuer_id=ISSUER_ID,
        credential_type=f"{ISSUER_DOMAIN}/kyc.basic",
        commitment_hash=verified["commitment_hash"],
        expires_at_height=expires_at_height,
        subject_accept_nonce=1,
    )
    acceptance = SubjectDelegatedAcceptance(
        subject=SUBJECT_ACCOUNT,
        subject_pubkey_hex=SUBJECT_PUBKEY_HEX,
        subject_accept_signature_hex=_mock_mldsa_sign(SUBJECT_PUBKEY_HEX, preimage),
        subject_accept_nonce=1,
    )

    issued = adapter.issue_from_vc(
        vc=signed_vc,
        subject_acceptance=acceptance,
        current_block_height=200_000,
        now_utc=now_utc,
        expires_at_height=expires_at_height,
        salt_bytes=salt,
        request_nonce="req-001",
    )

    op = issued["l1_operation"]
    assert op["operation"] == CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND
    assert op["signing_account"] == SIGNING_ACCOUNT
    assert op["issuer_id"] == ISSUER_ID
    assert op["subject"] == SUBJECT_ACCOUNT
    assert op["credential_type"] == f"{ISSUER_DOMAIN}/kyc.basic"
    assert len(op["commitment_hash"]) == 96
    assert op["expires_at_height"] == expires_at_height
    assert op["subject_accept_nonce"] == 1

    # CRITICAL ZERO-PII CHECK: zero PII bytes in emitted JSON OR on-disk store!
    persisted_text = store.state_path.read_text(encoding="utf-8")
    emitted_text = json.dumps(issued)
    for marker in pii_markers:
        assert marker not in persisted_text
        assert marker not in emitted_text
    assert "urn:uuid:vc-external-ref-001" not in persisted_text  # external ref is one-way hashed!

    # Replay nonce blocked
    with pytest.raises(ValueError, match="replay_detected"):
        adapter.issue_from_vc(
            vc=signed_vc,
            subject_acceptance=acceptance,
            current_block_height=200_005,
            now_utc=now_utc,
            expires_at_height=expires_at_height,
            salt_bytes=salt,
            request_nonce="req-001",
        )

    # Revocation by external VC ID emits exact Contract C4 credential_revoke op.
    revoked = adapter.revoke(
        external_vc_id="urn:uuid:vc-external-ref-001",
        revoked_at_height=205_000,
        reason_code="vendor_status_revoked",
    )
    assert revoked["l1_operation"]["operation"] == CREDENTIAL_REVOKE_TRANSACTION_KIND
    assert revoked["l1_operation"]["caller"] == SIGNING_ACCOUNT
    assert revoked["l1_operation"]["credential_id"] == issued["credential_id"]
    assert store.get(issued["credential_id"])["status"] == "revoked"


def test_w3c_vc_rejects_tampered_sig_expired_and_pii_leakage(tmp_path: Path) -> None:
    adapter, _, vendor_seed, vendor_did, vm, _ = _make_w3c_setup(tmp_path)
    now_utc = datetime(2026, 11, 27, 12, 0, 0, tzinfo=timezone.utc)
    vc = sign_w3c_vc_ed25519(
        {
            "@context": ["https://www.w3.org/2018/credentials/v1"],
            "id": "urn:uuid:vc-2",
            "type": ["VerifiableCredential", "KycBasicCredential"],
            "issuer": vendor_did,
            "issuanceDate": "2026-01-01T00:00:00Z",
            "expirationDate": "2026-06-01T00:00:00Z",
            "credentialSubject": {"id": format_did_pftl(CHAIN_ID, SUBJECT_ACCOUNT)},
        },
        key_seed_32=vendor_seed,
        verification_method=vm,
        created_iso="2026-01-01T00:00:00Z",
        proof_type="Ed25519Signature2020",
    )
    with pytest.raises(ValueError, match="vc_expired"):
        adapter.verify_vc_and_derive_commitment(vc, now_utc=now_utc)

    # Tampered signature rejected
    tampered = json.loads(json.dumps(vc))
    tampered["expirationDate"] = "2027-06-01T00:00:00Z"
    with pytest.raises(ValueError, match="vc_invalid_ed25519_signature"):
        adapter.verify_vc_and_derive_commitment(tampered, now_utc=now_utc)

    # Zero-PII firewall blocks forbidden keys
    with pytest.raises(PiiLeakageError):
        assert_zero_pii_structure({"nested": {"passportNumber": "X123"}})


def test_zk_passport_sandbox_issuance_nullifier_dedup_and_revocation(tmp_path: Path) -> None:
    adapter, store, zk_seed = _make_zkp_setup(tmp_path)
    now_unix = 1_795_750_000
    salt = hashlib.sha256(b"ephemeral-salt-zkp-test").digest()
    raw_nullifier = "0x" + "9a" * 32

    envelope = build_signed_sandbox_zkp_envelope(
        provider="self",
        circuit_id="self-passport-disclose-v1",
        public_signals={
            "scope": f"postfiat:{CHAIN_ID}:nav-gate",
            "merkle_root": "0x" + "11" * 32,
            "ofac_root": "0x" + "22" * 32,
            "age_verified": True,
            "minimum_age": 18,
            "ofac_passed": True,
            "nationality_excluded": False,
            "nullifier": raw_nullifier,
            "user_identifier": format_did_pftl(CHAIN_ID, SUBJECT_ACCOUNT),
            "proof_expires_at_unix": now_unix + 3600,
        },
        proof_payload={"pi_a": ["1", "2"], "pi_b": [["3", "4"]], "pi_c": ["5", "6"]},
        sandbox_signer_seed_32=zk_seed,
    )

    verified = adapter.verify_proof_envelope(envelope, now_unix=now_unix, salt_bytes=salt)
    expires_at_height = 400_000
    preimage = delegated_accept_preimage(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        subject=SUBJECT_ACCOUNT,
        issuer_id=ISSUER_ID,
        credential_type=f"{ISSUER_DOMAIN}/passport.over18",
        commitment_hash=verified["commitment_hash"],
        expires_at_height=expires_at_height,
        subject_accept_nonce=1,
    )
    acceptance = SubjectDelegatedAcceptance(
        subject=SUBJECT_ACCOUNT,
        subject_pubkey_hex=SUBJECT_PUBKEY_HEX,
        subject_accept_signature_hex=_mock_mldsa_sign(SUBJECT_PUBKEY_HEX, preimage),
        subject_accept_nonce=1,
    )

    issued = adapter.verify_and_issue(
        proof_envelope=envelope,
        subject_acceptance=acceptance,
        current_block_height=300_000,
        now_unix=now_unix,
        expires_at_height=expires_at_height,
        salt_bytes=salt,
    )
    assert issued["l1_operation"]["operation"] == CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND
    assert issued["l1_operation"]["credential_type"] == f"{ISSUER_DOMAIN}/passport.over18"
    # Raw nullifier is never stored in cleartext on disk
    assert raw_nullifier not in store.state_path.read_text(encoding="utf-8")

    # Anti-Sybil: reusing the SAME passport nullifier for a SECOND account fails closed!
    envelope_sybil = build_signed_sandbox_zkp_envelope(
        provider="self",
        circuit_id="self-passport-disclose-v1",
        public_signals={
            **envelope["public_signals"],
            "user_identifier": format_did_pftl(CHAIN_ID, SUBJECT_2_ACCOUNT),
        },
        proof_payload=envelope["proof"],
        sandbox_signer_seed_32=zk_seed,
    )
    v2 = adapter.verify_proof_envelope(envelope_sybil, now_unix=now_unix, salt_bytes=salt)
    preimage2 = delegated_accept_preimage(
        chain_id=CHAIN_ID,
        genesis_hash=GENESIS_HASH,
        subject=SUBJECT_2_ACCOUNT,
        issuer_id=ISSUER_ID,
        credential_type=f"{ISSUER_DOMAIN}/passport.over18",
        commitment_hash=v2["commitment_hash"],
        expires_at_height=expires_at_height,
        subject_accept_nonce=1,
    )
    acc2 = SubjectDelegatedAcceptance(
        subject=SUBJECT_2_ACCOUNT,
        subject_pubkey_hex=SUBJECT_2_PUBKEY_HEX,
        subject_accept_signature_hex=_mock_mldsa_sign(SUBJECT_2_PUBKEY_HEX, preimage2),
        subject_accept_nonce=1,
    )
    with pytest.raises(ValueError, match="zkp_nullifier_already_used"):
        adapter.verify_and_issue(
            proof_envelope=envelope_sybil,
            subject_acceptance=acc2,
            current_block_height=300_010,
            now_unix=now_unix,
            expires_at_height=expires_at_height,
            salt_bytes=salt,
        )

    # Revocation works
    rev = adapter.revoke(
        credential_id=issued["credential_id"],
        revoked_at_height=305_000,
        reason_code="user_requested_revocation",
    )
    assert rev["l1_operation"]["operation"] == CREDENTIAL_REVOKE_TRANSACTION_KIND
    assert store.get(issued["credential_id"])["status"] == "revoked"


def test_http_webhook_server_end_to_end_and_cli(tmp_path: Path) -> None:
    w3c_adapter, store, vendor_seed, vendor_did, vm, hmac_bytes = _make_w3c_setup(tmp_path)
    zkp_adapter, _, zk_seed = _make_zkp_setup(tmp_path, store=store)
    service = CredentialAdapterService(store=store, w3c_adapter=w3c_adapter, zkp_adapter=zkp_adapter)
    server = create_http_server("127.0.0.1", 0, service)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        base = f"http://127.0.0.1:{port}"
        with urllib.request.urlopen(f"{base}/health", timeout=5) as resp:
            health = json.loads(resp.read().decode("utf-8"))
        assert health["ok"] is True
        assert health["zero_pii_enforced"] is True

        # Issue via W3C VC webhook with HMAC header
        now_utc = datetime(2026, 11, 27, 12, 0, 0, tzinfo=timezone.utc)
        salt = hashlib.sha256(b"http-vc-salt").digest()
        vc = sign_w3c_vc_ed25519(
            {
                "@context": ["https://www.w3.org/ns/credentials/v2"],
                "id": "urn:uuid:vc-http-1",
                "type": ["VerifiableCredential", "KycBasicCredential"],
                "issuer": vendor_did,
                "validFrom": "2026-11-01T00:00:00Z",
                "validUntil": "2027-11-01T00:00:00Z",
                "credentialSubject": {
                    "id": format_did_pftl(CHAIN_ID, SUBJECT_ACCOUNT),
                    "fullName": "Sensitive Name Never Persisted",
                },
            },
            key_seed_32=vendor_seed,
            verification_method=vm,
            created_iso="2026-11-27T11:55:00Z",
        )
        verified = w3c_adapter.verify_vc_and_derive_commitment(vc, now_utc=now_utc, salt_bytes=salt)
        preimage = delegated_accept_preimage(
            chain_id=CHAIN_ID,
            genesis_hash=GENESIS_HASH,
            subject=SUBJECT_ACCOUNT,
            issuer_id=ISSUER_ID,
            credential_type=f"{ISSUER_DOMAIN}/kyc.basic",
            commitment_hash=verified["commitment_hash"],
            expires_at_height=250_000,
            subject_accept_nonce=2,
        )
        req_body = json.dumps(
            {
                "verifiable_credential": vc,
                "subject_acceptance": {
                    "subject": SUBJECT_ACCOUNT,
                    "subject_pubkey_hex": SUBJECT_PUBKEY_HEX,
                    "subject_accept_signature_hex": _mock_mldsa_sign(SUBJECT_PUBKEY_HEX, preimage),
                    "subject_accept_nonce": 2,
                },
                "current_block_height": 200_000,
                "expires_at_height": 250_000,
                "now_utc": now_utc.isoformat(),
                "salt_hex": salt.hex(),
            }
        ).encode("utf-8")

        # Missing HMAC header -> 401
        bad_req = urllib.request.Request(
            f"{base}/v1/w3c-vc/issue",
            data=req_body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with pytest.raises(urllib.error.HTTPError) as exc_info:
            urllib.request.urlopen(bad_req, timeout=5)
        assert exc_info.value.code == 401

        # Valid HMAC -> 200 and zero PII
        sig_hex = hmac.new(hmac_bytes, req_body, hashlib.sha256).hexdigest()
        good_req = urllib.request.Request(
            f"{base}/v1/w3c-vc/issue",
            data=req_body,
            headers={
                "Content-Type": "application/json",
                "X-PostFiat-Webhook-Signature": sig_hex,
            },
            method="POST",
        )
        with urllib.request.urlopen(good_req, timeout=5) as resp:
            issued_resp = json.loads(resp.read().decode("utf-8"))
        assert issued_resp["ok"] is True
        assert "Sensitive Name Never Persisted" not in json.dumps(issued_resp)
        cred_id = issued_resp["result"]["credential_id"]

        # GET /v1/credentials/<id> returns zero-PII status
        with urllib.request.urlopen(f"{base}/v1/credentials/{cred_id}", timeout=5) as resp:
            status_resp = json.loads(resp.read().decode("utf-8"))
        assert status_resp["result"]["status"] == "issued"
        assert "Sensitive Name Never Persisted" not in json.dumps(status_resp)
    finally:
        server.shutdown()
        server.server_close()

    # CLI smoke test
    assert cli.main(["did", "--chain-id", CHAIN_ID, "--pubkey-hex", SUBJECT_PUBKEY_HEX]) == 0
    assert (
        cli.main(
            [
                "credential-id",
                "--subject",
                SUBJECT_ACCOUNT,
                "--issuer-id",
                ISSUER_ID,
                "--credential-type",
                f"{ISSUER_DOMAIN}/kyc.basic",
            ]
        )
        == 0
    )
