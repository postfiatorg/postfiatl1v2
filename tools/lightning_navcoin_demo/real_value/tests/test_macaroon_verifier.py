from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from ..macaroon_verifier import (
    MacaroonVerificationError,
    RECEIVE_ONLY_PERMISSIONS,
    verification_evidence,
    verify_printmacaroon_report,
)


class MacaroonVerifierTests(unittest.TestCase):
    def report(self) -> dict[str, object]:
        return {
            "version": 2,
            "location": "lnd",
            "root_key_id": "0",
            "permissions": [
                {"entity": "info", "action": "read"},
                {"entity": "offchain", "action": "read"},
                {"entity": "invoices", "action": "read"},
                {"entity": "invoices", "action": "write"},
            ],
            "caveats": None,
        }

    def test_exact_receive_only_permissions_and_no_caveats_pass(self) -> None:
        normalized = verify_printmacaroon_report(self.report())
        self.assertEqual(set(normalized), RECEIVE_ONLY_PERMISSIONS)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receive-only.macaroon"
            path.write_bytes(b"opaque macaroon bytes")
            path.chmod(0o600)
            evidence = verification_evidence(self.report(), path)
            self.assertTrue(evidence["ok"])
            self.assertEqual(
                evidence["profile"], "LIGHTNING_NAVCOIN_RECEIVE_ONLY_V1"
            )
            self.assertEqual(evidence["caveats"], [])

    def test_string_permission_encoding_is_supported_exactly(self) -> None:
        report = self.report()
        report["permissions"] = [
            "info:read",
            "offchain:read",
            "invoices:read",
            "invoices:write",
        ]
        self.assertEqual(
            set(verify_printmacaroon_report(report)),
            RECEIVE_ONLY_PERMISSIONS,
        )

    def test_extra_missing_duplicate_and_caveated_authority_fail(self) -> None:
        mutations = []
        extra = self.report()
        extra["permissions"] = [
            *extra["permissions"],
            {"entity": "offchain", "action": "write"},
        ]
        mutations.append(extra)
        missing = self.report()
        missing["permissions"] = missing["permissions"][:-1]
        mutations.append(missing)
        duplicate = self.report()
        duplicate["permissions"] = [
            *duplicate["permissions"][:-1],
            {"entity": "invoices", "action": "read"},
        ]
        mutations.append(duplicate)
        caveated = self.report()
        caveated["caveats"] = ["time-before 2000000000"]
        mutations.append(caveated)
        for report in mutations:
            with self.subTest(report=report), self.assertRaises(
                MacaroonVerificationError
            ):
                verify_printmacaroon_report(report)

    def test_macaroon_file_must_be_private(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receive-only.macaroon"
            path.write_bytes(b"opaque macaroon bytes")
            path.chmod(0o640)
            with self.assertRaisesRegex(
                MacaroonVerificationError, "mode 0600"
            ):
                verification_evidence(self.report(), path)

    def test_mainnet_serve_launcher_has_mandatory_verification_gate(self) -> None:
        repo_root = Path(__file__).resolve().parents[4]
        launcher = (
            repo_root / "scripts/lightning-navcoin-mainnet-coordinator"
        ).read_text(encoding="utf-8")
        gate = launcher.index('"$ENVCTL" verify-macaroon')
        serve = launcher.index('if [[ "$1" == "serve" ]]')
        process_exec = launcher.index('exec "$VENV_DIR/bin/python"')
        self.assertLess(serve, gate)
        self.assertLess(gate, process_exec)


if __name__ == "__main__":
    unittest.main()
