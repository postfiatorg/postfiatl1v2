"""Synthetic KMS wire-boundary tests; no AWS calls or real credentials."""

import base64
from datetime import datetime, timezone
import json
from pathlib import Path

import pytest

from pftl_schwab_tee.nitro_kms_transport import (
    AwsTemporaryCredentials, NitroKmsJsonClient,
)
from pftl_schwab_tee.nitro_transport import NitroHttpResponse


REGION = "us-east-2"
KEY = "arn:aws:kms:us-east-2:111122223333:key/synthetic-key"
NOW = datetime(2026, 9, 5, 12, 34, 56, tzinfo=timezone.utc)
CREDENTIALS = AwsTemporaryCredentials("synthetic-access", "synthetic-secret", "synthetic-session")
RECIPIENT = {"KeyEncryptionAlgorithm": "RSAES_OAEP_SHA_256",
             "AttestationDocument": b"synthetic-nitro-document"}


class Transport:
    host = "kms.us-east-2.amazonaws.com"
    max_response_bytes = 64 * 1024

    def __init__(self, content=None, status=200):
        self.requests = []
        self.content = json.dumps({
            "KeyId": KEY, "CiphertextForRecipient": base64.b64encode(b"cms-ciphertext").decode(),
            "CiphertextBlob": base64.b64encode(b"kms-ciphertext").decode(),
            "EncryptionAlgorithm": "SYMMETRIC_DEFAULT", "Plaintext": None,
        }).encode() if content is None else content
        self.status = status

    def request(self, method, path, **kwargs):
        self.requests.append((method, path, kwargs))
        return NitroHttpResponse(self.status, {}, self.content)


def client(transport):
    return NitroKmsJsonClient(transport, region=REGION,
                             credentials=lambda: CREDENTIALS, clock=lambda: NOW)


def call(instance, operation="Decrypt", **overrides):
    args = {"KeyId": KEY, "Recipient": RECIPIENT,
            "EncryptionContext": {"runId": "synthetic-wire-test", "label": "évidence"}}
    if operation == "Decrypt":
        args["CiphertextBlob"] = b"encrypted-secret"
        args.update(overrides)
        return instance.decrypt(**args)
    args["KeySpec"] = "AES_256"
    args.update(overrides)
    return instance.generate_data_key(**args)


@pytest.mark.parametrize("operation", ["Decrypt", "GenerateDataKey"])
def test_kms_request_matches_independent_botocore_signature(operation):
    vectors = json.loads((Path(__file__).parent / "fixtures/yolo_kms_sigv4_v1.json").read_text())
    transport = Transport()
    result = call(client(transport), operation)
    method, path, request = transport.requests[0]
    assert (method, path) == ("POST", "/")
    expected = vectors["cases"][operation]
    assert request["body"].decode() == expected["body"]
    assert request["headers"]["authorization"] == expected["authorization"]
    assert "host" not in request["headers"]
    assert result["CiphertextForRecipient"] == b"cms-ciphertext"
    assert "Plaintext" not in result
    if operation == "GenerateDataKey":
        assert result["CiphertextBlob"] == b"kms-ciphertext"


@pytest.mark.parametrize("change", [
    {"Recipient": {}},
    {"Recipient": {**RECIPIENT, "KeyEncryptionAlgorithm": "RSAES_OAEP_SHA_1"}},
    {"Recipient": {**RECIPIENT, "AttestationDocument": b"x" * (16 * 1024 + 1)}},
    {"Recipient": {**RECIPIENT, "unknown": True}},
    {"KeyId": "alias/unpinned"},
    {"KeyId": KEY.replace(REGION, "us-west-2")},
    {"EncryptionContext": {}},
    {"EncryptionContext": {"runId": True}},
    {"CiphertextBlob": b""},
    {"CiphertextBlob": b"x" * 6145},
])
def test_kms_rejects_invalid_release_request_before_transport(change):
    transport = Transport()
    with pytest.raises(ValueError):
        call(client(transport), **change)
    assert transport.requests == []


@pytest.mark.parametrize("field,value", [
    ("Plaintext", base64.b64encode(b"private-data-key").decode()),
    ("KeyId", KEY + "other"),
    ("CiphertextForRecipient", "?"),
    ("CiphertextForRecipient", ""),
    ("CiphertextForRecipient", base64.b64encode(b"x" * 6145).decode()),
    ("EncryptionAlgorithm", "RSAES_OAEP_SHA_256"),
    ("unknown", True),
])
def test_kms_rejects_plaintext_or_substituted_response(field, value):
    transport = Transport()
    response = json.loads(transport.content)
    response[field] = value
    transport.content = json.dumps(response).encode()
    with pytest.raises((ValueError, RuntimeError)) as caught:
        call(client(transport))
    assert "private-data-key" not in str(caught.value)


@pytest.mark.parametrize("content", [
    b'{"KeyId":"first","KeyId":"second"}', b'[]', b'\xff', b'{',
    b'[' * 2000 + b']' * 2000, b'x' * (64 * 1024 + 1),
])
def test_kms_bounded_response_parser(content):
    with pytest.raises(ValueError):
        call(client(Transport(content)))


def test_kms_error_does_not_log_response_body_or_retry():
    transport = Transport(b'private-source-or-key', status=403)
    with pytest.raises(RuntimeError, match="HTTP 403") as caught:
        call(client(transport))
    assert "private-source-or-key" not in str(caught.value)
    assert len(transport.requests) == 1


def test_kms_client_has_no_general_operation_or_credential_discovery():
    transport = Transport()
    instance = client(transport)
    with pytest.raises(TypeError):
        instance.decrypt(KeyId=KEY, CiphertextBlob=b"ciphertext")
    with pytest.raises(ValueError):
        call(instance, "GenerateDataKey", KeySpec="AES_128")
    with pytest.raises(ValueError):
        instance._request("Encrypt", KeyId=KEY, Recipient=RECIPIENT,
                          EncryptionContext={"runId": "synthetic"})
    assert transport.requests == []
    assert repr(CREDENTIALS) == "AwsTemporaryCredentials()"


@pytest.mark.parametrize("host,bound", [
    ("example.com", 64 * 1024), ("kms.us-west-2.amazonaws.com", 64 * 1024),
    ("kms.us-east-2.amazonaws.com", 64 * 1024 + 1),
])
def test_kms_fixed_endpoint_and_response_bound(host, bound):
    transport = Transport()
    transport.host, transport.max_response_bytes = host, bound
    with pytest.raises(ValueError):
        client(transport)


def test_kms_rechecks_fixed_destination_before_using_credentials():
    transport = Transport()
    instance = client(transport)
    transport.host = "example.com"
    instance.credentials = lambda: pytest.fail("credentials must not be obtained")
    with pytest.raises(ValueError, match="configuration changed"):
        call(instance)
    assert not transport.requests
