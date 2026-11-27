"""Non-issuer attestor signature validation and L1 nav_reserve_attest builders.

Enforces the Pillar 7 / Contract C4 non-issuer rule: at least ``min_attestations``
distinct registered attestors where ``attestor != asset_issuer`` must validly sign
``pass == True`` over the exact verified HyperCore snapshot observation root and
reserve packet hash.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

ATTESTATION_SIGNING_DOMAIN_V1 = "postfiat.hyperliquid_nav.attestation.v1"
_CANONICAL_PF_RE = re.compile(r"^pf[0-9a-f]{40}$")
_HEX96_RE = re.compile(r"^[0-9a-f]{96}$")


@dataclass(frozen=True)
class RegisteredAttestor:
    address: str
    domain: str
    ed25519_public_key_hex: str
    bond: int = 0

    def validate(self) -> None:
        if not _CANONICAL_PF_RE.match(self.address):
            raise ValueError(f"invalid attestor address: {self.address}")
        if not self.domain:
            raise ValueError("attestor domain must be non-empty")
        raw = bytes.fromhex(self.ed25519_public_key_hex)
        if len(raw) != 32:
            raise ValueError("ed25519_public_key_hex must be 32 bytes")


@dataclass(frozen=True)
class NonIssuerAttestation:
    attestor: str
    asset_id: str
    epoch: int
    reserve_packet_hash: str
    observation_root: str
    snapshot_commitment: str
    verified_net_assets_usd_e6: int
    passed: bool
    attested_at_unix: int
    signature_hex: str


@dataclass(frozen=True)
class AttestorQuorumReport:
    quorum_met: bool
    valid_non_issuer_count: int
    required_quorum: int
    valid_attestors: tuple[str, ...]
    rejected_reasons: tuple[str, ...]
    attestor_root: str
    l1_attest_operations: tuple[dict[str, Any], ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "quorum_met": self.quorum_met,
            "valid_non_issuer_count": self.valid_non_issuer_count,
            "required_quorum": self.required_quorum,
            "valid_attestors": list(self.valid_attestors),
            "rejected_reasons": list(self.rejected_reasons),
            "attestor_root": self.attestor_root,
            "l1_attest_operations": list(self.l1_attest_operations),
        }


def attestation_signing_message(
    *,
    attestor: str,
    asset_id: str,
    epoch: int,
    reserve_packet_hash: str,
    observation_root: str,
    snapshot_commitment: str,
    verified_net_assets_usd_e6: int,
    passed: bool,
    attested_at_unix: int,
) -> bytes:
    canonical = json.dumps(
        {
            "attestor": attestor,
            "asset_id": asset_id,
            "epoch": epoch,
            "reserve_packet_hash": reserve_packet_hash,
            "observation_root": observation_root,
            "snapshot_commitment": snapshot_commitment.lower(),
            "verified_net_assets_usd_e6": verified_net_assets_usd_e6,
            "pass": bool(passed),
            "attested_at_unix": attested_at_unix,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(ATTESTATION_SIGNING_DOMAIN_V1.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.digest()


def sign_attestation(
    *,
    key_seed_32: bytes,
    attestor: str,
    asset_id: str,
    epoch: int,
    reserve_packet_hash: str,
    observation_root: str,
    snapshot_commitment: str,
    verified_net_assets_usd_e6: int,
    passed: bool,
    attested_at_unix: int,
) -> NonIssuerAttestation:
    """Create a cryptographically signed Ed25519 attestor vote."""
    key = Ed25519PrivateKey.from_private_bytes(key_seed_32)
    msg = attestation_signing_message(
        attestor=attestor,
        asset_id=asset_id,
        epoch=epoch,
        reserve_packet_hash=reserve_packet_hash,
        observation_root=observation_root,
        snapshot_commitment=snapshot_commitment,
        verified_net_assets_usd_e6=verified_net_assets_usd_e6,
        passed=passed,
        attested_at_unix=attested_at_unix,
    )
    sig_hex = key.sign(msg).hex()
    return NonIssuerAttestation(
        attestor=attestor,
        asset_id=asset_id,
        epoch=epoch,
        reserve_packet_hash=reserve_packet_hash,
        observation_root=observation_root,
        snapshot_commitment=snapshot_commitment.lower(),
        verified_net_assets_usd_e6=verified_net_assets_usd_e6,
        passed=bool(passed),
        attested_at_unix=attested_at_unix,
        signature_hex=sig_hex,
    )


def build_nav_reserve_attest_op(att: NonIssuerAttestation) -> dict[str, Any]:
    """Canonical L1 ``nav_reserve_attest`` operation matching ``python/postfiat_rpc/navcoin.py``."""
    return {
        "operation": "nav_reserve_attest",
        "attestor": att.attestor,
        "asset_id": att.asset_id,
        "epoch": att.epoch,
        "reserve_packet_hash": att.reserve_packet_hash,
        "pass": att.passed,
        "observation_root": att.observation_root,
    }


def verify_non_issuer_attestor_quorum(
    *,
    asset_issuer: str,
    asset_id: str,
    epoch: int,
    reserve_packet_hash: str,
    expected_observation_root: str,
    expected_snapshot_commitment: str,
    expected_net_assets_usd_e6: int,
    registered_attestors: list[RegisteredAttestor],
    attestations: list[NonIssuerAttestation],
    min_attestations: int = 2,
    now_unix: int | None = None,
    max_attestation_age_seconds: int = 300,
) -> AttestorQuorumReport:
    """Validate non-issuer attestor signatures and enforce quorum fail-closed."""
    if not _CANONICAL_PF_RE.match(asset_issuer):
        raise ValueError("asset_issuer must be a canonical pf address")
    if not _HEX96_RE.match(asset_id) or not _HEX96_RE.match(reserve_packet_hash):
        raise ValueError("asset_id and reserve_packet_hash must be 96 lowercase hex")
    if not _HEX96_RE.match(expected_observation_root):
        raise ValueError("expected_observation_root must be 96 lowercase hex")
    if min_attestations < 1:
        raise ValueError("min_attestations must be >= 1")

    registry: dict[str, RegisteredAttestor] = {}
    for entry in registered_attestors:
        entry.validate()
        registry[entry.address] = entry

    valid_attestors: list[str] = []
    valid_ops: list[dict[str, Any]] = []
    rejected: list[str] = []
    seen: set[str] = set()

    for att in attestations:
        if att.attestor in seen:
            rejected.append(f"{att.attestor}:duplicate_attestation")
            continue
        seen.add(att.attestor)

        if att.attestor == asset_issuer:
            rejected.append(f"{att.attestor}:nav_fee_issuer_self_attestation_forbidden")
            continue
        reg = registry.get(att.attestor)
        if reg is None:
            rejected.append(f"{att.attestor}:unregistered_attestor")
            continue
        if not att.passed:
            rejected.append(f"{att.attestor}:attestation_failed_vote")
            continue
        if (
            att.asset_id != asset_id
            or att.epoch != epoch
            or att.reserve_packet_hash != reserve_packet_hash
            or att.observation_root != expected_observation_root
            or att.snapshot_commitment.lower() != expected_snapshot_commitment.lower()
            or att.verified_net_assets_usd_e6 != expected_net_assets_usd_e6
        ):
            rejected.append(f"{att.attestor}:binding_mismatch")
            continue
        if now_unix is not None:
            age = now_unix - att.attested_at_unix
            if age < -30 or age > max_attestation_age_seconds:
                rejected.append(f"{att.attestor}:attestation_stale_or_future")
                continue

        msg = attestation_signing_message(
            attestor=att.attestor,
            asset_id=att.asset_id,
            epoch=att.epoch,
            reserve_packet_hash=att.reserve_packet_hash,
            observation_root=att.observation_root,
            snapshot_commitment=att.snapshot_commitment,
            verified_net_assets_usd_e6=att.verified_net_assets_usd_e6,
            passed=att.passed,
            attested_at_unix=att.attested_at_unix,
        )
        try:
            pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(reg.ed25519_public_key_hex))
            pub.verify(bytes.fromhex(att.signature_hex), msg)
        except Exception:
            rejected.append(f"{att.attestor}:invalid_ed25519_signature")
            continue

        valid_attestors.append(att.attestor)
        valid_ops.append(build_nav_reserve_attest_op(att))

    valid_attestors.sort()
    attestor_root_hasher = hashlib.sha3_384()
    attestor_root_hasher.update(b"postfiat.nav_attestor_quorum_root.v1")
    attestor_root_hasher.update(b"\x00")
    attestor_root_hasher.update(
        json.dumps(
            {
                "asset_id": asset_id,
                "epoch": epoch,
                "observation_root": expected_observation_root,
                "valid_attestors": valid_attestors,
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    )
    attestor_root = attestor_root_hasher.hexdigest()

    return AttestorQuorumReport(
        quorum_met=len(valid_attestors) >= min_attestations,
        valid_non_issuer_count=len(valid_attestors),
        required_quorum=min_attestations,
        valid_attestors=tuple(valid_attestors),
        rejected_reasons=tuple(rejected),
        attestor_root=attestor_root,
        l1_attest_operations=tuple(valid_ops),
    )
