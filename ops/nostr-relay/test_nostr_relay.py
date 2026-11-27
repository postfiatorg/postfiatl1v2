from __future__ import annotations

import asyncio
import importlib.util
import json
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path


RELAY_DIR = Path(__file__).resolve().parent


def _load_module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, RELAY_DIR / filename)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


wp = _load_module("postfiat_relay_write_policy", "write-policy.py")
vr = _load_module("postfiat_relay_verify", "verify_relay.py")


class NostrRelayPackagingTests(unittest.TestCase):
    def test_deployment_configs_and_invariants(self) -> None:
        compose = (RELAY_DIR / "docker-compose.yml").read_text(encoding="utf-8")
        caddy = (RELAY_DIR / "Caddyfile").read_text(encoding="utf-8")
        strfry = (RELAY_DIR / "strfry.conf").read_text(encoding="utf-8")
        systemd = (RELAY_DIR / "postfiat-nostr-relay.service").read_text(encoding="utf-8")
        mirror = json.loads((RELAY_DIR / "mirror-sync.json").read_text(encoding="utf-8"))

        self.assertIn("relay.postfiat.org", compose)
        self.assertIn("write-policy.py", compose)
        self.assertIn("/healthz", caddy)
        self.assertIn("Strict-Transport-Security", caddy)
        self.assertIn("auth_required = true", strfry)
        self.assertIn("restricted_writes = true", strfry)
        self.assertIn('plugin = "/etc/strfry/write-policy.py"', strfry)
        self.assertIn("ProtectSystem=strict", systemd)
        self.assertIn("NoNewPrivileges=yes", systemd)
        self.assertEqual(mirror["primary_relay"], "wss://relay.postfiat.org")
        self.assertTrue(mirror["inbound_filter"]["registered_members_only"])
        self.assertTrue(mirror["outbound_policy"]["block_kind0_in_non_production"])
        self.assertEqual(mirror["inbound_filter"]["allowed_kinds"], [0, 1, 13, 1059, 10050])

    def test_write_policy_nonprod_kind0_and_nip17_and_subprocess_jsonl(self) -> None:
        member = "c3" * 32
        outsider = "d4" * 32
        with tempfile.TemporaryDirectory(prefix="postfiat-wp-test-") as tmpdir:
            allowlist = Path(tmpdir) / "allowlist.json"
            allowlist.write_text(json.dumps({"registered_pubkeys": [member]}), encoding="utf-8")

            # Non-production environment blocks kind:0 profile publishing
            nonprod = wp.WritePolicyEngine(
                wp.RelayPolicyConfig(env="staging", allowlist_path=allowlist, require_nip42_auth=True)
            )
            ev0 = vr.make_synthetic_nostr_event(pubkey=member, kind=0, content='{"name":"test"}')
            res0 = nonprod.evaluate({"type": "new", "event": ev0, "sourceType": "IP4", "authedPubkey": member})
            self.assertEqual(res0["action"], "reject")
            self.assertIn("non-production", res0["msg"])

            # Production allows NIP-17 kind:1059 gift wrap addressed to a registered member with randomized past timestamp
            prod = wp.WritePolicyEngine(
                wp.RelayPolicyConfig(env="production", allowlist_path=allowlist, require_nip42_auth=True)
            )
            gift = vr.make_synthetic_nostr_event(
                pubkey=outsider,
                kind=1059,
                content="encrypted-nip17-wrap",
                tags=[["p", member]],
                created_at=int(time.time()) - 3600 * 24,  # NIP-17 randomized past timestamp
            )
            res_gift_mirror = prod.evaluate({"type": "new", "event": gift, "sourceType": "Stream"})
            self.assertEqual(res_gift_mirror["action"], "accept")

            # Subprocess JSONL strfry plugin protocol check
            ev1 = vr.make_synthetic_nostr_event(pubkey=member, kind=1, content="hello hive")
            ev_bad = vr.make_synthetic_nostr_event(pubkey=member, kind=24133, content="nip46 blocked")
            stdin_lines = (
                json.dumps({"type": "new", "event": ev1, "sourceType": "IP4", "authedPubkey": member}) + "\n"
                + json.dumps({"type": "new", "event": ev_bad, "sourceType": "IP4", "authedPubkey": member}) + "\n"
            )
            proc = subprocess.run(
                [sys.executable, str(RELAY_DIR / "write-policy.py")],
                input=stdin_lines,
                text=True,
                capture_output=True,
                check=True,
                env={
                    **{k: v for k, v in __import__("os").environ.items()},
                    "NOSTR_RELAY_ENV": "production",
                    "NOSTR_RELAY_ALLOWLIST_PATH": str(allowlist),
                },
            )
            lines = [json.loads(l) for l in proc.stdout.strip().splitlines()]
            self.assertEqual(len(lines), 2)
            self.assertEqual(lines[0]["action"], "accept")
            self.assertEqual(lines[1]["action"], "reject")

    def test_verify_relay_self_test_harness(self) -> None:
        result = asyncio.run(vr.run_self_test())
        self.assertTrue(result["ok"])
        self.assertTrue(result["nip42_auth_ok"])
        self.assertTrue(result["disallowed_kind_blocked"])
        self.assertTrue(result["per_pubkey_rate_limit_enforced"])
        self.assertTrue(result["inbound_mirror_spam_filtered"])


if __name__ == "__main__":
    unittest.main()
