"""Recipient-only KMS JSON/SigV4 requests over enclave-owned TLS.

Credentials are supplied explicitly by the runtime. This client has no default
credential chain, metadata-service access, general AWS operation, or retry path.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Mapping

from .canonical import canonical_json_bytes
from .nitro_kms import NITRO_RECIPIENT_ALGORITHM
from .nitro_transport import NitroHttpsJsonTransport


MAX_KMS_RESPONSE_BYTES = 64 * 1024


def _text(value: Any, maximum: int, name: str) -> str:
    if (not isinstance(value, str) or not 0 < len(value) <= maximum
            or any(ord(char) < 33 or ord(char) > 126 for char in value)):
        raise ValueError(f"invalid {name}")
    return value


@dataclass(frozen=True)
class AwsTemporaryCredentials:
    access_key_id: str = field(repr=False)
    secret_access_key: str = field(repr=False)
    session_token: str = field(repr=False)

    def __post_init__(self) -> None:
        _text(self.access_key_id, 128, "AWS access key identifier")
        _text(self.secret_access_key, 256, "AWS secret access key")
        _text(self.session_token, 16 * 1024, "AWS session token")


def _object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate KMS response field")
        result[key] = value
    return result


def _binary(value: Any, maximum: int, name: str) -> bytes:
    if not isinstance(value, bytes) or not 0 < len(value) <= maximum:
        raise ValueError(f"invalid {name}")
    return value


class NitroKmsJsonClient:
    """Two fixed KMS operations; every request requires Nitro Recipient.

    Version 1 supports the standard commercial KMS endpoint and an explicit
    regional key ARN. Region, key and encryption context have no defaults.
    """

    def __init__(
        self,
        transport: NitroHttpsJsonTransport,
        *,
        region: str,
        credentials: Callable[[], AwsTemporaryCredentials],
        clock: Callable[[], datetime],
    ) -> None:
        if (not isinstance(region, str) or not 1 <= len(region) <= 64
                or any(char not in "abcdefghijklmnopqrstuvwxyz0123456789-" for char in region)):
            raise ValueError("invalid KMS signing region")
        if transport.host != f"kms.{region}.amazonaws.com":
            raise ValueError("KMS transport must use the fixed regional endpoint")
        if (type(transport.max_response_bytes) is not int
                or not 0 < transport.max_response_bytes <= MAX_KMS_RESPONSE_BYTES):
            raise ValueError("KMS transport response bound exceeds 64 KiB")
        self.transport = transport
        self.region = region
        self.credentials = credentials
        self.clock = clock

    def _request(
        self, operation: str, *, KeyId: str, Recipient: Mapping[str, Any],
        EncryptionContext: Mapping[str, str], CiphertextBlob: bytes | None = None,
    ) -> dict[str, Any]:
        if (self.transport.host != f"kms.{self.region}.amazonaws.com"
                or type(self.transport.max_response_bytes) is not int
                or not 0 < self.transport.max_response_bytes <= MAX_KMS_RESPONSE_BYTES):
            raise ValueError("KMS transport configuration changed outside its fixed bounds")
        _text(KeyId, 2048, "KMS key ARN")
        parts = KeyId.split(":")
        if (len(parts) != 6 or parts[:4] != ["arn", "aws", "kms", self.region]
                or len(parts[4]) != 12 or any(c not in "0123456789" for c in parts[4])
                or not parts[5].startswith("key/") or len(parts[5]) <= 4):
            raise ValueError("KMS key must be an explicit regional key ARN")
        if (not isinstance(Recipient, Mapping)
                or set(Recipient) != {"KeyEncryptionAlgorithm", "AttestationDocument"}
                or Recipient["KeyEncryptionAlgorithm"] != NITRO_RECIPIENT_ALGORITHM):
            raise ValueError("KMS request requires the Nitro recipient profile")
        document = _binary(Recipient["AttestationDocument"], 16 * 1024, "Nitro document")
        if (not isinstance(EncryptionContext, Mapping) or not EncryptionContext
                or any(not isinstance(k, str) or not k or not isinstance(v, str) or not v
                       for k, v in EncryptionContext.items())):
            raise ValueError("KMS encryption context must contain explicit string pairs")
        value: dict[str, Any] = {
            "KeyId": KeyId, "EncryptionContext": dict(EncryptionContext),
            "Recipient": {"KeyEncryptionAlgorithm": NITRO_RECIPIENT_ALGORITHM,
                          "AttestationDocument": base64.b64encode(document).decode("ascii")},
        }
        if operation == "Decrypt":
            value["CiphertextBlob"] = base64.b64encode(
                _binary(CiphertextBlob, 6144, "KMS ciphertext blob")
            ).decode("ascii")
        elif operation == "GenerateDataKey" and CiphertextBlob is None:
            value["KeySpec"] = "AES_256"
        else:
            raise ValueError("unsupported KMS operation")
        body = canonical_json_bytes(value)
        if len(body) > MAX_KMS_RESPONSE_BYTES:
            raise ValueError("KMS request exceeds its bound")
        credentials = self.credentials()
        if not isinstance(credentials, AwsTemporaryCredentials):
            raise ValueError("explicit temporary AWS credentials are required")
        now = self.clock()
        if not isinstance(now, datetime) or now.tzinfo is None or now.utcoffset() is None:
            raise ValueError("KMS signing clock must be timezone-aware")
        timestamp = now.astimezone(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        day = timestamp[:8]
        headers = {
            "content-type": "application/x-amz-json-1.1", "host": self.transport.host,
            "x-amz-date": timestamp, "x-amz-security-token": credentials.session_token,
            "x-amz-target": "TrentService." + operation,
        }
        names = ";".join(sorted(headers))
        canonical_headers = "".join(f"{name}:{headers[name]}\n" for name in sorted(headers))
        canonical_request = "\n".join([
            "POST", "/", "", canonical_headers, names, hashlib.sha256(body).hexdigest(),
        ])
        scope = f"{day}/{self.region}/kms/aws4_request"
        string_to_sign = "\n".join([
            "AWS4-HMAC-SHA256", timestamp, scope,
            hashlib.sha256(canonical_request.encode("ascii")).hexdigest(),
        ])
        signing_key = ("AWS4" + credentials.secret_access_key).encode("ascii")
        for item in (day, self.region, "kms", "aws4_request"):
            signing_key = hmac.digest(signing_key, item.encode("ascii"), "sha256")
        signature = hmac.new(signing_key, string_to_sign.encode("ascii"), "sha256").hexdigest()
        headers["authorization"] = (
            f"AWS4-HMAC-SHA256 Credential={credentials.access_key_id}/{scope}, "
            f"SignedHeaders={names}, Signature={signature}"
        )
        del headers["host"]  # The fixed transport writes the identical Host header.
        response = self.transport.request("POST", "/", headers=headers, body=body)
        if response.status_code != 200:
            raise RuntimeError(f"KMS recipient request returned HTTP {response.status_code}")
        if len(response.content) > MAX_KMS_RESPONSE_BYTES:
            raise ValueError("KMS response exceeds its bound")
        try:
            result = json.loads(response.content, object_pairs_hook=_object)
        except (ValueError, UnicodeDecodeError, RecursionError):
            raise ValueError("invalid KMS response JSON") from None
        if (not isinstance(result, dict) or set(result) - {
            "KeyId", "CiphertextForRecipient", "CiphertextBlob", "Plaintext",
            "EncryptionAlgorithm", "KeyMaterialId",
        }):
            raise ValueError("unexpected KMS response fields")
        if result.get("Plaintext") not in (None, ""):
            raise RuntimeError("KMS exposed plaintext instead of recipient ciphertext")
        if result.get("KeyId") != KeyId:
            raise ValueError("KMS response key differs from the explicit key ARN")
        if operation == "Decrypt" and result.get("EncryptionAlgorithm") != "SYMMETRIC_DEFAULT":
            raise ValueError("KMS response encryption algorithm differs")
        required = {"CiphertextForRecipient"}
        if operation == "GenerateDataKey":
            required.add("CiphertextBlob")
        decoded: dict[str, Any] = {"KeyId": KeyId}
        for name in required:
            encoded = result.get(name)
            try:
                if not isinstance(encoded, str) or not encoded or len(encoded) > 8192:
                    raise ValueError()
                binary = base64.b64decode(encoded, validate=True)
                _binary(binary, 6144, "KMS ciphertext")
                if base64.b64encode(binary).decode("ascii") != encoded:
                    raise ValueError()
            except (TypeError, ValueError):
                raise ValueError("invalid KMS response ciphertext") from None
            decoded[name] = binary
        return decoded

    def decrypt(
        self, *, KeyId: str, CiphertextBlob: bytes, Recipient: Mapping[str, Any],
        EncryptionContext: Mapping[str, str],
    ) -> dict[str, Any]:
        return self._request("Decrypt", KeyId=KeyId, CiphertextBlob=CiphertextBlob,
                             Recipient=Recipient, EncryptionContext=EncryptionContext)

    def generate_data_key(
        self, *, KeyId: str, KeySpec: str, Recipient: Mapping[str, Any],
        EncryptionContext: Mapping[str, str],
    ) -> dict[str, Any]:
        if KeySpec != "AES_256":
            raise ValueError("Nitro archive keys must be AES_256")
        return self._request("GenerateDataKey", KeyId=KeyId, Recipient=Recipient,
                             EncryptionContext=EncryptionContext)
