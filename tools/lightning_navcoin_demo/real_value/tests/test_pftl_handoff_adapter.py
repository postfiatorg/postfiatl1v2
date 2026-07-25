from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import tempfile
import unittest
from unittest.mock import patch

from ...coordinator.protocol import SecretPreimage, encode_condition
from ..pftl_effect_store import EffectStoreError, PftlEffectStore
from ..pftl_handoff import (
    HandoffError,
    PersistentPftlHandoff,
    load_persistent_handoff,
    sha256_file,
)
from ..pftl_handoff_check import HandoffCheckError, PftlHandoffDryCheck
from ..pftl_signer_backend import (
    CommandResult,
    EXECUTION_ACK,
    PersistentHandoffPftlBackend,
    PftlBackendError,
    SignerHandle,
    _signed_transaction_id,
)


CHAIN_ID = "local-pftl-proven-nav-v2-20260724"
GENESIS = "81" * 48
ROOT = "08" * 48
TIP = "c3" * 48
ASSET_ID = "f9" * 48
RESERVE_HASH = "02" * 48
PROFILE_ID = "1f" * 48
ISSUER = "pf" + "b5" * 20
COORDINATOR = "pf" + "79" * 20
USER = "pf" + "05" * 20
NAV_PER_UNIT = 1_035_074_022
SUPPLY = 3_000_000_000


def _write_executable(path: Path, data: bytes) -> str:
    path.write_bytes(data)
    os.chmod(path, 0o700)
    return sha256_file(path)


def _fixture_document(
    root: Path,
    *,
    binary_sha256: str,
    helper: Path,
) -> dict[str, object]:
    topology = root / "topology.json"
    topology.write_text(
        json.dumps(
            {
                "topology_id": "97" * 48,
                "chain_id": CHAIN_ID,
                "genesis_hash": GENESIS,
                "protocol_version": 1,
                "peers": [
                    {
                        "node_id": f"validator-{index}",
                        "host": "127.0.0.1",
                        "p2p_port": 29760 + (index * 2),
                        "rpc_port": 29761 + (index * 2),
                        "p2p_address": f"/ip4/127.0.0.1/tcp/{29760 + index * 2}",
                    }
                    for index in range(6)
                ],
            }
        )
    )
    return {
        "schema": "postfiat.lightning.navcoin.orc3_handoff.v1",
        "status": "ready",
        "chain": {
            "chain_id": CHAIN_ID,
            "genesis_hash": GENESIS,
            "height": 6,
            "state_root": ROOT,
            "validator_count": 6,
            "consensus_v2_activation_height": 1,
            "data_root": str(root),
            "topology_file": str(topology),
        },
        "binary": {
            "path": str(root / "postfiat-node"),
            "build_git_revision": "ae3c53c9",
            "sha256": binary_sha256,
        },
        "rpc": {
            "protocol": "newline-delimited JSON, postfiat-local-rpc-v1",
            "primary": "tcp://127.0.0.1:31660",
            "endpoints": [
                f"tcp://127.0.0.1:{31660 + index}" for index in range(6)
            ],
            "status_request": {
                "version": "postfiat-local-rpc-v1",
                "id": "status-1",
                "method": "status",
                "params": {},
            },
        },
        "navcoin": {
            "asset_id": ASSET_ID,
            "code": "LNNAVTEST",
            "display_name": "Proven NAVcoin",
            "precision": 6,
            "issuer": ISSUER,
            "circulating_supply_atoms": SUPPLY,
            "finalized_epoch": 1,
            "verified_net_assets_usd_e8": 3_105_222_068_834,
            "nav_per_unit_usd_e8": NAV_PER_UNIT,
            "nav_per_unit_usd": "10.35074022",
            "reserve_packet_hash": RESERVE_HASH,
            "profile_id": PROFILE_ID,
        },
        "accounts": {"coordinator": COORDINATOR, "user": USER},
        "live_escrow": {
            "escrow_id": "4b" * 48,
            "create_tx_id": "93" * 48,
            "amount_atoms": 1_000_000,
            "amount_coins": "1.000000",
            "payment_hash_sha256": "b7" * 32,
            "condition": "a0258020" + "b7" * 32 + "810120",
            "cancel_after_height": 105,
            "created_height": 6,
            "state": "open",
            "certificate_id": "a3" * 48,
            "certificate_votes": 5,
        },
        "hashlock_encoding": {
            "condition": "a0258020<sha256(preimage_bytes)>810120",
            "fulfillment": "a0228020<32_byte_preimage_hex>",
            "finish_signer": "recipient/user",
            "cancel_signer": "owner/coordinator after cancel_after_height",
            "local_certification_helper": str(helper),
        },
        "proof_assurance": {
            "lifecycle": [
                "nav_reserve_submit",
                "nav_reserve_attest",
                "nav_epoch_finalize",
            ],
            "on_chain_profile": "multi-fetch-quorum",
            "attestation_count": 1,
            "proof_bytes_stored_on_chain": True,
            "consensus_native_groth16_verification": False,
            "note": "test assurance boundary",
        },
    }


