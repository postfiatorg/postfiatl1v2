"""Zero-PII W3C Verifiable Credential (VC) webhook & issuer adapter for Pillar 8.

Verifies W3C VC Data Model v1.1 / v2.0 proofs off-chain using Ed25519
(`Ed25519Signature2020` / `DataIntegrityProof` with `eddsa-jcs-2022`),
discards all raw PII immediately after computing a salted SHA3-384 commitment,
and emits Contract C4 ``credential_create_and_accept`` & ``credential_revoke``
payloads.
"""

from __future__ import annotations

import hashlib
import hmac
import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any

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

W3C_CONTEXTS = frozenset(
    {
        "https://www.w3.org/2018/credentials/v1",
        "https://www.w3.org/ns/credentials/v2",
    }
)
SUPPORTED_PROOF_TYPES = frozenset({"Ed25519Signature2020", "DataIntegrityProof"})


@dataclass(frozen=True)
class TrustedVendorDid:
    vendor_did: str
    verification_method: str
    ed25519_public_key_hex: str


@dataclass(frozen=True)
class W3cVcAdapterConfig:
    chain_id: str
    genesis_hash: str
    issuer_id: str
    issuer_domain: str
    signing_account: str
    trusted_vendors: tuple[TrustedVendorDid, ...]
    vc_type_mapping: dict[str, str]
    default_ttl_blocks: int = 31_536_000  # 1 year at 1s nominal cadence
    webhook_hmac_bytes: bytes | None = None
    allowed_credential_types: tuple[str, ...] = field(default_factory=tuple)

    def validate(self) -> None:
        validate_hex96("genesis_hash", self.genesis_hash)
        validate_hex96("issuer_id", self.issuer_id)
        validate_canonical_pf_address("signing_account", self.signing_account)
        if not self.trusted_vendors:
            raise ValueError("trusted_vendors must contain at least one vendor DID")
        for mapped in self.vc_type_mapping.values():
            validate_namespaced_credential_type(self.issuer_domain, mapped)


def _canonical_vc_signing_bytes(vc: dict[str, Any]) -> bytes:
    unsigned = {k: v for k, v in vc.items() if k != "proof"}
    return json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sign_w3c_vc_ed25519(
    vc_body: dict[str, Any],
    *,
    key_seed_32: bytes,
    verification_method: str,
    created_iso: str,
    proof_type: str = "DataIntegrityProof",
) -> dict[str, Any]:
    """Helper to sign a W3C VC JSON document with Ed25519 for tests and sandboxes."""
    key = Ed25519PrivateKey.from_private_bytes(key_seed_32)
    payload = _canonical_vc_signing_bytes(vc_body)
    sig_hex = key.sign(payload).hex()
    proof: dict[str, Any] = {
        "type": proof_type,
        "created": created_iso,
        "proofPurpose": "assertionMethod",
        "verificationMethod": verification_method,
        "proofValueHex": sig_hex,
    }
    if proof_type == "DataIntegrityProof":
        proof["cryptosuite"] = "eddsa-jcs-2022"
    signed = dict(vc_body)
    signed["proof"] = proof
    return signed


def verify_webhook_hmac(raw_body: bytes, provided_hex: str, hmac_bytes: bytes) -> bool:
    expected = hmac.new(hmac_bytes, raw_body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, provided_hex.lower())


def _parse_iso_utc(raw: str, label: str) -> datetime:
    if not isinstance(raw, str) or not raw:
        raise ValueError(f"{label} must be an ISO-8601 timestamp string")
    dt = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


