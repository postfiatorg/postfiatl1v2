import contextlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import exit_govern as g
from govern import REVIEW_READER


class GovernanceTest(unittest.TestCase):
    def setUp(self):
        self.head = dict(block_height=1761, block_tip_hash="tip", state_root="root",
                         build_git_revision="reviewed")
        self.route = g.read(Path(__file__).parent / "fixtures/fork-open-route.json")
        self.route.update(burns_open=False, current_height=1761)
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
                return subprocess.run([sys.executable, "-I", "-c", REVIEW_READER, path],
                                      capture_output=True)
            path.chmod(0o666)
            self.assertNotEqual(read().returncode, 0)
            path.chmod(0o600)
            # Exercise the root-owner rule on ordinary developer logins too.
            self.assertEqual(read().returncode == 0, os.getuid() == 0)
            path.rename(path.with_suffix(".owned"))
            path.symlink_to(path.with_suffix(".owned"))
            self.assertNotEqual(read().returncode, 0)

    def test_paid_requires_six_protected_reviews_before_signing(self):
        import json
        import govern

        names = [f"validator-{i}" for i in range(6)]
        parent = g.parent_of(self.head)
        approval = dict(kind="reviewed-paid", parent=parent)
        amendment = dict(kind=approval["kind"], value=1, quorum=6, validators=names,
                         activation_height=parent["block_height"] + 1)
        with tempfile.TemporaryDirectory() as tmp:
            file = Path(tmp) / "review"
            file.write_text(json.dumps(approval))
            file.chmod(0o666)
            checked = []

            def remote(i, args, data=None):
                if args[0] == "pgrep":
                    return "123"
                if args == ["cat", "/proc/123/cmdline"]:
                    return ("/opt/postfiat/releases/test/postfiat-node\0--data-dir\0"
                            f"/var/lib/postfiat/validator-{i}\0--topology\0/etc/postfiat/releases/test\0")
                if str(args[-1]).endswith("/sol-exit-paid-review.json"):
                    checked.append(i)
                    if i == 5:
                        return subprocess.check_output([*args[:-1], file], text=True, stderr=subprocess.PIPE)
                    return json.dumps(approval)
                if args[0] == "cat" and args[-1].endswith("/amend"):
                    return json.dumps(amendment)
                if "governance-authorization-sign" in args:
                    self.fail("Signing started without six protected reviews")
                return ""

            with (patch.object(govern, "remote", side_effect=remote),
                  patch.object(govern, "fleet", return_value=self.head),
                  patch.object(govern, "STATE", Path(tmp)),
                  patch("exit_canary.match_reference")):
                with self.assertRaises(subprocess.CalledProcessError):
                    govern.govern("exit-paid", compatible="reviewed", qualified_parent=parent,
                                  payout=dict(packet="p", slot=1, ordinal=0, signature_hex="00"))
            self.assertEqual(sorted(checked), list(range(6)))
