#!/usr/bin/env python3
"""Run the service/restart/finality part of the signing-fix gate on local clones.

This runner refuses non-loopback topology entries. It never connects to a
validator host and writes only beneath the caller-supplied clone and work roots.
Snapshot import, verification, and transactional rebuild are separate receipt
steps because they are intentionally completed before any service starts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--clone-root", required=True, type=Path)
    parser.add_argument("--key-root", required=True, type=Path)
    parser.add_argument("--topology", required=True, type=Path)
    parser.add_argument("--work-root", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--signed-transfer-file", type=Path)
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def wait_ready(path: Path, process: subprocess.Popen[bytes], timeout: float = 180.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.is_file():
            return
        if process.poll() is not None:
            raise RuntimeError(f"service exited before readiness file {path}")
        time.sleep(0.25)
    raise RuntimeError(f"readiness file {path} did not appear")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def stage_local_keys(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Install each validator's isolated signer into its disposable clone."""
    staged: list[dict[str, Any]] = []
    for index in range(6):
        validator_id = f"validator-{index}"
        data_dir = args.clone_root / validator_id
        source_key = args.key_root / validator_id / "validator_keys.json"
        if not data_dir.is_dir() or not source_key.is_file():
            raise RuntimeError(f"missing local clone or signer for {validator_id}")
        subprocess.run(
            [
                str(args.binary),
                "validator-key-stage",
                "--data-dir",
                str(data_dir),
                "--source-key-file",
                str(source_key),
                "--validator-id",
                validator_id,
                "--source-validator-id",
                validator_id,
                "--replace",
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        staged_path = data_dir / "validator_keys.json"
        if not staged_path.is_file():
            raise RuntimeError(f"key staging produced no local signer for {validator_id}")
        staged.append({"validator_id": validator_id, "scope": "disposable_clone_only"})
    return staged


class Services:
    def __init__(self, args: argparse.Namespace, topology: dict[str, Any]) -> None:
        self.args = args
        self.topology = topology
        self.processes: dict[tuple[int, str], subprocess.Popen[bytes]] = {}
        self.orders: dict[str, str] = {}
        self.args.work_root.mkdir(parents=True, exist_ok=True)

    def spawn(self, index: int, service: str, command: list[str], env: dict[str, str]) -> None:
        log_dir = self.args.work_root / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        stdout = (log_dir / f"validator-{index}.{service}.stdout.log").open("ab")
        stderr = (log_dir / f"validator-{index}.{service}.stderr.log").open("ab")
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
            env=env,
        )
        self.processes[(index, service)] = process
        time.sleep(0.5)
        if process.poll() is not None:
            raise RuntimeError(f"validator-{index} {service} exited rc={process.returncode}")

    def start_one(self, index: int, order: tuple[str, str]) -> None:
        peer = self.topology["peers"][index]
        data = self.args.clone_root / f"validator-{index}"
        key = self.args.key_root / f"validator-{index}" / "validator_keys.json"
        runtime = self.args.work_root / "runtime" / f"validator-{index}"
        votes = runtime / "votes"
        spool = runtime / "rpc-spool"
        artifacts = runtime / "finality-artifacts"
        for path in (runtime, votes, spool, artifacts):
            path.mkdir(parents=True, exist_ok=True)
        transport_ready = runtime / "transport.ready.json"
        rpc_ready = runtime / "rpc.ready.json"
        transport_ready.unlink(missing_ok=True)
        rpc_ready.unlink(missing_ok=True)
        base_env = dict(os.environ)
        transport_env = {
            **base_env,
            "POSTFIAT_PREWARM_SHIELDED_VERIFIER": "1",
            "POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER": "1",
            "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER": "1",
            "POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE": str(transport_ready),
        }
        commands = {
            "transport": [
                str(self.args.binary),
                "transport-validator-serve",
                "--unsafe-devnet-file-signer",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                str(data),
                "--topology",
                str(self.args.topology),
                "--key-file",
                str(key),
                "--vote-dir",
                str(votes),
                "--bind-host",
                "127.0.0.1",
                "--max-connections",
                "10000",
                "--timeout-ms",
                "900000",
                "--event-log",
                str(self.args.work_root / "logs" / f"validator-{index}.transport.ndjson"),
            ],
            "rpc": [
                str(self.args.binary),
                "rpc-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                str(data),
                "--spool-dir",
                str(spool),
                "--ready-file",
                str(rpc_ready),
                "--bind-host",
                "127.0.0.1",
                "--port",
                str(peer["rpc_port"]),
                "--max-requests",
                "10000",
                "--timeout-ms",
                "30000",
                "--child-timeout-ms",
                "30000",
                "--event-log",
                str(self.args.work_root / "logs" / f"validator-{index}.rpc.ndjson"),
                "--allow-mempool-submit-finality",
                "--finality-topology",
                str(self.args.topology),
                "--finality-key-file",
                str(key),
                "--finality-proposal-key-file",
                str(key),
                "--finality-artifact-root",
                str(artifacts),
                "--finality-timeout-ms",
                "30000",
                "--finality-send-retries",
                "16",
                "--finality-retry-backoff-ms",
                "250",
                "--finality-quorum-early-full-propagation",
                "--keep-alive",
            ],
        }
        environments = {"transport": transport_env, "rpc": base_env}
        ready_files = {"transport": transport_ready, "rpc": rpc_ready}
        for service in order:
            self.spawn(index, service, commands[service], environments[service])
            wait_ready(ready_files[service], self.processes[(index, service)])
        self.orders[f"validator-{index}"] = f"{order[0]}-first"

    def start_all(self) -> None:
        for index in range(6):
            order = ("transport", "rpc") if index % 2 == 0 else ("rpc", "transport")
            self.start_one(index, order)

    def stop_all(self) -> None:
        for process in list(self.processes.values()):
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if all(process.poll() is not None for process in self.processes.values()):
                break
            time.sleep(0.25)
        for process in list(self.processes.values()):
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=10)
        self.processes.clear()


