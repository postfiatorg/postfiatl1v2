"""Zero-PII ZK-Passport sandbox verifier adapter (Self / OpenPassport compatible).

Verifies off-chain ZK-passport proof envelopes (zero SNARKs in L1 consensus, per
Pillar 8 locked design), checks trusted ICAO/OFAC roots, age/sanctions signals,
and anti-Sybil nullifier uniqueness, then issues a Contract C4 zero-PII
``credential_create_and_accept`` payload.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Any, Callable

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

from .types_c4 import (
    MlDsaVerifierFn,
    SubjectDelegatedAcceptance,
    build_credential_create_and_accept_op,
    build_credential_revoke_op,
    derive_credential_id,
    parse_did_pftl,
    validate_canonical_pf_address,
    validate_hex96,
    validate_namespaced_credential_type,
)
from .zero_pii_store import ZeroPiiCredentialStore, assert_zero_pii_structure, compute_salted_commitment

SUPPORTED_ZKP_PROVIDERS = frozenset({"self", "openpassport", "zkpassport-sandbox"})
ZKP_SANDBOX_ENVELOPE_DOMAIN = "postfiat.zkpassport.sandbox_proof.v1"

Groth16VerifierFn = Callable[[str, dict[str, Any], dict[str, Any]], bool]


@dataclass(frozen=True)
class ZkPassportAdapterConfig:
    chain_id: str
    genesis_hash: str
    issuer_id: str
    issuer_domain: str
    signing_account: str
    credential_type: str
    expected_scope: str
    trusted_icao_roots: tuple[str, ...]
    trusted_ofac_roots: tuple[str, ...]
    sandbox_verifier_pubkey_hex: str
    required_minimum_age: int = 18
    default_ttl_blocks: int = 31_536_000
    allowed_circuits: tuple[str, ...] = ("self-passport-disclose-v1", "openpassport-disclose-v1")
    allow_sandbox_envelope: bool = True

    def validate(self) -> None:
        validate_hex96("genesis_hash", self.genesis_hash)
        validate_hex96("issuer_id", self.issuer_id)
        validate_canonical_pf_address("signing_account", self.signing_account)
        validate_namespaced_credential_type(self.issuer_domain, self.credential_type)
        if not self.expected_scope:
            raise ValueError("expected_scope must be non-empty")
        if not self.trusted_icao_roots or not self.trusted_ofac_roots:
            raise ValueError("trusted_icao_roots and trusted_ofac_roots must be non-empty")
        if len(bytes.fromhex(self.sandbox_verifier_pubkey_hex)) != 32:
            raise ValueError("sandbox_verifier_pubkey_hex must be 32 bytes")


def _sandbox_envelope_digest(envelope: dict[str, Any]) -> bytes:
    body = {
        "provider": envelope["provider"],
        "circuit_id": envelope["circuit_id"],
        "public_signals": envelope["public_signals"],
        "proof_digest_hex": envelope["proof_digest_hex"],
    }
    canonical = json.dumps(body, sort_keys=True, separators=(",", ":")).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(ZKP_SANDBOX_ENVELOPE_DOMAIN.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.digest()


def build_signed_sandbox_zkp_envelope(
    *,
    provider: str,
    circuit_id: str,
    public_signals: dict[str, Any],
    proof_payload: dict[str, Any],
    sandbox_signer_seed_32: bytes,
) -> dict[str, Any]:
    """Create a deterministic signed sandbox ZK-passport proof envelope for UAT/tests."""
    proof_digest_hex = hashlib.sha3_384(
        json.dumps(proof_payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    envelope = {
        "provider": provider,
        "circuit_id": circuit_id,
        "public_signals": public_signals,
        "proof": proof_payload,
        "proof_digest_hex": proof_digest_hex,
    }
    key = Ed25519PrivateKey.from_private_bytes(sandbox_signer_seed_32)
    envelope["sandbox_attestation_sig_hex"] = key.sign(_sandbox_envelope_digest(envelope)).hex()
    return envelope


class ZkPassportSandboxAdapter:
    """Off-chain ZK-Passport sandbox verifier emitting Contract C4 zero-PII payloads."""

    def __init__(
        self,
        config: ZkPassportAdapterConfig,
        store: ZeroPiiCredentialStore,
        *,
        mldsa_verifier: MlDsaVerifierFn | None = None,
        groth16_verifier: Groth16VerifierFn | None = None,
    ) -> None:
        config.validate()
        self.config = config
        self.store = store
        self.mldsa_verifier = mldsa_verifier
        self.groth16_verifier = groth16_verifier

    def verify_proof_envelope(
        self,
        envelope: dict[str, Any],
        *,
        now_unix: int,
        salt_bytes: bytes | None = None,
    ) -> dict[str, Any]:
        """Verify ZK-passport proof envelope and derive a zero-PII commitment."""
        provider = str(envelope.get("provider", ""))
        if provider not in SUPPORTED_ZKP_PROVIDERS:
            raise ValueError(f"zkp_unsupported_provider:{provider}")
        circuit_id = str(envelope.get("circuit_id", ""))
        if circuit_id not in self.config.allowed_circuits:
            raise ValueError(f"zkp_unsupported_circuit:{circuit_id}")

        signals = envelope.get("public_signals")
        if not isinstance(signals, dict):
            raise ValueError("zkp_missing_public_signals")

        # Verify proof payload digest and either external Groth16 verifier or sandbox attestation.
        proof_obj = envelope.get("proof")
        if not isinstance(proof_obj, dict):
            raise ValueError("zkp_missing_proof")
        expected_digest = hashlib.sha3_384(
            json.dumps(proof_obj, sort_keys=True, separators=(",", ":")).encode("utf-8")
        ).hexdigest()
        if envelope.get("proof_digest_hex") != expected_digest:
            raise ValueError("zkp_proof_digest_mismatch")

        if self.groth16_verifier is not None:
            if not self.groth16_verifier(circuit_id, proof_obj, signals):
                raise ValueError("zkp_groth16_verification_failed")
        elif self.config.allow_sandbox_envelope:
            sig_hex = envelope.get("sandbox_attestation_sig_hex")
            if not isinstance(sig_hex, str) or len(sig_hex) != 128:
                raise ValueError("zkp_missing_sandbox_signature")
            pub = Ed25519PublicKey.from_public_bytes(
                bytes.fromhex(self.config.sandbox_verifier_pubkey_hex)
            )
            try:
                pub.verify(bytes.fromhex(sig_hex), _sandbox_envelope_digest(envelope))
            except Exception as exc:
                raise ValueError("zkp_invalid_sandbox_signature") from exc
        else:
            raise ValueError("zkp_verifier_unavailable")

        # Validate public circuit assertions (zero personal data required!).
        if signals.get("scope") != self.config.expected_scope:
            raise ValueError("zkp_scope_mismatch")
        if signals.get("merkle_root") not in self.config.trusted_icao_roots:
            raise ValueError("zkp_untrusted_icao_root")
        if signals.get("ofac_root") not in self.config.trusted_ofac_roots:
            raise ValueError("zkp_untrusted_ofac_root")
        if signals.get("age_verified") is not True:
            raise ValueError("zkp_age_check_failed")
        if int(signals.get("minimum_age", 0)) < self.config.required_minimum_age:
            raise ValueError("zkp_minimum_age_below_policy")
        if signals.get("ofac_passed") is not True:
            raise ValueError("zkp_ofac_check_failed")
        if signals.get("nationality_excluded") is True:
            raise ValueError("zkp_excluded_jurisdiction")

        proof_expiry = int(signals.get("proof_expires_at_unix", 0))
        if proof_expiry <= now_unix:
            raise ValueError("zkp_proof_expired")

        nullifier = str(signals.get("nullifier", ""))
        if len(nullifier) < 32:
            raise ValueError("zkp_invalid_nullifier")

        did_chain, subject_account = parse_did_pftl(str(signals.get("user_identifier", "")))
        if did_chain != self.config.chain_id:
            raise ValueError(f"zkp_chain_mismatch:{did_chain}!={self.config.chain_id}")

        commitment_hash = compute_salted_commitment(
            subject=subject_account,
            issuer_id=self.config.issuer_id,
            credential_type=self.config.credential_type,
            verified_claims={
                "provider": provider,
                "circuit_id": circuit_id,
                "merkle_root": signals["merkle_root"],
                "ofac_root": signals["ofac_root"],
                "minimum_age": int(signals["minimum_age"]),
                "ofac_passed": True,
                "nullifier": nullifier,
            },
            salt_bytes=salt_bytes,
        )

        return {
            "provider": provider,
            "subject": subject_account,
            "credential_type": self.config.credential_type,
            "commitment_hash": commitment_hash,
            "nullifier": nullifier,
        }

    def verify_and_issue(
        self,
        *,
        proof_envelope: dict[str, Any],
        subject_acceptance: SubjectDelegatedAcceptance,
        current_block_height: int,
        now_unix: int,
        expires_at_height: int | None = None,
        salt_bytes: bytes | None = None,
        request_nonce: str | None = None,
    ) -> dict[str, Any]:
        """Verify ZK-passport proof and emit a zero-PII ``credential_create_and_accept`` op."""
        verified = self.verify_proof_envelope(
            proof_envelope,
            now_unix=now_unix,
            salt_bytes=salt_bytes,
        )
        if subject_acceptance.subject != verified["subject"]:
            raise ValueError("subject_mismatch: delegated acceptance subject != ZKP user_identifier")

        if expires_at_height is None:
            expires_at_height = current_block_height + self.config.default_ttl_blocks

        op = build_credential_create_and_accept_op(
            signing_account=self.config.signing_account,
            issuer_id=self.config.issuer_id,
            domain=self.config.issuer_domain,
            subject=verified["subject"],
            credential_type=verified["credential_type"],
            commitment_hash=verified["commitment_hash"],
            expires_at_height=expires_at_height,
            current_block_height=current_block_height,
            subject_pubkey_hex=subject_acceptance.subject_pubkey_hex,
            subject_accept_signature_hex=subject_acceptance.subject_accept_signature_hex,
            subject_accept_nonce=subject_acceptance.subject_accept_nonce,
            chain_id=self.config.chain_id,
            genesis_hash=self.config.genesis_hash,
            mldsa_verifier=self.mldsa_verifier,
        )
        credential_id = derive_credential_id(
            verified["subject"],
            self.config.issuer_id,
            verified["credential_type"],
        )

        record = self.store.record_issuance(
            credential_id=credential_id,
            subject=verified["subject"],
            issuer_id=self.config.issuer_id,
            credential_type=verified["credential_type"],
            commitment_hash=verified["commitment_hash"],
            provider=f"zk-passport:{verified['provider']}",
            issued_at_height=current_block_height,
            expires_at_height=expires_at_height,
            raw_nullifier=verified["nullifier"],
            request_nonce=request_nonce,
        )
        result = {
            "schema": "postfiat.credential_adapter.issuance.v1",
            "adapter": "zk-passport-sandbox",
            "credential_id": credential_id,
            "l1_operation": op,
            "audit_record": record,
        }
        assert_zero_pii_structure(result)
        return result

    def revoke(
        self,
        *,
        credential_id: str,
        revoked_at_height: int,
        reason_code: str,
    ) -> dict[str, Any]:
        record = self.store.record_revocation(
            credential_id=credential_id,
            revoked_at_height=revoked_at_height,
            reason_code=reason_code,
        )
        op = build_credential_revoke_op(
            caller=self.config.signing_account,
            credential_id=record["credential_id"],
            reason_code=reason_code,
        )
        result = {
            "schema": "postfiat.credential_adapter.revocation.v1",
            "adapter": "zk-passport-sandbox",
            "credential_id": record["credential_id"],
            "l1_operation": op,
            "audit_record": record,
        }
        assert_zero_pii_structure(result)
        return result
