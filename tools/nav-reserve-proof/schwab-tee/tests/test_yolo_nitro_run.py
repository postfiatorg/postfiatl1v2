"""Synthetic NSM/source/KMS qualification of the complete enclave coordinator."""

import base64
from datetime import date, timedelta
import json

import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

from test_yolo_methodology import parameters, snapshots, target_input
from test_yolo_nitro_attestation import _certificates, _document
from test_yolo_nitro_runtime import Kms
from pftl_schwab_tee.canonical import canonical_json_bytes, domain_sha256, sha256_bytes
from pftl_schwab_tee.models import CollectionEpoch, OptionChainQuery
from pftl_schwab_tee.nitro_kms import NitroKmsEnvelopeEncryptor, decrypt_nitro_ciphertext_bundle
from pftl_schwab_tee.nitro_run import NitroTargetRun, MANIFEST_SCHEMA, SOURCES_SCHEMA, PACKET_SCHEMA
from pftl_schwab_tee.nitro_transport import NitroHttpResponse


PROGRAM = "9aa40d86cc478a01fd7c6b2a63308188fb7e671079bd3321b405e7ac1a275cff"


class Harness:
    def __init__(self, *, fail_call=None, bad_packet=None, bad_pcr=False, archive_failure=False, halted=False,
                 program_sha256=PROGRAM):
        self.observations = snapshots(trade_date=date(2026, 9, 4),
            expirations=((date(2026, 11, 3), (48, 49, 50, 51, 52)),), underlier_halted=halted)
        original = target_input(self.observations, trade_date=date(2026, 9, 4)).to_dict()
        self.prior = {key: original[key] for key in ("positions", "settledCashMicrodollars",
            "unsettledCashMicrodollars", "accruedLiabilitiesMicrodollars", "incumbentExpiration", "incumbentSymbols")}
        self.clock_value = self.observations[0].scheduled_at_utc
        self.index, self.calls, self.events, self.archived = 0, [], [], {}
        self.fail_call, self.bad_packet, self.bad_pcr = fail_call, bad_packet, bad_pcr
        self.archive_failure, self.halted = archive_failure, halted
        self.certificates = _certificates()
        self.signer = Ed25519PrivateKey.generate()
        sources = {"schema": SOURCES_SCHEMA, "calendarId": "synthetic-calendar", "calendarVersion": "1",
            "contractReferenceId": "synthetic-reference", "contractReferenceVersion": "1",
            "haltSourceId": "synthetic-halts", "underlierQuoteSourceId": "synthetic-quotes",
            "inputSignerEd25519": self.signer.public_key().public_bytes_raw().hex()}
        epoch = CollectionEpoch(epoch_id="synthetic-phase4-run", methodology_sha256=original["methodologySha256"],
            underlier="HOOD", source_id="synthetic-schwab", account_application_identity_sha256="bb" * 32,
            collector_code_sha256="cc" * 32, prior_epoch_sha256=None,
            required_response_fields=("callExpDateMap", "numberOfContracts", "putExpDateMap", "symbol"),
            query=OptionChainQuery(symbol="HOOD", contract_type="ALL", strategy="SINGLE",
                include_underlying_quote=True, from_date=date(2026, 9, 4), to_date=date(2026, 11, 3), strike_count=None),
            window_start_utc=self.observations[0].scheduled_at_utc,
            window_end_utc=self.observations[-1].scheduled_at_utc, expected_snapshot_count=5,
            snapshot_offsets_seconds=(0, 450, 900, 1350, 1800))
        root_der = self.certificates[1].public_bytes(serialization.Encoding.DER)
        self.manifest = {"schema": MANIFEST_SCHEMA, "runId": "synthetic-phase4-run",
            "seriesId": original["seriesId"], "underlierId": original["underlierId"],
            "tradeDate": original["tradeDate"], "parameterManifestSha256": parameters().sha256,
            "normalizationSourcesSha256": domain_sha256(SOURCES_SCHEMA, sources), "epoch": epoch.to_dict(),
            "proofPolicy": {"schema": "postfiat.yolo.nitro_proof_policy.v1",
                "rootCertificateSha256": sha256_bytes(root_der),
                "approvedPcrSets": [[{"index": i, "sha384": bytes([0x11 + i]).hex() * 48} for i in range(3)]],
                "maxAgeMs": 1_800_000, "maxFutureSkewMs": 1000, "timeRule": "EPOCH_WINDOW_END",
                "statementRule": "COLLECTION_AND_TARGET_INPUT_V1", "verifierId": "synthetic-nitro-v2"}}
        self.run = NitroTargetRun(manifest=self.manifest, parameters=parameters().to_dict(), sources=sources,
            program_sha256=program_sha256, root_der=root_der, nsm=self, encryptor=self.encryptor, archive=self,
            source_packet=self.packet, client=self, clock=lambda: self.clock_value,
            wait_until=self.wait, event=self.events.append)
        self.kms = Kms(self.run.keys.recipient_private_key.public_key())

    def attest(self, *, user_data, nonce, public_key):
        body = json.loads(user_data)
        return _document(timestamp=self.clock_value, certificates=self.certificates,
            statement_key=body["statementKey"], statement_digest=body["statementBindingDigest"],
            nonce=nonce, recipient_key=self.run.keys.recipient_private_key,
            all_pcrs_zero=self.bad_pcr)[0]

    def encryptor(self, document):
        return NitroKmsEnvelopeEncryptor(kms_client=self.kms, key_id="synthetic-key",
            attestation_document=document, recipient_private_key=self.run.keys.recipient_private_key,
            encryption_context={"runId": self.manifest["runId"]})

    def store(self, *, run_id, artifact_id, envelope):
        if self.archive_failure: raise OSError("synthetic archive failure")
        assert artifact_id not in self.archived
        self.archived[artifact_id] = envelope
        return {"runId": run_id, "artifactId": artifact_id, "ciphertextSha256": envelope.ciphertext_sha256}

    def wait(self, scheduled):
        self.clock_value = scheduled + timedelta(seconds=1)

    def request(self, method, path, *, params=None, payload=None):
        index = len(self.calls)
        self.calls.append((method, path))
        if index == self.fail_call: raise OSError("synthetic network failure")
        observation = self.observations[index]
        strikes = {}
        for row in observation.contracts:
            strikes[str(row.strike_microdollars // 1_000_000)] = [{
                "symbol": row.occ_symbol, "putCall": "CALL", "expirationDate": row.expiration_date.isoformat(),
                "strikePrice": row.strike_microdollars // 1_000_000, "bid": 4.9, "ask": 5.1,
                "bidSize": 10, "askSize": 10, "totalVolume": 500, "openInterest": 2500,
                "quoteTimeInLong": int(self.clock_value.timestamp() * 1000), "multiplier": 100}]
        value = {"symbol": "HOOD", "numberOfContracts": 5, "callExpDateMap": {"2026-11-03:60": strikes},
                 "putExpDateMap": {}}
        return value, NitroHttpResponse(200, {}, canonical_json_bytes(value))

    def packet(self, kind, sequence, snapshot_hash, document):
        if kind == "prior-state": payload = self.prior
        else:
            assert f"snapshot-{sequence}" in self.archived
            assert self.events[-1]["status"] == "SNAPSHOT_ARCHIVED"
            observation = self.observations[sequence]
            payload = {"underlierMidpointMicrodollars": 50_000_000, "underlierHalted": self.halted,
                "optionHaltFlags": {row.occ_symbol: False for row in observation.contracts},
                "contractReference": [{"occSymbol": row.occ_symbol, "expirationDate": row.expiration_date.isoformat(),
                    "strikeMicrodollars": row.strike_microdollars, "optionType": "CALL", "multiplier": 100,
                    "deliverableShares": 100, "exerciseStyle": "AMERICAN", "standard": True}
                    for row in observation.contracts]}
        body = {"schema": PACKET_SCHEMA, "collectionManifestSha256": self.run.manifest_sha256,
            "normalizationSourcesSha256": self.manifest["normalizationSourcesSha256"], "kind": kind,
            "sequence": sequence, "privateSnapshotSha256": snapshot_hash,
            "sourceTimestampUtc": self.clock_value.isoformat(), "payload": payload}
        signature = self.signer.sign(bytes.fromhex(domain_sha256(PACKET_SCHEMA, body)))
        if self.bad_packet and kind == "normalization":
            if self.bad_packet == "signature": signature = b"\0" * 64
            elif self.bad_packet == "payload": body["payload"]["underlierMidpointMicrodollars"] += 1
            elif self.bad_packet == "sequence": body["sequence"] = (sequence + 1) % 5
            elif self.bad_packet == "source": body["normalizationSourcesSha256"] = "00" * 32
            elif self.bad_packet == "snapshot": body["privateSnapshotSha256"] = "00" * 32
        return canonical_json_bytes({"body": body, "signature": base64.b64encode(signature).decode()})

    def replay(self, result):
        envelope = self.archived["target-replay"]
        return json.loads(decrypt_nitro_ciphertext_bundle(envelope.ciphertext,
            associated_data=canonical_json_bytes(result["replayAssociatedData"]), kms_client=self.kms,
            attestation_document=self.run.recipient_document(), recipient_private_key=self.run.keys.recipient_private_key))


