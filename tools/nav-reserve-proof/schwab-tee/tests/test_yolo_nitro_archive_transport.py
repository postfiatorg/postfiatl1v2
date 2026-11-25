"""Exercise archive framing and fsync-before-ack using synthetic ciphertext."""

import json
import socket
import threading

import pytest

from pftl_schwab_tee.canonical import canonical_json_bytes, sha256_bytes
from pftl_schwab_tee.nitro_archive_transport import (
    ARCHIVE_REQUEST_SCHEMA, MAX_METADATA_BYTES, MAX_CIPHERTEXT_BYTES,
    NitroCiphertextArchiveClient, receive_ciphertext_archive, _read_frame,
)
from pftl_schwab_tee.private_archive import CiphertextArchive, CiphertextEnvelope


def envelope():
    ciphertext = b"synthetic-encrypted-archive" * 5000
    return CiphertextEnvelope("synthetic-encryptor", "synthetic-key", "aa" * 32,
                              "bb" * 32, sha256_bytes(ciphertext), ciphertext)


def test_archive_round_trip_waits_for_file_and_directory_fsync(tmp_path, monkeypatch):
    import pftl_schwab_tee.private_archive as module
    calls = []
    real_fsync = module.os.fsync
    monkeypatch.setattr(module.os, "fsync", lambda fd: (calls.append(fd), real_fsync(fd))[1])
    left, right = socket.socketpair()
    left.settimeout(2); right.settimeout(2)
    errors, paths = [], []
    def receive():
        try:
            paths.append(receive_ciphertext_archive(right, CiphertextArchive(tmp_path)))
        except Exception as error:
            errors.append(error)
        finally:
            right.close()
    worker = threading.Thread(target=receive)
    worker.start()
    ack = NitroCiphertextArchiveClient(lambda: left).store(
        run_id="synthetic-run", artifact_id="snapshot/../../escape", envelope=envelope())
    worker.join(2)
    assert not worker.is_alive() and not errors
    assert len(calls) == 2
    assert len(paths) == 1 and paths[0].parent == tmp_path
    assert paths[0].read_bytes() == envelope().ciphertext
    assert paths[0].stat().st_mode & 0o777 == 0o600
    assert ack["ciphertextSha256"] == envelope().ciphertext_sha256


def test_new_archive_directories_are_persisted_before_acknowledgement(tmp_path, monkeypatch):
    synced = []
    real_sync = CiphertextArchive._sync_directory
    def sync(path):
        synced.append(path)
        real_sync(path)
    monkeypatch.setattr(CiphertextArchive, "_sync_directory", staticmethod(sync))
    root = tmp_path / "new" / "archive"
    archive = CiphertextArchive(root)
    assert synced == [tmp_path, tmp_path / "new"]
    assert (tmp_path / "new").stat().st_mode & 0o777 == 0o700
    assert root.stat().st_mode & 0o777 == 0o700
    archive.write_once("snapshot/0.ciphertext", envelope())
    assert synced[-2:] == [root, root / "snapshot"]


class Stream:
    def __init__(self, incoming=b""):
        self.incoming = incoming
        self.sent = bytearray()
        self.closed = False
        self.reads = []
    def recv(self, count):
        self.reads.append(count)
        chunk, self.incoming = self.incoming[:count], self.incoming[count:]
        return chunk
    def sendall(self, value): self.sent.extend(value)
    def close(self): self.closed = True


def frame(value): return len(value).to_bytes(4, "big") + value


def request():
    value = envelope()
    return {"schema": ARCHIVE_REQUEST_SCHEMA, "runId": "synthetic-run",
            "artifactId": "snapshot-0", "envelope": value.public_dict(),
            "ciphertextBytes": len(value.ciphertext)}


@pytest.mark.parametrize("length", [0, MAX_METADATA_BYTES + 1, 2**32 - 1])
def test_frame_rejects_bad_length_before_body_allocation(length):
    stream = Stream(length.to_bytes(4, "big"))
    with pytest.raises(ValueError, match="bounds"):
        _read_frame(stream, MAX_METADATA_BYTES)
    assert stream.reads == [4]


@pytest.mark.parametrize("response", [b'{}', b'{"schema":"wrong"}', b'not-json'])
def test_client_rejects_substituted_or_invalid_ack(response):
    stream = Stream(frame(response))
    with pytest.raises(ValueError):
        NitroCiphertextArchiveClient(lambda: stream).store(
            run_id="synthetic-run", artifact_id="snapshot-0", envelope=envelope())
    assert stream.closed


