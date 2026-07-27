"""Offline auditor-authorization gate for EVM deployment broadcasts.

Call ``require_auditor_authorization`` before constructing Web3, loading a
signer/agent, opening a session, or selecting a sender.  The helper is chain
agnostic; callers supply the expected manifest chain id.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


class AuthorizationError(RuntimeError):
    """Raised when a broadcast authorization is absent or does not bind."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise AuthorizationError(f"invalid {label}: {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise AuthorizationError(f"{label} must be a JSON object: {path}")
    return value


def _require_string(document: dict[str, Any], path: tuple[str, ...]) -> str:
    value: Any = document
    for key in path:
        if not isinstance(value, dict) or key not in value:
            raise AuthorizationError("authorization missing " + ".".join(path))
        value = value[key]
    if not isinstance(value, str) or not value:
        raise AuthorizationError("authorization has invalid " + ".".join(path))
    return value


def _require_int(document: dict[str, Any], path: tuple[str, ...]) -> int:
    value: Any = document
    for key in path:
        if not isinstance(value, dict) or key not in value:
            raise AuthorizationError("authorization missing " + ".".join(path))
        value = value[key]
    if not isinstance(value, int) or isinstance(value, bool):
        raise AuthorizationError("authorization has invalid " + ".".join(path))
    return value


def _artifact_plan(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    try:
        artifacts = manifest["contracts"]["artifacts"]
    except (KeyError, TypeError) as exc:
        raise AuthorizationError("manifest lacks contracts.artifacts") from exc
    if not isinstance(artifacts, list):
        raise AuthorizationError("manifest contracts.artifacts is not a list")
    result: dict[str, dict[str, Any]] = {}
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise AuthorizationError("manifest contract artifact is not an object")
        name = artifact.get("contract")
        if name in result or not isinstance(name, str):
            raise AuthorizationError("manifest has duplicate or invalid contract artifact")
        result[name] = artifact
    return result


def require_auditor_authorization(
    manifest_path: Path,
    manifest: dict[str, Any],
    authorization_path: Path | None,
    expected_chain_id: int,
) -> dict[str, Any]:
    """Require a PASS authorization bound to this exact pre-broadcast manifest.

    This routine only reads local files and computes hashes.  It deliberately
    does not inspect the network, credentials, agents, or signing material.
    """
    if authorization_path is None:
        raise AuthorizationError("--auditor-authorization is required for broadcast")
    authorization = _load_object(authorization_path.resolve(), "auditor authorization")
    if authorization.get("schema") != "postfiat.deployment_auditor_authorization.v1":
        raise AuthorizationError("authorization schema is unsupported")
    if authorization.get("status") != "PASS":
        raise AuthorizationError("authorization status is not PASS")
    actual_path = str(manifest_path.resolve())
    actual_digest = sha256_file(manifest_path)
    if _require_string(authorization, ("manifest_path",)) != actual_path:
        raise AuthorizationError("authorization manifest_path does not exactly match the input manifest")
    if _require_string(authorization, ("manifest_sha256",)).lower() != actual_digest:
        raise AuthorizationError("authorization manifest_sha256 does not match the input manifest")
    revision = manifest.get("revision")
    if not isinstance(revision, str) or _require_string(authorization, ("route_revision",)) != revision:
        raise AuthorizationError("authorization route_revision does not match the manifest")
    chain_id = manifest.get("network", {}).get("source_chain_id")
    if chain_id != expected_chain_id or _require_int(authorization, ("chain_id",)) != expected_chain_id:
        raise AuthorizationError("authorization chain_id does not match the manifest scope")
    plan = _artifact_plan(manifest)
    authorization_plan = authorization.get("planned_deployments")
    if not isinstance(authorization_plan, dict):
        raise AuthorizationError("authorization missing planned_deployments")
    for authorization_name, contract in (("verifier", "PFTLFinalityVerifierV1"), ("vault", "ERC20BridgeVaultL1")):
        if contract not in plan or not isinstance(authorization_plan.get(authorization_name), dict):
            raise AuthorizationError(f"authorization lacks {authorization_name} deployment binding")
        expected = plan[contract]
        supplied = authorization_plan[authorization_name]
        if supplied.get("nonce") != expected.get("precomputed_create_nonce"):
            raise AuthorizationError(f"authorization {authorization_name} nonce does not match manifest")
        address = supplied.get("address")
        if not isinstance(address, str) or address.lower() != str(expected.get("address", "")).lower():
            raise AuthorizationError(f"authorization {authorization_name} address does not match manifest")
    _require_string(authorization, ("auditor", "identity"))
    _require_string(authorization, ("auditor", "name"))
    artifact_digest = _require_string(authorization, ("audit_artifact_sha256",))
    if len(artifact_digest) != 64 or any(char not in "0123456789abcdefABCDEF" for char in artifact_digest):
        raise AuthorizationError("authorization audit_artifact_sha256 is not a SHA-256 digest")
    _require_string(authorization, ("timestamp",))
    return {
        "authorization_path": str(authorization_path.resolve()),
        "manifest_path": actual_path,
        "manifest_sha256": actual_digest,
        "authorization": authorization,
    }


def retain_prebroadcast_manifest(manifest_path: Path, evidence_dir: Path, manifest_sha256: str) -> Path:
    """Copy the immutable input manifest before any deployment enrichment."""
    evidence_dir.mkdir(parents=True, exist_ok=True)
    retained = evidence_dir / f"manifest.pre-broadcast.sha256-{manifest_sha256}.json"
    source = manifest_path.read_bytes()
    if retained.exists():
        if retained.read_bytes() != source:
            raise AuthorizationError(f"pre-broadcast manifest copy conflicts: {retained}")
        return retained
    retained.write_bytes(source)
    return retained
