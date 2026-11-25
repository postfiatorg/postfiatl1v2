"""Public-value schemas for provider-neutral YOLO collection proofs."""

from __future__ import annotations

from dataclasses import dataclass
import struct
from typing import Any

from .canonical import domain_sha256
from .models import CollectionEpoch, SnapshotCommitment, assess_collection


COLLECTION_PROOF_PUBLIC_VALUES_SCHEMA = (
    "postfiat.yolo.collection_proof_public_values.v2"
)
NORMALIZED_INPUT_ROOT_SCHEMA = "postfiat.yolo.normalized_input_root.v2"
PFTL_COLLECTION_PUBLIC_VALUES_SCHEMA = "postfiat.yolo.collection_public_values.v1"
PFTL_COLLECTION_PUBLIC_VALUES_MAGIC = b"PFTYCOL1"
PFTL_COLLECTION_PUBLIC_VALUES_VERSION = 1
PFTL_COLLECTION_PUBLIC_VALUES_BYTES = 304
PFTL_COLLECTION_MAX_SNAPSHOTS = 64
SOURCE_ID_COMMITMENT_DOMAIN = "postfiat.yolo.source_id.v1"
PFTL_TARGET_PUBLIC_VALUES_SCHEMA = "postfiat.yolo.target_public_values.v1"
PFTL_TARGET_PUBLIC_VALUES_MAGIC = b"PFTYTGT1"
PFTL_TARGET_PUBLIC_VALUES_VERSION = 1
PFTL_TARGET_PUBLIC_VALUES_BYTES = 408
PFTL_TARGET_MAX_SNAPSHOTS = 64
PFTL_TARGET_MAX_SELECTED_CONTRACTS = 64
PFTL_TARGET_STATUSES = (
    "TARGET_COMPUTED",
    "NO_RECONSTITUTION",
    "NO_ELIGIBLE_SUCCESSOR",
    "HALTED_INSTRUMENT",
    "UNRESOLVED_CAPITAL_POLICY",
)


def _digest(value: str, field: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    original = str(value)
    if len(original) != 64 or any(
        character not in "0123456789abcdef" for character in original
    ):
        raise ValueError(f"{field} must be a lowercase SHA-256 hex digest")
    return original


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
class CollectionProofPublicValues:
    """Values a future SP1 guest exposes without revealing licensed snapshots."""

    program_sha256: str
    epoch_sha256: str
    methodology_sha256: str
    collector_code_sha256: str
    source_id: str
    account_application_identity_sha256: str
    commitments_sha256: str
    attested_collection_sha256: str
    normalized_input_root_sha256: str
    snapshot_count: int

    def __post_init__(self) -> None:
        for field in (
            "program_sha256",
            "epoch_sha256",
            "methodology_sha256",
            "collector_code_sha256",
            "account_application_identity_sha256",
            "commitments_sha256",
            "attested_collection_sha256",
            "normalized_input_root_sha256",
        ):
            _digest(getattr(self, field), field)
        if not isinstance(self.source_id, str) or not self.source_id.strip():
            raise ValueError("source_id is required")
        if (
            not isinstance(self.snapshot_count, int)
            or isinstance(self.snapshot_count, bool)
            or self.snapshot_count <= 0
        ):
            raise ValueError("snapshot_count must be a positive integer")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": COLLECTION_PROOF_PUBLIC_VALUES_SCHEMA,
            "programSha256": self.program_sha256,
            "epochSha256": self.epoch_sha256,
            "methodologySha256": self.methodology_sha256,
            "collectorCodeSha256": self.collector_code_sha256,
            "sourceId": self.source_id,
            "accountApplicationIdentitySha256": (
                self.account_application_identity_sha256
            ),
            "commitmentsSha256": self.commitments_sha256,
            "attestedCollectionSha256": self.attested_collection_sha256,
            "normalizedInputRootSha256": self.normalized_input_root_sha256,
            "snapshotCount": self.snapshot_count,
        }

    @classmethod
    def from_dict(cls, value: Any) -> CollectionProofPublicValues:
        public_values = _strict_object(
            value,
            fields={
                "schema",
                "programSha256",
                "epochSha256",
                "methodologySha256",
                "collectorCodeSha256",
                "sourceId",
                "accountApplicationIdentitySha256",
                "commitmentsSha256",
                "attestedCollectionSha256",
                "normalizedInputRootSha256",
                "snapshotCount",
            },
            name="collection proof public values",
        )
        if public_values["schema"] != COLLECTION_PROOF_PUBLIC_VALUES_SCHEMA:
            raise ValueError("collection proof public values schema mismatch")
        return cls(
            program_sha256=public_values["programSha256"],
            epoch_sha256=public_values["epochSha256"],
            methodology_sha256=public_values["methodologySha256"],
            collector_code_sha256=public_values["collectorCodeSha256"],
            source_id=public_values["sourceId"],
            account_application_identity_sha256=(
                public_values["accountApplicationIdentitySha256"]
            ),
            commitments_sha256=public_values["commitmentsSha256"],
            attested_collection_sha256=public_values["attestedCollectionSha256"],
            normalized_input_root_sha256=(
                public_values["normalizedInputRootSha256"]
            ),
            snapshot_count=public_values["snapshotCount"],
        )

    @property
    def sha256(self) -> str:
        return domain_sha256(COLLECTION_PROOF_PUBLIC_VALUES_SCHEMA, self.to_dict())

    def to_pftl_v1(self) -> PftlCollectionPublicValuesV1:
        """Convert to the fixed-width, disclosure-safe PFTL consensus ABI."""

        return PftlCollectionPublicValuesV1(
            program_sha256=self.program_sha256,
            epoch_sha256=self.epoch_sha256,
            methodology_sha256=self.methodology_sha256,
            collector_code_sha256=self.collector_code_sha256,
            source_id_sha256=domain_sha256(
                SOURCE_ID_COMMITMENT_DOMAIN,
                self.source_id,
            ),
            account_application_identity_sha256=(
                self.account_application_identity_sha256
            ),
            commitments_sha256=self.commitments_sha256,
            attested_collection_sha256=self.attested_collection_sha256,
            normalized_input_root_sha256=self.normalized_input_root_sha256,
            snapshot_count=self.snapshot_count,
        )


