"""Zero-PII firewall, salted commitment generator, and durable credential status store.

Enforces Pillar 8's non-negotiable zero-PII rule: raw personal data is processed
strictly in memory to verify external proofs and derive a salted SHA3-384
commitment hash, then discarded. Neither disk storage nor emitted L1 payloads
may ever contain PII fields.
"""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
import secrets
import tempfile
from pathlib import Path
from typing import Any

from .types_c4 import CREDENTIAL_COMMITMENT_DOMAIN, validate_canonical_pf_address, validate_hex96

STORE_SCHEMA_V1 = "postfiat.credential_adapter.zero_pii_store.v1"

# Strict denylist of PII field names (case-insensitive normalized) that MUST NEVER
# appear anywhere in persisted records or emitted L1 payloads.
FORBIDDEN_PII_KEYS = frozenset(
    {
        "name",
        "fullname",
        "givenname",
        "familyname",
        "firstname",
        "lastname",
        "middlename",
        "dateofbirth",
        "birthdate",
        "dob",
        "passportnumber",
        "documentnumber",
        "nationalid",
        "ssn",
        "taxid",
        "tin",
        "email",
        "emailaddress",
        "phone",
        "phonenumber",
        "address",
        "streetaddress",
        "residentialaddress",
        "postalcode",
        "zipcode",
        "mrz",
        "mrzline1",
        "mrzline2",
        "photo",
        "faceimage",
        "biometric",
        "documentimage",
        "rawcredential",
        "rawproof",
        "credentialsubject",
        "citizenship",
        "nationality",
        "gender",
        "sex",
    }
)

ALLOWED_STORE_RECORD_KEYS = frozenset(
    {
        "credential_id",
        "subject",
        "issuer_id",
        "credential_type",
        "commitment_hash",
        "provider",
        "nullifier_digest",
        "external_ref_digest",
        "issued_at_height",
        "expires_at_height",
        "status",
        "revoked_at_height",
        "revocation_reason",
    }
)


class PiiLeakageError(ValueError):
    """Raised whenever forbidden PII fields attempt to cross a persistence boundary."""


def _normalize_key(key: str) -> str:
    return "".join(ch for ch in key.lower() if ch.isalnum())


def assert_zero_pii_structure(obj: Any, path: str = "$") -> None:
    """Recursively reject any object containing forbidden PII keys."""
    if isinstance(obj, dict):
        for key, value in obj.items():
            if _normalize_key(str(key)) in FORBIDDEN_PII_KEYS:
                raise PiiLeakageError(f"forbidden PII key detected at {path}.{key}")
            assert_zero_pii_structure(value, f"{path}.{key}")
    elif isinstance(obj, (list, tuple)):
        for idx, item in enumerate(obj):
            assert_zero_pii_structure(item, f"{path}[{idx}]")


