"""Ciphertext-only archive boundary for private option-chain snapshots."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol, TYPE_CHECKING

from .canonical import canonical_json_bytes, sha256_bytes
from .models import PrivateOptionChainSnapshot, SnapshotCommitment
if TYPE_CHECKING:
    from .reserve import PrivateBrokerReserveSnapshot, YoloBrokerReserveDisclosure


CIPHERTEXT_ENVELOPE_SCHEMA = "postfiat.yolo.ciphertext_envelope.v1"
ARCHIVE_ASSOCIATED_DATA_SCHEMA = "postfiat.yolo.snapshot_archive_aad.v1"
ARCHIVED_SNAPSHOT_RECEIPT_SCHEMA = "postfiat.yolo.archived_snapshot_receipt.v1"
BROKER_RESERVE_ARCHIVE_AAD_SCHEMA = "postfiat.yolo.broker_reserve_archive_aad.v1"
ARCHIVED_BROKER_RESERVE_RECEIPT_SCHEMA = (
    "postfiat.yolo.archived_broker_reserve_receipt.v1"
)


class SnapshotEncryptor(Protocol):
    """Implemented by attestation-gated KMS or an enclave-local encryptor."""

    provider_id: str
    key_id: str

    def encrypt(self, plaintext: bytes, *, associated_data: bytes) -> bytes: ...


@dataclass(frozen=True)
class CiphertextEnvelope:
    provider_id: str
    key_id: str
    associated_data_sha256: str
    plaintext_sha256: str
    ciphertext_sha256: str
    ciphertext: bytes

    def public_dict(self) -> dict[str, Any]:
        return {
            "schema": CIPHERTEXT_ENVELOPE_SCHEMA,
            "providerId": self.provider_id,
            "keyId": self.key_id,
            "associatedDataSha256": self.associated_data_sha256,
            "plaintextSha256": self.plaintext_sha256,
            "ciphertextSha256": self.ciphertext_sha256,
        }


def encrypt_snapshot(
    plaintext: bytes,
    *,
    associated_data: bytes,
    encryptor: SnapshotEncryptor,
) -> CiphertextEnvelope:
    ciphertext = encryptor.encrypt(plaintext, associated_data=associated_data)
    if not isinstance(ciphertext, bytes) or not ciphertext:
        raise ValueError("snapshot encryptor returned no ciphertext")
    if ciphertext == plaintext:
        raise ValueError("snapshot encryptor returned plaintext unchanged")
    return CiphertextEnvelope(
        provider_id=encryptor.provider_id,
        key_id=encryptor.key_id,
        associated_data_sha256=sha256_bytes(associated_data),
        plaintext_sha256=sha256_bytes(plaintext),
        ciphertext_sha256=sha256_bytes(ciphertext),
        ciphertext=ciphertext,
    )


class CiphertextArchive:
    """Write only encrypted bytes with create-once, owner-only semantics."""

    def __init__(self, root: Path | str) -> None:
        self.root = Path(root)
        self._create_directory(self.root)
        os.chmod(self.root, 0o700)

    @staticmethod
    def _sync_directory(path: Path) -> None:
        descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)

    @classmethod
    def _create_directory(cls, path: Path) -> None:
        missing = []
        ancestor = path
        while not ancestor.exists():
            missing.append(ancestor)
            ancestor = ancestor.parent
        # Persist every newly created directory entry, including a new archive
        # root, before a later file acknowledgement can depend on it.
        for created in reversed(missing):
            created.mkdir(exist_ok=True, mode=0o700)
            cls._sync_directory(created.parent)
        path.mkdir(exist_ok=True, mode=0o700)

    def write_once(self, relative_path: Path | str, envelope: CiphertextEnvelope) -> Path:
        relative = Path(relative_path)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("archive path must be relative and remain under its root")
        target = self.root / relative
        self._create_directory(target.parent)
        os.chmod(target.parent, 0o700)
        descriptor = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            with os.fdopen(descriptor, "wb") as handle:
                handle.write(envelope.ciphertext)
                handle.flush()
                os.fsync(handle.fileno())
            # Persist the directory entry before reporting durable creation.
            self._sync_directory(target.parent)
        except BaseException:
            # Preserve a possibly partial artifact for diagnosis. O_EXCL prevents
            # a retry from silently replacing it.
            raise
        return target


@dataclass(frozen=True)
class ArchivedSnapshotReceipt:
    """Public metadata proving which private snapshot ciphertext was retained."""

    commitment: SnapshotCommitment
    envelope: CiphertextEnvelope
    archive_path: Path

    def public_dict(self) -> dict[str, Any]:
        return {
            "schema": ARCHIVED_SNAPSHOT_RECEIPT_SCHEMA,
            "snapshotCommitment": self.commitment.to_dict(),
            "ciphertextEnvelope": self.envelope.public_dict(),
        }


@dataclass(frozen=True)
class ArchivedBrokerReserveReceipt:
    """Public metadata for an encrypted private broker-reserve observation."""

    disclosure: YoloBrokerReserveDisclosure
    private_snapshot_sha256: str
    envelope: CiphertextEnvelope
    archive_path: Path

    def public_dict(self) -> dict[str, Any]:
        return {
            "schema": ARCHIVED_BROKER_RESERVE_RECEIPT_SCHEMA,
            "disclosure": self.disclosure.to_dict(),
            "privateSnapshotSha256": self.private_snapshot_sha256,
            "ciphertextEnvelope": self.envelope.public_dict(),
        }


def snapshot_archive_associated_data(snapshot: PrivateOptionChainSnapshot) -> bytes:
    return canonical_json_bytes({
        "schema": ARCHIVE_ASSOCIATED_DATA_SCHEMA,
        "epochSha256": snapshot.epoch_sha256,
        "sequence": snapshot.sequence,
        "sourceId": snapshot.source_id,
        "privateSnapshotSha256": snapshot.sha256,
    })


def archive_snapshot(
    snapshot: PrivateOptionChainSnapshot,
    *,
    relative_path: Path | str,
    encryptor: SnapshotEncryptor,
    archive: CiphertextArchive,
) -> ArchivedSnapshotReceipt:
    """Encrypt and retain one complete snapshot without exposing its contents."""

    plaintext = canonical_json_bytes(snapshot.to_private_dict())
    associated_data = snapshot_archive_associated_data(snapshot)
    envelope = encrypt_snapshot(
        plaintext,
        associated_data=associated_data,
        encryptor=encryptor,
    )
    archive_path = archive.write_once(relative_path, envelope)
    return ArchivedSnapshotReceipt(
        commitment=snapshot.commitment(),
        envelope=envelope,
        archive_path=archive_path,
    )


def broker_reserve_archive_associated_data(
    snapshot: PrivateBrokerReserveSnapshot,
) -> bytes:
    return canonical_json_bytes({
        "schema": BROKER_RESERVE_ARCHIVE_AAD_SCHEMA,
        "brokerSourceId": snapshot.broker_source_id,
        "accountApplicationIdentitySha256": (
            snapshot.account_application_identity_sha256
        ),
        "observedAtUtc": snapshot.to_private_dict()["observedAtUtc"],
        "privateSnapshotSha256": snapshot.sha256,
        "disclosure": snapshot.disclosure().to_dict(),
    })


def archive_broker_reserve_snapshot(
    snapshot: PrivateBrokerReserveSnapshot,
    *,
    relative_path: Path | str,
    encryptor: SnapshotEncryptor,
    archive: CiphertextArchive,
) -> ArchivedBrokerReserveReceipt:
    """Encrypt and retain one broker-reserve snapshot without exposing it."""

    plaintext = canonical_json_bytes(snapshot.to_private_dict())
    associated_data = broker_reserve_archive_associated_data(snapshot)
    envelope = encrypt_snapshot(
        plaintext,
        associated_data=associated_data,
        encryptor=encryptor,
    )
    archive_path = archive.write_once(relative_path, envelope)
    return ArchivedBrokerReserveReceipt(
        disclosure=snapshot.disclosure(),
        private_snapshot_sha256=snapshot.sha256,
        envelope=envelope,
        archive_path=archive_path,
    )


def verify_replay_plaintext(
    plaintext: bytes,
    *,
    commitment: SnapshotCommitment,
) -> PrivateOptionChainSnapshot:
    """Strictly reconstruct a private snapshot and bind it to its public receipt."""

    if not isinstance(plaintext, bytes):
        raise TypeError("replay plaintext must be bytes")
    if not plaintext:
        raise ValueError("replay plaintext is empty")
    try:
        value = json.loads(plaintext.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("replay plaintext must be canonical JSON") from exc
    if canonical_json_bytes(value) != plaintext:
        raise ValueError("replay plaintext must use canonical JSON bytes")
    snapshot = PrivateOptionChainSnapshot.from_private_dict(value)
    if snapshot.commitment().to_dict() != commitment.to_dict():
        raise ValueError("replayed private snapshot does not match public commitment")
    return snapshot


def verify_broker_reserve_replay_plaintext(
    plaintext: bytes,
    *,
    receipt: ArchivedBrokerReserveReceipt,
) -> PrivateBrokerReserveSnapshot:
    """Reconstruct a broker-reserve snapshot and bind it to its public receipt."""

    from .reserve import PrivateBrokerReserveSnapshot

    if not isinstance(plaintext, bytes):
        raise TypeError("broker reserve replay plaintext must be bytes")
    if not plaintext:
        raise ValueError("broker reserve replay plaintext is empty")
    try:
        value = json.loads(plaintext.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("broker reserve replay plaintext must be canonical JSON") from exc
    if canonical_json_bytes(value) != plaintext:
        raise ValueError("broker reserve replay plaintext must use canonical JSON bytes")
    snapshot = PrivateBrokerReserveSnapshot.from_private_dict(value)
    if snapshot.sha256 != receipt.private_snapshot_sha256:
        raise ValueError("replayed broker reserve does not match private snapshot hash")
    if snapshot.disclosure().to_dict() != receipt.disclosure.to_dict():
        raise ValueError("replayed broker reserve does not match public disclosure")
    if sha256_bytes(plaintext) != receipt.envelope.plaintext_sha256:
        raise ValueError("replayed broker reserve does not match archived plaintext hash")
    return snapshot
