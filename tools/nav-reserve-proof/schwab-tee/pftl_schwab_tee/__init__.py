"""Provider-neutral collection and proof primitives for YOLO indices.

This package intentionally has no basket construction or order-submission path.
Those behaviors require a separately locked methodology.
"""

from .canonical import canonical_json_bytes, domain_sha256, sha256_bytes
from .attestation import (
    AttestedStatement,
    TeeEvidence,
    TeeEvidenceVerifier,
    verify_attested_collection,
)
from .models import (
    CollectionEpoch,
    CollectionStatus,
    OptionChainQuery,
    PrivateOptionChainSnapshot,
    SnapshotCommitment,
    assess_collection,
)
from .methodology import (
    AggregatedContract,
    ContractQuote,
    CurrentPosition,
    MethodologySnapshot,
    PortfolioParameters,
    PortfolioTarget,
    PortfolioTargetInput,
    TargetPosition,
    create_portfolio_target,
    target_quantity_from_midpoint,
)
from .schwab_chain import SchwabOptionChainCollector
from .schwab_methodology import (
    ContractReferenceTerms,
    methodology_snapshot_from_schwab,
)
from .proofs import (
    CollectionProofPublicValues,
    PftlCollectionPublicValuesV1,
    PftlTargetPublicValuesV1,
    collection_proof_public_values,
)
from .nitro_kms import (
    KmsRecipientClient,
    NitroKmsEnvelopeEncryptor,
    NitroRecipientCiphertext,
    NitroRecipientDataKey,
    decrypt_nitro_ciphertext_bundle,
    request_attested_data_key_ciphertext,
    request_attested_secret_ciphertext,
)
from .nitro_attestation import (
    NITRO_USER_DATA_SCHEMA,
    NitroAttestationDocument,
    NitroNativeAttestationVerifier,
)
from .nitro_runtime import (
    Ed25519HexSignatureVerifier,
    NitroSecureModule,
    NitroStatementKeys,
    decrypt_recipient_ciphertext,
    nitro_user_data,
)
from .nitro_transport import (
    FixedUpstream,
    NitroHttpResponse,
    NitroHttpsJsonTransport,
    NitroSchwabApiClient,
    NitroVsockTlsSocketFactory,
    relay_bidirectional,
    serve_fixed_upstream_proxy,
)
from .tee_policy import (
    NativeAttestationClaims,
    PolicyBoundTeeEvidenceVerifier,
    TeeMeasurementPolicy,
)
from .reserve import (
    BrokerReserveReadRequest,
    BrokerReserveResponseCollector,
    PrivateBrokerApiResponse,
    PrivateBrokerReserveSnapshot,
    YOLO_BROKER_ADAPTER_KIND_V1,
    YoloBrokerEvidenceCommitments,
    YoloBrokerReserveDisclosure,
)
from .private_archive import (
    ArchivedBrokerReserveReceipt,
    ArchivedSnapshotReceipt,
    CiphertextArchive,
    CiphertextEnvelope,
    SnapshotEncryptor,
    archive_broker_reserve_snapshot,
    archive_snapshot,
    broker_reserve_archive_associated_data,
    encrypt_snapshot,
    snapshot_archive_associated_data,
    verify_broker_reserve_replay_plaintext,
    verify_replay_plaintext,
)

__all__ = [
    "AttestedStatement",
    "ArchivedBrokerReserveReceipt",
    "ArchivedSnapshotReceipt",
    "BrokerReserveReadRequest",
    "BrokerReserveResponseCollector",
    "CiphertextArchive",
    "CiphertextEnvelope",
    "CollectionEpoch",
    "CollectionProofPublicValues",
    "PftlCollectionPublicValuesV1",
    "PftlTargetPublicValuesV1",
    "CollectionStatus",
    "ContractQuote",
    "ContractReferenceTerms",
    "CurrentPosition",
    "KmsRecipientClient",
    "NitroRecipientCiphertext",
    "NitroRecipientDataKey",
    "NitroKmsEnvelopeEncryptor",
    "NitroAttestationDocument",
    "NitroNativeAttestationVerifier",
    "NitroSecureModule",
    "NitroStatementKeys",
    "Ed25519HexSignatureVerifier",
    "NITRO_USER_DATA_SCHEMA",
    "FixedUpstream",
    "NitroHttpResponse",
    "NitroHttpsJsonTransport",
    "NitroSchwabApiClient",
    "NitroVsockTlsSocketFactory",
    "NativeAttestationClaims",
    "MethodologySnapshot",
    "OptionChainQuery",
    "PrivateBrokerApiResponse",
    "PrivateBrokerReserveSnapshot",
    "PrivateOptionChainSnapshot",
    "PolicyBoundTeeEvidenceVerifier",
    "PortfolioParameters",
    "PortfolioTarget",
    "PortfolioTargetInput",
    "SchwabOptionChainCollector",
    "SnapshotEncryptor",
    "SnapshotCommitment",
    "TeeEvidence",
    "TeeEvidenceVerifier",
    "TeeMeasurementPolicy",
    "TargetPosition",
    "AggregatedContract",
    "YOLO_BROKER_ADAPTER_KIND_V1",
    "YoloBrokerEvidenceCommitments",
    "YoloBrokerReserveDisclosure",
    "assess_collection",
    "archive_broker_reserve_snapshot",
    "archive_snapshot",
    "broker_reserve_archive_associated_data",
    "canonical_json_bytes",
    "collection_proof_public_values",
    "create_portfolio_target",
    "methodology_snapshot_from_schwab",
    "target_quantity_from_midpoint",
    "domain_sha256",
    "encrypt_snapshot",
    "sha256_bytes",
    "snapshot_archive_associated_data",
    "verify_broker_reserve_replay_plaintext",
    "verify_replay_plaintext",
    "verify_attested_collection",
    "request_attested_secret_ciphertext",
    "request_attested_data_key_ciphertext",
    "decrypt_nitro_ciphertext_bundle",
    "decrypt_recipient_ciphertext",
    "nitro_user_data",
    "relay_bidirectional",
    "serve_fixed_upstream_proxy",
]
