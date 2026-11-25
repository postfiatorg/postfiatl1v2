"""Replay the public real-hardware capture evidence at its verification time.

These are historical signatures, not a claim that the documents are fresh now.
The AWS root, owner manifest and accepted PCRs are independently pinned here.
Private option-chain responses and archive recovery keys are not test fixtures.
"""

import base64
from datetime import datetime
import json
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import serialization
import pytest

from pftl_schwab_tee.canonical import domain_sha256, sha256_bytes
from pftl_schwab_tee.nitro_attestation import NitroNativeAttestationVerifier
from pftl_schwab_tee.nitro_capture import manifest_identity
from pftl_schwab_tee.nitro_runtime import Ed25519HexSignatureVerifier


EVIDENCE = Path(__file__).parent / "evidence/yolo_closed_market_capture_20260905.json"
ROOT_SHA256 = "641a0321a3e244efe456463195d606317ed7cdcc3c1756e09893f3c68f79bb5b"
MANIFEST_SHA256 = "44a3f102539f50279e96f0aae3d8f34b902665d14734c33708bf57fd88187ef5"
ACCEPTED_PCRS = (
    "fdb14ec2629a7fecdeacff271f0e40948ad7485194d3e97ac096606c0725f63b412bd46370bf4af613fa36fb0dcda73d",
    "0343b056cd8485ca7890ddd833476d78460aed2aa161548e4e26bedf321726696257d623e8805f3f605946b3d8b0c6aa",
    "523f845309b62a7262813671f61cea678c260917dca6692848a1931f55b75cdee8cfefefee1ed06fd06341d73829a142",
)


def test_real_capture_signatures_and_measurements_replay_with_independent_pins():
    evidence = json.loads(EVIDENCE.read_bytes())
    manifest = evidence["captureManifest"]
    assert manifest_identity(manifest)[0] == MANIFEST_SHA256
    root = base64.b64decode(evidence["awsRootCertificateDerBase64"], validate=True)
    assert sha256_bytes(root) == ROOT_SHA256
    attempt = evidence["attempts"][-1]
    verifier = NitroNativeAttestationVerifier(
        root_certificate_pem=x509.load_der_x509_certificate(root).public_bytes(serialization.Encoding.PEM),
        max_age_ms=300_000, max_future_skew_ms=5_000,
        clock=lambda: datetime.fromisoformat("2026-09-05T20:47:29.346169+00:00"),
    )
    recipient, result = attempt["recipient"], attempt["result"]
    recipient_binding = domain_sha256("postfiat.yolo.capture_recipient.v1", {
        "captureManifestSha256": MANIFEST_SHA256, "statementKey": recipient["statementKey"],
    })
    assert recipient["bindingDigest"] == recipient_binding
    before = verifier.parse_and_verify(base64.b64decode(recipient["documentBase64"]),
        attestation_binding_digest=recipient_binding)
    body = result["body"]
    binding = domain_sha256(body["schema"], body)
    assert binding == result["bindingDigest"]
    after = verifier.parse_and_verify(base64.b64decode(result["documentBase64"]),
        attestation_binding_digest=binding)
    for parsed in (before, after):
        assert tuple(parsed.pcrs[i].hex() for i in range(3)) == ACCEPTED_PCRS
        assert parsed.statement_key == body["statementKey"] == recipient["statementKey"]
    assert before.public_key == after.public_key
    signatures = Ed25519HexSignatureVerifier()
    assert signatures.verify(statement_key=after.statement_key,
        signing_digest=binding, signature=result["signature"])
    assert body["captureManifestSha256"] == MANIFEST_SHA256
    assert body["runId"] == manifest["runId"]
    assert body["status"] == "CAPTURED"
    assert body["mode"] == "ON_DEMAND_NO_PORTFOLIO_CALCULATION"
    assert [(x["symbol"], x["contractType"]) for x in body["chains"]] == [
        (symbol, side) for symbol in manifest["symbols"] for side in ("CALL", "PUT")]
    for chain in body["chains"]:
        assert chain["httpStatus"] == 200
        capture_ms = datetime.fromisoformat(chain["capturedAtUtc"]).timestamp() * 1000
        assert chain["newestQuoteTimestampMs"] < capture_ms - 24 * 3600 * 1000
        assert chain["archive"] in [x["receipt"] for x in body["archives"]]
    body["chains"].reverse()
    assert not signatures.verify(statement_key=after.statement_key,
        signing_digest=domain_sha256(body["schema"], body), signature=result["signature"])
    with pytest.raises(ValueError):
        verifier.parse_and_verify(base64.b64decode(result["documentBase64"]),
            attestation_binding_digest="11" * 32)
