"""Contract C4 Pillar 8 credential wire models, domain hashes, and DID helpers.

Mirrors ``docs/specs/l1-upgrade-train-b-contract.md`` Sections 4.2-4.7 and
``crates/crypto_provider/src/address.rs`` byte-for-byte.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from typing import Any, Callable

CREDENTIAL_ISSUER_ID_DOMAIN = "postfiat.credential_issuer_id.v1"
CREDENTIAL_ID_DOMAIN = "postfiat.credential_id.v1"
CREDENTIAL_DELEGATED_ACCEPT_DOMAIN = "postfiat.credential.delegated_accept.v1"
CREDENTIAL_COMMITMENT_DOMAIN = "postfiat.credential.commitment.v1"
ADDRESS_DERIVATION_DOMAIN = "postfiat.address.v1"

CREDENTIAL_CREATE_TRANSACTION_KIND = "credential_create"
CREDENTIAL_ACCEPT_TRANSACTION_KIND = "credential_accept"
CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND = "credential_create_and_accept"
CREDENTIAL_REVOKE_TRANSACTION_KIND = "credential_revoke"
CREDENTIAL_DELETE_TRANSACTION_KIND = "credential_delete"
CREDENTIAL_DEPOSIT_ATOMS = 10_000

ML_DSA_65_PUBLIC_KEY_BYTES = 1_952
ML_DSA_65_PUBLIC_KEY_HEX_LEN = ML_DSA_65_PUBLIC_KEY_BYTES * 2
ML_DSA_65_SIGNATURE_BYTES = 3_309
ML_DSA_65_SIGNATURE_HEX_LEN = ML_DSA_65_SIGNATURE_BYTES * 2

_CANONICAL_PF_RE = re.compile(r"^pf[0-9a-f]{40}$")
_HEX96_RE = re.compile(r"^[0-9a-f]{96}$")
_LOWER_HEX_RE = re.compile(r"^[0-9a-f]+$")
_DOMAIN_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$")
_CRED_TYPE_SUFFIX_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")

MlDsaVerifierFn = Callable[[bytes, bytes, bytes], bool]


def _sha3_384_domain_hex(domain: str, payload: bytes) -> str:
    hasher = hashlib.sha3_384()
    hasher.update(domain.encode("utf-8"))
    hasher.update(b"\x00")
    hasher.update(payload)
    return hasher.hexdigest()


def validate_canonical_pf_address(label: str, value: str) -> str:
    if not isinstance(value, str) or not _CANONICAL_PF_RE.match(value):
        raise ValueError(f"{label} must be canonical lowercase pf + 40 hex characters")
    return value


def validate_hex96(label: str, value: str) -> str:
    if not isinstance(value, str) or not _HEX96_RE.match(value):
        raise ValueError(f"{label} must be 96 lowercase hex characters")
    return value


def validate_namespaced_credential_type(domain: str, credential_type: str) -> str:
    """Enforce Pillar 8 `<domain>/<type>` credential type namespacing."""
    if not _DOMAIN_RE.match(domain):
        raise ValueError(f"invalid issuer domain: {domain!r}")
    prefix = f"{domain}/"
    if not isinstance(credential_type, str) or not credential_type.startswith(prefix):
        raise ValueError(f"issuer_invalid_credential_type: must start with {prefix!r}")
    suffix = credential_type[len(prefix) :]
    if not _CRED_TYPE_SUFFIX_RE.match(suffix):
        raise ValueError(f"issuer_invalid_credential_type: invalid suffix {suffix!r}")
    return credential_type


def derive_address_from_pubkey_hex(pubkey_hex: str) -> str:
    """Exact mirror of ``postfiat_crypto_provider::address_from_public_key``."""
    if (
        not isinstance(pubkey_hex, str)
        or len(pubkey_hex) != ML_DSA_65_PUBLIC_KEY_HEX_LEN
        or not _LOWER_HEX_RE.match(pubkey_hex)
    ):
        raise ValueError(
            f"subject_pubkey_hex must be {ML_DSA_65_PUBLIC_KEY_HEX_LEN} lowercase hex chars (ML-DSA-65)"
        )
    digest = hashlib.sha3_384(
        ADDRESS_DERIVATION_DOMAIN.encode("utf-8") + b"\x00" + bytes.fromhex(pubkey_hex)
    ).digest()
    return "pf" + digest[:20].hex()


def derive_issuer_id(chain_id: str, admin_account: str, registration_nonce: int) -> str:
    validate_canonical_pf_address("admin_account", admin_account)
    if registration_nonce < 0:
        raise ValueError("registration_nonce must be non-negative")
    preimage = f"chain_id={chain_id}\nadmin={admin_account}\nnonce={registration_nonce}\n".encode("utf-8")
    return _sha3_384_domain_hex(CREDENTIAL_ISSUER_ID_DOMAIN, preimage)


def derive_credential_id(subject: str, issuer_id: str, credential_type: str) -> str:
    validate_canonical_pf_address("subject", subject)
    validate_hex96("issuer_id", issuer_id)
    if not credential_type:
        raise ValueError("credential_type must be non-empty")
    preimage = f"subject={subject}\nissuer_id={issuer_id}\ncredential_type={credential_type}\n".encode("utf-8")
    return _sha3_384_domain_hex(CREDENTIAL_ID_DOMAIN, preimage)


def parse_did_pftl(did: str) -> tuple[str, str]:
    """Parse ``did:pftl:<chain_id>:<canonical_pf_account>`` per Contract C4 §4.7."""
    if not isinstance(did, str) or not did.startswith("did:pftl:"):
        raise ValueError("DID must use the did:pftl:<chain_id>:<account> method")
    parts = did.split(":")
    if len(parts) != 4 or not parts[2]:
        raise ValueError("malformed did:pftl syntax")
    chain_id = parts[2]
    account = validate_canonical_pf_address("did.account", parts[3])
    return chain_id, account


def format_did_pftl(chain_id: str, account: str) -> str:
    validate_canonical_pf_address("account", account)
    if not chain_id:
        raise ValueError("chain_id must be non-empty")
    return f"did:pftl:{chain_id}:{account}"


def delegated_accept_preimage(
    *,
    chain_id: str,
    genesis_hash: str,
    subject: str,
    issuer_id: str,
    credential_type: str,
    commitment_hash: str,
    expires_at_height: int,
    subject_accept_nonce: int,
) -> bytes:
    """Exact Contract C4 §4.5 ML-DSA-65 delegated acceptance preimage."""
    validate_hex96("genesis_hash", genesis_hash)
    validate_canonical_pf_address("subject", subject)
    validate_hex96("issuer_id", issuer_id)
    validate_hex96("commitment_hash", commitment_hash)
    if expires_at_height <= 0 or subject_accept_nonce < 0:
        raise ValueError("expires_at_height must be > 0 and subject_accept_nonce >= 0")
    return (
        f"{CREDENTIAL_DELEGATED_ACCEPT_DOMAIN}\n"
        f"chain_id={chain_id}\n"
        f"genesis_hash={genesis_hash}\n"
        f"subject={subject}\n"
        f"issuer_id={issuer_id}\n"
        f"credential_type={credential_type}\n"
        f"commitment_hash={commitment_hash}\n"
        f"expires_at_height={expires_at_height}\n"
        f"subject_accept_nonce={subject_accept_nonce}\n"
    ).encode("utf-8")


@dataclass(frozen=True)
class SubjectDelegatedAcceptance:
    """Subject's off-chain ML-DSA-65 proof-of-control & acceptance signature."""

    subject: str
    subject_pubkey_hex: str
    subject_accept_signature_hex: str
    subject_accept_nonce: int


