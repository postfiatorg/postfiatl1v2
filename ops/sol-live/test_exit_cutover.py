from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

HERE=Path(__file__).resolve().parent


class CutoverTest(unittest.TestCase):
    def test_runtime_uses_public_umask_and_checks_imports_before_key_move(self):
        script=HERE/"install-exit-runtime.sh"
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run(["bash","-c",'umask 077; bash "$1" --probe "$2"', "_",script,tmp],check=True)
            self.assertEqual((Path(tmp)/"bin").stat().st_mode & 0o777,0o755)
        doc=(HERE/"RUNBOOK-LIVE.md").read_text()
        self.assertLess(doc.index("import exit_service"), doc.index('mv -T "$fee"'))
        self.assertIn("wait-closed",doc)

    def test_enabled_relay_outage_cannot_pass_disabled_check(self):
        from exit_cutover import relay_flag
        with self.assertRaises(ValueError):
            relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=true"])
        with self.assertRaises(ValueError):
            relay_flag([])
        relay_flag([b"L1_SOLANA_WITHDRAWALS_ENABLED=false"])
