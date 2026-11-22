"""Wallet bridge job API. Bind on loopback, behind the wallet proxy."""

import hmac
import json
import logging
import re
import subprocess
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from storage import DEPOSIT, AwaitingSource, Invalid, encode, load


def serve(relay, port):
    token = Path(relay.cfg["http_token_file"]).read_text().strip()
    if not token:
        raise Invalid("empty HTTP service credential")
    workers = ThreadPoolExecutor(max_workers=relay.cfg.get("workers", 4))
    outstanding = set()
    lock = threading.Lock()

    def enqueue(job):
        def run():
            try:
                relay.run(job, wait=False)
            except AwaitingSource:
                return
            except (
                OSError,
                ValueError,
                RuntimeError,
                subprocess.SubprocessError,
                KeyError,
            ) as error:
                logging.getLogger(__name__).error(
                    "Solana relay job %s: %s", job.name, error
                )
            finally:
                with lock:
                    outstanding.discard(job)

        with lock:
            if job not in outstanding:
                outstanding.add(job)
                workers.submit(run)

    class Handler(BaseHTTPRequestHandler):
        def send(self, code, body):
            raw = encode(body)
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

        def handle_api(self):
            url = urlsplit(self.path)
            query = parse_qs(url.query)
            if url.path == "/api/bridge/jobs" and not hmac.compare_digest(
                self.headers.get("Authorization", "").encode(),
                ("Bearer " + token).encode(),
            ):
                return self.send(403, {"ok": False, "code": "bridge_session_required"})
            if self.command == "POST" and url.path == "/api/bridge/jobs":
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 8192:
                    raise Invalid("invalid request body length")
                body = json.loads(self.rfile.read(length))
                if not isinstance(body, dict) or body.get("route_id") != relay.route:
                    raise Invalid("unsupported bridge route")
                sig = body.get("deposit_tx_hash", body.get("signature"))
                if sig != body.get("signature", sig):
                    raise Invalid("conflicting deposit signatures")
                job, replay = relay.create("deposit", sig)
                state = load(job / "job.json")
                if state["status"] != "accepted" and state.get("retryable", True):
                    enqueue(job)
                return self.send(
                    202,
                    dict(
                        state,
                        idempotent_replay=replay,
                        idempotency_key=body.get("idempotency_key"),
                    ),
                )
            if self.command == "GET":
                if url.path == "/api/bridge/readiness" and query.get("route") == [
                    relay.route
                ]:
                    try:
                        for field in ("host", "deposit_elf", "key_file"):
                            if not Path(relay.cfg[field]).is_file():
                                raise FileNotFoundError("missing " + field)
                        if relay.cfg["ledger"] == "node":
                            relay.ledger.route()
                        else:
                            relay.ledger.client()
                        # Deposits close while the Solana observer or the prover is unhealthy.
                        if relay.cfg.get("health_file"):
                            health = json.loads(Path(relay.cfg["health_file"]).read_text())
                            if not health["ok"] or time.time() - health["checked_at"] > 120:
                                raise RuntimeError(health.get("message") or "bridge health check is stale")
                    except (OSError, ValueError, RuntimeError, KeyError) as error:
                        return self.send(
                            503,
                            {
                                "ok": False,
                                "ready": False,
                                "route_id": relay.route,
                                "message": str(error),
                            },
                        )
                    return self.send(
                        200,
                        {
                            "ok": True,
                            "ready": True,
                            "route_profile_hash": relay.cfg["profile_hash"],
                            "route_id": relay.route,
                            "source_chain_id": 0,
                            "source_proof_kind": DEPOSIT,
                            "program_vkey": relay.cfg["deposit_vkey"],
                            "observer_attestor_enabled": False,
                            "ledger": relay.cfg["ledger"],
                        },
                    )
                if url.path == "/api/bridge/jobs":
                    recipient = query.get("recipient", [""])[0].lower()
                    if not re.fullmatch(r"pf[0-9a-f]{40}", recipient):
                        raise Invalid("invalid recipient")
                    limit = max(1, min(100, int(query.get("limit", ["20"])[0])))
                    jobs = [
                        j
                        for j in relay.all_jobs()
                        if j["request"].get("pftl_recipient") == recipient
                        or any(
                            r["pftl_recipient"] == recipient
                            for r in j.get("receipts", [])
                        )
                    ]
                    jobs.sort(key=lambda j: j["created_at"], reverse=True)
                    return self.send(
                        200,
                        {
                            "ok": True,
                            "schema": "postfiat-trustless-bridge-job-list-v1",
                            "recipient": recipient,
                            "jobs": jobs[:limit],
                        },
                    )
                if url.path.startswith("/api/bridge/jobs/"):
                    return self.send(
                        200, relay.status(url.path.removeprefix("/api/bridge/jobs/"))
                    )
            return self.send(404, {"ok": False, "code": "bridge_job_not_found"})

        def dispatch(self):
            try:
                self.handle_api()
            except FileNotFoundError:
                self.send(404, {"ok": False, "code": "bridge_job_not_found"})
            except (ValueError, TypeError, KeyError) as error:
                self.send(
                    400,
                    {
                        "ok": False,
                        "code": "invalid_bridge_request",
                        "message": str(error),
                    },
                )

        do_GET = dispatch
        do_POST = dispatch

    class Server(ThreadingHTTPServer):
        def service_actions(self):
            # Pending finality never monopolizes proof workers. Recover after a process
            # death and retry transient failures using the same signed outbox.
            for file in relay.jobs.glob("*/job.json"):
                state = load(file)
                if state["status"] != "accepted" and state.get("retryable", True):
                    enqueue(file.parent)

    with Server(("127.0.0.1", port), Handler) as server:
        server.serve_forever(poll_interval=0.5)
