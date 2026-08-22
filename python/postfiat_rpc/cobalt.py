"""Inspect Cobalt governance safety without granting it consensus authority.

Run from the repository root with::

    PYTHONPATH=python python3 -m postfiat_rpc.cobalt trust-graph

The CLI executes the existing ``postfiat-consensus-cobalt`` examples. It does
not reimplement their safety rules, initialize a node, process blocks, or alter
the validator registry.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Sequence


CLI_SCHEMA = "postfiat-cobalt-governance-cli-v1"
MAX_REPORT_BYTES = 4 * 1024 * 1024
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
    if command == "trust-graph":
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
        "command",
        choices=[*EXAMPLES, "shadow-readiness", *SHADOW_COMMANDS],
        help="governance check to run",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.timeout <= 0:
        raise SystemExit("--timeout must be greater than zero")
    try:
        root = repository_root()
        cargo = resolve_cargo(args.cargo)

        def runner(spec: ExampleSpec) -> dict[str, Any]:
            return run_example(
                spec,
                root=root,
                cargo=cargo,
                target=args.target,
                timeout_seconds=args.timeout,
            )

        if args.command in SHADOW_COMMANDS:
            if args.data_dir is None:
                raise CobaltCliError(
                    "--data-dir is required for shadow-service-status and shadow-service-drill"
                )
            report = run_shadow_service(
                SHADOW_COMMANDS[args.command],
                root=root,
                data_dir=args.data_dir,
                cargo=cargo,
                target=args.target,
                timeout_seconds=args.timeout,
            )
            result = shadow_result(args.command, report)
        else:
            result = execute(args.command, runner)
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
