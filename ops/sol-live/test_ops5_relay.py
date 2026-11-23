import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

HERE = Path(__file__).resolve().parent
BOOK = (HERE / "RUNBOOK-LIVE.md").read_text()


class FifthReview(unittest.TestCase):
    def test_browser_post_rechecks_paid_id_signature_and_exact_200(self):
        from exit_relay import verify_paid
        cfg = dict(gateway="https://wallet.example", cookie_file="/cookie")
        burn, sig = "ab"*48, "recorded"
        paid = dict(stage="paid", burn_tx_id=burn, solana_tx=sig)
        def run(post_response):
            with patch("exit_relay.http", side_effect=[(200, dict(ok=True, ready=True)),
                                                      (200, paid), post_response]) as http:
                verify_paid(cfg, burn, sig)
                self.assertEqual(http.call_args.kwargs["body"], {"burn_tx_id": burn})
        run((200, paid))
        for response in [(403, {}), (202, paid), (200, dict(paid, solana_tx="different"))]:
            with self.assertRaises(ValueError):
                run(response)

    def test_gateway_only_503_fails_disabled_probe(self):
        from exit_relay import verify_disabled
        cfg = dict(gateway="https://wallet.example", cookie_file="/cookie", unit="relay.service",
                   build="approved")
        responses = [(200, dict(ok=True, buildId="approved")),
                     (200, dict(ok=True, ready=False, message="Solana withdrawals are not available yet")),
                     (503, dict(ok=False, code="solana_withdrawals_unavailable"))]
        with patch("exit_relay.http", side_effect=responses), patch("exit_relay.relay_request_seen", return_value=False):
            with self.assertRaisesRegex(ValueError, "relay"):
                verify_disabled(cfg)

    def test_disable_available_in_fresh_login_without_release_variables(self):
        command = HERE / "exit_relay.py"
        with tempfile.TemporaryDirectory() as d:
            home = Path(d)
            config = home / ".config/postfiat/sol-exit-relay.json"
            config.parent.mkdir(parents=True)
            flag = home / "release.env"
            flag.write_text("L1_SOLANA_WITHDRAWALS_ENABLED=true\n")
            config.write_text(json.dumps(dict(unit="fixture.service", flag_file=str(flag))))
            # Stop after recording the requested restart. There is no system manager in this drill.
            systemctl = home / "systemctl"
            systemctl.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$HOME/systemctl.args"\nexit 1\n')
            systemctl.chmod(0o755)
            result = subprocess.run(["env", "-i", "HOME="+d, "PATH="+d+":/usr/bin:/bin", "/bin/bash",
                                     "--noprofile", "--norc", "-c", '"$1"', "_", command],
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("L1_SOLANA_WITHDRAWALS_ENABLED=false", flag.read_text())
            self.assertEqual((home / "systemctl.args").read_text().splitlines(), ["--user","restart","fixture.service"])
        self.assertIn('bash -lc', BOOK)
        self.assertIn('disable-sol-exit', BOOK)
