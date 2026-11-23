"""AWS KMS recipient boundary for a future Nitro Enclave implementation.

This module deliberately does not emulate Nitro attestation. It only constructs
the attested-recipient KMS request and rejects any response that exposes secret
plaintext to the caller.
"""

from __future__ import annotations

import base64
import json
import os
from dataclasses import dataclass
from typing import Any, Mapping, Protocol

from .canonical import sha256_bytes
from .canonical import canonical_json_bytes
from .nitro_cms import decrypt_recipient_ciphertext


NITRO_RECIPIENT_ALGORITHM = "RSAES_OAEP_SHA_256"


class KmsRecipientClient(Protocol):
    def decrypt(self, **kwargs: Any) -> Mapping[str, Any]: ...

    def generate_data_key(self, **kwargs: Any) -> Mapping[str, Any]: ...


@dataclass(frozen=True)
class NitroRecipientCiphertext:
    """KMS result still encrypted to the enclave's attested public key."""

    key_id: str
    attestation_document_sha256: str
    ciphertext_for_recipient: bytes

    def __post_init__(self) -> None:
        if not isinstance(self.key_id, str) or not self.key_id.strip():
            raise ValueError("KMS key_id is required")
        if (
            not isinstance(self.attestation_document_sha256, str)
            or len(self.attestation_document_sha256) != 64
            or any(
                character not in "0123456789abcdef"
                for character in self.attestation_document_sha256
            )
        ):
            raise ValueError("attestation document hash must be lowercase SHA-256")
        if not isinstance(self.ciphertext_for_recipient, bytes) or not (
            self.ciphertext_for_recipient
        ):
            raise ValueError("KMS returned no ciphertext for the Nitro recipient")


@dataclass(frozen=True)
class NitroRecipientDataKey:
    key_id: str
    attestation_document_sha256: str
    encrypted_data_key: bytes
    ciphertext_for_recipient: bytes

    def __post_init__(self) -> None:
        if not isinstance(self.key_id, str) or not self.key_id.strip():
            raise ValueError("KMS key_id is required")
        if (
            not isinstance(self.encrypted_data_key, bytes)
            or not self.encrypted_data_key
        ):
            raise ValueError("KMS returned no encrypted data key")
        if (
            not isinstance(self.ciphertext_for_recipient, bytes)
            or not self.ciphertext_for_recipient
        ):
            raise ValueError("KMS returned no recipient-encrypted data key")


NITRO_CIPHERTEXT_BUNDLE_SCHEMA = "postfiat.yolo.nitro_ciphertext_bundle.v1"


def request_attested_secret_ciphertext(
    kms_client: KmsRecipientClient,
    *,
    key_id: str,
    encrypted_secret: bytes,
    attestation_document: bytes,
    encryption_context: Mapping[str, str] | None = None,
) -> NitroRecipientCiphertext:
    """Ask KMS to re-encrypt a secret only to an attested Nitro recipient."""

    if not isinstance(key_id, str) or not key_id.strip():
        raise ValueError("KMS key_id is required")
    if not isinstance(encrypted_secret, bytes) or not encrypted_secret:
        raise ValueError("encrypted_secret must be non-empty bytes")
    if not isinstance(attestation_document, bytes) or not attestation_document:
        raise ValueError("attestation_document must be non-empty bytes")
    if encryption_context is not None and any(
        not isinstance(key, str) or not isinstance(value, str)
        for key, value in encryption_context.items()
    ):
        raise ValueError("encryption_context must contain only string pairs")
    request: dict[str, Any] = {
        "KeyId": key_id,
        "CiphertextBlob": encrypted_secret,
        "Recipient": {
            "KeyEncryptionAlgorithm": NITRO_RECIPIENT_ALGORITHM,
            "AttestationDocument": attestation_document,
        },
    }
    if encryption_context is not None:
        request["EncryptionContext"] = dict(encryption_context)
    response = kms_client.decrypt(**request)
    plaintext = response.get("Plaintext")
    if plaintext not in (None, b""):
        raise RuntimeError("KMS exposed plaintext instead of recipient ciphertext")
    ciphertext = response.get("CiphertextForRecipient")
    if not isinstance(ciphertext, bytes) or not ciphertext:
        raise RuntimeError("KMS returned no CiphertextForRecipient")
    return NitroRecipientCiphertext(
        key_id=key_id,
        attestation_document_sha256=sha256_bytes(attestation_document),
        ciphertext_for_recipient=ciphertext,
    )


def request_attested_data_key_ciphertext(
    kms_client: KmsRecipientClient,
    *,
    key_id: str,
    attestation_document: bytes,
    encryption_context: Mapping[str, str],
) -> NitroRecipientDataKey:
    """Generate an AES-256 key that KMS exposes only to the attested enclave."""

    if not isinstance(key_id, str) or not key_id.strip():
        raise ValueError("KMS key_id is required")
    if not isinstance(attestation_document, bytes) or not attestation_document:
        raise ValueError("attestation_document must be non-empty bytes")
    if not encryption_context or any(
        not isinstance(key, str)
        or not key
        or not isinstance(value, str)
        or not value
        for key, value in encryption_context.items()
    ):
        raise ValueError("encryption_context must contain non-empty string pairs")
    response = kms_client.generate_data_key(
        KeyId=key_id,
        KeySpec="AES_256",
        EncryptionContext=dict(encryption_context),
        Recipient={
            "KeyEncryptionAlgorithm": NITRO_RECIPIENT_ALGORITHM,
            "AttestationDocument": attestation_document,
        },
    )
    if response.get("Plaintext") not in (None, b""):
        raise RuntimeError("KMS exposed plaintext data key")
    encrypted_data_key = response.get("CiphertextBlob")
    recipient = response.get("CiphertextForRecipient")
    if not isinstance(encrypted_data_key, bytes) or not encrypted_data_key:
        raise RuntimeError("KMS returned no CiphertextBlob")
    if not isinstance(recipient, bytes) or not recipient:
        raise RuntimeError("KMS returned no CiphertextForRecipient")
    return NitroRecipientDataKey(
        key_id=key_id,
        attestation_document_sha256=sha256_bytes(attestation_document),
        encrypted_data_key=encrypted_data_key,
        ciphertext_for_recipient=recipient,
    )


