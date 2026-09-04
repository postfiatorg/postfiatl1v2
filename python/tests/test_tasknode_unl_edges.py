"""Focused tests for deterministic public Task Node UNL edge extraction."""

from __future__ import annotations

import copy
import hashlib
import json
import unittest
from fractions import Fraction
from pathlib import Path

from eth_keys import keys

from postfiat_rpc import tasknode_unl_schema as schema
from postfiat_rpc.tasknode_unl_binding import (
    wallet_address_from_public_key,
)
from postfiat_rpc.tasknode_unl_edges import (
    extract_cowork_edges,
    extract_funding_edges,
    extract_public_edges,
    extract_vouch_edges,
    sign_vouch_statement,
)
from postfiat_rpc.tasknode_unl_schema import TaskNodeUnlError
from postfiat_rpc.tasknode_unl_trust_graph import (
    TrustGraphEvidence,
    build_raw_rows,
    derive_trust_graph,
)

FIXTURE_DIR = (
    Path(__file__).resolve().parent / "fixtures" / "tasknode_unl"
)


def _fixture(name: str) -> dict:
    return json.loads(
        (FIXTURE_DIR / name).read_text(encoding="utf-8")
    )


def _vouches() -> dict:
    return copy.deepcopy(_fixture("vouch-memos.json"))


def _cowork() -> dict:
    return copy.deepcopy(_fixture("cowork-pointers.json"))


def _funding() -> dict:
    return copy.deepcopy(_fixture("funding-transfers.json"))


def _exclusions() -> dict:
    return copy.deepcopy(_fixture("funding-exclusions.json"))


def _edge_pairs(extracted, kind: str) -> set[tuple[str, str]]:
    return {
        (edge.source, edge.target)
        for edge in extracted.edges
        if edge.kind == kind
    }


class ThrowawayVouchSigner:
    """Deterministic in-test signer derived only from a TEST-ONLY label."""

    algorithm_id = schema.VOUCH_SIGNATURE_ALGORITHM

    def __init__(self, label: str = "edge-alice") -> None:
        scalar = hashlib.sha256(f"TEST-ONLY:{label}".encode()).digest()
        self._test_only_key = keys.PrivateKey(scalar)
        self.public_key_hex = (
            self._test_only_key.public_key.to_compressed_bytes().hex()
        )

    def sign_digest(self, digest: bytes) -> bytes:
        return self._test_only_key.sign_msg_hash(digest).to_bytes()


class VouchEdgeTests(unittest.TestCase):
    def test_valid_signed_public_ledger_vouch_produces_directed_edge(
        self,
    ) -> None:
        extracted = extract_vouch_edges(_vouches())

        self.assertEqual(len(extracted.edges), 1)
        edge = extracted.edges[0]
        self.assertEqual(edge.kind, "vouch")
        self.assertEqual(edge.source, "account-alice")
        self.assertEqual(edge.target, "account-bob")
        self.assertEqual(
            extracted.provenance[0].qualification_reasons,
            ("signed_public_ledger_vouch",),
        )

    def test_fixture_signature_uses_only_throwaway_test_vector(self) -> None:
        document = _vouches()
        statement = document["memos"][0]["memo"]["statement"]
        signer = ThrowawayVouchSigner()

        self.assertEqual(
            wallet_address_from_public_key(signer.public_key_hex),
            statement["source_wallet_address"],
        )
        self.assertEqual(
            sign_vouch_statement(statement, signer),
            document["memos"][0]["memo"],
        )

    def test_private_or_encrypted_message_is_rejected_not_skipped(
        self,
    ) -> None:
        document = _vouches()
        document["memos"][0]["visibility"] = "encrypted"

        with self.assertRaises(TaskNodeUnlError) as caught:
            extract_vouch_edges(document)

        self.assertEqual(caught.exception.code, "private_vouch_forbidden")

        combined = extract_public_edges(
            vouch_ledger=document,
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=_exclusions(),
        )
        self.assertEqual(combined.status, "hold")
        self.assertEqual(combined.edges, ())
        self.assertTrue(
            combined.hold_reasons[0].startswith(
                "private_vouch_forbidden:"
            )
        )

    def test_forged_vouch_signature_is_rejected(self) -> None:
        document = _vouches()
        document["memos"][0]["memo"]["signature_hex"] = "00" * 65

        with self.assertRaises(TaskNodeUnlError) as caught:
            extract_vouch_edges(document)

        self.assertIn(
            caught.exception.code,
            {
                "signature_verification_failed",
                "signature_recovery_failed",
                "signature_recovery_mismatch",
            },
        )


