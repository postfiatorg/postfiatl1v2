#!/usr/bin/env python3
"""Health, NIP-11, NIP-42 AUTH, write-admission, and rate-limit verification harness
for relay.postfiat.org (Pillar 6).

Modes:
  1. Live probe:
       python3 ops/nostr-relay/verify_relay.py \
         --http-url https://relay.postfiat.org \
         --ws-url wss://relay.postfiat.org
  2. Self-test (spins up a local HTTP+WebSocket Nostr relay backed directly by
     ops/nostr-relay/write-policy.py and proves every admission & rate-limit gate):
       python3 ops/nostr-relay/verify_relay.py --self-test
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import importlib.util
import json
import secrets
import sys
import tempfile
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Any

import websockets


HERE = Path(__file__).resolve().parent


def _load_write_policy_module():
    spec = importlib.util.spec_from_file_location("postfiat_nostr_write_policy", HERE / "write-policy.py")
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod


def make_synthetic_nostr_event(
    *,
    pubkey: str,
    kind: int,
    content: str = "",
    tags: list[list[str]] | None = None,
    created_at: int | None = None,
) -> dict[str, Any]:
    ts = int(time.time()) if created_at is None else created_at
    tag_list = tags if tags is not None else []
    serialized = json.dumps([0, pubkey, ts, kind, tag_list, content], separators=(",", ":"), ensure_ascii=False)
    event_id = hashlib.sha256(serialized.encode("utf-8")).hexdigest()
    return {
        "id": event_id,
        "pubkey": pubkey,
        "created_at": ts,
        "kind": kind,
        "tags": tag_list,
        "content": content,
        "sig": "00" * 64,
    }


def probe_http_health_and_nip11(http_url: str, timeout: float = 5.0) -> dict[str, Any]:
    base = http_url.rstrip("/")
    with urllib.request.urlopen(f"{base}/healthz", timeout=timeout) as resp:
        health_status = resp.status
        health_body = json.loads(resp.read().decode("utf-8"))
    if health_status != 200 or not health_body.get("ok"):
        raise AssertionError(f"/healthz failed: status={health_status} body={health_body}")

    req = urllib.request.Request(base + "/", headers={"Accept": "application/nostr+json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        nip11_status = resp.status
        nip11_body = json.loads(resp.read().decode("utf-8"))
    if nip11_status != 200:
        raise AssertionError(f"NIP-11 failed: status={nip11_status}")
    supported = set(nip11_body.get("supported_nips") or [])
    if not {1, 11, 42}.issubset(supported):
        raise AssertionError(f"NIP-11 missing required supported_nips {{1,11,42}}: {supported}")
    limitation = nip11_body.get("limitation") or {}
    if not limitation.get("auth_required") or not limitation.get("restricted_writes"):
        raise AssertionError(f"NIP-11 must advertise auth_required=true and restricted_writes=true: {limitation}")
    return {"healthz": health_body, "nip11": nip11_body}


async def probe_ws_challenge_and_unauthed_reject(ws_url: str) -> dict[str, Any]:
    async with websockets.connect(ws_url, open_timeout=5.0, close_timeout=5.0) as ws:
        # Expect NIP-42 AUTH challenge on connect
        first_msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=5.0))
        if not (isinstance(first_msg, list) and len(first_msg) >= 2 and first_msg[0] == "AUTH" and first_msg[1]):
            raise AssertionError(f"expected NIP-42 AUTH challenge on connect, got {first_msg!r}")

        # Unauthenticated write must be rejected
        unauthed_event = make_synthetic_nostr_event(
            pubkey="ab" * 32,
            kind=1,
            content="unauthenticated probe",
        )
        await ws.send(json.dumps(["EVENT", unauthed_event]))
        ok_msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=5.0))
        if not (isinstance(ok_msg, list) and ok_msg[0] == "OK" and ok_msg[2] is False):
            raise AssertionError(f"unauthenticated EVENT unexpectedly accepted: {ok_msg!r}")
        return {"auth_challenge_received": True, "unauthed_write_rejected": True, "reason": ok_msg[3]}


async def run_self_test() -> dict[str, Any]:
    wp = _load_write_policy_module()
    registered_pubkey = "a1" * 32
    unregistered_pubkey = "b2" * 32

    with tempfile.TemporaryDirectory(prefix="postfiat-relay-selftest-") as tmpdir:
        allowlist_path = Path(tmpdir) / "members-allowlist.json"
        allowlist_path.write_text(
            json.dumps({"schema": "postfiat-nostr-allowlist-v1", "registered_pubkeys": [registered_pubkey]}),
            encoding="utf-8",
        )
        engine = wp.WritePolicyEngine(
            wp.RelayPolicyConfig(
                env="production",
                allowlist_path=allowlist_path,
                require_nip42_auth=True,
                rate_limit_max_events=3,
                rate_limit_window_seconds=60,
                mirror_rate_limit_max_events=5,
            )
        )

        # 1. Start local HTTP server exposing /healthz and NIP-11 metadata
        class _Handler(BaseHTTPRequestHandler):
            def log_message(self, format: str, *args: Any) -> None:
                return

            def do_GET(self) -> None:
                if self.path == "/healthz":
                    payload = json.dumps({"ok": True, "service": "postfiat-nostr-relay", "host": "relay.postfiat.org"}).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(payload)))
                    self.end_headers()
                    self.wfile.write(payload)
                    return
                if "application/nostr+json" in (self.headers.get("Accept") or ""):
                    payload = json.dumps(
                        {
                            "name": "Post Fiat Hive Relay",
                            "supported_nips": [1, 11, 17, 42],
                            "limitation": {
                                "auth_required": True,
                                "restricted_writes": True,
                                "max_content_length": 32768,
                                "max_event_tags": 64,
                            },
                        }
                    ).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/nostr+json")
                    self.send_header("Content-Length", str(len(payload)))
                    self.end_headers()
                    self.wfile.write(payload)
                    return
                self.send_response(404)
                self.end_headers()

        http_server = HTTPServer(("127.0.0.1", 0), _Handler)
        http_port = http_server.server_port
        http_thread = threading.Thread(target=http_server.serve_forever, daemon=True)
        http_thread.start()

        # 2. Start local WebSocket Nostr relay wired directly to write-policy.py
        async def _ws_handler(ws):
            challenge = secrets.token_hex(16)
            authed_pubkey = ""
            await ws.send(json.dumps(["AUTH", challenge]))
            async for raw in ws:
                msg = json.loads(raw)
                if not isinstance(msg, list) or len(msg) < 2:
                    continue
                verb = msg[0]
                if verb == "AUTH":
                    auth_ev = msg[1]
                    tags = auth_ev.get("tags") or []
                    challenge_ok = any(len(t) >= 2 and t[0] == "challenge" and t[1] == challenge for t in tags)
                    if auth_ev.get("kind") == 22242 and challenge_ok:
                        authed_pubkey = str(auth_ev.get("pubkey", "")).lower()
                        await ws.send(json.dumps(["OK", auth_ev["id"], True, ""]))
                    else:
                        await ws.send(json.dumps(["OK", auth_ev.get("id", ""), False, "restricted: invalid AUTH"]))
                elif verb == "EVENT":
                    ev = msg[1]
                    decision = engine.evaluate(
                        {
                            "type": "new",
                            "event": ev,
                            "receivedAt": int(time.time()),
                            "sourceType": "IP4",
                            "authedPubkey": authed_pubkey,
                        }
                    )
                    accepted = decision["action"] == "accept"
                    await ws.send(json.dumps(["OK", ev["id"], accepted, decision["msg"]]))

        ws_server = await websockets.serve(_ws_handler, "127.0.0.1", 0)
        ws_port = ws_server.sockets[0].getsockname()[1]

        try:
            http_res = probe_http_health_and_nip11(f"http://127.0.0.1:{http_port}")
            unauthed_res = await probe_ws_challenge_and_unauthed_reject(f"ws://127.0.0.1:{ws_port}")

            # 3. Authenticated registered-member workflow + kind allowlist + rate limit
            async with websockets.connect(f"ws://127.0.0.1:{ws_port}") as ws:
                auth_frame = json.loads(await ws.recv())
                assert auth_frame[0] == "AUTH"
                challenge = auth_frame[1]

                # Complete NIP-42 AUTH as registered member
                auth_ev = make_synthetic_nostr_event(
                    pubkey=registered_pubkey,
                    kind=22242,
                    tags=[["relay", "wss://relay.postfiat.org"], ["challenge", challenge]],
                )
                await ws.send(json.dumps(["AUTH", auth_ev]))
                auth_ok = json.loads(await ws.recv())
                assert auth_ok[2] is True, f"NIP-42 AUTH failed: {auth_ok}"

                # Disallowed kind (30078) must be rejected before consuming client rate budget
                bad_kind_ev = make_synthetic_nostr_event(pubkey=registered_pubkey, kind=30078, content="blocked kind")
                await ws.send(json.dumps(["EVENT", bad_kind_ev]))
                bad_kind_ok = json.loads(await ws.recv())
                assert bad_kind_ok[2] is False and "not on relay.postfiat.org allowlist" in bad_kind_ok[3]

                # 3 allowed kind:1 posts succeed, 4th triggers per-pubkey rate limit!
                for idx in range(3):
                    ev = make_synthetic_nostr_event(pubkey=registered_pubkey, kind=1, content=f"hive post {idx}")
                    await ws.send(json.dumps(["EVENT", ev]))
                    ok = json.loads(await ws.recv())
                    assert ok[2] is True, f"allowed post {idx} rejected: {ok}"

                ev_limited = make_synthetic_nostr_event(pubkey=registered_pubkey, kind=1, content="hive post 4")
                await ws.send(json.dumps(["EVENT", ev_limited]))
                limited_ok = json.loads(await ws.recv())
                assert limited_ok[2] is False and limited_ok[3].startswith("rate-limited:"), f"expected rate limit, got {limited_ok}"

            # 4. Inbound mirror filter drops unregistered spam via shadowReject
            spam_ev = make_synthetic_nostr_event(pubkey=unregistered_pubkey, kind=1, content="public relay spam")
            mirror_spam_decision = engine.evaluate(
                {"type": "new", "event": spam_ev, "receivedAt": int(time.time()), "sourceType": "Stream"}
            )
            assert mirror_spam_decision["action"] == "shadowReject"

            # Registered member inbound mirror event is accepted
            member_mirror_ev = make_synthetic_nostr_event(pubkey=registered_pubkey, kind=1, content="mirrored hive note")
            mirror_ok_decision = engine.evaluate(
                {"type": "new", "event": member_mirror_ev, "receivedAt": int(time.time()), "sourceType": "Stream"}
            )
            assert mirror_ok_decision["action"] == "accept"

            return {
                "ok": True,
                "mode": "self-test",
                "http": http_res,
                "unauthed": unauthed_res,
                "nip42_auth_ok": True,
                "disallowed_kind_blocked": True,
                "per_pubkey_rate_limit_enforced": True,
                "inbound_mirror_spam_filtered": True,
            }
        finally:
            ws_server.close()
            await ws_server.wait_closed()
            http_server.shutdown()
            http_server.server_close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify relay.postfiat.org health, NIP-11, NIP-42, and rate limits")
    parser.add_argument("--self-test", action="store_true", help="Run full offline local relay + write-policy self-test")
    parser.add_argument("--http-url", default="https://relay.postfiat.org", help="HTTP/HTTPS base URL for /healthz and NIP-11")
    parser.add_argument("--ws-url", default="wss://relay.postfiat.org", help="WebSocket relay URL")
    args = parser.parse_args()

    if args.self_test:
        result = asyncio.run(run_self_test())
        print(json.dumps(result, indent=2))
        return 0

    http_res = probe_http_health_and_nip11(args.http_url)
    ws_res = asyncio.run(probe_ws_challenge_and_unauthed_reject(args.ws_url))
    print(json.dumps({"ok": True, "mode": "live", "http": http_res, "ws": ws_res}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
