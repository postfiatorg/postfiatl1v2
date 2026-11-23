"""Policy binding around platform-native TEE attestation verification."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Protocol

from .attestation import AttestedStatement
from .canonical import sha256_bytes


HardwarePlatform = Literal["flare-fcc", "aws-nitro"]


@dataclass(frozen=True)
class NativeAttestationClaims:
    """Claims returned only after a platform adapter verifies native evidence."""

    platform: HardwarePlatform
    hardware_backed: bool
    debug_mode: bool
    measurement: str
    statement_key: str

    def __post_init__(self) -> None:
        if self.platform not in {"flare-fcc", "aws-nitro"}:
            raise ValueError("native attestation claims require a hardware platform")
        if not isinstance(self.hardware_backed, bool) or not isinstance(
            self.debug_mode, bool
        ):
            raise ValueError("native attestation flags must be boolean")
        if not isinstance(self.measurement, str) or not self.measurement:
            raise ValueError("native attestation measurement is required")
        if not isinstance(self.statement_key, str) or not self.statement_key:
            raise ValueError("native attestation statement key is required")


class AttestationDocumentSource(Protocol):
    """Resolve a private/native evidence document by its public digest."""

    def get_document(self, document_sha256: str) -> bytes: ...


class NativeAttestationVerifier(Protocol):
    """Verify the native certificate/signature chain and return trusted claims."""

    def verify_and_extract(
        self,
        document: bytes,
        *,
        attestation_binding_digest: str,
    ) -> NativeAttestationClaims: ...


class StatementSignatureVerifier(Protocol):
    """Verify a statement signature under the attested enclave key."""

    def verify(
        self,
        *,
        statement_key: str,
        signing_digest: str,
        signature: str,
    ) -> bool: ...


@dataclass(frozen=True)
class TeeMeasurementPolicy:
    verifier_id: str
    platform: HardwarePlatform
    allowed_measurements: tuple[str, ...]

    def __post_init__(self) -> None:
        if not isinstance(self.verifier_id, str) or not self.verifier_id:
            raise ValueError("TEE verifier_id is required")
        if self.platform not in {"flare-fcc", "aws-nitro"}:
            raise ValueError("TEE measurement policy requires a hardware platform")
        if not self.allowed_measurements:
            raise ValueError("TEE measurement policy requires approved measurements")
        if tuple(sorted(set(self.allowed_measurements))) != self.allowed_measurements:
            raise ValueError("approved TEE measurements must be sorted and unique")
        if any(
            not isinstance(measurement, str) or not measurement
            for measurement in self.allowed_measurements
        ):
            raise ValueError("approved TEE measurements contain an invalid value")


class PolicyBoundTeeEvidenceVerifier:
    """Bind verified native claims and signatures to an approved TEE policy."""

    def __init__(
        self,
        policy: TeeMeasurementPolicy,
        *,
        document_source: AttestationDocumentSource,
        native_verifier: NativeAttestationVerifier,
        signature_verifier: StatementSignatureVerifier,
    ) -> None:
        self.policy = policy
        self.verifier_id = policy.verifier_id
        self.document_source = document_source
        self.native_verifier = native_verifier
        self.signature_verifier = signature_verifier

    def verify(self, statement: AttestedStatement) -> None:
        evidence = statement.evidence
        if evidence.verifier_id != self.verifier_id:
            raise ValueError("TEE evidence verifier identity does not match policy")
        if evidence.platform != self.policy.platform:
            raise ValueError("TEE evidence platform does not match policy")
        if evidence.trust_class != "attested" or not evidence.hardware_backed:
            raise ValueError("TEE evidence is not hardware-backed attested evidence")
        if evidence.debug_mode:
            raise ValueError("debug-mode TEE evidence is not accepted")

        document = self.document_source.get_document(
            evidence.attestation_document_sha256
        )
        if not isinstance(document, bytes) or not document:
            raise ValueError("TEE attestation document is unavailable")
        if sha256_bytes(document) != evidence.attestation_document_sha256:
            raise ValueError("TEE attestation document digest does not match evidence")
        claims = self.native_verifier.verify_and_extract(
            document,
            attestation_binding_digest=statement.attestation_binding_digest,
        )
        if claims.platform != evidence.platform:
            raise ValueError("native attestation platform does not match evidence")
        if not claims.hardware_backed or claims.debug_mode:
            raise ValueError("native attestation does not prove non-debug hardware")
        if claims.measurement != evidence.measurement:
            raise ValueError("native attestation measurement does not match evidence")
        if claims.measurement not in self.policy.allowed_measurements:
            raise ValueError("native attestation measurement is not approved")
        if claims.statement_key != evidence.statement_key:
            raise ValueError("native attestation statement key does not match evidence")
        if not self.signature_verifier.verify(
            statement_key=claims.statement_key,
            signing_digest=statement.signing_digest,
            signature=statement.signature,
        ):
            raise ValueError("attested statement signature is invalid")
