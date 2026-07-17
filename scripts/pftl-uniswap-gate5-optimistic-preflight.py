#!/usr/bin/env python3
"""Validate Gate 5 optimistic verifier evidence without approving public launch."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


REQUIRED_PARAMETER_TABLE = {
    "packet_notional_cap",
    "invalid_profit_bound",
    "challenger_gas_cost_with_margin",
    "challenge_resolution_mode",
    "griefing_cost_bound",
    "policy_floor",
    "destination_finality",
    "proof_submission_margin",
    "watcher_liveness_slo",
}

REQUIRED_RUNBOOK_MARKERS = {
    "OPTIMISTIC",
    "public routing must remain disabled",
    "Detect posted claims within `60` seconds",
    "classify claims within `300` seconds",
    "at least `900` seconds",
    "Fail-Closed Triggers",
}

REQUIRED_CLAIM_LABELS = {"seed", "mint-only", "mint-and-swap"}


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def file_digest(path: Path) -> str:
    return hashlib.sha3_384(path.read_bytes()).hexdigest()


def is_hex(value: Any, length: int, prefixed: bool = False) -> bool:
    if not isinstance(value, str):
        return False
    text = value[2:] if prefixed and value.startswith("0x") else value
    if prefixed and not value.startswith("0x"):
        return False
    return len(text) == length and all(ch in "0123456789abcdef" for ch in text.lower())


def nested_value(value: dict[str, Any], *keys: str) -> Any:
    current: Any = value
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def validate_parameters(
    parameters: dict[str, Any],
    calibration: dict[str, Any],
    binding: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    if parameters.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-parameters-v2":
        errors.append("parameters schema mismatch")
    if parameters.get("route_trust_class") != "OPTIMISTIC":
        errors.append("parameters route_trust_class must be OPTIMISTIC")
    if parameters.get("status") != "fork_measured_and_launch_bound_not_public_launch_approval":
        errors.append("parameters status must remain pre-public")
    if parameters.get("optimistic_launch_binding_digest") != binding.get("binding_digest"):
        errors.append("parameters optimistic launch binding digest does not match binding report")

    table = parameters.get("parameter_table")
    if not isinstance(table, dict):
        errors.append("parameters parameter_table must be an object")
        table = {}
    missing = sorted(REQUIRED_PARAMETER_TABLE.difference(table))
    if missing:
        errors.append(f"parameters missing parameter_table entries: {', '.join(missing)}")

    selected = parameters.get("selected_constructor_values")
    if not isinstance(selected, dict):
        errors.append("parameters selected_constructor_values must be an object")
        selected = {}
    for key in [
        "poster_bond_wei",
        "challenger_bond_wei",
        "challenge_window_seconds",
        "challenge_resolution_window_seconds",
    ]:
        if str(selected.get(key)) != str(binding.get(key)):
            errors.append(f"parameters selected constructor {key} does not match binding report")
        if str(selected.get(key)) != str(calibration.get(key)):
            errors.append(f"parameters selected constructor {key} does not match calibration report")

    for parameter_key, calibration_key in [
        ("packet_notional_cap", ("packet_notional_cap", "value")),
        ("invalid_profit_bound", ("invalid_profit_bound", "value")),
        ("challenger_gas_cost_with_margin", ("challenge_gas_cost_with_margin_wei",)),
        ("griefing_cost_bound", ("griefing_cost_bound", "value")),
        ("policy_floor", ("policy_floor", "value")),
        ("destination_finality", ("destination_finality", "value")),
        ("proof_submission_margin", ("proof_submission_margin", "value")),
    ]:
        table_value = nested_value(table, parameter_key, "value")
        calibration_value = nested_value(calibration, *calibration_key)
        if str(table_value) != str(calibration_value):
            errors.append(f"parameters {parameter_key} does not match calibration report")

    if str(nested_value(table, "proof_submission_margin", "value")) != "900":
        errors.append("proof submission margin must be 900 seconds")
    if str(selected.get("challenge_resolution_window_seconds")) != "900":
        errors.append("challenge resolution window must be 900 seconds")
    if str(binding.get("challenge_gas_cost_with_4x_margin_wei")) != str(calibration.get("challenge_gas_cost_with_margin_wei")):
        errors.append("binding challenge gas margin does not match calibration report")
    if parameters.get("challenge_resolution_mode") != binding.get("challenge_resolution_mode"):
        errors.append("parameters challenge_resolution_mode does not match binding report")
    table_resolution_mode = nested_value(table, "challenge_resolution_mode", "value")
    if str(table_resolution_mode) != str(binding.get("challenge_resolution_mode")):
        errors.append("parameters challenge_resolution_mode table value does not match binding report")

    fail_closed = parameters.get("fail_closed_requirements")
    if not isinstance(fail_closed, list) or len(fail_closed) < 3:
        errors.append("parameters fail_closed_requirements must list the Gate 5 fail-closed requirements")
    if parameters.get("remaining_before_public_launch") == []:
        errors.append("parameters must keep remaining_before_public_launch blockers")
    return errors


def validate_watcher_slo(slo: dict[str, Any], runbook_text: str, binding: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if slo.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-watcher-slo-evidence-v1":
        errors.append("watcher SLO schema mismatch")
    if slo.get("route_trust_class") != "OPTIMISTIC":
        errors.append("watcher SLO route_trust_class must be OPTIMISTIC")

    slo_values = slo.get("slo")
    if not isinstance(slo_values, dict):
        errors.append("watcher SLO values must be an object")
        slo_values = {}
    expected = {
        "claim_detection_seconds": 60,
        "claim_classification_seconds": 300,
        "proof_submission_margin_seconds": 900,
        "challenge_resolution_window_seconds": int(binding.get("challenge_resolution_window_seconds", -1)),
    }
    for key, value in expected.items():
        if slo_values.get(key) != value:
            errors.append(f"watcher SLO {key} must be {value}")

    checked = slo.get("checked_results")
    if not isinstance(checked, dict):
        errors.append("watcher checked_results must be an object")
        checked = {}
    for key in [
        "valid_challenge_blocks_settlement",
        "valid_challenge_resolution_keeps_claim_unaccepted",
        "unchallenged_valid_claims_finalize",
        "unresolved_challenge_fails_closed",
    ]:
        if checked.get(key) is not True:
            errors.append(f"watcher checked result {key} must be true")
    if checked.get("public_routing_enabled") is not False:
        errors.append("watcher checked result public_routing_enabled must be false")
    if checked.get("trustless_claim_allowed") is not False:
        errors.append("watcher checked result trustless_claim_allowed must be false")

    missing_markers = sorted(marker for marker in REQUIRED_RUNBOOK_MARKERS if marker not in runbook_text)
    if missing_markers:
        errors.append(f"watcher runbook missing required markers: {', '.join(missing_markers)}")
    if not slo.get("remaining_before_public_launch"):
        errors.append("watcher SLO evidence must keep production launch blockers")
    return errors


def validate_fork_reports(
    claims: dict[str, Any],
    challenge: dict[str, Any],
    calibration: dict[str, Any],
    binding: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
    if claims.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-claims-evidence-v1":
        errors.append("claims report schema mismatch")
    if claims.get("route_trust_class") != "OPTIMISTIC":
        errors.append("claims report route_trust_class must be OPTIMISTIC")
    claim_rows = claims.get("claims")
    if not isinstance(claim_rows, list):
        errors.append("claims report claims must be an array")
        claim_rows = []
    labels = {row.get("label") for row in claim_rows if isinstance(row, dict)}
    missing_labels = sorted(REQUIRED_CLAIM_LABELS.difference(labels))
    if missing_labels:
        errors.append(f"claims report missing labels: {', '.join(missing_labels)}")
    for index, row in enumerate(claim_rows):
        if not isinstance(row, dict):
            errors.append(f"claims[{index}] must be an object")
            continue
        if row.get("accepted_before_post") is not False:
            errors.append(f"claim {row.get('label', index)} must be unaccepted before post")
        if row.get("accepted_during_challenge_window") is not False:
            errors.append(f"claim {row.get('label', index)} must be unaccepted during challenge window")
        if row.get("accepted_after_finalize") is not True:
            errors.append(f"claim {row.get('label', index)} must finalize after the window")
        for key in ["packet_digest", "claim_id", "post_tx", "finalize_tx"]:
            if not is_hex(row.get(key), 64, prefixed=True):
                errors.append(f"claim {row.get('label', index)} {key} is missing or malformed")

    if challenge.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-challenge-evidence-v1":
        errors.append("challenge report schema mismatch")
    if challenge.get("accepted_after_challenge") is not False:
        errors.append("challenged claim must remain unaccepted after challenge")
    if challenge.get("accepted_after_valid_challenge_resolution") is not False:
        errors.append("valid challenge resolution must keep claim unaccepted")
    if challenge.get("consume_after_valid_challenge_rejected") is not True:
        errors.append("controller consume must reject after valid challenge")
    if not is_hex(challenge.get("challenge_evidence_hash"), 64, prefixed=True):
        errors.append("challenge evidence hash must be nonzero bytes32")
    if challenge.get("challenge_evidence_hash") == "0x" + "0" * 64:
        errors.append("challenge evidence hash must be nonzero")
    if str(challenge.get("challenge_gas_cost_with_4x_margin_wei")) != str(calibration.get("challenge_gas_cost_with_margin_wei")):
        errors.append("challenge gas margin does not match calibration report")
    if str(challenge.get("challenge_gas_cost_with_4x_margin_wei")) != str(binding.get("challenge_gas_cost_with_4x_margin_wei")):
        errors.append("challenge gas margin does not match launch binding")
    return errors


def validate_binding(binding: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if binding.get("schema") != "postfiat-pftl-uniswap-gate5-optimistic-launch-binding-v1":
        errors.append("launch binding schema mismatch")
    if binding.get("route_trust_class") != "OPTIMISTIC":
        errors.append("launch binding route_trust_class must be OPTIMISTIC")
    if binding.get("verifier_mode") != "optimistic":
        errors.append("launch binding verifier mode must be optimistic")
    if binding.get("public_routing_enabled") is not False:
        errors.append("launch binding public_routing_enabled must be false")
    if binding.get("trustless_claim_allowed") is not False:
        errors.append("launch binding trustless_claim_allowed must be false")
    if binding.get("challenge_resolution_mode") != "owner_arbitrated":
        errors.append("launch binding challenge_resolution_mode must disclose owner_arbitrated")
    if not is_hex(binding.get("binding_digest"), 96):
        errors.append("launch binding digest is missing or malformed")
    for key in ["verifier", "controller", "replay_registry", "challenge_resolver", "resolver_owner"]:
        if not is_hex(binding.get(key), 40, prefixed=True):
            errors.append(f"launch binding {key} is missing or malformed")
    return errors


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    parameters_path = args.parameters.resolve()
    slo_path = args.watcher_slo.resolve()
    runbook_path = args.runbook.resolve()
    claims_path = args.claims_report.resolve()
    challenge_path = args.challenge_report.resolve()
    calibration_path = args.calibration_report.resolve()
    binding_path = args.binding_report.resolve()

    parameters = load_json(parameters_path)
    slo = load_json(slo_path)
    runbook_text = runbook_path.read_text(encoding="utf-8")
    claims = load_json(claims_path)
    challenge = load_json(challenge_path)
    calibration = load_json(calibration_path)
    binding = load_json(binding_path)

    errors: list[str] = []
    errors.extend(validate_binding(binding))
    errors.extend(validate_parameters(parameters, calibration, binding))
    errors.extend(validate_watcher_slo(slo, runbook_text, binding))
    errors.extend(validate_fork_reports(claims, challenge, calibration, binding))

    preflight_passed = not errors
    blockers = [
        "missing final manager approval of optimistic launch binding digest",
        "missing production watcher service run against final deployed verifier/controller/replay registry addresses",
        "missing Gate 6 final deployment digest and release approval",
    ]
    return {
        "schema": "postfiat-pftl-uniswap-gate5-optimistic-preflight-report-v1",
        "status": (
            "gate5_optimistic_preflight_passed_public_launch_blocked"
            if preflight_passed
            else "gate5_optimistic_preflight_failed"
        ),
        "route_id": binding.get("route_id") or parameters.get("route_id"),
        "route_trust_class": binding.get("route_trust_class"),
        "optimistic_launch_binding_digest": binding.get("binding_digest"),
        "parameters_digest": file_digest(parameters_path),
        "watcher_slo_digest": file_digest(slo_path),
        "watcher_runbook_digest": file_digest(runbook_path),
        "claims_report_digest": file_digest(claims_path),
        "challenge_report_digest": file_digest(challenge_path),
        "calibration_report_digest": file_digest(calibration_path),
        "binding_report_digest": file_digest(binding_path),
        "claim_labels": sorted(REQUIRED_CLAIM_LABELS),
        "watcher_slo": slo.get("slo"),
        "preflight_passed": preflight_passed,
        "public_launch_ready": False,
        "errors": errors,
        "public_launch_blockers": blockers,
    }


def run_self_test() -> None:
    root = repo_root()
    report = build_report(default_args(root))
    if not report["preflight_passed"]:
        raise AssertionError(f"default evidence failed self-test: {report['errors']}")
    if report["public_launch_ready"]:
        raise AssertionError("Gate 5 preflight must not mark public launch ready")


def default_args(root: Path) -> argparse.Namespace:
    evidence_dir = root / "docs" / "evidence" / "pftl-uniswap-gate5-optimistic-2026-07-01"
    fork_dir = root / "docs" / "evidence" / "pftl-uniswap-gate5-optimistic-fork-2026-07-01"
    return argparse.Namespace(
        parameters=evidence_dir / "parameters.json",
        watcher_slo=evidence_dir / "watcher-slo-evidence.json",
        runbook=evidence_dir / "watcher-runbook.md",
        claims_report=fork_dir / "reports" / "13-optimistic-claims.json",
        challenge_report=fork_dir / "reports" / "14-optimistic-challenge.json",
        calibration_report=fork_dir / "reports" / "15-gate5-parameters-calibration.json",
        binding_report=fork_dir / "reports" / "17-optimistic-launch-config-binding.json",
        output=evidence_dir / "reports" / "gate5-optimistic-preflight.json",
        self_test=False,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    defaults = default_args(repo_root())
    parser.add_argument("--self-test", action="store_true", help="run validator self-tests and exit")
    parser.add_argument("--parameters", type=Path, default=defaults.parameters)
    parser.add_argument("--watcher-slo", type=Path, default=defaults.watcher_slo)
    parser.add_argument("--runbook", type=Path, default=defaults.runbook)
    parser.add_argument("--claims-report", type=Path, default=defaults.claims_report)
    parser.add_argument("--challenge-report", type=Path, default=defaults.challenge_report)
    parser.add_argument("--calibration-report", type=Path, default=defaults.calibration_report)
    parser.add_argument("--binding-report", type=Path, default=defaults.binding_report)
    parser.add_argument("--output", type=Path, default=defaults.output)
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
    return 0 if report["preflight_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
