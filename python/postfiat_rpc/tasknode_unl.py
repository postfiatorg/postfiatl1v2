"""Offline Task Node identity-derived UNL utilities.

This CLI prepares and verifies SHADOW_ONLY validator-to-wallet binding
artifacts and derives fixture-only shadow reports. It has no transaction
preparation or submission command, performs no network access, never accepts
private keys, and cannot mutate registry state. Signatures arrive as detached
envelopes produced by custody-preserving signer adapters.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

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
    SHADOW_INPUT_MANIFEST_SCHEMA,
    SHADOW_MODE,
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
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "prepare-bind":
            challenge = prepare_bind_challenge(
                validator_id=args.validator_id,
                validator_public_key_hex=args.validator_public_key_hex,
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

        raise TaskNodeUnlError("unknown_command", args.command)
    except (OSError, TaskNodeUnlError) as exc:
        print(f"tasknode-unl: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
