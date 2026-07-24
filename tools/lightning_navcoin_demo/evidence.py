"""Tamper-evident, secret-aware evidence bundle writer."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
from typing import Any, Mapping


SCHEMA = "postfiat.lightning_navcoin_demo.evidence.v1"
REDACTED = "<redacted>"
CANONICAL_PREIMAGE_RE = re.compile(r"^[0-9a-f]{64}$")
SECRET_FIELD_MARKERS = (
    "preimage",
    "fulfillment",
    "seed",
    "mnemonic",
    "macaroon",
    "private_key",
    "wallet_password",
)


class EvidenceError(RuntimeError):
    """Evidence is unsafe, non-canonical, or already finalized."""


def canonical_json_bytes(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _assert_general_log_safe(value: Any, path: str = "$") -> None:
    if isinstance(value, Mapping):
        for key, child in value.items():
            key_text = str(key).lower()
            child_path = f"{path}.{key}"
            if any(marker in key_text for marker in SECRET_FIELD_MARKERS):
                if child != REDACTED:
                    raise EvidenceError(f"secret field must be redacted at {child_path}")
            _assert_general_log_safe(child, child_path)
    elif isinstance(value, (list, tuple)):
        for index, child in enumerate(value):
            _assert_general_log_safe(child, f"{path}[{index}]")


def _atomic_write(path: Path, payload: bytes, *, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp.{os.getpid()}")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL,
        mode,
    )
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


@dataclass
class EvidenceBundle:
    """One append-only hash chain plus immutable JSON artifacts."""

    root: Path
    run_id: str

    def __post_init__(self) -> None:
        if not self.run_id or not self.run_id.isascii():
            raise EvidenceError("run_id must be nonempty ASCII")
        self.root = self.root.resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        if any(self.root.iterdir()):
            raise EvidenceError("evidence root must be empty")
        self._events_path = self.root / "events.jsonl"
        self._manifest_path = self.root / "manifest.json"
        if self._manifest_path.exists():
            raise EvidenceError("bundle is already finalized")
        self._sequence = 0
        self._previous_hash = "0" * 64
        if self._events_path.exists() and self._events_path.stat().st_size:
            raise EvidenceError("refusing to append to an existing event log")

    def record(self, event: str, payload: Mapping[str, Any]) -> str:
        if self._manifest_path.exists():
            raise EvidenceError("bundle is already finalized")
        if not event or not event.isascii():
            raise EvidenceError("event name must be nonempty ASCII")
        _assert_general_log_safe(payload)
        body = {
            "schema": SCHEMA,
            "run_id": self.run_id,
            "sequence": self._sequence,
            "recorded_at": datetime.now(timezone.utc).isoformat(),
            "event": event,
            "payload": dict(payload),
            "previous_record_sha256": self._previous_hash,
        }
        record_hash = hashlib.sha256(canonical_json_bytes(body)).hexdigest()
        record = dict(body)
        record["record_sha256"] = record_hash
        encoded = canonical_json_bytes(record)
        descriptor = os.open(
            self._events_path,
            os.O_WRONLY | os.O_CREAT | os.O_APPEND,
            0o600,
        )
        with os.fdopen(descriptor, "ab") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        self._sequence += 1
        self._previous_hash = record_hash
        return record_hash

    def write_json(self, relative_path: str, value: Any) -> Path:
        if self._manifest_path.exists():
            raise EvidenceError("bundle is already finalized")
        relative = Path(relative_path)
        if relative.is_absolute() or ".." in relative.parts or not relative.parts:
            raise EvidenceError("artifact path must remain inside the bundle")
        if relative.name in {
            "events.jsonl",
            "manifest.json",
            "test-vector.json",
            "test-vectors.json",
        }:
            raise EvidenceError("artifact path is reserved")
        _assert_general_log_safe(value)
        destination = self.root / relative
        if destination.exists():
            raise EvidenceError(f"artifact already exists: {relative_path}")
        _atomic_write(destination, canonical_json_bytes(value))
        return destination

    def write_test_vector(
        self,
        *,
        preimage_hex: str,
        payment_hash_hex: str,
        condition: str,
        fulfillment: str,
    ) -> Path:
        """Write the sole artifact permitted to disclose a synthetic preimage."""

        if CANONICAL_PREIMAGE_RE.fullmatch(preimage_hex) is None:
            raise EvidenceError("test-vector preimage must be 32-byte lowercase hex")
        if hashlib.sha256(bytes.fromhex(preimage_hex)).hexdigest() != payment_hash_hex:
            raise EvidenceError("test-vector preimage does not match payment hash")
        return self.write_test_vectors(
            [
                {
                    "name": "canonical",
                    "preimage": preimage_hex,
                    "payment_hash": payment_hash_hex,
                    "condition": condition,
                    "fulfillment": fulfillment,
                }
            ],
            filename="test-vector.json",
        )

    def write_test_vectors(
        self,
        vectors: list[Mapping[str, str]],
        *,
        filename: str = "test-vectors.json",
    ) -> Path:
        """Write the only artifact class allowed to disclose test secrets."""

        if filename not in {"test-vector.json", "test-vectors.json"}:
            raise EvidenceError("test-vector filename is reserved")
        if not vectors:
            raise EvidenceError("at least one test vector is required")
        validated: list[dict[str, str]] = []
        names: set[str] = set()
        for vector in vectors:
            name = vector.get("name")
            preimage_hex = vector.get("preimage")
            payment_hash_hex = vector.get("payment_hash")
            condition = vector.get("condition")
            fulfillment = vector.get("fulfillment")
            if (
                type(name) is not str
                or not name
                or not name.isascii()
                or name in names
            ):
                raise EvidenceError("test-vector names must be unique nonempty ASCII")
            if (
                type(preimage_hex) is not str
                or CANONICAL_PREIMAGE_RE.fullmatch(preimage_hex) is None
            ):
                raise EvidenceError(
                    "test-vector preimage must be 32-byte lowercase hex"
                )
            if (
                type(payment_hash_hex) is not str
                or hashlib.sha256(bytes.fromhex(preimage_hex)).hexdigest()
                != payment_hash_hex
            ):
                raise EvidenceError("test-vector preimage does not match payment hash")
            if type(condition) is not str or payment_hash_hex not in condition:
                raise EvidenceError("test-vector condition does not bind payment hash")
            if type(fulfillment) is not str or preimage_hex not in fulfillment:
                raise EvidenceError("test-vector fulfillment does not contain preimage")
            names.add(name)
            validated.append(
                {
                    "name": name,
                    "preimage": preimage_hex,
                    "payment_hash": payment_hash_hex,
                    "condition": condition,
                    "fulfillment": fulfillment,
                }
            )
        destination = self.root / filename
        if destination.exists():
            raise EvidenceError("test-vector artifact already exists")
        _atomic_write(
            destination,
            canonical_json_bytes(
                {
                    "schema": "postfiat.lightning_navcoin_demo.test_vectors.v1",
                    "scope": "synthetic-regtest-only",
                    "vectors": validated,
                }
            ),
        )
        return destination

    def finalize(self, summary: Mapping[str, Any]) -> Path:
        _assert_general_log_safe(summary)
        files: dict[str, dict[str, Any]] = {}
        for path in sorted(self.root.rglob("*")):
            if not path.is_file() or path == self._manifest_path:
                continue
            relative = path.relative_to(self.root).as_posix()
            files[relative] = {
                "bytes": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        manifest = {
            "schema": SCHEMA,
            "run_id": self.run_id,
            "event_count": self._sequence,
            "event_chain_tip_sha256": self._previous_hash,
            "files": files,
            "summary": dict(summary),
        }
        _atomic_write(self._manifest_path, canonical_json_bytes(manifest))
        return self._manifest_path


def verify_bundle(root: Path) -> dict[str, Any]:
    root = root.resolve()
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="ascii"))
    if manifest.get("schema") != SCHEMA:
        raise EvidenceError("unexpected manifest schema")
    files = manifest.get("files")
    if not isinstance(files, Mapping):
        raise EvidenceError("manifest files must be an object")
    _assert_general_log_safe(manifest.get("summary"))
    actual_files: set[str] = set()
    for path in root.rglob("*"):
        if path.is_symlink():
            raise EvidenceError("evidence bundle must not contain symlinks")
        if path.is_file() and path != manifest_path:
            actual_files.add(path.relative_to(root).as_posix())
    if actual_files != set(files):
        raise EvidenceError("manifest file set does not match bundle contents")
    for relative, metadata in files.items():
        if type(relative) is not str:
            raise EvidenceError("artifact path must be a string")
        relative_path = Path(relative)
        if (
            relative_path.is_absolute()
            or ".." in relative_path.parts
            or relative_path.as_posix() != relative
            or not relative_path.parts
        ):
            raise EvidenceError(f"non-canonical artifact path: {relative!r}")
        path = root / relative
        if root not in path.resolve().parents:
            raise EvidenceError(f"artifact escapes evidence root: {relative}")
        if not path.is_file():
            raise EvidenceError(f"missing artifact: {relative}")
        if not isinstance(metadata, Mapping):
            raise EvidenceError(f"invalid artifact metadata: {relative}")
        if path.stat().st_size != metadata.get("bytes"):
            raise EvidenceError(f"size mismatch: {relative}")
        if sha256_file(path) != metadata.get("sha256"):
            raise EvidenceError(f"hash mismatch: {relative}")
        if relative.endswith(".json") and relative not in {
            "test-vector.json",
            "test-vectors.json",
        }:
            _assert_general_log_safe(
                json.loads(path.read_text(encoding="ascii"))
            )

    previous_hash = "0" * 64
    event_count = 0
    with (root / "events.jsonl").open("rb") as handle:
        for encoded in handle:
            record = json.loads(encoded)
            claimed_hash = record.pop("record_sha256", None)
            _assert_general_log_safe(record.get("payload"))
            if record.get("sequence") != event_count:
                raise EvidenceError("event sequence is not contiguous")
            if record.get("previous_record_sha256") != previous_hash:
                raise EvidenceError("event hash chain is broken")
            actual_hash = hashlib.sha256(canonical_json_bytes(record)).hexdigest()
            if claimed_hash != actual_hash:
                raise EvidenceError("event record hash mismatch")
            previous_hash = actual_hash
            event_count += 1
    if event_count != manifest.get("event_count"):
        raise EvidenceError("manifest event count mismatch")
    if previous_hash != manifest.get("event_chain_tip_sha256"):
        raise EvidenceError("manifest event chain tip mismatch")
    return manifest