def compute_salted_commitment(
    *,
    subject: str,
    issuer_id: str,
    credential_type: str,
    verified_claims: dict[str, Any],
    salt_bytes: bytes | None = None,
) -> str:
    """Compute a 96-hex SHA3-384 salted commitment over verified claims.

    The 32-byte salt is generated from ``secrets.token_bytes(32)`` when omitted
    and is NEVER persisted by the adapter, preventing rainbow-table reversal.
    """
    validate_canonical_pf_address("subject", subject)
    validate_hex96("issuer_id", issuer_id)
    salt = salt_bytes if salt_bytes is not None else secrets.token_bytes(32)
    if len(salt) < 16:
        raise ValueError("commitment salt must be at least 16 bytes")
    canonical = json.dumps(
        {
            "subject": subject,
            "issuer_id": issuer_id,
            "credential_type": credential_type,
            "claims": verified_claims,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(CREDENTIAL_COMMITMENT_DOMAIN.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(salt)
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def digest_external_identifier(domain: str, value: str, pepper: bytes) -> str:
    """One-way keyed SHA3-384 digest for nullifiers or external VC IDs."""
    hasher = hashlib.sha3_384()
    hasher.update(domain.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(pepper)
    hasher.update(b"\x00")
    hasher.update(value.encode("utf-8"))
    return hasher.hexdigest()


class ZeroPiiCredentialStore:
    """File-locked durable audit & revocation store guaranteed to hold ZERO PII."""

    def __init__(self, state_path: Path, *, pepper: bytes | None = None) -> None:
        self.state_path = Path(state_path)
        self.lock_path = self.state_path.with_suffix(self.state_path.suffix + ".lock")
        self._pepper = pepper if pepper is not None else b"postfiat.zero_pii.default_pepper.v1"

    def hash_nullifier(self, raw_nullifier: str) -> str:
        return digest_external_identifier("postfiat.zkp.nullifier.v1", raw_nullifier, self._pepper)

    def hash_external_ref(self, external_id: str) -> str:
        return digest_external_identifier("postfiat.vc.external_ref.v1", external_id, self._pepper)

    def _default(self) -> dict[str, Any]:
        return {
            "schema": STORE_SCHEMA_V1,
            "credentials": {},
            "nullifiers": {},
            "external_refs": {},
            "used_nonces": [],
        }

    def load(self) -> dict[str, Any]:
        if not self.state_path.exists():
            return self._default()
        raw = json.loads(self.state_path.read_text(encoding="utf-8"))
        if not isinstance(raw, dict) or raw.get("schema") != STORE_SCHEMA_V1:
            raise ValueError(f"unsupported zero-PII store schema in {self.state_path}")
        return raw

    def _save_atomic(self, state: dict[str, Any]) -> None:
        assert_zero_pii_structure(state)
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp_name = tempfile.mkstemp(
            dir=str(self.state_path.parent),
            prefix=f".{self.state_path.name}.",
            suffix=".tmp",
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as handle:
                json.dump(state, handle, indent=2, sort_keys=True)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            os.chmod(tmp_name, 0o600)
            os.replace(tmp_name, self.state_path)
        except Exception:
            try:
                os.unlink(tmp_name)
            except OSError:
                pass
            raise

    def record_issuance(
        self,
        *,
        credential_id: str,
        subject: str,
        issuer_id: str,
        credential_type: str,
        commitment_hash: str,
        provider: str,
        issued_at_height: int,
        expires_at_height: int,
        raw_nullifier: str | None = None,
        external_ref_id: str | None = None,
        request_nonce: str | None = None,
    ) -> dict[str, Any]:
        validate_hex96("credential_id", credential_id)
        validate_canonical_pf_address("subject", subject)
        validate_hex96("issuer_id", issuer_id)
        validate_hex96("commitment_hash", commitment_hash)

        nullifier_digest = self.hash_nullifier(raw_nullifier) if raw_nullifier else None
        external_ref_digest = self.hash_external_ref(external_ref_id) if external_ref_id else None

        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.lock_path, "a+", encoding="utf-8") as lock_file:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
            try:
                state = self.load()
                if request_nonce:
                    if request_nonce in state["used_nonces"]:
                        raise ValueError("replay_detected: request nonce already consumed")
                if nullifier_digest:
                    existing_cred = state["nullifiers"].get(nullifier_digest)
                    if existing_cred is not None and existing_cred != credential_id:
                        raise ValueError(
                            "zkp_nullifier_already_used: passport nullifier is bound to another credential"
                        )
                record = {
                    "credential_id": credential_id,
                    "subject": subject,
                    "issuer_id": issuer_id,
                    "credential_type": credential_type,
                    "commitment_hash": commitment_hash,
                    "provider": provider,
                    "nullifier_digest": nullifier_digest,
                    "external_ref_digest": external_ref_digest,
                    "issued_at_height": int(issued_at_height),
                    "expires_at_height": int(expires_at_height),
                    "status": "issued",
                    "revoked_at_height": None,
                    "revocation_reason": None,
                }
                if set(record.keys()) != ALLOWED_STORE_RECORD_KEYS:
                    raise PiiLeakageError("record keys drifted from zero-PII allowlist")
                assert_zero_pii_structure(record)
                state["credentials"][credential_id] = record
                if nullifier_digest:
                    state["nullifiers"][nullifier_digest] = credential_id
                if external_ref_digest:
                    state["external_refs"][external_ref_digest] = credential_id
                if request_nonce:
                    state["used_nonces"].append(request_nonce)
                self._save_atomic(state)
                return record
            finally:
                fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)

    def record_revocation(
        self,
        *,
        credential_id: str | None = None,
        external_ref_id: str | None = None,
        revoked_at_height: int,
        reason_code: str,
    ) -> dict[str, Any]:
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.lock_path, "a+", encoding="utf-8") as lock_file:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
            try:
                state = self.load()
                target_id = credential_id
                if target_id is None and external_ref_id is not None:
                    ref_digest = self.hash_external_ref(external_ref_id)
                    target_id = state["external_refs"].get(ref_digest)
                if not target_id or target_id not in state["credentials"]:
                    raise KeyError("credential_not_found")
                record = state["credentials"][target_id]
                record["status"] = "revoked"
                record["revoked_at_height"] = int(revoked_at_height)
                record["revocation_reason"] = str(reason_code)
                assert_zero_pii_structure(record)
                self._save_atomic(state)
                return dict(record)
            finally:
                fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)

    def get(self, credential_id: str) -> dict[str, Any] | None:
        state = self.load()
        rec = state["credentials"].get(credential_id)
        return dict(rec) if rec is not None else None
