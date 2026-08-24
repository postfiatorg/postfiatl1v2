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
    topology: Path,
    wallet_key_file: Path,
    wallet_address: str,
    recipient: str,
    rounds: int,
    lane: str,
) -> list[str]:
    lane_dir = root / lane
    return [
        str(node_bin),
        "tx-latency-benchmark",
        "--base-dir",
        str(root / "nodes"),
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
        "90000",
        "--send-retries",
        "2",
        "--retry-backoff-ms",
        "25",
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


def nested_p95(report: dict[str, Any], metric: str) -> float:
    value = report.get("latency", {}).get(metric, {}).get("p95_ms")
    if not isinstance(value, (int, float)):
        raise ValueError(f"missing {metric}.p95_ms")
    return float(value)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--cobalt-bin", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--rounds", type=int, default=50)
    parser.add_argument(
        "--cobalt-cpu-quota-percent",
        type=int,
        default=VALIDATORS * 25,
        help="aggregate quota for six simulated sidecars; production unit is 25%% each",
    )
    args = parser.parse_args()

    node_bin = args.node_bin.resolve()
    cobalt_bin = args.cobalt_bin.resolve()
    root = args.output_dir.resolve()
    if not node_bin.is_file() or not cobalt_bin.is_file():
        raise ValueError("both binaries must be regular files")
    if args.rounds < 20:
        raise ValueError("--rounds must be at least 20 for a p95 gate")
    if args.cobalt_cpu_quota_percent <= 0:
        raise ValueError("--cobalt-cpu-quota-percent must be positive")
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
    cobalt_stop = threading.Event()
    cobalt_intervals: list[tuple[int, int]] = []
    cobalt_runs: list[dict[str, Any]] = []
    cobalt_error: list[str] = []

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
                stdout_path=logs / f"{node_id}.snapshot-import.json",
                stderr_path=logs / f"{node_id}.snapshot-import.stderr",
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
                stdout_path=logs / f"{node_id}.key-stage.json",
                stderr_path=logs / f"{node_id}.key-stage.stderr",
            )

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

        for index in range(VALIDATORS):
            node_id = f"validator-{index}"
            data_dir = nodes / node_id
            ready = logs / f"{node_id}.ready.json"
            stdout_handle = (logs / f"{node_id}.stdout.log").open("wb")
            stderr_handle = (logs / f"{node_id}.stderr.log").open("wb")
            validator_logs.append((stdout_handle, stderr_handle))
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
                    str(root / "votes" / node_id),
                    "--max-connections",
                    "10000",
                    "--timeout-ms",
                    "90000",
                ],
                stdout=stdout_handle,
                stderr=stderr_handle,
                env=env,
            )
            validator_processes.append(process)
            wait_ready(ready, process)

        fleet_pids = [process.pid for process in validator_processes]
        baseline_command = benchmark_command(
            node_bin,
            root,
            topology,
            wallet_key,
            wallet_address,
            recipient,
            args.rounds,
            "baseline",
        )
        baseline_start_ns = time.monotonic_ns()
        run(
            baseline_command,
            stdout_path=logs / "baseline.stdout.json",
            stderr_path=logs / "baseline.stderr",
        )
        baseline_end_ns = time.monotonic_ns()

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
                        "started_monotonic_ns": started,
                        "ended_monotonic_ns": ended,
                    }
                )

        cobalt_thread = threading.Thread(target=cobalt_loop, name="cobalt-load", daemon=False)
        cobalt_thread.start()
        integration_command = benchmark_command(
            node_bin,
            root,
            topology,
            wallet_key,
            wallet_address,
            recipient,
            args.rounds,
            "integration",
        )
        integration_start_ns = time.monotonic_ns()
        run(
            integration_command,
            stdout_path=logs / "integration.stdout.json",
            stderr_path=logs / "integration.stderr",
        )
        integration_end_ns = time.monotonic_ns()
        cobalt_stop.set()
        cobalt_thread.join()
        if cobalt_error:
            raise RuntimeError("; ".join(cobalt_error))

        baseline = read_json(root / "baseline" / "report.json")
        integration = read_json(root / "integration" / "report.json")
        metric = "consensus_round_ms"
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
        same_fleet_alive = all(
            process.pid == expected and process.poll() is None
            for process, expected in zip(validator_processes, fleet_pids, strict=True)
        )
        baseline_checks = baseline.get("checks", {})
        integration_checks = integration.get("checks", {})
        checks = {
            "baseline_passed": baseline.get("status") == "passed"
            and isinstance(baseline_checks, dict)
            and all(baseline_checks.values()),
            "integration_passed": integration.get("status") == "passed"
            and isinstance(integration_checks, dict)
            and all(integration_checks.values()),
            "same_six_validator_processes": same_fleet_alive
            and len(fleet_pids) == VALIDATORS
            and len(set(fleet_pids)) == VALIDATORS,
            "consensus_v2_active_for_all_measured_rounds": (
                baseline.get("final_state", {}).get("height", 0) >= ACTIVATION_HEIGHT
                and integration.get("final_state", {}).get("height", 0)
                > baseline.get("final_state", {}).get("height", 0)
            ),
            "cobalt_active_during_integration": bool(cobalt_runs)
            and coverage >= 0.95
            and all(run_receipt["status"] == "passed" for run_receipt in cobalt_runs),
            "p95_within_five_percent": delta_percent <= 5.0,
            "durable_history_only_through_consensus": (
                baseline_checks.get("state_verified_after_run") is True
                and integration_checks.get("state_verified_after_run") is True
                and baseline_checks.get("converged") is True
                and integration_checks.get("converged") is True
            ),
            "simulation_only": True,
        }
        report = {
            "schema": "postfiat-consensus-v2-cobalt-paired-integration-v1",
            "status": "passed" if all(checks.values()) else "failed",
            "scope": "six-validator local protocol-capability integration simulation",
            "source_commit": args.source_commit,
            "binaries": {
                "postfiat_node_sha256": digest(node_bin),
                "cobalt_liveness_simulation_sha256": digest(cobalt_bin),
            },
            "config": {
                "validators": VALIDATORS,
                "rounds_per_lane": args.rounds,
                "vote_policy": "full",
                "consensus_v2_activation_height": ACTIVATION_HEIGHT,
                "same_fleet": True,
                "external_operators_required": False,
                "production_cobalt_cpu_quota_percent_per_validator": 25,
                "simulated_validator_domains": VALIDATORS,
                "aggregate_cobalt_cpu_quota_percent": args.cobalt_cpu_quota_percent,
                "quota_derivation": "six simulated sidecars times 25 percent per production service unit",
            },
            "metric": {
                "name": metric,
                "meaning": "client-visible Consensus v2 round finality",
                "baseline_p95_ms": baseline_p95,
                "integration_p95_ms": integration_p95,
                "delta_percent": delta_percent,
                "budget_percent": 5.0,
            },
            "secondary_metrics": {
                name: {
                    "baseline_p95_ms": nested_p95(baseline, name),
                    "integration_p95_ms": nested_p95(integration, name),
                }
                for name in ("wallet_to_finality_ms", "admitted_to_finality_ms")
            },
            "timing": {
                "baseline_duration_ms": (baseline_end_ns - baseline_start_ns) / 1_000_000,
                "integration_duration_ms": (
                    integration_end_ns - integration_start_ns
                )
                / 1_000_000,
                "cobalt_coverage_ratio": coverage,
            },
            "fleet_pids": fleet_pids,
            "cobalt_runs": cobalt_runs,
            "heights": {
                "baseline_final": baseline.get("final_state", {}).get("height"),
                "integration_final": integration.get("final_state", {}).get("height"),
            },
            "checks": checks,
            "evidence": {
                "baseline_report_sha256": digest(root / "baseline" / "report.json"),
                "integration_report_sha256": digest(
                    root / "integration" / "report.json"
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
        cobalt_stop.set()
        for process in validator_processes:
            if process.poll() is None:
                process.terminate()
        deadline = time.monotonic() + 5
        for process in validator_processes:
            if process.poll() is None:
                remaining = max(0.0, deadline - time.monotonic())
                try:
                    process.wait(timeout=remaining)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        for stdout_handle, stderr_handle in validator_logs:
            stdout_handle.close()
            stderr_handle.close()


if __name__ == "__main__":
    raise SystemExit(main())
