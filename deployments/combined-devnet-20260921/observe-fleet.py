#!/usr/bin/env python3
"""DEPLOY DAY ONLY: read fleet RPC and systemd identity, enforce convergence.
Not executed during the September 21 preparation.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import sys

PACKET = Path(__file__).resolve().parent
sys.path.insert(0, str(PACKET.parents[1] / "python"))
from postfiat_ops.safe_rollout import parse_inventory, rollout_order

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def rpc(port, method):
    request = {"version": "postfiat-local-rpc-v1", "id": f"deploy-observe-{method}",
               "method": method, "params": {}}
    with socket.create_connection(("127.0.0.1", port), timeout=30) as sock:
        stream = sock.makefile("rwb")
        stream.write((json.dumps(request) + "\n").encode())
        stream.flush()
        raw = stream.readline(8 * 1024 * 1024)
    reply = json.loads(raw)
    require(reply.get("ok") is True, f"RPC {method} failed on {port}")
    return reply["result"]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--inventory-file", type=Path, default=PACKET / "inventory.txt")
    parser.add_argument("--rpc-tunnel-base-port", type=int, default=39650)
    parser.add_argument("--applied", default="", help="comma-separated completed rollout prefix")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(not args.output.exists(), "observation output already exists")
    applied = args.applied.split(",") if args.applied else []
    require(applied == rollout_order("validator-1")[:len(applied)], "invalid applied prefix")
    desired = json.loads((PACKET / "manifest-input.unsigned.json").read_text())
    previous = json.loads((PACKET / "observed/previous-manifest-identity.json").read_text())
    manifest = args.stage / "rootfs/etc/postfiat/releases" / desired["deployment_id"] / "deployment-manifest.json"
    new_hash = hashlib.sha256(manifest.read_bytes()).hexdigest() if applied else None
    baseline = json.loads(args.baseline.read_text()) if args.baseline else None
    if baseline:
        require(baseline.get("verified") is True, "baseline was not verified")
    rows = []
    for index, entry in enumerate(parse_inventory(args.inventory_file)):
        validator = entry.validator_id
        expect = desired if validator in applied else previous
        expect_revision = desired["git_revision"] if validator in applied else "707e006f"
        expect_manifest = new_hash if validator in applied else previous["manifest_sha256"]
        status = rpc(args.rpc_tunnel_base_port + index, "status")
        info = rpc(args.rpc_tunnel_base_port + index, "server_info")
        mempool = rpc(args.rpc_tunnel_base_port + index, "mempool_status")
        for field, value in {
            "node_id": validator, "deployment_validator_id": validator,
            "deployment_manifest_sha256": expect_manifest,
            "build_git_revision": expect_revision, "build_profile": "release",
            "chain_id": desired["chain_id"], "genesis_hash": desired["genesis_hash"],
            "protocol_version": desired["protocol_version"], "rpc_schema": desired["rpc_schema"],
            "mempool_pending": 0,
        }.items():
            require(status.get(field) == value, f"{validator}: {field} differs from expected {value}")
        # Runtime status hashes files; its manifest_verified flag is deliberately false.
        # Authenticate with the real deployment-manifest-verify command below.
        expected_services = next(r["services"] for r in expect["validator_bindings"] if r["validator_id"] == validator)
        require(status.get("deployment_service_artifacts") == expected_services, f"{validator}: unit/environment hashes")
        require(status.get("deployment_runtime_artifacts") == {
            key: expect[key] for key in ("binary_sha256", "topology_sha256",
                  "swap_circuit_metadata_sha256", "private_egress_circuit_metadata_sha256")
        }, f"{validator}: runtime hashes")
        units = [f"postfiat-{validator}.service", f"postfiat-{validator}-rpc.service",
                 "postfiat-cobalt-shadow.service", "navcoin-ethereum-archive-rpc-20260907.service"]
        command = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", f"root@{entry.host}"]
        remote_config = f"/etc/postfiat/releases/{expect['deployment_id']}"
        remote_binary = f"/opt/postfiat/releases/{expect['deployment_id']}/postfiat-node"
        subprocess.run(command + [remote_binary, "deployment-manifest-verify",
            "--manifest-file", f"{remote_config}/deployment-manifest.json",
            "--trusted-publisher-key-file", f"{remote_config}/deployment.public.json",
            "--validator-id", validator,
            "--validator-bindings-file", f"{remote_config}/{validator}.bindings.json",
            "--runtime-binary-file", remote_binary,
            "--runtime-topology-file", f"{remote_config}/topology.json",
            "--runtime-swap-circuit-metadata-file", f"{remote_config}/swap.metadata.json",
            "--runtime-private-egress-circuit-metadata-file", f"{remote_config}/private-egress.metadata.json"],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        active = subprocess.run(command + ["systemctl", "is-active", *units],
                                check=True, capture_output=True, text=True).stdout.splitlines()
        require(active == ["active"] * 4, f"{validator}: four service health")
        process_hashes = []
        for unit in units:
            pid = subprocess.run(command + ["systemctl", "show", "--property=MainPID", "--value", unit],
                                 check=True, capture_output=True, text=True).stdout.strip()
            require(pid.isdecimal() and int(pid) > 0, f"{validator}: invalid PID")
            digest = subprocess.run(command + ["sha256sum", f"/proc/{pid}/exe"],
                                    check=True, capture_output=True, text=True).stdout.split()[0]
            if unit in units[:2]:
                require(digest == expect["binary_sha256"], f"{validator}: running executable hash")
            process_hashes.append(digest)
        auxiliary = subprocess.run(command + ["systemctl", "show", "--property=ExecStart", *units[2:]],
                                   check=True, capture_output=True, text=True).stdout
        auxiliary_config = subprocess.run(command + ["systemctl", "cat", *units[2:]],
                                          check=True, capture_output=True).stdout
        auxiliary_config_hash = hashlib.sha256(auxiliary_config).hexdigest()
        # ExecStart includes PID/timestamps; compare stable unit bytes and executable hashes.
        if baseline:
            previous_row = next(r for r in baseline["validators"] if r["validator_id"] == validator)
            require(process_hashes[2:] == previous_row["process_sha256"][2:], f"{validator}: auxiliary executable changed")
            require(auxiliary_config_hash == previous_row["auxiliary_unit_configuration_sha256"], f"{validator}: auxiliary unit configuration changed")
        if validator in applied:
            for marker in ("transport-validator.ready.json", "rpc.ready.json"):
                subprocess.run(command + ["test", "-s", f"/var/lib/postfiat/{validator}/readiness/{marker}"], check=True)
        rows.append({"validator_id": validator, "status": status, "server_info": info,
                     "mempool_status": mempool, "units": dict(zip(units, active)),
                     "process_sha256": process_hashes, "auxiliary_exec_start": auxiliary,
                     "auxiliary_unit_configuration_sha256": auxiliary_config_hash,
                     "manifest_signature_gate": "PASS",
                     "candidate_readiness": "PASS" if validator in applied else "NOT_CHECKED_LEGACY"})
    identities = {(r["status"]["block_height"], r["status"]["block_tip_hash"],
                   r["status"]["state_root"]) for r in rows}
    require(len(identities) == 1, "six-node ledger divergence")
    identity = next(iter(identities))
    require(isinstance(identity[0], int) and identity[0] >= 0 and identity[1] and identity[2], "incomplete ledger identity")
    if baseline:
        require(list(identity) == baseline["ledger_identity"], "ledger advanced/changed during the pre-activation rollout")
    output = {"observed_at": datetime.now(timezone.utc).isoformat(), "verified": True,
              "applied": applied, "ledger_identity": list(identity), "validators": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as handle:
        json.dump(output, handle, indent=2)
        handle.write("\n")
    print(f"PASS: four services per host, exact running binaries and deployment identities, six-node convergence; {args.output}")

if __name__ == "__main__":
    main()
