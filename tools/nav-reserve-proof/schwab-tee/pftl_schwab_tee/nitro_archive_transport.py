"""Bounded ciphertext-only archive transfer and durable parent acknowledgement."""

from __future__ import annotations

import json
import socket
from pathlib import Path
from typing import Callable

from .canonical import canonical_json_bytes, sha256_bytes
from .private_archive import CiphertextArchive, CiphertextEnvelope, CIPHERTEXT_ENVELOPE_SCHEMA


ARCHIVE_REQUEST_SCHEMA = "postfiat.yolo.nitro_archive_request.v1"
ARCHIVE_ACK_SCHEMA = "postfiat.yolo.nitro_archive_ack.v1"
MAX_METADATA_BYTES = 16 * 1024
MAX_CIPHERTEXT_BYTES = 256 * 1024 * 1024


def _receive(stream: socket.socket, length: int) -> bytes:
    chunks = bytearray()
    while len(chunks) < length:
        data = stream.recv(min(length - len(chunks), 64 * 1024))
        if not data:
            raise ValueError("archive frame ended before its declared length")
        chunks.extend(data)
    return bytes(chunks)


def _read_frame(stream: socket.socket, maximum: int) -> bytes:
    length = int.from_bytes(_receive(stream, 4), "big")
    if not 0 < length <= maximum:
        raise ValueError("archive frame exceeds its bounds")
    return _receive(stream, length)


def _write_frame(stream: socket.socket, value: bytes, maximum: int) -> None:
    if not isinstance(value, bytes) or not 0 < len(value) <= maximum:
        raise ValueError("archive frame exceeds its bounds")
    stream.sendall(len(value).to_bytes(4, "big"))
    stream.sendall(value)


def _object(encoded: bytes) -> dict:
    try:
        value = json.loads(encoded)
        if not isinstance(value, dict) or canonical_json_bytes(value) != encoded:
            raise ValueError()
    except (ValueError, UnicodeDecodeError, RecursionError):
        raise ValueError("archive metadata must be a canonical JSON object") from None
    return value


def _text(value: object) -> bool:
    return isinstance(value, str) and 0 < len(value.encode("utf-8")) <= 256 and "\0" not in value


def _digest(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value)


def _validate_request(request: dict) -> dict:
    if (set(request) != {"schema", "runId", "artifactId", "envelope", "ciphertextBytes"}
            or request["schema"] != ARCHIVE_REQUEST_SCHEMA
            or not _text(request["runId"]) or not _text(request["artifactId"])
            or type(request["ciphertextBytes"]) is not int
            or not 0 < request["ciphertextBytes"] <= MAX_CIPHERTEXT_BYTES):
        raise ValueError("invalid archive request")
    envelope = request["envelope"]
    if (not isinstance(envelope, dict) or set(envelope) != {
        "schema", "providerId", "keyId", "associatedDataSha256",
        "plaintextSha256", "ciphertextSha256",
    } or envelope["schema"] != CIPHERTEXT_ENVELOPE_SCHEMA
            or not _text(envelope["providerId"])
            or not isinstance(envelope["keyId"], str) or not 0 < len(envelope["keyId"]) <= 2048
            or any(not _digest(envelope[key]) for key in (
                "associatedDataSha256", "plaintextSha256", "ciphertextSha256"))):
        raise ValueError("invalid archive ciphertext envelope")
    return envelope


def _ack(request: dict) -> dict:
    return {
        "schema": ARCHIVE_ACK_SCHEMA, "runId": request["runId"],
        "requestSha256": sha256_bytes(canonical_json_bytes(request)),
        "ciphertextSha256": request["envelope"]["ciphertextSha256"],
        "ciphertextBytes": request["ciphertextBytes"],
    }


class NitroCiphertextArchiveClient:
    """Send one encrypted artifact; return only after a matching acknowledgement.

    The supplied socket factory must use the dedicated parent archive vsock
    port and an explicit timeout. The acknowledgement is a storage receipt,
    not cryptographic proof that an untrusted parent will retain the artifact.
    """

    def __init__(self, socket_factory: Callable[[], socket.socket]) -> None:
        self.socket_factory = socket_factory

    def store(self, *, run_id: str, artifact_id: str, envelope: CiphertextEnvelope) -> dict:
        request = {
            "schema": ARCHIVE_REQUEST_SCHEMA, "runId": run_id, "artifactId": artifact_id,
            "envelope": envelope.public_dict(), "ciphertextBytes": len(envelope.ciphertext),
        }
        _validate_request(request)
        if sha256_bytes(envelope.ciphertext) != envelope.ciphertext_sha256:
            raise ValueError("archive ciphertext does not match its commitment")
        stream = self.socket_factory()
        try:
            _write_frame(stream, canonical_json_bytes(request), MAX_METADATA_BYTES)
            _write_frame(stream, envelope.ciphertext, MAX_CIPHERTEXT_BYTES)
            acknowledgement = _object(_read_frame(stream, MAX_METADATA_BYTES))
            if acknowledgement != _ack(request):
                raise ValueError("archive acknowledgement does not match the submitted artifact")
            return acknowledgement
        finally:
            stream.close()


def receive_ciphertext_archive(stream: socket.socket, archive: CiphertextArchive) -> Path:
    """Serve one dedicated parent archive connection without caller-supplied paths.

    Write/fsync must finish before acknowledgement. A transfer or storage error
    produces no success frame, and callers must close the connection on error.
    """

    request = _object(_read_frame(stream, MAX_METADATA_BYTES))
    metadata = _validate_request(request)
    ciphertext = _read_frame(stream, min(request["ciphertextBytes"], MAX_CIPHERTEXT_BYTES))
    if (len(ciphertext) != request["ciphertextBytes"]
            or sha256_bytes(ciphertext) != metadata["ciphertextSha256"]):
        raise ValueError("received archive ciphertext does not match its commitment")
    envelope = CiphertextEnvelope(
        provider_id=metadata["providerId"], key_id=metadata["keyId"],
        associated_data_sha256=metadata["associatedDataSha256"],
        plaintext_sha256=metadata["plaintextSha256"],
        ciphertext_sha256=metadata["ciphertextSha256"], ciphertext=ciphertext,
    )
    # A flat content-addressed filename avoids path input and directory-creation
    # durability races. Different metadata creates a different archive identity.
    name = sha256_bytes(canonical_json_bytes(request)) + ".ciphertext"
    path = archive.write_once(name, envelope)
    _write_frame(stream, canonical_json_bytes(_ack(request)), MAX_METADATA_BYTES)
    return path
