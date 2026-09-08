"""Focused vectors and boundary tests for the UNL V2 evidence contract."""

from __future__ import annotations

import copy
import hashlib
import json
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

from eth_keys import keys

from postfiat_rpc.tasknode_unl_schema import (
    TaskNodeUnlError,
    canonical_json_bytes,
    format_utc_timestamp,
)
from postfiat_rpc.tasknode_unl_v2_evidence import (
    bilateral_statement_digest,
    control_declaration_statement_digest,
    control_epoch_from_event,
    control_registry_root,
    evidence_input_root,
    policy_root,
    seal_evidence_snapshot,
    sign_bilateral_record,
    sign_control_declaration,
    verify_evidence_snapshot,
)
from postfiat_rpc.tasknode_unl_v2_schema import (
    EVIDENCE_LIMITATIONS,
    V2_BILATERAL_RECORD_SCHEMA,
    V2_CONTROL_DECLARATION_SCHEMA,
    V2_CONTROL_EVENT_SCHEMA,
    V2_CONTROL_REGISTRY_SCHEMA,
    V2_EVIDENCE_INPUT_SCHEMA,
    V2_FUNDING_OBSERVATION_SCHEMA,
    V2_POLICY_ID,
    V2_SCORE_EVIDENCE_SCHEMA,
    V2_SIGNATURE_ALGORITHM,
    V2_WINDOW_DAYS,
    V2_WINDOW_SCHEMA,
    v2_header,
)

UTC = timezone.utc
CHAIN_ID = "pftl-testnet"
GENESIS_HASH = hashlib.sha256(b"TEST-ONLY:V2:genesis").hexdigest()
FIXTURE_PATH = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "tasknode_unl_v2"
    / "evidence-golden.json"
)


class ThrowawayEvidenceSigner:
    """Deterministic test-only signer; no production custody is represented."""

    algorithm_id = V2_SIGNATURE_ALGORITHM

    def __init__(self, label: str) -> None:
        scalar = hashlib.sha256(f"TEST-ONLY:V2:{label}".encode()).digest()
        self._test_only_key = keys.PrivateKey(scalar)
        self.public_key_hex = (
            self._test_only_key.public_key.to_compressed_bytes().hex()
        )

    def sign_digest(self, digest: bytes) -> bytes:
        return self._test_only_key.sign_msg_hash(digest).to_bytes()


def _digest(label: str) -> str:
    return hashlib.sha256(f"TEST-ONLY:V2:{label}".encode()).hexdigest()


def _time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def _window(index: int, start: datetime) -> dict:
    return {
        **v2_header(V2_WINDOW_SCHEMA),
        "index": index,
        "start": format_utc_timestamp(start),
        "end": format_utc_timestamp(start + timedelta(days=V2_WINDOW_DAYS)),
        "days": V2_WINDOW_DAYS,
    }


def _signers() -> dict[str, ThrowawayEvidenceSigner]:
    return {
        name: ThrowawayEvidenceSigner(name)
        for name in ("account-alice", "account-bob", "account-carol")
    }


def _registry(signers: dict[str, ThrowawayEvidenceSigner]) -> dict:
    entries = []
    for account in sorted(signers):
        entries.append(
            {
                **v2_header(V2_CONTROL_EVENT_SCHEMA),
                "policy_id": V2_POLICY_ID,
                "chain_id": CHAIN_ID,
                "genesis_hash": GENESIS_HASH,
                "account_id": account,
                "event_kind": "baseline_binding",
                "binding_event_digest": _digest(f"epoch:{account}"),
                "event_at": "2026-01-01T00:00:00Z",
                "public_key_hex": signers[account].public_key_hex,
                "authorization_source": "binding",
                "incumbent": account == "account-alice",
            }
        )
    return {
        **v2_header(V2_CONTROL_REGISTRY_SCHEMA),
        "policy_id": V2_POLICY_ID,
        "chain_id": CHAIN_ID,
        "genesis_hash": GENESIS_HASH,
        "entries": entries,
    }


