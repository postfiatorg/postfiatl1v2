#!/usr/bin/env python3
"""Restore through a fixed target with the node's certified-delta verifier and a read-only peer."""
import argparse
import json
from pathlib import Path
import socket
import subprocess


def rpc(port, method, params):
    request = dict(version="postfiat-local-rpc-v1", id="release6-restore", method=method, params=params)
    with socket.create_connection(("127.0.0.1", port), timeout=120) as conn:
        conn.sendall(json.dumps(request).encode() + b"\n")
        response = json.loads(conn.makefile("rb").readline())
    if response.get("ok") is not True:
        raise RuntimeError(response.get("error"))
    return response["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("store", type=Path)
    parser.add_argument("work", type=Path)
    parser.add_argument("peer", type=int, choices=range(6))
    parser.add_argument("target", type=int)
    args = parser.parse_args()
    port = 39650 + args.peer
    status = json.loads(subprocess.check_output([args.binary, "status", "--data-dir", args.store]))
    height = status["block_height"]
    # Keep blocks responses within the live RPC byte bound.
    while height < args.target:
        end = min(height + 4, args.target)
        block, = rpc(port, "blocks", {"from_height": end, "limit": 1})
        header = block["header"]
        if header["height"] != end:
            raise RuntimeError(f"expected remote height {end}, got {header['height']}")
        directory = args.work / str(end)
        directory.mkdir(parents=True, exist_ok=True)
        command = [str(args.binary), "rpc-catch-up-certified-delta", "--data-dir", str(args.store),
                   "--source-host", "127.0.0.1", "--source-rpc-port", str(port),
                   "--expected-height", str(end), "--expected-block-hash", header["block_hash"],
                   "--expected-state-root", header["state_root"], "--work-dir", str(directory),
                   "--max-blocks", str(end-height), "--timeout-ms", "120000"]
        print(" ".join(command), flush=True)
        with (directory / "result.json").open("w") as result:
            subprocess.run(command, check=True, stdout=result)
        report = json.loads((directory / "result.json").read_text())
        if report["local_height_after"] != end or report["applied_count"] != end-height:
            raise RuntimeError("incomplete certified catch-up")
        print(end, report["local_block_tip_after"], report["local_state_root_after"], flush=True)
        height = end


if __name__ == "__main__":
    main()