class W3cVcIssuerAdapter:
    """Off-chain W3C VC verifier issuing zero-PII Contract C4 credential payloads."""

    def __init__(
        self,
        config: W3cVcAdapterConfig,
        store: ZeroPiiCredentialStore,
        *,
        mldsa_verifier: MlDsaVerifierFn | None = None,
    ) -> None:
        config.validate()
        self.config = config
        self.store = store
        self.mldsa_verifier = mldsa_verifier
        self._vendors_by_did = {v.vendor_did: v for v in config.trusted_vendors}

    def verify_vc_and_derive_commitment(
        self,
        vc: dict[str, Any],
        *,
        now_utc: datetime,
        salt_bytes: bytes | None = None,
    ) -> dict[str, Any]:
        """Verify W3C VC structure, vendor Ed25519 proof, DID subject, and validity window.

        Returns ONLY zero-PII derived fields (subject, credential_type, commitment_hash,
        external_vc_id, valid_until_utc). Raw VC subject PII never leaves this method.
        """
        contexts = vc.get("@context")
        if not isinstance(contexts, list) or not any(c in W3C_CONTEXTS for c in contexts):
            raise ValueError("vc_invalid_context: missing W3C Verifiable Credentials context")

        vc_types = vc.get("type")
        if not isinstance(vc_types, list) or "VerifiableCredential" not in vc_types:
            raise ValueError("vc_invalid_type: must include VerifiableCredential")

        mapped_type: str | None = None
        for t in vc_types:
            if t in self.config.vc_type_mapping:
                mapped_type = self.config.vc_type_mapping[t]
                break
        if mapped_type is None:
            raise ValueError("vc_unsupported_credential_type")
        if (
            self.config.allowed_credential_types
            and mapped_type not in self.config.allowed_credential_types
        ):
            raise ValueError("issuer_invalid_credential_type: not in allowed_credential_types")

        issuer_field = vc.get("issuer")
        vendor_did = issuer_field.get("id") if isinstance(issuer_field, dict) else issuer_field
        vendor = self._vendors_by_did.get(str(vendor_did))
        if vendor is None:
            raise ValueError(f"vc_untrusted_issuer:{vendor_did}")

        proof = vc.get("proof")
        if not isinstance(proof, dict):
            raise ValueError("vc_missing_proof")
        proof_type = proof.get("type")
        if proof_type not in SUPPORTED_PROOF_TYPES:
            raise ValueError(f"vc_unsupported_proof_type:{proof_type}")
        if proof_type == "DataIntegrityProof" and proof.get("cryptosuite") != "eddsa-jcs-2022":
            raise ValueError("vc_unsupported_cryptosuite")
        if proof.get("proofPurpose") != "assertionMethod":
            raise ValueError("vc_invalid_proof_purpose")
        if proof.get("verificationMethod") != vendor.verification_method:
            raise ValueError("vc_verification_method_mismatch")

        sig_hex = proof.get("proofValueHex")
        if not isinstance(sig_hex, str) or len(sig_hex) != 128:
            raise ValueError("vc_invalid_proof_signature_format")
        pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(vendor.ed25519_public_key_hex))
        try:
            pub.verify(bytes.fromhex(sig_hex), _canonical_vc_signing_bytes(vc))
        except Exception as exc:
            raise ValueError("vc_invalid_ed25519_signature") from exc

        # Validate validity timestamps (supports both VC v1.1 and VC v2.0 field names).
        valid_from_raw = vc.get("validFrom") or vc.get("issuanceDate")
        valid_until_raw = vc.get("validUntil") or vc.get("expirationDate")
        valid_from = _parse_iso_utc(valid_from_raw, "validFrom")
        valid_until = _parse_iso_utc(valid_until_raw, "validUntil")
        now_z = now_utc.astimezone(timezone.utc)
        if now_z < valid_from:
            raise ValueError("vc_not_yet_valid")
        if now_z >= valid_until:
            raise ValueError("vc_expired")

        # Validate subject DID binding to did:pftl:<chain_id>:<account>.
        subject_obj = vc.get("credentialSubject")
        if not isinstance(subject_obj, dict) or not subject_obj.get("id"):
            raise ValueError("vc_missing_credential_subject_id")
        did_chain, subject_account = parse_did_pftl(str(subject_obj["id"]))
        if did_chain != self.config.chain_id:
            raise ValueError(f"vc_chain_mismatch:{did_chain}!={self.config.chain_id}")

        # Hash verified claims in memory with ephemeral salt -> discard raw PII immediately.
        commitment_hash = compute_salted_commitment(
            subject=subject_account,
            issuer_id=self.config.issuer_id,
            credential_type=mapped_type,
            verified_claims={
                "vendor_did": vendor.vendor_did,
                "vc_id": str(vc.get("id", "")),
                "vc_types": sorted(str(t) for t in vc_types),
                "subject_claims": subject_obj,
            },
            salt_bytes=salt_bytes,
        )

        return {
            "subject": subject_account,
            "credential_type": mapped_type,
            "commitment_hash": commitment_hash,
            "external_vc_id": str(vc.get("id", "")) or None,
            "valid_until_utc": valid_until,
        }

    def issue_from_vc(
        self,
        *,
        vc: dict[str, Any],
        subject_acceptance: SubjectDelegatedAcceptance,
        current_block_height: int,
        now_utc: datetime,
        expires_at_height: int | None = None,
        salt_bytes: bytes | None = None,
        request_nonce: str | None = None,
    ) -> dict[str, Any]:
        """Verify external W3C VC and emit a zero-PII ``credential_create_and_accept`` op."""
        verified = self.verify_vc_and_derive_commitment(vc, now_utc=now_utc, salt_bytes=salt_bytes)
        if subject_acceptance.subject != verified["subject"]:
            raise ValueError("subject_mismatch: delegated acceptance subject != VC did:pftl subject")

        if expires_at_height is None:
            remaining_sec = int(
                (verified["valid_until_utc"] - now_utc.astimezone(timezone.utc)).total_seconds()
            )
            ttl = min(remaining_sec, self.config.default_ttl_blocks)
            if ttl <= 0:
                raise ValueError("credential_invalid_expiry")
            expires_at_height = current_block_height + ttl

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
            provider="w3c-vc",
            issued_at_height=current_block_height,
            expires_at_height=expires_at_height,
            external_ref_id=verified["external_vc_id"],
            request_nonce=request_nonce,
        )
        result = {
            "schema": "postfiat.credential_adapter.issuance.v1",
            "adapter": "w3c-vc",
            "credential_id": credential_id,
            "l1_operation": op,
            "audit_record": record,
        }
        assert_zero_pii_structure(result)
        return result

    def revoke(
        self,
        *,
        credential_id: str | None = None,
        external_vc_id: str | None = None,
        revoked_at_height: int,
        reason_code: str,
    ) -> dict[str, Any]:
        """Revoke a previously issued credential and emit a C4 ``credential_revoke`` op."""
        record = self.store.record_revocation(
            credential_id=credential_id,
            external_ref_id=external_vc_id,
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
            "adapter": "w3c-vc",
            "credential_id": record["credential_id"],
            "l1_operation": op,
            "audit_record": record,
        }
        assert_zero_pii_structure(result)
        return result