def _bilateral(
    registry: dict,
    signers: dict[str, ThrowawayEvidenceSigner],
    *,
    kind: str,
    source: str = "account-alice",
    target: str = "account-bob",
    action: str = "grant",
    statement: str | None = None,
    finalized_window: int = 6,
    effective_window: int | None = None,
    expiry_window: int = 9,
) -> dict:
    epochs = {
        entry["account_id"]: entry["binding_event_digest"]
        for entry in registry["entries"]
    }
    record = {
        **v2_header(V2_BILATERAL_RECORD_SCHEMA),
        "policy_id": V2_POLICY_ID,
        "chain_id": CHAIN_ID,
        "genesis_hash": GENESIS_HASH,
        "action": action,
        "relation_kind": kind,
        "source_account": source,
        "target_account": target,
        "source_epoch": epochs[source],
        "target_epoch": epochs[target],
        "statement_digest": _digest(statement or f"{kind}:{source}:{target}"),
        "work_completed_at": (
            "2026-01-02T00:00:00Z" if kind == "cowork" else None
        ),
        "acknowledged_at": "2026-01-03T00:00:00Z",
        "ledger_finality_digest": _digest(
            f"finality:{finalized_window}"
        ),
        "finalized_window": finalized_window,
        "effective_window": (
            finalized_window + 1
            if effective_window is None
            else effective_window
        ),
        "expiry_window": expiry_window,
        "signatures": [],
    }
    required_signers = (
        (source, target) if action in ("grant", "renew") else (source,)
    )
    record["signatures"] = sorted(
        (
            sign_bilateral_record(
                record,
                account_id=account,
                signer=signers[account],
            )
            for account in required_signers
        ),
        key=lambda item: item["account_id"],
    )
    return record


def _declaration(
    registry: dict,
    signers: dict[str, ThrowawayEvidenceSigner],
    *,
    members: tuple[str, ...] = ("account-alice", "account-bob"),
    action: str = "declare",
    statement: str = "control-group-one",
    finalized_window: int = 6,
    effective_window: int | None = None,
    expiry_window: int = 9,
) -> dict:
    epochs = {
        entry["account_id"]: entry["binding_event_digest"]
        for entry in registry["entries"]
    }
    delay = 1 if action == "declare" else 2
    record = {
        **v2_header(V2_CONTROL_DECLARATION_SCHEMA),
        "policy_id": V2_POLICY_ID,
        "chain_id": CHAIN_ID,
        "genesis_hash": GENESIS_HASH,
        "action": action,
        "statement_digest": _digest(statement),
        "members": [
            {"account_id": account, "control_epoch": epochs[account]}
            for account in sorted(members)
        ],
        "acknowledged_at": "2026-01-03T00:00:00Z",
        "ledger_finality_digest": _digest(
            f"finality:{finalized_window}"
        ),
        "finalized_window": finalized_window,
        "effective_window": (
            finalized_window + delay
            if effective_window is None
            else effective_window
        ),
        "expiry_window": expiry_window,
        "signatures": [],
    }
    record["signatures"] = [
        sign_control_declaration(
            record,
            account_id=account,
            signer=signers[account],
        )
        for account in sorted(members)
    ]
    return record


def _score(entry: dict, kind: str) -> dict:
    account = entry["account_id"]
    return {
        **v2_header(V2_SCORE_EVIDENCE_SCHEMA),
        "account_id": account,
        "control_epoch": entry["binding_event_digest"],
        "evidence_kind": kind,
        "evidence_digest": _digest(f"score:{account}:{kind}"),
        "evidence_at": "2026-01-04T00:00:00Z",
    }


def _build_vector() -> dict:
    signers = _signers()
    registry = _registry(signers)
    vouch = _bilateral(registry, signers, kind="vouch")
    cowork = _bilateral(registry, signers, kind="cowork")
    declaration = _declaration(registry, signers)
    evidence = {
        **v2_header(V2_EVIDENCE_INPUT_SCHEMA),
        "policy_id": V2_POLICY_ID,
        "chain_id": CHAIN_ID,
        "genesis_hash": GENESIS_HASH,
        "window": _window(7, _time("2026-01-01T00:00:00Z")),
        "score_evidence": [
            _score(entry, kind)
            for entry in registry["entries"]
            for kind in ("work", "quality", "standing", "accountability")
        ],
        "funding_observations": [
            {
                **v2_header(V2_FUNDING_OBSERVATION_SCHEMA),
                "observation_digest": _digest("funding:alice:carol"),
                "observation_kind": "first_funder",
                "source_account": "account-alice",
                "target_account": "account-carol",
                "observed_at": "2026-03-01T00:00:00Z",
                "value_units": 1,
                "source_record_digests": [_digest("funding-transfer-one")],
            }
        ],
        "bilateral_records": [vouch, cowork],
        "control_declarations": [declaration],
    }
    snapshot = seal_evidence_snapshot(evidence, registry)
    result = verify_evidence_snapshot(snapshot, registry)
    return {
        "fixture_schema": "tasknode-unl-v2-evidence-golden-v1",
        "control_registry": registry,
        "evidence": evidence,
        "snapshot": snapshot,
        "vectors": {
            "policy_root": policy_root(),
            "input_root": evidence_input_root(evidence),
            "registry_root": control_registry_root(registry),
            "vouch_signed_digest": bilateral_statement_digest(vouch).hex(),
            "cowork_signed_digest": bilateral_statement_digest(cowork).hex(),
            "declaration_signed_digest": (
                control_declaration_statement_digest(declaration).hex()
            ),
            "snapshot_sha256": hashlib.sha256(
                canonical_json_bytes(snapshot)
            ).hexdigest(),
            "result_sha256": hashlib.sha256(
                result.canonical_bytes()
            ).hexdigest(),
        },
        "expected_result": result.to_dict(),
    }


