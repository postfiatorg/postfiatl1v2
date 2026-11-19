#!/usr/bin/env python3
"""Replay the release 5 rollout transfer at 1426 over six isolated release 6 RPCs."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import socket
import subprocess
import time

BINARY = "/opt/postfiat-node"


def cli(*args):
    command = [BINARY, *map(str, args)]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{command[1]}: {result.stderr.split('usage:')[0][-1300:]}")
    return json.loads(result.stdout)


def rpc(port, method, params):
    request = dict(version="postfiat-local-rpc-v1", id="rehearsal", method=method, params=params)
    with socket.create_connection(("127.0.0.1", port), timeout=120) as conn:
        conn.sendall(json.dumps(request).encode() + b"\n")
        response = json.loads(conn.makefile("rb").readline())
    if response.get("ok") is not True:
        raise RuntimeError(response.get("error"))
    return response["result"]


def replay(n, args, receipt_id):
    name = f"validator-{n}"
    dest = args.work / name
    cli("snapshot-import-finalized-checkpoint", "--snapshot-dir", args.snapshot, "--data-dir", dest,
        "--node-id", name)
    old = cli("verify-finalized-checkpoint", "--data-dir", dest)
    if old["checkpoint_height"] != 1425 or not old["verified"]:
        raise ValueError("unexpected historical base")
    port = 42100 + n
    block, = rpc(port, "blocks", {"from_height": 1426, "limit": 1})
    source = rpc(port, "status", {})
    recorded = args.records[name]
    if (block["header"]["height"], block["header"]["batch_kind"], block["header"]["parent_hash"],
        block["header"]["block_hash"], block["receipt_ids"]) != (
            1426, "transparent", old["checkpoint_block_hash"], recorded["block_hash"], [receipt_id]):
        raise ValueError(f"{name} differs from release 5 rollout record")
    run = cli("rpc-catch-up-certified-delta", "--data-dir", dest, "--source-host", "127.0.0.1",
              "--source-rpc-port", port, "--expected-height", 1426, "--expected-block-hash", recorded["block_hash"],
              "--expected-state-root", recorded["state_root"], "--work-dir", args.work/f"delta-{n}",
              "--max-blocks", 1, "--timeout-ms", 120000)
    verified = cli("verify-finalized-checkpoint", "--data-dir", dest)
    receipt, = [r for r in cli("receipts", "--data-dir", dest) if r["tx_id"] == receipt_id]
    if (source["node_id"] != name or run["local_height_after"] != 1426 or run["applied_count"] != 1
            or not verified["verified"] or verified["checkpoint_state_root"] != recorded["state_root"]
            or receipt != recorded["receipts"][0] or not receipt["accepted"]):
        raise ValueError(f"{name} failed historical transfer checks")
    return dict(node=name, source_revision=source["build_git_revision"], source_height=source["block_height"],
                height=verified["checkpoint_height"], tip=verified["checkpoint_block_hash"],
                root=verified["checkpoint_state_root"], quorum=verified["quorum"], tx=receipt_id,
                accepted=receipt["accepted"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ["sources", "snapshot", "work", "release5_record"]:
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    if set(os.listdir("/sys/class/net")) != {"lo"}:
        raise RuntimeError("requires an isolated network namespace")
    args.records = json.loads(args.release5_record.read_text())
    receipt_ids = {r["tx_id"] for row in args.records.values() for r in row["receipts"] if r["accepted"]}
    if len(args.records) != 6 or len(receipt_ids) != 1:
        raise ValueError("release 5 records disagree")
    receipt_id, = receipt_ids
    servers = []
    try:
        for n in range(6):
            data = args.sources / f"validator-{n}"
            ready = args.work / f"rpc-{n}.ready"
            ready.unlink(missing_ok=True)
            with (args.work/f"rpc-{n}.log").open("w") as log:
                proc = subprocess.Popen([BINARY, "rpc-serve", "--unsafe-devnet-json-storage", "--disable-owned-lane",
                    "--data-dir", str(data), "--spool-dir", str(args.work/f"spool-{n}"), "--ready-file", str(ready),
                    "--bind-host", "127.0.0.1", "--port", str(42100+n), "--timeout-ms", "120000"],
                    stdout=log, stderr=log)
            servers.append(proc)
        deadline = time.monotonic() + 180
        while len(list(args.work.glob("rpc-*.ready"))) != 6:
            for n, proc in enumerate(servers):
                if proc.poll() is not None:
                    raise RuntimeError((args.work/f"rpc-{n}.log").read_text().split("usage:")[0][-1200:])
            if time.monotonic() > deadline:
                raise TimeoutError("RPC startup")
            time.sleep(.25)
        with ThreadPoolExecutor(max_workers=6) as pool:
            for result in pool.map(lambda n: replay(n, args, receipt_id), range(6)):
                print(json.dumps(result), flush=True)
    finally:
        for proc in servers:
            proc.terminate()
        for proc in servers:
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()


if __name__ == "__main__":
    main()
