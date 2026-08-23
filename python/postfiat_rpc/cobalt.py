"""Inspect Cobalt governance safety without granting it consensus authority.

Run from the repository root with::

    PYTHONPATH=python python3 -m postfiat_rpc.cobalt trust-graph

The CLI executes the existing ``postfiat-consensus-cobalt`` examples. It does
not reimplement their safety rules, initialize a node, process blocks, or alter
the validator registry.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Callable, Sequence


CLI_SCHEMA = "postfiat-cobalt-governance-cli-v2"
MAX_REPORT_BYTES = 4 * 1024 * 1024
MAX_CHECKSUM_MANIFEST_BYTES = 64 * 1024
MAX_PACKET_FILES = 64
BENCHMARK_PACKET_SCHEMA = "postfiat-cobalt-rippled-liveness-verifier-v1"
HANDOFF_PACKET_SCHEMA = "postfiat-cobalt-handoff-rehearsal-verifier-v1"
DEFAULT_BENCHMARK_PACKET_SHA256 = (
    "7968a085033419255b52b844edd586346a1e85561394e52c69e6683b2561c50b"
)
DEFAULT_HANDOFF_PACKET_SHA256 = (
    "b678b3f45eb2a14299b941101bd556d61795a1033f1f6e53557442b7e315807e"
)
DEFAULT_HANDOFF_VERIFIER_SHA256 = (
    "dfb9000d272f71d6d1578b7d8332a844a142d8d08d561889da2b3842f62cc9e9"
)
BENCHMARK_REQUIRED_FILES = {
    "cobalt-report.json",
    "kpi-report.json",
    "rippled-report.json",
    "scenario-manifest.json",
    "verifier.json",
}
HANDOFF_REQUIRED_FILES = {
    "activation-result.json",
    "forward-rollback-result.json",
    "live-fleet-after.json",
    "live-fleet-before.json",
    "negative-cases.json",
    "pre-activation-abort.json",
    "validator-update-result.json",
}
BENCHMARK_REQUIRED_CHECKS = {
    "both_adapter_hashes_match_manifest",
    "case_order_matches",
    "cobalt_authority_disabled",
    "cobalt_passed",
    "cobalt_replay_equal",
    "cobalt_zero_conflicts",
    "rippled_native_fork_control_present",
    "rippled_passed",
    "rippled_zero_conflicts",
    "scenario_case_count",
    "scenario_manifest_hash",
    "scenario_manifest_schema",
}
HANDOFF_REQUIRED_CHECKS = {
    "activation_clone_state_changed",
    "activation_future_height",
    "all_six_negative_cases_rejected",
    "current_registry_and_validator_count",
    "forward_history_two_transitions_one_update",
    "forward_rollback_restored_foundation",
    "live_authority_and_block_control_disabled",
    "live_registry_and_trust_roots_unchanged",
    "live_validator_processes_unchanged",
    "pre_activation_abort_without_mutation",
    "scoped_validator_update_accepted",
    "unrelated_governance_rejected",
    "validator_private_keys_never_left_validators",
}
AUTHORITY = {
    "mode": "advisory",
    "live": False,
    "controls_block_consensus": False,
    "writes_validator_registry": False,
}


class CobaltCliError(RuntimeError):
    """A bounded, user-facing Cobalt CLI failure."""


@dataclass(frozen=True)
class ExampleSpec:
    command: str
    example: str
    feature: str | None = None


EXAMPLES = {
    "trust-graph": ExampleSpec("trust-graph", "current_trust_graph_root"),
    "transition-witness": ExampleSpec("transition-witness", "cobalt_safety_witness"),
    "protocol-replay": ExampleSpec(
        "protocol-replay", "cobalt_crash_restart", "cobalt-unsafe-simulation"
    ),
}
SHADOW_COMMANDS = {
    "shadow-service-status": "status",
    "shadow-service-drill": "drill",
}
RUNTIME_COMMANDS = {"probe", "snapshot", "replay"}
HISTORY_COMMANDS = {"history-export", "history-verify", "catch-up"}


def repository_root(start: Path | None = None) -> Path:
    """Find the workspace containing the Cobalt crate without changing cwd."""

    candidate = (start or Path(__file__)).resolve()
    if candidate.is_file():
        candidate = candidate.parent
    for parent in (candidate, *candidate.parents):
        if (parent / "crates" / "consensus_cobalt" / "Cargo.toml").is_file():
            return parent
    raise CobaltCliError(
        "cannot find crates/consensus_cobalt; run this CLI from a postfiatl1v2 checkout"
    )


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_packet_bytes(path: Path, limit: int = MAX_REPORT_BYTES) -> bytes:
    try:
        if path.is_symlink() or not path.is_file():
            raise CobaltCliError(f"packet file is missing or not regular: {path.name}")
        size = path.stat().st_size
    except OSError as error:
        raise CobaltCliError(f"cannot inspect packet file {path.name}: {error}") from error
    if size > limit:
        raise CobaltCliError(f"packet file {path.name} exceeds the {limit}-byte read limit")
    try:
        return path.read_bytes()
    except OSError as error:
        raise CobaltCliError(f"cannot read packet file {path.name}: {error}") from error


def read_packet_json(path: Path) -> dict[str, Any]:
    payload = read_packet_bytes(path)
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CobaltCliError(f"packet file {path.name} is not valid JSON") from error
    if not isinstance(value, dict):
        raise CobaltCliError(f"packet file {path.name} is not a JSON object")
    return value


def verify_packet(
    packet_dir: Path,
    *,
    expected_manifest_sha256: str,
    expected_verifier_schema: str,
    required_files: set[str],
    required_checks: set[str],
    expected_verifier_sha256: str | None = None,
) -> dict[str, Any]:
    """Verify a flat evidence packet against a pinned checksum-manifest root."""

    packet_dir = packet_dir.resolve()
    if not packet_dir.is_dir():
        raise CobaltCliError(f"packet directory does not exist: {packet_dir}")
    manifest_path = packet_dir / "SHA256SUMS"
    manifest_payload = read_packet_bytes(manifest_path, MAX_CHECKSUM_MANIFEST_BYTES)
    manifest_sha256 = sha256_bytes(manifest_payload)
    if manifest_sha256 != expected_manifest_sha256:
        raise CobaltCliError(
            "packet checksum root mismatch: "
            f"expected {expected_manifest_sha256}, received {manifest_sha256}"
        )
    try:
        lines = manifest_payload.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise CobaltCliError("packet checksum manifest is not ASCII") from error
    if not lines or len(lines) > MAX_PACKET_FILES:
        raise CobaltCliError("packet checksum manifest has an invalid file count")

    checksums: dict[str, str] = {}
    for line in lines:
        digest, separator, name = line.partition("  ")
        path_name = PurePosixPath(name)
        if (
            separator != "  "
            or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
            or not name
            or path_name.is_absolute()
            or len(path_name.parts) != 1
            or path_name.parts[0] in {".", ".."}
            or name in checksums
        ):
            raise CobaltCliError("packet checksum manifest contains a malformed entry")
        checksums[name] = digest

    missing = sorted(required_files - checksums.keys())
    if missing:
        raise CobaltCliError(f"packet checksum manifest omits required files: {', '.join(missing)}")
    for name, digest in checksums.items():
        actual = sha256_bytes(read_packet_bytes(packet_dir / name))
        if actual != digest:
            raise CobaltCliError(f"packet checksum mismatch for {name}")

    verifier_path = packet_dir / "verifier.json"
    verifier_payload = read_packet_bytes(verifier_path)
    verifier_sha256 = sha256_bytes(verifier_payload)
    if expected_verifier_sha256 and verifier_sha256 != expected_verifier_sha256:
        raise CobaltCliError(
            "packet verifier hash mismatch: "
            f"expected {expected_verifier_sha256}, received {verifier_sha256}"
        )
    verifier = read_packet_json(verifier_path)
    if verifier.get("schema") != expected_verifier_schema:
        raise CobaltCliError("packet verifier schema is not the expected version")
    if verifier.get("result") != "passed":
        raise CobaltCliError("packet verifier did not report passed")
    verifier_checks = verifier.get("checks")
    if not isinstance(verifier_checks, dict):
        raise CobaltCliError("packet verifier checks are malformed")
    if not required_checks.issubset(verifier_checks):
        raise CobaltCliError("packet verifier omits required checks")
    if not verifier_checks or any(value is not True for value in verifier_checks.values()):
        raise CobaltCliError("packet verifier contains a failing check")
    declared_root = verifier.get("sha256sums_sha256")
    if declared_root is not None and declared_root != manifest_sha256:
        raise CobaltCliError("packet verifier does not bind the checksum manifest")

    return {
        "directory": str(packet_dir),
        "manifest_sha256": manifest_sha256,
        "verifier_sha256": verifier_sha256,
        "verifier": verifier,
    }


def scenario_result(
    packet_dir: Path,
    *,
    expected_manifest_sha256: str = DEFAULT_BENCHMARK_PACKET_SHA256,
) -> dict[str, Any]:
    authenticated = verify_packet(
        packet_dir,
        expected_manifest_sha256=expected_manifest_sha256,
        expected_verifier_schema=BENCHMARK_PACKET_SCHEMA,
        required_files=BENCHMARK_REQUIRED_FILES,
        required_checks=BENCHMARK_REQUIRED_CHECKS,
    )
    packet = Path(authenticated["directory"])
    verifier = authenticated["verifier"]
    cobalt_report = read_packet_json(packet / "cobalt-report.json")
    rippled_report = read_packet_json(packet / "rippled-report.json")
    kpi_report = read_packet_json(packet / "kpi-report.json")
    scenario_manifest = read_packet_json(packet / "scenario-manifest.json")
    headline = kpi_report.get("headline")
    methodology = kpi_report.get("methodology_boundaries")
    cases = scenario_manifest.get("cases")
    non_decisions = kpi_report.get("first_class_non_decision_outcomes")
    if (
        not isinstance(headline, dict)
        or not isinstance(methodology, dict)
        or not isinstance(cases, list)
        or not isinstance(non_decisions, list)
    ):
        raise CobaltCliError("benchmark KPI or scenario structure is malformed")
    required_methodology = {
        "agti_control",
        "authority",
        "latency",
        "local_quorum",
        "rippled_control",
    }
    methodology_complete = required_methodology.issubset(methodology) and all(
        isinstance(methodology[key], str) and methodology[key].strip()
        for key in required_methodology
    )
    case_count = len(cases)
    cobalt_passed = cobalt_report.get("passed_case_count")
    rippled_passed = rippled_report.get("passed_case_count")
    cobalt_conflicts = cobalt_report.get("conflicting_decision_count")
    rippled_conflicts = rippled_report.get("conflicting_decision_count")
    counts_match = (
        case_count > 0
        and cobalt_report.get("case_count") == case_count
        and rippled_report.get("case_count") == case_count
        and headline.get("case_count") == case_count
        and cobalt_passed == case_count
        and rippled_passed == case_count
    )
    zero_conflicts = cobalt_conflicts == 0 and rippled_conflicts == 0
    checks = verifier["checks"]
    ok = (
        counts_match
        and zero_conflicts
        and headline.get("all_declared_outcomes_passed") is True
        and methodology_complete
        and checks.get("cobalt_replay_equal") is True
        and checks.get("rippled_native_fork_control_present") is True
        and checks.get("cobalt_authority_disabled") is True
    )
    return {
        "schema": CLI_SCHEMA,
        "command": "scenario",
        "ok": ok,
        "authority": dict(AUTHORITY),
        "summary": {
            "case_count": case_count,
            "cobalt_passed": cobalt_passed,
            "rippled_passed": rippled_passed,
            "cobalt_conflicting_decisions": cobalt_conflicts,
            "rippled_conflicting_decisions": rippled_conflicts,
            "cobalt_replay_equal": checks.get("cobalt_replay_equal") is True,
            "rippled_native_fork_control": checks.get(
                "rippled_native_fork_control_present"
            )
            is True,
            "safe_halt_outcomes": sum(
                1
                for row in non_decisions
                if isinstance(row, dict)
                and (
                    row.get("cobalt", {}).get("safe_halt") is True
                    or row.get("rippled_csf", {}).get("safe_halt") is True
                )
            ),
            "unresolved_methodology_exception": not methodology_complete,
        },
        "source_pins": verifier.get("source_pins", {}),
        "methodology_boundaries": methodology,
        "packet": {
            key: authenticated[key]
            for key in ("directory", "manifest_sha256", "verifier_sha256")
        },
    }


def readiness_result(
    benchmark_packet: Path,
    handoff_packet: Path,
    *,
    benchmark_manifest_sha256: str = DEFAULT_BENCHMARK_PACKET_SHA256,
    handoff_manifest_sha256: str = DEFAULT_HANDOFF_PACKET_SHA256,
    handoff_verifier_sha256: str = DEFAULT_HANDOFF_VERIFIER_SHA256,
) -> dict[str, Any]:
    scenario = scenario_result(
        benchmark_packet, expected_manifest_sha256=benchmark_manifest_sha256
    )
    authenticated = verify_packet(
        handoff_packet,
        expected_manifest_sha256=handoff_manifest_sha256,
        expected_verifier_schema=HANDOFF_PACKET_SCHEMA,
        required_files=HANDOFF_REQUIRED_FILES,
        required_checks=HANDOFF_REQUIRED_CHECKS,
        expected_verifier_sha256=handoff_verifier_sha256,
    )
    packet = Path(authenticated["directory"])
    verifier = authenticated["verifier"]
    activation = read_packet_json(packet / "activation-result.json")
    update = read_packet_json(packet / "validator-update-result.json")
    abort = read_packet_json(packet / "pre-activation-abort.json")
    rollback = read_packet_json(packet / "forward-rollback-result.json")
    negative = read_packet_json(packet / "negative-cases.json")
    before_payload = read_packet_bytes(packet / "live-fleet-before.json")
    after_payload = read_packet_bytes(packet / "live-fleet-after.json")
    try:
        live_before = json.loads(before_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CobaltCliError("live fleet receipt is not valid JSON") from error
    if not isinstance(live_before, list) or not live_before:
        raise CobaltCliError("live fleet receipt is empty or malformed")
    live_authority_unchanged = before_payload == after_payload and all(
        isinstance(node, dict)
        and node.get("cobalt_shadow", {}).get("live_authority") is False
        and node.get("cobalt_shadow", {}).get("controls_block_consensus") is False
        for node in live_before
    )
    checks = [
        {
            "key": "matched_scenario",
            "label": "Matched Cobalt/RippleD scenario packet passes",
            "ok": scenario["ok"],
            "source": scenario["packet"]["manifest_sha256"],
        },
        {
            "key": "zero_conflicts_and_replay",
            "label": "Zero conflicting decisions and deterministic Cobalt replay",
            "ok": scenario["summary"]["cobalt_conflicting_decisions"] == 0
            and scenario["summary"]["rippled_conflicting_decisions"] == 0
            and scenario["summary"]["cobalt_replay_equal"] is True,
            "source": "matched benchmark verifier",
        },
        {
            "key": "handoff_rehearsal",
            "label": "Disposable authority handoff verifier passes",
            "ok": verifier.get("result") == "passed",
            "source": authenticated["manifest_sha256"],
        },
        {
            "key": "negative_abort_rollback",
            "label": "Unsafe handoffs reject; abort and forward rollback preserve rules",
            "ok": negative.get("all_rejected") is True
            and negative.get("durable_state_unchanged") is True
            and abort.get("accepted") is False
            and abort.get("applied") is False
            and abort.get("governance_commitment_before")
            == abort.get("governance_commitment_after")
            and rollback.get("accepted") is True
            and rollback.get("authority_mode_after") == 0,
            "source": "handoff negative/abort/rollback receipts",
        },
        {
            "key": "scoped_validator_update",
            "label": "Rehearsed Cobalt authority accepts only validator-trust update",
            "ok": activation.get("accepted") is True
            and activation.get("authority_mode_after") == 1
            and update.get("accepted") is True
            and update.get("operation") == "validator_trust_update"
            and bool(update.get("unrelated_governance_rejected")),
            "source": "disposable clone activation and update receipts",
        },
        {
            "key": "live_authority_unchanged",
            "label": "Live validators stayed on Foundation and Consensus v2",
            "ok": live_authority_unchanged,
            "source": "byte-identical live fleet receipts",
        },
    ]
    ready = all(check["ok"] for check in checks)
    return {
        "schema": CLI_SCHEMA,
        "command": "readiness",
        "ok": ready,
        "authority": dict(AUTHORITY),
        "status": "GO" if ready else "HOLD",
        "decision_scope": (
            "separately authorized controlled-testnet validator-trust cutover"
        ),
        "recommendation": (
            "GO_FOR_SEPARATELY_AUTHORIZED_CONTROLLED_TESTNET_CUTOVER"
            if ready
            else "HOLD_AND_REMEDIATE_FAILED_EVIDENCE"
        ),
        "activation_performed": False,
        "actual_authority": {
            "validator_trust": "foundation",
            "cobalt_active": False,
            "block_finality": "consensus-v2",
            "observed_validator_count": len(live_before),
        },
        "checks": checks,
        "scenario": scenario["summary"],
        "packets": {
            "benchmark": scenario["packet"],
            "handoff": {
                key: authenticated[key]
                for key in ("directory", "manifest_sha256", "verifier_sha256")
            },
        },
    }


def resolve_cargo(requested: str | None) -> str:
    candidate = requested or os.environ.get("CARGO") or shutil.which("cargo")
    if not candidate:
        user_local = Path.home() / ".cargo" / "bin" / "cargo"
        if user_local.is_file():
            candidate = str(user_local)
    if not candidate:
        raise CobaltCliError("cargo is required to run the Cobalt governance checks")
    return candidate


def cargo_command(spec: ExampleSpec, *, cargo: str, target: str | None) -> list[str]:
    command = [
        cargo,
        "run",
        "--quiet",
        "--package",
        "postfiat-consensus-cobalt",
        "--locked",
    ]
    if target:
        command.extend(["--target", target])
    command.extend(["--example", spec.example])
    if spec.feature:
        command.extend(["--features", spec.feature])
    return command


def run_example(
    spec: ExampleSpec,
    *,
    root: Path,
    cargo: str,
    target: str | None,
    timeout_seconds: float,
) -> dict[str, Any]:
    command = cargo_command(spec, cargo=cargo, target=target)
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            env=os.environ.copy(),
            capture_output=True,
            check=False,
            text=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise CobaltCliError(
            f"{spec.example} exceeded the {timeout_seconds:g}s execution limit"
        ) from error
    except OSError as error:
        raise CobaltCliError(f"could not execute cargo: {error}") from error

    if len(completed.stdout) > MAX_REPORT_BYTES or len(completed.stderr) > MAX_REPORT_BYTES:
        raise CobaltCliError(f"{spec.example} exceeded the bounded output limit")
    stdout = completed.stdout.decode("utf-8", errors="replace")
    stderr = completed.stderr.decode("utf-8", errors="replace").strip()
    if completed.returncode != 0:
        detail = stderr.rsplit("\n", 1)[-1] if stderr else "no diagnostic emitted"
        raise CobaltCliError(
            f"{spec.example} failed with exit code {completed.returncode}: {detail}"
        )
    try:
        report = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise CobaltCliError(f"{spec.example} did not emit one valid JSON report") from error
    if not isinstance(report, dict):
        raise CobaltCliError(f"{spec.example} emitted a non-object JSON report")
    return report


def run_shadow_service(
    action: str,
    *,
    root: Path,
    data_dir: Path,
    cargo: str,
    target: str | None,
    timeout_seconds: float,
) -> dict[str, Any]:
    command = [
        cargo,
        "run",
        "--quiet",
        "--package",
        "postfiat-node",
        "--bin",
        "postfiat-cobalt-shadow",
        "--locked",
    ]
    if target:
        command.extend(["--target", target])
    command.extend(["--", action, "--data-dir", str(data_dir)])
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            env=os.environ.copy(),
            capture_output=True,
            check=False,
            text=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise CobaltCliError(
            f"Cobalt shadow {action} exceeded the {timeout_seconds:g}s execution limit"
        ) from error
    except OSError as error:
        raise CobaltCliError(f"could not execute Cobalt shadow binary: {error}") from error
    if len(completed.stdout) > MAX_REPORT_BYTES or len(completed.stderr) > MAX_REPORT_BYTES:
        raise CobaltCliError("Cobalt shadow service exceeded the bounded output limit")
    stdout = completed.stdout.decode("utf-8", errors="replace")
    stderr = completed.stderr.decode("utf-8", errors="replace").strip()
    if completed.returncode != 0:
        detail = stderr.rsplit("\n", 1)[-1] if stderr else "no diagnostic emitted"
        raise CobaltCliError(
            f"Cobalt shadow {action} failed with exit code {completed.returncode}: {detail}"
        )
    try:
        report = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise CobaltCliError("Cobalt shadow service did not emit valid JSON") from error
    if not isinstance(report, dict):
        raise CobaltCliError("Cobalt shadow service emitted a non-object JSON report")
    return report


def shadow_result(command: str, report: dict[str, Any]) -> dict[str, Any]:
    if command == "shadow-service-drill":
        ok = report.get("ok") is True and report.get("status") == "passed"
    else:
        ok = (
            report.get("authority_mode") == "shadow-advisory"
            and report.get("live_authority") is False
            and report.get("controls_block_consensus") is False
            and report.get("transport_healthy") is True
        )
    return {
        "schema": CLI_SCHEMA,
        "command": command,
        "ok": ok,
        "authority": dict(AUTHORITY),
        "source": {
            "crate": "postfiat-node",
            "binary": "postfiat-cobalt-shadow",
            "action": SHADOW_COMMANDS[command],
        },
        "report": report,
    }


def runtime_request(
    endpoint: str,
    operation: str,
    *,
    timeout_seconds: float,
    fields: dict[str, Any] | None = None,
) -> dict[str, Any] | list[Any]:
    """Read one bounded JSON response from the Rust shadow socket service."""

    host, separator, port_text = endpoint.rpartition(":")
    if not separator or not host:
        raise CobaltCliError("endpoint must be HOST:PORT")
    try:
        port = int(port_text)
    except ValueError as error:
        raise CobaltCliError("endpoint port must be an integer") from error
    if not 1 <= port <= 65535:
        raise CobaltCliError("endpoint port must be in 1..=65535")
    request_object = {"operation": operation}
    if fields:
        request_object.update(fields)
    request_body = json.dumps(request_object, separators=(",", ":")).encode("utf-8")
    if len(request_body) > MAX_REPORT_BYTES:
        raise CobaltCliError("Cobalt shadow request exceeded the write limit")
    try:
        with socket.create_connection((host, port), timeout=timeout_seconds) as connection:
            connection.settimeout(timeout_seconds)
            connection.sendall(request_body)
            connection.shutdown(socket.SHUT_WR)
            chunks: list[bytes] = []
            size = 0
            while True:
                chunk = connection.recv(65536)
                if not chunk:
                    break
                size += len(chunk)
                if size > MAX_REPORT_BYTES:
                    raise CobaltCliError("Cobalt shadow response exceeded the read limit")
                chunks.append(chunk)
    except OSError as error:
        raise CobaltCliError(f"could not reach Cobalt shadow service: {error}") from error
    try:
        envelope = json.loads(b"".join(chunks))
    except json.JSONDecodeError as error:
        raise CobaltCliError("Cobalt shadow service did not emit valid JSON") from error
    if not isinstance(envelope, dict) or envelope.get("ok") is not True:
        detail = envelope.get("error", "request failed") if isinstance(envelope, dict) else "request failed"
        raise CobaltCliError(f"Cobalt shadow service rejected the request: {detail}")
    result = envelope.get("result")
    if not isinstance(result, (dict, list)):
        raise CobaltCliError("Cobalt shadow response did not contain structured output")
    return result


def runtime_result(command: str, report: dict[str, Any] | list[Any]) -> dict[str, Any]:
    if command == "probe":
        ok = (
            isinstance(report, dict)
            and report.get("peer_health") == "healthy"
            and report.get("queue_health") == "healthy"
            and report.get("replay_posture") == "consistent"
            and report.get("catch_up_status") == "current"
            and report.get("live_authority") is False
            and report.get("controls_block_consensus") is False
        )
    elif command == "snapshot":
        ok = (
            isinstance(report, dict)
            and report.get("authority_mode") == "shadow-advisory"
            and report.get("live_authority") is False
            and report.get("controls_block_consensus") is False
        )
    else:
        ok = isinstance(report, list) and all(
            isinstance(row, dict) and row.get("ratification_id") for row in report
        )
    return {
        "schema": CLI_SCHEMA,
        "command": command,
        "ok": ok,
        "authority": dict(AUTHORITY),
        "source": {"service": "postfiat-cobalt-shadow", "operation": command},
        "report": report,
    }


def fleet_result(endpoints: list[str], reports: list[dict[str, Any]]) -> dict[str, Any]:
    roots = {
        (report.get("registry_root"), report.get("trust_graph_root"))
        for report in reports
    }
    ok = bool(reports) and len(roots) == 1 and all(
        runtime_result("probe", report)["ok"] for report in reports
    )
    return {
        "schema": CLI_SCHEMA,
        "command": "fleet",
        "ok": ok,
        "authority": dict(AUTHORITY),
        "summary": {
            "reachable": len(reports),
            "configured": len(endpoints),
            "consistent_roots": len(roots) == 1,
            "live_authority": False,
        },
        "nodes": [
            {"endpoint": endpoint, "probe": report}
            for endpoint, report in zip(endpoints, reports, strict=True)
        ],
    }


def result_envelope(spec: ExampleSpec, report: dict[str, Any]) -> dict[str, Any]:
    if spec.command == "trust-graph":
        ok = bool(report.get("trust_graph_root")) and report.get("cobalt_mode") == "non_uniform"
    else:
        ok = report.get("ok") is True and report.get("status") == "passed"
    return {
        "schema": CLI_SCHEMA,
        "command": spec.command,
        "ok": ok,
        "authority": dict(AUTHORITY),
        "source": {
            "crate": "postfiat-consensus-cobalt",
            "example": spec.example,
            "feature": spec.feature,
        },
        "report": report,
    }


Runner = Callable[[ExampleSpec], dict[str, Any]]


def execute(command: str, runner: Runner) -> dict[str, Any]:
    if command != "shadow-readiness":
        spec = EXAMPLES[command]
        return result_envelope(spec, runner(spec))

    checks = [result_envelope(spec, runner(spec)) for spec in EXAMPLES.values()]
    return {
        "schema": CLI_SCHEMA,
        "command": command,
        "ok": all(check["ok"] for check in checks),
        "authority": dict(AUTHORITY),
        "checks": checks,
        "summary": {
            "passed": sum(1 for check in checks if check["ok"]),
            "total": len(checks),
            "live_authority": False,
            "recommendation": "continue shadow evaluation; no authority handoff implied",
        },
    }


def short_hash(value: Any) -> str:
    text = str(value or "unavailable")
    return text if len(text) <= 20 else f"{text[:12]}…{text[-6:]}"


def status_line(ok: bool) -> str:
    return "PASS" if ok else "FAIL"


def render_human(result: dict[str, Any]) -> str:
    command = result["command"]
    lines = [
        "Cobalt governance check",
        f"Status: {status_line(bool(result['ok']))}",
        "Authority: advisory only; live block consensus remains unchanged",
        "",
    ]
    if command == "fleet":
        summary = result["summary"]
        lines.extend(
            [
                "Cobalt shadow fleet",
                f"  Reachable: {summary['reachable']}/{summary['configured']}",
                f"  Registry/graph roots consistent: {'yes' if summary['consistent_roots'] else 'no'}",
                "  Live authority: no",
            ]
        )
        lines.extend(
            f"  [{status_line(node['probe'].get('peer_health') == 'healthy')}] "
            f"{node['probe'].get('node_id', node['endpoint'])} at {node['endpoint']}"
            for node in result["nodes"]
        )
    elif command == "probe":
        report = result["report"]
        lines.extend(
            [
                "Cobalt shadow probe",
                f"  Node: {report.get('node_id', 'unknown')}",
                f"  Peers: {report.get('configured_peers', 'unknown')} ({report.get('peer_health', 'unknown')})",
                f"  Queue: {report.get('queue_depth', 'unknown')} ({report.get('queue_health', 'unknown')})",
                f"  Registry root: {short_hash(report.get('registry_root'))}",
                f"  Trust graph root: {short_hash(report.get('trust_graph_root'))}",
                f"  Ratification locks: {report.get('ratification_locks', 'unknown')}",
                f"  Decisions: {report.get('protocol_decisions', 'unknown')}",
                f"  Contiguous history: {report.get('contiguous_sequence', 'unknown')}",
                f"  History head: {short_hash(report.get('history_head'))}",
                f"  Catch-up: {report.get('catch_up_status', 'unknown')}",
                f"  Certificate signers: {report.get('certificate_signer_count', 'unknown')}",
                f"  Replay: {report.get('replay_posture', 'unknown')}",
                f"  Signed traffic: {report.get('messages_received', 0)} messages / {report.get('bytes_received', 0)} bytes",
                "  Stage validation: "
                + ", ".join(
                    f"{stage}={micros}us"
                    for stage, micros in report.get("stage_validation_micros", {}).items()
                ),
                "  Live authority: no",
            ]
        )
    elif command == "snapshot":
        report = result["report"]
        lines.extend(
            [
                "Cobalt shadow snapshot",
                f"  Node: {report.get('identity', {}).get('node_id', 'unknown')}",
                f"  Registry root: {short_hash(report.get('registry_root'))}",
                f"  Trust graph root: {short_hash(report.get('trust_graph_root'))}",
                f"  Protocol high-water mark: {report.get('protocol_high_watermark', 'unknown')}",
                f"  Decisions: {len(report.get('protocol_decisions', {}))}",
                f"  State hash: {short_hash(report.get('state_hash'))}",
                "  Live authority: no",
            ]
        )
    elif command == "replay":
        report = result["report"]
        lines.extend(
            [
                "Cobalt shadow replay",
                f"  Decisions: {len(report)}",
                "  Live authority: no",
            ]
        )
        lines.extend(
            f"  [PASS] round {row.get('round')}: {short_hash(row.get('ratification_id'))}"
            for row in report
        )
    elif command in HISTORY_COMMANDS:
        report = result["report"]
        lines.extend(
            [
                f"Cobalt shadow {command}",
                f"  Start sequence: {report.get('start_sequence', 'unknown')}",
                f"  End sequence: {report.get('end_sequence', report.get('contiguous_sequence', 'unknown'))}",
                f"  History head: {short_hash(report.get('history_head', report.get('range_hash')))}",
                f"  Catch-up status: {report.get('catch_up_status', 'verified' if report.get('verified') else 'exported')}",
                "  Live authority: no",
            ]
        )
    elif command == "trust-graph":
        report = result["report"]
        lines.extend(
            [
                "Trust graph",
                f"  Mode: {report.get('cobalt_mode', 'unknown')}",
                f"  Active graph: {report.get('active_graph', 'unknown')}",
                f"  Validators/views: {report.get('g1_trust_view_count', 'unknown')}",
                f"  Root: {short_hash(report.get('trust_graph_root'))}",
                f"  Activation height: {report.get('g1_activation_height', 'unknown')}",
            ]
        )
    elif command == "transition-witness":
        report = result["report"]
        scenarios = report.get("scenarios", [])
        lines.extend(
            [
                "Transition witness",
                f"  Scenarios: {len(scenarios)}",
                f"  Passed: {sum(1 for row in scenarios if row.get('ok') is True)}",
                f"  Byzantine budget: {report.get('profile', {}).get('byzantine_budget', 'unknown')}",
                f"  Witness hash: {short_hash(report.get('scenario_hash'))}",
            ]
        )
        lines.extend(
            f"  [{status_line(row.get('ok') is True)}] {row.get('name')}: {row.get('reason')}"
            for row in scenarios
        )
    elif command == "protocol-replay":
        report = result["report"]
        scenarios = report.get("scenarios", [])
        lines.extend(
            [
                "Governance protocol replay",
                f"  Scope: {report.get('scope', 'unknown')}",
                f"  Validators: {report.get('validator_count', 'unknown')}",
                f"  Scenarios: {len(scenarios)}",
                f"  Passed: {sum(1 for row in scenarios if row.get('ok') is True)}",
            ]
        )
        lines.extend(
            f"  [{status_line(row.get('ok') is True)}] {row.get('name')}"
            for row in scenarios
        )
    elif command == "scenario":
        summary = result["summary"]
        lines.extend(
            [
                "Matched Cobalt/RippleD scenario evidence",
                f"  Cases: {summary['case_count']}",
                f"  Cobalt passed: {summary['cobalt_passed']}/{summary['case_count']}",
                f"  RippleD passed: {summary['rippled_passed']}/{summary['case_count']}",
                f"  Conflicting decisions: Cobalt={summary['cobalt_conflicting_decisions']} "
                f"RippleD={summary['rippled_conflicting_decisions']}",
                f"  Deterministic Cobalt replay: {'yes' if summary['cobalt_replay_equal'] else 'no'}",
                f"  RippleD native fork control: {'present' if summary['rippled_native_fork_control'] else 'missing'}",
                f"  Safe-halt outcomes: {summary['safe_halt_outcomes']}",
                f"  Methodology exception: {'unresolved' if summary['unresolved_methodology_exception'] else 'none'}",
                f"  Packet root: {result['packet']['manifest_sha256']}",
            ]
        )
    elif command == "readiness":
        actual = result["actual_authority"]
        lines.extend(
            [
                "Controlled-testnet Cobalt cutover decision",
                f"  Recommendation: {result['status']}",
                f"  Scope: {result['decision_scope']}",
                f"  Validator-trust authority now: {actual['validator_trust']}",
                f"  Block finality now: {actual['block_finality']}",
                "  Activation performed by this command: no",
            ]
        )
        lines.extend(
            f"  [{status_line(check['ok'])}] {check['label']} · {check['source']}"
            for check in result["checks"]
        )
    elif command == "shadow-service-status":
        report = result["report"]
        lines.extend(
            [
                "Shadow service",
                f"  Node: {report.get('node_id', 'unknown')}",
                f"  Signer: {report.get('signer_algorithm', 'unknown')} "
                f"({'loaded' if report.get('signer_private_key_loaded') else 'unavailable'})",
                f"  Peers: {report.get('peer_count', 'unknown')}",
                f"  Queue: {report.get('queue_depth', 'unknown')}",
                f"  Accepted messages: {report.get('accepted_messages', 'unknown')}",
                f"  Duplicate messages: {report.get('duplicate_messages', 'unknown')}",
                f"  Rejected messages: {report.get('rejected_messages', 'unknown')}",
                f"  Randomness rounds: {report.get('randomness_rounds', 'unknown')}",
                f"  Transport healthy: {'yes' if report.get('transport_healthy') else 'no'}",
                "  Live authority: no",
            ]
        )
    elif command == "shadow-service-drill":
        report = result["report"]
        lines.extend(
            [
                "Shadow service adversarial drill",
                f"  Validators: {report.get('validator_count', 'unknown')}",
                f"  Active randomness contributors: "
                f"{report.get('active_contributor_count', 'unknown')}",
                f"  Common randomness: {short_hash(report.get('common_randomness_hash'))}",
                f"  Governance digest: "
                f"{short_hash(report.get('converged_governance_digest'))}",
            ]
        )
        lines.extend(
            f"  [{status_line(value is True)}] {name.replace('_', ' ')}"
            for name, value in report.get("checks", {}).items()
        )
        lines.append("  Live authority: no")
    else:
        summary = result["summary"]
        lines.extend(
            [
                "Shadow readiness",
                f"  Checks passed: {summary['passed']}/{summary['total']}",
                "  Live authority: no",
                f"  Recommendation: {summary['recommendation']}",
            ]
        )
        lines.extend(
            f"  [{status_line(check['ok'])}] {check['command']} ({check['source']['example']})"
            for check in result["checks"]
        )
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit the stable JSON envelope")
    parser.add_argument("--cargo", help="path to cargo (defaults to CARGO or PATH)")
    parser.add_argument("--target", help="optional Rust target triple")
    parser.add_argument(
        "--timeout",
        type=float,
        default=120.0,
        help="per-example execution limit in seconds (default: 120)",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        help="Cobalt shadow-service state directory (required for shadow-service commands)",
    )
    parser.add_argument(
        "--endpoint",
        help="Cobalt shadow service endpoint for probe, snapshot, or replay (HOST:PORT)",
    )
    parser.add_argument(
        "--endpoints",
        help="comma-separated Cobalt shadow endpoints for the fleet command",
    )
    parser.add_argument("--source-endpoint", help="source Cobalt shadow endpoint")
    parser.add_argument("--target-endpoint", help="target Cobalt shadow endpoint")
    parser.add_argument("--start-sequence", type=int, help="first history sequence")
    parser.add_argument("--limit", type=int, default=64, help="bounded history range size")
    parser.add_argument("--range", dest="range_path", type=Path, help="history range JSON file")
    parser.add_argument("--output", type=Path, help="optional history export path")
    parser.add_argument(
        "--benchmark-packet",
        type=Path,
        default=Path("benchmarks/cobalt-rippled-liveness/packet"),
        help="matched liveness packet directory",
    )
    parser.add_argument(
        "--benchmark-sha256",
        default=DEFAULT_BENCHMARK_PACKET_SHA256,
        help="pinned SHA-256 of the benchmark SHA256SUMS file",
    )
    parser.add_argument(
        "--handoff-packet",
        type=Path,
        default=Path("benchmarks/cobalt-handoff-rehearsal/packet"),
        help="disposable handoff rehearsal packet directory",
    )
    parser.add_argument(
        "--handoff-sha256",
        default=DEFAULT_HANDOFF_PACKET_SHA256,
        help="pinned SHA-256 of the handoff SHA256SUMS file",
    )
    parser.add_argument(
        "--handoff-verifier-sha256",
        default=DEFAULT_HANDOFF_VERIFIER_SHA256,
        help="pinned SHA-256 of the handoff verifier",
    )
    parser.add_argument(
        "command",
        choices=[
            *EXAMPLES,
            "graph",
            "fleet",
            "scenario",
            "readiness",
            *RUNTIME_COMMANDS,
            *HISTORY_COMMANDS,
            "shadow-status",
            "shadow-readiness",
            *SHADOW_COMMANDS,
        ],
        help="governance check to run",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.timeout <= 0:
        raise SystemExit("--timeout must be greater than zero")
    try:
        root = repository_root()
        command = {
            "graph": "trust-graph",
            "shadow-status": "shadow-service-status",
        }.get(args.command, args.command)
        benchmark_packet = (
            args.benchmark_packet
            if args.benchmark_packet.is_absolute()
            else root / args.benchmark_packet
        )
        handoff_packet = (
            args.handoff_packet
            if args.handoff_packet.is_absolute()
            else root / args.handoff_packet
        )
        if command == "scenario":
            result = scenario_result(
                benchmark_packet,
                expected_manifest_sha256=args.benchmark_sha256,
            )
        elif command == "readiness":
            result = readiness_result(
                benchmark_packet,
                handoff_packet,
                benchmark_manifest_sha256=args.benchmark_sha256,
                handoff_manifest_sha256=args.handoff_sha256,
                handoff_verifier_sha256=args.handoff_verifier_sha256,
            )
        elif command in RUNTIME_COMMANDS:
            if not args.endpoint:
                raise CobaltCliError(f"--endpoint is required for {command}")
            result = runtime_result(
                command,
                runtime_request(args.endpoint, command, timeout_seconds=args.timeout),
            )
        elif command in HISTORY_COMMANDS:
            if not 1 <= args.limit <= 1024:
                raise CobaltCliError("--limit must be in 1..=1024")
            if command == "history-export":
                if not args.endpoint or args.start_sequence is None:
                    raise CobaltCliError(
                        "--endpoint and --start-sequence are required for history-export"
                    )
                report = runtime_request(
                    args.endpoint,
                    "history_range",
                    timeout_seconds=args.timeout,
                    fields={
                        "start_sequence": args.start_sequence,
                        "limit": args.limit,
                    },
                )
                if not isinstance(report, dict):
                    raise CobaltCliError("history export returned non-object output")
                if args.output:
                    args.output.write_text(
                        json.dumps(report, indent=2, sort_keys=True) + "\n",
                        encoding="utf-8",
                    )
                ok = bool(report.get("range_hash"))
            else:
                if command == "history-verify":
                    if not args.endpoint or args.range_path is None:
                        raise CobaltCliError(
                            "--endpoint and --range are required for history-verify"
                        )
                    if args.range_path.stat().st_size > MAX_REPORT_BYTES:
                        raise CobaltCliError("history range file exceeds the read limit")
                    range_report = json.loads(args.range_path.read_text(encoding="utf-8"))
                    report = runtime_request(
                        args.endpoint,
                        "verify_history_range",
                        timeout_seconds=args.timeout,
                        fields={"range": range_report},
                    )
                else:
                    if (
                        not args.source_endpoint
                        or not args.target_endpoint
                        or args.start_sequence is None
                    ):
                        raise CobaltCliError(
                            "--source-endpoint, --target-endpoint, and --start-sequence "
                            "are required for catch-up"
                        )
                    range_report = runtime_request(
                        args.source_endpoint,
                        "history_range",
                        timeout_seconds=args.timeout,
                        fields={
                            "start_sequence": args.start_sequence,
                            "limit": args.limit,
                        },
                    )
                    report = runtime_request(
                        args.target_endpoint,
                        "catch_up",
                        timeout_seconds=args.timeout,
                        fields={"range": range_report},
                    )
                if not isinstance(report, dict):
                    raise CobaltCliError(f"{command} returned non-object output")
                ok = report.get("verified") is True if command == "history-verify" else (
                    report.get("catch_up_status") == "current"
                )
            result = {
                "schema": CLI_SCHEMA,
                "command": command,
                "ok": ok,
                "authority": dict(AUTHORITY),
                "report": report,
            }
        elif command == "fleet":
            endpoints = [
                value.strip()
                for value in (args.endpoints or "").split(",")
                if value.strip()
            ]
            if not endpoints:
                raise CobaltCliError("--endpoints is required for fleet")
            reports = []
            for endpoint in endpoints:
                report = runtime_request(endpoint, "probe", timeout_seconds=args.timeout)
                if not isinstance(report, dict):
                    raise CobaltCliError("fleet probe returned non-object output")
                reports.append(report)
            result = fleet_result(endpoints, reports)
        elif command in SHADOW_COMMANDS:
            if args.data_dir is None:
                raise CobaltCliError(
                    "--data-dir is required for shadow-service-status and shadow-service-drill"
                )
            cargo = resolve_cargo(args.cargo)
            report = run_shadow_service(
                SHADOW_COMMANDS[command],
                root=root,
                data_dir=args.data_dir,
                cargo=cargo,
                target=args.target,
                timeout_seconds=args.timeout,
            )
            result = shadow_result(command, report)
        else:
            cargo = resolve_cargo(args.cargo)

            def runner(spec: ExampleSpec) -> dict[str, Any]:
                return run_example(
                    spec,
                    root=root,
                    cargo=cargo,
                    target=args.target,
                    timeout_seconds=args.timeout,
                )

            result = execute(command, runner)
    except CobaltCliError as error:
        print(f"cobalt governance check failed: {error}", file=sys.stderr)
        return 2

    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(render_human(result))
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
