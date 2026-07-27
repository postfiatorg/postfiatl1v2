from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "pfusdc-conservation-identity.py"
SPEC = importlib.util.spec_from_file_location("pfusdc_conservation_identity", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
ASSET = "a" * 96


def host(index: int, *, height: int = 310, tip: str = "tip", root: str = "root", pending: int = 0) -> dict:
    return {"validator_id": f"validator-{index}", "status": {"node_id": f"validator-{index}", "status": "running", "validator_count": 6, "mempool_pending": pending, "block_height": height, "block_tip_hash": tip, "state_root": root}}


def audit(*, v: int = 101, s: int = 90, d: int = 5, b: int = 8, r: int = 2, height: int = 310) -> dict:
    return {"asset_id": ASSET, "current_height": height, "source_vault_atoms": v, "live_claim_atoms": s, "uncredited_deposit_atoms": d, "burned_unsettled_atoms": b, "released_unsettled_atoms": r, "expected_source_vault_atoms": s + d + b - r, "unexplained_delta_atoms": v - (s + d + b - r), "conserved": v == s + d + b - r, "issued_supply_atoms": s, "wrapped_supply_atoms": s, "nav_subscription_claim_atoms": 0, "other_claim_atoms": 0, "recognized_but_unallocated_atoms": d, "observed_but_uncounted_atoms": 0, "route_count": 1, "deposit_count": 1, "redemption_count": 1, "routes": [], "deposits": [], "redemptions": []}


def route(chain_id: int) -> dict:
    return {
        "profile_hash": "profile-hash", "route_id": "route-id", "route_epoch": 7,
        "source_chain_id": chain_id, "vault_address": "0xvault", "token_address": "0xtoken",
        "vault_balance_atoms": 101, "balance_counted_once": True,
        "activation_height": 123, "expires_at_height": 456,
        "current_for_new_ingress": False,
    }


class ConservationIdentityTests(unittest.TestCase):
    def test_balanced_state(self) -> None:
        result = MODULE.audit_identity(audit(), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")
        self.assertEqual("verified", result["status"])
        self.assertEqual("0", result["residual_atoms"])

    def test_one_atom_drift(self) -> None:
        result = MODULE.audit_identity(audit(v=100), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")
        self.assertEqual("violated", result["status"])
        self.assertEqual("-1", result["residual_atoms"])

    def test_six_host_divergence(self) -> None:
        rows = [host(index) for index in range(6)]
        rows[-1] = host(5, height=311, tip="later", root="later")
        with self.assertRaises(MODULE.CheckerError): MODULE.common_parent(rows, 6)

    def test_missing_host(self) -> None:
        with self.assertRaises(MODULE.CheckerError): MODULE.common_parent([host(index) for index in range(5)], 6)

    def test_negative_and_overflow_atoms(self) -> None:
        with self.assertRaises(MODULE.CheckerError): MODULE.audit_identity(audit(v=-1), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")
        with self.assertRaises(MODULE.CheckerError): MODULE.audit_identity(audit(v=0, s=MODULE.MAX_U64, d=1, b=0, r=0), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")

    def test_inconsistent_term_source_height(self) -> None:
        with self.assertRaises(MODULE.CheckerError): MODULE.audit_identity(audit(height=309), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")

    def test_missing_local_cast_is_execution_blocked(self) -> None:
        verdict = MODULE.execution_blocked(ASSET, "local cast binary is missing", [])
        self.assertEqual("execution_blocked", verdict["status"])
        self.assertIsNone(verdict["components"])

    def test_snapshot_height_root_mismatch(self) -> None:
        with self.assertRaises(MODULE.CheckerError): MODULE.bind_snapshot({"block_height": 309, "block_tip_hash": "tip", "state_root": "root"}, {"block_height": 310, "block_tip_hash": "tip", "state_root": "root"}, (310, "tip", "root"))

    def test_import_verification_failure_is_execution_blocked(self) -> None:
        snapshot = {"manifest_sha256": "manifest-hash"}
        verdict = MODULE.execution_blocked(
            ASSET, "local finalized-checkpoint verification failed", [],
            (310, "tip", "root"), snapshot,
        )
        self.assertEqual("execution_blocked", verdict["status"])
        self.assertIsNone(verdict["components"])
        self.assertEqual(310, verdict["height"])
        self.assertEqual(snapshot, verdict["snapshot"])

    def test_blocked_legacy_audit_is_not_an_identity_violation(self) -> None:
        verdict = MODULE.execution_blocked(
            ASSET, "local audit could not execute", [], (310, "tip", "root"),
            source_rpc_url="https://explicit-legacy-rpc",
            legacy_label="deprecated-Arbitrum-legacy",
            legacy_finding={"path": "finding.md", "sha256": "finding-hash"},
        )
        self.assertEqual("execution_blocked", verdict["status"])
        self.assertIsNone(verdict["components"])
        self.assertEqual("deprecated-Arbitrum-legacy", verdict["legacy_backing_migration"]["classification"])

    def test_successful_locally_audited_snapshot(self) -> None:
        manifest = {"block_height": 310, "block_tip_hash": "tip", "state_root": "root"}
        MODULE.bind_snapshot(manifest, manifest, (310, "tip", "root"))
        self.assertEqual("verified", MODULE.audit_identity(audit(), ASSET, (310, "tip", "root"), "https://source", [], None, "hash")["status"])

    def test_arbitrum_sepolia_requires_explicit_legacy_label(self) -> None:
        report = audit()
        report["routes"] = [route(421_614)]
        with self.assertRaises(MODULE.CheckerError):
            MODULE.audit_identity(report, ASSET, (310, "tip", "root"), "https://source", [], None, "audit-hash")

    def test_arbitrum_sepolia_emits_exact_legacy_finding(self) -> None:
        report = audit()
        report["routes"] = [route(421_614)]
        result = MODULE.audit_identity(
            report, ASSET, (310, "tip", "root"), "https://source", [],
            "deprecated-Arbitrum-legacy", "audit-hash",
            {"path": "finding.md", "sha256": "finding-hash"},
        )
        self.assertEqual({
            "classification": "deprecated-Arbitrum-legacy",
            "canonical_finding": {"path": "finding.md", "sha256": "finding-hash"},
            "evidence_hash": "audit-hash",
            "pftl_finalized_height": 310,
            "routes": [{
                "source_chain_id": 421_614,
                "vault_address": "0xvault",
                "token_address": "0xtoken",
                "vault_balance_atoms": "101",
                "activation_height": 123,
                "expires_at_height": 456,
                "profile_hash": "profile-hash",
                "route_id": "route-id",
                "route_epoch": 7,
                "current_for_new_ingress": False,
            }],
            "finding": "lineage finding only; pre-Ethereum supply must be reconciled to Ethereum backing or redeemed out",
        }, result["legacy_backing_migration"])

    def test_legacy_route_requires_canonical_finding(self) -> None:
        report = audit()
        report["routes"] = [route(421_614)]
        with self.assertRaises(MODULE.CheckerError):
            MODULE.audit_identity(
                report, ASSET, (310, "tip", "root"), "https://source", [],
                "deprecated-Arbitrum-legacy", "audit-hash",
            )

    def test_existing_signed_snapshot_metadata_is_compact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot_dir = Path(directory)
            (snapshot_dir / "snapshot_manifest.json").write_text(
                '{"block_height":310,"block_tip_hash":"tip","state_root":"root"}'
            )
            (snapshot_dir / "snapshot.signed-manifest.json").write_text(
                '{"schema":"postfiat.signed_snapshot_manifest.v1","publisher":"pf-test"}'
            )
            snapshot = MODULE.existing_signed_snapshot(snapshot_dir)
        self.assertEqual("supplied_signed_finalized_checkpoint", snapshot["source"])
        self.assertEqual("postfiat.signed_snapshot_manifest.v1", snapshot["signed_manifest_schema"])
        self.assertNotIn("signed_manifest", snapshot)

    def test_reused_import_requires_hash_bound_six_host_parent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = []
            for index in range(6):
                raw = root / f"validator-{index}.json"
                raw.write_text(json.dumps(host(index)))
                evidence.append({
                    "validator_id": f"validator-{index}",
                    "path": str(raw),
                    "sha256": MODULE.sha256_file(raw),
                })
            prior = root / "prior.json"
            prior.write_text(json.dumps({
                "height": 310,
                "tip": "tip",
                "state_root": "root",
                "source_rpc": {"raw_response_paths": evidence},
            }))
            parent, retained = MODULE.prior_parent_evidence(prior, 6)
        self.assertEqual((310, "tip", "root"), parent)
        self.assertEqual(evidence, retained)

    def test_reused_import_rejects_modified_parent_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            raw = root / "validator-0.json"
            raw.write_text(json.dumps(host(0)))
            prior = root / "prior.json"
            prior.write_text(json.dumps({
                "height": 310,
                "tip": "tip",
                "state_root": "root",
                "source_rpc": {"raw_response_paths": [{
                    "validator_id": "validator-0", "path": str(raw), "sha256": "0" * 64,
                }]},
            }))
            with self.assertRaises(MODULE.CheckerError):
                MODULE.prior_parent_evidence(prior, 6)

    def test_opening_bracket_requires_exact_signed_residual_delta(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            opening = Path(directory) / "opening.json"
            opening.write_text(json.dumps({
                "status": "violated", "height": 310, "residual_atoms": "5000010",
            }))
            identity = {"residual_atoms": "5000010"}
            MODULE.attach_opening_bracket(identity, opening)
        self.assertEqual("0", identity["opening_bracket"]["residual_delta_from_h310"])
        self.assertTrue(identity["opening_bracket"]["residual_delta_zero"])

    def test_ethereum_sepolia_is_not_a_legacy_domain(self) -> None:
        report = audit()
        report["routes"] = [route(11_155_111)]
        with self.assertRaises(MODULE.CheckerError):
            MODULE.audit_identity(
                report, ASSET, (310, "tip", "root"), "https://source", [],
                "deprecated-Arbitrum-legacy", "audit-hash",
            )


if __name__ == "__main__":
    unittest.main()
