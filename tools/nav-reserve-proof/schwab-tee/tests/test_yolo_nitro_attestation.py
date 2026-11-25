from __future__ import annotations

from datetime import datetime, timedelta, timezone
from dataclasses import replace
from pathlib import Path
import json

import pytest
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, rsa
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature
from cryptography.x509.oid import NameOID

from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.canonical import sha256_bytes
from pftl_schwab_tee.attestation import AttestedStatement, TeeEvidence
from pftl_schwab_tee.nitro_runtime import (
    Ed25519HexSignatureVerifier,
    NitroStatementKeys,
)
from pftl_schwab_tee.tee_policy import (
    PolicyBoundTeeEvidenceVerifier,
    TeeMeasurementPolicy,
)
from pftl_schwab_tee.nitro_attestation import (
    NITRO_USER_DATA_SCHEMA,
    NitroNativeAttestationVerifier,
    _CborTag,
    _encode_cbor,
)


NOW = datetime(2026, 9, 4, 15, 0, tzinfo=timezone.utc)
BINDING_DIGEST = "42" * 32
STATEMENT_KEY = "24" * 32


def _key_usage(*, ca: bool) -> x509.KeyUsage:
    return x509.KeyUsage(
        digital_signature=not ca,
        content_commitment=False,
        key_encipherment=False,
        data_encipherment=False,
        key_agreement=False,
        key_cert_sign=ca,
        crl_sign=ca,
        encipher_only=False,
        decipher_only=False,
    )


def _certificates() -> tuple[
    ec.EllipticCurvePrivateKey,
    x509.Certificate,
    ec.EllipticCurvePrivateKey,
    x509.Certificate,
]:
    root_key = ec.generate_private_key(ec.SECP384R1())
    root_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "aws.nitro-enclaves")])
    root = (
        x509.CertificateBuilder()
        .subject_name(root_name)
        .issuer_name(root_name)
        .public_key(root_key.public_key())
        .serial_number(1)
        .not_valid_before(NOW - timedelta(days=1))
        .not_valid_after(NOW + timedelta(days=1))
        .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
        .add_extension(_key_usage(ca=True), critical=True)
        .sign(root_key, hashes.SHA384())
    )
    leaf_key = ec.generate_private_key(ec.SECP384R1())
    leaf_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "nitro-leaf")])
    leaf = (
        x509.CertificateBuilder()
        .subject_name(leaf_name)
        .issuer_name(root_name)
        .public_key(leaf_key.public_key())
        .serial_number(2)
        .not_valid_before(NOW - timedelta(hours=1))
        .not_valid_after(NOW + timedelta(hours=1))
        .add_extension(x509.BasicConstraints(ca=False, path_length=None), critical=True)
        .add_extension(_key_usage(ca=False), critical=True)
        .sign(root_key, hashes.SHA384())
    )
    return root_key, root, leaf_key, leaf


def _document(
    *,
    timestamp: datetime = NOW,
    pcr0: bytes = b"\x11" * 48,
    all_pcrs_zero: bool = False,
    nonce: bytes = bytes.fromhex(BINDING_DIGEST),
    statement_digest: str = BINDING_DIGEST,
    statement_key: str = STATEMENT_KEY,
    user_data_schema: str = NITRO_USER_DATA_SCHEMA,
    certificates=None,
    recipient_key=None,
    payload_overrides=None,
    protected_header=None,
    unprotected_header=None,
) -> tuple[bytes, bytes]:
    _, root, leaf_key, leaf = certificates or _certificates()
    if recipient_key is None:
        recipient_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    user_data = canonical_json_bytes({
        "schema": user_data_schema,
        "statementKey": statement_key,
        "statementBindingDigest": statement_digest,
    })
    pcrs = (
        {0: b"\x00" * 48, 1: b"\x00" * 48, 2: b"\x00" * 48}
        if all_pcrs_zero
        else {0: pcr0, 1: b"\x12" * 48, 2: b"\x13" * 48}
    )
    payload_value = {
        "module_id": "i-test-enc-test",
        "digest": "SHA384",
        "timestamp": int(timestamp.timestamp() * 1000),
        "pcrs": pcrs,
        "certificate": leaf.public_bytes(serialization.Encoding.DER),
        "cabundle": [root.public_bytes(serialization.Encoding.DER)],
        "public_key": recipient_key.public_key().public_bytes(
            serialization.Encoding.DER,
            serialization.PublicFormat.SubjectPublicKeyInfo,
        ),
        "user_data": user_data,
        "nonce": nonce,
    }
    if payload_overrides:
        payload_value.update(payload_overrides)
    payload = _encode_cbor(payload_value)
    protected = _encode_cbor({1: -35} if protected_header is None else protected_header)
    sig_structure = _encode_cbor(["Signature1", protected, b"", payload])
    der_signature = leaf_key.sign(sig_structure, ec.ECDSA(hashes.SHA384()))
    r, s = decode_dss_signature(der_signature)
    signature = r.to_bytes(48, "big") + s.to_bytes(48, "big")
    document = _encode_cbor(
        _CborTag(18, [protected, {} if unprotected_header is None else unprotected_header, payload, signature])
    )
    return document, root.public_bytes(serialization.Encoding.PEM)