def _fixture() -> dict:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _verify_vector(vector: dict):
    return verify_evidence_snapshot(
        vector["snapshot"],
        vector["control_registry"],
        expected_policy_root=vector["vectors"]["policy_root"],
        expected_input_root=vector["vectors"]["input_root"],
        expected_registry_root=vector["vectors"]["registry_root"],
    )


class GoldenVectorTests(unittest.TestCase):
    def test_committed_vector_is_byte_identical_across_two_runs(self) -> None:
        fixture = _fixture()
        first = _verify_vector(copy.deepcopy(fixture))
        second = _verify_vector(json.loads(json.dumps(fixture)))

        self.assertEqual(first.status, "verified")
        self.assertEqual(first.canonical_bytes(), second.canonical_bytes())
        self.assertEqual(first.to_dict(), fixture["expected_result"])
        self.assertEqual(_build_vector(), fixture)

    def test_unknown_top_level_version_holds_with_named_field(self) -> None:
        fixture = _fixture()
        fixture["snapshot"]["version"] = 99

        result = _verify_vector(fixture)

        self.assertEqual(result.status, "hold")
        self.assertEqual(result.failures[0].code, "unknown_version")
        self.assertEqual(result.failures[0].field, "snapshot.version")

    def test_unknown_record_version_is_rejected_and_accounts_hold(self) -> None:
        fixture = _fixture()
        fixture["evidence"]["bilateral_records"][0]["version"] = 99
        fixture["snapshot"] = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )

        result = verify_evidence_snapshot(
            fixture["snapshot"], fixture["control_registry"]
        )

        self.assertEqual(result.status, "verified_with_holds")
        self.assertEqual(result.record_rejections[0].code, "unknown_version")
        self.assertEqual(
            result.record_rejections[0].field,
            "evidence.bilateral_records[0].version",
        )
        self.assertIn("account-alice", result.hold_accounts)
        self.assertEqual(result.active_vouches, ())

    def test_mismatched_commitment_holds_with_named_root(self) -> None:
        fixture = _fixture()
        fixture["snapshot"]["commitments"]["input_root"] = "00" * 32

        result = verify_evidence_snapshot(
            fixture["snapshot"], fixture["control_registry"]
        )

        self.assertEqual(result.status, "hold")
        self.assertEqual(result.failures[0].code, "commitment_mismatch")
        self.assertEqual(
            result.failures[0].field, "snapshot.commitments.input_root"
        )

    def test_acknowledgement_chain_identity_mismatch_is_isolated(self) -> None:
        fixture = _fixture()
        fixture["evidence"]["bilateral_records"][0]["chain_id"] = (
            "another-chain"
        )
        fixture["snapshot"] = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )

        result = verify_evidence_snapshot(
            fixture["snapshot"], fixture["control_registry"]
        )

        self.assertEqual(result.status, "verified_with_holds")
        self.assertEqual(result.record_rejections[0].code, "chain_id_mismatch")
        self.assertEqual(
            result.record_rejections[0].field,
            "evidence.bilateral_records[0].chain_id",
        )
        self.assertIn("account-alice", result.hold_accounts)

    def test_acknowledgement_with_stale_control_epoch_is_rejected(self) -> None:
        fixture = _fixture()
        fixture["evidence"]["bilateral_records"][0]["source_epoch"] = (
            "00" * 32
        )
        fixture["snapshot"] = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )

        result = verify_evidence_snapshot(
            fixture["snapshot"], fixture["control_registry"]
        )

        self.assertEqual(result.status, "verified_with_holds")
        self.assertEqual(
            result.record_rejections[0].code, "relation_epoch_mismatch"
        )
        self.assertEqual(
            result.record_rejections[0].field,
            "evidence.bilateral_records[0].source_epoch",
        )