@dataclass(frozen=True)
class PftlCollectionPublicValuesV1:
    """Fixed-width values shared by Python, the SP1 guest, and PFTL."""

    program_sha256: str
    epoch_sha256: str
    methodology_sha256: str
    collector_code_sha256: str
    source_id_sha256: str
    account_application_identity_sha256: str
    commitments_sha256: str
    attested_collection_sha256: str
    normalized_input_root_sha256: str
    snapshot_count: int

    def __post_init__(self) -> None:
        for field in (
            "program_sha256",
            "epoch_sha256",
            "methodology_sha256",
            "collector_code_sha256",
            "source_id_sha256",
            "account_application_identity_sha256",
            "commitments_sha256",
            "attested_collection_sha256",
            "normalized_input_root_sha256",
        ):
            _digest(getattr(self, field), field)
        if (
            not isinstance(self.snapshot_count, int)
            or isinstance(self.snapshot_count, bool)
            or not 1 <= self.snapshot_count <= PFTL_COLLECTION_MAX_SNAPSHOTS
        ):
            raise ValueError(
                f"snapshot_count must be in 1..={PFTL_COLLECTION_MAX_SNAPSHOTS}"
            )

    def encode(self) -> bytes:
        digests = (
            self.program_sha256,
            self.epoch_sha256,
            self.methodology_sha256,
            self.collector_code_sha256,
            self.source_id_sha256,
            self.account_application_identity_sha256,
            self.commitments_sha256,
            self.attested_collection_sha256,
            self.normalized_input_root_sha256,
        )
        encoded = b"".join(
            (
                PFTL_COLLECTION_PUBLIC_VALUES_MAGIC,
                struct.pack(">I", PFTL_COLLECTION_PUBLIC_VALUES_VERSION),
                *(bytes.fromhex(value) for value in digests),
                struct.pack(">I", self.snapshot_count),
            )
        )
        if len(encoded) != PFTL_COLLECTION_PUBLIC_VALUES_BYTES:
            raise AssertionError("PFTL collection ABI length invariant failed")
        return encoded

    @classmethod
    def decode(cls, encoded: bytes) -> PftlCollectionPublicValuesV1:
        if not isinstance(encoded, bytes) or len(encoded) != PFTL_COLLECTION_PUBLIC_VALUES_BYTES:
            raise ValueError(
                f"PFTL collection public values must be exactly {PFTL_COLLECTION_PUBLIC_VALUES_BYTES} bytes"
            )
        if encoded[:8] != PFTL_COLLECTION_PUBLIC_VALUES_MAGIC:
            raise ValueError("PFTL collection public-values magic mismatch")
        version = struct.unpack(">I", encoded[8:12])[0]
        if version != PFTL_COLLECTION_PUBLIC_VALUES_VERSION:
            raise ValueError("PFTL collection public-values version mismatch")
        offset = 12
        digests: list[str] = []
        for _ in range(9):
            digests.append(encoded[offset : offset + 32].hex())
            offset += 32
        snapshot_count = struct.unpack(">I", encoded[offset : offset + 4])[0]
        return cls(*digests, snapshot_count=snapshot_count)


