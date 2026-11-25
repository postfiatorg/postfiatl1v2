from __future__ import annotations

import base64

import pytest
from test_yolo_nitro_cms import recipient_envelope

from pftl_schwab_tee import (
    Ed25519HexSignatureVerifier,
    NitroKmsEnvelopeEncryptor,
    NitroStatementKeys,
    decrypt_nitro_ciphertext_bundle,
    nitro_user_data,
)


SIGNING_DIGEST = "12" * 32


class Nsm:
    def attest(self, *, user_data, nonce, public_key):
        self.user_data = user_data
        self.nonce = nonce
        self.public_key = public_key
        return b"cose-document"


class Kms:
    def __init__(self, recipient_public_key) -> None:
        self.recipient_public_key = recipient_public_key
        self.data_key = b"d" * 32
        self.generated = None
        self.decrypted = None

    def _for_recipient(self) -> bytes:
        return recipient_envelope(self.recipient_public_key, self.data_key)

    def generate_data_key(self, **kwargs):
        self.generated = kwargs
        return {
            "CiphertextBlob": b"kms-wrapped-data-key",
            "CiphertextForRecipient": self._for_recipient(),
        }

    def decrypt(self, **kwargs):
        self.decrypted = kwargs
        assert kwargs["CiphertextBlob"] == b"kms-wrapped-data-key"
        return {"CiphertextForRecipient": self._for_recipient()}


def test_ephemeral_keys_bind_nsm_attestation_and_statement_signature() -> None:
    keys = NitroStatementKeys.generate()
    nsm = Nsm()

    document = keys.attest_statement(nsm, attestation_binding_digest=SIGNING_DIGEST)
    signature = keys.sign_digest(SIGNING_DIGEST)

    assert document == b"cose-document"
    assert nsm.nonce == bytes.fromhex(SIGNING_DIGEST)
    assert nsm.user_data == nitro_user_data(
        statement_key=keys.statement_key,
        attestation_binding_digest=SIGNING_DIGEST,
    )
    assert Ed25519HexSignatureVerifier().verify(
        statement_key=keys.statement_key,
        signing_digest=SIGNING_DIGEST,
        signature=signature,
    )
    assert not Ed25519HexSignatureVerifier().verify(
        statement_key=keys.statement_key,
        signing_digest="13" * 32,
        signature=signature,
    )


def test_kms_envelope_round_trip_never_exposes_plaintext_key() -> None:
    keys = NitroStatementKeys.generate()
    kms = Kms(keys.recipient_private_key.public_key())
    context = {
        "purpose": "yolo-private-snapshot",
        "epochSha256": "34" * 32,
    }
    encryptor = NitroKmsEnvelopeEncryptor(
        kms_client=kms,
        key_id="arn:aws:kms:us-east-1:111122223333:key/test",
        attestation_document=b"cose-document",
        recipient_private_key=keys.recipient_private_key,
        encryption_context=context,
    )

    plaintext = b'{"private":"chain"}'
    associated_data = b"bound-snapshot-receipt"
    bundle = encryptor.encrypt(plaintext, associated_data=associated_data)
    recovered = decrypt_nitro_ciphertext_bundle(
        bundle,
        associated_data=associated_data,
        kms_client=kms,
        attestation_document=b"fresh-cose-document",
        recipient_private_key=keys.recipient_private_key,
    )

    assert recovered == plaintext
    assert plaintext not in bundle
    assert kms.generated["KeySpec"] == "AES_256"
    assert kms.generated["Recipient"]["AttestationDocument"] == b"cose-document"
    assert kms.decrypted["Recipient"]["AttestationDocument"] == b"fresh-cose-document"
    assert kms.generated["EncryptionContext"] == context
    with pytest.raises(Exception):
        decrypt_nitro_ciphertext_bundle(
            bundle,
            associated_data=b"different-associated-data",
            kms_client=kms,
            attestation_document=b"fresh-cose-document",
            recipient_private_key=keys.recipient_private_key,
        )


def test_kms_data_key_plaintext_response_is_rejected() -> None:
    keys = NitroStatementKeys.generate()

    class PlaintextKms(Kms):
        def generate_data_key(self, **kwargs):
            return {
                "Plaintext": b"d" * 32,
                "CiphertextBlob": b"wrapped",
                "CiphertextForRecipient": self._for_recipient(),
            }

    encryptor = NitroKmsEnvelopeEncryptor(
        kms_client=PlaintextKms(keys.recipient_private_key.public_key()),
        key_id="key",
        attestation_document=b"cose-document",
        recipient_private_key=keys.recipient_private_key,
        encryption_context={"purpose": "test"},
    )
    with pytest.raises(RuntimeError, match="plaintext data key"):
        encryptor.encrypt(b"private", associated_data=b"receipt")