def verify_subject_delegated_acceptance(
    *,
    acceptance: SubjectDelegatedAcceptance,
    chain_id: str,
    genesis_hash: str,
    issuer_id: str,
    credential_type: str,
    commitment_hash: str,
    expires_at_height: int,
    mldsa_verifier: MlDsaVerifierFn | None = None,
) -> bytes:
    """Validate subject address derivation, FIPS-204 lengths, and optional ML-DSA-65 sig."""
    validate_canonical_pf_address("subject", acceptance.subject)
    derived = derive_address_from_pubkey_hex(acceptance.subject_pubkey_hex)
    if derived != acceptance.subject:
        raise ValueError(
            "credential_delegated_accept_invalid: subject_pubkey_hex does not derive to subject"
        )
    sig_hex = acceptance.subject_accept_signature_hex
    if (
        not isinstance(sig_hex, str)
        or len(sig_hex) != ML_DSA_65_SIGNATURE_HEX_LEN
        or not _LOWER_HEX_RE.match(sig_hex)
    ):
        raise ValueError(
            f"credential_delegated_accept_invalid: signature must be {ML_DSA_65_SIGNATURE_HEX_LEN} lowercase hex chars"
        )
    preimage = delegated_accept_preimage(
        chain_id=chain_id,
        genesis_hash=genesis_hash,
        subject=acceptance.subject,
        issuer_id=issuer_id,
        credential_type=credential_type,
        commitment_hash=commitment_hash,
        expires_at_height=expires_at_height,
        subject_accept_nonce=acceptance.subject_accept_nonce,
    )
    if mldsa_verifier is not None:
        ok = mldsa_verifier(
            bytes.fromhex(acceptance.subject_pubkey_hex),
            preimage,
            bytes.fromhex(sig_hex),
        )
        if not ok:
            raise ValueError("credential_delegated_accept_invalid: ML-DSA-65 signature verification failed")
    return preimage