def compact_status_data(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "block_height": int(status["block_height"]),
        "block_tip_hash": status["block_tip_hash"],
        "state_root": status["state_root"],
        "mempool_pending": int(status["mempool_pending"]),
        "build_git_revision": status["build_git_revision"],
        "storage_backend": status["storage"]["backend"],
        "transactional_active": status["storage"]["transactional_active"],
    }


def compact_status(client: Any) -> dict[str, Any]:
    return compact_status_data(client.status())


def statuses(clients: list[Any]) -> list[dict[str, Any]]:
    result = [compact_status(client) for client in clients]
    identities = {
        (row["block_height"], row["block_tip_hash"], row["state_root"])
        for row in result
    }
    if len(identities) != 1:
        raise RuntimeError(f"local clone roots diverged: {result}")
    return result


def disk_statuses(binary: Path, clone_root: Path) -> list[dict[str, Any]]:
    result = []
    for index in range(6):
        raw = subprocess.check_output(
            [
                str(binary),
                "status",
                "--data-dir",
                str(clone_root / f"validator-{index}"),
            ],
            text=True,
        )
        result.append(compact_status_data(json.loads(raw)))
    identities = {
        (row["block_height"], row["block_tip_hash"], row["state_root"])
        for row in result
    }
    if len(identities) != 1:
        raise RuntimeError(f"local clone roots diverged on disk: {result}")
    return result


