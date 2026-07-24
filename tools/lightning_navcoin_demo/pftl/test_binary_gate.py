from __future__ import annotations

import hashlib
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from tools.lightning_navcoin_demo.pftl import binary_gate


REQUIRED_HELP = " ".join(
    (
        "transport-peer-certified-mempool-round",
        "transport-validator-serve",
        "rpc-serve",
        "escrow_info",
        "asset_info",
    )
)
REVISION = "ab" * 20


class BinaryProvenanceTests(unittest.TestCase):
    def _pair(self, root: Path) -> tuple[Path, str, str]:
        node = root / "postfiat-node"
        sdk = root / "postfiat-rpc-sdk"
        node.write_bytes(b"synthetic hardened node")
        sdk.write_bytes(b"synthetic hardened sdk")
        node.chmod(0o700)
        sdk.chmod(0o700)
        return (
            node,
            hashlib.sha256(node.read_bytes()).hexdigest(),
            hashlib.sha256(sdk.read_bytes()).hexdigest(),
        )

    def test_exact_node_and_sdk_hashes_are_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            node, node_sha, sdk_sha = self._pair(Path(directory))
            status = {
                "build_git_revision": REVISION,
                "build_profile": "release",
                "protocol_version": 1,
            }
            help_result = SimpleNamespace(
                returncode=0,
                stdout=REQUIRED_HELP,
            )
            with (
                mock.patch.object(binary_gate, "_json_command", return_value=status),
                mock.patch.object(
                    binary_gate.subprocess,
                    "run",
                    return_value=help_result,
                ),
            ):
                report = binary_gate.verify_binary(
                    node,
                    expected_revision=REVISION,
                    expected_binary_sha256=node_sha,
                    expected_wallet_sdk_sha256=sdk_sha,
                    run_semantic_probe=False,
                )
                self.assertEqual(report["binary_sha256"], node_sha)
                self.assertEqual(report["wallet_sdk_sha256"], sdk_sha)
                with self.assertRaisesRegex(
                    binary_gate.BinaryGateError,
                    "node binary SHA-256 mismatch",
                ):
                    binary_gate.verify_binary(
                        node,
                        expected_revision=REVISION,
                        expected_binary_sha256="00" * 32,
                        expected_wallet_sdk_sha256=sdk_sha,
                        run_semantic_probe=False,
                    )
                with self.assertRaisesRegex(
                    binary_gate.BinaryGateError,
                    "expected wallet SDK SHA-256 is not canonical",
                ):
                    binary_gate.verify_binary(
                        node,
                        expected_revision=REVISION,
                        expected_binary_sha256=node_sha,
                        expected_wallet_sdk_sha256="not-a-digest",
                        run_semantic_probe=False,
                    )


if __name__ == "__main__":
    unittest.main()
