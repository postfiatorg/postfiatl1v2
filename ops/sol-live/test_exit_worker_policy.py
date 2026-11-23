import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import subprocess

HERE = Path(__file__).parent


class WorkerPolicyTest(unittest.TestCase):
    def test_root_preflight_rejects_each_host_privilege(self):
        import exit_worker_policy as policy

        worker = SimpleNamespace(pw_uid=501, pw_gid=502)
        admin = SimpleNamespace(pw_dir="/home/admin")
        def lookup(name):
            return {"postfiat": admin, "pf-sol-exit": worker}[name]

        for hazard in ("none", "docker group", "socket", "sudo", "sudo error", "read home", "traverse home"):
            with self.subTest(hazard=hazard):
                def run(args, **kw):
                    if args[0] == "/usr/bin/sudo":
                        if hazard == "sudo":
                            return subprocess.CompletedProcess(args, 0, "permitted", "")
                        message = ("configuration error" if hazard == "sudo error" else
                                   "User pf-sol-exit is not allowed to run sudo on fixture.")
                        return subprocess.CompletedProcess(args, 1, message, "")
                    self.assertEqual(args[:2], ["/usr/bin/test", args[1]])
                    self.assertEqual((kw["user"], kw["group"]), (501, 502))
                    present = {("-w", "/run/docker.sock"): "socket",
                               ("-r", "/home/admin"): "read home",
                               ("-x", "/home/admin"): "traverse home"}
                    return subprocess.CompletedProcess(args, int(present[tuple(args[1:])] != hazard))
                with (patch.object(os, "geteuid", return_value=0),
                      patch.object(policy.pwd, "getpwnam", side_effect=lookup),
                      patch.object(policy.grp, "getgrnam", return_value=SimpleNamespace(gr_gid=503)),
                      patch.object(os, "getgrouplist", return_value=[502, 503] if hazard == "docker group" else [502]),
                      patch.object(policy.subprocess, "run", side_effect=run)):
                    if hazard == "none":
                        policy.check()
                    else:
                        with self.assertRaises(RuntimeError):
                            policy.check()

    def test_host_policy_precedes_sandboxed_proof(self):
        unit = (HERE / "pft-sol-exit-self-test.service").read_text()
        self.assertLess(unit.index("ExecStartPre=+/usr/bin/python3 -I "), unit.index("ExecStart="))
        self.assertIn("exit_worker_policy.py", (HERE / "build-exit-bundle.py").read_text())

    def test_operational_payer_is_absent_from_reviewed_tag3(self):
        import exit_setup as s
        cfg = s.read(s.CONFIG)
        ix = s.build_initialize(cfg, s.MAINNET_PAYER)
        s.check_reviewed_instruction(cfg, ix)
        self.assertNotIn(s.FEE_PAYER, [a.pubkey for a in ix.accounts])
        self.assertNotIn(bytes(s.FEE_PAYER), ix.data)
        self.assertEqual(bytes.fromhex(cfg["pause_authority"]), bytes(s.PAUSE))
        doc = (HERE / "RUNBOOK-LIVE.md").read_text()
        for text in ("trusted administrator", "100,000,000 lamports", "pfusdc-egress-verifier/src/vault.rs:"):
            self.assertIn(text, doc)
