"""One enclave-local target run with explicit, authenticated source inputs.

The source-packet protocol authenticates the owner-approved input attestor named
in the manifest. It does not claim that an arbitrary signature is native Schwab,
exchange, calendar or halt-provider evidence. Provider approval remains a run
input. The measured entrypoint never accepts an already-normalized target.
"""

from __future__ import annotations

import base64
from dataclasses import replace
from datetime import date, datetime, timezone
import json
from typing import Any, Callable

from cryptography import x509
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

from .attestation import AttestedStatement, TeeEvidence
from .canonical import canonical_json_bytes, domain_sha256, sha256_bytes
from .methodology import PortfolioParameters, PortfolioTargetInput, create_portfolio_target
from .models import CollectionEpoch, assess_collection
from .nitro_attestation import NitroNativeAttestationVerifier
from .nitro_runtime import NitroStatementKeys
from .private_archive import encrypt_snapshot, snapshot_archive_associated_data
from .schwab_chain import SchwabOptionChainCollector
from .schwab_methodology import ContractReferenceTerms, methodology_snapshot_from_schwab


MANIFEST_SCHEMA = "postfiat.yolo.target_collection_manifest.v1"
SOURCES_SCHEMA = "postfiat.yolo.normalization_sources.v1"
PACKET_SCHEMA = "postfiat.yolo.source_packet.v1"
INPUT_SCHEMA = "postfiat.yolo.attested_target_input.v1"
WITNESS_SCHEMA = "postfiat.yolo.target_proof_witness.v1"
REPLAY_SCHEMA = "postfiat.yolo.enclave_replay_package.v1"
MAX_PRIVATE_BYTES = 64 * 1024 * 1024


def strict(value: Any, fields: set[str], name: str) -> dict:
    if not isinstance(value, dict) or set(value) != fields:
        raise ValueError(f"invalid {name} fields")
    return value


def digest(value: Any) -> str:
    if (not isinstance(value, str) or len(value) != 64
            or any(c not in "0123456789abcdef" for c in value)):
        raise ValueError("expected lowercase SHA-256/Ed25519 bytes")
    return value


def utc(value: Any) -> datetime:
    if not isinstance(value, str):
        raise ValueError("expected UTC timestamp")
    parsed = datetime.fromisoformat(value)
    if parsed.tzinfo is None or parsed.utcoffset() != timezone.utc.utcoffset(None):
        raise ValueError("expected UTC timestamp")
    return parsed


def private_json(encoded: bytes) -> dict:
    if not isinstance(encoded, bytes) or not 0 < len(encoded) <= MAX_PRIVATE_BYTES:
        raise ValueError("private JSON exceeds its bound")
    try:
        value = json.loads(encoded)
        if canonical_json_bytes(value) != encoded or not isinstance(value, dict):
            raise ValueError()
    except (ValueError, UnicodeDecodeError, RecursionError):
        raise ValueError("invalid canonical private JSON") from None
    return value


def validate_sources(sources: dict) -> str:
    strict(sources, {"schema", "calendarId", "calendarVersion", "contractReferenceId",
                     "contractReferenceVersion", "haltSourceId", "underlierQuoteSourceId",
                     "inputSignerEd25519"}, "normalization sources")
    if sources["schema"] != SOURCES_SCHEMA:
        raise ValueError("normalization source schema differs")
    for name, value in sources.items():
        if not isinstance(value, str) or not 0 < len(value.encode()) <= 256 or "\0" in value:
            raise ValueError("invalid source descriptor")
    digest(sources["inputSignerEd25519"])
    return domain_sha256(SOURCES_SCHEMA, sources)


