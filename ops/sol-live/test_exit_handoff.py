"""Check the operator handoff against the runtime contract and the predecessor kit."""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import exit_canary
import exit_release_kit as release
from exit_service import BASE
from exit_setup import ASSET, BUCKET, EXIT, PROFILE, SERIES

HERE = Path(__file__).parent


class HandoffTest(unittest.TestCase):
    def test_contract_matches_the_wallet_intent(self):
        doc = (HERE / "TASKNODE-CONTRACT.md").read_text()
        intent = json.loads(
            doc.split("## Wallet signature")[1].split("```json\n")[1].split("\n```")[0]
        )
        with (
            patch.object(exit_canary, "cluster"),
            patch.object(exit_canary, "balances", return_value={"pfusdc_atoms": 20000}),
            patch.object(
                exit_canary,
                "fleet",
                return_value={"chain_id": "test", "genesis_hash": "00" * 48},
            ),
            patch.object(exit_canary.requests, "get") as get,
            patch.object(exit_canary, "rpc") as quote,
        ):
            get.return_value.json.return_value = dict(
                ready=True,
                asset_id=ASSET,
                bucket_id=BUCKET,
                series_asset_id=SERIES,
                issuer="issuer",
                epoch=123,
                reserve_packet_hash="11" * 48,
            )

            def fee(index, method, source, operation_json):
                self.assertEqual(method, "asset_fee_quote")
                return dict(
                    operation=json.loads(operation_json),
                    source=source,
                    protocol_version=1,
                    minimum_fee=1,
                    sequence=2,
                    chain_id="test",
                    genesis_hash="00" * 48,
                )

            quote.side_effect = fee
            fields = exit_canary.prepare(None)["fields"]
        self.assertEqual(intent["operation"].keys(), fields["operation"].keys())
        self.assertEqual(
            intent["operation"]["operation"], fields["operation"]["operation"]
        )
        self.assertIn(BASE + "/readiness", doc)
        self.assertIn(exit_canary.PREFIX, doc)

    def test_runbook_shell_syntax(self):
        runbook = (HERE / "RUNBOOK-LIVE.md").read_text()
        for script in runbook.split("```bash\n")[1:]:
            subprocess.run(
                ["bash", "-n"], input=script.split("\n```")[0], text=True, check=True
            )
        batch = json.loads(
            (
                HERE.parents[1]
                / "crates/node/tests/fixtures/solana-exit/governance-batch.json"
            ).read_text()
        )
        self.assertIn(EXIT, json.dumps(batch))
        self.assertIn(json.loads(PROFILE.read_text())["route_id"], json.dumps(batch))

    def test_release_kit_keeps_the_predecessor_and_a_fresh_stage(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            old = base / release.OLD
            (old / "evidence").mkdir(parents=True)
            (old / "evidence/source-revision").write_text("aabbccdd1234\n")
            (old / "env").write_text(
                f"export REL={release.OLD}\nexport OLD=sol-release6-4-20261119\n"
            )
            for name in (
                "stage.sh",
                "roll-rest.sh",
                "deploy_host.sh",
                "restore-watchdog.sh",
            ):
                (old / name).write_text(f"source {old}/env\n")
            binary = base / "binary"
            binary.write_bytes(b"rehearsal")
            with (
                patch.object(release, "KIT", old),
                patch.object(
                    release, "fleet", return_value={"build_git_revision": "aabbccdd"}
                ),
                patch.object(release, "six", return_value=[{}] * 6),
                patch(
                    "sys.argv",
                    ["kit", "--release", "sol-exit-test", "--binary", str(binary)],
                ),
            ):
                release.main()
            dest = base / "sol-exit-test"
            self.assertIn("export OLD=" + release.OLD, (dest / "env").read_text())
            self.assertEqual((dest / "stage.sh").read_text(), f"source {dest}/env\n")
            self.assertFalse((dest / "stage").exists())
