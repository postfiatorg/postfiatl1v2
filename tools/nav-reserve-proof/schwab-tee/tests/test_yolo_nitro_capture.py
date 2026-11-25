import base64
from datetime import datetime, timezone
import json
from types import SimpleNamespace

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
import pytest

from pftl_schwab_tee import nitro_capture as capture
from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nitro_transport import NitroHttpResponse


@pytest.fixture
def owner_key():
    return rsa.generate_private_key(public_exponent=65537, key_size=2048)


def manifest(key):
    public = key.public_key().public_bytes(serialization.Encoding.DER,
        serialization.PublicFormat.SubjectPublicKeyInfo)
    return {"schema": capture.MANIFEST_SCHEMA, "runId": "capture-test", "symbols": ["ABC", "XYZ"],
        "accountApplicationIdentitySha256": "11" * 32,
        "operatorPublicKeyDerBase64": base64.b64encode(public).decode()}


def test_manifest_binds_symbol_order_and_owner_recovery_key(owner_key):
    original = manifest(owner_key)
    before, _ = capture.manifest_identity(original)
    original["symbols"].reverse()
    assert capture.manifest_identity(original)[0] != before
    other = manifest(rsa.generate_private_key(public_exponent=65537, key_size=2048))
    assert capture.manifest_identity(other)[0] != before
    original["symbols"] = ["ABC", "ABC"]
    with pytest.raises(ValueError): capture.manifest_identity(original)


def test_exact_response_is_archived_before_next_symbol_and_old_timestamps_remain(owner_key):
    calls, saved = [], []
    now = datetime(2026, 9, 5, 20, tzinfo=timezone.utc)
    old = 1788544800000
    class Client:
        def request(self, method, path, params):
            assert len(saved) == len(calls)
            assert method == "GET" and path == "/marketdata/v1/chains"
            assert set(params) == {"symbol", "contractType", "strategy", "includeUnderlyingQuote"}
            assert params["contractType"] in {"CALL", "PUT"}
            calls.append(params["symbol"])
            data = {"symbol": params["symbol"], "callExpDateMap": {}, "putExpDateMap": {}}
            field = "callExpDateMap" if params["contractType"] == "CALL" else "putExpDateMap"
            data[field] = {"expiry": {"50": [
                {"quoteTimeInLong": old, "bid": 12.34, "ask": 56.78}]}}
            raw = json.dumps(data, indent=2).encode()
            return data, NitroHttpResponse(200, {"date": "a provider date", "set-cookie": "private"}, raw)
    def archive(name, encoded):
        saved.append((name, json.loads(encoded)))
        return {"ciphertextSha256": "22" * 32}
    result = capture.capture_chains(manifest(owner_key), Client(), archive, lambda: now)
    assert calls == ["ABC", "ABC", "XYZ", "XYZ"]
    assert [x["contractType"] for x in result] == ["CALL", "PUT", "CALL", "PUT"]
    assert [x["newestQuoteTimestampMs"] for x in result] == [old, old, old, old]
    assert all(x["capturedAtUtc"] == now.isoformat() for x in result)
    assert [x[0] for x in saved] == ["chain-0-call", "chain-0-put", "chain-1-call", "chain-1-put"]
    assert [x["contractCounts"] for x in result] == [
        {"CALL": 1, "PUT": 0}, {"CALL": 0, "PUT": 1}] * 2
    assert all("set-cookie" not in x[1]["responseHeaders"] for x in saved)
    assert b'"bid"' not in canonical_json_bytes(result)
    assert b'"ask"' not in canonical_json_bytes(result)
    assert json.loads(base64.b64decode(saved[0][1]["rawResponseBase64"]))["symbol"] == "ABC"


@pytest.mark.parametrize("status,symbol", [(500, "ABC"), (200, "WRONG")])
def test_bad_provider_response_is_retained_privately_and_rejected(owner_key, status, symbol):
    saved = []
    payload = {"symbol": symbol, "callExpDateMap": {}, "putExpDateMap": {}}
    class Client:
        def request(self, *args, **kwargs):
            return payload, NitroHttpResponse(status, {}, canonical_json_bytes(payload))
    def archive(name, encoded): saved.append(encoded); return {}
    with pytest.raises((RuntimeError, ValueError)):
        capture.capture_chains(manifest(owner_key), Client(), archive, lambda: datetime.now(timezone.utc))
    assert len(saved) == 1 and base64.b64decode(json.loads(saved[0])["rawResponseBase64"]) == canonical_json_bytes(payload)


def test_operator_recovery_survives_temporary_kms_deletion_and_rejects_substitution(owner_key, monkeypatch):
    key = b"\x84" * 32
    monkeypatch.setattr(capture, "request_attested_data_key_ciphertext", lambda *a, **kw:
        SimpleNamespace(ciphertext_for_recipient=b"cms", encrypted_data_key=b"kms"))
    monkeypatch.setattr(capture, "decrypt_recipient_ciphertext", lambda *a: key)
    encryptor = capture.CaptureEncryptor(kms=object(), key_id="test-kms", context={"Run":"test"},
        recipient_keys=SimpleNamespace(recipient_private_key=object()), document=lambda:b"document",
        operator_key=owner_key.public_key())
    aad, plaintext = b"bound capture artifact", b"private exact source response"
    bundle = json.loads(encryptor.encrypt(plaintext, associated_data=aad))
    unwrapped = owner_key.decrypt(base64.b64decode(bundle["operatorEncryptedDataKeyBase64"]),
        padding.OAEP(mgf=padding.MGF1(hashes.SHA256()), algorithm=hashes.SHA256(), label=aad))
    assert unwrapped == key
    nonce, ciphertext = base64.b64decode(bundle["nonceBase64"]), base64.b64decode(bundle["ciphertextBase64"])
    assert AESGCM(unwrapped).decrypt(nonce, ciphertext, aad) == plaintext
    with pytest.raises(InvalidTag): AESGCM(unwrapped).decrypt(nonce, ciphertext, b"another run")
    with pytest.raises(InvalidTag): AESGCM(unwrapped).decrypt(nonce, ciphertext[:-1]+bytes([ciphertext[-1]^1]), aad)
    with pytest.raises(ValueError): owner_key.decrypt(base64.b64decode(bundle["operatorEncryptedDataKeyBase64"]),
        padding.OAEP(mgf=padding.MGF1(hashes.SHA256()), algorithm=hashes.SHA256(), label=b"another owner context"))
