import contextlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import exit_govern as g


class GovernanceTest(unittest.TestCase):
    def setUp(self):
        self.head = dict(block_height=1761, block_tip_hash="tip", state_root="root",
                         build_git_revision="reviewed")
        self.route = dict(active=True, burns_open=False, current_height=1761,
                          profile_hash=g.EXIT, profile=g.read(g.PROFILE))
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.mock(g, "fleet", return_value=self.head)
        self.mock(g, "rpc", return_value=self.route)
        self.sign = self.mock(g, "govern", return_value={})
        self.mock(g, "save")

    def mock(self, module, name, **kwargs):
        return self.stack.enter_context(patch.object(module, name, **kwargs))

    def command(self, action):
        args = ["gov", action, "--revision", "reviewed", "--release-height", "1760",
                "--output", "unused"]
        if action == "paid":
            args.extend(["--paid", str(g.PROFILE)])
        with patch.object(sys, "argv", args):
            g.main()

    def test_open_requires_clearance_for_candidate_parent(self):
        import requests
        self.mock(g, "ethereum")
        self.mock(g, "require_open_accounting")
        self.mock(requests, "get").return_value.json.return_value = dict(
            payout_ready=True, burns_open=False, bucket_id=g.BUCKET,
            qualified_parent=g.parent_of(self.head))
        clearance = self.mock(g, "rotation_clearance", side_effect=ValueError("stale clearance"))
        with self.assertRaisesRegex(ValueError, "stale clearance"):
            self.command("open")
        clearance.assert_called_once_with(self.head)
        self.sign.assert_not_called()

    def test_close_and_paid_use_solana_parent_after_ethereum_changes(self):
        import exit_canary
        self.mock(g, "ethereum", side_effect=ValueError("new Ethereum epoch"))
        self.mock(exit_canary, "review_paid", return_value={"packet": "p"})
        for action in ("close", "paid"):
            self.command(action)
            self.assertEqual(self.sign.call_args.kwargs["qualified_parent"], g.parent_of(self.head))
            self.route["profile_hash"] = "other"
            with self.assertRaises(ValueError):
                self.command(action)
            self.route["profile_hash"] = g.EXIT

    def test_clearance_reader_checks_open_inode_before_read(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "clearance"
            path.write_text("{}")
            def read():
                return subprocess.run([sys.executable, "-I", "-c", g.CLEARANCE_READER, path],
                                      capture_output=True)
            path.chmod(0o666)
            self.assertNotEqual(read().returncode, 0)
            path.chmod(0o600)
            # Exercise the root-owner rule on ordinary developer logins too.
            self.assertEqual(read().returncode == 0, os.getuid() == 0)
            path.rename(path.with_suffix(".owned"))
            path.symlink_to(path.with_suffix(".owned"))
            self.assertNotEqual(read().returncode, 0)
