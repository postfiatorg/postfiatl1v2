"""Verify the exact first-release receive-only LND macaroon profile."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any, Mapping, Sequence


RECEIVE_ONLY_PERMISSIONS = frozenset(
    {
        ("info", "read"),
        ("offchain", "read"),
        ("invoices", "read"),
        ("invoices", "write"),
    }
)


class MacaroonVerificationError(ValueError):
    """The baked LND macaroon has broader or ambiguous authority."""


def _permission(value: Any) -> tuple[str, str]:
    if isinstance(value, Mapping):
        if frozenset(value.keys()) != {"entity", "action"}:
            raise MacaroonVerificationError(
                "macaroon permission object field set mismatch"
            )
        entity = value.get("entity")
        action = value.get("action")
    elif type(value) is str and value.count(":") == 1:
        entity, action = value.split(":", 1)
    else:
        raise MacaroonVerificationError("macaroon permission encoding is invalid")
    if (
        type(entity) is not str
        or type(action) is not str
        or not entity
        or not action
        or not entity.isascii()
        or not action.isascii()
    ):
        raise MacaroonVerificationError("macaroon permission is invalid")
    return entity, action


def verify_printmacaroon_report(value: Any) -> tuple[tuple[str, str], ...]:
    """Parse ``lncli printmacaroon`` and require the exact reviewed authority."""

    if not isinstance(value, Mapping):
        raise MacaroonVerificationError("printmacaroon report is not an object")
    permissions = value.get("permissions")
    if (
        not isinstance(permissions, Sequence)
        or isinstance(permissions, (str, bytes, bytearray))
    ):
        raise MacaroonVerificationError(
            "printmacaroon permissions are not a list"
        )
    normalized = tuple(_permission(permission) for permission in permissions)
    if (
        len(normalized) != len(RECEIVE_ONLY_PERMISSIONS)
        or len(set(normalized)) != len(normalized)
        or set(normalized) != RECEIVE_ONLY_PERMISSIONS
    ):
        raise MacaroonVerificationError(
            "macaroon permission set is not exactly receive-only v1"
        )
    if "caveats" not in value or value["caveats"] not in (None, []):
        raise MacaroonVerificationError(
            "receive-only v1 macaroon must have no first-party caveats"
        )
    return tuple(sorted(normalized))


def verify_macaroon_file(path: str | Path) -> str:
    macaroon_path = Path(path)
    try:
        metadata = macaroon_path.lstat()
    except OSError as error:
        raise MacaroonVerificationError("macaroon file is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise MacaroonVerificationError(
            "macaroon file must be regular and non-symlink"
        )
    if metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
        raise MacaroonVerificationError(
            "macaroon file must be coordinator-owned and mode 0600"
        )
    if metadata.st_size < 1 or metadata.st_size > (1 << 20):
        raise MacaroonVerificationError("macaroon file size is invalid")
    try:
        encoded = macaroon_path.read_bytes()
    except OSError as error:
        raise MacaroonVerificationError("macaroon file could not be read") from error
    if len(encoded) != metadata.st_size:
        raise MacaroonVerificationError("macaroon file changed while read")
    return hashlib.sha256(encoded).hexdigest()


def verification_evidence(value: Any, path: str | Path) -> dict[str, Any]:
    permissions = verify_printmacaroon_report(value)
    digest = verify_macaroon_file(path)
    return {
        "schema": "postfiat.lightning_receive_only_macaroon_check.v1",
        "ok": True,
        "value_moved": False,
        "macaroon_path": str(Path(path)),
        "macaroon_sha256": digest,
        "permissions": [
            {"entity": entity, "action": action}
            for entity, action in permissions
        ],
        "caveats": [],
        "profile": "LIGHTNING_NAVCOIN_RECEIVE_ONLY_V1",
    }


def main(argv: Sequence[str] | None = None) -> int:
    arguments = tuple(sys.argv[1:] if argv is None else argv)
    if len(arguments) != 1:
        print("usage: macaroon_verifier.py MACAROON_PATH", file=sys.stderr)
        return 2
    try:
        value = json.load(sys.stdin)
        evidence = verification_evidence(value, arguments[0])
    except (
        OSError,
        UnicodeError,
        json.JSONDecodeError,
        MacaroonVerificationError,
    ) as error:
        print(f"receive-only macaroon verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(evidence, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