def test_enclave_run_captures_normalizes_seals_and_exports_only_ciphertext():
    fixture = Harness()
    result = fixture.run.run()
    assert result["status"] == "SEALED" and result["methodologyStatus"] == "TARGET_COMPUTED"
    assert result["statementCount"] == 6 and result["archivedSnapshots"] == 5
    assert fixture.calls == [("GET", "/marketdata/v1/chains")] * 5
    replay = fixture.replay(result)
    assert replay["target"]["status"] == "TARGET_COMPUTED"
    assert len(replay["witness"]["statements"]) == 6
    assert len(replay["sourcePackets"]) == 6
    assert sha256_bytes(canonical_json_bytes(replay["witness"])) == result["witnessSha256"]
    public = canonical_json_bytes({"result": result, "events": fixture.events})
    for field in (b"strikePrice", b"settledCashMicrodollars", b"targetQuantity", b"privateKey"):
        assert field not in public
    with pytest.raises(ValueError, match="restarted"):
        fixture.run.run()


@pytest.mark.parametrize("bad_packet", ["signature", "payload", "sequence", "source", "snapshot"])
def test_enclave_rejects_source_substitution_without_terminal_witness(bad_packet):
    fixture = Harness(bad_packet=bad_packet)
    with pytest.raises(ValueError): fixture.run.run()
    assert fixture.run.state == "FAILED" and "target-replay" not in fixture.archived


def test_archive_failure_prevents_any_public_snapshot_commitment():
    fixture = Harness(archive_failure=True)
    with pytest.raises(OSError): fixture.run.run()
    assert fixture.events == [] and not fixture.run.statements


def test_debug_nsm_document_cannot_release_or_seal_run():
    fixture = Harness(bad_pcr=True)
    with pytest.raises(ValueError, match="PCRs"): fixture.run.run()
    assert not fixture.calls and not fixture.archived


def test_failed_scheduled_call_still_attempts_all_five_without_false_proof():
    fixture = Harness(fail_call=2)
    result = fixture.run.run()
    assert result["status"] == "COLLECTION_INCOMPLETE"
    assert result["failedSequences"] == [2] and len(fixture.calls) == 5
    assert "target-replay" not in fixture.archived


def test_locked_halt_status_is_preserved_by_the_runtime():
    fixture = Harness(halted=True)
    result = fixture.run.run()
    assert result["status"] == "SEALED" and result["methodologyStatus"] == "HALTED_INSTRUMENT"