class CoworkEdgeTests(unittest.TestCase):
    def test_hive_projects_and_team_grants_produce_edges(self) -> None:
        extracted = extract_cowork_edges(_cowork())
        evidence_ids = {edge.evidence_id for edge in extracted.edges}

        self.assertIn(
            "cowork:hive_project:hive-one", evidence_ids
        )
        self.assertIn(
            "cowork:team_grant:team-grant-one", evidence_ids
        )
        self.assertNotIn(
            ("account-alice", "account-carol"),
            _edge_pairs(extracted, "cowork"),
        )

    def test_distinct_unit_provenance_is_preserved_and_walk_caps_at_three(
        self,
    ) -> None:
        extracted = extract_cowork_edges(_cowork())

        pair_edges = [
            edge
            for edge in extracted.edges
            if (edge.source, edge.target)
            == ("account-alice", "account-bob")
        ]
        self.assertEqual(len(pair_edges), 5)
        self.assertEqual(len(extracted.provenance), 5)

        rows = build_raw_rows(
            ("account-alice", "account-bob"),
            tuple(pair_edges),
        )
        self.assertEqual(
            rows["account-alice"]["account-bob"],
            Fraction(3, 1),
        )
        self.assertEqual(
            rows["account-bob"]["account-alice"],
            Fraction(3, 1),
        )

    def test_repeated_pointer_is_deduplicated_without_new_edge(self) -> None:
        document = _cowork()
        baseline = extract_cowork_edges(document)
        document["pointers"].append(
            copy.deepcopy(document["pointers"][0])
        )

        repeated = extract_cowork_edges(document)

        self.assertEqual(repeated, baseline)


class FundingEdgeTests(unittest.TestCase):
    def test_first_funder_and_strict_majority_each_produce_an_edge(
        self,
    ) -> None:
        extracted = extract_funding_edges(_funding(), _exclusions())
        pairs = _edge_pairs(extracted, "funding")

        self.assertIn(("account-carol", "account-dave"), pairs)
        self.assertIn(("account-alice", "account-carol"), pairs)

        reasons = {
            (item.source, item.target): item.qualification_reasons
            for item in extracted.provenance
        }
        self.assertTrue(
            any(
                reason.startswith("first_funder:")
                for reason in reasons[
                    ("account-carol", "account-dave")
                ]
            )
        )
        self.assertTrue(
            any(
                reason.startswith("majority_inflow:")
                for reason in reasons[
                    ("account-alice", "account-carol")
                ]
            )
        )
        majority_provenance = next(
            item
            for item in extracted.provenance
            if (item.source, item.target)
            == ("account-alice", "account-carol")
        )
        self.assertEqual(
            set(majority_provenance.source_record_ids),
            {"24" * 32, "25" * 32},
        )

    def test_one_off_transfer_to_already_funded_wallet_is_not_edge(
        self,
    ) -> None:
        extracted = extract_funding_edges(_funding(), _exclusions())

        self.assertNotIn(
            ("account-alice", "account-bob"),
            _edge_pairs(extracted, "funding"),
        )

    def test_exactly_half_is_not_a_majority_edge(self) -> None:
        document = _funding()
        alice_to_carol = next(
            transfer
            for transfer in document["transfers"]
            if transfer["tx_hash"] == "24" * 32
        )
        alice_to_carol["value_units"] = 40

        extracted = extract_funding_edges(document, _exclusions())

        self.assertNotIn(
            ("account-alice", "account-carol"),
            _edge_pairs(extracted, "funding"),
        )

    def test_first_funder_dust_is_still_visible_as_funding_relation(
        self,
    ) -> None:
        extracted = extract_funding_edges(_funding(), _exclusions())
        provenance = next(
            item
            for item in extracted.provenance
            if (item.source, item.target)
            == ("account-carol", "account-dave")
        )

        self.assertIn("23" * 32, provenance.source_record_ids)
        self.assertTrue(
            any(
                reason.startswith("first_funder:")
                for reason in provenance.qualification_reasons
            )
        )

    def test_exclusion_list_suppresses_distribution_wallet_edges(
        self,
    ) -> None:
        excluded = extract_funding_edges(_funding(), _exclusions())
        no_longer_excluded = _exclusions()
        no_longer_excluded["addresses"] = [
            item
            for item in no_longer_excluded["addresses"]
            if item["category"] != "foundation_distribution"
        ]
        unsuppressed = extract_funding_edges(
            _funding(), no_longer_excluded
        )

        self.assertFalse(
            any(
                "account-foundation" in pair
                for pair in _edge_pairs(excluded, "funding")
            )
        )
        self.assertTrue(
            any(
                "account-foundation" in pair
                for pair in _edge_pairs(unsuppressed, "funding")
            )
        )

    def test_missing_or_stale_exclusion_input_holds_combined_output(
        self,
    ) -> None:
        missing = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=None,
        )
        stale_exclusions = _exclusions()
        stale_exclusions["valid_until"] = "2026-06-01T00:00:00Z"
        stale = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=stale_exclusions,
        )

        self.assertEqual(missing.status, "hold")
        self.assertEqual(missing.edges, ())
        self.assertTrue(
            missing.hold_reasons[0].startswith(
                "missing_funding_exclusion_list:"
            )
        )
        self.assertEqual(stale.status, "hold")
        self.assertEqual(stale.edges, ())
        self.assertTrue(
            stale.hold_reasons[0].startswith(
                "stale_funding_exclusion_list:"
            )
        )

    def test_malformed_exclusion_input_holds_combined_output(
        self,
    ) -> None:
        exclusions = _exclusions()
        exclusions["addresses"][0]["category"] = "unpublished-category"

        extracted = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=exclusions,
        )

        self.assertEqual(extracted.status, "hold")
        self.assertEqual(extracted.edges, ())
        self.assertTrue(
            extracted.hold_reasons[0].startswith(
                "unknown_exclusion_category:"
            )
        )

    def test_conflicting_transfer_rows_are_rejected(self) -> None:
        document = _funding()
        conflict = copy.deepcopy(document["transfers"][0])
        conflict["target_wallet_address"] = (
            "rGi5FS9oYuDdnFRFkrQJaSeDZVGQF5x93W"
        )
        document["transfers"].append(conflict)

        with self.assertRaises(TaskNodeUnlError) as caught:
            extract_funding_edges(document, _exclusions())

        self.assertEqual(
            caught.exception.code, "conflicting_transfer_record"
        )


