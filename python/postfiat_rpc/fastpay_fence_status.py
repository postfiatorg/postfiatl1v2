"""Client check of signed FastPay fence statuses (`owned_fence_status`).

Mirrors `postfiat_types::verify_fastpay_fence_status_quorum_v1`: the answer is
proved only when a quorum of distinct committee members signed the same
decision, operation, fence digest and anchored height for the lock, on the
committee's chain, genesis, protocol version and epoch. Observed heights may
differ. This package has no ML-DSA implementation, so the caller passes
``verify(public_key_hex, message, signature_hex)``, which must check ML-DSA-65
under ``FENCE_STATUS_CONTEXT``.
"""

from __future__ import annotations

import re
import struct
from typing import Any, Callable, Iterable

FENCE_STATUS_SCHEMA = "postfiat-fastpay-fence-status-v1"
FENCE_STATUS_CONTEXT = b"postfiat-l1-v2/fastpay-fence-status/v1"
FENCE_STATUS_SIGNING_TAG = b"postfiat.fastpay.fence-status.v1\0"
_HEX96 = re.compile(r"^[0-9a-f]{96}$")
_OPERATIONS = {None: 0, "transfer": 1, "unwrap": 2}


class FenceStatusRefused(ValueError):
    """A set of statuses that proves nothing; ``reason`` names why."""

    def __init__(self, reason: str, validator_id: str | None = None, **detail: Any):
        self.reason = reason
        self.validator_id = validator_id
        self.detail = detail
        super().__init__(f"{reason}: {validator_id or detail}")


def _text(value: str) -> bytes:
    raw = value.encode()
    return struct.pack(">Q", len(raw)) + raw


def _hex96(value: Any) -> bool:
    return isinstance(value, str) and bool(_HEX96.match(value))


def _shape_ok(status: dict[str, Any]) -> bool:
    if (
        status.get("schema") != FENCE_STATUS_SCHEMA
        or not status.get("chain_id")
        or not status.get("validator_id")
        or not status.get("signature_hex")
        or not isinstance(status.get("committee_epoch"), int)
        or status["committee_epoch"] < 1
        or not isinstance(status.get("observed_height"), int)
        or not all(_hex96(status.get(f)) for f in ("genesis_hash", "registry_root", "lock_id"))
    ):
        return False
    terminal = [status.get(f) for f in ("decision", "operation", "fence_digest", "anchored_height")]
    if all(value is None for value in terminal):
        return True
    decision, operation, digest, height = terminal
    if None in terminal or operation not in ("transfer", "unwrap") or not _hex96(digest):
        return False
    if not isinstance(height, int) or not 0 < height <= status["observed_height"]:
        return False
    if decision == "cancelled":
        return True
    confirmed = decision.get("confirmed") if isinstance(decision, dict) else None
    return isinstance(confirmed, dict) and all(
        _hex96(confirmed.get(f)) for f in ("order_digest", "certificate_digest")
    )


def signing_bytes(status: dict[str, Any]) -> bytes:
    """The canonical signed bytes, byte for byte as the node signs them."""
    if not _shape_ok(status):
        raise ValueError("malformed FastPay fence status")
    out = bytearray(FENCE_STATUS_SIGNING_TAG)
    for field in ("schema", "chain_id", "genesis_hash"):
        out += _text(status[field])
    out += struct.pack(">IQ", status["protocol_version"], status["committee_epoch"])
    out += _text(status["registry_root"]) + _text(status["lock_id"])
    decision = status["decision"]
    if decision is None:
        out.append(0)
    elif decision == "cancelled":
        out.append(2)
    else:
        out.append(1)
        out += _text(decision["confirmed"]["order_digest"])
        out += _text(decision["confirmed"]["certificate_digest"])
    out.append(_OPERATIONS[status["operation"]])
    if status["fence_digest"] is None:
        out.append(0)
    else:
        out += b"\x01" + _text(status["fence_digest"])
    if status["anchored_height"] is None:
        out.append(0)
    else:
        out += b"\x01" + struct.pack(">Q", status["anchored_height"])
    out += struct.pack(">Q", status["observed_height"])
    out += _text(status["validator_id"])
    return bytes(out)


def _answer(status: dict[str, Any]) -> tuple[Any, ...]:
    decision = status["decision"]
    key = repr(sorted(decision["confirmed"].items())) if isinstance(decision, dict) else decision
    return (key, status["operation"], status["fence_digest"], status["anchored_height"])


def verify_fence_status_quorum(
    statuses: Iterable[dict[str, Any]],
    committee: dict[str, Any],
    lock_id: str,
    verify: Callable[[str, bytes, str], bool],
) -> dict[str, Any]:
    """Return the proved answer, or raise ``FenceStatusRefused``."""
    statuses = list(statuses)
    members = {v["validator_id"]: v["public_key_hex"] for v in committee.get("validators", [])}
    quorum = committee.get("quorum")
    if not members or not isinstance(quorum, int) or quorum < 1:
        raise FenceStatusRefused("invalid_committee")
    seen: set[str] = set()
    groups: dict[tuple[Any, ...], list[dict[str, Any]]] = {}
    for status in statuses:
        validator_id = status.get("validator_id")
        try:
            message = signing_bytes(status)
        except (KeyError, TypeError, ValueError, struct.error):
            raise FenceStatusRefused("malformed", validator_id) from None
        if status["lock_id"] != lock_id:
            raise FenceStatusRefused("wrong_lock", validator_id)
        if (status["chain_id"], status["genesis_hash"], status["protocol_version"]) != (
            committee["chain_id"],
            committee["genesis_hash"],
            committee["protocol_version"],
        ):
            raise FenceStatusRefused("wrong_domain", validator_id)
        if (status["committee_epoch"], status["registry_root"]) != (
            committee["committee_epoch"],
            committee["registry_root"],
        ):
            raise FenceStatusRefused("wrong_epoch", validator_id)
        if validator_id not in members:
            raise FenceStatusRefused("non_member", validator_id)
        if validator_id in seen:
            raise FenceStatusRefused("duplicate_validator", validator_id)
        seen.add(validator_id)
        if not verify(members[validator_id], message, status["signature_hex"]):
            raise FenceStatusRefused("bad_signature", validator_id)
        groups.setdefault(_answer(status), []).append(status)
    terminal = [answer[3] for answer in groups if answer[3] is not None]
    not_terminal = [s["observed_height"] for s in statuses if s["decision"] is None]
    if len(terminal) > 1 or (terminal and not_terminal and max(not_terminal) >= terminal[0]):
        raise FenceStatusRefused("conflicting")
    best = max(groups.values(), key=len, default=[])
    if len(best) < quorum:
        raise FenceStatusRefused("below_quorum", agreeing=len(best), quorum=quorum)
    first = best[0]
    return {
        "lock_id": lock_id,
        "committee_epoch": committee["committee_epoch"],
        "decision": first["decision"],
        "operation": first["operation"],
        "fence_digest": first["fence_digest"],
        "anchored_height": first["anchored_height"],
        "observed_height": min(s["observed_height"] for s in best),
        "signers": sorted(s["validator_id"] for s in best),
        "quorum": quorum,
    }
