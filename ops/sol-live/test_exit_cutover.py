import io
import os
from pathlib import Path
import tarfile
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


class CutoverTest(unittest.TestCase):
    def test_runtime_uses_public_umask_and_checks_imports_before_key_move(self):
        script = HERE / "install-exit-runtime.sh"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for folder in ("bin", "historical"):
                (root / folder).mkdir()
            for filename in ("bin/proof-host", "historical/egress.elf", "witness"):
                (root / filename).touch()
            # Use a real venv, omitting dependency downloads in the focused test.
            shim = root / "bin/python3"
            shim.write_text(
                '#!/bin/bash\n/usr/bin/python3 -m venv --without-pip "$3" && ln -s /bin/true "$3/bin/pip"\n'
            )
            shim.chmod(0o755)
            archive = io.BytesIO()
            with tarfile.open(fileobj=archive, mode="w"):
                pass
            app = root / "app"
            subprocess.run(
                [
                    "bash",
                    "-ec",
                    'umask 077; bash "$@"',
                    "_",
                    script,
                    app,
                    root,
                    root / "witness",
                ],
                input=archive.getvalue(),
                env=dict(os.environ, PATH=str(root / "bin") + ":" + os.environ["PATH"]),
                check=True,
            )
            self.assertEqual((app / "venv/bin").stat().st_mode & 0o777, 0o755)
        doc = (HERE / "RUNBOOK-LIVE.md").read_text()
        self.assertLess(doc.index("import exit_service"), doc.index('mv -T "$fee"'))
        self.assertIn("wait-closed", doc)

    def test_enabled_relay_outage_cannot_pass_disabled_check(self):
        from exit_cutover import relay_flag

        with self.assertRaises(ValueError):
            relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=true"])
        with self.assertRaises(ValueError):
            relay_flag([])
        relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=false"])

    def test_relay_check_reads_the_main_process_startup_flag(self):
        from exit_cutover import relay
        import sys

        for flag in ("true", "false"):
            with subprocess.Popen(
                [
                    sys.executable,
                    "-c",
                    "import time; time.sleep(15)",
                    "server/l1/relay.js",
                ],
                env=dict(os.environ, L1_SOLANA_WITHDRAWALS_ENABLED=flag),
            ) as child:
                try:
                    with patch(
                        "exit_cutover.subprocess.check_output",
                        return_value=str(child.pid),
                    ):
                        if flag == "true":
                            with self.assertRaisesRegex(
                                ValueError, "explicitly disable"
                            ):
                                relay("navcoin-relay.service", user=True)
                        else:
                            relay("navcoin-relay.service", user=True)
                finally:
                    child.terminate()

    def test_enabled_relay_requires_the_reviewed_entry_and_service_url(self):
        from exit_cutover import relay_flag, relay_entry
        relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=true",
                    b"L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089"], enabled=True)
        with self.assertRaises(ValueError):
            relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=true"], enabled=True)
        root = Path("/tmp/reviewed")
        relay_entry(root, ["node", "server/l1/relay.js"], root)
        with self.assertRaises(ValueError):
            relay_entry(root, ["node", "/other/server/l1/relay.js"], root)