@dataclass(frozen=True)
class PftlTargetPublicValuesV1:
    """Fixed-width, disclosure-safe YOLO target proof output.

    This is a proof/receipt payload only. It has no broker order, reserve,
    issuance, redemption, or capital-movement authority.
    """

    program_sha256: str
    methodology_sha256: str
    parameter_manifest_sha256: str
    collection_manifest_sha256: str
    series_id_sha256: str
    underlier_id_sha256: str
    epoch_sha256: str
    collection_sha256: str
    aggregate_attestation_sha256: str
    prior_state_sha256: str
    selected_expiration_sha256: str
    target_sha256: str
    status: str
    snapshot_count: int
    selected_contract_count: int

    def __post_init__(self) -> None:
        for field in (
            "program_sha256",
            "methodology_sha256",
            "parameter_manifest_sha256",
            "collection_manifest_sha256",
            "series_id_sha256",
            "underlier_id_sha256",
            "epoch_sha256",
            "collection_sha256",
            "aggregate_attestation_sha256",
            "prior_state_sha256",
            "selected_expiration_sha256",
            "target_sha256",
        ):
            _digest(getattr(self, field), field)
        if self.status not in PFTL_TARGET_STATUSES:
            raise ValueError("unknown PFTL target status")
        for field, maximum in (
            ("snapshot_count", PFTL_TARGET_MAX_SNAPSHOTS),
            (
                "selected_contract_count",
                PFTL_TARGET_MAX_SELECTED_CONTRACTS,
            ),
        ):
            value = getattr(self, field)
            minimum = 1 if field == "snapshot_count" else 0
            if (
                not isinstance(value, int)
                or isinstance(value, bool)
                or not minimum <= value <= maximum
            ):
                raise ValueError(
                    f"{field} must be in {minimum}..={maximum}"
                )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": PFTL_TARGET_PUBLIC_VALUES_SCHEMA,
            "programSha256": self.program_sha256,
            "methodologySha256": self.methodology_sha256,
            "parameterManifestSha256": self.parameter_manifest_sha256,
            "collectionManifestSha256": self.collection_manifest_sha256,
            "seriesIdSha256": self.series_id_sha256,
            "underlierIdSha256": self.underlier_id_sha256,
            "epochSha256": self.epoch_sha256,
            "collectionSha256": self.collection_sha256,
            "aggregateAttestationSha256": self.aggregate_attestation_sha256,
            "priorStateSha256": self.prior_state_sha256,
            "selectedExpirationSha256": self.selected_expiration_sha256,
            "targetSha256": self.target_sha256,
            "status": self.status,
            "snapshotCount": self.snapshot_count,
            "selectedContractCount": self.selected_contract_count,
        }

    @classmethod
    def from_dict(cls, value: Any) -> PftlTargetPublicValuesV1:
        public_values = _strict_object(
            value,
            fields={
                "schema",
                "programSha256",
                "methodologySha256",
                "parameterManifestSha256",
                "collectionManifestSha256",
                "seriesIdSha256",
                "underlierIdSha256",
                "epochSha256",
                "collectionSha256",
                "aggregateAttestationSha256",
                "priorStateSha256",
                "selectedExpirationSha256",
                "targetSha256",
                "status",
                "snapshotCount",
                "selectedContractCount",
            },
            name="target public values",
        )
        if public_values["schema"] != PFTL_TARGET_PUBLIC_VALUES_SCHEMA:
            raise ValueError("target public values schema mismatch")
        return cls(
            program_sha256=public_values["programSha256"],
            methodology_sha256=public_values["methodologySha256"],
            parameter_manifest_sha256=public_values[
                "parameterManifestSha256"
            ],
            collection_manifest_sha256=public_values[
                "collectionManifestSha256"
            ],
            series_id_sha256=public_values["seriesIdSha256"],
            underlier_id_sha256=public_values["underlierIdSha256"],
            epoch_sha256=public_values["epochSha256"],
            collection_sha256=public_values["collectionSha256"],
            aggregate_attestation_sha256=public_values[
                "aggregateAttestationSha256"
            ],
            prior_state_sha256=public_values["priorStateSha256"],
            selected_expiration_sha256=public_values[
                "selectedExpirationSha256"
            ],
            target_sha256=public_values["targetSha256"],
            status=public_values["status"],
            snapshot_count=public_values["snapshotCount"],
            selected_contract_count=public_values["selectedContractCount"],
        )

    def encode(self) -> bytes:
        digests = (
            self.program_sha256,
            self.methodology_sha256,
            self.parameter_manifest_sha256,
            self.collection_manifest_sha256,
            self.series_id_sha256,
            self.underlier_id_sha256,
            self.epoch_sha256,
            self.collection_sha256,
            self.aggregate_attestation_sha256,
            self.prior_state_sha256,
            self.selected_expiration_sha256,
            self.target_sha256,
        )
        encoded = b"".join(
            (
                PFTL_TARGET_PUBLIC_VALUES_MAGIC,
                struct.pack(">I", PFTL_TARGET_PUBLIC_VALUES_VERSION),
                *(bytes.fromhex(value) for value in digests),
                struct.pack(">I", PFTL_TARGET_STATUSES.index(self.status) + 1),
                struct.pack(">I", self.snapshot_count),
                struct.pack(">I", self.selected_contract_count),
            )
        )
        if len(encoded) != PFTL_TARGET_PUBLIC_VALUES_BYTES:
            raise AssertionError("PFTL target ABI length invariant failed")
        return encoded

    @classmethod
    def decode(cls, encoded: bytes) -> PftlTargetPublicValuesV1:
        if (
            not isinstance(encoded, bytes)
            or len(encoded) != PFTL_TARGET_PUBLIC_VALUES_BYTES
        ):
            raise ValueError(
                "PFTL target public values must be exactly "
                f"{PFTL_TARGET_PUBLIC_VALUES_BYTES} bytes"
            )
        if encoded[:8] != PFTL_TARGET_PUBLIC_VALUES_MAGIC:
            raise ValueError("PFTL target public-values magic mismatch")
        version = struct.unpack(">I", encoded[8:12])[0]
        if version != PFTL_TARGET_PUBLIC_VALUES_VERSION:
            raise ValueError("PFTL target public-values version mismatch")
        offset = 12
        fields = (
            "program_sha256",
            "methodology_sha256",
            "parameter_manifest_sha256",
            "collection_manifest_sha256",
            "series_id_sha256",
            "underlier_id_sha256",
            "epoch_sha256",
            "collection_sha256",
            "aggregate_attestation_sha256",
            "prior_state_sha256",
            "selected_expiration_sha256",
            "target_sha256",
        )
        digests: dict[str, str] = {}
        for field in fields:
            digests[field] = encoded[offset : offset + 32].hex()
            offset += 32
        status_code = struct.unpack(">I", encoded[offset : offset + 4])[0]
        offset += 4
        if not 1 <= status_code <= len(PFTL_TARGET_STATUSES):
            raise ValueError("unknown PFTL target status code")
        status = PFTL_TARGET_STATUSES[status_code - 1]
        snapshot_count = struct.unpack(">I", encoded[offset : offset + 4])[0]
        offset += 4
        selected_contract_count = struct.unpack(
            ">I", encoded[offset : offset + 4]
        )[0]
        offset += 4
        if offset != len(encoded):
            raise ValueError("PFTL target public values have trailing bytes")
        return cls(
            **digests,
            status=status,
            snapshot_count=snapshot_count,
            selected_contract_count=selected_contract_count,
        )


