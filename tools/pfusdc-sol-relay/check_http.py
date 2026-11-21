#!/usr/bin/env python3
"""Exercise wallet authorization, discovery, replay and status on an accepted fixture job."""

import argparse
import json
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

from jobs import job_id
from storage import ROUTE, encode, load


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    cfg = load(args.config)
    root = Path(__file__).resolve().parents[2]
    sig = (Path(cfg["capture_root"]) / "signature").read_text().strip()
    route = cfg.get("route_id", ROUTE)
    jid = job_id("deposit", sig, route)
    wallet_token = secrets.token_hex(32)
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        relay_port = sock.getsockname()[1]
    with tempfile.TemporaryDirectory() as tmp:
        service = subprocess.Popen(
            [
                sys.executable,
                Path(__file__).with_name("relay.py"),
                "--config",
                args.config,
                "serve",
                "--port",
                str(relay_port),
            ]
        )
        env = {
            **os.environ,
            "WALLET_PROXY_API_TOKEN": wallet_token,
            "TRUSTLESS_BRIDGE_JOB_ROOT": tmp,
            "PFT_SOL_RELAY_TOKEN_FILE": cfg["http_token_file"],
            "PFT_SOL_RELAY_ROUTE_ID": route,
            "PFT_SOL_RELAY_URL": f"http://127.0.0.1:{relay_port}",
        }
        proxy = subprocess.Popen(
            [
                "node",
                "-e",
                "const {server}=require('./server'); server.listen(0,'127.0.0.1',()=>console.log('HTTP_PORT='+server.address().port))",
            ],
            cwd=root / "wallet-proxy",
            env=env,
            stdout=subprocess.PIPE,
            text=True,
        )
        try:
            for line in proxy.stdout:
                if line.startswith("HTTP_PORT="):
                    port = int(line.split("=")[1])
                    break
            else:
                raise RuntimeError("wallet proxy did not start")

            def request(path, body=None, auth=False):
                headers = {
                    "Content-Type": "application/json",
                }
                if auth:
                    headers["Origin"] = "http://localhost:5173"
                    headers["Authorization"] = "Bearer " + wallet_token
                req = urllib.request.Request(
                    f"http://127.0.0.1:{port}" + path,
                    encode(body) if body else None,
                    headers,
                )
                try:
                    with urllib.request.urlopen(req, timeout=6) as response:
                        return response.status, json.load(response)
                except urllib.error.HTTPError as e:
                    return e.code, json.load(e)

            ready_path = "/api/bridge/readiness?route=" + route
            for _ in range(20):
                code, body = request(ready_path)
                if code == 200 and body.get("ready"):
                    break
                time.sleep(0.1)
            assert code == 200 and body["ledger"] == "stub"
            path = "/api/bridge/jobs"
            body = {"route_id": route, "deposit_tx_hash": sig}
            recipient = "pf" + "33" * 20
            query = path + "?recipient=" + recipient
            assert request(path, body)[0] == 403 and request(query)[0] == 403
            code, posted = request(path, body, True)
            assert (
                code == 202 and posted["idempotent_replay"] and posted["job_id"] == jid
            )
            code, row = request(path + "/" + jid)
            assert (
                code == 200
                and row["status"] == "accepted"
                and row["receipt_code"] == "ACCEPTED"
            )
            code, items = request(query, auth=True)
            assert code == 200 and jid in [item["job_id"] for item in items["jobs"]]
            assert request(path + "/0x" + "00" * 32)[0] == 404
            print(
                "PASS wallet proxy readiness=200 unauthenticated_create=403 unauthenticated_list=403 replay=202 status=accepted list=200 unknown=404"
            )
        finally:
            for proc in (proxy, service):
                proc.terminate()
                proc.wait(timeout=10)


if __name__ == "__main__":
    main()