class CombinedExtractionTests(unittest.TestCase):
    def test_input_order_does_not_change_canonical_output(self) -> None:
        baseline = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=_exclusions(),
        )
        vouches = _vouches()
        vouches["memos"] += copy.deepcopy(vouches["memos"])
        vouches["memos"].reverse()
        cowork = _cowork()
        cowork["pointers"] += [copy.deepcopy(cowork["pointers"][0])]
        cowork["pointers"].reverse()
        funding = _funding()
        funding["transfers"] += [
            copy.deepcopy(funding["transfers"][0])
        ]
        funding["transfers"].reverse()
        funding["wallet_accounts"].reverse()
        exclusions = _exclusions()
        exclusions["addresses"].reverse()

        reordered = extract_public_edges(
            vouch_ledger=vouches,
            cowork_pointers=cowork,
            funding_transfers=funding,
            funding_exclusions=exclusions,
        )

        self.assertEqual(baseline.status, "extracted")
        self.assertEqual(
            reordered.canonical_bytes(), baseline.canonical_bytes()
        )

    def test_mismatched_source_windows_hold_without_partial_edges(
        self,
    ) -> None:
        cowork = _cowork()
        cowork["window"]["start"] = "2026-03-09T12:00:00Z"
        cowork["window"]["end"] = "2026-09-05T12:00:00Z"

        extracted = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=cowork,
            funding_transfers=_funding(),
            funding_exclusions=_exclusions(),
        )

        self.assertEqual(extracted.status, "hold")
        self.assertEqual(extracted.edges, ())
        self.assertEqual(
            extracted.hold_reasons,
            ("edge_window_mismatch:edge_sources.window",),
        )

    def test_extracted_edges_feed_step_two_walk_without_adaptation(
        self,
    ) -> None:
        extracted = extract_public_edges(
            vouch_ledger=_vouches(),
            cowork_pointers=_cowork(),
            funding_transfers=_funding(),
            funding_exclusions=_exclusions(),
        )
        evidence = TrustGraphEvidence(
            nodes=(
                "account-alice",
                "account-bob",
                "account-carol",
                "account-dave",
            ),
            ratified_nodes=(
                "account-alice",
                "account-bob",
                "account-carol",
                "account-dave",
            ),
            foundation_bound_nodes=(),
            edges=extracted.edges,
            baseline_list_size=4,
            seats=(),
        )

        result = derive_trust_graph(evidence)

        self.assertEqual(extracted.status, "extracted")
        self.assertEqual(result.status, "scored")
        rows = {
            source: dict(targets)
            for source, targets in result.raw_rows
        }
        self.assertEqual(
            rows["account-alice"]["account-bob"],
            Fraction(4, 1),
        )
        self.assertEqual(
            rows["account-alice"]["account-carol"],
            Fraction(2, 1),
        )


if __name__ == "__main__":
    unittest.main()
