"""Offline Task Node identity-derived UNL utilities.

This CLI prepares and verifies SHADOW_ONLY validator-to-wallet binding
artifacts, derives fixture-only shadow reports, and compares their explicit
deltas with local published-UNL baselines. It has no transaction preparation
or submission command, performs no network access, never accepts private keys,
and cannot mutate registry state. Signatures arrive as detached envelopes
produced by custody-preserving signer adapters.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Mapping, Sequence

from .tasknode_unl_binding import (
    binding_challenge_from_dict,
    binding_ledger_record_from_dict,
    binding_memo_artifact,
    create_bind_memo,
    create_revoke_memo,
    prepare_bind_challenge,
    prepare_revoke_challenge,
    replay_bindings_document,
    signature_envelope_from_dict,
    verified_record_document,
    verify_binding_record,
)
from .tasknode_unl_policy import (
    SHADOW_INPUT_FILES,
    derive_shadow_report,
    render_shadow_markdown,
)
from .tasknode_unl_schema import (
    SHADOW_BASELINE_DIFF_SCHEMA,
    SHADOW_INPUT_MANIFEST_SCHEMA,
    SHADOW_MODE,
    SHADOW_REPORT_SCHEMA,
    TaskNodeUnlError,
    canonical_json_bytes,
    require_closed_keys,
    require_identifier,
)

_MAX_INPUT_BYTES = 4 * 1024 * 1024
_SHADOW_MANIFEST_NAME = "shadow-input.json"


def _read_json(path: Path) -> Any:
    size = path.stat().st_size
    if size > _MAX_INPUT_BYTES:
        raise TaskNodeUnlError("input_file_too_large", str(path))
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TaskNodeUnlError("invalid_json", str(path)) from exc


def _emit(document: Any, output: Path | None) -> None:
    encoded = canonical_json_bytes(document)
    if output is None:
        sys.stdout.buffer.write(encoded)
    else:
        output.write_bytes(encoded)


def _shadow_documents(input_dir: Path) -> dict[str, object]:
    if not input_dir.is_dir():
        raise TaskNodeUnlError("input_directory_missing", str(input_dir))
    manifest = require_closed_keys(
        _read_json(input_dir / _SHADOW_MANIFEST_NAME),
        required=("schema", "mode", "files"),
        field="shadow_input_manifest",
    )
    if manifest["schema"] != SHADOW_INPUT_MANIFEST_SCHEMA:
        raise TaskNodeUnlError(
            "unknown_schema", "shadow_input_manifest.schema"
        )
    if manifest["mode"] != SHADOW_MODE:
        raise TaskNodeUnlError(
            "mode_mismatch", "shadow_input_manifest.mode"
        )
    files = require_closed_keys(
        manifest["files"],
        required=SHADOW_INPUT_FILES,
        field="shadow_input_manifest.files",
    )
    selected: dict[str, object] = {}
    names: set[str] = set()
    for logical_name in SHADOW_INPUT_FILES:
        name = require_identifier(
            files[logical_name],
            f"shadow_input_manifest.files.{logical_name}",
        )
        path = Path(name)
        if path.name != name or path.is_absolute() or name in (".", ".."):
            raise TaskNodeUnlError(
                "unsafe_input_filename",
                f"shadow_input_manifest.files.{logical_name}",
            )
        if name in names:
            raise TaskNodeUnlError("duplicate_input_filename", name)
        names.add(name)
        selected[logical_name] = _read_json(input_dir / name)
    return selected


def _emit_shadow(args: argparse.Namespace) -> None:
    if args.markdown_output is not None and args.markdown_output == args.output:
        raise TaskNodeUnlError("output_paths_must_differ")
    report = derive_shadow_report(_shadow_documents(args.input_dir))
    markdown = (
        render_shadow_markdown(report)
        if args.markdown_output is not None
        else None
    )
    _emit(report, args.output)
    if markdown is not None:
        args.markdown_output.write_text(markdown, encoding="utf-8")


def _identifier_list(value: object, field: str) -> list[str]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    identifiers = [
        require_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(value)
    ]
    if len(identifiers) != len(set(identifiers)):
        raise TaskNodeUnlError("duplicate_identifier", field)
    return sorted(identifiers)


def _baseline_validator_ids(document: object) -> list[str]:
    if isinstance(document, list):
        return _identifier_list(document, "baseline")
    if not isinstance(document, Mapping):
        raise TaskNodeUnlError("invalid_object", "baseline")
    if "unl" not in document:
        raise TaskNodeUnlError("missing_field", "baseline.unl")
    return _identifier_list(document["unl"], "baseline.unl")


def _string_list(value: object, field: str) -> list[str]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    return [
        require_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(value)
    ]


def _change_reasons(
    value: object,
    field: str,
) -> dict[str, object]:
    row = require_closed_keys(
        value,
        required=("validator_id", "reason_codes", "evidence_references"),
        field=field,
    )
    return {
        "validator_id": require_identifier(
            row["validator_id"], f"{field}.validator_id"
        ),
        "reason_codes": _string_list(
            row["reason_codes"], f"{field}.reason_codes"
        ),
        "evidence_references": _string_list(
            row["evidence_references"],
            f"{field}.evidence_references",
        ),
    }


def _change_reason_map(
    value: object,
    field: str,
) -> dict[str, dict[str, object]]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    changes: dict[str, dict[str, object]] = {}
    for index, item in enumerate(value):
        reason = _change_reasons(item, f"{field}[{index}]")
        validator_id = str(reason["validator_id"])
        if validator_id in changes:
            raise TaskNodeUnlError("duplicate_identifier", field)
        changes[validator_id] = reason
    return changes


def _upstream_reason(
    value: object,
    field: str,
) -> dict[str, object]:
    row = require_closed_keys(
        value,
        required=("code", "detail", "evidence_references", "field"),
        field=field,
    )
    detail = row["detail"]
    if not isinstance(detail, str):
        raise TaskNodeUnlError("invalid_string", f"{field}.detail")
    return {
        "code": require_identifier(row["code"], f"{field}.code"),
        "detail": detail,
        "evidence_references": _string_list(
            row["evidence_references"],
            f"{field}.evidence_references",
        ),
        "field": require_identifier(row["field"], f"{field}.field"),
    }


def shadow_baseline_diff(
    baseline_document: object,
    shadow_report: object,
) -> dict[str, object]:
    """Project a local SHADOW_ONLY report delta onto a published UNL."""

    if not isinstance(shadow_report, Mapping):
        raise TaskNodeUnlError("invalid_object", "shadow_report")
    if shadow_report.get("schema") != SHADOW_REPORT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", "shadow_report.schema")
    if shadow_report.get("mode") != SHADOW_MODE:
        raise TaskNodeUnlError("mode_mismatch", "shadow_report.mode")

    authority = shadow_report.get("authority_boundary")
    if not isinstance(authority, Mapping):
        raise TaskNodeUnlError("invalid_object", "authority_boundary")
    if any(
        authority.get(field) is not False
        for field in (
            "registry_write_supported",
            "transaction_submission_supported",
            "ratification_supported",
            "signable_delta_emitted",
        )
    ):
        raise TaskNodeUnlError("live_authority_forbidden")

    baseline_ids = _baseline_validator_ids(baseline_document)
    report_baseline_ids = _identifier_list(
        shadow_report.get("baseline_validator_ids"),
        "shadow_report.baseline_validator_ids",
    )
    report_shadow_ids = _identifier_list(
        shadow_report.get("shadow_candidate_list"),
        "shadow_report.shadow_candidate_list",
    )

    baseline_diff = shadow_report.get("baseline_diff")
    if not isinstance(baseline_diff, Mapping):
        raise TaskNodeUnlError("invalid_object", "shadow_report.baseline_diff")
    addition_reasons = _change_reason_map(
        baseline_diff.get("additions"),
        "shadow_report.baseline_diff.additions",
    )
    removal_reasons = _change_reason_map(
        baseline_diff.get("removals"),
        "shadow_report.baseline_diff.removals",
    )
    report_added_ids = sorted(
        set(report_shadow_ids) - set(report_baseline_ids)
    )
    report_removed_ids = sorted(
        set(report_baseline_ids) - set(report_shadow_ids)
    )
    if report_added_ids != sorted(addition_reasons):
        raise TaskNodeUnlError(
            "shadow_report_diff_mismatch", "additions"
        )
    if report_removed_ids != sorted(removal_reasons):
        raise TaskNodeUnlError(
            "shadow_report_diff_mismatch", "removals"
        )

    added_ids = sorted(set(report_added_ids) - set(baseline_ids))
    removed_ids = sorted(set(report_removed_ids) & set(baseline_ids))
    projected_ids = sorted(
        (set(baseline_ids) - set(removed_ids)) | set(added_ids)
    )
    retained_ids = sorted(set(baseline_ids) & set(projected_ids))

    hold_reasons = _change_reason_map(
        baseline_diff.get("holds"),
        "shadow_report.baseline_diff.holds",
    )
    candidates = shadow_report.get("candidates")
    if not isinstance(candidates, list):
        raise TaskNodeUnlError("invalid_array", "shadow_report.candidates")
    held_accounts: list[dict[str, object]] = []
    for index, value in enumerate(candidates):
        field = f"shadow_report.candidates[{index}]"
        if not isinstance(value, Mapping):
            raise TaskNodeUnlError("invalid_object", field)
        if value.get("status") != "hold":
            continue
        validator_id = require_identifier(
            value.get("validator_id"), f"{field}.validator_id"
        )
        account_id = require_identifier(
            value.get("account_id"), f"{field}.account_id"
        )
        if validator_id not in hold_reasons:
            raise TaskNodeUnlError(
                "shadow_diff_reason_missing",
                f"hold:{validator_id}",
            )
        upstream = value.get("upstream_holds")
        if not isinstance(upstream, list):
            raise TaskNodeUnlError(
                "invalid_array", f"{field}.upstream_holds"
            )
        held_accounts.append(
            {
                "account_id": account_id,
                "validator_id": validator_id,
                "decision_reason": hold_reasons[validator_id],
                "upstream_reason_objects": [
                    _upstream_reason(
                        item,
                        f"{field}.upstream_holds[{reason_index}]",
                    )
                    for reason_index, item in enumerate(upstream)
                ],
            }
        )
    held_accounts.sort(
        key=lambda item: (str(item["account_id"]), str(item["validator_id"]))
    )

    return {
        "schema": SHADOW_BASELINE_DIFF_SCHEMA,
        "mode": SHADOW_MODE,
        "authority_boundary": dict(authority),
        "shadow_report_hash": require_identifier(
            shadow_report.get("report_hash"), "shadow_report.report_hash"
        ),
        "baseline_validator_ids": baseline_ids,
        "projected_validator_ids": projected_ids,
        "counts": {
            "added": len(added_ids),
            "removed": len(removed_ids),
            "retained": len(retained_ids),
            "held_accounts": len(held_accounts),
        },
        "diff": {
            "added": [addition_reasons[item] for item in added_ids],
            "removed": [removal_reasons[item] for item in removed_ids],
            "retained": [
                {"validator_id": item} for item in retained_ids
            ],
            "held_accounts": held_accounts,
        },
    }


def _emit_shadow_diff(args: argparse.Namespace) -> None:
    report = shadow_baseline_diff(
        _read_json(args.baseline),
        _read_json(args.shadow_report),
    )
    _emit(report, args.output)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="postfiat_rpc.tasknode_unl",
        description=__doc__,
    )
    commands = parser.add_subparsers(dest="command", required=True)

    prepare_bind = commands.add_parser(
        "prepare-bind",
        help="prepare canonical bind challenge bytes; does not sign or submit",
    )
    prepare_bind.add_argument("--validator-id", required=True)
    prepare_bind.add_argument("--validator-public-key-hex", required=True)
    prepare_bind.add_argument(
        "--validator-registry-public-key-hash",
        required=True,
    )
    prepare_bind.add_argument("--wallet-address", required=True)
    prepare_bind.add_argument("--wallet-public-key-hex", required=True)
    prepare_bind.add_argument("--nonce-hex", required=True)
    prepare_bind.add_argument("--previous-wallet-address")
    prepare_bind.add_argument("--output", type=Path)

    finalize_bind = commands.add_parser(
        "finalize-bind",
        help="verify two detached signatures and emit a bounded memo payload",
    )
    finalize_bind.add_argument("--challenge", type=Path, required=True)
    finalize_bind.add_argument(
        "--validator-signature",
        type=Path,
        required=True,
    )
    finalize_bind.add_argument(
        "--wallet-signature",
        type=Path,
        required=True,
    )
    finalize_bind.add_argument("--output", type=Path)

    prepare_revoke = commands.add_parser(
        "prepare-revoke",
        help="prepare a revoke challenge for one verified active bind record",
    )
    prepare_revoke.add_argument("--binding-record", type=Path, required=True)
    prepare_revoke.add_argument("--nonce-hex", required=True)
    prepare_revoke.add_argument("--output", type=Path)

    finalize_revoke = commands.add_parser(
        "finalize-revoke",
        help="verify either detached signer and emit a bounded revoke memo",
    )
    finalize_revoke.add_argument("--challenge", type=Path, required=True)
    finalize_revoke.add_argument("--signature", type=Path, required=True)
    finalize_revoke.add_argument("--output", type=Path)

    verify_record = commands.add_parser(
        "verify-record",
        help="verify one explicit local ledger-record document",
    )
    verify_record.add_argument("--record", type=Path, required=True)
    verify_record.add_argument("--output", type=Path)

    replay = commands.add_parser(
        "replay",
        help="replay explicit local binding and rotation records",
    )
    replay.add_argument("--input", type=Path, required=True)
    replay.add_argument("--output", type=Path)

    shadow = commands.add_parser(
        "shadow-derive",
        aliases=("derive",),
        help="derive a fixture-only report; cannot write or submit state",
    )
    shadow.add_argument(
        "--input-dir",
        "--fixture-dir",
        dest="input_dir",
        type=Path,
        required=True,
    )
    shadow.add_argument("--output", type=Path, required=True)
    shadow.add_argument("--markdown-output", type=Path)

    shadow_diff = commands.add_parser(
        "shadow-diff",
        help="project one local shadow delta onto a published UNL",
    )
    shadow_diff.add_argument("--baseline", type=Path, required=True)
    shadow_diff.add_argument(
        "--shadow-report",
        type=Path,
        required=True,
    )
    shadow_diff.add_argument("--output", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "prepare-bind":
            challenge = prepare_bind_challenge(
                validator_id=args.validator_id,
                validator_public_key_hex=args.validator_public_key_hex,
                validator_registry_public_key_hash=(
                    args.validator_registry_public_key_hash
                ),
                wallet_address=args.wallet_address,
                wallet_public_key_hex=args.wallet_public_key_hex,
                nonce_hex=args.nonce_hex,
                previous_wallet_address=args.previous_wallet_address,
            )
            _emit(challenge.to_dict(), args.output)
            return 0

        if args.command == "finalize-bind":
            challenge = binding_challenge_from_dict(
                _read_json(args.challenge)
            )
            validator_signature = signature_envelope_from_dict(
                _read_json(args.validator_signature)
            )
            wallet_signature = signature_envelope_from_dict(
                _read_json(args.wallet_signature)
            )
            memo = create_bind_memo(
                challenge,
                validator_signature,
                wallet_signature,
            )
            _emit(binding_memo_artifact(memo), args.output)
            return 0

        if args.command == "prepare-revoke":
            record = binding_ledger_record_from_dict(
                _read_json(args.binding_record)
            )
            active_binding = verify_binding_record(record)
            challenge = prepare_revoke_challenge(
                active_binding,
                nonce_hex=args.nonce_hex,
            )
            _emit(challenge.to_dict(), args.output)
            return 0

        if args.command == "finalize-revoke":
            challenge = binding_challenge_from_dict(
                _read_json(args.challenge)
            )
            signature = signature_envelope_from_dict(
                _read_json(args.signature)
            )
            memo = create_revoke_memo(challenge, signature)
            _emit(binding_memo_artifact(memo), args.output)
            return 0

        if args.command == "verify-record":
            record = binding_ledger_record_from_dict(
                _read_json(args.record)
            )
            _emit(
                verified_record_document(verify_binding_record(record)),
                args.output,
            )
            return 0

        if args.command == "replay":
            result = replay_bindings_document(_read_json(args.input))
            _emit(result.to_dict(), args.output)
            return 0

        if args.command in ("shadow-derive", "derive"):
            _emit_shadow(args)
            return 0

        if args.command == "shadow-diff":
            _emit_shadow_diff(args)
            return 0

        raise TaskNodeUnlError("unknown_command", args.command)
    except (OSError, TaskNodeUnlError) as exc:
        print(f"tasknode-unl: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
