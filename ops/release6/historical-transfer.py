#!/usr/bin/env python3
"""Verify an archived ordinary transfer over six isolated release 6 RPC and transport pairs."""
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
    height = args.height
    name = f"validator-{n}"
    dest = args.work / name
    cli("snapshot-import-finalized-checkpoint", "--snapshot-dir", args.snapshot, "--data-dir", dest,
        "--node-id", name)
    old = cli("verify-finalized-checkpoint", "--data-dir", dest)
    if old["checkpoint_height"] != height - 1 or not old["verified"]:
        raise ValueError("unexpected historical base")
    port = 42100 + n
    block, = rpc(port, "blocks", {"from_height": height, "limit": 1})
    source = rpc(port, "status", {})
    recorded = args.records[name]
    if (block["header"]["height"], block["header"]["batch_kind"], block["header"]["parent_hash"],
        block["header"]["block_hash"], block["receipt_ids"]) != (
            height, "transparent", old["checkpoint_block_hash"], recorded["block_hash"], [receipt_id]):
        raise ValueError(f"{name} differs from the recorded transfer")
    run = cli("rpc-catch-up-certified-delta", "--data-dir", dest, "--source-host", "127.0.0.1",
              "--source-rpc-port", port, "--expected-height", height, "--expected-block-hash", recorded["block_hash"],
              "--expected-state-root", recorded["state_root"], "--work-dir", args.work/f"delta-{n}",
              "--max-blocks", 1, "--timeout-ms", 120000)
    verified = cli("verify-finalized-checkpoint", "--data-dir", dest)
    receipt, = [r for r in cli("receipts", "--data-dir", dest) if r["tx_id"] == receipt_id]
    if (source["node_id"] != name or run["local_height_after"] != height or run["applied_count"] != 1
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
    parser.add_argument("--height", type=int, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    if set(os.listdir("/sys/class/net")) != {"lo"}:
        raise RuntimeError("requires an isolated network namespace")
    args.records = json.loads(args.release5_record.read_text())
    receipt_ids = {r["tx_id"] for row in args.records.values() for r in row["receipts"] if r["accepted"]}
    if len(args.records) != 6 or len(receipt_ids) != 1:
        raise ValueError("source records disagree")
    receipt_id, = receipt_ids
    topology = args.work / "topology.json"
    cli("topology-consensus-v2", "--chain-id", "postfiat-wan-devnet-2", "--validators", 6,
        "--activation-height", 1, "--base-port", 42000, "--rpc-base-port", 42100,
        "--hosts", ",".join(["127.0.0.1"] * 6), "--output", topology)
    servers = []
    try:
        for n in range(6):
            data = args.sources / f"validator-{n}"
            signer = data / "validator_keys.json"
            if signer.exists():
                raise RuntimeError("requires a keyless restored validator")
            # Exercise the normal finality configuration check before starting the read RPC.
            probe = subprocess.run([BINARY, "rpc-serve", "--unsafe-devnet-json-storage", "--disable-owned-lane",
                "--allow-mempool-submit", "--allow-mempool-submit-finality", "--data-dir", str(data),
                "--finality-topology", str(topology), "--finality-key-file", str(signer),
                "--spool-dir", str(args.work/f"probe-{n}"), "--ready-file", str(args.work/f"probe-{n}.json"),
                "--bind-host", "127.0.0.1", "--port", str(42100+n)], capture_output=True, text=True, timeout=45)
            message = probe.stderr.split("usage:")[0].strip()
            if probe.returncode == 0 or "finality configuration files are unavailable" not in message:
                raise RuntimeError(message[:800])
            print(f"validator-{n} fresh finality startup exit={probe.returncode} {message}", flush=True)
            for role in ("transport", "rpc"):
                ready = args.work / f"{role}-{n}.ready"
                ready.unlink(missing_ok=True)
                env = {**os.environ, "RAYON_NUM_THREADS": "3", "POSTFIAT_PREWARM_SHIELDED_VERIFIER": "1",
                       "POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE": str(ready)}
                if role == "rpc":
                    command = [BINARY, "rpc-serve", "--unsafe-devnet-json-storage", "--disable-owned-lane",
                        "--data-dir", str(data), "--spool-dir", str(args.work/f"spool-{n}"), "--ready-file", str(ready),
                        "--bind-host", "127.0.0.1", "--port", str(42100+n), "--timeout-ms", "120000"]
                else:
                    command = [BINARY, "transport-validator-serve", "--unsafe-devnet-file-signer",
                        "--unsafe-devnet-json-storage", "--data-dir", str(data), "--topology", str(topology),
                        "--key-file", str(signer), "--vote-dir", str(args.work/f"votes-{n}"),
                        "--bind-host", "127.0.0.1", "--timeout-ms", "120000"]
                log_path = args.work / f"{role}-{n}.log"
                with log_path.open("w") as log:
                    proc = subprocess.Popen(command, stdout=log, stderr=log, env=env)
                servers.append((proc, log_path))
        deadline = time.monotonic() + 240
        while len(list(args.work.glob("*.ready"))) != 12:
            for proc, log_path in servers:
                if proc.poll() is not None:
                    raise RuntimeError(log_path.read_text().split("usage:")[0][-1200:])
            if time.monotonic() > deadline:
                raise TimeoutError("six transport and six RPC services did not become ready")
            time.sleep(.25)
        print("six transport validators and six read RPCs ready on isolated loopback", flush=True)
        with ThreadPoolExecutor(max_workers=6) as pool:
            for result in pool.map(lambda n: replay(n, args, receipt_id), range(6)):
                print(json.dumps(result), flush=True)
    finally:
        for proc, _ in servers:
            proc.terminate()
        for proc, _ in servers:
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()


if __name__ == "__main__":
    main()
