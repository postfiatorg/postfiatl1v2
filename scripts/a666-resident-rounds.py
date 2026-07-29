#!/usr/bin/env python3
"""Prewarm and use one-shot resident PFTL consensus workers.

The worker process is started before an Ethereum value-moving transaction.
It pays binary startup and shielded-verifier prewarm outside the user latency
clock, then waits for exactly one atomically published batch.  Every worker is
bound to one deterministic height, elected proposer, batch kind, and isolated
certified-send outbox.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time
from typing import Any


EXPECTED_VALIDATORS = 6
MANIFEST_SCHEMA = "postfiat.a666.resident_round_manifest.v1"
READY_SCHEMA = "postfiat-transport-peer-certified-batch-loop-ready-v1"
LOOP_SCHEMA = "postfiat-transport-peer-certified-batch-loop-v1"


def run(
    command: list[str],
    *,
    capture: bool = False,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        check=check,
        text=True,
        capture_output=capture,
    )


def ssh(host: str, command: str, *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    return run(
        ["ssh", "-o", "BatchMode=yes", f"root@{host}", command],
        capture=capture,
    )


def load_rpc_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("a666_rpc_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import RPC helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_json(path: Path, value: Any, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def load_hosts(path: Path) -> dict[str, str]:
    value = json.loads(path.read_text())
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if not isinstance(value, dict) or set(value) != expected:
        raise RuntimeError("proposer hosts file must map validator-0 through validator-5")
    for host in value.values():
        if (
            not isinstance(host, str)
            or not host
            or any(character.isspace() for character in host)
            or "@" in host
        ):
            raise RuntimeError("proposer hosts file contains an invalid SSH host")
    return value


def safe_workflow(value: str) -> str:
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,39}", value):
        raise RuntimeError("workflow id is not a safe path component")
    return value


def parse_plan(value: str) -> list[str]:
    kinds = value.split(",")
    allowed = {"transparent", "governance", "shielded", "bridge"}
    if not kinds or any(kind not in allowed for kind in kinds):
        raise RuntimeError(
            "plan must be a comma-separated list of transparent, governance, "
            "shielded, or bridge"
        )
    return kinds


def proposer_for_height(
    hosts: dict[str, str],
    remote_binary: str,
    height: int,
) -> str:
    probe = run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            f"root@{hosts['validator-0']}",
            remote_binary,
            "block-proposer",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            "/var/lib/postfiat/validator-0",
            "--height",
            str(height),
            "--view",
            "0",
        ],
        capture=True,
    )
    proposer = json.loads(probe.stdout).get("proposer")
    if proposer not in hosts:
        raise RuntimeError(f"height {height} selected unknown proposer {proposer!r}")
    return str(proposer)


def remote_paths(workflow: str, proposer: str, height: int) -> dict[str, str]:
    index = proposer.rsplit("-", 1)[1]
    data_dir = f"/var/lib/postfiat/validator-{index}"
    root = f"{data_dir}/a666-resident-rounds/{workflow}/h{height}"
    return {
        "data_dir": data_dir,
        "root": root,
        "batch_dir": f"{root}/incoming",
        "artifact_root": f"{root}/artifacts",
        "processed_dir": f"{root}/processed",
        "isolated_outbox": f"{root}/isolated-outbox",
        "ready_file": f"{root}/ready.json",
        "report_file": f"{root}/loop-report.json",
        "report_tmp_file": f"{root}/loop-report.tmp",
        "stderr_file": f"{root}/stderr.log",
        "exit_file": f"{root}/exit-code.txt",
        "pid_file": f"{root}/pid.txt",
    }


def start_worker(entry: dict[str, Any], remote_binary: str, remote_topology: str) -> None:
    host = entry["host"]
    paths = entry["remote"]
    height = entry["height"]
    batch_kind = entry["batch_kind"]
    key_file = f"{paths['data_dir']}/validator_keys.json"

    prepare = "set -euo pipefail; " + "; ".join(
        [
            f"test ! -e {shlex.quote(paths['root'])}",
            (
                "install -d -o postfiat -g postfiat -m 700 "
                f"{shlex.quote(paths['root'])}"
            ),
            (
                "install -d -o postfiat -g postfiat -m 700 "
                + " ".join(
                    shlex.quote(paths[name])
                    for name in (
                        "batch_dir",
                        "artifact_root",
                        "processed_dir",
                        "isolated_outbox",
                    )
                )
            ),
        ]
    )
    ssh(host, prepare)

    worker_args = [
        remote_binary,
        "transport-peer-certified-batch-loop",
        "--unsafe-devnet-json-storage",
        "--data-dir",
        paths["data_dir"],
        "--topology",
        remote_topology,
        "--batch-kind",
        batch_kind,
        "--batch-dir",
        paths["batch_dir"],
        "--key-file",
        key_file,
        "--proposal-key-file",
        key_file,
        "--require-local-proposer",
        "--quorum-early-full-propagation",
        "--local-apply-before-certified-send",
        "--artifact-root",
        paths["artifact_root"],
        "--processed-dir",
        paths["processed_dir"],
        "--max-rounds",
        "1",
        "--start-height",
        str(height),
        "--poll-ms",
        "25",
        "--timeout-ms",
        "30000",
        "--idle-timeout-ms",
        "7200000",
        "--send-retries",
        "16",
        "--retry-backoff-ms",
        "250",
    ]
    inner = (
        "set +e; "
        f"mount --bind {shlex.quote(paths['isolated_outbox'])} "
        f"{shlex.quote(paths['data_dir'] + '/certified-send-outbox')}; "
        "runuser -u postfiat -- env "
        "POSTFIAT_PREWARM_SHIELDED_VERIFIER=1 "
        "POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=1 "
        "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1 "
        f"POSTFIAT_CERTIFIED_BATCH_LOOP_READY_FILE={shlex.quote(paths['ready_file'])} "
        + " ".join(shlex.quote(value) for value in worker_args)
        + f" > {shlex.quote(paths['report_tmp_file'])} "
        + f"2> {shlex.quote(paths['stderr_file'])}; "
        "code=$?; "
        f"mv {shlex.quote(paths['report_tmp_file'])} "
        f"{shlex.quote(paths['report_file'])}; "
        f"printf '%s\\n' \"$code\" > {shlex.quote(paths['exit_file'])}; "
        "exit \"$code\""
    )
    launch = (
        "set -euo pipefail; "
        "nohup unshare --mount --propagation private -- /bin/bash -c "
        f"{shlex.quote(inner)} </dev/null >/dev/null 2>&1 & "
        f"printf '%s\\n' \"$!\" > {shlex.quote(paths['pid_file'])}"
    )
    ssh(host, launch)


def wait_ready(entry: dict[str, Any], timeout: float) -> dict[str, Any]:
    paths = entry["remote"]
    command = (
        "set -euo pipefail; "
        f"deadline=$((SECONDS+{int(timeout)})); "
        f"while ! test -s {shlex.quote(paths['ready_file'])}; do "
        f"if test -s {shlex.quote(paths['exit_file'])}; then "
        f"cat {shlex.quote(paths['stderr_file'])} >&2; exit 1; fi; "
        'if test "$SECONDS" -ge "$deadline"; then exit 124; fi; '
        "sleep 0.1; done; "
        f"cat {shlex.quote(paths['ready_file'])}"
    )
    completed = ssh(entry["host"], command, capture=True)
    ready = json.loads(completed.stdout)
    if (
        ready.get("schema") != READY_SCHEMA
        or ready.get("node_id") != entry["proposer"]
        or ready.get("start_height") != entry["height"]
        or ready.get("max_rounds") != 1
    ):
        raise RuntimeError(f"resident worker readiness mismatch at height {entry['height']}")
    prewarm = ready.get("shielded_verifier_prewarm") or {}
    if (
        prewarm.get("requested") is not True
        or prewarm.get("asset_orchard_swap_verifier_warm") is not True
        or prewarm.get("asset_orchard_private_egress_verifier_warm") is not True
    ):
        raise RuntimeError(
            f"resident worker verifier prewarm failed at height {entry['height']}"
        )
    return ready


def command_start(args: argparse.Namespace) -> None:
    workflow = safe_workflow(args.workflow_id)
    plan = parse_plan(args.plan)
    hosts = load_hosts(args.proposer_hosts_file)
    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")
    fleet = rpc.wait_for_fleet_status(ports, args.timeout_seconds, args.timeout_seconds)
    observed_height = int(fleet[0]["block_height"])
    if observed_height != args.start_height:
        raise RuntimeError(
            f"resident start expected PFTL height {args.start_height}, "
            f"observed {observed_height}"
        )
    if args.output.exists():
        raise RuntimeError(f"resident manifest already exists: {args.output}")

    entries: list[dict[str, Any]] = []
    for offset, batch_kind in enumerate(plan, start=1):
        height = args.start_height + offset
        proposer = proposer_for_height(hosts, args.remote_binary, height)
        entries.append(
            {
                "height": height,
                "batch_kind": batch_kind,
                "proposer": proposer,
                "host": hosts[proposer],
                "remote": remote_paths(workflow, proposer, height),
            }
        )

    with ThreadPoolExecutor(max_workers=len(entries)) as executor:
        futures = [
            executor.submit(
                start_worker,
                entry,
                args.remote_binary,
                args.remote_topology,
            )
            for entry in entries
        ]
        for future in futures:
            future.result()
    with ThreadPoolExecutor(max_workers=len(entries)) as executor:
        ready_reports = list(
            executor.map(lambda entry: wait_ready(entry, args.ready_timeout), entries)
        )
    for entry, ready in zip(entries, ready_reports, strict=True):
        entry["ready"] = ready

    manifest = {
        "schema": MANIFEST_SCHEMA,
        "workflow_id": workflow,
        "created_unix": int(time.time()),
        "start_height": args.start_height,
        "start_state_root": fleet[0]["state_root"],
        "remote_binary": args.remote_binary,
        "remote_topology": args.remote_topology,
        "entries": entries,
    }
    write_json(args.output, manifest, 0o644)
    print(json.dumps(manifest, indent=2, sort_keys=True))


def load_manifest(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if value.get("schema") != MANIFEST_SCHEMA:
        raise RuntimeError("resident round manifest schema mismatch")
    safe_workflow(str(value.get("workflow_id", "")))
    entries = value.get("entries")
    if not isinstance(entries, list) or not entries:
        raise RuntimeError("resident round manifest has no entries")
    return value


def select_entry(
    manifest: dict[str, Any],
    height: int,
    batch_kind: str,
) -> dict[str, Any]:
    matches = [
        entry
        for entry in manifest["entries"]
        if entry.get("height") == height and entry.get("batch_kind") == batch_kind
    ]
    if len(matches) != 1:
        raise RuntimeError(
            f"resident manifest does not contain exactly one {batch_kind} worker "
            f"for height {height}"
        )
    return matches[0]


def wait_remote_report(entry: dict[str, Any], timeout: float) -> tuple[dict[str, Any], int]:
    paths = entry["remote"]
    command = (
        "set -euo pipefail; "
        f"deadline=$((SECONDS+{int(timeout)})); "
        f"while ! test -s {shlex.quote(paths['report_file'])} "
        f"|| ! test -s {shlex.quote(paths['exit_file'])}; do "
        'if test "$SECONDS" -ge "$deadline"; then exit 124; fi; '
        "sleep 0.1; done; "
        f"cat {shlex.quote(paths['report_file'])}; "
        "printf '\\n__A666_EXIT__='; "
        f"cat {shlex.quote(paths['exit_file'])}"
    )
    completed = ssh(entry["host"], command, capture=True)
    report_text, marker = completed.stdout.rsplit("\n__A666_EXIT__=", 1)
    return json.loads(report_text), int(marker.strip())


def wait_for_height(
    rpc: Any,
    ports: list[int],
    rpc_timeout: float,
    convergence_timeout: float,
    expected_height: int,
) -> list[dict[str, Any]]:
    deadline = time.monotonic() + convergence_timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            fleet = rpc.fleet_status(ports, rpc_timeout)
            if int(fleet[0]["block_height"]) == expected_height:
                return fleet
            last_error = RuntimeError(
                f"expected height {expected_height}, observed {fleet[0]['block_height']}"
            )
        except Exception as error:
            last_error = error
        time.sleep(0.25)
    raise RuntimeError(
        f"fleet did not converge at height {expected_height}: {last_error}"
    ) from last_error


def command_submit(args: argparse.Namespace) -> None:
    manifest = load_manifest(args.manifest)
    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")
    if args.artifact_dir.exists():
        raise RuntimeError(f"artifact directory already exists: {args.artifact_dir}")
    if not args.batch_file.is_file():
        raise RuntimeError(f"batch file does not exist: {args.batch_file}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", args.label):
        raise RuntimeError("label is not a safe path component")

    pre = rpc.wait_for_fleet_status(
        ports, args.timeout_seconds, args.preflight_seconds
    )
    parent = pre[0]
    height = int(parent["block_height"]) + 1
    start_height = int(manifest["start_height"])
    last_height = start_height + len(manifest["entries"])
    if not start_height < height <= last_height:
        raise RuntimeError(
            f"height {height} is outside resident plan {start_height + 1}..{last_height}"
        )
    entry = select_entry(manifest, height, args.batch_kind)
    if parent["state_root"] == manifest.get("start_state_root") and height != (
        int(manifest["start_height"]) + 1
    ):
        raise RuntimeError("resident manifest height progression is inconsistent")

    args.artifact_dir.mkdir(parents=True, mode=0o700)
    rpc.write_json(
        args.artifact_dir / "preflight-fleet.json",
        {
            "schema": "postfiat.a666.resident_round_submit_preflight.v1",
            "label": args.label,
            "batch_kind": args.batch_kind,
            "height": parent["block_height"],
            "state_root": parent["state_root"],
            "nodes": pre,
            "resident_entry": entry,
        },
        0o644,
    )
    paths = entry["remote"]
    remote_partial = f"{paths['batch_dir']}/.{args.label}.partial"
    remote_batch = f"{paths['batch_dir']}/{args.label}.batch.json"
    ssh(
        entry["host"],
        "set -euo pipefail; "
        f"test -s {shlex.quote(paths['ready_file'])}; "
        f"test ! -e {shlex.quote(paths['report_file'])}; "
        f"test ! -e {shlex.quote(remote_partial)}; "
        f"test ! -e {shlex.quote(remote_batch)}",
    )
    run(
        [
            "scp",
            "-q",
            str(args.batch_file),
            f"root@{entry['host']}:{remote_partial}",
        ]
    )
    ssh(
        entry["host"],
        "set -euo pipefail; "
        f"chown postfiat:postfiat {shlex.quote(remote_partial)}; "
        f"chmod 600 {shlex.quote(remote_partial)}; "
        f"mv {shlex.quote(remote_partial)} {shlex.quote(remote_batch)}",
    )

    report, exit_code = wait_remote_report(entry, args.round_timeout_seconds)
    if report.get("schema") != LOOP_SCHEMA:
        raise RuntimeError("resident worker returned the wrong report schema")
    rounds = report.get("rounds")
    if (
        exit_code != 0
        or report.get("loop_ok") is not True
        or report.get("processed_round_count") != 1
        or not isinstance(rounds, list)
        or len(rounds) != 1
    ):
        stderr = ssh(
            entry["host"],
            f"cat {shlex.quote(paths['stderr_file'])}",
            capture=True,
        ).stdout.strip()
        raise RuntimeError(
            f"resident worker failed at height {height}, exit={exit_code}: {stderr}"
        )
    round_report = rounds[0]

    consensus_dir = args.artifact_dir / "consensus"
    consensus_dir.mkdir(mode=0o700)
    run(
        [
            "rsync",
            "-a",
            f"root@{entry['host']}:{paths['artifact_root']}/round-{height}/",
            f"{consensus_dir}/",
        ]
    )
    rpc.write_json(consensus_dir / "round-report.json", round_report)
    rpc.write_json(args.artifact_dir / "resident-loop-report.json", report, 0o644)

    post = wait_for_height(
        rpc,
        ports,
        args.timeout_seconds,
        args.postflight_seconds,
        height,
    )
    certification = round_report.get("certification") or {}
    accepted = (
        round_report.get("round_ok") is True
        and round_report.get("all_sends_verified") is True
        and round_report.get("local_apply_verified") is True
        and certification.get("block_height") == height
        and certification.get("batch_kind") == args.batch_kind
    )
    converged = len({node["state_root"] for node in post}) == 1
    summary = {
        "schema": "postfiat-a666-ce22-remote-finality-batch-v1",
        "execution_mode": "prewarmed_resident_worker",
        "label": args.label,
        "batch_kind": args.batch_kind,
        "accepted": accepted,
        "confirmed": converged,
        "round_ok": round_report.get("round_ok"),
        "proposer": entry["proposer"],
        "vote_count": certification.get("vote_count"),
        "start_height": parent["block_height"],
        "end_height": post[0]["block_height"],
        "start_state_root": parent["state_root"],
        "end_state_root": post[0]["state_root"],
        "all_sends_verified": round_report.get("all_sends_verified"),
        "local_apply_verified": round_report.get("local_apply_verified"),
        "runner_exit_code": exit_code,
        "resident_prewarm": report.get("shielded_verifier_prewarm"),
    }
    rpc.write_json(args.artifact_dir / "summary.json", summary, 0o644)
    print(json.dumps(summary, indent=2, sort_keys=True))
    if not accepted or not converged:
        raise RuntimeError("resident round did not finish accepted and converged")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    subparsers = root.add_subparsers(dest="command", required=True)

    start = subparsers.add_parser("start")
    start.add_argument("--workflow-id", required=True)
    start.add_argument("--start-height", type=int, required=True)
    start.add_argument("--plan", required=True)
    start.add_argument("--output", type=Path, required=True)
    start.add_argument("--proposer-hosts-file", type=Path, required=True)
    start.add_argument("--remote-binary", required=True)
    start.add_argument("--remote-topology", required=True)
    start.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    start.add_argument("--timeout-seconds", type=float, default=45.0)
    start.add_argument("--ready-timeout", type=float, default=180.0)

    submit = subparsers.add_parser("submit")
    submit.add_argument("--manifest", type=Path, required=True)
    submit.add_argument("--batch-file", type=Path, required=True)
    submit.add_argument(
        "--batch-kind",
        choices=("transparent", "governance", "shielded", "bridge"),
        required=True,
    )
    submit.add_argument("--label", required=True)
    submit.add_argument("--artifact-dir", type=Path, required=True)
    submit.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    submit.add_argument("--timeout-seconds", type=float, default=45.0)
    submit.add_argument("--preflight-seconds", type=float, default=45.0)
    submit.add_argument("--postflight-seconds", type=float, default=45.0)
    submit.add_argument("--round-timeout-seconds", type=float, default=180.0)
    return root


def main() -> None:
    args = parser().parse_args()
    if args.command == "start":
        command_start(args)
    else:
        command_submit(args)


if __name__ == "__main__":
    main()