def _verifier(root_pem: bytes) -> NitroNativeAttestationVerifier:
    return NitroNativeAttestationVerifier(
        root_certificate_pem=root_pem,
        max_age_ms=60_000,
        max_future_skew_ms=1_000,
        clock=lambda: NOW,
    )


def test_real_nitro_cose_and_certificate_path_produce_bound_claims() -> None:
    document, root_pem = _document()

    claims = _verifier(root_pem).verify_and_extract(
        document,
        attestation_binding_digest=BINDING_DIGEST,
    )

    assert claims.platform == "aws-nitro"
    assert claims.hardware_backed is True
    assert claims.debug_mode is False
    assert claims.measurement == "pcr0:" + "11" * 48
    assert claims.statement_key == STATEMENT_KEY


def test_nitro_attestation_rejects_replay_tamper_stale_and_debug() -> None:
    document, root_pem = _document()
    verifier = _verifier(root_pem)

    with pytest.raises(ValueError, match="another statement"):
        verifier.verify_and_extract(document, attestation_binding_digest="43" * 32)

    tampered = document[:-1] + bytes([document[-1] ^ 1])
    with pytest.raises(ValueError, match="COSE signature"):
        verifier.verify_and_extract(tampered, attestation_binding_digest=BINDING_DIGEST)

    stale, stale_root = _document(timestamp=NOW - timedelta(minutes=2))
    with pytest.raises(ValueError, match="stale"):
        _verifier(stale_root).verify_and_extract(
            stale,
            attestation_binding_digest=BINDING_DIGEST,
        )

    pcr0_zero, pcr0_zero_root = _document(pcr0=b"\x00" * 48)
    parsed = _verifier(pcr0_zero_root).verify_and_extract(
        pcr0_zero,
        attestation_binding_digest=BINDING_DIGEST,
    )
    assert parsed.debug_mode is False

    all_zero, zero_root = _document(all_pcrs_zero=True)
    assert _verifier(zero_root).verify_and_extract(
        all_zero,
        attestation_binding_digest=BINDING_DIGEST,
    ).debug_mode is True


def test_nitro_attestation_requires_statement_digest_nonce() -> None:
    document, root_pem = _document(nonce=b"wrong")
    with pytest.raises(ValueError, match="nonce"):
        _verifier(root_pem).verify_and_extract(
            document,
            attestation_binding_digest=BINDING_DIGEST,
        )


@pytest.fixture
def complete_statement():
    """Exercise real synthetic Nitro PKI and Ed25519 through the full adapter."""
    keys = NitroStatementKeys.generate()
    pending = AttestedStatement(
        statement_kind="option-chain-snapshot",
        epoch_sha256="01" * 32,
        input_sha256="02" * 32,
        output_sha256="03" * 32,
        sequence=0,
        previous_statement_sha256=None,
        signature="pending",
        evidence=TeeEvidence(
            platform="aws-nitro", trust_class="attested", hardware_backed=True,
            debug_mode=False, measurement="pcr0:" + "11" * 48,
            statement_key=keys.statement_key,
            attestation_document_sha256="00" * 32,
            verifier_id="synthetic-nitro-v2",
        ),
    )
    binding = pending.attestation_binding_digest
    document, root = _document(
        statement_digest=binding, nonce=bytes.fromhex(binding),
        statement_key=keys.statement_key,
    )
    unsigned = replace(pending, evidence=replace(
        pending.evidence, attestation_document_sha256=sha256_bytes(document),
    ))
    complete = replace(unsigned, signature=keys.sign_digest(unsigned.signing_digest))
    documents = {sha256_bytes(document): document}

    class Documents:
        def get_document(self, digest):
            return documents.get(digest, b"")

    verifier = PolicyBoundTeeEvidenceVerifier(
        TeeMeasurementPolicy(
            verifier_id=complete.evidence.verifier_id, platform="aws-nitro",
            allowed_measurements=(complete.evidence.measurement,),
        ),
        document_source=Documents(), native_verifier=_verifier(root),
        signature_verifier=Ed25519HexSignatureVerifier(),
    )
    return pending, complete, verifier, documents


