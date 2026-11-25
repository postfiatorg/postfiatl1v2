"""Synthetic Phase 3 qualification inputs; never reads broker data or secrets.

Run as a script with --output and --program-sha256 to generate fresh signed
fixtures. Only public synthetic certificates and signatures are written. The
ephemeral test private keys remain in memory and are discarded.
"""

from dataclasses import replace
from datetime import datetime, timedelta, timezone
from pathlib import Path
import argparse
import json

from cryptography.hazmat.primitives import serialization

from pftl_schwab_tee.attestation import AttestedStatement, TeeEvidence
from pftl_schwab_tee.canonical import canonical_json_bytes, domain_sha256, sha256_bytes
from pftl_schwab_tee.models import CollectionEpoch, OptionChainQuery, SnapshotCommitment, assess_collection
from pftl_schwab_tee.methodology import create_portfolio_target
from pftl_schwab_tee.nitro_runtime import NitroStatementKeys
from pftl_schwab_tee.proofs import PftlTargetPublicValuesV1
from pftl_schwab_tee.tee_policy import PolicyBoundTeeEvidenceVerifier, TeeMeasurementPolicy
from pftl_schwab_tee.nitro_runtime import Ed25519HexSignatureVerifier
from pftl_schwab_tee.nitro_attestation import NitroNativeAttestationVerifier
from test_yolo_methodology import cross_language_target_vectors, parameters, snapshots
from test_yolo_nitro_attestation import _certificates, _document

MANIFEST_SCHEMA = "postfiat.yolo.target_collection_manifest.v1"
INPUT_SCHEMA = "postfiat.yolo.attested_target_input.v1"
AGGREGATE_SCHEMA = "postfiat.yolo.target_attested_collection.v1"