@pytest.mark.parametrize("change", [
    {"schema": "wrong"}, {"runId": ""}, {"ciphertextBytes": True},
    {"ciphertextBytes": MAX_CIPHERTEXT_BYTES + 1}, {"path": "../../escape"},
])
def test_parent_rejects_bad_metadata_without_body_read_or_ack(tmp_path, change):
    value = request(); value.update(change)
    metadata = canonical_json_bytes(value)
    stream = Stream(frame(metadata))
    with pytest.raises(ValueError):
        receive_ciphertext_archive(stream, CiphertextArchive(tmp_path))
    assert stream.sent == b"" and not list(tmp_path.iterdir())


@pytest.mark.parametrize("kind", ["hash", "short", "eof", "duplicate-json"])
def test_parent_rejects_corrupted_transfer_without_artifact_or_ack(tmp_path, kind):
    value = request()
    ciphertext = envelope().ciphertext
    if kind == "hash": ciphertext = bytes([ciphertext[0] ^ 1]) + ciphertext[1:]
    if kind == "short": ciphertext = ciphertext[:-1]
    metadata = canonical_json_bytes(value)
    if kind == "duplicate-json": metadata = b'{"schema":"a","schema":"b"}'
    wire = frame(metadata) + frame(ciphertext)
    if kind == "eof": wire = wire[:-1]
    stream = Stream(wire)
    with pytest.raises(ValueError):
        receive_ciphertext_archive(stream, CiphertextArchive(tmp_path))
    assert stream.sent == b"" and not list(tmp_path.iterdir())


def test_parent_storage_failure_and_duplicate_have_no_success_ack(tmp_path):
    class BrokenArchive:
        def write_once(self, *args): raise OSError("synthetic storage failure")
    wire = frame(canonical_json_bytes(request())) + frame(envelope().ciphertext)
    failed = Stream(wire)
    with pytest.raises(OSError):
        receive_ciphertext_archive(failed, BrokenArchive())
    assert not failed.sent
    archive = CiphertextArchive(tmp_path)
    receive_ciphertext_archive(Stream(wire), archive)
    duplicate = Stream(wire)
    with pytest.raises(FileExistsError):
        receive_ciphertext_archive(duplicate, archive)
    assert not duplicate.sent and len(list(tmp_path.iterdir())) == 1


def test_recipient_kms_wire_to_durable_archive_and_authorized_replay(tmp_path):
    import base64
    from test_yolo_nitro_cms import recipient_envelope
    from test_yolo_nitro_kms_transport import Transport, KEY, client
    from pftl_schwab_tee.nitro_kms import (
        NitroKmsEnvelopeEncryptor, decrypt_nitro_ciphertext_bundle,
    )
    from pftl_schwab_tee.nitro_runtime import NitroStatementKeys
    from pftl_schwab_tee.private_archive import encrypt_snapshot

    keys = NitroStatementKeys.generate()
    transport = Transport()
    transport.content = json.dumps({
        "KeyId": KEY, "EncryptionAlgorithm": "SYMMETRIC_DEFAULT",
        "CiphertextBlob": base64.b64encode(b"wrapped-key").decode(),
        "CiphertextForRecipient": base64.b64encode(recipient_envelope(
            keys.recipient_private_key.public_key(), b"k" * 32)).decode(),
    }).encode()
    kms = client(transport)
    plaintext, aad = b'{"synthetic-private-snapshot":true}', b"synthetic-associated-data"
    encrypted = encrypt_snapshot(plaintext, associated_data=aad, encryptor=NitroKmsEnvelopeEncryptor(
        kms_client=kms, key_id=KEY, attestation_document=b"synthetic-NSM-document",
        recipient_private_key=keys.recipient_private_key,
        encryption_context={"runId": "synthetic-integration"},
    ))
    metadata = request()
    metadata.update(envelope=encrypted.public_dict(), ciphertextBytes=len(encrypted.ciphertext))
    stream = Stream(frame(canonical_json_bytes(metadata)) + frame(encrypted.ciphertext))
    path = receive_ciphertext_archive(stream, CiphertextArchive(tmp_path))
    assert plaintext not in path.read_bytes() and plaintext not in stream.sent
    replayed = decrypt_nitro_ciphertext_bundle(
        path.read_bytes(), associated_data=aad, kms_client=kms,
        attestation_document=b"fresh-synthetic-NSM-document",
        recipient_private_key=keys.recipient_private_key,
    )
    assert replayed == plaintext
    assert [call[2]["headers"]["x-amz-target"] for call in transport.requests] == [
        "TrentService.GenerateDataKey", "TrentService.Decrypt",
    ]