def collection_proof_public_values(
    epoch: CollectionEpoch,
    commitments: tuple[SnapshotCommitment, ...],
    *,
    program_sha256: str,
    attested_collection_sha256: str,
) -> CollectionProofPublicValues:
    """Derive disclosure-safe public values from one complete collection."""

    status = assess_collection(epoch, commitments)
    if status.state != "COMPLETE" or status.commitments_sha256 is None:
        detail = ",".join(status.issues) if status.issues else status.state
        raise ValueError(f"collection is not complete: {detail}")
    ordered = tuple(sorted(commitments, key=lambda item: item.sequence))
    normalized_input_root = domain_sha256(
        NORMALIZED_INPUT_ROOT_SCHEMA,
        {
            "schema": NORMALIZED_INPUT_ROOT_SCHEMA,
            "epochSha256": epoch.sha256,
            "snapshots": [
                {
                    "sequence": item.sequence,
                    "sourcePayloadSha256": item.source_payload_sha256,
                    "responseCoverageSha256": item.response_coverage_sha256,
                    "contractsSha256": item.contracts_sha256,
                }
                for item in ordered
            ],
        },
    )
    return CollectionProofPublicValues(
        program_sha256=_digest(program_sha256, "program_sha256"),
        epoch_sha256=epoch.sha256,
        methodology_sha256=epoch.methodology_sha256,
        collector_code_sha256=epoch.collector_code_sha256,
        source_id=epoch.source_id,
        account_application_identity_sha256=(
            epoch.account_application_identity_sha256
        ),
        commitments_sha256=status.commitments_sha256,
        attested_collection_sha256=_digest(
            attested_collection_sha256,
            "attested_collection_sha256",
        ),
        normalized_input_root_sha256=normalized_input_root,
        snapshot_count=len(ordered),
    )