def make_handoff(root: Path) -> PersistentPftlHandoff:
    binary = root / "postfiat-node"
    helper = root / "certify-signed-escrow.sh"
    binary_sha = _write_executable(binary, b"test pinned binary\n")
    helper_sha = _write_executable(helper, b"#!/bin/sh\nexit 0\n")
    document = _fixture_document(
        root,
        binary_sha256=binary_sha,
        helper=helper,
    )
    handoff_path = root / "orc3-handoff.json"
    handoff_path.write_text(json.dumps(document, sort_keys=True))
    return load_persistent_handoff(
        handoff_path,
        expected_handoff_sha256=sha256_file(handoff_path),
        expected_certification_helper_sha256=helper_sha,
    )


class FakeFleet:
    def __init__(self) -> None:
        self.height = 6
        self.tip = TIP
        self.root = ROOT
        self.sequence = 4
        self.user_asset_balance = 0
        self.user_asset_limit = 5_000_000_000
        self.receipts: dict[str, dict[str, object]] = {}

    def client_factory(self, endpoint: str) -> "FakeClient":
        return FakeClient(self, int(endpoint.rsplit(":", 1)[1]) - 31660)


class FakeClient:
    def __init__(self, fleet: FakeFleet, index: int) -> None:
        self.fleet = fleet
        self.index = index

    def status(self) -> dict[str, object]:
        return {
            "active_nav_profiles": [
                {
                    "asset_id": ASSET_ID,
                    "finalized_epoch": 1,
                    "finalized_reserve_packet_hash": RESERVE_HASH,
                    "halted": False,
                    "min_attestations": 1,
                    "nav_per_unit": NAV_PER_UNIT,
                    "profile_id": PROFILE_ID,
                    "source_class": "sp1-groth16-existing-proof-attested",
                    "verifier_kind": "multi-fetch-quorum",
                }
            ],
            "block_height": self.fleet.height,
            "block_tip_hash": self.fleet.tip,
            "build_git_revision": "ae3c53c9",
            "chain_id": CHAIN_ID,
            "genesis_hash": GENESIS,
            "mempool_pending": 0,
            "node_id": f"validator-{self.index}",
            "protocol_version": 1,
            "rpc_schema": "postfiat-local-rpc-v1",
            "state_root": self.fleet.root,
            "status": "running",
            "validator_count": 6,
        }

    def asset_info(self, asset_id: str) -> dict[str, object]:
        return {
            "chain_id": CHAIN_ID,
            "genesis_hash": GENESIS,
            "found": True,
            "asset": {
                "asset_id": ASSET_ID,
                "issuer": ISSUER,
                "code": "LNNAVTEST",
                "display_name": "Proven NAVcoin",
                "precision": 6,
                "outstanding_supply": SUPPLY,
                "freeze_enabled": False,
                "clawback_enabled": False,
                "requires_authorization": False,
            },
        }

    def account_lines(self, account: str, **_: object) -> dict[str, object]:
        balance = (
            self.fleet.user_asset_balance
            if account == USER
            else 1_999_000_000
        )
        limit = (
            self.fleet.user_asset_limit
            if account == USER
            else 5_000_000_000
        )
        return {
            "chain_id": CHAIN_ID,
            "genesis_hash": GENESIS,
            "account": account,
            "asset_id": ASSET_ID,
            "lines": [
                {
                    "account": account,
                    "asset_id": ASSET_ID,
                    "issuer": ISSUER,
                    "code": "LNNAVTEST",
                    "precision": 6,
                    "authorized": True,
                    "frozen": False,
                    "balance": balance,
                    "limit": limit,
                }
            ],
        }

    def account(self, address: str) -> dict[str, object]:
        return {"address": address, "balance": 9_999_890, "sequence": self.fleet.sequence}

    def escrow_fee_quote_response(
        self,
        source: str,
        operation: dict[str, object],
        *,
        sequence: int,
        request_id: str,
    ) -> dict[str, object]:
        return {
            "version": "postfiat-local-rpc-v1",
            "id": request_id,
            "ok": True,
            "error": None,
            "events": [],
            "result": {
                "schema": "postfiat-escrow-fee-quote-v1",
                "chain_id": CHAIN_ID,
                "genesis_hash": GENESIS,
                "protocol_version": 1,
                "source": source,
                "sequence": sequence,
                "sequence_source": "explicit",
                "operation": operation,
                "transaction_kind": operation["operation"],
                "sender_meets_reserve_after_fee": True,
                "sender_balance": 9_999_890,
                "sender_balance_after_fee": 9_999_857,
                "account_reserve": 1_000,
                "mempool_pending_for_sender": 0,
                "minimum_fee": 33,
                "sender_sequence": sequence - 1,
            },
        }

    def receipts(self, *, tx_id: str, limit: int) -> list[dict[str, object]]:
        receipt = self.fleet.receipts.get(tx_id)
        return [] if receipt is None else [dict(receipt)]

    def escrow_info(self, escrow_id: str) -> dict[str, object]:
        return {
            "found": True,
            "escrow": {
                "escrow_id": escrow_id,
                "owner": COORDINATOR,
                "recipient": USER,
                "asset_id": ASSET_ID,
                "amount": 100,
                "condition": encode_condition("11" * 32),
                "finish_after": 0,
                "cancel_after": 500,
                "state": "open",
            },
        }