class ControlEpochTests(unittest.TestCase):
    def test_non_owner_cannot_reset_another_accounts_epoch(self) -> None:
        event = copy.deepcopy(_fixture()["control_registry"]["entries"][0])
        digest = event["binding_event_digest"]

        with self.assertRaises(TaskNodeUnlError) as caught:
            control_epoch_from_event(event, {digest: "account-bob"})

        self.assertEqual(caught.exception.code, "control_event_owner_mismatch")
        self.assertEqual(caught.exception.detail, "control_event.account_id")

    def test_accusations_unsolicited_transfers_and_signing_do_not_reset(self) -> None:
        event = copy.deepcopy(_fixture()["control_registry"]["entries"][0])
        digest = event["binding_event_digest"]
        for untrusted_kind in (
            "accusation",
            "unsolicited_transfer",
            "routine_signing",
        ):
            with self.subTest(event_kind=untrusted_kind):
                event["event_kind"] = untrusted_kind
                with self.assertRaises(TaskNodeUnlError) as caught:
                    control_epoch_from_event(
                        event, {digest: event["account_id"]}
                    )
                self.assertEqual(
                    caught.exception.code, "unauthorized_control_event_kind"
                )

    def test_each_authorized_public_event_starts_its_digest_epoch(self) -> None:
        base = copy.deepcopy(_fixture()["control_registry"]["entries"][0])
        for kind in (
            "declared_transfer",
            "validator_key_replacement",
            "wallet_binding_replacement",
            "recovery",
        ):
            with self.subTest(event_kind=kind):
                event = copy.deepcopy(base)
                event["event_kind"] = kind
                event["authorization_source"] = (
                    "recovery" if kind == "recovery" else "binding"
                )
                event["binding_event_digest"] = _digest(f"event:{kind}")
                epoch = control_epoch_from_event(
                    event,
                    {event["binding_event_digest"]: event["account_id"]},
                )
                self.assertEqual(epoch.epoch, event["binding_event_digest"])

    def test_fresh_window_passes_at_exactly_180_days(self) -> None:
        fixture = _fixture()

        exact = _verify_vector(fixture)

        alice = next(
            item for item in exact.continuity if item.account_id == "account-alice"
        )
        self.assertEqual(alice.status, "READY")
        self.assertEqual(alice.fresh_score_evidence, 4)
        self.assertGreaterEqual(alice.renewed_vouches, 1)
        self.assertGreaterEqual(alice.post_epoch_cowork, 1)

        registry = copy.deepcopy(fixture["control_registry"])
        registry["entries"][0]["event_at"] = "2026-01-01T00:00:01Z"
        snapshot = seal_evidence_snapshot(fixture["evidence"], registry)
        before_boundary = verify_evidence_snapshot(snapshot, registry)
        alice_before = next(
            item
            for item in before_boundary.continuity
            if item.account_id == "account-alice"
        )
        self.assertEqual(alice_before.status, "HOLD_CONTINUITY")
        self.assertIn("fresh_window_incomplete", alice_before.reasons)
        self.assertTrue(alice_before.retain_incumbent)

    def test_incumbent_key_rotation_holds_continuity_without_eviction(self) -> None:
        fixture = _fixture()
        registry = copy.deepcopy(fixture["control_registry"])
        alice = registry["entries"][0]
        alice["event_kind"] = "validator_key_replacement"
        alice["event_at"] = "2026-06-01T00:00:00Z"
        evidence = copy.deepcopy(fixture["evidence"])
        evidence["bilateral_records"] = []
        evidence["control_declarations"] = []
        for score in evidence["score_evidence"]:
            if score["account_id"] == "account-alice":
                score["evidence_at"] = "2026-06-02T00:00:00Z"
        snapshot = seal_evidence_snapshot(evidence, registry)

        result = verify_evidence_snapshot(snapshot, registry)
        assessment = next(
            item
            for item in result.continuity
            if item.account_id == "account-alice"
        )

        self.assertEqual(assessment.status, "HOLD_CONTINUITY")
        self.assertTrue(assessment.incumbent)
        self.assertTrue(assessment.retain_incumbent)


