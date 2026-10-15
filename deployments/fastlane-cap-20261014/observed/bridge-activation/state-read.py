#!/usr/bin/env python3
"""DEPLOY-SHEET section 7: read-only state read before and after each live step.

Usage: state-read.py TAG   writes observed/bridge-activation/state-TAG.json.
Reads through the localhost RPC tunnels (27650 + N): status of all six,
`fastswap_checkpoint_status` and `fastswap_capabilities` (validator-5 is
expected to answer `fastswap_unavailable` until its local committee changes),
`owned_recovery_capabilities` (FastPay committee), the A666 and BMNRC route
claims and the mempool. Then runs observe-fleet.py for six-way agreement.
Writes nothing on any host.
"""
import json
import socket
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PACKET = HERE.parents[1]
REPO = PACKET.parents[1]
STAGE = Path.home() / ".postfiat/deployments/fastlane-cap-20261014/stage"
APPLIED = "validator-1,validator-0,validator-2,validator-3,validator-4,validator-5"
ROUTES = ("pftl-a666-ethereum-wA666-usdc-v1", "pftl-bmnrc-ethereum-wBMNRC-usdc-v1")


def rpc(port, method, params=None):
    request = {"version": "postfiat-local-rpc-v1", "id": f"bridge-activation-{method}",
               "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", port), timeout=60) as sock:
        stream = sock.makefile("rwb")
        stream.write((json.dumps(request) + "\n").encode())
        stream.flush()
        return json.loads(stream.readline(64 * 1024 * 1024))


def hexify(value):
    """Byte arrays from serde become hex; everything else unchanged."""
    if isinstance(value, list) and len(value) >= 16 and all(
            isinstance(item, int) and 0 <= item < 256 for item in value):
        return bytes(value).hex()
    if isinstance(value, list):
        return [hexify(item) for item in value]
    if isinstance(value, dict):
        return {key: hexify(item) for key, item in value.items()}
    return value


def checkpoint(reply):
    if not reply.get("ok"):
        return {"error": reply.get("error", {}).get("code")}
    result = hexify(reply["result"])
    point = result["checkpoint"]
    return {
        "committee_epoch": point["committee"]["committee_epoch"],
        "committee_root": point["committee"]["committee_root"],
        "drain_ready": result["drain_ready"],
        "rotation_ready": result["rotation_ready"],
        "fenced_policy_epochs": point["fenced_policy_epochs"],
        "previous_checkpoint_id": point["previous_checkpoint_id"],
        "highest_wal_sequence": point["highest_wal_sequence"],
        "active_policy_hashes": point["active_policy_hashes"],
        "exit_claim_totals": point["exit_claim_totals"],
        "terminal_root": point["terminal_root"],
    }


def main():
    tag = sys.argv[1]
    record = {"tag": tag, "read_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "validators": {}}
    for n in range(6):
        port = 27650 + n
        status = rpc(port, "status")["result"]
        capabilities = rpc(port, "fastswap_capabilities")
        fastpay = rpc(port, "owned_recovery_capabilities")
        record["validators"][f"validator-{n}"] = {
            "height": status["block_height"],
            "tip": status["block_tip_hash"],
            "state_root": status["state_root"],
            "mempool_pending": status["mempool_pending"],
            "fastswap_committee": (
                {"epoch": capabilities["result"]["committee"]["committee_epoch"],
                 "root": hexify(capabilities["result"]["committee"]["committee_root"])}
                if capabilities.get("ok") else {"error": capabilities["error"]["code"]}),
            "fastswap_checkpoint_status": checkpoint(rpc(port, "fastswap_checkpoint_status")),
            "fastpay_committee": (
                {key: fastpay["result"][key] for key in
                 ("committee_epoch", "quorum", "validator_count", "current_height")}
                if fastpay.get("ok") else {"error": fastpay["error"]["code"]}),
        }
    routes = rpc(27651, "navcoin_bridge_routes")["result"]["routes"]
    record["routes"] = [
        {key: route[key] for key in ("route_id", "outstanding_bridge_claims_atoms",
                                     "pending_return_import_claims_atoms",
                                     "outstanding_export_packet_count",
                                     "pending_return_burn_count", "paused", "ledger_hash")}
        for route in routes if route["route_id"] in ROUTES]
    observe = HERE / f"observe-{tag}.json"
    run = subprocess.run(["python3", str(PACKET / "observe-fleet.py"), "--stage", str(STAGE),
                          "--applied", APPLIED, "--output", str(observe)],
                         capture_output=True, text=True, cwd=REPO)
    record["observe_fleet"] = (run.stdout.strip() or run.stderr.strip()).splitlines()[-1:]
    record["observe_fleet_rc"] = run.returncode
    out = HERE / f"state-{tag}.json"
    out.write_text(json.dumps(record, indent=2) + "\n")
    heights = {v["height"] for v in record["validators"].values()}
    roots = {v["state_root"] for v in record["validators"].values()}
    print(f"{tag}: heights={sorted(heights)} roots={[r[:12] for r in roots]} "
          f"observe rc={run.returncode} {record['observe_fleet']}")
    for name, row in record["validators"].items():
        cp = row["fastswap_checkpoint_status"]
        print(f"  {name}: fastswap={row['fastswap_committee']} fastpay={row['fastpay_committee']} "
              f"checkpoint={ {k: cp.get(k) for k in ('committee_epoch', 'fenced_policy_epochs', 'previous_checkpoint_id', 'highest_wal_sequence', 'drain_ready', 'error')} }")
    for route in record["routes"]:
        print(f"  {route['route_id']}: claims={route['outstanding_bridge_claims_atoms']} "
              f"returns={route['pending_return_import_claims_atoms']}")
    return run.returncode


if __name__ == "__main__":
    sys.exit(main())
