"""Capture a short window near the public finalized head and timestamp finalized RPC observations."""
import argparse
import json
import os
from pathlib import Path
import time
import urllib.request


def slot(url, commitment):
    request = urllib.request.Request(url, json.dumps({"jsonrpc": "2.0", "id": 1, "method": "getSlot",
                                      "params": [{"commitment": commitment}]}).encode(),
                                     {"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=4) as response:
        return int(json.load(response)["result"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cluster", choices=["testnet", "mainnet-beta"])
    parser.add_argument("ledger", type=Path)
    parser.add_argument("rpc_port", type=int)
    args = parser.parse_args()
    live = f"https://api.{args.cluster}.solana.com"
    local = f"http://127.0.0.1:{args.rpc_port}"
    head = slot(live, "finalized")
    local_slot = slot(local, "processed")
    if abs(local_slot - head) > 20:
        parser.exit(1, f"observer replay at {local_slot}, public finalized head {head}\n")
    first, last = head + 12, head + 35
    directory = args.ledger / "public-capture"
    temp = directory / f".slots-{os.getpid()}"
    temp.write_text(f"{first} {last}\n")
    temp.replace(directory / "slots")
    print(f"armed {first}..{last} public={head} observer={local_slot}", flush=True)
    journal = directory / f"finality-{first}-{last}.jsonl"
    with journal.open("x") as out:
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            row = {"request_ns": time.time_ns()}
            try:
                row["finalized"] = slot(live, "finalized")
            except Exception as e:
                row["error"] = str(e)
            row["response_ns"] = time.time_ns()
            print(json.dumps(row), file=out, flush=True)
            if row.get("finalized", 0) >= last + 1:
                return
            time.sleep(0.25)
        raise SystemExit("finalized polling window incomplete")


if __name__ == "__main__":
    main()