def signed_fixture(name, original, program_sha256):
    params = parameters()
    baseline = snapshots(
        trade_date=original.trade_date,
        expirations=((original.trade_date + timedelta(days=60), (48, 49, 50, 51, 52)),),
    )
    all_observations = {row.sequence: row for row in baseline}
    all_observations.update({row.sequence: row for row in original.snapshots})
    start = baseline[0].scheduled_at_utc
    end = baseline[-1].scheduled_at_utc
    epoch = CollectionEpoch(
        epoch_id="synthetic-phase3-" + name, methodology_sha256=original.methodology_sha256,
        underlier="HOOD", source_id="synthetic-schwab-fixture",
        account_application_identity_sha256=sha256_bytes(b"synthetic-account-no-credentials"),
        collector_code_sha256=sha256_bytes(b"synthetic-collector-not-an-approved-image"),
        prior_epoch_sha256=None, required_response_fields=("callExpDateMap", "symbol"),
        query=OptionChainQuery(
            symbol="HOOD", contract_type="ALL", strategy="SINGLE", include_underlying_quote=True,
            from_date=original.trade_date, to_date=original.trade_date + timedelta(days=90), strike_count=None,
        ),
        window_start_utc=start, window_end_utc=end, expected_snapshot_count=5,
        snapshot_offsets_seconds=(0, 450, 900, 1350, 1800),
    )
    commitments = []
    for index in range(5):
        row = all_observations[index]
        content = row.to_dict()
        encoded = canonical_json_bytes(content)
        commitment = SnapshotCommitment(
            schema="postfiat.yolo.snapshot_commitment.v2", epoch_sha256=epoch.sha256,
            sequence=index, scheduled_at_utc=content["scheduledAtUtc"], observed_at_utc=content["observedAtUtc"],
            source_id=epoch.source_id, http_status=200, response_date=None,
            source_payload_sha256=domain_sha256("postfiat.yolo.schwab_source_payload.v1", content),
            raw_response_sha256=sha256_bytes(encoded),
            response_coverage_sha256=domain_sha256("postfiat.yolo.response_coverage.v1", {"synthetic": True}),
            contracts_sha256=domain_sha256("postfiat.yolo.option_contracts.v1", content["contracts"]),
            flattened_contract_count=len(row.contracts), reported_contract_count=len(row.contracts),
            previous_snapshot_sha256=None if not commitments else commitments[-1].sha256,
            private_snapshot_sha256=domain_sha256("postfiat.yolo.synthetic_private_snapshot.v1", content),
        )
        commitments.append(commitment)
    collection = assess_collection(epoch, tuple(commitments))
    assert collection.state == "COMPLETE"
    target_input = replace(original, collection_sha256=collection.commitments_sha256)
    certificates = _certificates()
    root_der = certificates[1].public_bytes(serialization.Encoding.DER)
    root_pem = certificates[1].public_bytes(serialization.Encoding.PEM)
    keys = NitroStatementKeys.generate()
    policy = {
        "schema": "postfiat.yolo.nitro_proof_policy.v1",
        "rootCertificateSha256": sha256_bytes(root_der),
        "approvedPcrSets": [[{"index": i, "sha384": bytes([0x11 + i]).hex() * 48} for i in range(3)]],
        "maxAgeMs": 1_800_000, "maxFutureSkewMs": 1000,
        "timeRule": "EPOCH_WINDOW_END", "statementRule": "COLLECTION_AND_TARGET_INPUT_V1",
        "verifierId": "synthetic-nitro-v2",
    }
    manifest = {
        "schema": MANIFEST_SCHEMA, "runId": epoch.epoch_id,
        "seriesId": target_input.series_id, "underlierId": target_input.underlier_id,
        "tradeDate": target_input.trade_date.isoformat(),
        "parameterManifestSha256": params.sha256,
        "normalizationSourcesSha256": sha256_bytes(b"synthetic-references-calendar-halts"),
        "epoch": epoch.to_dict(), "proofPolicy": policy,
    }
    manifest_hash = domain_sha256(MANIFEST_SCHEMA, manifest)
    input_hash = domain_sha256(INPUT_SCHEMA, {
        "schema": INPUT_SCHEMA, "collectionManifestSha256": manifest_hash,
        "parameterManifestSha256": params.sha256, "targetInput": target_input.to_dict(),
    })
    statements, documents = [], []
    for index in range(6):
        statement = AttestedStatement(
            statement_kind="option-chain-snapshot" if index < 5 else "portfolio-target-input",
            epoch_sha256=epoch.sha256,
            input_sha256=commitments[index].private_snapshot_sha256 if index < 5 else collection.commitments_sha256,
            output_sha256=commitments[index].sha256 if index < 5 else input_hash,
            sequence=index, previous_statement_sha256=None if not statements else statements[-1].sha256,
            signature="pending", evidence=TeeEvidence(
                platform="aws-nitro", trust_class="attested", hardware_backed=True, debug_mode=False,
                measurement="pcr0:" + "11" * 48, statement_key=keys.statement_key,
                attestation_document_sha256="00" * 32, verifier_id=policy["verifierId"],
            ),
        )
        binding = statement.attestation_binding_digest
        document, _ = _document(
            timestamp=all_observations[index].observed_at_utc if index < 5 else end + timedelta(seconds=1),
            nonce=bytes.fromhex(binding), statement_digest=binding, statement_key=keys.statement_key,
            certificates=certificates, recipient_key=keys.recipient_private_key,
        )
        statement = replace(statement, evidence=replace(statement.evidence, attestation_document_sha256=sha256_bytes(document)))
        statement = replace(statement, signature=keys.sign_digest(statement.signing_digest))
        statements.append(statement)
        documents.append(document)
    document_map = {sha256_bytes(doc): doc for doc in documents}

    class Documents:
        def get_document(self, digest): return document_map[digest]

    verifier = PolicyBoundTeeEvidenceVerifier(
        TeeMeasurementPolicy(policy["verifierId"], "aws-nitro", ("pcr0:" + "11" * 48,)),
        document_source=Documents(), signature_verifier=Ed25519HexSignatureVerifier(),
        native_verifier=NitroNativeAttestationVerifier(
            root_certificate_pem=root_pem, max_age_ms=policy["maxAgeMs"],
            max_future_skew_ms=policy["maxFutureSkewMs"], clock=lambda: end,
        ),
    )
    for statement in statements: verifier.verify(statement)
    target = create_portfolio_target(target_input, params)
    aggregate = domain_sha256(AGGREGATE_SCHEMA, {
        "schema": AGGREGATE_SCHEMA, "collectionManifestSha256": manifest_hash,
        "epochSha256": epoch.sha256, "commitmentsSha256": collection.commitments_sha256,
        "statementSha256s": [statement.sha256 for statement in statements],
    })
    prior = target_input.to_dict()
    prior = {key: prior[key] for key in (
        "positions", "settledCashMicrodollars", "unsettledCashMicrodollars",
        "accruedLiabilitiesMicrodollars", "incumbentExpiration", "incumbentSymbols",
    )}
    prior["schema"] = "postfiat.yolo.prior_portfolio_state.v1"
    prior["positions"] = sorted(prior["positions"], key=lambda row: row["occSymbol"])
    prior["incumbentSymbols"] = sorted(prior["incumbentSymbols"])
    public = PftlTargetPublicValuesV1(
        program_sha256=program_sha256, methodology_sha256=target_input.methodology_sha256,
        parameter_manifest_sha256=params.sha256, collection_manifest_sha256=manifest_hash,
        series_id_sha256=domain_sha256("postfiat.yolo.series_id.v1", target_input.series_id),
        underlier_id_sha256=domain_sha256("postfiat.yolo.underlier_id.v1", target_input.underlier_id),
        epoch_sha256=epoch.sha256, collection_sha256=collection.commitments_sha256,
        aggregate_attestation_sha256=aggregate,
        prior_state_sha256=domain_sha256(prior["schema"], prior),
        selected_expiration_sha256=domain_sha256("postfiat.yolo.selected_expiration.v1", None if target.selected_expiration is None else target.selected_expiration.isoformat()),
        target_sha256=target.sha256, status=target.status,
        snapshot_count=len(target_input.snapshots),
        selected_contract_count=sum(row.target_quantity > 0 for row in target.positions),
    )
    return {
        "schema": "postfiat.yolo.synthetic_target_proof_fixture.v1", "name": name,
        "expectedPublicValuesHex": public.encode().hex(), "expectedTarget": target.to_dict(),
        "witness": {
            "schema": "postfiat.yolo.target_proof_witness.v1", "programSha256": program_sha256,
            "manifest": manifest, "parameters": params.to_dict(),
            "commitments": [row.to_dict() for row in commitments], "targetInput": target_input.to_dict(),
            "statements": [row.to_dict() for row in statements],
            "attestationDocumentsHex": [doc.hex() for doc in documents], "rootCertificateDerHex": root_der.hex(),
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--program-sha256", required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, value, expected_status, expected_expiration, expected_hash in cross_language_target_vectors():
        unchanged = create_portfolio_target(value, parameters())
        assert (unchanged.status, unchanged.selected_expiration, unchanged.sha256) == (expected_status, expected_expiration, expected_hash)
        fixture = signed_fixture(name, value, args.program_sha256)
        assert fixture["expectedTarget"]["status"] == expected_status
        with (args.output / (name + ".json")).open("x") as output:
            json.dump(fixture, output, indent=2)
            output.write("\n")
    print(f"Created 14 signed synthetic fixtures in {args.output}")


if __name__ == "__main__": main()
