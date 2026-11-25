"""Strict AWS Nitro Enclaves attestation verification.

The verifier implements the documented CBOR/COSE_Sign1 and Nitro PKI boundary.
It requires a statement-specific nonce and canonical user data, so a valid
attestation document cannot be replayed for another YOLO statement.  The AWS
commercial-partition root certificate is supplied explicitly by the caller and
is never downloaded during verification.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Callable

from cryptography import x509
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, rsa
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature

from .canonical import canonical_json_bytes
from .tee_policy import NativeAttestationClaims


NITRO_USER_DATA_SCHEMA = "postfiat.yolo.nitro_user_data.v2"
NITRO_COSE_ALGORITHM_ES384 = -35
NITRO_COSE_SIGN1_TAG = 18
MAX_ATTESTATION_DOCUMENT_BYTES = 64 * 1024
MAX_ATTESTATION_PAYLOAD_BYTES = 16 * 1024
MAX_CERTIFICATE_BYTES = 1024
MAX_USER_DATA_BYTES = 1024


@dataclass(frozen=True)
class _CborTag:
    tag: int
    value: Any


class _CborReader:
    def __init__(self, data: bytes) -> None:
        if not isinstance(data, bytes):
            raise TypeError("CBOR input must be bytes")
        self.data = data
        self.offset = 0
        self.items = 0

    def decode(self) -> Any:
        value = self._item(depth=0)
        if self.offset != len(self.data):
            raise ValueError("CBOR input has trailing bytes")
        return value

    def _read(self, length: int) -> bytes:
        end = self.offset + length
        if length < 0 or end > len(self.data):
            raise ValueError("CBOR input is truncated")
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def _length(self, additional: int) -> int:
        if additional < 24:
            return additional
        widths = {24: 1, 25: 2, 26: 4, 27: 8}
        width = widths.get(additional)
        if width is None:
            raise ValueError("indefinite or reserved CBOR lengths are not accepted")
        encoded = self._read(width)
        value = int.from_bytes(encoded, "big")
        minimum = {1: 24, 2: 256, 4: 65_536, 8: 4_294_967_296}[width]
        if value < minimum:
            raise ValueError("CBOR integer or length is not minimally encoded")
        return value

    def _item(self, *, depth: int) -> Any:
        if depth > 32:
            raise ValueError("CBOR nesting exceeds the maximum")
        self.items += 1
        if self.items > 4096:
            raise ValueError("CBOR item count exceeds the maximum")
        initial = self._read(1)[0]
        major = initial >> 5
        additional = initial & 31
        indefinite = additional == 31 and major in (4, 5)
        value = None if indefinite else self._length(additional)
        if major == 0:
            return value
        if major == 1:
            return -1 - value
        if major == 2:
            return self._read(value)
        if major == 3:
            try:
                return self._read(value).decode("utf-8")
            except UnicodeDecodeError as exc:
                raise ValueError("CBOR text is not UTF-8") from exc
        if major == 4:
            output = []
            while indefinite or len(output) < value:
                if indefinite and self.data[self.offset:self.offset + 1] == b"\xff":
                    self.offset += 1
                    break
                output.append(self._item(depth=depth + 1))
            return output
        if major == 5:
            output: dict[Any, Any] = {}
            while indefinite or len(output) < value:
                if indefinite and self.data[self.offset:self.offset + 1] == b"\xff":
                    self.offset += 1
                    break
                key = self._item(depth=depth + 1)
                if isinstance(key, bool) or not isinstance(key, (int, str, bytes)):
                    raise ValueError("CBOR map key type is not accepted")
                if key in output:
                    raise ValueError("CBOR map contains a duplicate key")
                output[key] = self._item(depth=depth + 1)
            return output
        if major == 6:
            return _CborTag(value, self._item(depth=depth + 1))
        if major == 7:
            simple = {20: False, 21: True, 22: None}
            if additional in simple:
                return simple[additional]
            raise ValueError("CBOR simple or floating-point value is not accepted")
        raise ValueError("unsupported CBOR major type")


def _decode_cbor(data: bytes) -> Any:
    return _CborReader(data).decode()


def _encode_head(major: int, value: int) -> bytes:
    if value < 0:
        raise ValueError("CBOR head value must be non-negative")
    if value < 24:
        return bytes([(major << 5) | value])
    for marker, width, limit in (
        (24, 1, 1 << 8),
        (25, 2, 1 << 16),
        (26, 4, 1 << 32),
        (27, 8, 1 << 64),
    ):
        if value < limit:
            return bytes([(major << 5) | marker]) + value.to_bytes(width, "big")
    raise ValueError("CBOR integer exceeds uint64")


def _encode_cbor(value: Any) -> bytes:
    """Minimal deterministic encoder used for the COSE Sig_structure."""

    if isinstance(value, _CborTag):
        return _encode_head(6, value.tag) + _encode_cbor(value.value)
    if value is None:
        return b"\xf6"
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int):
        if value >= 0:
            return _encode_head(0, value)
        return _encode_head(1, -1 - value)
    if isinstance(value, bytes):
        return _encode_head(2, len(value)) + value
    if isinstance(value, str):
        encoded = value.encode("utf-8")
        return _encode_head(3, len(encoded)) + encoded
    if isinstance(value, (list, tuple)):
        return _encode_head(4, len(value)) + b"".join(
            _encode_cbor(item) for item in value
        )
    if isinstance(value, dict):
        encoded_items = [
            (_encode_cbor(key), _encode_cbor(item)) for key, item in value.items()
        ]
        encoded_items.sort(key=lambda pair: (len(pair[0]), pair[0]))
        return _encode_head(5, len(encoded_items)) + b"".join(
            key + item for key, item in encoded_items
        )
    raise TypeError(f"cannot CBOR-encode {type(value).__name__}")


def _utc(value: datetime) -> datetime:
    if value.tzinfo is None or value.utcoffset() is None:
        raise ValueError("verification time must be timezone-aware")
    return value.astimezone(timezone.utc)


def _certificate_time(certificate: x509.Certificate, field: str) -> datetime:
    current_name = field + "_utc"
    value = getattr(certificate, current_name, None)
    if value is None:
        value = getattr(certificate, field).replace(tzinfo=timezone.utc)
    return _utc(value)


def _verify_certificate_signature(
    certificate: x509.Certificate,
    issuer: x509.Certificate,
) -> None:
    key = issuer.public_key()
    if (
        not isinstance(key, ec.EllipticCurvePublicKey)
        or not isinstance(key.curve, ec.SECP384R1)
        or certificate.signature_algorithm_oid.dotted_string != "1.2.840.10045.4.3.3"
    ):
        raise ValueError("Nitro certificate signatures require P-384 and SHA-384")
    key.verify(certificate.signature, certificate.tbs_certificate_bytes, ec.ECDSA(hashes.SHA384()))


def _extension(certificate: x509.Certificate, extension_type: type[Any]) -> Any:
    try:
        return certificate.extensions.get_extension_for_class(extension_type).value
    except x509.ExtensionNotFound as exc:
        raise ValueError(
            f"Nitro certificate is missing {extension_type.__name__}"
        ) from exc


def _verify_certificate_chain(
    leaf_der: bytes,
    ca_bundle_der: list[bytes],
    *,
    root_certificate: x509.Certificate,
    verification_time: datetime,
) -> x509.Certificate:
    if not ca_bundle_der:
        raise ValueError("Nitro CA bundle is empty")
    root_der = root_certificate.public_bytes(serialization.Encoding.DER)
    if ca_bundle_der[0] != root_der:
        raise ValueError("Nitro CA bundle does not begin with the pinned AWS root")
    encoded_chain = [leaf_der, *reversed(ca_bundle_der)]
    if len(set(encoded_chain)) != len(encoded_chain):
        raise ValueError("Nitro certificate chain contains a duplicate")
    try:
        chain = [x509.load_der_x509_certificate(item) for item in encoded_chain]
    except ValueError as exc:
        raise ValueError("Nitro certificate is not valid DER") from exc
    if chain[-1].public_bytes(serialization.Encoding.DER) != root_der:
        raise ValueError("Nitro certificate chain has the wrong trust anchor")

    moment = _utc(verification_time)
    for certificate in chain:
        try:
            extensions = certificate.extensions
        except x509.DuplicateExtension as exc:
            raise ValueError("Nitro certificate has duplicate extensions") from exc
        for extension in extensions:
            if extension.critical and extension.oid.dotted_string not in {
                "2.5.29.19", "2.5.29.15", "2.5.29.14", "2.5.29.35",
            }:
                raise ValueError("Nitro certificate has an unsupported critical extension")
        if not (
            _certificate_time(certificate, "not_valid_before")
            <= moment
            <= _certificate_time(certificate, "not_valid_after")
        ):
            raise ValueError("Nitro certificate is outside its validity interval")

    leaf_constraints = _extension(chain[0], x509.BasicConstraints)
    leaf_usage = _extension(chain[0], x509.KeyUsage)
    if leaf_constraints.ca or leaf_constraints.path_length is not None:
        raise ValueError("Nitro leaf certificate has CA constraints")
    if not leaf_usage.digital_signature:
        raise ValueError("Nitro leaf certificate cannot sign attestations")

    for index, certificate in enumerate(chain[1:], start=1):
        constraints = _extension(certificate, x509.BasicConstraints)
        usage = _extension(certificate, x509.KeyUsage)
        if not constraints.ca or not usage.key_cert_sign:
            raise ValueError("Nitro CA certificate lacks signing authority")
        ca_certificates_below = index - 1
        if (
            constraints.path_length is not None
            and ca_certificates_below > constraints.path_length
        ):
            raise ValueError("Nitro certificate path-length constraint is exceeded")

    for child, issuer in zip(chain, chain[1:], strict=False):
        if child.issuer != issuer.subject:
            raise ValueError("Nitro certificate issuer chain does not match")
        try:
            _verify_certificate_signature(child, issuer)
        except InvalidSignature as exc:
            raise ValueError("Nitro certificate signature is invalid") from exc
    if chain[-1].issuer != chain[-1].subject:
        raise ValueError("pinned Nitro root is not self-issued")
    try:
        _verify_certificate_signature(chain[-1], chain[-1])
    except InvalidSignature as exc:
        raise ValueError("pinned Nitro root self-signature is invalid") from exc
    return chain[0]


def _lower_hex(value: Any, field: str, *, byte_length: int) -> str:
    if (
        not isinstance(value, str)
        or len(value) != byte_length * 2
        or value != value.lower()
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{field} must be {byte_length} bytes of lowercase hex")
    return value


@dataclass(frozen=True)
class NitroAttestationDocument:
    module_id: str
    timestamp_ms: int
    pcrs: dict[int, bytes]
    public_key: bytes
    statement_key: str
    statement_binding_digest: str
    nonce: bytes


class NitroNativeAttestationVerifier:
    """Validate an AWS Nitro document and return provider-neutral claims."""

    def __init__(
        self,
        *,
        root_certificate_pem: bytes,
        max_age_ms: int,
        max_future_skew_ms: int,
        clock: Callable[[], datetime] | None = None,
    ) -> None:
        if not isinstance(root_certificate_pem, bytes) or not root_certificate_pem:
            raise ValueError("pinned Nitro root certificate is required")
        if not isinstance(max_age_ms, int) or isinstance(max_age_ms, bool) or max_age_ms <= 0:
            raise ValueError("max_age_ms must be a positive integer")
        if (
            not isinstance(max_future_skew_ms, int)
            or isinstance(max_future_skew_ms, bool)
            or max_future_skew_ms < 0
        ):
            raise ValueError("max_future_skew_ms must be a non-negative integer")
        try:
            self.root_certificate = x509.load_pem_x509_certificate(
                root_certificate_pem
            )
        except ValueError as exc:
            raise ValueError("pinned Nitro root certificate is not valid PEM") from exc
        self.max_age_ms = max_age_ms
        self.max_future_skew_ms = max_future_skew_ms
        self.clock = clock or (lambda: datetime.now(timezone.utc))

    def verify_and_extract(
        self,
        document: bytes,
        *,
        attestation_binding_digest: str,
    ) -> NativeAttestationClaims:
        parsed = self.parse_and_verify(
            document, attestation_binding_digest=attestation_binding_digest,
        )
        pcr0 = parsed.pcrs.get(0)
        if pcr0 is None:
            raise ValueError("Nitro attestation does not contain PCR0")
        debug_mode = all(
            value and all(byte == 0 for byte in value)
            for value in parsed.pcrs.values()
        )
        return NativeAttestationClaims(
            platform="aws-nitro",
            hardware_backed=True,
            debug_mode=debug_mode,
            measurement="pcr0:" + pcr0.hex(),
            statement_key=parsed.statement_key,
        )

    def parse_and_verify(
        self,
        document: bytes,
        *,
        attestation_binding_digest: str,
    ) -> NitroAttestationDocument:
        attestation_binding_digest = _lower_hex(
            attestation_binding_digest, "attestation_binding_digest", byte_length=32,
        )
        if not isinstance(document, bytes) or not document:
            raise ValueError("Nitro attestation document is empty")
        if len(document) > MAX_ATTESTATION_DOCUMENT_BYTES:
            raise ValueError("Nitro attestation document exceeds the maximum")
        outer = _decode_cbor(document)
        if isinstance(outer, _CborTag):
            if outer.tag != NITRO_COSE_SIGN1_TAG:
                raise ValueError("Nitro attestation has the wrong COSE tag")
            outer = outer.value
        if not isinstance(outer, list) or len(outer) != 4:
            raise ValueError("Nitro attestation is not a COSE_Sign1 array")
        protected_bytes, unprotected, payload_bytes, signature = outer
        if not isinstance(protected_bytes, bytes) or not protected_bytes:
            raise ValueError("Nitro COSE protected header is missing")
        protected = _decode_cbor(protected_bytes)
        if not isinstance(protected, dict) or protected != {1: NITRO_COSE_ALGORITHM_ES384}:
            raise ValueError("Nitro COSE algorithm must be ES384")
        if not isinstance(unprotected, dict) or unprotected:
            raise ValueError("Nitro COSE unprotected header is invalid")
        if (
            not isinstance(payload_bytes, bytes)
            or not payload_bytes
            or len(payload_bytes) > MAX_ATTESTATION_PAYLOAD_BYTES
        ):
            raise ValueError("Nitro attestation payload size is invalid")
        if not isinstance(signature, bytes) or len(signature) != 96:
            raise ValueError("Nitro ES384 signature must be 96 bytes")
        payload = _decode_cbor(payload_bytes)
        if not isinstance(payload, dict):
            raise ValueError("Nitro attestation payload must be a map")
        allowed = {
            "module_id",
            "digest",
            "timestamp",
            "pcrs",
            "certificate",
            "cabundle",
            "public_key",
            "user_data",
            "nonce",
        }
        required = allowed
        if set(payload) != required:
            unknown = set(payload) - allowed
            missing = required - set(payload)
            detail = sorted(unknown or missing)[0]
            raise ValueError(f"Nitro attestation payload field mismatch: {detail}")
        if not isinstance(payload["module_id"], str) or not 1 <= len(payload["module_id"].encode("utf-8")) <= 256:
            raise ValueError("Nitro module_id is required")
        if payload["digest"] != "SHA384":
            raise ValueError("Nitro attestation digest must be SHA384")
        timestamp = payload["timestamp"]
        if not isinstance(timestamp, int) or isinstance(timestamp, bool) or timestamp <= 0:
            raise ValueError("Nitro attestation timestamp is invalid")
        now = _utc(self.clock())
        now_ms = int(now.timestamp() * 1000)
        if timestamp > now_ms + self.max_future_skew_ms:
            raise ValueError("Nitro attestation timestamp is in the future")
        if now_ms - timestamp > self.max_age_ms:
            raise ValueError("Nitro attestation timestamp is stale")

        pcrs = payload["pcrs"]
        if not isinstance(pcrs, dict) or not 1 <= len(pcrs) <= 32:
            raise ValueError("Nitro PCR map size is invalid")
        for index, value in pcrs.items():
            if (
                not isinstance(index, int)
                or isinstance(index, bool)
                or not 0 <= index < 32
                or not isinstance(value, bytes)
                or len(value) != 48
            ):
                raise ValueError("Nitro PCR entry is invalid")

        certificate = payload["certificate"]
        bundle = payload["cabundle"]
        if (
            not isinstance(certificate, bytes)
            or not 1 <= len(certificate) <= MAX_CERTIFICATE_BYTES
            or not isinstance(bundle, list)
            or not 1 <= len(bundle) <= 16
            or any(
                not isinstance(item, bytes)
                or not 1 <= len(item) <= MAX_CERTIFICATE_BYTES
                for item in bundle
            )
        ):
            raise ValueError("Nitro certificate material is invalid")
        leaf = _verify_certificate_chain(
            certificate,
            bundle,
            root_certificate=self.root_certificate,
            verification_time=now,
        )

        sig_structure = _encode_cbor(
            ["Signature1", protected_bytes, b"", payload_bytes]
        )
        key = leaf.public_key()
        if not isinstance(key, ec.EllipticCurvePublicKey) or not isinstance(
            key.curve, ec.SECP384R1
        ):
            raise ValueError("Nitro attestation signer must use P-384")
        r = int.from_bytes(signature[:48], "big")
        s = int.from_bytes(signature[48:], "big")
        try:
            key.verify(
                encode_dss_signature(r, s),
                sig_structure,
                ec.ECDSA(hashes.SHA384()),
            )
        except InvalidSignature as exc:
            raise ValueError("Nitro COSE signature is invalid") from exc

        public_key = payload["public_key"]
        if not isinstance(public_key, bytes) or not 1 <= len(public_key) <= MAX_USER_DATA_BYTES:
            raise ValueError("Nitro recipient public key is invalid")
        try:
            recipient_key = serialization.load_der_public_key(public_key)
        except ValueError as exc:
            raise ValueError("Nitro recipient public key is not DER") from exc
        if not isinstance(recipient_key, rsa.RSAPublicKey) or recipient_key.key_size != 2048:
            raise ValueError("Nitro recipient public key must be RSA-2048")

        user_data = payload["user_data"]
        if not isinstance(user_data, bytes) or len(user_data) > MAX_USER_DATA_BYTES:
            raise ValueError("Nitro user data is invalid")
        try:
            user_value = json.loads(user_data.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("Nitro user data must be canonical JSON") from exc
        if canonical_json_bytes(user_value) != user_data:
            raise ValueError("Nitro user data must use canonical JSON bytes")
        if not isinstance(user_value, dict) or set(user_value) != {
            "schema",
            "statementKey",
            "statementBindingDigest",
        }:
            raise ValueError("Nitro user data schema fields do not match")
        if user_value["schema"] != NITRO_USER_DATA_SCHEMA:
            raise ValueError("Nitro user data schema mismatch")
        statement_key = _lower_hex(
            user_value["statementKey"], "statementKey", byte_length=32
        )
        if user_value["statementBindingDigest"] != attestation_binding_digest:
            raise ValueError("Nitro user data is bound to another statement")
        nonce = payload["nonce"]
        if not isinstance(nonce, bytes) or nonce != bytes.fromhex(attestation_binding_digest):
            raise ValueError("Nitro nonce is not the statement attestation binding digest")
        return NitroAttestationDocument(
            module_id=payload["module_id"],
            timestamp_ms=timestamp,
            pcrs=dict(pcrs),
            public_key=public_key,
            statement_key=statement_key,
            statement_binding_digest=attestation_binding_digest,
            nonce=nonce,
        )
