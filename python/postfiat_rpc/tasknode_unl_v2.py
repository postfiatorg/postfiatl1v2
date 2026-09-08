"""Read-only, explicitly selected UNL V2 shadow CLI and report renderer.

This module consumes local Section-A evidence and a closed Section-B admission
packet.  It cannot submit, ratify, fund, contact a network, or mutate a live
validator registry.  The V1 side of each comparison is a root-bound reference
supplied in the same input packet; V1 inputs are never reinterpreted as V2.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Mapping, Sequence

from .tasknode_unl_schema import (
    SHADOW_MODE,
    TaskNodeUnlError,
    canonical_json_bytes,
    require_closed_keys,
    require_int,
)
from .tasknode_unl_v2_evidence import verify_evidence_snapshot
from .tasknode_unl_v2_policy import (
    EXISTING_BREACH,
    PROPOSE_ADD,
    AdmissionCandidate,
    PriorBreach,
    RegistryRoundState,
    RegistrySeat,
    admission_policy_document,
    admission_policy_root,
    evaluate_admission_round,
    freeze_admission_window,
)
from .tasknode_unl_v2_schema import (
    EVIDENCE_LIMITATIONS,
    MAX_CANONICAL_DOCUMENT_BYTES,
    MAX_RECORDS,
    V2_VERSION,
    require_array,
    require_bounded_identifier,
    require_lower_hex,
    require_v2_object,
)

CLI_ADMISSION_INPUT_SCHEMA = "tasknode-unl-v2-cli-admission-input-v2"
CLI_EVIDENCE_BUNDLE_SCHEMA = "tasknode-unl-v2-cli-evidence-bundle-v2"
CLI_REPORT_SCHEMA = "tasknode-unl-v2-cli-report-v2"
V1_REFERENCE_SCHEMA = "tasknode-unl-v1-admission-reference-v1"

CLI_INPUT_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/cli-input-root/v2"
V1_REFERENCE_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/v1-reference-root/v2"
CLI_REPORT_ROOT_DOMAIN = "postfiat/tasknode-unl-v2/cli-report-root/v2"

_GOLDEN_EVIDENCE_SCHEMA = "tasknode-unl-v2-evidence-golden-v1"
_POLICY_VERSION = "v2"
_MAX_CANDIDATES = 512
_V1_ACTIONS = frozenset(("admit", "hold", "reject", "no_proposal"))


def _domain_hash(domain: str, value: object) -> str:
    return hashlib.sha256(
        domain.encode("utf-8") + b"\x00" + canonical_json_bytes(value)
    ).hexdigest()


def _json_value(value: object) -> Any:
    """Materialize canonical types as the exact JSON shape emitted on disk."""

    return json.loads(canonical_json_bytes(value))


def _read_json(path: Path) -> Any:
    size = path.stat().st_size
    if size > MAX_CANONICAL_DOCUMENT_BYTES:
        raise TaskNodeUnlError("input_file_too_large", str(path))
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TaskNodeUnlError("invalid_json", str(path)) from exc


def _emit_json(document: object, output: Path | None) -> None:
    encoded = canonical_json_bytes(document)
    if output is None:
        sys.stdout.buffer.write(encoded)
    else:
        output.write_bytes(encoded)


def _emit_text(text: str, output: Path | None) -> None:
    encoded = text.encode("utf-8")
    if output is None:
        sys.stdout.buffer.write(encoded)
    else:
        output.write_bytes(encoded)


def _canonical_ids(value: object, field: str) -> tuple[str, ...]:
    values = require_array(value, field)
    checked = tuple(
        require_bounded_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(values)
    )
    if len(set(checked)) != len(checked):
        raise TaskNodeUnlError("duplicate_identifier", field)
    if checked != tuple(sorted(checked)):
        raise TaskNodeUnlError("non_canonical_order", field)
    return checked


def _parse_seat(value: object, field: str) -> RegistrySeat:
    row = require_closed_keys(
        value,
        required=("validator_id", "account_id"),
        field=field,
    )
    return RegistrySeat(
        validator_id=require_bounded_identifier(
            row["validator_id"], f"{field}.validator_id"
        ),
        account_id=require_bounded_identifier(
            row["account_id"], f"{field}.account_id"
        ),
    )


def _parse_seats(value: object, field: str) -> tuple[RegistrySeat, ...]:
    rows = require_array(value, field)
    seats = tuple(_parse_seat(row, f"{field}[{index}]") for index, row in enumerate(rows))
    if seats != tuple(sorted(seats)):
        raise TaskNodeUnlError("non_canonical_order", field)
    if len({item.validator_id for item in seats}) != len(seats):
        raise TaskNodeUnlError("duplicate_validator_seat", field)
    return seats


def _parse_prior_breaches(value: object) -> tuple[PriorBreach, ...]:
    rows = require_array(value, "admission.prior_breaches")
    breaches: list[PriorBreach] = []
    for index, value in enumerate(rows):
        field = f"admission.prior_breaches[{index}]"
        row = require_closed_keys(
            value,
            required=("limit_id", "first_observed_window"),
            field=field,
        )
        breaches.append(
            PriorBreach(
                limit_id=require_lower_hex(row["limit_id"], f"{field}.limit_id"),
                first_observed_window=require_int(
                    row["first_observed_window"],
                    f"{field}.first_observed_window",
                    minimum=0,
                ),
            )
        )
    result = tuple(breaches)
    if result != tuple(sorted(result)):
        raise TaskNodeUnlError("non_canonical_order", "admission.prior_breaches")
    return result


def _parse_round_state(value: object) -> RegistryRoundState:
    field = "admission.registry_state"
    row = require_closed_keys(
        value,
        required=(
            "round_index",
            "current_registry_root",
            "prior_registry_root",
            "prior_registry_round",
            "seats",
            "changes_this_round",
            "transition_budget",
        ),
        field=field,
    )
    prior_root = row["prior_registry_root"]
    if prior_root is not None:
        prior_root = require_lower_hex(prior_root, f"{field}.prior_registry_root")
    prior_round = row["prior_registry_round"]
    if prior_round is not None:
        prior_round = require_int(prior_round, f"{field}.prior_registry_round", minimum=0)
    return RegistryRoundState(
        round_index=require_int(row["round_index"], f"{field}.round_index", minimum=0),
        current_registry_root=require_lower_hex(
            row["current_registry_root"], f"{field}.current_registry_root"
        ),
        prior_registry_root=prior_root,
        prior_registry_round=prior_round,
        seats=_parse_seats(row["seats"], f"{field}.seats"),
        changes_this_round=require_int(
            row["changes_this_round"], f"{field}.changes_this_round", minimum=0
        ),
        transition_budget=require_int(
            row["transition_budget"], f"{field}.transition_budget", minimum=0
        ),
    )


def _parse_candidate(value: object, field: str) -> AdmissionCandidate:
    row = require_closed_keys(
        value,
        required=(
            "validator_id",
            "account_id",
            "control_epoch",
            "source_registry_root",
            "source_registry_round",
        ),
        field=field,
    )
    return AdmissionCandidate(
        validator_id=require_bounded_identifier(
            row["validator_id"], f"{field}.validator_id"
        ),
        account_id=require_bounded_identifier(row["account_id"], f"{field}.account_id"),
        control_epoch=require_lower_hex(row["control_epoch"], f"{field}.control_epoch"),
        source_registry_root=require_lower_hex(
            row["source_registry_root"], f"{field}.source_registry_root"
        ),
        source_registry_round=require_int(
            row["source_registry_round"], f"{field}.source_registry_round", minimum=0
        ),
    )


def _parse_candidates(value: object) -> tuple[AdmissionCandidate, ...]:
    rows = require_array(value, "admission.candidates", maximum=_MAX_CANDIDATES)
    candidates = tuple(
        _parse_candidate(item, f"admission.candidates[{index}]")
        for index, item in enumerate(rows)
    )
    if candidates != tuple(sorted(candidates, key=lambda item: (item.validator_id, item.account_id))):
        raise TaskNodeUnlError("non_canonical_order", "admission.candidates")
    keys = {(item.validator_id, item.account_id) for item in candidates}
    if len(keys) != len(candidates):
        raise TaskNodeUnlError("duplicate_candidate", "admission.candidates")
    return candidates


def _parse_v1_reference(value: object) -> dict[tuple[str, str], dict[str, Any]]:
    field = "admission.v1_reference"
    row = require_closed_keys(
        value,
        required=("schema", "policy_id", "candidates", "reference_root"),
        field=field,
    )
    if row["schema"] != V1_REFERENCE_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", f"{field}.schema")
    policy_id = require_bounded_identifier(row["policy_id"], f"{field}.policy_id")
    values = require_array(row["candidates"], f"{field}.candidates", maximum=_MAX_CANDIDATES)
    candidates: list[dict[str, Any]] = []
    for index, value in enumerate(values):
        candidate_field = f"{field}.candidates[{index}]"
        item = require_closed_keys(
            value,
            required=("validator_id", "account_id", "action", "reason_codes"),
            field=candidate_field,
        )
        action = require_bounded_identifier(item["action"], f"{candidate_field}.action")
        if action not in _V1_ACTIONS:
            raise TaskNodeUnlError("unknown_v1_action", f"{candidate_field}.action")
        reason_codes = _canonical_ids(item["reason_codes"], f"{candidate_field}.reason_codes")
        candidates.append(
            {
                "validator_id": require_bounded_identifier(
                    item["validator_id"], f"{candidate_field}.validator_id"
                ),
                "account_id": require_bounded_identifier(
                    item["account_id"], f"{candidate_field}.account_id"
                ),
                "action": action,
                "reason_codes": list(reason_codes),
            }
        )
    if candidates != sorted(candidates, key=lambda item: (item["validator_id"], item["account_id"])):
        raise TaskNodeUnlError("non_canonical_order", f"{field}.candidates")
    keys = [(item["validator_id"], item["account_id"]) for item in candidates]
    if len(set(keys)) != len(keys):
        raise TaskNodeUnlError("duplicate_candidate", f"{field}.candidates")
    payload = {
        "schema": V1_REFERENCE_SCHEMA,
        "policy_id": policy_id,
        "candidates": candidates,
    }
    expected_root = require_lower_hex(row["reference_root"], f"{field}.reference_root")
    if _domain_hash(V1_REFERENCE_ROOT_DOMAIN, payload) != expected_root:
        raise TaskNodeUnlError("commitment_mismatch", f"{field}.reference_root")
    return {key: candidate for key, candidate in zip(keys, candidates)}


def _parse_evidence_bundle(value: object) -> tuple[object, object]:
    """Accept the operational bundle or the committed Section-A golden bundle."""

    if isinstance(value, Mapping) and "fixture_schema" in value:
        row = require_closed_keys(
            value,
            required=(
                "fixture_schema",
                "snapshot",
                "control_registry",
                "evidence",
                "expected_result",
                "vectors",
            ),
            field="evidence_bundle",
        )
        if row["fixture_schema"] != _GOLDEN_EVIDENCE_SCHEMA:
            raise TaskNodeUnlError("unknown_schema", "evidence_bundle.fixture_schema")
        return row["snapshot"], row["control_registry"]
    row = require_v2_object(
        value,
        schema=CLI_EVIDENCE_BUNDLE_SCHEMA,
        required=("snapshot", "control_registry"),
        field="evidence_bundle",
    )
    return row["snapshot"], row["control_registry"]


def _parse_admission_input(value: object) -> dict[str, Any]:
    row = require_v2_object(
        value,
        schema=CLI_ADMISSION_INPUT_SCHEMA,
        required=(
            "policy_id",
            "opening_registry_root",
            "nodes",
            "opening_seats",
            "foundation_accounts",
            "prior_breaches",
            "registry_state",
            "candidates",
            "v1_reference",
        ),
        field="admission",
    )
    if row["policy_id"] != admission_policy_document()["policy_id"]:
        raise TaskNodeUnlError("policy_id_mismatch", "admission.policy_id")
    nodes = _canonical_ids(row["nodes"], "admission.nodes")
    seats = _parse_seats(row["opening_seats"], "admission.opening_seats")
    foundation = _canonical_ids(row["foundation_accounts"], "admission.foundation_accounts")
    candidates = _parse_candidates(row["candidates"])
    v1 = _parse_v1_reference(row["v1_reference"])
    candidate_keys = {(item.validator_id, item.account_id) for item in candidates}
    if candidate_keys != set(v1):
        raise TaskNodeUnlError("comparison_candidate_mismatch", "admission.v1_reference.candidates")
    return {
        "opening_registry_root": require_lower_hex(
            row["opening_registry_root"], "admission.opening_registry_root"
        ),
        "nodes": nodes,
        "opening_seats": seats,
        "foundation_accounts": foundation,
        "prior_breaches": _parse_prior_breaches(row["prior_breaches"]),
        "registry_state": _parse_round_state(row["registry_state"]),
        "candidates": candidates,
        "v1": v1,
        "v1_reference_root": require_lower_hex(
            row["v1_reference"]["reference_root"],
            "admission.v1_reference.reference_root",
        ),
    }


def _reason_text(reasons: Sequence[str]) -> str:
    return ",".join(reasons) if reasons else "no reasons"


def derive_v2_cli_report(evidence_bundle: object, admission_input: object) -> dict[str, Any]:
    """Derive one canonical V1-reference/V2 comparison with no side effects."""

    snapshot, control_registry = _parse_evidence_bundle(evidence_bundle)
    admission = _parse_admission_input(admission_input)
    input_root = _domain_hash(
        CLI_INPUT_ROOT_DOMAIN,
        {"evidence_bundle": evidence_bundle, "admission_input": admission_input},
    )
    evidence = verify_evidence_snapshot(snapshot, control_registry)
    frozen = freeze_admission_window(
        evidence,
        opening_registry_root=admission["opening_registry_root"],
        nodes=admission["nodes"],
        opening_seats=admission["opening_seats"],
        foundation_accounts=admission["foundation_accounts"],
        prior_breaches=admission["prior_breaches"],
    )

    comparisons: list[dict[str, Any]] = []
    continuity_holds: list[dict[str, Any]] = []
    denials: list[dict[str, Any]] = []
    reports: list[dict[str, Any]] = []
    breaches: dict[str, dict[str, Any]] = {}
    for candidate in admission["candidates"]:
        key = (candidate.validator_id, candidate.account_id)
        v1 = admission["v1"][key]
        report = evaluate_admission_round(frozen, admission["registry_state"], candidate)
        report_document = _json_value(report.to_dict())
        reports.append(report_document)
        v2_codes = [item.code for item in report.decision.reasons]
        verdict_line = (
            f"{candidate.validator_id}: V1 {v1['action'].upper()} "
            f"({_reason_text(v1['reason_codes'])}) | V2 {report.decision.action} "
            f"({_reason_text(v2_codes)})"
        )
        comparison = {
            "validator_id": candidate.validator_id,
            "account_id": candidate.account_id,
            "v1": {**v1, "source": "ROOT_BOUND_SUPPLIED_V1_REFERENCE"},
            "v2": {
                "action": report.decision.action,
                "reason_codes": v2_codes,
                "report_root": report.report_root,
            },
            "verdict_line": verdict_line,
        }
        comparisons.append(comparison)
        if "HOLD_CONTINUITY" in v2_codes or candidate.account_id in frozen.hold_accounts:
            continuity_holds.append(
                {
                    "validator_id": candidate.validator_id,
                    "account_id": candidate.account_id,
                    "reason_codes": v2_codes,
                    "report_root": report.report_root,
                }
            )
        if report.decision.action != PROPOSE_ADD:
            denials.append(
                {
                    "validator_id": candidate.validator_id,
                    "account_id": candidate.account_id,
                    "action": report.decision.action,
                    "reason_codes": v2_codes,
                    "report_root": report.report_root,
                }
            )
        for item in report.unresolved_limits:
            if item.state == EXISTING_BREACH:
                breaches[item.limit_id] = _json_value(item.to_dict())

    frozen_roots = dict(frozen.roots)
    roots = {
        "cli_input_root": input_root,
        "v1_reference_root": admission["v1_reference_root"],
        "admission_policy_root": admission_policy_root(),
        "frozen_window_root": frozen.frozen_window_root,
        **frozen_roots,
    }
    payload = {
        "schema": CLI_REPORT_SCHEMA,
        "version": V2_VERSION,
        "mode": SHADOW_MODE,
        "policy_id": admission_policy_document()["policy_id"],
        "overall_status": "SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS",
        "authority_boundary": {
            "submission_supported": False,
            "live_mutation_supported": False,
            "network_access_used": False,
            "promotion_authorized": False,
            "v1_inputs_reinterpreted": False,
        },
        "roots": roots,
        "constants": admission_policy_document()["constants"],
        "evidence_status": evidence.status,
        "frozen_status": frozen.status,
        "side_by_side": comparisons,
        "continuity_holds": continuity_holds,
        "admission_denials": denials,
        "existing_breaches": [breaches[key] for key in sorted(breaches)],
        "known_control_limits": list(EVIDENCE_LIMITATIONS),
        "candidate_reports": reports,
    }
    materialized = _json_value(payload)
    materialized["report_root"] = _domain_hash(CLI_REPORT_ROOT_DOMAIN, materialized)
    return materialized


def validate_v2_cli_report(value: object) -> Mapping[str, Any]:
    row = require_v2_object(
        value,
        schema=CLI_REPORT_SCHEMA,
        required=(
            "policy_id",
            "overall_status",
            "authority_boundary",
            "roots",
            "constants",
            "evidence_status",
            "frozen_status",
            "side_by_side",
            "continuity_holds",
            "admission_denials",
            "existing_breaches",
            "known_control_limits",
            "candidate_reports",
            "report_root",
        ),
        field="report",
    )
    if row["policy_id"] != admission_policy_document()["policy_id"]:
        raise TaskNodeUnlError("policy_id_mismatch", "report.policy_id")
    supplied_root = require_lower_hex(row["report_root"], "report.report_root")
    payload = {key: row[key] for key in row if key != "report_root"}
    if _domain_hash(CLI_REPORT_ROOT_DOMAIN, payload) != supplied_root:
        raise TaskNodeUnlError("commitment_mismatch", "report.report_root")
    return row


def render_v2_markdown(value: object) -> str:
    """Render the compact human interface from a root-verified CLI report."""

    report = validate_v2_cli_report(value)
    lines = [
        "# Task Node UNL V2 shadow comparison",
        "",
        "**SHADOW_ONLY — decision support only; no submission, live mutation, or promotion is available.**",
        "",
        f"Overall: `{report['overall_status']}`. This is not an all-green status.",
        "",
        f"Version: `{report['version']}` · Policy: `{report['policy_id']}` · Report root: `{report['report_root']}`",
        "",
        "## Side-by-side verdicts",
        "",
    ]
    comparisons = require_array(report["side_by_side"], "report.side_by_side", maximum=_MAX_CANDIDATES)
    lines.extend(f"- `{item['verdict_line']}`" for item in comparisons)

    lines.extend(("", "## HOLD_CONTINUITY", ""))
    holds = require_array(report["continuity_holds"], "report.continuity_holds", maximum=_MAX_CANDIDATES)
    if holds:
        lines.extend(
            f"- `{item['validator_id']}` / `{item['account_id']}` — "
            f"{_reason_text(item['reason_codes'])}; report `{item['report_root']}`"
            for item in holds
        )
    else:
        lines.append("- None in this input; the fresh-window requirement still applies.")

    lines.extend(("", "## Admission denials", ""))
    denials = require_array(report["admission_denials"], "report.admission_denials", maximum=_MAX_CANDIDATES)
    if denials:
        lines.extend(
            f"- `{item['validator_id']}` — `{item['action']}`: "
            f"{_reason_text(item['reason_codes'])}; report `{item['report_root']}`"
            for item in denials
        )
    else:
        lines.append("- None in this input.")

    lines.extend(("", "## EXISTING_BREACH", ""))
    breaches = require_array(report["existing_breaches"], "report.existing_breaches")
    if breaches:
        lines.extend(
            f"- `{item['limit_kind']}` `{item['limit_id']}`: {item['excess_seats']} excess "
            f"seat(s), review `{item['review_state']}`, evidence "
            f"{_reason_text(item['causative_evidence'])}"
            for item in breaches
        )
    else:
        lines.append("- None in this input.")

    lines.extend(("", "## Known control and evidence limits", ""))
    limitations = require_array(report["known_control_limits"], "report.known_control_limits")
    lines.extend(f"- {item}" for item in limitations)
    lines.extend((
        "",
        "A valid custody signature is not a personhood claim. An unchanged-key account sale or other control transfer is not detectable from these public inputs. Funding remains audit-only: it creates neither positive trust mass nor a unilateral veto.",
        "",
        "## Bound roots",
        "",
    ))
    roots = report["roots"]
    if not isinstance(roots, Mapping):
        raise TaskNodeUnlError("invalid_object", "report.roots")
    lines.extend(f"- `{name}`: `{roots[name]}`" for name in sorted(roots))
    lines.extend((
        "",
        "## Activation boundary",
        "",
        "This report implements the locked V2 policy in shadow mode only. It does not alter V1, the validator registry, Cobalt ratification, quorum, score provenance, or token economics. Live promotion requires a separate approval.",
        "",
    ))
    return "\n".join(lines)


def _require_explicit_v2(value: str) -> None:
    if value != _POLICY_VERSION:
        raise TaskNodeUnlError("unknown_version", "policy_version")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.tasknode_unl_v2",
        description="offline, read-only UNL V2 shadow derivation and reporting",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    derive = commands.add_parser(
        "derive",
        help="derive canonical JSON and optional Markdown from local evidence",
    )
    derive.add_argument("--policy-version", required=True, metavar="v2")
    derive.add_argument("--evidence-bundle", type=Path, required=True)
    derive.add_argument("--admission-input", type=Path, required=True)
    derive.add_argument("--output", type=Path)
    derive.add_argument("--markdown-output", type=Path)

    render = commands.add_parser(
        "render",
        help="verify a derived report root and render the human Markdown view",
    )
    render.add_argument("--policy-version", required=True, metavar="v2")
    render.add_argument("--input", type=Path, required=True)
    render.add_argument("--output", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        _require_explicit_v2(args.policy_version)
        if args.command == "derive":
            if args.output is not None and args.markdown_output == args.output:
                raise TaskNodeUnlError("output_paths_must_differ", "markdown_output")
            report = derive_v2_cli_report(
                _read_json(args.evidence_bundle),
                _read_json(args.admission_input),
            )
            markdown = render_v2_markdown(report) if args.markdown_output is not None else None
            _emit_json(report, args.output)
            if markdown is not None:
                _emit_text(markdown, args.markdown_output)
            return 0
        if args.command == "render":
            _emit_text(render_v2_markdown(_read_json(args.input)), args.output)
            return 0
        raise TaskNodeUnlError("unknown_command", args.command)
    except (OSError, TaskNodeUnlError) as exc:
        print(f"tasknode-unl-v2: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
