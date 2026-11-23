import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
BOOK = (HERE / "RUNBOOK-LIVE.md").read_text()


class FifthReview(unittest.TestCase):
    def test_browser_post_rechecks_paid_id_signature_and_exact_200(self):
        from exit_relay import verify_paid

        cfg = dict(gateway="https://wallet.example", cookie_file="/cookie")
        burn, sig = "ab" * 48, "recorded"
        paid = dict(stage="paid", burn_tx_id=burn, solana_tx=sig)

        def run(post_response):
            with patch(
                "exit_relay.http",
                side_effect=[
                    (200, dict(ok=True, ready=True)),
                    (200, paid),
                    post_response,
                ],
            ) as http:
                verify_paid(cfg, burn, sig)
                self.assertEqual(http.call_args.kwargs["body"], {"burn_tx_id": burn})

        run((200, paid))
        for response in [
            (403, {}),
            (202, paid),
            (200, dict(paid, solana_tx="different")),
        ]:
            with self.assertRaises(ValueError):
                run(response)

    def test_gateway_only_503_fails_disabled_probe(self):
        from exit_relay import verify_disabled

        cfg = dict(
            gateway="https://wallet.example",
            cookie_file="/cookie",
            unit="relay.service",
            build="approved",
        )
        responses = [
            (200, dict(ok=True, buildId="approved")),
            (
                200,
                dict(
                    ok=True,
                    ready=False,
                    message="Solana withdrawals are not available yet",
                ),
            ),
            (503, dict(ok=False, code="solana_withdrawals_unavailable")),
        ]
        with patch("exit_relay.http", side_effect=responses), patch(
            "exit_relay.relay_request_seen", return_value=False
        ):
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
            config.write_text(
                json.dumps(dict(unit="fixture.service", flag_file=str(flag)))
            )
            # Stop after recording the requested restart. There is no system manager in this drill.
            systemctl = home / "systemctl"
            systemctl.write_text(
                '#!/bin/sh\nprintf "%s\\n" "$@" > "$HOME/systemctl.args"\nexit 1\n'
            )
            systemctl.chmod(0o755)
            result = subprocess.run(
                [
                    "env",
                    "-i",
                    "HOME=" + d,
                    "PATH=" + d + ":/usr/bin:/bin",
                    "/bin/bash",
                    "--login",
                    "-c",
                    'export PATH="$1:/usr/bin:/bin"; exec "$2"',
                    "_",
                    d,
                    command,
                ],
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("L1_SOLANA_WITHDRAWALS_ENABLED=false", flag.read_text())
            self.assertEqual(
                (home / "systemctl.args").read_text().splitlines(),
                ["--user", "restart", "fixture.service"],
            )
        self.assertIn("bash -lc", BOOK)
        self.assertIn("disable-sol-exit", BOOK)

    def test_relay_rejects_unsupported_node_before_install_or_restart(self):
        from exit_relay import require_supported_node

        for version in ("v18.20.8", "v24.20.0", "v25.0.0"):
            with patch(
                "exit_relay.subprocess.check_output", return_value=version
            ), self.assertRaises(ValueError):
                require_supported_node("/fixture/node")
        with patch("exit_relay.subprocess.check_output", return_value="v24.22.1\n"):
            require_supported_node("/fixture/node")
        self.assertIn('"$NODE" "$node_bin/npm" ci && "$NODE" --test', BOOK)
        self.assertIn('ExecStart=\\"$NODE\\" --require', BOOK)

    def test_paid_repost_uses_the_browser_origin_and_original_json_body(self):
        from http.server import BaseHTTPRequestHandler, HTTPServer
        import threading
        from exit_relay import verify_paid, ROUTE

        burn, sig = "ab" * 48, "original"
        paid = dict(stage="paid", burn_tx_id=burn, solana_tx=sig)
        seen = []

        class Handler(BaseHTTPRequestHandler):
            def reply(self, result):
                self.send_response(200)
                self.end_headers()
                self.wfile.write(json.dumps(result).encode())

            def do_GET(self):
                self.reply(
                    dict(ok=True, ready=True)
                    if self.path.endswith("/readiness")
                    else paid
                )

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                seen.append(
                    (
                        self.path,
                        self.headers["Origin"],
                        self.headers["Sec-Fetch-Site"],
                        body,
                    )
                )
                self.reply(paid)

            def log_message(self, *args):
                pass

        with HTTPServer(
            ("127.0.0.1", 0), Handler
        ) as server, tempfile.TemporaryDirectory() as tmp:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                origin = f"http://127.0.0.1:{server.server_port}"
                cookie = Path(tmp) / "cookie"
                cookie.touch()
                verify_paid(dict(gateway=origin, cookie_file=str(cookie)), burn, sig)
                self.assertEqual(
                    seen, [(ROUTE, origin, "same-origin", {"burn_tx_id": burn})]
                )
            finally:
                server.shutdown()
                thread.join()