class NitroTargetRun:
    """Sequential collection/calculation, with no plaintext output path.

    All callbacks are enclave-local implementations. The production entrypoint
    supplies the real NSM, fixed Schwab TLS, recipient KMS and archive transports;
    it offers no way to import a callback or supply a Python module from a host.
    """

    def __init__(self, *, manifest: dict, parameters: dict, sources: dict,
                 program_sha256: str, root_der: bytes, nsm: Any,
                 encryptor: Callable[[bytes], Any], archive: Any,
                 source_packet: Callable[[str, int | None, str | None, bytes], bytes],
                 client: Any, clock: Callable[[], datetime],
                 wait_until: Callable[[datetime], None], event: Callable[[dict], None]):
        # Copy mutable caller structures before computing any commitments.
        manifest = private_json(canonical_json_bytes(manifest))
        sources = private_json(canonical_json_bytes(sources))
        strict(manifest, {"schema", "runId", "seriesId", "underlierId", "tradeDate",
                          "parameterManifestSha256", "normalizationSourcesSha256",
                          "epoch", "proofPolicy"}, "target manifest")
        if manifest["schema"] != MANIFEST_SCHEMA:
            raise ValueError("target manifest schema differs")
        for name in ("runId", "seriesId", "underlierId"):
            value = manifest[name]
            if not isinstance(value, str) or not 0 < len(value.encode()) <= 256 or "\0" in value:
                raise ValueError("invalid manifest identifier")
        self.epoch = CollectionEpoch.from_dict(manifest["epoch"])
        self.parameters = PortfolioParameters.from_dict(parameters)
        if (manifest["parameterManifestSha256"] != self.parameters.sha256
                or manifest["normalizationSourcesSha256"] != validate_sources(sources)):
            raise ValueError("run parameters or sources differ from manifest")
        if self.epoch.expected_snapshot_count != self.parameters.observation_count:
            raise ValueError("epoch count differs from locked parameters")
        self.trade_date = date.fromisoformat(manifest["tradeDate"])
        if self.trade_date.isoformat() != manifest["tradeDate"]:
            raise ValueError("trade date must be canonical")
        policy = strict(manifest["proofPolicy"], {"schema", "rootCertificateSha256",
            "approvedPcrSets", "maxAgeMs", "maxFutureSkewMs", "timeRule",
            "statementRule", "verifierId"}, "Nitro proof policy")
        if (policy["schema"] != "postfiat.yolo.nitro_proof_policy.v1"
                or policy["timeRule"] != "EPOCH_WINDOW_END"
                or policy["statementRule"] != "COLLECTION_AND_TARGET_INPUT_V1"
                or not isinstance(policy["verifierId"], str)
                or not 0 < len(policy["verifierId"].encode()) <= 256
                or sha256_bytes(root_der) != policy["rootCertificateSha256"]):
            raise ValueError("Nitro root or proof policy differs")
        self.pcr_sets = self._pcr_sets(policy["approvedPcrSets"])
        root = x509.load_der_x509_certificate(root_der)
        self.verifier = NitroNativeAttestationVerifier(
            root_certificate_pem=root.public_bytes(serialization.Encoding.PEM),
            max_age_ms=policy["maxAgeMs"], max_future_skew_ms=policy["maxFutureSkewMs"],
            clock=lambda: self.epoch.window_end_utc,
        )
        self.manifest, self.sources, self.root_der = manifest, sources, root_der
        self.manifest_sha256 = domain_sha256(MANIFEST_SCHEMA, manifest)
        self.program_sha256 = digest(program_sha256)
        self.keys = NitroStatementKeys.generate()
        self.nsm, self.encryptor, self.archive = nsm, encryptor, archive
        self.source_packet, self.clock = source_packet, clock
        self.wait_until, self.event = wait_until, event
        self.collector = SchwabOptionChainCollector(client, clock=clock)
        self.state = "NEW"
        self.statements: list[AttestedStatement] = []
        self.documents: list[bytes] = []
        self.packet_records: list[dict] = []

    @staticmethod
    def _pcr_sets(value: Any) -> list[dict[int, bytes]]:
        if not isinstance(value, list) or not 1 <= len(value) <= 32:
            raise ValueError("PCR set count outside bound")
        result, previous = [], None
        for group in value:
            if not isinstance(group, list) or not 1 <= len(group) <= 32:
                raise ValueError("PCR group size outside bound")
            pairs = []
            for entry in group:
                strict(entry, {"index", "sha384"}, "PCR")
                index, text = entry["index"], entry["sha384"]
                if (type(index) is not int or not 0 <= index < 32
                        or not isinstance(text, str) or len(text) != 96
                        or any(c not in "0123456789abcdef" for c in text)
                        or text == "00" * 48):
                    raise ValueError("invalid non-debug PCR measurement")
                pairs.append((index, bytes.fromhex(text)))
            if (pairs[0][0] != 0 or [p[0] for p in pairs] != sorted(set(p[0] for p in pairs))
                    or previous is not None and pairs <= previous):
                raise ValueError("PCR groups must be sorted, unique and contain PCR0")
            result.append(dict(pairs)); previous = pairs
        return result

    def _document(self, binding: str) -> tuple[bytes, str]:
        document = self.keys.attest_statement(self.nsm, attestation_binding_digest=binding)
        parsed = self.verifier.parse_and_verify(document, attestation_binding_digest=binding)
        if (parsed.statement_key != self.keys.statement_key
                or parsed.public_key != self.keys.recipient_public_key_der
                or not any(all(parsed.pcrs.get(i) == v for i, v in group.items())
                           for group in self.pcr_sets)):
            raise ValueError("NSM keys or PCRs differ from the approved run")
        return document, "pcr0:" + parsed.pcrs[0].hex()

    def recipient_document(self) -> bytes:
        # This bootstrap domain is separate from all collection statements.
        binding = domain_sha256("postfiat.yolo.enclave_recipient.v1", {
            "collectionManifestSha256": self.manifest_sha256,
            "statementKey": self.keys.statement_key,
        })
        return self._document(binding)[0]

    def _packet(self, kind: str, sequence: int | None, snapshot_hash: str | None) -> dict:
        document = self.recipient_document()
        packet = private_json(self.source_packet(kind, sequence, snapshot_hash, document))
        strict(packet, {"body", "signature"}, "source packet")
        body = strict(packet["body"], {"schema", "collectionManifestSha256",
            "normalizationSourcesSha256", "kind", "sequence", "privateSnapshotSha256",
            "sourceTimestampUtc", "payload"}, "source packet body")
        if (body["schema"] != PACKET_SCHEMA
                or body["collectionManifestSha256"] != self.manifest_sha256
                or body["normalizationSourcesSha256"] != self.manifest["normalizationSourcesSha256"]
                or body["kind"] != kind or body["sequence"] != sequence
                or type(body["sequence"]) is not type(sequence)
                or body["privateSnapshotSha256"] != snapshot_hash):
            raise ValueError("source packet binding differs")
        utc(body["sourceTimestampUtc"])
        try:
            signature = base64.b64decode(packet["signature"], validate=True)
            if base64.b64encode(signature).decode() != packet["signature"]:
                raise ValueError()
            Ed25519PublicKey.from_public_bytes(bytes.fromhex(self.sources["inputSignerEd25519"])).verify(
                signature, bytes.fromhex(domain_sha256(PACKET_SCHEMA, body)))
        except Exception:
            raise ValueError("source packet signature is invalid") from None
        self.packet_records.append(packet)
        return body["payload"]

    def _statement(self, kind: str, input_hash: str, output_hash: str) -> tuple[AttestedStatement, bytes]:
        # Determine the live measurement before constructing the acyclic binding.
        _, measurement = self._document(domain_sha256("postfiat.yolo.enclave_measurement.v1", {
            "collectionManifestSha256": self.manifest_sha256, "sequence": len(self.statements),
        }))
        statement = AttestedStatement(
            statement_kind=kind, epoch_sha256=self.epoch.sha256,
            input_sha256=input_hash, output_sha256=output_hash, sequence=len(self.statements),
            previous_statement_sha256=None if not self.statements else self.statements[-1].sha256,
            signature="pending", evidence=TeeEvidence(platform="aws-nitro", trust_class="attested",
                hardware_backed=True, debug_mode=False, measurement=measurement,
                statement_key=self.keys.statement_key, attestation_document_sha256="00" * 32,
                verifier_id=self.manifest["proofPolicy"]["verifierId"]),
        )
        document, actual = self._document(statement.attestation_binding_digest)
        if actual != measurement:
            raise ValueError("NSM measurement changed during a statement")
        statement = replace(statement, evidence=replace(statement.evidence,
            attestation_document_sha256=sha256_bytes(document)))
        return replace(statement, signature=self.keys.sign_digest(statement.signing_digest)), document

    def _archive(self, name: str, plaintext: bytes, aad: bytes, document: bytes) -> dict:
        if len(plaintext) > MAX_PRIVATE_BYTES:
            raise ValueError("private run artifact exceeds its bound")
        envelope = encrypt_snapshot(plaintext, associated_data=aad, encryptor=self.encryptor(document))
        return self.archive.store(run_id=self.manifest["runId"], artifact_id=name, envelope=envelope)

    def _normalize(self, captured: Any, payload: dict) -> Any:
        strict(payload, {"underlierMidpointMicrodollars", "underlierHalted",
                         "optionHaltFlags", "contractReference"}, "normalization facts")
        rows = payload["contractReference"]
        if not isinstance(rows, list) or len(rows) > 65_536:
            raise ValueError("contract reference count outside bound")
        reference = {}
        for row in rows:
            strict(row, {"occSymbol", "expirationDate", "strikeMicrodollars", "optionType",
                         "multiplier", "deliverableShares", "exerciseStyle", "standard"}, "contract reference")
            key = row["occSymbol"]
            if not isinstance(key, str) or key in reference:
                raise ValueError("contract references require unique symbols")
            for field in ("strikeMicrodollars", "multiplier", "deliverableShares"):
                if type(row[field]) is not int:
                    raise ValueError("reference numeric terms must be integers")
            reference[key] = ContractReferenceTerms(
                occ_symbol=key, expiration_date=date.fromisoformat(row["expirationDate"]),
                strike_microdollars=row["strikeMicrodollars"], option_type=row["optionType"],
                multiplier=row["multiplier"], deliverable_shares=row["deliverableShares"],
                exercise_style=row["exerciseStyle"], standard=row["standard"],
            )
        flags = payload["optionHaltFlags"]
        if not isinstance(flags, dict) or len(flags) > 65_536 or any(type(v) is not bool for v in flags.values()):
            raise ValueError("invalid option halt facts")
        symbols = {row["source"]["symbol"] for row in captured.contracts if row["mapSide"] == "CALL"}
        if not symbols <= flags.keys():
            raise ValueError("source halt facts do not cover every captured call")
        # Select facts for every captured call; no source row is filtered here.
        return methodology_snapshot_from_schwab(captured,
            underlier_midpoint_microdollars=payload["underlierMidpointMicrodollars"],
            underlier_halted=payload["underlierHalted"],
            option_halt_flags={symbol: flags[symbol] for symbol in symbols}, contract_reference=reference)

    def run(self) -> dict:
        if self.state != "NEW":
            raise ValueError("an enclave run cannot be restarted in the same process")
        self.state = "RUNNING"
        try:
            return self._run()
        except Exception:
            self.state = "FAILED"
            # Exception details can contain private contract identifiers. The
            # entrypoint reports a fixed stage/status code instead of this text.
            raise

    def _run(self) -> dict:
        prior = self._packet("prior-state", None, None)
        strict(prior, {"positions", "settledCashMicrodollars", "unsettledCashMicrodollars",
                      "accruedLiabilitiesMicrodollars", "incumbentExpiration", "incumbentSymbols"}, "prior state")
        commitments, normalized, receipts, failures = [], [], [], []
        for sequence, scheduled in enumerate(self.epoch.scheduled_at_utc):
            self.wait_until(scheduled)
            try:
                captured = self.collector.collect(self.epoch, sequence=sequence,
                    previous_snapshot_sha256=None if not commitments else commitments[-1].sha256)
            except (ValueError, RuntimeError, OSError):
                failures.append(sequence)
                self.event({"schema": "postfiat.yolo.enclave_event.v1", "runId": self.manifest["runId"],
                            "status": "COLLECTION_ATTEMPT_FAILED", "sequence": sequence})
                continue
            commitment = captured.commitment()
            receipt = self._archive(f"snapshot-{sequence}", canonical_json_bytes(captured.to_private_dict()),
                                    snapshot_archive_associated_data(captured), self.recipient_document())
            commitments.append(commitment); receipts.append(receipt)
            # No public commitment or source request precedes durable archive acknowledgement.
            self.event({"schema": "postfiat.yolo.enclave_event.v1", "runId": self.manifest["runId"],
                        "status": "SNAPSHOT_ARCHIVED", "sequence": sequence,
                        "privateSnapshotSha256": captured.sha256, "commitmentSha256": commitment.sha256})
            if failures:
                continue
            statement, document = self._statement("option-chain-snapshot", captured.sha256, commitment.sha256)
            self.statements.append(statement); self.documents.append(document)
            facts = self._packet("normalization", sequence, captured.sha256)
            normalized.append(self._normalize(captured, facts))
        collection = assess_collection(self.epoch, tuple(commitments))
        if failures or collection.state != "COMPLETE":
            self.state = "INCOMPLETE"
            return {"schema": "postfiat.yolo.enclave_result.v1", "runId": self.manifest["runId"],
                    "status": "COLLECTION_INCOMPLETE", "attemptedSnapshots": self.epoch.expected_snapshot_count,
                    "archivedSnapshots": len(commitments), "failedSequences": failures}
        value = {"schema": "postfiat.yolo.portfolio_target_input.v1",
                 "seriesId": self.manifest["seriesId"], "underlierId": self.manifest["underlierId"],
                 "tradeDate": self.manifest["tradeDate"], "methodologySha256": self.epoch.methodology_sha256,
                 "collectionSha256": collection.commitments_sha256,
                 "snapshots": [row.to_dict() for row in normalized], **prior}
        target_input = PortfolioTargetInput.from_dict(value)
        target = create_portfolio_target(target_input, self.parameters)
        binding = domain_sha256(INPUT_SCHEMA, {"schema": INPUT_SCHEMA,
            "collectionManifestSha256": self.manifest_sha256,
            "parameterManifestSha256": self.parameters.sha256, "targetInput": target_input.to_dict()})
        statement, document = self._statement("portfolio-target-input", collection.commitments_sha256, binding)
        self.statements.append(statement); self.documents.append(document)
        witness = {"schema": WITNESS_SCHEMA, "programSha256": self.program_sha256,
            "manifest": self.manifest, "parameters": self.parameters.to_dict(),
            "commitments": [row.to_dict() for row in commitments], "targetInput": target_input.to_dict(),
            "statements": [row.to_dict() for row in self.statements],
            "attestationDocumentsHex": [doc.hex() for doc in self.documents],
            "rootCertificateDerHex": self.root_der.hex()}
        encoded_witness = canonical_json_bytes(witness)
        if len(encoded_witness) > MAX_PRIVATE_BYTES:
            raise ValueError("target proof witness exceeds its bound")
        replay = {"schema": REPLAY_SCHEMA, "runId": self.manifest["runId"], "witness": witness,
                  "target": target.to_dict(), "sourcePackets": self.packet_records,
                  "snapshotArchiveReceipts": receipts}
        aad = canonical_json_bytes({"schema": "postfiat.yolo.target_replay_aad.v1",
            "runId": self.manifest["runId"], "collectionManifestSha256": self.manifest_sha256,
            "targetInputSha256": binding, "targetSha256": target.sha256})
        receipt = self._archive("target-replay", canonical_json_bytes(replay), aad, document)
        self.state = "SEALED"
        return {"schema": "postfiat.yolo.enclave_result.v1", "runId": self.manifest["runId"],
                "status": "SEALED", "methodologyStatus": target.status,
                "collectionManifestSha256": self.manifest_sha256, "collectionSha256": collection.commitments_sha256,
                "targetSha256": target.sha256, "targetInputSha256": binding,
                "witnessSha256": sha256_bytes(encoded_witness), "replayAssociatedData": json.loads(aad),
                "replayReceipt": receipt, "attemptedSnapshots": self.epoch.expected_snapshot_count,
                "archivedSnapshots": len(commitments), "statementCount": len(self.statements)}