class PftlHandoffParserTests(unittest.TestCase):
    def test_exact_pins_and_assurance_are_loaded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            self.assertEqual(handoff.rpc_endpoints[0], "tcp://127.0.0.1:31660")
            self.assertEqual(handoff.binary_build_git_revision, "ae3c53c9")
            self.assertEqual(handoff.profile_id, PROFILE_ID)
            self.assertEqual(handoff.data_root, Path(directory))
            self.assertFalse(handoff.consensus_native_groth16_verification)
            self.assertEqual(
                handoff.verify_artifacts()["certification_helper"]["sha256"],
                handoff.certification_helper_sha256,
            )

    def test_handoff_digest_and_assurance_tampering_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff = make_handoff(root)
            document = json.loads(handoff.handoff_path.read_text())
            document["proof_assurance"]["consensus_native_groth16_verification"] = True
            handoff.handoff_path.write_text(json.dumps(document))
            with self.assertRaises(HandoffError):
                load_persistent_handoff(
                    handoff.handoff_path,
                    expected_handoff_sha256=handoff.handoff_sha256,
                    expected_certification_helper_sha256=(
                        handoff.certification_helper_sha256
                    ),
                )


class PftlHandoffCheckTests(unittest.TestCase):
    def test_six_of_six_read_only_check(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()
            report = PftlHandoffDryCheck(
                handoff, client_factory=fleet.client_factory
            ).run()
            self.assertTrue(report["ok"])
            self.assertEqual(report["live"]["agreeing_validator_count"], 6)
            self.assertFalse(
                report["assurance_boundary"][
                    "consensus_native_groth16_verification"
                ]
            )

    def test_one_divergent_root_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()

            def divergent(endpoint: str) -> FakeClient:
                client = fleet.client_factory(endpoint)
                if client.index == 5:
                    original = client.status

                    def status() -> dict[str, object]:
                        result = original()
                        result["state_root"] = "ff" * 48
                        return result

                    client.status = status  # type: ignore[method-assign]
                return client

            with self.assertRaises(HandoffCheckError):
                PftlHandoffDryCheck(handoff, client_factory=divergent).run()

    def test_mid_bundle_finalized_view_change_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()

            def racing(endpoint: str) -> FakeClient:
                client = fleet.client_factory(endpoint)
                if client.index == 5:
                    original = client.status
                    calls = 0

                    def status() -> dict[str, object]:
                        nonlocal calls
                        calls += 1
                        result = original()
                        if calls == 2:
                            result["block_height"] = int(
                                result["block_height"]
                            ) + 1
                            result["block_tip_hash"] = "6a" * 48
                            result["state_root"] = "6b" * 48
                        return result

                    client.status = status  # type: ignore[method-assign]
                return client

            with self.assertRaisesRegex(
                HandoffCheckError, "finalized view changed"
            ):
                PftlHandoffDryCheck(
                    handoff, client_factory=racing
                ).run()


class EffectStoreTests(unittest.TestCase):
    def test_v1_effect_journal_migrates_without_losing_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "effects.sqlite3"
            connection = sqlite3.connect(path)
            connection.executescript(
                """
                CREATE TABLE pftl_effects (
                    effect_key TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    request_sha256 TEXT NOT NULL,
                    signer_address TEXT NOT NULL,
                    signer_sequence INTEGER,
                    escrow_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    signed_artifact_path TEXT,
                    signed_artifact_sha256 TEXT,
                    tx_id TEXT,
                    evidence_json TEXT,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL,
                    CHECK(status IN ('PLANNED','SIGNED','SUBMITTING','SUCCEEDED'))
                );
                INSERT INTO pftl_effects VALUES(
                    'old-effect', 'PFTL_ESCROW_CREATE',
                    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                    'pf7979797979797979797979797979797979797979',
                    5,
                    'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                    'PLANNED', NULL, NULL, NULL, NULL, 1, 1
                );
                """
            )
            connection.close()
            store = PftlEffectStore(path)
            self.assertEqual(store.get("old-effect")["status"], "PLANNED")
            connection = sqlite3.connect(path)
            schema = connection.execute(
                "SELECT sql FROM sqlite_master WHERE name = 'pftl_effects'"
            ).fetchone()[0]
            connection.close()
            self.assertIn("'REJECTED'", schema)

    def test_effect_key_is_idempotent_and_conflicts_fail(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            store = PftlEffectStore(Path(directory) / "effects.sqlite3")
            request = {"kind": "create", "amount": 1}
            first = store.begin(
                effect_key="swap-1:create",
                kind="PFTL_ESCROW_CREATE",
                request=request,
                signer_address=COORDINATOR,
                escrow_id="4b" * 48,
                signer_sequence=5,
            )
            second = store.begin(
                effect_key="swap-1:create",
                kind="PFTL_ESCROW_CREATE",
                request=request,
                signer_address=COORDINATOR,
                escrow_id="4b" * 48,
                signer_sequence=5,
            )
            self.assertEqual(first["request_sha256"], second["request_sha256"])
            with self.assertRaises(EffectStoreError):
                store.begin(
                    effect_key="swap-1:create",
                    kind="PFTL_ESCROW_CREATE",
                    request={"kind": "create", "amount": 2},
                    signer_address=COORDINATOR,
                    escrow_id="4b" * 48,
                    signer_sequence=5,
                )

    def test_rejected_receipt_is_a_distinct_idempotent_terminal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            store = PftlEffectStore(root / "effects.sqlite3")
            effect_key = "swap-rejected:create"
            store.begin(
                effect_key=effect_key,
                kind="PFTL_ESCROW_CREATE",
                request={"kind": "create", "amount": 1},
                signer_address=COORDINATOR,
                escrow_id="4b" * 48,
                signer_sequence=5,
            )
            signed = root / "signed.json"
            signed.write_text("{}")
            store.mark_signed(
                effect_key,
                signed_artifact_path=signed,
                signed_artifact_sha256=sha256_file(signed),
            )
            evidence = {
                "tx_id": "7a" * 48,
                "accepted": False,
                "code": "insufficient_balance",
                "mutation_free": True,
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "finalized_height": 8,
                "state_root": "7b" * 48,
                "block_tip_hash": "7c" * 48,
            }
            first = store.mark_rejected(
                effect_key, tx_id=evidence["tx_id"], evidence=evidence
            )
            second = store.mark_rejected(
                effect_key, tx_id=evidence["tx_id"], evidence=evidence
            )
            self.assertEqual(first["status"], "REJECTED")
            self.assertEqual(second["evidence"], evidence)
            with self.assertRaisesRegex(
                EffectStoreError, "literal accepted receipt"
            ):
                store.mark_succeeded(
                    effect_key,
                    tx_id=evidence["tx_id"],
                    evidence=evidence,
                )


class BackendTests(unittest.TestCase):
    def test_every_signer_bundle_rejects_a_mid_read_tip_race(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()
            backend = PersistentHandoffPftlBackend(
                handoff, client_factory=fleet.client_factory
            )
            raced = backend._clients[5]
            original_status = raced.status
            call_count = 0

            def moving_status() -> dict[str, object]:
                nonlocal call_count
                call_count += 1
                result = original_status()
                if call_count % 2 == 0:
                    result["block_height"] = int(result["block_height"]) + 1
                    result["block_tip_hash"] = "3a" * 48
                    result["state_root"] = "3b" * 48
                return result

            raced.status = moving_status
            create = {
                "operation": "escrow_create",
                "owner": COORDINATOR,
                "recipient": USER,
                "asset_id": ASSET_ID,
                "amount": 100,
                "condition": encode_condition("11" * 32),
                "finish_after": 0,
                "cancel_after": 500,
            }
            operations = (
                lambda: backend._six_account_sequence(COORDINATOR),
                lambda: backend._six_asset_account_view(COORDINATOR),
                lambda: backend._six_fee_eligibility(
                    source=COORDINATOR,
                    operation=create,
                    sequence=5,
                    request_label="escrow-create",
                ),
                lambda: backend._receipt_quorum("3c" * 48),
                lambda: backend._open_escrow("3d" * 48),
            )
            for operation in operations:
                with self.subTest(operation=operation), self.assertRaisesRegex(
                    PftlBackendError, "finalized view changed during RPC read"
                ):
                    operation()
            raced.status = original_status
            quote_client = backend._clients[0]
            original_quote_status = quote_client.status
            quote_status_calls = 0

            def quote_race() -> dict[str, object]:
                nonlocal quote_status_calls
                quote_status_calls += 1
                result = original_quote_status()
                # Calls one/two are the six-validator fee gate. Calls
                # three/four sandwich the exact response sent to the signer.
                if quote_status_calls == 4:
                    result["block_height"] = int(result["block_height"]) + 1
                    result["block_tip_hash"] = "3e" * 48
                    result["state_root"] = "3f" * 48
                return result

            quote_client.status = quote_race
            with self.assertRaisesRegex(
                PftlBackendError, "signing-quote finalized view changed"
            ):
                backend._quote(
                    source=COORDINATOR,
                    operation=create,
                    sequence=5,
                    request_id="quote-race",
                )

    def test_literal_consensus_rejection_is_consumable_but_never_success(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff = make_handoff(root)
            fleet = FakeFleet()
            backend = PersistentHandoffPftlBackend(
                handoff, client_factory=fleet.client_factory
            )
            tx_id = "4e" * 48
            fleet.receipts[tx_id] = {
                "tx_id": tx_id,
                "accepted": False,
                "code": "insufficient_balance",
                "message": "escrow transaction rejected",
                "fee_charged": 0,
                "fee_burned": 0,
                "minimum_fee": 0,
                "account_reserve": 0,
                "state_expansion_fee": 0,
            }
            receipt = backend._receipt_quorum(tx_id)
            self.assertIsNotNone(receipt)
            self.assertIs(receipt["accepted"], False)
            self.assertIs(receipt["mutation_free"], True)

            store = PftlEffectStore(root / "journal" / "effects.sqlite3")
            effect_key = "literal-rejection"
            store.begin(
                effect_key=effect_key,
                kind="PFTL_ESCROW_CREATE",
                request={"kind": "create"},
                signer_address=COORDINATOR,
                escrow_id="4f" * 48,
                signer_sequence=5,
            )
            signed = root / "signed.json"
            signed.write_text("{}")
            store.mark_signed(
                effect_key,
                signed_artifact_path=signed,
                signed_artifact_sha256=sha256_file(signed),
            )
            terminal = backend._complete_receipt(
                store, effect_key, tx_id, receipt
            )
            effect = backend._effect_from_evidence(terminal["evidence"])
            self.assertEqual(terminal["status"], "REJECTED")
            self.assertIs(effect.accepted, False)
            self.assertIs(effect.mutation_free, True)

            unsafe = dict(fleet.receipts[tx_id])
            unsafe["fee_charged"] = 1
            fleet.receipts[tx_id] = unsafe
            with self.assertRaisesRegex(
                PftlBackendError, "zero-effect consensus receipt"
            ):
                backend._receipt_quorum(tx_id)

    def test_signed_artifact_replace_fsyncs_file_and_parent_directory(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            temporary = root / "signed.tmp"
            destination = root / "signed.json"
            temporary.write_bytes(b'{"signed":true}\n')
            observed: list[str] = []
            real_fsync = os.fsync

            def recording_fsync(descriptor: int) -> None:
                mode = os.fstat(descriptor).st_mode
                observed.append(
                    "file" if stat.S_ISREG(mode) else
                    "directory" if stat.S_ISDIR(mode) else
                    "other"
                )
                real_fsync(descriptor)

            with patch(
                "tools.lightning_navcoin_demo.real_value."
                "pftl_signer_backend.os.fsync",
                side_effect=recording_fsync,
            ):
                PersistentHandoffPftlBackend._durable_replace_signed(
                    temporary, destination
                )
            self.assertEqual(observed, ["file", "directory"])
            self.assertFalse(temporary.exists())
            self.assertEqual(destination.read_bytes(), b'{"signed":true}\n')
            self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)

    def test_missing_durable_signed_artifact_holds_signed_and_submitting(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff = make_handoff(root)
            signer_file = root / "coordinator.key.json"
            signer_file.write_bytes(b"opaque signer handle")
            os.chmod(signer_file, 0o600)
            store = PftlEffectStore(root / "journal" / "effects.sqlite3")
            artifact_dir = root / "artifacts"
            backend = PersistentHandoffPftlBackend(
                handoff,
                signer=SignerHandle(signer_file, COORDINATOR),
                effect_store=store,
                artifact_dir=artifact_dir,
                execution_ack=EXECUTION_ACK,
                client_factory=FakeFleet().client_factory,
            )
            plan = backend.plan_create(
                owner=COORDINATOR,
                recipient=USER,
                asset_id=ASSET_ID,
                amount_atoms=100,
                condition=encode_condition("11" * 32),
                finish_after=0,
                cancel_after=500,
            )
            for desired_status in ("SIGNED", "SUBMITTING"):
                with self.subTest(status=desired_status):
                    effect_key = f"missing-{desired_status.lower()}"
                    request = {
                        "schema": "postfiat.lightning.pftl_effect_request.v1",
                        "chain_id": CHAIN_ID,
                        "genesis_hash": GENESIS,
                        "kind": "PFTL_ESCROW_CREATE",
                        "operation": dict(plan.operation),
                        "escrow_id": plan.expected_escrow_id,
                    }
                    store.begin(
                        effect_key=effect_key,
                        kind="PFTL_ESCROW_CREATE",
                        request=request,
                        signer_address=COORDINATOR,
                        escrow_id=plan.expected_escrow_id,
                        signer_sequence=plan.owner_sequence,
                    )
                    effect_tag = hashlib.sha256(
                        effect_key.encode("ascii")
                    ).hexdigest()
                    missing = (
                        artifact_dir
                        / f"{effect_tag}.signed-escrow.json"
                    )
                    store.mark_signed(
                        effect_key,
                        signed_artifact_path=missing,
                        signed_artifact_sha256="00" * 32,
                    )
                    if desired_status == "SUBMITTING":
                        store.mark_submitting(effect_key)
                    with self.assertRaisesRegex(
                        PftlBackendError, "artifact is unavailable"
                    ):
                        backend._execute(
                            effect_key=effect_key,
                            kind="PFTL_ESCROW_CREATE",
                            operation=plan.operation,
                            signer_sequence=plan.owner_sequence,
                            escrow_id=plan.expected_escrow_id,
                        )
                    self.assertEqual(
                        store.get(effect_key)["status"], desired_status
                    )

    def test_terminal_escrow_retries_reconcile_durable_effect_before_open_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff = make_handoff(root)
            signer_file = root / "coordinator.key.json"
            signer_file.write_bytes(b"opaque signer handle")
            os.chmod(signer_file, 0o600)

            class DurableSubmitting:
                @staticmethod
                def get(_effect_key: str) -> dict[str, object]:
                    return {"status": "SUBMITTING"}

            backend = PersistentHandoffPftlBackend(
                handoff,
                signer=SignerHandle(signer_file, COORDINATOR),
                effect_store=PftlEffectStore(root / "journal" / "effects.sqlite3"),
                artifact_dir=root / "artifacts",
                execution_ack=EXECUTION_ACK,
                client_factory=FakeFleet().client_factory,
            )
            backend.effect_store = DurableSubmitting()  # type: ignore[assignment]
            calls: list[dict[str, object]] = []
            marker = object()
            backend._open_escrow = (  # type: ignore[method-assign]
                lambda _escrow_id: self.fail(
                    "terminal consensus escrow must not block durable reconciliation"
                )
            )
            backend._execute = (  # type: ignore[method-assign]
                lambda **kwargs: calls.append(dict(kwargs)) or marker
            )
            escrow_id = "4b" * 48
            secret = SecretPreimage(bytes.fromhex("12" * 32))
            self.assertIs(
                backend.submit_finish(
                    owner=USER,
                    recipient=COORDINATOR,
                    escrow_id=escrow_id,
                    secret=secret,
                    effect_key="finish-recovery",
                ),
                marker,
            )
            self.assertIs(
                backend.submit_cancel(
                    owner=COORDINATOR,
                    escrow_id=escrow_id,
                    effect_key="cancel-recovery",
                ),
                marker,
            )
            self.assertEqual(
                [call["kind"] for call in calls],
                ["PFTL_ESCROW_FINISH", "PFTL_ESCROW_CANCEL"],
            )

    def test_unarmed_backend_plans_but_cannot_execute(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()
            backend = PersistentHandoffPftlBackend(
                handoff,
                client_factory=fleet.client_factory,
            )
            plan = backend.plan_create(
                owner=COORDINATOR,
                recipient=USER,
                asset_id=ASSET_ID,
                amount_atoms=100,
                condition=encode_condition("11" * 32),
                finish_after=0,
                cancel_after=500,
            )
            self.assertEqual(plan.owner_sequence, 5)
            with self.assertRaises(PftlBackendError):
                backend.submit_create(plan, effect_key="swap-2:create")

    def test_user_owned_reverse_plan_never_uses_coordinator_signer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()
            fleet.user_asset_balance = 1_000
            backend = PersistentHandoffPftlBackend(
                handoff,
                client_factory=fleet.client_factory,
            )
            plan = backend.plan_create(
                owner=USER,
                recipient=COORDINATOR,
                asset_id=ASSET_ID,
                amount_atoms=100,
                condition=encode_condition("22" * 32),
                finish_after=0,
                cancel_after=500,
            )
            self.assertEqual(plan.owner, USER)
            self.assertEqual(plan.recipient, COORDINATOR)
            with self.assertRaises(PftlBackendError):
                backend.submit_create(plan, effect_key="swap-reverse:create")

    def test_onramp_plan_requires_recipient_navcoin_headroom(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            handoff = make_handoff(Path(directory))
            fleet = FakeFleet()
            fleet.user_asset_balance = fleet.user_asset_limit
            backend = PersistentHandoffPftlBackend(
                handoff,
                client_factory=fleet.client_factory,
            )
            with self.assertRaisesRegex(PftlBackendError, "recipient trustline headroom"):
                backend.plan_create(
                    owner=COORDINATOR,
                    recipient=USER,
                    asset_id=ASSET_ID,
                    amount_atoms=100,
                    condition=encode_condition("23" * 32),
                    finish_after=0,
                    cancel_after=500,
                )

    def test_armed_backend_signs_certifies_and_reconciles_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff = make_handoff(root)
            fleet = FakeFleet()
            signer_file = root / "coordinator.key.json"
            signer_file.write_bytes(b"opaque signer contents never opened by adapter")
            os.chmod(signer_file, 0o600)
            calls: list[str] = []

            def runner(
                command: list[str] | tuple[str, ...],
                stdout_path: Path,
                timeout_seconds: float,
            ) -> CommandResult:
                self.assertGreater(timeout_seconds, 0)
                if command[1] == "wallet-sign-escrow-transaction":
                    calls.append("sign")
                    quote_path = Path(command[command.index("--quote-file") + 1])
                    quote = json.loads(quote_path.read_text())
                    result = quote["result"]
                    operation = result["operation"]
                    unsigned = {
                        "chain_id": result["chain_id"],
                        "genesis_hash": result["genesis_hash"],
                        "protocol_version": 1,
                        "address_namespace": "postfiat",
                        "transaction_kind": operation["operation"],
                        "signature_algorithm_id": "ML-DSA-65",
                        "source": result["source"],
                        "fee": result["minimum_fee"],
                        "sequence": result["sequence"],
                        **operation,
                    }
                    signed = {
                        "unsigned": unsigned,
                        "algorithm_id": "ML-DSA-65",
                        "public_key_hex": "aa",
                        "signature_hex": "bb",
                    }
                    stdout_path.write_text(json.dumps(signed))
                    os.chmod(stdout_path, 0o600)
                    return CommandResult(0, b"")
                calls.append("certify")
                signed = json.loads(Path(command[1]).read_text())
                tx_id = _signed_transaction_id(signed)
                fleet.height = 7
                fleet.tip = "d4" * 48
                fleet.root = "e5" * 48
                fleet.sequence = 5
                fleet.receipts[tx_id] = {
                    "tx_id": tx_id,
                    "accepted": True,
                    "code": "accepted",
                    "message": "escrow transaction applied; fee burned",
                }
                stdout_path.write_text("certified\n")
                os.chmod(stdout_path, 0o600)
                return CommandResult(0, b"")

            backend = PersistentHandoffPftlBackend(
                handoff,
                signer=SignerHandle(signer_file, COORDINATOR),
                effect_store=PftlEffectStore(root / "journal" / "effects.sqlite3"),
                artifact_dir=root / "artifacts",
                execution_ack=EXECUTION_ACK,
                client_factory=fleet.client_factory,
                command_runner=runner,
            )
            plan = backend.plan_create(
                owner=COORDINATOR,
                recipient=USER,
                asset_id=ASSET_ID,
                amount_atoms=100,
                condition=encode_condition("11" * 32),
                finish_after=0,
                cancel_after=500,
            )
            first = backend.submit_create(plan, effect_key="swap-3:create")
            second = backend.submit_create(plan, effect_key="swap-3:create")
            self.assertEqual(calls, ["sign", "certify"])
            self.assertTrue(first.accepted)
            self.assertEqual(first.tx_id, second.tx_id)
            self.assertEqual(first.agreeing_validator_count, 6)


if __name__ == "__main__":
    unittest.main()
