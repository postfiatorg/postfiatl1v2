"""Enclave-local key and Nitro Secure Module operations."""

from __future__ import annotations

import base64
import ctypes
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519, rsa
from cryptography.exceptions import InvalidSignature

from .canonical import canonical_json_bytes
from .nitro_attestation import NITRO_USER_DATA_SCHEMA
from .nitro_cms import decrypt_recipient_ciphertext


MAX_NSM_ATTESTATION_BYTES = 16 * 1024
MAX_NSM_USER_DATA_BYTES = 512
MAX_NSM_NONCE_BYTES = 512
MAX_NSM_PUBLIC_KEY_BYTES = 1024


def _digest(value: str, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{field} must be a lowercase SHA-256 digest")
    return value


def nitro_user_data(*, statement_key: str, attestation_binding_digest: str) -> bytes:
    _digest(statement_key, "statement_key")
    _digest(attestation_binding_digest, "attestation_binding_digest")
    value = canonical_json_bytes({
        "schema": NITRO_USER_DATA_SCHEMA,
        "statementKey": statement_key,
        "statementBindingDigest": attestation_binding_digest,
    })
    if len(value) > MAX_NSM_USER_DATA_BYTES:
        raise ValueError("Nitro user data exceeds the NSM limit")
    return value


class NsmAttestationSource(Protocol):
    def attest(
        self,
        *,
        user_data: bytes,
        nonce: bytes,
        public_key: bytes,
    ) -> bytes: ...


class NitroSecureModule:
    """Small ctypes binding to AWS's enclave-provided libnsm."""

    def __init__(self, library_path: Path | str = "/usr/lib64/libnsm.so") -> None:
        self.library = ctypes.CDLL(str(library_path))
        self.library.nsm_lib_init.argtypes = []
        self.library.nsm_lib_init.restype = ctypes.c_int
        self.library.nsm_lib_exit.argtypes = [ctypes.c_int]
        self.library.nsm_lib_exit.restype = None
        self.library.nsm_get_attestation_doc.argtypes = [
            ctypes.c_int,
            ctypes.c_void_p,
            ctypes.c_uint32,
            ctypes.c_void_p,
            ctypes.c_uint32,
            ctypes.c_void_p,
            ctypes.c_uint32,
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_uint32),
        ]
        self.library.nsm_get_attestation_doc.restype = ctypes.c_int

    @staticmethod
    def _input(value: bytes, *, maximum: int, field: str) -> Any:
        if not isinstance(value, bytes) or not value or len(value) > maximum:
            raise ValueError(f"{field} size is invalid")
        return ctypes.create_string_buffer(value, len(value))

    def attest(
        self,
        *,
        user_data: bytes,
        nonce: bytes,
        public_key: bytes,
    ) -> bytes:
        user = self._input(
            user_data,
            maximum=MAX_NSM_USER_DATA_BYTES,
            field="NSM user_data",
        )
        challenge = self._input(
            nonce,
            maximum=MAX_NSM_NONCE_BYTES,
            field="NSM nonce",
        )
        key = self._input(
            public_key,
            maximum=MAX_NSM_PUBLIC_KEY_BYTES,
            field="NSM public_key",
        )
        output = ctypes.create_string_buffer(MAX_NSM_ATTESTATION_BYTES)
        output_length = ctypes.c_uint32(MAX_NSM_ATTESTATION_BYTES)
        descriptor = self.library.nsm_lib_init()
        if descriptor < 0:
            raise RuntimeError("NSM initialization failed")
        try:
            status = self.library.nsm_get_attestation_doc(
                descriptor,
                ctypes.cast(user, ctypes.c_void_p),
                len(user_data),
                ctypes.cast(challenge, ctypes.c_void_p),
                len(nonce),
                ctypes.cast(key, ctypes.c_void_p),
                len(public_key),
                ctypes.cast(output, ctypes.c_void_p),
                ctypes.byref(output_length),
            )
        finally:
            self.library.nsm_lib_exit(descriptor)
        if status != 0:
            raise RuntimeError(f"NSM attestation request failed with status {status}")
        if not 0 < output_length.value <= MAX_NSM_ATTESTATION_BYTES:
            raise RuntimeError("NSM returned an invalid attestation length")
        return bytes(output.raw[: output_length.value])


@dataclass(frozen=True)
class NitroStatementKeys:
    """Ephemeral signing and KMS-recipient keys generated inside an enclave."""

    statement_private_key: ed25519.Ed25519PrivateKey
    recipient_private_key: rsa.RSAPrivateKey

    @classmethod
    def generate(cls) -> "NitroStatementKeys":
        return cls(
            statement_private_key=ed25519.Ed25519PrivateKey.generate(),
            recipient_private_key=rsa.generate_private_key(
                public_exponent=65537,
                key_size=2048,
            ),
        )

    @property
    def statement_key(self) -> str:
        return self.statement_private_key.public_key().public_bytes_raw().hex()

    @property
    def recipient_public_key_der(self) -> bytes:
        return self.recipient_private_key.public_key().public_bytes(
            serialization.Encoding.DER,
            serialization.PublicFormat.SubjectPublicKeyInfo,
        )

    def sign_digest(self, signing_digest: str) -> str:
        _digest(signing_digest, "signing_digest")
        return base64.b64encode(
            self.statement_private_key.sign(bytes.fromhex(signing_digest))
        ).decode("ascii")

    def attest_statement(
        self,
        source: NsmAttestationSource,
        *,
        attestation_binding_digest: str,
    ) -> bytes:
        _digest(attestation_binding_digest, "attestation_binding_digest")
        return source.attest(
            user_data=nitro_user_data(
                statement_key=self.statement_key,
                attestation_binding_digest=attestation_binding_digest,
            ),
            nonce=bytes.fromhex(attestation_binding_digest),
            public_key=self.recipient_public_key_der,
        )


class Ed25519HexSignatureVerifier:
    """Verify base64 Ed25519 signatures over raw SHA-256 digest bytes."""

    def verify(
        self,
        *,
        statement_key: str,
        signing_digest: str,
        signature: str,
    ) -> bool:
        try:
            _digest(statement_key, "statement_key")
            _digest(signing_digest, "signing_digest")
            encoded_signature = base64.b64decode(signature, validate=True)
            if len(encoded_signature) != 64:
                return False
            ed25519.Ed25519PublicKey.from_public_bytes(
                bytes.fromhex(statement_key)
            ).verify(encoded_signature, bytes.fromhex(signing_digest))
        except (TypeError, ValueError):
            return False
        except InvalidSignature:
            return False
        return True
