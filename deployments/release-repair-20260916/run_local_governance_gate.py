#!/usr/bin/env python3
"""Certify a signed FastPay V2 rotation on six disposable loopback clones."""
import argparse
import json
from pathlib import Path
import runpy
import socket
import subprocess
import sys
import time


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "clone-root", "key-root", "topology", "work-root", "receipt", "batch-file"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    args = parser.parse_args()
    for name, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, name, value.resolve())
    repo = Path(__file__).resolve().parents[2]
    gate = runpy.run_path(str(repo / "deployments/signing-fix-qualification-20260909/run_local_service_gate.py"))
    topology = gate["load_json"](args.topology)
    peers = topology.get("peers", [])
    if len(peers) != 6 or any(p["host"] != "127.0.0.1" for p in peers):
        raise RuntimeError("requires six disposable clones with loopback peers")
    for peer in peers:
        for field in ("rpc_port", "p2p_port"):
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", peer[field]))
    batch = gate["load_json"](args.batch_file)
    bootstrap = batch["fastpay_recovery_bootstraps"]
    if len(bootstrap) != 1 or bootstrap[0]["payload"]["committee"]["schema"] != "postfiat-fastpay-recovery-committee-v2":
        raise RuntimeError("requires one signed FastPay V2 rotation")
    amendment_id = bootstrap[0]["amendment"]["amendment_id"]
    sys.path.insert(0, str(repo / "python"))
    from postfiat_rpc.client import PostFiatRpcClient
    clients = [PostFiatRpcClient(f"127.0.0.1:{p['rpc_port']}", timeout_seconds=180) for p in peers]
    services = gate["Services"](args, topology)
    receipt = {"schema": "postfiat-release-repair-service-gate-v1", "scope": "disposable_loopback_clones",
               "source_commit": args.source_commit, "binary_sha256": gate["sha256"](args.binary),
               "batch_sha256": gate["sha256"](args.batch_file), "live_mutations": False}
    def command(*parts):
        return json.loads(subprocess.check_output([str(args.binary), *map(str, parts)], text=True))
    try:
        receipt["key_staging"] = gate["stage_local_keys"](args)
        services.start_all()
        receipt["start_orders"] = services.orders
        initial = gate["statuses"](clients)
        receipt["initial_statuses"] = initial
        height = initial[0]["block_height"] + 1
        proposer = command("block-proposer", "--data-dir", args.clone_root / "validator-0", "--height", height, "--view", 0)["proposer"]
        print(f"START PASS: six services agree at height {height - 1}", flush=True)
        round_result = command("transport-peer-certified-batch-round", "--data-dir", args.clone_root / proposer,
                               "--topology", args.topology, "--batch-kind", "governance", "--batch-file", args.batch_file,
                               "--key-file", args.key_root / proposer / "validator_keys.json",
                               "--proposal-key-file", args.key_root / proposer / "validator_keys.json",
                               "--require-local-proposer", "--artifact-dir", args.work_root / "round",
                               "--height", height, "--timeout-ms", 60000, "--send-retries", 16, "--retry-backoff-ms", 250)
        (args.work_root / "round-result.json").write_text(json.dumps(round_result, indent=2) + "\n")
        deadline = time.monotonic() + 180
        while True:
            try:
                final = gate["disk_statuses"](args.binary, args.clone_root)
                if final[0]["block_height"] == height:
                    break
            except Exception:
                pass
            if time.monotonic() >= deadline:
                raise RuntimeError("six clones failed to converge after certified rotation")
            time.sleep(2)
        receipts = []
        for index in range(6):
            rows = command("receipts", "--data-dir", args.clone_root / f"validator-{index}", "--tx-id", amendment_id)
            if len(rows) != 1 or not rows[0]["accepted"] or rows[0]["code"] != "fastpay_recovery_committee_rotated":
                raise RuntimeError(f"validator-{index} lacks the accepted rotation receipt")
            receipts.append(rows[0])
        receipt["accepted_receipts"] = receipts
        receipt["finality_statuses"] = final
        print(f"FINALITY PASS: all six accepted V2 rotation at height {height}", flush=True)
        services.stop_all()
        services.start_all()
        restarted = gate["statuses"](clients)
        identity = lambda row: (row["block_height"], row["block_tip_hash"], row["state_root"])
        if identity(restarted[0]) != identity(final[0]):
            raise RuntimeError("restart changed the certified tip")
        receipt["restart_statuses"] = restarted
        receipt["result"] = "PASS"
        print(f"RESTART PASS: all six retain the certified height {height}", flush=True)
        result = 0
    except Exception as error:
        receipt.update(result="FAIL", error=str(error))
        print(f"GATE FAIL: {error}", file=sys.stderr)
        result = 1
    finally:
        services.stop_all()
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