class ConsentClassTests(unittest.TestCase):
    def test_all_three_consent_classes_round_trip_separately(self) -> None:
        result = _verify_vector(_fixture())

        self.assertEqual(len(result.audit_funding_observations), 1)
        self.assertEqual(
            result.audit_funding_observations[0]["observation_kind"],
            "first_funder",
        )
        self.assertEqual(len(result.active_vouches), 1)
        self.assertEqual(len(result.active_cowork), 1)
        self.assertEqual(len(result.active_control_declarations), 1)
        self.assertNotEqual(
            result.active_vouches[0].credit_id,
            result.active_cowork[0].credit_id,
        )
        self.assertIn("audit-only", EVIDENCE_LIMITATIONS[2])
        serialized = json.dumps(result.to_dict(), sort_keys=True).lower()
        self.assertNotIn("sale_detected", serialized)
        self.assertNotIn("personhood_verified", serialized)

    def test_duplicate_bilateral_record_never_multiplies_credit(self) -> None:
        fixture = _fixture()
        fixture["evidence"]["bilateral_records"].append(
            copy.deepcopy(fixture["evidence"]["bilateral_records"][0])
        )
        snapshot = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )

        result = verify_evidence_snapshot(snapshot, fixture["control_registry"])

        self.assertEqual(len(result.active_vouches), 1)
        self.assertEqual(result.status, "verified_with_holds")
        self.assertEqual(
            result.record_rejections[0].code, "replayed_acknowledgement"
        )

    def test_revocation_takes_effect_only_in_the_next_window(self) -> None:
        fixture = _fixture()
        signers = _signers()
        revoke = _bilateral(
            fixture["control_registry"],
            signers,
            kind="vouch",
            action="revoke",
            finalized_window=7,
            effective_window=8,
        )
        fixture["evidence"]["bilateral_records"].append(revoke)
        current_snapshot = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )
        current = verify_evidence_snapshot(
            current_snapshot, fixture["control_registry"]
        )
        self.assertEqual(len(current.active_vouches), 1)

        fixture["evidence"]["window"] = _window(
            8, _time("2026-06-30T00:00:00Z")
        )
        next_snapshot = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )
        following = verify_evidence_snapshot(
            next_snapshot, fixture["control_registry"]
        )
        self.assertEqual(following.active_vouches, ())

    def test_control_dissolution_waits_one_full_window(self) -> None:
        fixture = _fixture()
        dissolve = _declaration(
            fixture["control_registry"],
            _signers(),
            action="dissolve",
            finalized_window=7,
            effective_window=9,
            expiry_window=9,
        )
        fixture["evidence"]["control_declarations"].append(dissolve)

        fixture["evidence"]["window"] = _window(
            8, _time("2026-06-30T00:00:00Z")
        )
        during_delay = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )
        self.assertEqual(
            len(
                verify_evidence_snapshot(
                    during_delay, fixture["control_registry"]
                ).active_control_declarations
            ),
            1,
        )

        fixture["evidence"]["window"] = _window(
            9, _time("2026-12-27T00:00:00Z")
        )
        after_delay = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )
        self.assertEqual(
            verify_evidence_snapshot(
                after_delay, fixture["control_registry"]
            ).active_control_declarations,
            (),
        )

    def test_malformed_declaration_is_isolated_from_valid_snapshot(self) -> None:
        fixture = _fixture()
        malformed = fixture["evidence"]["control_declarations"][0]
        malformed["signatures"][0]["signature_hex"] = "00" * 65
        valid = _declaration(
            fixture["control_registry"],
            _signers(),
            members=("account-bob", "account-carol"),
            statement="control-group-two",
        )
        fixture["evidence"]["control_declarations"].append(valid)
        snapshot = seal_evidence_snapshot(
            fixture["evidence"], fixture["control_registry"]
        )

        result = verify_evidence_snapshot(snapshot, fixture["control_registry"])

        self.assertEqual(result.status, "verified_with_holds")
        self.assertEqual(result.failures, ())
        self.assertEqual(len(result.record_rejections), 1)
        self.assertEqual(
            result.record_rejections[0].record_class, "control_declaration"
        )
        self.assertIn("account-alice", result.hold_accounts)
        self.assertEqual(len(result.active_control_declarations), 1)
        self.assertEqual(
            result.active_control_declarations[0].members,
            ("account-bob", "account-carol"),
        )


if __name__ == "__main__":
    unittest.main()
