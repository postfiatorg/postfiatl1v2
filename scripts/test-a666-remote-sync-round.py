#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts/a666-remote-sync-round.py"
PADDING = ROOT / "scripts/a666-advance-live-proof-height.py"


def load_padding_module():
    spec = importlib.util.spec_from_file_location("a666_height_padding", PADDING)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load height-padding module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class RemoteSyncRoundTests(unittest.TestCase):
    def run_kind(self, kind: str) -> list[str]:
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            fake_node = root / "fake-node.py"
            args_file = root / "args.json"
            fake_node.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "open(os.environ['FAKE_ARGS_FILE'], 'w').write(json.dumps(sys.argv[1:]))\n"
                "print(json.dumps({\n"
                " 'schema': 'fake-round-v1', 'node_id': 'validator-0',\n"
                " 'submitted_tx_id': 'aa', 'round_ok': True,\n"
                " 'round': {\n"
                "  'certification': {'block_height': 1, 'certificate_id': 'bb', 'vote_count': 6},\n"
                "  'all_sends_verified': True, 'local_apply_verified': True\n"
                " }\n"
                "}))\n"
            )
            fake_node.chmod(0o755)
            signed_file = root / "signed.json"
            signed_file.write_text(json.dumps({"signed": True}) + "\n")
            artifact_dir = root / "artifacts"
            artifact_dir.mkdir()
            env = os.environ.copy()
            env["FAKE_ARGS_FILE"] = str(args_file)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--node-bin",
                    str(fake_node),
                    "--data-dir",
                    str(root / "data"),
                    "--topology",
                    str(root / "topology.json"),
                    "--key-file",
                    str(root / "validator-key.json"),
                    "--signed-file",
                    str(signed_file),
                    "--transaction-kind",
                    kind,
                    "--artifact-dir",
                    str(artifact_dir),
                    "--height",
                    "1",
                ],
                check=True,
                text=True,
                capture_output=True,
                env=env,
            )
            self.assertTrue(json.loads(completed.stdout)["round_ok"])
            return json.loads(args_file.read_text())

    def test_asset_uses_inline_signed_asset_json(self) -> None:
        args = self.run_kind("asset")
        flag_index = args.index("--signed-asset-transaction-json")
        self.assertEqual(json.loads(args[flag_index + 1]), {"signed": True})
        self.assertNotIn("--signed-transfer-file", args)

    def test_transfer_uses_signed_transfer_file(self) -> None:
        args = self.run_kind("transfer")
        flag_index = args.index("--signed-transfer-file")
        self.assertTrue(args[flag_index + 1].endswith("/signed.json"))
        self.assertNotIn("--signed-asset-transaction-json", args)


class ProofHeightPaddingTests(unittest.TestCase):
    def test_round_plan_alternates_value_carrying_transfers(self) -> None:
        padding = load_padding_module()
        plan = padding.build_round_plan(776, 784, 10, "pf-a", "pf-b")
        self.assertEqual(len(plan), 8)
        self.assertEqual(plan[0]["height"], 777)
        self.assertEqual(plan[-1]["height"], 784)
        self.assertEqual(plan[0]["source"], "pf-a")
        self.assertEqual(plan[1]["source"], "pf-b")
        self.assertTrue(all(item["amount"] == 10 for item in plan))

    def test_round_plan_is_noop_at_or_after_target(self) -> None:
        padding = load_padding_module()
        self.assertEqual(padding.build_round_plan(784, 784, 10, "a", "b"), [])
        self.assertEqual(padding.build_round_plan(785, 784, 10, "a", "b"), [])


if __name__ == "__main__":
    unittest.main()