def build_credential_create_and_accept_op(
    *,
    signing_account: str,
    issuer_id: str,
    domain: str,
    subject: str,
    credential_type: str,
    commitment_hash: str,
    expires_at_height: int,
    current_block_height: int,
    subject_pubkey_hex: str,
    subject_accept_signature_hex: str,
    subject_accept_nonce: int,
    chain_id: str,
    genesis_hash: str,
    mldsa_verifier: MlDsaVerifierFn | None = None,
) -> dict[str, Any]:
    """Build and validate a Contract C4 ``credential_create_and_accept`` operation."""
    validate_canonical_pf_address("signing_account", signing_account)
    validate_hex96("issuer_id", issuer_id)
    validate_namespaced_credential_type(domain, credential_type)
    validate_hex96("commitment_hash", commitment_hash)
    if expires_at_height <= current_block_height:
        raise ValueError("credential_invalid_expiry: expires_at_height must exceed current_block_height")

    verify_subject_delegated_acceptance(
        acceptance=SubjectDelegatedAcceptance(
            subject=subject,
            subject_pubkey_hex=subject_pubkey_hex,
            subject_accept_signature_hex=subject_accept_signature_hex,
            subject_accept_nonce=subject_accept_nonce,
        ),
        chain_id=chain_id,
        genesis_hash=genesis_hash,
        issuer_id=issuer_id,
        credential_type=credential_type,
        commitment_hash=commitment_hash,
        expires_at_height=expires_at_height,
        mldsa_verifier=mldsa_verifier,
    )

    return {
        "operation": CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND,
        "signing_account": signing_account,
        "issuer_id": issuer_id,
        "subject": subject,
        "credential_type": credential_type,
        "commitment_hash": commitment_hash,
        "expires_at_height": expires_at_height,
        "subject_pubkey_hex": subject_pubkey_hex,
        "subject_accept_signature_hex": subject_accept_signature_hex,
        "subject_accept_nonce": subject_accept_nonce,
    }


def build_credential_revoke_op(
    *,
    caller: str,
    credential_id: str,
    reason_code: str,
) -> dict[str, Any]:
    """Build a Contract C4 ``credential_revoke`` operation."""
    validate_canonical_pf_address("caller", caller)
    validate_hex96("credential_id", credential_id)
    if not isinstance(reason_code, str) or not reason_code or len(reason_code) > 64:
        raise ValueError("reason_code must be a non-empty string <= 64 chars")
    return {
        "operation": CREDENTIAL_REVOKE_TRANSACTION_KIND,
        "caller": caller,
        "credential_id": credential_id,
        "reason_code": reason_code,
    }


def build_credential_delete_op(*, caller: str, credential_id: str) -> dict[str, Any]:
    """Build a Contract C4 ``credential_delete`` operation."""
    validate_canonical_pf_address("caller", caller)
    validate_hex96("credential_id", credential_id)
    return {
        "operation": CREDENTIAL_DELETE_TRANSACTION_KIND,
        "caller": caller,
        "credential_id": credential_id,
    }