def main() -> int:
    args = parse_args()
    args.binary = args.binary.resolve()
    args.clone_root = args.clone_root.resolve()
    args.key_root = args.key_root.resolve()
    args.topology = args.topology.resolve()
    args.work_root = args.work_root.resolve()
    args.receipt = args.receipt.resolve()
    if args.signed_transfer_file is not None:
        args.signed_transfer_file = args.signed_transfer_file.resolve()
    topology = load_json(args.topology)
    if len(topology.get("peers", [])) != 6:
        raise RuntimeError("gate requires exactly six topology entries")
    if any(peer.get("host") not in {"127.0.0.1", "::1"} for peer in topology["peers"]):
        raise RuntimeError("gate refuses non-loopback topology entries")
    for peer in topology["peers"]:
        for name in ("p2p_port", "rpc_port"):
            with socket.socket() as sock:
                try:
                    sock.bind(("127.0.0.1", int(peer[name])))
                except OSError as error:
                    raise RuntimeError(f"loopback port {peer[name]} is already in use") from error

    sys.path.insert(0, str(Path.cwd() / "python"))
    from postfiat_rpc.client import PostFiatRpcClient

    clients = [
        PostFiatRpcClient(f"127.0.0.1:{peer['rpc_port']}", timeout_seconds=180)
        for peer in topology["peers"]
    ]
    services = Services(args, topology)
    receipt: dict[str, Any] = {
        "schema": "postfiat-signing-fix-local-service-gate-v1",
        "scope": "local_clones_only",
        "network_boundary": "loopback_only",
        "source_commit": args.source_commit,
        "binary_sha256": sha256(args.binary),
    }
    try:
        receipt["key_staging"] = stage_local_keys(args)
        print("KEY STAGING PASS: six isolated signers installed in disposable clones")
        services.start_all()
        print("START PASS: six loopback clone services ready; both start orders exercised")
        initial = statuses(clients)
        print(
            "INITIAL PASS: "
            f"height={initial[0]['block_height']} root={initial[0]['state_root']}"
        )
        receipt["start_orders"] = services.orders
        receipt["initial_statuses"] = initial
        before_height = initial[0]["block_height"]

        proposer_raw = subprocess.check_output(
            [
                str(args.binary),
                "block-proposer",
                "--data-dir",
                str(args.clone_root / "validator-0"),
                "--height",
                str(before_height + 1),
                "--view",
                "0",
            ],
            text=True,
        )
        proposer = int(json.loads(proposer_raw)["proposer"].rsplit("-", 1)[1])
        faucet = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
        recipient = "pfde0ba09f38b1748f8d77709715e1095a0ff74d0f"
        signed_path = args.work_root / "round.signed.json"
        if args.signed_transfer_file is None:
            quote = clients[0].transfer_fee_quote_response(
                faucet,
                recipient,
                125000,
                request_id="signing-fix-gate-quote",
            )
            quote_path = args.work_root / "round.quote.json"
            quote_path.write_text(json.dumps(quote["result"]), encoding="utf-8")
            signed = subprocess.check_output(
                [
                    str(args.binary),
                    "wallet-sign-transfer",
                    "--key-file",
                    str(args.key_root / "validator-0" / "faucet_key.json"),
                    "--quote-file",
                    str(quote_path),
                ],
                text=True,
            )
        else:
            signed = args.signed_transfer_file.read_text(encoding="utf-8")
            receipt["reused_signed_transfer_sha256"] = sha256(args.signed_transfer_file)
        signed_path.write_text(signed, encoding="utf-8")
        finality = clients[proposer].mempool_submit_signed_transfer_finality(
            json.loads(signed), request_id="signing-fix-gate-finality"
        )
        finality_path = args.work_root / "round.finality.json"
        finality_path.write_text(json.dumps(finality, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        if finality.get("finality", {}).get("confirmed") is not True:
            raise RuntimeError("local finality submission was not confirmed")
        deadline = time.monotonic() + 180
        final = None
        while time.monotonic() < deadline:
            try:
                # The long-running RPC process intentionally caches its health
                # view. Transport writes are checked directly on disk here and
                # then checked through RPC after the service restart below.
                candidate = disk_statuses(args.binary, args.clone_root)
                if candidate[0]["block_height"] == before_height + 1:
                    final = candidate
                    break
            except Exception:
                pass
            time.sleep(2)
        if final is None:
            raise RuntimeError("six local clones did not converge after the finality round")
        receipt["finality_disk_statuses"] = final
        print(
            "FINALITY PASS: "
            f"height={final[0]['block_height']} root={final[0]['state_root']}"
        )

        final_identity = (
            final[0]["block_height"],
            final[0]["block_tip_hash"],
            final[0]["state_root"],
        )
        services.stop_all()
        services.start_all()
        restarted = statuses(clients)
        restart_identity = (
            restarted[0]["block_height"],
            restarted[0]["block_tip_hash"],
            restarted[0]["state_root"],
        )
        if restart_identity != final_identity:
            raise RuntimeError("restart changed the local clone identity")
        receipt["restart_rpc_statuses"] = restarted
        receipt["result"] = "PASS"
        print(
            "RESTART PASS: "
            f"height={restarted[0]['block_height']} root={restarted[0]['state_root']}"
        )
        return_code = 0
    except Exception as error:
        receipt["result"] = "FAIL"
        receipt["error"] = str(error)
        print(f"GATE FAIL: {error}", file=sys.stderr)
        return_code = 1
    finally:
        services.stop_all()
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return return_code


if __name__ == "__main__":
    raise SystemExit(main())