def test_complete_nitro_statement_has_acyclic_binding(complete_statement) -> None:
    pending, complete, verifier, _ = complete_statement
    assert pending.attestation_binding_digest == complete.attestation_binding_digest
    assert pending.signing_digest != complete.signing_digest
    assert pending.sha256 != complete.sha256
    before = complete.to_dict()
    verifier.verify(AttestedStatement.from_dict(before))
    assert complete.to_dict() == before


@pytest.mark.parametrize("field,value", [
    ("statement_kind", "portfolio-target-input"),
    ("epoch_sha256", "04" * 32),
    ("input_sha256", "05" * 32),
    ("output_sha256", "06" * 32),
    ("sequence", 1),
    ("previous_statement_sha256", "07" * 32),
])
def test_complete_nitro_statement_rejects_bound_field_changes(
    complete_statement, field, value,
) -> None:
    _, complete, verifier, _ = complete_statement
    altered = replace(complete, **{field: value})
    assert altered.attestation_binding_digest != complete.attestation_binding_digest
    with pytest.raises(ValueError, match="bound to another statement"):
        verifier.verify(altered)


@pytest.mark.parametrize("field,value", [
    ("measurement", "pcr0:" + "12" * 48),
    ("statement_key", "13" * 32),
    ("verifier_id", "unapproved-verifier"),
])
def test_complete_nitro_statement_rejects_evidence_changes(
    complete_statement, field, value,
) -> None:
    _, complete, verifier, _ = complete_statement
    altered = replace(complete, evidence=replace(complete.evidence, **{field: value}))
    assert altered.attestation_binding_digest != complete.attestation_binding_digest
    with pytest.raises(ValueError):
        verifier.verify(altered)


def test_complete_nitro_statement_rejects_document_and_signature_changes(
    complete_statement,
) -> None:
    _, complete, verifier, documents = complete_statement
    document_hash = complete.evidence.attestation_document_sha256
    original = documents[document_hash]
    documents[document_hash] = original[:-1] + bytes([original[-1] ^ 1])
    with pytest.raises(ValueError, match="document digest"):
        verifier.verify(complete)
    documents[document_hash] = original
    altered = replace(complete, evidence=replace(
        complete.evidence, attestation_document_sha256="09" * 32,
    ))
    assert altered.attestation_binding_digest == complete.attestation_binding_digest
    assert altered.signing_digest != complete.signing_digest
    with pytest.raises(ValueError, match="unavailable"):
        verifier.verify(altered)
    with pytest.raises(ValueError, match="signature is invalid"):
        verifier.verify(replace(complete, signature="invalid"))


def test_nitro_binding_rejects_legacy_user_data_schema() -> None:
    document, root = _document(user_data_schema="postfiat.yolo.nitro_user_data.v1")
    with pytest.raises(ValueError, match="schema mismatch"):
        _verifier(root).verify_and_extract(
            document, attestation_binding_digest=BINDING_DIGEST,
        )


@pytest.mark.parametrize("case", json.loads(
    (Path(__file__).parent / "fixtures/yolo_nitro_validation_v2.json").read_text()
)["cases"], ids=lambda case: case["name"])
def test_shared_nitro_validation_matrix(case) -> None:
    root = x509.load_der_x509_certificate(bytes.fromhex(case["rootDerHex"]))
    policy = case["policy"]
    verifier = NitroNativeAttestationVerifier(
        root_certificate_pem=root.public_bytes(serialization.Encoding.PEM),
        max_age_ms=policy["maxAgeMs"], max_future_skew_ms=policy["maxFutureSkewMs"],
        clock=lambda: datetime.fromtimestamp(case["verificationTimeMs"] / 1000, timezone.utc),
    )
    def verify():
        parsed = verifier.parse_and_verify(
            bytes.fromhex(case["documentHex"]), attestation_binding_digest=case["bindingDigest"],
        )
        approved = any(all(
            parsed.pcrs.get(pcr["index"], b"").hex() == pcr["sha384"] for pcr in pcr_set
        ) for pcr_set in policy["approvedPcrSets"])
        if not approved:
            raise ValueError("Nitro PCRs do not match the approved set")
    if case["valid"]:
        verify()
    else:
        with pytest.raises(ValueError): verify()
