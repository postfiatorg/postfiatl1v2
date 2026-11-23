from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
DOC = (HERE / "RUNBOOK-LIVE.md").read_text()


class OperatorReviewTest(unittest.TestCase):
    def test_staff_and_rehearse_six_signer_recovery_before_open(self):
        pre = DOC.split("exit_govern.py open")[0]
        for text in ("settlement on-call", "restore rehearsal", "each of the six"):
            self.assertIn(text, pre.lower().split("## 7.")[1][:12000])
        recovery = DOC.split("## Recovery")[1].lower()
        for text in ("disable public admission immediately", "direct burns remain possible",
                     "sixth signer", "pending claims"):
            self.assertIn(text, recovery)

    def test_reviewed_relay_and_gateway_precede_tag3_and_match_paid_job(self):
        before, after = DOC.split('exit_setup.py "${setup_args[@]}" --execute')
        for text in ("wallet/solana-withdraw", "56dbebb4672172c9f336dc95b831deb8281e3fd7",
                     "relay-disabled", "withdrawal-routes.test.mjs", "RELAY_COOKIE_FILE"):
            self.assertTrue(text in before, text)
        for text in ("relay-enabled", ".burn_tx_id==$burn", ".solana_tx==$sig"):
            self.assertTrue(text in after.split("## 10.")[1], text)

    def test_port_and_restart_branch_precede_move(self):
        before = DOC.split('sudo mv -T "$fee"')[0]
        self.assertIn("sport = :18089", before)
        self.assertIn('if sudo test -f "$svc/fee-payer.json"', before)

    def test_default_user_unit_runs_same_sandbox_for_preflight_and_service(self):
        unit = (HERE / "pft-sol-mainnet-withdraw-user.service").read_text()
        start = [line.split("=",1)[1].rsplit(" ",1) for line in unit.splitlines()
                 if line.startswith(("ExecStartPre=", "ExecStart="))]
        self.assertEqual(len(start), 2)
        self.assertEqual(start[0][0], start[1][0])
        self.assertEqual([row[1] for row in start], ["self-test", "serve"])
        subprocess.run(["bash", "-n", HERE / "exit-sandbox.sh"], check=True)

    def test_sandbox_never_binds_a_directory_as_a_credential(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = root / "ops/sol-live/exit-sandbox.sh"
            script.parent.mkdir(parents=True)
            script.write_bytes((HERE / script.name).read_bytes())
            (root / "runtime-files").touch()
            fee = root / ".config/postfiat/sol-bridge/exit-fee-payer.json"
            fee.parent.mkdir(parents=True)
            fee.touch(mode=0o600)
            state = root / "state"
            state.mkdir(mode=0o700)
            trust = root / "known_hosts"
            trust.touch()
            env = dict(os.environ, HOME=tmp, EXIT_JOB_STATE=str(state),
                       EXIT_PROVER_IDENTITY=str(root), EXIT_PROVER_KNOWN_HOSTS=str(trust))
            result = subprocess.run(["bash", script, "self-test"], env=env, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"Expected a regular prover identity file", result.stderr)

    def test_unknown_port_status_cannot_provision_the_payer(self):
        import os
        import tempfile
        block = DOC.split('fee="$HOME/.config/postfiat/sol-bridge/exit-fee-payer.json"')[1]
        check = block.split('if sudo test -f')[0].split('test -f "$fee"')[0]
        with tempfile.TemporaryDirectory() as tmp:
            ss = Path(tmp) / "ss"
            for body in ("exit 1", "echo LISTEN"):
                ss.write_text("#!/bin/sh\n"+body+"\n")
                ss.chmod(0o755)
                result = subprocess.run(["bash", "-ec", check],
                                        env=dict(os.environ, PATH=tmp+":"+os.environ["PATH"]))
                self.assertNotEqual(result.returncode, 0)
