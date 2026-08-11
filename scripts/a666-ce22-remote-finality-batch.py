#!/usr/bin/env python3
"""Submit one prebuilt batch through a clean elected-proposer ce22 round."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
from typing import Any


EXPECTED_VALIDATORS = 6


def load_rpc_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("a666_rpc_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import RPC helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_runtime_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666_remote_runtime.py"
    spec = importlib.util.spec_from_file_location("a666_remote_runtime", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import runtime helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, check=True, text=True, capture_output=capture)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch-file", type=Path, required=True)
    parser.add_argument("--batch-kind", choices=("transparent", "governance", "shielded", "bridge"), required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--remote-runner", type=Path, required=True)
    parser.add_argument("--proposer-hosts-file", type=Path, required=True)
    parser.add_argument("--remote-binary", required=True)
    parser.add_argument("--remote-topology", required=True)
    parser.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    parser.add_argument("--timeout-seconds", type=float, default=45.0)
    parser.add_argument("--preflight-seconds", type=float, default=45.0)
    parser.add_argument("--postflight-seconds", type=float, default=45.0)
    parser.add_argument("--resume-postflight", action="store_true")
    parser.add_argument("--resident-manifest", type=Path)
    return parser.parse_args()


def wait_for_height(
    rpc: Any,
    ports: list[int],
    rpc_timeout: float,
    convergence_timeout: float,
    expected_height: int,
) -> list[dict[str, Any]]:
    deadline = time.monotonic() + convergence_timeout
    last_error: Exception | None = None
    while True:
        try:
            candidate = rpc.fleet_status(ports, rpc_timeout)
            observed_height = int(candidate[0]["block_height"])
            if observed_height == expected_height:
                return candidate
            last_error = RuntimeError(
                f"expected height {expected_height}, observed {observed_height}"
            )
        except Exception as error:
            last_error = error
        if time.monotonic() >= deadline:
            raise RuntimeError(
                f"fleet did not converge at height {expected_height} within "
                f"{convergence_timeout}s: {last_error}"
            ) from last_error
        time.sleep(0.25)


def main() -> None:
    args = parse_args()
    script_dir = Path(__file__).resolve().parent
    runtime = load_runtime_helpers(script_dir)
    release_id = runtime.validated_release_id(
        args.remote_binary,
        args.remote_topology,
    )
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", args.label):
        raise RuntimeError("label is not a safe remote path component")
    if args.resident_manifest is not None:
        if args.resume_postflight:
            raise RuntimeError("resident rounds do not support one-shot postflight resume")
        run(
            [
                sys.executable,
                str(Path(__file__).resolve().parent / "a666-resident-rounds.py"),
                "submit",
                "--manifest",
                str(args.resident_manifest),
                "--batch-file",
                str(args.batch_file),
                "--batch-kind",
                args.batch_kind,
                "--label",
                args.label,
                "--artifact-dir",
                str(args.artifact_dir),
                "--ports",
                args.ports,
                "--timeout-seconds",
                str(args.timeout_seconds),
                "--preflight-seconds",
                str(args.preflight_seconds),
                "--postflight-seconds",
                str(args.postflight_seconds),
            ]
        )
        return
    proposer_hosts = runtime.load_proposer_hosts(args.proposer_hosts_file)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != EXPECTED_VALIDATORS:
        raise RuntimeError("exactly six RPC endpoints are required")

    rpc = load_rpc_helpers(script_dir)
    consensus_dir = args.artifact_dir / "consensus"
    if args.resume_postflight:
        if not args.artifact_dir.is_dir():
            raise RuntimeError(
                f"resume artifact directory does not exist: {args.artifact_dir}"
            )
        if (args.artifact_dir / "summary.json").exists():
            raise RuntimeError("resume artifact already contains a summary")
        preflight = json.loads(
            (args.artifact_dir / "preflight-fleet.json").read_text()
        )
        if (
            preflight.get("label") != args.label
            or preflight.get("batch_kind") != args.batch_kind
        ):
            raise RuntimeError("resume label or batch kind does not match preflight")
        pre = preflight.get("nodes")
        if not isinstance(pre, list) or len(pre) != EXPECTED_VALIDATORS:
            raise RuntimeError("resume preflight does not contain six validators")
        parent = pre[0]
        if (
            any(node.get("block_height") != preflight.get("height") for node in pre)
            or any(node.get("state_root") != preflight.get("state_root") for node in pre)
        ):
            raise RuntimeError("resume preflight does not prove one parent state")
        next_height = int(preflight["height"]) + 1
        report = json.loads((consensus_dir / "round-report.json").read_text())
        proposer = report.get("proposal_proposer")
        runner_exit_code = None
        runner_stderr = None
        recovered_postflight = True
    else:
        if args.artifact_dir.exists():
            raise RuntimeError(
                f"artifact directory already exists: {args.artifact_dir}"
            )
        args.artifact_dir.mkdir(parents=True, mode=0o700)
        runtime_report = runtime.probe_remote_runtime(
            proposer_hosts,
            args.remote_binary,
            args.remote_topology,
            timeout_seconds=args.timeout_seconds,
        )
        rpc.write_json(
            args.artifact_dir / "remote-runtime-identity.json",
            runtime_report,
            0o644,
        )
        pre = rpc.wait_for_fleet_status(
            ports,
            args.timeout_seconds,
            args.preflight_seconds,
        )
        parent = pre[0]
        next_height = int(parent["block_height"]) + 1
        rpc.write_json(
            args.artifact_dir / "preflight-fleet.json",
            {
                "schema": "postfiat-a666-ce22-remote-finality-batch-preflight-v1",
                "label": args.label,
                "batch_kind": args.batch_kind,
                "release_id": release_id,
                "remote_binary": args.remote_binary,
                "remote_topology": args.remote_topology,
                "height": parent["block_height"],
                "state_root": parent["state_root"],
                "nodes": pre,
            },
            0o644,
        )

        proposer_probe = run(
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                f"root@{proposer_hosts['validator-0']}",
                args.remote_binary,
                "block-proposer",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                "/var/lib/postfiat/validator-0",
                "--height",
                str(next_height),
                "--view",
                "0",
            ],
            capture=True,
        )
        proposer = json.loads(proposer_probe.stdout)["proposer"]
        if proposer not in proposer_hosts:
            raise RuntimeError(f"unknown elected proposer: {proposer}")
        host = proposer_hosts[proposer]
        index = proposer.rsplit("-", 1)[1]
        data_dir = f"/var/lib/postfiat/validator-{index}"
        key_path = f"{data_dir}/validator_keys.json"
        remote_label = f"a666-{next_height}-{args.label}"
        remote_batch = f"{data_dir}/{remote_label}.batch.json"
        remote_artifacts = f"{data_dir}/a666-finality-artifacts/{remote_label}"
        isolated_outbox = f"{data_dir}/a666-isolated-outboxes/{remote_label}"
        remote_runner = "/usr/local/sbin/a666-remote-sync-batch-round.py"

        run(
            [
                "scp",
                "-q",
                str(args.remote_runner),
                f"root@{host}:/tmp/{remote_label}.runner.py",
            ]
        )
        run(
            [
                "scp",
                "-q",
                str(args.batch_file),
                f"root@{host}:/tmp/{remote_label}.batch.json",
            ]
        )
        prepare = "set -euo pipefail; " + "; ".join(
            [
                f"test ! -e {shlex.quote(remote_artifacts)}",
                f"test ! -e {shlex.quote(isolated_outbox)}",
                (
                    "install -d -o postfiat -g postfiat -m 700 "
                    f"{shlex.quote(isolated_outbox)}"
                ),
                (
                    "install -d -o postfiat -g postfiat -m 700 "
                    f"{shlex.quote(data_dir + '/a666-finality-artifacts')}"
                ),
                (
                    "install -o postfiat -g postfiat -m 600 "
                    f"{shlex.quote('/tmp/' + remote_label + '.batch.json')} "
                    f"{shlex.quote(remote_batch)}"
                ),
                (
                    "install -o root -g root -m 755 "
                    f"{shlex.quote('/tmp/' + remote_label + '.runner.py')} "
                    f"{shlex.quote(remote_runner)}"
                ),
            ]
        )
        run(["ssh", "-o", "BatchMode=yes", f"root@{host}", prepare])

        runner_args = [
            remote_runner,
            "--node-bin",
            args.remote_binary,
            "--data-dir",
            data_dir,
            "--topology",
            args.remote_topology,
            "--key-file",
            key_path,
            "--batch-kind",
            args.batch_kind,
            "--batch-file",
            remote_batch,
            "--artifact-dir",
            remote_artifacts,
            "--height",
            str(next_height),
            "--view",
            "0",
        ]
        namespace_command = (
            "set -euo pipefail; "
            f"mount --bind {shlex.quote(isolated_outbox)} "
            f"{shlex.quote(data_dir + '/certified-send-outbox')}; "
            "exec runuser -u postfiat -- "
            + " ".join(shlex.quote(value) for value in runner_args)
        )
        runner = subprocess.run(
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                f"root@{host}",
                "unshare --mount --propagation private -- /bin/bash -c "
                + shlex.quote(namespace_command),
            ],
            text=True,
            capture_output=True,
        )

        consensus_dir.mkdir(mode=0o700)
        run(
            [
                "rsync",
                "-a",
                f"root@{host}:{remote_artifacts}/",
                f"{consensus_dir}/",
            ]
        )
        report_file = consensus_dir / "round-report.json"
        report = json.loads(report_file.read_text()) if report_file.exists() else {}
        runner_exit_code = runner.returncode
        runner_stderr = runner.stderr.strip() or None
        recovered_postflight = False

    certification = report.get("certification") or {}
    if (
        proposer not in proposer_hosts
        or certification.get("block_height") != next_height
        or certification.get("batch_kind") != args.batch_kind
    ):
        raise RuntimeError("batch report does not match the expected finality round")

    post = wait_for_height(
        rpc,
        ports,
        args.timeout_seconds,
        args.postflight_seconds,
        next_height,
    )
    consensus_confirmed = len({node["state_root"] for node in post}) == 1
    application_accepted = (
        runner_exit_code in (None, 0)
        and report.get("round_ok") is True
        and report.get("all_sends_verified") is True
        and report.get("local_apply_verified") is True
    )
    summary = {
        "schema": "postfiat-a666-ce22-remote-finality-batch-v1",
        "label": args.label,
        "batch_kind": args.batch_kind,
        "accepted": application_accepted,
        "confirmed": consensus_confirmed,
        "round_ok": report.get("round_ok"),
        "proposer": proposer,
        "vote_count": (report.get("certification") or {}).get("vote_count"),
        "start_height": parent["block_height"],
        "end_height": post[0]["block_height"] if post is not None else None,
        "start_state_root": parent["state_root"],
        "end_state_root": post[0]["state_root"] if post is not None else None,
        "all_sends_verified": report.get("all_sends_verified"),
        "local_apply_verified": report.get("local_apply_verified"),
        "runner_exit_code": runner_exit_code,
        "runner_stderr": runner_stderr,
        "recovered_postflight": recovered_postflight,
    }
    rpc.write_json(args.artifact_dir / "summary.json", summary, 0o644)
    print(json.dumps(summary, indent=2, sort_keys=True))
    if not consensus_confirmed:
        raise RuntimeError("fleet state roots diverged after finality")
    if not application_accepted:
        raise RuntimeError(
            "batch reached consensus but its application was not accepted; "
            "artifacts and summary were preserved"
        )


if __name__ == "__main__":
    main()
