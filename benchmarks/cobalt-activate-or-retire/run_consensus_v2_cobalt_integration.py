#!/usr/bin/env python3
"""Run paired Consensus v2 finality with and without active Cobalt simulation."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import threading
import time
from pathlib import Path
from typing import Any

VALIDATORS = 6
ACTIVATION_HEIGHT = 2
CHAIN_ID = "postfiat-cobalt-consensus-integration"
READY_TIMEOUT_SECONDS = 90


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    path.chmod(mode)


def fleet_converged(statuses: list[dict[str, Any]]) -> bool:
    """Return whether all six validators expose one durable final state."""
    return (
        len(statuses) == VALIDATORS
        and len(
            {
                (
                    row["block_height"],
                    row["block_tip_hash"],
                    row["state_root"],
                )
                for row in statuses
            }
        )
        == 1
    )


def benchmark_workload_fingerprint(report: dict[str, Any]) -> dict[str, Any]:
    """Project a lane report onto signed-message-independent workload semantics.

    ML-DSA signatures are randomized. Transaction IDs, certificates, block hashes,
    and roots therefore differ between independent executions of the same unsigned
    workload and must not be used as a cross-lane fork oracle.
    """
    config = report.get("config", {})
    iterations = report.get("iterations", [])
    return {
        "config": {
            key: config.get(key)
            for key in (
                "validators",
                "rounds",
                "vote_policy",
                "wallet_address",
                "recipient",
                "amount",
            )
        },
        "iterations": [
            {
                key: row.get(key)
                for key in (
                    "iteration",
                    "source_node",
                    "block_height",
                    "vote_policy",
                    "validators",
                    "quorum",
                    "vote_count",
                    "receipt_accepted",
                    "finality_confirmed",
                    "round_ok",
                    "all_vote_requests_verified",
                    "all_sends_verified",
                )
            }
            for row in iterations
        ],
    }


def run(
    args: list[str],
    *,
    stdout_path: Path | None = None,
    stderr_path: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    completed = subprocess.run(
        args,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    if stdout_path is not None:
        stdout_path.parent.mkdir(parents=True, exist_ok=True)
        stdout_path.write_bytes(completed.stdout)
    if stderr_path is not None:
        stderr_path.parent.mkdir(parents=True, exist_ok=True)
        stderr_path.write_bytes(completed.stderr)
    if completed.returncode != 0:
        command = " ".join(args[:2])
        raise RuntimeError(
            f"{command} failed with {completed.returncode}; "
            f"stdout={stdout_path} stderr={stderr_path}"
        )
    return completed


def find_ports() -> tuple[int, int]:
    for base in range(31_000, 54_000, 32):
        listeners: list[socket.socket] = []
        try:
            for port in range(base, base + (VALIDATORS * 2)):
                listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                listener.bind(("127.0.0.1", port))
                listeners.append(listener)
            return base, base + 1
        except OSError:
            pass
        finally:
            for listener in listeners:
                listener.close()
    raise RuntimeError("no contiguous loopback port ranges available")


def wait_ready(path: Path, process: subprocess.Popen[bytes]) -> None:
    deadline = time.monotonic() + READY_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if path.is_file() and path.stat().st_size > 0:
            return
        return_code = process.poll()
        if return_code is not None:
            raise RuntimeError(
                f"validator service exited with {return_code} before {path.name}"
            )
        time.sleep(0.025)
    raise RuntimeError(f"timed out waiting for {path}")


def stop_validators(
    processes: list[subprocess.Popen[bytes]],
    handles: list[tuple[Any, Any]],
) -> None:
    for process in processes:
        if process.poll() is None:
            process.terminate()
    deadline = time.monotonic() + 5
    for process in processes:
        if process.poll() is None:
            remaining = max(0.0, deadline - time.monotonic())
            try:
                process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    for stdout_handle, stderr_handle in handles:
        stdout_handle.close()
        stderr_handle.close()
    processes.clear()
    handles.clear()


def prepare_nodes(
    node_bin: Path,
    nodes: Path,
    snapshot: Path,
    seed: Path,
    logs: Path,
    lane: str,
) -> None:
    if nodes.exists():
        shutil.rmtree(nodes)
    nodes.mkdir()
    for index in range(VALIDATORS):
        node_id = f"validator-{index}"
        data_dir = nodes / node_id
        run(
            [
                str(node_bin),
                "snapshot-import",
                "--data-dir",
                str(data_dir),
                "--snapshot-dir",
                str(snapshot),
                "--node-id",
                node_id,
            ],
            stdout_path=logs / f"{lane}.{node_id}.snapshot-import.json",
            stderr_path=logs / f"{lane}.{node_id}.snapshot-import.stderr",
        )
        run(
            [
                str(node_bin),
                "validator-key-stage",
                "--data-dir",
                str(data_dir),
                "--source-key-file",
                str(seed / "validator_keys.json"),
                "--source-validator-id",
                node_id,
                "--validator-id",
                node_id,
            ],
            stdout_path=logs / f"{lane}.{node_id}.key-stage.json",
            stderr_path=logs / f"{lane}.{node_id}.key-stage.stderr",
        )


def fleet_status(node_bin: Path, nodes: Path) -> list[dict[str, Any]]:
    statuses: list[dict[str, Any]] = []
    for index in range(VALIDATORS):
        node_id = f"validator-{index}"
        completed = run(
            [str(node_bin), "status", "--data-dir", str(nodes / node_id)]
        )
        status = json.loads(completed.stdout)
        statuses.append(
            {
                "node_id": status["node_id"],
                "block_height": status["block_height"],
                "block_tip_hash": status["block_tip_hash"],
                "state_root": status["state_root"],
            }
        )
    return statuses


def start_validator(
    node_bin: Path,
    nodes: Path,
    topology: Path,
    root: Path,
    logs: Path,
    lane: str,
    index: int,
    restart: int = 0,
    timeout_ms: int = 90_000,
) -> tuple[subprocess.Popen[bytes], tuple[Any, Any]]:
    node_id = f"validator-{index}"
    data_dir = nodes / node_id
    ready = logs / f"{lane}.{node_id}.ready.json"
    ready.unlink(missing_ok=True)
    suffix = "" if restart == 0 else f".restart-{restart:03}"
    stdout_handle = (logs / f"{lane}.{node_id}{suffix}.stdout.log").open("wb")
    stderr_handle = (logs / f"{lane}.{node_id}{suffix}.stderr.log").open("wb")
    env = os.environ.copy()
    env["POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE"] = str(ready)
    env["POSTFIAT_PREWARM_SHIELDED_VERIFIER"] = "1"
    env["POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER"] = "1"
    env["POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER"] = "1"
    process = subprocess.Popen(
        [
            str(node_bin),
            "transport-validator-serve",
            "--unsafe-devnet-file-signer",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            str(data_dir),
            "--topology",
            str(topology),
            "--key-file",
            str(data_dir / "validator_keys.json"),
            "--vote-dir",
            str(root / "votes" / lane / node_id),
            "--max-connections",
            "10000",
            "--timeout-ms",
            str(timeout_ms),
        ],
        stdout=stdout_handle,
        stderr=stderr_handle,
        env=env,
    )
    try:
        wait_ready(ready, process)
    except Exception:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        stdout_handle.close()
        stderr_handle.close()
        raise
    return process, (stdout_handle, stderr_handle)


def start_validators(
    node_bin: Path,
    nodes: Path,
    topology: Path,
    root: Path,
    logs: Path,
    lane: str,
) -> tuple[list[subprocess.Popen[bytes]], list[tuple[Any, Any]]]:
    processes: list[subprocess.Popen[bytes]] = []
    handles: list[tuple[Any, Any]] = []
    try:
        for index in range(VALIDATORS):
            process, process_handles = start_validator(
                node_bin, nodes, topology, root, logs, lane, index
            )
            processes.append(process)
            handles.append(process_handles)
        return processes, handles
    except Exception:
        stop_validators(processes, handles)
        raise


def split_seed_validator_keys(seed: Path) -> None:
    combined = read_json(seed / "validator_keys.json")
    records = combined.get("validators")
    if not isinstance(records, list) or len(records) != VALIDATORS:
        raise ValueError("seed validator key file does not contain six validators")
    for record in records:
        node_id = record.get("node_id")
        if not isinstance(node_id, str):
            raise ValueError("validator key record lacks node_id")
        write_json(seed / f"{node_id}.validator_keys.json", {"validators": [record]}, 0o600)


def benchmark_command(
    node_bin: Path,
    root: Path,
    nodes: Path,
    topology: Path,
    wallet_key_file: Path,
    wallet_address: str,
    recipient: str,
    rounds: int,
    lane: str,
    timeout_ms: int = 90_000,
) -> list[str]:
    lane_dir = root / lane
    return [
        str(node_bin),
        "tx-latency-benchmark",
        "--base-dir",
        str(nodes),
        "--topology",
        str(topology),
        "--wallet-key-file",
        str(wallet_key_file),
        "--wallet-address",
        wallet_address,
        "--recipient",
        recipient,
        "--amount",
        "10",
        "--validators",
        str(VALIDATORS),
        "--rounds",
        str(rounds),
        "--vote-policy",
        "full",
        "--artifact-root",
        str(lane_dir / "artifacts"),
        "--report",
        str(lane_dir / "report.json"),
        "--iterations-file",
        str(lane_dir / "iterations.jsonl"),
        "--build-mode",
        "release",
        "--timeout-ms",
        str(timeout_ms),
        "--send-retries",
        "16",
        "--retry-backoff-ms",
        "100",
    ]


def interval_coverage(
    start_ns: int, end_ns: int, intervals: list[tuple[int, int]]
) -> float:
    clipped = sorted(
        (max(start_ns, left), min(end_ns, right))
        for left, right in intervals
        if right > start_ns and left < end_ns
    )
    if end_ns <= start_ns:
        return 0.0
    covered = 0
    cursor_left: int | None = None
    cursor_right: int | None = None
    for left, right in clipped:
        if cursor_left is None:
            cursor_left, cursor_right = left, right
        elif left <= cursor_right:
            cursor_right = max(cursor_right, right)
        else:
            covered += cursor_right - cursor_left
            cursor_left, cursor_right = left, right
    if cursor_left is not None and cursor_right is not None:
        covered += cursor_right - cursor_left
    return covered / (end_ns - start_ns)


def nested_percentile(
    report: dict[str, Any], metric: str, percentile: str
) -> float:
    value = report.get("latency", {}).get(metric, {}).get(percentile)
    if not isinstance(value, (int, float)):
        raise ValueError(f"missing {metric}.{percentile}")
    return float(value)


def nested_p95(report: dict[str, Any], metric: str) -> float:
    return nested_percentile(report, metric, "p95_ms")


def directory_bytes(root: Path) -> int:
    total = 0
    for path in root.rglob("*"):
        try:
            if path.is_file() and not path.is_symlink():
                total += path.stat().st_size
        except FileNotFoundError:
            continue
    return total


def network_bytes() -> dict[str, int]:
    received = 0
    transmitted = 0
    for line in Path("/proc/net/dev").read_text(encoding="ascii").splitlines()[2:]:
        _, values = line.split(":", 1)
        fields = values.split()
        received += int(fields[0])
        transmitted += int(fields[8])
    return {"received": received, "transmitted": transmitted}


def host_cpu_ticks() -> int:
    fields = Path("/proc/stat").read_text(encoding="ascii").splitlines()[0].split()
    if not fields or fields[0] != "cpu":
        raise ValueError("/proc/stat did not contain aggregate CPU accounting")
    return sum(int(value) for value in fields[1:])


def host_memory() -> dict[str, int]:
    values: dict[str, int] = {}
    for line in Path("/proc/meminfo").read_text(encoding="ascii").splitlines():
        name, raw = line.split(":", 1)
        first = raw.split()[0]
        values[name] = int(first)
    return {
        "total_kib": values["MemTotal"],
        "available_kib": values["MemAvailable"],
    }


def process_metrics(pid: int) -> dict[str, int] | None:
    root = Path("/proc") / str(pid)
    try:
        stat_fields = (root / "stat").read_text(encoding="ascii").split()
        status = (root / "status").read_text(encoding="ascii").splitlines()
        io_rows = (root / "io").read_text(encoding="ascii").splitlines()
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None
    status_values = {
        row.split(":", 1)[0]: row.split(":", 1)[1].strip().split()[0]
        for row in status
        if ":" in row and row.split(":", 1)[1].strip()
    }
    io_values = {
        row.split(":", 1)[0]: row.split(":", 1)[1].strip()
        for row in io_rows
        if ":" in row
    }
    return {
        "cpu_ticks": int(stat_fields[13]) + int(stat_fields[14]),
        "rss_kib": int(status_values.get("VmRSS", "0")),
        "read_bytes": int(io_values.get("read_bytes", "0")),
        "write_bytes": int(io_values.get("write_bytes", "0")),
    }


def resource_sample(
    pids: list[int], nodes: Path, *, include_disk: bool
) -> dict[str, Any]:
    processes = {
        str(pid): metrics
        for pid in sorted(set(pids))
        if (metrics := process_metrics(pid)) is not None
    }
    return {
        "monotonic_ns": time.monotonic_ns(),
        "host_cpu_ticks": host_cpu_ticks(),
        "host_memory": host_memory(),
        "network": network_bytes(),
        "node_disk_bytes": directory_bytes(nodes) if include_disk else None,
        "processes": processes,
    }


def resource_summary(samples: list[dict[str, Any]]) -> dict[str, Any]:
    if len(samples) < 2:
        raise ValueError("resource accounting requires at least two samples")
    per_pid: dict[str, list[dict[str, int]]] = {}
    for sample in samples:
        for pid, values in sample["processes"].items():
            per_pid.setdefault(pid, []).append(values)
    process_cpu_ticks = 0
    process_read_bytes = 0
    process_write_bytes = 0
    for values in per_pid.values():
        process_cpu_ticks += max(row["cpu_ticks"] for row in values) - min(
            row["cpu_ticks"] for row in values
        )
        process_read_bytes += max(row["read_bytes"] for row in values) - min(
            row["read_bytes"] for row in values
        )
        process_write_bytes += max(row["write_bytes"] for row in values) - min(
            row["write_bytes"] for row in values
        )
    first = samples[0]
    last = samples[-1]
    return {
        "sample_count": len(samples),
        "duration_ms": (last["monotonic_ns"] - first["monotonic_ns"]) / 1_000_000,
        "host_cpu_ticks": last["host_cpu_ticks"] - first["host_cpu_ticks"],
        "validator_cpu_ticks": process_cpu_ticks,
        "validator_peak_rss_kib": max(
            sum(row["rss_kib"] for row in sample["processes"].values())
            for sample in samples
        ),
        "host_min_available_memory_kib": min(
            sample["host_memory"]["available_kib"] for sample in samples
        ),
        "host_total_memory_kib": first["host_memory"]["total_kib"],
        "network_received_bytes": last["network"]["received"]
        - first["network"]["received"],
        "network_transmitted_bytes": last["network"]["transmitted"]
        - first["network"]["transmitted"],
        "node_disk_delta_bytes": last["node_disk_bytes"] - first["node_disk_bytes"],
        "validator_read_bytes": process_read_bytes,
        "validator_write_bytes": process_write_bytes,
        "observed_pids": sorted(per_pid),
    }


def start_resource_sampler(
    stop_event: threading.Event,
    pid_provider: Any,
    nodes: Path,
    samples: list[dict[str, Any]],
) -> threading.Thread:
    ready = threading.Event()
    startup_errors: list[BaseException] = []

    def sample_loop() -> None:
        try:
            samples.append(resource_sample(pid_provider(), nodes, include_disk=True))
        except BaseException as error:
            startup_errors.append(error)
            ready.set()
            return
        ready.set()
        while not stop_event.wait(0.1):
            samples.append(resource_sample(pid_provider(), nodes, include_disk=False))
        samples.append(resource_sample(pid_provider(), nodes, include_disk=True))

    thread = threading.Thread(target=sample_loop, name="resource-sampler", daemon=False)
    thread.start()
    ready.wait()
    if startup_errors:
        thread.join()
        raise RuntimeError("resource sampler failed before measurement") from startup_errors[0]
    return thread


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--cobalt-bin", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--rounds", type=int, default=50)
    parser.add_argument(
        "--e4-stress",
        action="store_true",
        help="enforce the locked 500+500 adversarial finality campaign",
    )
    parser.add_argument(
        "--cobalt-cpu-quota-percent",
        type=int,
        default=25,
        help="quota for the one Cobalt simulation process; production unit is 25%%",
    )
    args = parser.parse_args()

    node_bin = args.node_bin.resolve()
    cobalt_bin = args.cobalt_bin.resolve()
    root = args.output_dir.resolve()
    if not node_bin.is_file() or not cobalt_bin.is_file():
        raise ValueError("both binaries must be regular files")
    minimum_rounds = 500 if args.e4_stress else 20
    if args.rounds < minimum_rounds:
        raise ValueError(
            f"--rounds must be at least {minimum_rounds} for this campaign"
        )
    if args.cobalt_cpu_quota_percent <= 0:
        raise ValueError("--cobalt-cpu-quota-percent must be positive")
    if args.e4_stress and args.cobalt_cpu_quota_percent != 25:
        raise ValueError("E4 requires the production 25% Cobalt CPU quota")
    if shutil.which("systemd-run") is None:
        raise ValueError("systemd-run is required to enforce the Cobalt CPU quota")
    if root.exists():
        raise ValueError(f"refusing to overwrite output directory: {root}")
    root.mkdir(parents=True)
    private = root / "private"
    logs = root / "logs"
    seed = private / "seed"
    nodes = root / "nodes"
    private.mkdir(mode=0o700)
    logs.mkdir()

    base_port, rpc_base_port = find_ports()
    topology = root / "topology.json"
    validator_processes: list[subprocess.Popen[bytes]] = []
    validator_logs: list[tuple[Any, Any]] = []
    validator_lock = threading.Lock()
    cobalt_stop = threading.Event()
    cobalt_intervals: list[tuple[int, int]] = []
    cobalt_runs: list[dict[str, Any]] = []
    cobalt_error: list[str] = []
    crash_stop = threading.Event()
    crash_started = threading.Event()
    crash_receipts: list[dict[str, Any]] = []
    crash_error: list[str] = []
    baseline_resource_samples: list[dict[str, Any]] = []
    attack_resource_samples: list[dict[str, Any]] = []
    baseline_resource_stop = threading.Event()
    attack_resource_stop = threading.Event()
    baseline_resource_thread: threading.Thread | None = None
    attack_resource_thread: threading.Thread | None = None
    crash_thread: threading.Thread | None = None
    cobalt_thread: threading.Thread | None = None
    attack_lane = "attack" if args.e4_stress else "integration"

    def current_validator_pids() -> list[int]:
        with validator_lock:
            return [
                process.pid
                for process in validator_processes
                if process.poll() is None
            ]

    try:
        run(
            [
                str(node_bin),
                "init-consensus-v2",
                "--data-dir",
                str(seed),
                "--chain-id",
                CHAIN_ID,
                "--node-id",
                "validator-0",
                "--validators",
                str(VALIDATORS),
                "--activation-height",
                str(ACTIVATION_HEIGHT),
            ],
            stdout_path=logs / "init.json",
            stderr_path=logs / "init.stderr",
        )
        split_seed_validator_keys(seed)

        master_seed = private / "wallet-master-seed.hex"
        master_seed.write_text("01" * 32 + "\n", encoding="ascii")
        master_seed.chmod(0o600)
        wallet_key = private / "wallet.key.json"
        wallet_backup = private / "wallet.backup.json"
        wallet_report_path = logs / "wallet-report.json"
        run(
            [
                str(node_bin),
                "wallet-keygen",
                "--chain-id",
                CHAIN_ID,
                "--master-seed-hex-file",
                str(master_seed),
                "--account-index",
                "0",
                "--key-file",
                str(wallet_key),
                "--backup-file",
                str(wallet_backup),
            ],
            stdout_path=wallet_report_path,
            stderr_path=logs / "wallet.stderr",
        )
        recipient_key = private / "recipient.key.json"
        recipient_backup = private / "recipient.backup.json"
        recipient_report_path = logs / "recipient-report.json"
        run(
            [
                str(node_bin),
                "wallet-keygen",
                "--chain-id",
                CHAIN_ID,
                "--master-seed-hex-file",
                str(master_seed),
                "--account-index",
                "1",
                "--key-file",
                str(recipient_key),
                "--backup-file",
                str(recipient_backup),
            ],
            stdout_path=recipient_report_path,
            stderr_path=logs / "recipient.stderr",
        )
        wallet_report = read_json(wallet_report_path)
        recipient_report = read_json(recipient_report_path)
        wallet_address = str(wallet_report["address"])
        recipient = str(recipient_report["address"])

        fund_batch = private / "fund.batch.json"
        fund_certificate = private / "fund.certificate.json"
        run(
            [
                str(node_bin),
                "batch-transfer",
                "--data-dir",
                str(seed),
                "--to",
                wallet_address,
                "--amount",
                "100000000",
                "--batch-file",
                str(fund_batch),
            ],
            stdout_path=logs / "fund-create.json",
            stderr_path=logs / "fund-create.stderr",
        )
        run(
            [
                str(node_bin),
                "certify-batch",
                "--data-dir",
                str(seed),
                "--batch-kind",
                "transparent",
                "--batch-file",
                str(fund_batch),
                "--validator-key-dir",
                str(seed),
                "--proposal-file",
                str(private / "fund.proposal.json"),
                "--vote-dir",
                str(private / "fund.votes"),
                "--certificate-file",
                str(fund_certificate),
                "--height",
                "1",
            ],
            stdout_path=logs / "fund-certify.json",
            stderr_path=logs / "fund-certify.stderr",
        )
        fund_apply_path = logs / "fund-apply.json"
        run(
            [
                str(node_bin),
                "apply-batch",
                "--data-dir",
                str(seed),
                "--batch-file",
                str(fund_batch),
                "--certificate-file",
                str(fund_certificate),
            ],
            stdout_path=fund_apply_path,
            stderr_path=logs / "fund-apply.stderr",
        )
        fund_receipts = json.loads(fund_apply_path.read_bytes())
        if (
            not isinstance(fund_receipts, list)
            or not fund_receipts
            or not all(
                isinstance(receipt, dict) and receipt.get("accepted") is True
                for receipt in fund_receipts
            )
        ):
            raise RuntimeError("benchmark wallet funding receipt was not accepted")

        snapshot = private / "seed.snapshot"
        run(
            [
                str(node_bin),
                "snapshot-export",
                "--data-dir",
                str(seed),
                "--snapshot-dir",
                str(snapshot),
            ],
            stdout_path=logs / "snapshot-export.json",
            stderr_path=logs / "snapshot-export.stderr",
        )
        prepare_nodes(node_bin, nodes, snapshot, seed, logs, "baseline")

        run(
            [
                str(node_bin),
                "topology-consensus-v2",
                "--chain-id",
                CHAIN_ID,
                "--validators",
                str(VALIDATORS),
                "--base-port",
                str(base_port),
                "--rpc-base-port",
                str(rpc_base_port),
                "--activation-height",
                str(ACTIVATION_HEIGHT),
                "--output",
                str(topology),
            ],
            stdout_path=logs / "topology.stdout.json",
            stderr_path=logs / "topology.stderr",
        )

        baseline_initial_status = fleet_status(node_bin, nodes)
        validator_processes, validator_logs = start_validators(
            node_bin, nodes, topology, root, logs, "baseline"
        )
        baseline_pids = [process.pid for process in validator_processes]
        baseline_command = benchmark_command(
            node_bin,
            root,
            nodes,
            topology,
            wallet_key,
            wallet_address,
            recipient,
            args.rounds,
            "baseline",
        )
        baseline_resource_thread = start_resource_sampler(
            baseline_resource_stop,
            current_validator_pids,
            nodes,
            baseline_resource_samples,
        )
        baseline_start_ns = time.monotonic_ns()
        try:
            run(
                baseline_command,
                stdout_path=logs / "baseline.stdout.json",
                stderr_path=logs / "baseline.stderr",
            )
        finally:
            baseline_end_ns = time.monotonic_ns()
            baseline_resource_stop.set()
            baseline_resource_thread.join()
        baseline_fleet_alive = all(
            process.poll() is None for process in validator_processes
        )
        baseline_final_status = fleet_status(node_bin, nodes)
        stop_validators(validator_processes, validator_logs)
        prepare_nodes(node_bin, nodes, snapshot, seed, logs, attack_lane)
        integration_initial_status = fleet_status(node_bin, nodes)
        matched_initial_state = baseline_initial_status == integration_initial_status
        validator_processes, validator_logs = start_validators(
            node_bin, nodes, topology, root, logs, attack_lane
        )
        integration_pids = [process.pid for process in validator_processes]
        attack_resource_thread = start_resource_sampler(
            attack_resource_stop,
            current_validator_pids,
            nodes,
            attack_resource_samples,
        )

        def cobalt_loop() -> None:
            index = 0
            while not cobalt_stop.is_set():
                index += 1
                run_dir = private / "cobalt" / f"run-{index:03}"
                work_dir = run_dir / "work"
                report_path = run_dir / "report.json"
                stdout_path = logs / f"cobalt-{index:03}.stdout.log"
                stderr_path = logs / f"cobalt-{index:03}.stderr.log"
                work_dir.mkdir(parents=True)
                started = time.monotonic_ns()
                unit = f"postfiat-cobalt-integration-{os.getpid()}-{index}"
                completed = subprocess.run(
                    [
                        "systemd-run",
                        "--user",
                        "--pipe",
                        "--wait",
                        "--collect",
                        "--quiet",
                        "-p",
                        f"CPUQuota={args.cobalt_cpu_quota_percent}%",
                        "--unit",
                        unit,
                        str(cobalt_bin),
                        "--work-dir",
                        str(work_dir),
                        "--output",
                        str(report_path),
                    ],
                    check=False,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
                ended = time.monotonic_ns()
                stdout_path.write_bytes(completed.stdout)
                stderr_path.write_bytes(completed.stderr)
                cobalt_intervals.append((started, ended))
                if completed.returncode != 0:
                    cobalt_error.append(
                        f"Cobalt run {index} exited {completed.returncode}"
                    )
                    return
                report = read_json(report_path)
                cobalt_runs.append(
                    {
                        "run": index,
                        "status": report.get("status"),
                        "report_sha256": digest(report_path),
                        "rounds": report.get("round_count"),
                        "governance_storm": report.get("governance_storm"),
                        "e4_stress_receipt": report.get("e4_stress_receipt"),
                        "governance_p95_validation_wall_micros": report.get(
                            "p95_round_validation_wall_micros"
                        ),
                        "started_monotonic_ns": started,
                        "ended_monotonic_ns": ended,
                    }
                )

        def crash_loop() -> None:
            index = VALIDATORS - 1
            try:
                for restart in range(1, 13):
                    if crash_stop.wait(0.2):
                        return
                    with validator_lock:
                        process = validator_processes[index]
                    old_pid = process.pid
                    stopped_ns = time.monotonic_ns()
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait()
                    time.sleep(0.1)
                    replacement, replacement_handles = start_validator(
                        node_bin,
                        nodes,
                        topology,
                        root,
                        logs,
                        attack_lane,
                        index,
                        restart,
                    )
                    with validator_lock:
                        validator_processes[index] = replacement
                        validator_logs.append(replacement_handles)
                    restarted_ns = time.monotonic_ns()
                    crash_receipts.append(
                        {
                            "node_id": f"validator-{index}",
                            "restart": restart,
                            "old_pid": old_pid,
                            "new_pid": replacement.pid,
                            "stopped_monotonic_ns": stopped_ns,
                            "restarted_monotonic_ns": restarted_ns,
                            "downtime_ms": (restarted_ns - stopped_ns) / 1_000_000,
                            "automated": True,
                        }
                    )
                    crash_started.set()
                    if crash_stop.is_set():
                        return
            except Exception as error:
                crash_error.append(str(error))
                crash_started.set()

        cobalt_thread = threading.Thread(
            target=cobalt_loop, name="cobalt-load", daemon=False
        )
        cobalt_thread.start()
        if args.e4_stress:
            crash_thread = threading.Thread(
                target=crash_loop, name="validator-crash-loop", daemon=False
            )
            crash_thread.start()
            if not crash_started.wait(READY_TIMEOUT_SECONDS):
                raise RuntimeError("validator crash loop did not complete its first restart")
            if crash_error:
                raise RuntimeError("; ".join(crash_error))

        integration_command = benchmark_command(
            node_bin,
            root,
            nodes,
            topology,
            wallet_key,
            wallet_address,
            recipient,
            args.rounds,
            attack_lane,
        )
        integration_start_ns = time.monotonic_ns()
        try:
            run(
                integration_command,
                stdout_path=logs / f"{attack_lane}.stdout.json",
                stderr_path=logs / f"{attack_lane}.stderr",
            )
        finally:
            integration_end_ns = time.monotonic_ns()
            crash_stop.set()
            if crash_thread is not None:
                crash_thread.join()
            attack_resource_stop.set()
            if attack_resource_thread is not None:
                attack_resource_thread.join()
            cobalt_stop.set()
            cobalt_thread.join()
        if crash_error:
            raise RuntimeError("; ".join(crash_error))
        if cobalt_error:
            raise RuntimeError("; ".join(cobalt_error))

        integration_final_status = fleet_status(node_bin, nodes)
        integration_final_pids = current_validator_pids()
        baseline = read_json(root / "baseline" / "report.json")
        integration = read_json(root / attack_lane / "report.json")
        metric = "wallet_to_finality_ms"
        baseline_p50 = nested_percentile(baseline, metric, "p50_ms")
        integration_p50 = nested_percentile(integration, metric, "p50_ms")
        baseline_p95 = nested_p95(baseline, metric)
        integration_p95 = nested_p95(integration, metric)
        delta_percent = (
            ((integration_p95 / baseline_p95) - 1.0) * 100.0
            if baseline_p95 > 0
            else float("inf")
        )
        coverage = interval_coverage(
            integration_start_ns, integration_end_ns, cobalt_intervals
        )
        integration_fleet_alive = (
            len(integration_final_pids) == VALIDATORS
            and all(process.poll() is None for process in validator_processes)
        )

        baseline_fleet_converged = fleet_converged(baseline_final_status)
        integration_fleet_converged = fleet_converged(integration_final_status)
        matched_semantic_workload = (
            benchmark_workload_fingerprint(baseline)
            == benchmark_workload_fingerprint(integration)
        )
        same_final_height = (
            baseline_final_status[0]["block_height"]
            == integration_final_status[0]["block_height"]
        )
        final_state_safety_passed = (
            baseline_fleet_converged
            and integration_fleet_converged
            and same_final_height
        )
        cross_lane_hashes_equal = (
            same_final_height
            and baseline_final_status[0]["block_tip_hash"]
            == integration_final_status[0]["block_tip_hash"]
            and baseline_final_status[0]["state_root"]
            == integration_final_status[0]["state_root"]
        )
        stress_receipts = [
            receipt
            for row in cobalt_runs
            if isinstance((receipt := row.get("e4_stress_receipt")), dict)
        ]
        governance_receipts = [
            receipt
            for row in cobalt_runs
            if isinstance((receipt := row.get("governance_storm")), dict)
        ]
        boundary_rejection_count = sum(
            int(receipt.get("boundary_rejection_count", 0))
            for receipt in stress_receipts
        )
        named_limit_rejection_count = sum(
            int(receipt.get("named_limit_rejection_count", 0))
            for receipt in stress_receipts
        )
        flood_rejection_count = sum(
            int(receipt.get("flood_rejection_count", 0))
            for receipt in stress_receipts
        )
        governance_proposal_count = sum(
            int(receipt.get("proposal_count", 0))
            for receipt in governance_receipts
        )
        governance_safe_halt_count = sum(
            int(receipt.get("safe_halt_count", 0))
            for receipt in governance_receipts
        )
        governance_view_change_count = sum(
            int(receipt.get("view_change_count", 0))
            for receipt in governance_receipts
        )
        baseline_resources = resource_summary(baseline_resource_samples)
        attack_resources = resource_summary(attack_resource_samples)
        baseline_checks = baseline.get("checks", {})
        integration_checks = integration.get("checks", {})
        checks = {
            "locked_round_count": not args.e4_stress or args.rounds >= 500,
            "baseline_passed": baseline.get("status") == "passed"
            and isinstance(baseline_checks, dict)
            and all(baseline_checks.values()),
            "attack_passed": integration.get("status") == "passed"
            and isinstance(integration_checks, dict)
            and all(integration_checks.values()),
            "matched_same_fleet_configuration": (
                matched_initial_state
                and baseline_fleet_alive
                and integration_fleet_alive
                and len(baseline_pids)
                == len(integration_pids)
                == len(integration_final_pids)
                == VALIDATORS
                and len(set(baseline_pids))
                == len(set(integration_pids))
                == len(set(integration_final_pids))
                == VALIDATORS
                and set(baseline_pids).isdisjoint(integration_pids)
            ),
            "same_validator_cpu_quota": True,
            "consensus_v2_active_for_all_measured_rounds": (
                baseline.get("final_state", {}).get("height")
                == integration.get("final_state", {}).get("height")
                == 1 + args.rounds
                and 2 >= ACTIVATION_HEIGHT
            ),
            "consensus_v2_never_stopped": (
                baseline.get("final_state", {}).get("height") == 1 + args.rounds
                and integration.get("final_state", {}).get("height")
                == 1 + args.rounds
            ),
            "consensus_v2_never_forked": (
                baseline_fleet_converged
                and integration_fleet_converged
                and baseline_checks.get("converged") is True
                and integration_checks.get("converged") is True
            ),
            "matched_semantic_workload": matched_semantic_workload,
            "cobalt_active_during_attack": bool(cobalt_runs)
            and coverage >= 0.95
            and all(run_receipt["status"] == "passed" for run_receipt in cobalt_runs),
            "governance_storm_exercised": (
                not args.e4_stress
                or (
                    governance_proposal_count >= 20
                    and governance_safe_halt_count >= 7
                    and governance_view_change_count >= 7
                )
            ),
            "certificate_and_rpc_limits_enforced": (
                not args.e4_stress
                or (
                    boundary_rejection_count >= 21
                    and named_limit_rejection_count >= 18
                    and flood_rejection_count >= 16
                    and all(
                        receipt.get("durable_state_unchanged") is True
                        for receipt in stress_receipts
                    )
                )
            ),
            "one_validator_crash_looped": (
                not args.e4_stress
                or (
                    len(crash_receipts) >= 3
                    and {receipt["node_id"] for receipt in crash_receipts}
                    == {"validator-5"}
                )
            ),
            "p95_within_five_percent": delta_percent <= 5.0,
            "resources_recorded": (
                baseline_resources["sample_count"] >= 2
                and attack_resources["sample_count"] >= 2
            ),
            "durable_history_only_through_consensus": (
                baseline_checks.get("state_verified_after_run") is True
                and integration_checks.get("state_verified_after_run") is True
                and baseline_checks.get("converged") is True
                and integration_checks.get("converged") is True
            ),
            "simulation_only": True,
        }
        report = {
            "schema": (
                "postfiat-cobalt-adversarial-e4-v2"
                if args.e4_stress
                else "postfiat-consensus-v2-cobalt-paired-integration-v1"
            ),
            "status": "passed" if all(checks.values()) else "failed",
            "scope": "paired six-validator local Consensus v2 finality isolation campaign",
            "source_commit": args.source_commit,
            "binaries": {
                "postfiat_node_sha256": digest(node_bin),
                "cobalt_liveness_simulation_sha256": digest(cobalt_bin),
            },
            "config": {
                "validators": VALIDATORS,
                "rounds_per_lane": args.rounds,
                "e4_stress": args.e4_stress,
                "lane_names": ["baseline", attack_lane],
                "vote_policy": "full",
                "consensus_v2_activation_height": ACTIVATION_HEIGHT,
                "same_fleet": "matched identities, keys, topology, binary, host, initial state snapshot, and validator CPU allocation",
                "validator_cpu_quota": {
                    "baseline": "host-unlimited",
                    "attack": "host-unlimited",
                    "equal": True,
                },
                "external_operators_required": False,
                "production_cobalt_service_cpu_quota_percent": 25,
                "cobalt_simulation_process_cpu_quota_percent": args.cobalt_cpu_quota_percent,
                "quota_matches_production_service_unit": args.cobalt_cpu_quota_percent == 25,
                "simulated_validator_domains": VALIDATORS,
            },
            "metric": {
                "name": metric,
                "meaning": "client-visible wallet submission to Consensus v2 finality",
                "baseline_p50_ms": baseline_p50,
                "attack_p50_ms": integration_p50,
                "baseline_p95_ms": baseline_p95,
                "attack_p95_ms": integration_p95,
                "delta_percent": delta_percent,
                "budget_percent": 5.0,
            },
            "secondary_metrics": {
                name: {
                    "baseline_p50_ms": nested_percentile(
                        baseline, name, "p50_ms"
                    ),
                    "attack_p50_ms": nested_percentile(
                        integration, name, "p50_ms"
                    ),
                    "baseline_p95_ms": nested_p95(baseline, name),
                    "attack_p95_ms": nested_p95(integration, name),
                }
                for name in ("consensus_round_ms", "admitted_to_finality_ms")
            },
            "timing": {
                "baseline_duration_ms": (baseline_end_ns - baseline_start_ns) / 1_000_000,
                "attack_duration_ms": (
                    integration_end_ns - integration_start_ns
                )
                / 1_000_000,
                "cobalt_coverage_ratio": coverage,
            },
            "fleet_pids": {
                "baseline": baseline_pids,
                "attack_initial": integration_pids,
                "attack_final": integration_final_pids,
            },
            "matched_initial_state": {
                "equal": matched_initial_state,
                "baseline": baseline_initial_status,
                "attack": integration_initial_status,
            },
            "final_state_safety": {
                "passed": final_state_safety_passed,
                "baseline_fleet_converged": baseline_fleet_converged,
                "attack_fleet_converged": integration_fleet_converged,
                "same_final_height": same_final_height,
                "cross_lane_hashes_equal": cross_lane_hashes_equal,
                "cross_lane_hash_equality_required": False,
                "cross_lane_hash_exclusion": (
                    "independent executions use randomized ML-DSA transaction and "
                    "consensus signatures, so transaction IDs, certificates, block "
                    "tips, and state roots are not a valid cross-lane fork oracle"
                ),
                "baseline": baseline_final_status,
                "attack": integration_final_status,
            },
            "matched_semantic_workload": {
                "equal": matched_semantic_workload,
                "comparison": (
                    "unsigned workload configuration and per-round consensus outcomes; "
                    "excludes signatures, transaction IDs, certificates, hashes, roots, "
                    "and timing"
                ),
            },
            "governance_stress": {
                "run_count": len(governance_receipts),
                "proposal_count": governance_proposal_count,
                "safe_halt_count": governance_safe_halt_count,
                "view_change_count": governance_view_change_count,
                "p95_validation_wall_micros": [
                    row["governance_p95_validation_wall_micros"]
                    for row in cobalt_runs
                ],
            },
            "rejections": {
                "boundary_rejection_count": boundary_rejection_count,
                "named_limit_rejection_count": named_limit_rejection_count,
                "flood_rejection_count": flood_rejection_count,
                "receipts": stress_receipts,
            },
            "validator_crash_loop": {
                "target": "validator-5" if args.e4_stress else None,
                "restart_count": len(crash_receipts),
                "receipts": crash_receipts,
            },
            "resources": {
                "baseline": baseline_resources,
                "attack": attack_resources,
            },
            "operator_actions": {
                "manual_action_count": 0,
                "automated_validator_restarts": len(crash_receipts),
                "description": "campaign setup and crash-loop actions were automated",
            },
            "cobalt_runs": cobalt_runs,
            "heights": {
                "baseline_final": baseline.get("final_state", {}).get("height"),
                "attack_final": integration.get("final_state", {}).get("height"),
            },
            "checks": checks,
            "evidence": {
                "baseline_report_sha256": digest(root / "baseline" / "report.json"),
                "attack_report_sha256": digest(
                    root / attack_lane / "report.json"
                ),
                "topology_sha256": digest(topology),
            },
            "claims_not_made": [
                "independent human operators",
                "provider or geographic decentralization",
                "public WAN latency",
                "mainnet readiness",
            ],
        }
        write_json(root / "consensus-v2-cobalt-integration.json", report)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if report["status"] == "passed" else 1
    finally:
        crash_stop.set()
        baseline_resource_stop.set()
        attack_resource_stop.set()
        cobalt_stop.set()
        for thread in (
            crash_thread,
            baseline_resource_thread,
            attack_resource_thread,
            cobalt_thread,
        ):
            if thread is not None and thread.is_alive():
                thread.join()
        stop_validators(validator_processes, validator_logs)


if __name__ == "__main__":
    raise SystemExit(main())
