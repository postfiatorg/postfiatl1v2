from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
BOOK = (HERE / "RUNBOOK-LIVE.md").read_text()


class FifthReview(unittest.TestCase):
    def test_unapproved_private_identity_is_rejected_even_with_approved_sidecar(self):
        with tempfile.TemporaryDirectory() as d:
            key = Path(d) / "prover"
            for path in [key, key.with_name("other")]:
                subprocess.run(
                    ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", path],
                    check=True,
                )
            fingerprint = subprocess.check_output(
                ["ssh-keygen", "-lf", str(key) + ".pub"], text=True
            ).split()[1]
            check = ["bash", HERE / "check-prover-identity.sh", key, fingerprint]
            subprocess.run(check, check=True, capture_output=True)
            key.with_name("other").replace(key)
            self.assertNotEqual(
                subprocess.run(check, capture_output=True).returncode, 0
            )

    def test_system_identity_proves_archive_before_funding_and_tag3(self):
        install = BOOK.index("sudo bash ops/sol-live/install-exit-system.sh")
        test = BOOK.index(
            "sudo -n /usr/bin/systemctl start pft-sol-exit-self-test.service", install
        )
        fund = BOOK.index("exit_fund.py --execute")
        init = BOOK.index('exit_setup.py "${setup_args[@]}" --execute')
        self.assertTrue(install < test < fund < init)
        unit = (HERE / "pft-sol-exit-self-test.service").read_text()
        self.assertIn("User=pf-sol-exit", unit)
        self.assertIn("system-self-test", unit)
        script = (HERE / "install-exit-system.sh").read_text()
        self.assertNotRegex(
            script, r"(?m)^\s*(sudo )?systemctl (enable|start|restart) "
        )

    def test_funding_and_tag3_require_verified_role_proof_after_key_move(self):
        from solders.keypair import Keypair
        import exit_setup as s

        fee, pause = Keypair(), Keypair()
        with patch.object(s, "FEE_PAYER", fee.pubkey()), patch.object(
            s, "PAUSE", pause.pubkey()
        ):
            cfg = dict(s.read(s.CONFIG), pause_authority=bytes(pause.pubkey()).hex())
            with patch.object(s, "load", side_effect=[fee, pause]):
                record = s.prove_roles(cfg)
            with patch.object(s, "load", side_effect=AssertionError):
                s.verify_role_record(cfg, record)
                record["nonce"] = "00" * 32
                with self.assertRaises(ValueError):
                    s.verify_role_record(cfg, record)
        for script in ("exit_setup.py", "exit_fund.py"):
            self.assertIn("require_installed_self_test(", (HERE / script).read_text())

    def test_tag8_setup_rehearsal_works_without_worker_payer(self):
        import exit_tag8 as t

        with patch.object(t, "ExitCluster") as cluster, patch.object(
            t, "check_payout_authorities"
        ):
            t.setup_cluster("stage")
            cluster.assert_called_once_with(t.MAINNET_PAYER)
            cluster.return_value.unlock.assert_called_once_with(
                t.DIRECTORY / "mainnet-payer.json"
            )
            cluster.reset_mock()
            t.setup_cluster("simulate")
            cluster.return_value.unlock.assert_not_called()

    def test_prover_login_accepts_only_the_fixed_proof_commands(self):
        script = HERE / "exit-prover-command.sh"
        result = subprocess.run(
            ["bash", script],
            env=dict(SSH_ORIGINAL_COMMAND="uname -a"),
            capture_output=True,
        )
        self.assertEqual(result.returncode, 64)
