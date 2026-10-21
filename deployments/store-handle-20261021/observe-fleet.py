#!/usr/bin/env python3
"""Read-only fleet observation for store-handle-20261021.

Reads status, server_info and mempool_status through the localhost RPC tunnels,
and over SSH: unit health, running executable hashes and paths, the real
deployment-manifest-verify of each host's running release, the
apt-daily-upgrade timer and root-filesystem usage. Enforces the expected
identity (applied prefix on the new release, the rest on fastlane-cap-20261014) and six-node
convergence. Writes nothing on any host.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import sys

PACKET = Path(__file__).resolve().parent
sys.path.insert(0, str(PACKET.parents[1] / "python"))
from postfiat_ops.safe_rollout import parse_inventory, rollout_order  # noqa: E402

PREVIOUS = {
    "release_id": "fastlane-cap-20261014",
    "binary_sha256": "b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405",
    "build_git_revision": "8bed9b90",
    "manifest_sha256": "4f379c57b42c4a050154a344e7cea72e1af381e9f2a05ff21d3b2a1831465b48",
}
CHAIN = {
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",
    "protocol_version": 1,
    "rpc_schema": "postfiat-local-rpc-v1",
}
AUXILIARY = ["postfiat-cobalt-shadow.service", "navcoin-ethereum-rpc-28703.service"]


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
        reply = json.loads(stream.readline(8 * 1024 * 1024))
    require(reply.get("ok") is True, f"RPC {method} failed on {port}")
    return reply["result"]


def ssh(host, *command):
    return subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15",
                           f"root@{host}", *command],
                          check=True, capture_output=True, text=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, help="signed stage (required once a validator is applied)")
    parser.add_argument("--rpc-tunnel-base-port", type=int, default=27650)
    parser.add_argument("--applied", default="", help="comma-separated completed rollout prefix")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(not args.output.exists(), "observation output already exists")
    applied = args.applied.split(",") if args.applied else []
    require(applied == rollout_order("validator-1")[:len(applied)], "invalid applied prefix")
    new = None
    if applied:
        desired = json.loads((PACKET / "manifest-input.unsigned.json").read_text())
        require(all(desired[k] == v for k, v in CHAIN.items()), "manifest input chain identity")
        new = {"release_id": desired["deployment_id"], "binary_sha256": desired["binary_sha256"],
               "build_git_revision": desired["git_revision"][:8]}
        manifest = args.stage / "rootfs/etc/postfiat/releases" / new["release_id"] / "deployment-manifest.json"
        new["manifest_sha256"] = hashlib.sha256(manifest.read_bytes()).hexdigest()
    def observe(index, entry):
        validator, host = entry.validator_id, entry.host
        expect = new if validator in applied else PREVIOUS
        port = args.rpc_tunnel_base_port + index
        status, info, mempool = rpc(port, "status"), rpc(port, "server_info"), rpc(port, "mempool_status")
        for field, value in {
            "node_id": validator, "deployment_validator_id": validator,
            "deployment_manifest_sha256": expect["manifest_sha256"],
            "build_git_revision": expect["build_git_revision"], "build_profile": "release",
            **CHAIN, "mempool_pending": 0,
        }.items():
            require(status.get(field) == value, f"{validator}: {field}={status.get(field)!r}, expected {value!r}")
        require(status["deployment_runtime_artifacts"]["binary_sha256"] == expect["binary_sha256"],
                f"{validator}: runtime binary hash")
        if validator in applied:
            services = next(r["services"] for r in desired["validator_bindings"] if r["validator_id"] == validator)
            require(status.get("deployment_service_artifacts") == services, f"{validator}: unit/environment hashes")
        units = [f"postfiat-{validator}.service", f"postfiat-{validator}-rpc.service", *AUXILIARY]
        active = ssh(host, "systemctl", "is-active", *units).splitlines()
        require(active == ["active"] * len(units), f"{validator}: service health {active}")
        processes = {}
        for unit in units:
            pid = ssh(host, "systemctl", "show", "--property=MainPID", "--value", unit).strip()
            require(pid.isdecimal() and int(pid) > 0, f"{validator}: invalid PID for {unit}")
            digest = ssh(host, "sha256sum", f"/proc/{pid}/exe").split()[0]
            path = ssh(host, "readlink", "-f", f"/proc/{pid}/exe").strip()
            processes[unit] = {"pid": int(pid), "exe": path, "sha256": digest}
        release_binary = f"/opt/postfiat/releases/{expect['release_id']}/postfiat-node"
        for unit in units[:2]:
            require(processes[unit]["sha256"] == expect["binary_sha256"], f"{validator}: running {unit} hash")
            require(processes[unit]["exe"] == release_binary, f"{validator}: running {unit} path")
        config = f"/etc/postfiat/releases/{expect['release_id']}"
        ssh(host, release_binary, "deployment-manifest-verify",
            "--manifest-file", f"{config}/deployment-manifest.json",
            "--trusted-publisher-key-file", f"{config}/deployment.public.json",
            "--validator-id", validator,
            "--validator-bindings-file", f"{config}/{validator}.bindings.json",
            "--runtime-binary-file", release_binary,
            "--runtime-topology-file", f"{config}/topology.json",
            "--runtime-swap-circuit-metadata-file", f"{config}/swap.metadata.json",
            "--runtime-private-egress-circuit-metadata-file", f"{config}/private-egress.metadata.json")
        timer = ssh(host, "systemctl", "show", "apt-daily-upgrade.timer",
                    "--property=NextElapseUSecRealtime,LastTriggerUSec").strip().splitlines()
        upgrade_active = subprocess.run(["ssh", "-o", "BatchMode=yes", f"root@{host}", "systemctl",
                                         "is-active", "apt-daily-upgrade.service"],
                                        capture_output=True, text=True).stdout.strip()
        require(upgrade_active != "active", f"{validator}: apt-daily-upgrade is running")
        disk = ssh(host, "df", "-B1", "--output=size,used,avail,pcent", "/").splitlines()[1].split()
        return {
            "validator_id": validator, "host": host, "release_id": expect["release_id"],
            "height": status["block_height"], "tip": status["block_tip_hash"],
            "state_root": status["state_root"], "mempool_pending": status["mempool_pending"],
            "build_git_revision": status["build_git_revision"],
            "deployment_manifest_sha256": status["deployment_manifest_sha256"],
            "units": dict(zip(units, active)), "processes": processes,
            "manifest_signature_gate": "PASS", "apt_daily_upgrade_timer": timer,
            "apt_daily_upgrade_service": upgrade_active,
            "root_fs_bytes": dict(zip(["size", "used", "avail", "pcent"], disk)),
            "status": status, "server_info": info, "mempool_status": mempool,
        }
    # The six validators are observed concurrently; each check is unchanged.
    entries = list(enumerate(parse_inventory(PACKET / "inventory.txt")))
    with ThreadPoolExecutor(max_workers=len(entries)) as pool:
        rows = list(pool.map(lambda pair: observe(*pair), entries))
    identities = {(r["height"], r["tip"], r["state_root"]) for r in rows}
    require(len(identities) == 1, f"six-node ledger divergence: {sorted(identities)}")
    height, tip, root = next(iter(identities))
    output = {"observed_at": datetime.now(timezone.utc).isoformat(), "verified": True,
              "applied": applied, "ledger_identity": {"height": height, "tip": tip, "state_root": root},
              "validators": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as handle:
        json.dump(output, handle, indent=2)
        handle.write("\n")
    summary = ", ".join(f"{r['validator_id']}={r['release_id']}" for r in rows)
    print(f"PASS height={height} tip={tip[:12]} root={root[:12]} {summary}; {args.output}")


if __name__ == "__main__":
    main()