class NitroKmsEnvelopeEncryptor:
    """AES-GCM snapshot encryption with an attestation-bound KMS data key."""

    provider_id = "aws-nitro-kms-aes256-gcm-v1"

    def __init__(
        self,
        *,
        kms_client: KmsRecipientClient,
        key_id: str,
        attestation_document: bytes,
        recipient_private_key: Any,
        encryption_context: Mapping[str, str],
    ) -> None:
        if not key_id.strip():
            raise ValueError("KMS key_id is required")
        self.kms_client = kms_client
        self.key_id = key_id
        self.attestation_document = attestation_document
        self.recipient_private_key = recipient_private_key
        self.encryption_context = dict(encryption_context)

    def encrypt(self, plaintext: bytes, *, associated_data: bytes) -> bytes:
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM

        if not isinstance(plaintext, bytes) or not plaintext:
            raise ValueError("Nitro envelope plaintext must be non-empty bytes")
        if not isinstance(associated_data, bytes) or not associated_data:
            raise ValueError("Nitro envelope associated data must be non-empty bytes")
        result = request_attested_data_key_ciphertext(
            self.kms_client,
            key_id=self.key_id,
            attestation_document=self.attestation_document,
            encryption_context=self.encryption_context,
        )
        data_key = decrypt_recipient_ciphertext(
            self.recipient_private_key, result.ciphertext_for_recipient,
        )
        if len(data_key) != 32:
            raise RuntimeError("KMS recipient data key is not AES-256")
        nonce = os.urandom(12)
        ciphertext = AESGCM(data_key).encrypt(nonce, plaintext, associated_data)
        return canonical_json_bytes({
            "schema": NITRO_CIPHERTEXT_BUNDLE_SCHEMA,
            "kmsKeyId": self.key_id,
            "encryptionContext": self.encryption_context,
            "encryptedDataKeyBase64": base64.b64encode(
                result.encrypted_data_key
            ).decode("ascii"),
            "nonceBase64": base64.b64encode(nonce).decode("ascii"),
            "ciphertextBase64": base64.b64encode(ciphertext).decode("ascii"),
        })


def decrypt_nitro_ciphertext_bundle(
    bundle: bytes,
    *,
    associated_data: bytes,
    kms_client: KmsRecipientClient,
    attestation_document: bytes,
    recipient_private_key: Any,
) -> bytes:
    """Authorized replay decryption; the data key still enters only an enclave."""

    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    try:
        value = json.loads(bundle.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("Nitro ciphertext bundle must be canonical JSON") from exc
    if canonical_json_bytes(value) != bundle:
        raise ValueError("Nitro ciphertext bundle must use canonical JSON bytes")
    if not isinstance(value, dict) or set(value) != {
        "schema",
        "kmsKeyId",
        "encryptionContext",
        "encryptedDataKeyBase64",
        "nonceBase64",
        "ciphertextBase64",
    }:
        raise ValueError("Nitro ciphertext bundle fields do not match")
    if value["schema"] != NITRO_CIPHERTEXT_BUNDLE_SCHEMA:
        raise ValueError("Nitro ciphertext bundle schema mismatch")
    context = value["encryptionContext"]
    if not isinstance(context, dict) or not context or any(
        not isinstance(key, str)
        or not key
        or not isinstance(item, str)
        or not item
        for key, item in context.items()
    ):
        raise ValueError("Nitro ciphertext encryption context is invalid")
    try:
        encrypted_data_key = base64.b64decode(
            value["encryptedDataKeyBase64"], validate=True
        )
        nonce = base64.b64decode(value["nonceBase64"], validate=True)
        ciphertext = base64.b64decode(value["ciphertextBase64"], validate=True)
    except (TypeError, ValueError) as exc:
        raise ValueError("Nitro ciphertext bundle has invalid base64") from exc
    if len(nonce) != 12 or not encrypted_data_key or not ciphertext:
        raise ValueError("Nitro ciphertext bundle has invalid binary fields")
    result = request_attested_secret_ciphertext(
        kms_client,
        key_id=value["kmsKeyId"],
        encrypted_secret=encrypted_data_key,
        attestation_document=attestation_document,
        encryption_context=context,
    )
    data_key = decrypt_recipient_ciphertext(
        recipient_private_key, result.ciphertext_for_recipient,
    )
    if len(data_key) != 32:
        raise RuntimeError("KMS recipient data key is not AES-256")
    return AESGCM(data_key).decrypt(nonce, ciphertext, associated_data)
