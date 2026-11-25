"""Provider-neutral TEE evidence envelopes for YOLO statements.

Cryptographic platform verification belongs in a platform adapter. This module
defines the common result that Flare FCC, AWS Nitro, or another reviewed adapter
must produce; it never treats a local simulation as hardware evidence.
"""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Mapping
from typing import Any, Literal, Protocol

from .canonical import domain_sha256
from .models import CollectionEpoch, SnapshotCommitment, assess_collection


ATTESTED_STATEMENT_SCHEMA = "postfiat.yolo.attested_statement.v1"
TEE_EVIDENCE_SCHEMA = "postfiat.yolo.tee_evidence.v1"
ATTESTED_COLLECTION_SCHEMA = "postfiat.yolo.attested_collection.v1"
STATEMENT_ATTESTATION_BINDING_SCHEMA = "postfiat.yolo.statement_attestation_binding.v1"

TeePlatform = Literal["local-simulated", "flare-fcc", "aws-nitro"]
TrustClass = Literal["controlled", "attested"]


def _digest(value: str, field: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    original = str(value)
    normalized = original.lower()
    if len(normalized) != 64 or any(
        character not in "0123456789abcdef" for character in normalized
    ) or normalized != original:
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    return normalized


def _strict_object(value: Any, *, fields: set[str], name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    unknown = set(value) - fields
    missing = fields - set(value)
    if unknown:
        raise ValueError(f"{name} has unknown field {sorted(unknown)[0]!r}")
    if missing:
        raise ValueError(f"{name} is missing field {sorted(missing)[0]!r}")
    return value


@dataclass(frozen=True)
class TeeEvidence:
    platform: TeePlatform
    trust_class: TrustClass
    hardware_backed: bool
    debug_mode: bool
    measurement: str
    statement_key: str
    attestation_document_sha256: str
    verifier_id: str

    def __post_init__(self) -> None:
        if self.platform not in {"local-simulated", "flare-fcc", "aws-nitro"}:
            raise ValueError("unsupported TEE platform")
        if self.trust_class not in {"controlled", "attested"}:
            raise ValueError("unsupported TEE trust class")
        if not isinstance(self.hardware_backed, bool) or not isinstance(
            self.debug_mode, bool
        ):
            raise ValueError("TEE hardware/debug flags must be boolean")
        if self.platform == "local-simulated":
            if self.hardware_backed or self.trust_class != "controlled":
                raise ValueError(
                    "local simulation must be controlled and not hardware-backed"
                )
        elif not self.hardware_backed or self.trust_class != "attested":
            raise ValueError(
                "Flare FCC and AWS Nitro evidence must be hardware-backed and attested"
            )
        if self.debug_mode and self.trust_class == "attested":
            raise ValueError("debug-mode evidence cannot be classified as attested")
        if not isinstance(self.measurement, str) or not self.measurement:
            raise ValueError("TEE measurement is required")
        if not isinstance(self.statement_key, str) or not self.statement_key:
            raise ValueError("TEE statement key is required")
        if not isinstance(self.verifier_id, str) or not self.verifier_id:
            raise ValueError("TEE verifier identity is required")
        _digest(
            self.attestation_document_sha256,
            "attestation_document_sha256",
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": TEE_EVIDENCE_SCHEMA,
            "platform": self.platform,
            "trustClass": self.trust_class,
            "hardwareBacked": self.hardware_backed,
            "debugMode": self.debug_mode,
            "measurement": self.measurement,
            "statementKey": self.statement_key,
            "attestationDocumentSha256": self.attestation_document_sha256,
            "verifierId": self.verifier_id,
        }

    @classmethod
    def from_dict(cls, value: Any) -> TeeEvidence:
        evidence = _strict_object(
            value,
            fields={
                "schema",
                "platform",
                "trustClass",
                "hardwareBacked",
                "debugMode",
                "measurement",
                "statementKey",
                "attestationDocumentSha256",
                "verifierId",
            },
            name="TEE evidence",
        )
        if evidence["schema"] != TEE_EVIDENCE_SCHEMA:
            raise ValueError("TEE evidence schema mismatch")
        return cls(
            platform=evidence["platform"],
            trust_class=evidence["trustClass"],
            hardware_backed=evidence["hardwareBacked"],
            debug_mode=evidence["debugMode"],
            measurement=evidence["measurement"],
            statement_key=evidence["statementKey"],
            attestation_document_sha256=evidence["attestationDocumentSha256"],
            verifier_id=evidence["verifierId"],
        )


@dataclass(frozen=True)
class AttestedStatement:
    statement_kind: str
    epoch_sha256: str
    input_sha256: str
    output_sha256: str
    sequence: int
    previous_statement_sha256: str | None
    signature: str
    evidence: TeeEvidence

    def __post_init__(self) -> None:
        if not isinstance(self.statement_kind, str) or not self.statement_kind:
            raise ValueError("statement_kind is required")
        for field in ("epoch_sha256", "input_sha256", "output_sha256"):
            _digest(getattr(self, field), field)
        if self.previous_statement_sha256 is not None:
            _digest(self.previous_statement_sha256, "previous_statement_sha256")
        if (
            not isinstance(self.sequence, int)
            or isinstance(self.sequence, bool)
            or self.sequence < 0
        ):
            raise ValueError("statement sequence must be non-negative")
        if not isinstance(self.signature, str) or not self.signature:
            raise ValueError("statement signature is required")
        if not isinstance(self.evidence, TeeEvidence):
            raise ValueError("statement evidence must be a TeeEvidence record")

    def unsigned_dict(self) -> dict[str, Any]:
        return {
            "schema": ATTESTED_STATEMENT_SCHEMA,
            "statementKind": self.statement_kind,
            "epochSha256": self.epoch_sha256,
            "inputSha256": self.input_sha256,
            "outputSha256": self.output_sha256,
            "sequence": self.sequence,
            "previousStatementSha256": self.previous_statement_sha256,
            "evidence": self.evidence.to_dict(),
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self.unsigned_dict(), "signature": self.signature}

    @classmethod
    def from_dict(cls, value: Any) -> AttestedStatement:
        statement = _strict_object(
            value,
            fields={
                "schema",
                "statementKind",
                "epochSha256",
                "inputSha256",
                "outputSha256",
                "sequence",
                "previousStatementSha256",
                "evidence",
                "signature",
            },
            name="attested statement",
        )
        if statement["schema"] != ATTESTED_STATEMENT_SCHEMA:
            raise ValueError("attested statement schema mismatch")
        return cls(
            statement_kind=statement["statementKind"],
            epoch_sha256=statement["epochSha256"],
            input_sha256=statement["inputSha256"],
            output_sha256=statement["outputSha256"],
            sequence=statement["sequence"],
            previous_statement_sha256=statement["previousStatementSha256"],
            signature=statement["signature"],
            evidence=TeeEvidence.from_dict(statement["evidence"]),
        )

    @property
    def attestation_binding_digest(self) -> str:
        """Bind the NSM request before its returned document exists.

        The final signing digest still covers the document hash. This separate
        versioned preimage prevents the document from having to contain a hash
        of itself while binding every other unsigned statement field.
        """
        value = self.unsigned_dict()
        value["schema"] = STATEMENT_ATTESTATION_BINDING_SCHEMA
        del value["evidence"]["attestationDocumentSha256"]
        return domain_sha256(STATEMENT_ATTESTATION_BINDING_SCHEMA, value)

    @property
    def signing_digest(self) -> str:
        return domain_sha256(ATTESTED_STATEMENT_SCHEMA, self.unsigned_dict())

    @property
    def sha256(self) -> str:
        return domain_sha256(ATTESTED_STATEMENT_SCHEMA, self.to_dict())


class TeeEvidenceVerifier(Protocol):
    """Platform adapter contract; implementations verify native evidence."""

    verifier_id: str

    def verify(self, statement: AttestedStatement) -> None: ...


def verify_attested_collection(
    epoch: CollectionEpoch,
    commitments: tuple[SnapshotCommitment, ...],
    statements: tuple[AttestedStatement, ...],
    *,
    verifiers: Mapping[str, TeeEvidenceVerifier],
) -> str:
    """Verify a complete snapshot set and its provider-specific attestations."""

    status = assess_collection(epoch, commitments)
    if status.state != "COMPLETE":
        detail = ",".join(status.issues) if status.issues else status.state
        raise ValueError(f"collection is not complete: {detail}")
    ordered_commitments = tuple(sorted(commitments, key=lambda item: item.sequence))
    ordered_statements = tuple(sorted(statements, key=lambda item: item.sequence))
    expected_sequences = tuple(range(epoch.expected_snapshot_count))
    statement_sequences = tuple(item.sequence for item in ordered_statements)
    if statement_sequences != expected_sequences:
        raise ValueError("attested statements do not match the epoch sequence")
    previous_statement_sha256: str | None = None
    for commitment, statement in zip(
        ordered_commitments,
        ordered_statements,
        strict=True,
    ):
        if statement.statement_kind != "option-chain-snapshot":
            raise ValueError("attested statement kind is not option-chain-snapshot")
        if statement.epoch_sha256 != epoch.sha256:
            raise ValueError("attested statement epoch does not match")
        if statement.input_sha256 != commitment.private_snapshot_sha256:
            raise ValueError("attested statement input does not match private snapshot")
        if statement.output_sha256 != commitment.sha256:
            raise ValueError("attested statement output does not match commitment")
        if statement.sequence != commitment.sequence:
            raise ValueError("attested statement sequence does not match commitment")
        if statement.previous_statement_sha256 != previous_statement_sha256:
            raise ValueError("attested statement chain is broken")
        verifier_id = statement.evidence.verifier_id
        verifier = verifiers.get(verifier_id)
        if verifier is None:
            raise ValueError(f"no verifier was supplied for {verifier_id!r}")
        if verifier.verifier_id != verifier_id:
            raise ValueError("supplied verifier identity does not match evidence")
        verifier.verify(statement)
        previous_statement_sha256 = statement.sha256
    return domain_sha256(
        ATTESTED_COLLECTION_SCHEMA,
        {
            "schema": ATTESTED_COLLECTION_SCHEMA,
            "epochSha256": epoch.sha256,
            "commitmentsSha256": status.commitments_sha256,
            "statementSha256s": [item.sha256 for item in ordered_statements],
        },
    )
