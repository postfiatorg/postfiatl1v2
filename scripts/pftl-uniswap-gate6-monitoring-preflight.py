#!/usr/bin/env python3
"""Validate Gate 6 monitoring/runbook evidence without enabling public routing."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any


REQUIRED_ALERT_IDS = {
    "stale_proof",
    "route_pause",
    "cap_exhaustion",
    "verifier_issue",
    "challenge_event",
    "replay_rejection",
    "pool_liquidity_drop",
}

REQUIRED_ALERT_FIELDS = {
    "id",
    "enabled",
    "severity",
    "signal",
    "threshold",
    "fail_closed_action",
}

REQUIRED_RUNBOOK_MARKERS = {
    "stale proof",
    "route pause",
    "cap exhaustion",
    "verifier issue",
    "challenge event",
    "replay rejection",
    "pool liquidity drop",
    "not trustless",
    "public routing remains disabled",
}


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def canonical_digest(value: dict[str, Any]) -> str:
    digest_value = copy.deepcopy(value)
    digest_value.pop("monitoring_config_digest", None)
    encoded = json.dumps(digest_value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha3_384(encoded).hexdigest()


def file_digest(path: Path) -> str:
    return hashlib.sha3_384(path.read_bytes()).hexdigest()


def is_hex(value: Any, length: int) -> bool:
    return isinstance(value, str) and len(value) == length and all(ch in "0123456789abcdef" for ch in value.lower())


def is_prefixed_hex(value: Any, length: int) -> bool:
    if not isinstance(value, str) or not value.startswith("0x"):
        return False
    text = value[2:]
    return len(text) == length and all(ch in "0123456789abcdef" for ch in text.lower())


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def validate_monitoring_config(config: dict[str, Any], binding: dict[str, Any], runbook_text: str) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    blockers: list[str] = []

    if config.get("schema") != "postfiat-pftl-uniswap-gate6-monitoring-config-v1":
        errors.append("monitoring config schema mismatch")
    if config.get("route_trust_class") != "OPTIMISTIC":
        errors.append("Gate 6 monitoring config must be OPTIMISTIC for this selected path")
    if config.get("trustless_claim_allowed") is not False:
        errors.append("optimistic Gate 6 config must explicitly disallow trustless claims")

    binding_digest = binding.get("binding_digest")
    if not is_hex(binding_digest, 96):
        errors.append("Gate 5 optimistic binding digest is missing or malformed")
    if config.get("optimistic_launch_binding_digest") != binding_digest:
        errors.append("monitoring config optimistic binding digest does not match Gate 5 binding")
    if binding.get("challenge_resolution_mode") != "owner_arbitrated":
        errors.append("Gate 5 binding challenge_resolution_mode must disclose owner_arbitrated")
    if config.get("challenge_resolution_mode") != binding.get("challenge_resolution_mode"):
        errors.append("monitoring config challenge_resolution_mode does not match Gate 5 binding")
    for key in ["replay_registry", "challenge_resolver", "resolver_owner"]:
        if not is_prefixed_hex(binding.get(key), 40):
            errors.append(f"Gate 5 binding {key} is missing or malformed")
        if config.get(key) != binding.get(key):
            errors.append(f"monitoring config {key} does not match Gate 5 binding")

    if config.get("public_routing_enabled") is not False:
        errors.append("public routing must remain disabled in this pre-public evidence package")

    alerts = config.get("monitor_alerts")
    if not isinstance(alerts, list):
        errors.append("monitor_alerts must be an array")
        alerts = []

    seen_alerts: set[str] = set()
    for index, alert in enumerate(alerts):
        if not isinstance(alert, dict):
            errors.append(f"monitor_alerts[{index}] must be an object")
            continue
        missing_fields = sorted(REQUIRED_ALERT_FIELDS.difference(alert))
        if missing_fields:
            errors.append(f"monitor_alerts[{index}] missing fields: {', '.join(missing_fields)}")
        alert_id = alert.get("id")
        if isinstance(alert_id, str):
            seen_alerts.add(alert_id)
        if alert.get("enabled") is not True:
            errors.append(f"monitor alert {alert_id or index} is not enabled")
        if not str(alert.get("fail_closed_action", "")).strip():
            errors.append(f"monitor alert {alert_id or index} has empty fail_closed_action")

    missing_alerts = sorted(REQUIRED_ALERT_IDS.difference(seen_alerts))
    if missing_alerts:
        errors.append(f"missing required Gate 6 alert ids: {', '.join(missing_alerts)}")

    runbook_lower = runbook_text.lower()
    missing_markers = sorted(marker for marker in REQUIRED_RUNBOOK_MARKERS if marker not in runbook_lower)
    if missing_markers:
        errors.append(f"runbook missing required markers: {', '.join(missing_markers)}")

    deployment = config.get("deployment")
    if not isinstance(deployment, dict):
        errors.append("deployment must be an object")
        deployment = {}

    if not is_hex(deployment.get("final_deployment_digest"), 96):
        blockers.append("missing final deployment digest")
    if not non_empty_string(deployment.get("release_owner_approval")):
        blockers.append("missing release-owner approval")
    if not non_empty_string(deployment.get("production_watcher_run_report")):
        blockers.append("missing production watcher run report against final deployed verifier/controller/replay registry addresses")
    if not non_empty_string(deployment.get("monitor_alert_delivery_report")):
        blockers.append("missing monitor alert delivery report")

    return errors, blockers


def validate_wallet_acceptance_evidence(wallet_evidence_path: Path) -> tuple[list[str], dict[str, str]]:
    errors: list[str] = []
    digests: dict[str, str] = {}
    if not wallet_evidence_path.exists():
        return [f"wallet acceptance evidence missing: {wallet_evidence_path}"], digests

    readme_text = wallet_evidence_path.read_text(encoding="utf-8")
    digests["readme"] = file_digest(wallet_evidence_path)
    readme_lower = readme_text.lower()
    for marker in [
        "wallet/proxy route acceptance policy",
        "wallet-proxy-navswap-adapter.txt",
        "three-way agreement",
        "finality_pending",
        "disabled",
    ]:
        if marker not in readme_lower:
            errors.append(f"wallet acceptance README missing marker: {marker}")

    reports_dir = wallet_evidence_path.parent / "reports"
    reports = {
        "wallet-route-acceptance.tap": ["# tests 54", "# pass 54", "# fail 0"],
        "wallet-npm-test.tap": ["# tests 169", "# pass 169", "# fail 0"],
        "wallet-proxy-navswap-adapter.txt": ["navswap adapter tests passed"],
    }
    for name, markers in reports.items():
        path = reports_dir / name
        if not path.exists():
            errors.append(f"wallet acceptance report missing: reports/{name}")
            continue
        text = path.read_text(encoding="utf-8")
        digests[name] = file_digest(path)
        for marker in markers:
            if marker not in text:
                errors.append(f"wallet acceptance report reports/{name} missing marker: {marker}")

    return errors, digests


def validate_gate5_preflight(preflight_path: Path, binding: dict[str, Any]) -> tuple[list[str], dict[str, Any]]:
    errors: list[str] = []
    if not preflight_path.exists():
        return [f"Gate 5 optimistic preflight missing: {preflight_path}"], {}
    preflight = load_json(preflight_path)
    if preflight.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-preflight-report-v1":
        errors.append("Gate 5 optimistic preflight schema mismatch")
    if preflight.get("preflight_passed") is not True:
        errors.append("Gate 5 optimistic preflight must pass before Gate 6 monitoring preflight")
    if preflight.get("public_launch_ready") is not False:
        errors.append("Gate 5 optimistic preflight must not claim public launch readiness")
    if preflight.get("optimistic_launch_binding_digest") != binding.get("binding_digest"):
        errors.append("Gate 5 optimistic preflight binding digest does not match binding report")
    if not preflight.get("public_launch_blockers"):
        errors.append("Gate 5 optimistic preflight must preserve public launch blockers")
    return errors, preflight


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    config_path = args.alerts_config.resolve()
    runbook_path = args.runbook.resolve()
    binding_path = args.gate5_binding.resolve()
    gate5_preflight_path = args.gate5_preflight.resolve()
    wallet_evidence_path = args.wallet_evidence.resolve()

    config = load_json(config_path)
    binding = load_json(binding_path)
    runbook_text = runbook_path.read_text(encoding="utf-8")

    errors, blockers = validate_monitoring_config(config, binding, runbook_text)
    gate5_errors, gate5_preflight = validate_gate5_preflight(gate5_preflight_path, binding)
    errors.extend(gate5_errors)
    wallet_errors, wallet_digests = validate_wallet_acceptance_evidence(wallet_evidence_path)
    errors.extend(wallet_errors)
    monitoring_ok = not errors
    public_launch_ready = monitoring_ok and not blockers

    return {
        "schema": "postfiat-pftl-uniswap-gate6-monitoring-preflight-report-v1",
        "status": (
            "public_launch_ready"
            if public_launch_ready
            else "monitoring_preflight_passed_public_launch_blocked"
            if monitoring_ok
            else "monitoring_preflight_failed"
        ),
        "route_id": config.get("route_id"),
        "route_trust_class": config.get("route_trust_class"),
        "public_routing_enabled": config.get("public_routing_enabled"),
        "trustless_claim_allowed": config.get("trustless_claim_allowed"),
        "optimistic_launch_binding_digest": config.get("optimistic_launch_binding_digest"),
        "gate5_binding_digest": binding.get("binding_digest"),
        "gate5_preflight_evidence": str(gate5_preflight_path.relative_to(repo_root())),
        "gate5_preflight_digest": file_digest(gate5_preflight_path) if gate5_preflight_path.exists() else None,
        "gate5_preflight_passed": not gate5_errors,
        "gate5_preflight_status": gate5_preflight.get("status"),
        "monitoring_config_digest": canonical_digest(config),
        "runbook_digest": file_digest(runbook_path),
        "wallet_acceptance_evidence": str(wallet_evidence_path.relative_to(repo_root())),
        "wallet_acceptance_preflight_passed": not wallet_errors,
        "wallet_acceptance_report_digests": wallet_digests,
        "required_alert_ids": sorted(REQUIRED_ALERT_IDS),
        "present_alert_ids": sorted(
            alert.get("id")
            for alert in config.get("monitor_alerts", [])
            if isinstance(alert, dict) and isinstance(alert.get("id"), str)
        ),
        "monitoring_preflight_passed": monitoring_ok,
        "public_launch_ready": public_launch_ready,
        "errors": errors,
        "public_launch_blockers": blockers,
    }


def run_self_test() -> None:
    binding = {
        "binding_digest": "a" * 96,
        "challenge_resolution_mode": "owner_arbitrated",
        "replay_registry": "0x" + "3" * 40,
        "challenge_resolver": "0x" + "1" * 40,
        "resolver_owner": "0x" + "2" * 40,
    }
    valid = {
        "schema": "postfiat-pftl-uniswap-gate6-monitoring-config-v1",
        "route_trust_class": "OPTIMISTIC",
        "public_routing_enabled": False,
        "trustless_claim_allowed": False,
        "optimistic_launch_binding_digest": "a" * 96,
        "challenge_resolution_mode": "owner_arbitrated",
        "replay_registry": "0x" + "3" * 40,
        "challenge_resolver": "0x" + "1" * 40,
        "resolver_owner": "0x" + "2" * 40,
        "deployment": {},
        "monitor_alerts": [
            {
                "id": alert_id,
                "enabled": True,
                "severity": "page",
                "signal": alert_id,
                "threshold": "nonzero",
                "fail_closed_action": "pause route",
            }
            for alert_id in sorted(REQUIRED_ALERT_IDS)
        ],
    }
    runbook = "\n".join(sorted(REQUIRED_RUNBOOK_MARKERS))
    errors, blockers = validate_monitoring_config(valid, binding, runbook)
    if errors:
        raise AssertionError(f"valid self-test config failed: {errors}")
    if not blockers:
        raise AssertionError("self-test expected launch blockers without deployment approvals")

    invalid = copy.deepcopy(valid)
    invalid["monitor_alerts"] = []
    errors, _ = validate_monitoring_config(invalid, binding, runbook)
    if not any("missing required Gate 6 alert ids" in error for error in errors):
        raise AssertionError("self-test expected missing alert ids to fail")


def parse_args() -> argparse.Namespace:
    root = repo_root()
    evidence_dir = root / "docs" / "evidence" / "pftl-uniswap-gate6-monitoring-2026-07-01"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="run validator self-tests and exit")
    parser.add_argument(
        "--alerts-config",
        type=Path,
        default=evidence_dir / "monitoring-alerts.json",
        help="Gate 6 monitoring alert config JSON",
    )
    parser.add_argument(
        "--runbook",
        type=Path,
        default=evidence_dir / "public-release-runbook.md",
        help="Gate 6 public release runbook",
    )
    parser.add_argument(
        "--gate5-binding",
        type=Path,
        default=root
        / "docs"
        / "evidence"
        / "pftl-uniswap-gate5-optimistic-fork-2026-07-01"
        / "reports"
        / "17-optimistic-launch-config-binding.json",
        help="Gate 5 optimistic launch binding report",
    )
    parser.add_argument(
        "--gate5-preflight",
        type=Path,
        default=root
        / "docs"
        / "evidence"
        / "pftl-uniswap-gate5-optimistic-2026-07-01"
        / "reports"
        / "gate5-optimistic-preflight.json",
        help="Gate 5 optimistic preflight report",
    )
    parser.add_argument(
        "--wallet-evidence",
        type=Path,
        default=root / "docs" / "evidence" / "pftl-uniswap-gate6-wallet-acceptance-2026-07-01" / "README.md",
        help="Gate 6 wallet acceptance evidence README",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=evidence_dir / "reports" / "gate6-monitoring-preflight.json",
        help="output report path",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        print("self-test ok")
        return 0

    report = build_report(args)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["monitoring_preflight_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
