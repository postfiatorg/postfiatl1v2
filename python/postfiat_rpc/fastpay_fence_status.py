"""Client check of signed FastPay fence statuses (`owned_fence_status`).

Mirrors `postfiat_types::verify_fastpay_fence_status_quorum_v2`: the answer is
proved only when a quorum of distinct committee members signed the same
decision, operation and fence digest for the lock, on the committee's chain,
genesis, protocol version and epoch. Anchored and observed heights may differ
between honest signers; the proved `anchored_at_or_before_height` is the
highest one in the agreeing quorum. A faulty minority (at most the
committee's fault bound) cannot veto that quorum: its invalid or
contradicting statuses are reported beside the answer.

The committee is validated as the Rust type validates it (registry root
recomputed from the members, the protocol quorum for the member count,
schema, order range, canonical member order) and its root must equal the
caller's ``pinned_registry_root``, taken from the caller's own trusted
configuration, never from the answer that carries the statuses. The root
commits to the chain id, genesis hash, protocol version, epoch and members.

This package has no ML-DSA implementation, so the caller passes
``verify(public_key_hex, message, signature_hex)``, which must check ML-DSA-65
under ``FENCE_STATUS_CONTEXT``.
"""

from __future__ import annotations

import hashlib
import re
import struct
from typing import Any, Callable, Iterable

FENCE_STATUS_SCHEMA = "postfiat-fastpay-fence-status-v2"
FENCE_STATUS_CONTEXT = b"postfiat-l1-v2/fastpay-fence-status/v2"
FENCE_STATUS_SIGNING_TAG = b"postfiat.fastpay.fence-status.v2\0"
COMMITTEE_ROOT_DOMAINS = {
    "postfiat-fastpay-recovery-committee-v1": "postfiat.fastpay.recovery-committee.root.v1",
    "postfiat-fastpay-recovery-committee-v2": "postfiat.fastpay.recovery-committee.root.v2",
}
MAX_COMMITTEE_VALIDATORS = 128
MAX_PUBLIC_KEY_HEX_LEN = 4096
MAX_SIGNATURE_HEX_LEN = 8192
ML_DSA_65 = "ML-DSA-65"
STATUS_FIELDS = frozenset(
    (
        "schema",
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "committee_epoch",
        "registry_root",
        "lock_id",
        "decision",
        "operation",
        "fence_digest",
        "anchored_at_or_before_height",
        "observed_height",
        "validator_id",
        "signature_hex",
    )
)
COMMITTEE_FIELDS = frozenset(
    (
        "schema",
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "committee_epoch",
        "valid_from_height",
        "new_orders_through_height",
        "registry_root",
        "quorum",
        "validators",
    )
)
# Matched with `fullmatch` and lengths checked first: `$` would also accept
# a trailing newline, which the Rust checks refuse.
_HEX = re.compile(r"[0-9a-f]+")
_OPERATIONS = {None: 0, "transfer": 1, "unwrap": 2}
_U32 = 2**32
_U64 = 2**64


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
    return isinstance(value, str) and len(value) == 96 and _HEX.fullmatch(value) is not None


def _lower_hex(value: Any, max_len: int) -> bool:
    """Nonempty, even-length lowercase hex of at most ``max_len`` characters."""
    return (
        isinstance(value, str)
        and 0 < len(value) <= max_len
        and len(value) % 2 == 0
        and _HEX.fullmatch(value) is not None
    )


def _uint(value: Any, bound: int) -> bool:
    return type(value) is int and 0 <= value < bound


def expected_quorum(validator_count: int) -> int:
    """The protocol quorum for a committee of ``validator_count`` members."""
    return validator_count - (validator_count - 1) // 3 if validator_count > 0 else 0


def committee_root(committee: dict[str, Any]) -> str:
    """The registry root recomputed from the members, as Rust's `computed_root`."""
    domain = COMMITTEE_ROOT_DOMAINS.get(committee.get("schema"))
    validators = committee.get("validators")
    if domain is None:
        raise ValueError("schema")
    if (
        not isinstance(validators, list)
        or not 0 < len(validators) <= MAX_COMMITTEE_VALIDATORS
        or not isinstance(committee.get("chain_id"), str)
        or not committee["chain_id"]
        or not _hex96(committee.get("genesis_hash"))
        or not _uint(committee.get("protocol_version"), _U32)
        or not _uint(committee.get("committee_epoch"), _U64)
        or committee["committee_epoch"] == 0
        or not _uint(committee.get("valid_from_height"), _U64)
        or committee["valid_from_height"] == 0
        or not _uint(committee.get("new_orders_through_height"), _U64)
        or committee["new_orders_through_height"] < committee["valid_from_height"]
    ):
        raise ValueError("size_or_epoch")
    out = bytearray(domain.encode() + b"\0")
    out += _text(committee["chain_id"]) + _text(committee["genesis_hash"])
    out += struct.pack(">IQ", committee["protocol_version"], committee["committee_epoch"])
    if committee["schema"].endswith("-v2"):
        out += struct.pack(">QQ", committee["valid_from_height"], committee["new_orders_through_height"])
    out += struct.pack(">Q", len(validators))
    previous = None
    for member in validators:
        if not isinstance(member, dict) or set(member) != {"validator_id", "algorithm_id", "public_key_hex"}:
            raise ValueError("member_shape")
        validator_id, key = member["validator_id"], member["public_key_hex"]
        if not isinstance(validator_id, str) or not validator_id or member["algorithm_id"] != ML_DSA_65:
            raise ValueError("member_shape")
        if previous is not None and previous.encode() >= validator_id.encode():
            raise ValueError("member_order")
        if not _lower_hex(key, MAX_PUBLIC_KEY_HEX_LEN):
            raise ValueError("member_key")
        out += _text(validator_id) + _text(ML_DSA_65) + _text(key)
        previous = validator_id
    return hashlib.sha3_384(domain.encode() + b"\0" + bytes(out)).hexdigest()


def validate_committee(committee: Any, pinned_registry_root: str) -> dict[str, str]:
    """Validate ``committee`` as Rust does and against the pinned root.

    Returns the member map; raises ``FenceStatusRefused`` with reason
    ``invalid_committee`` (``detail["problem"]`` names the check) or
    ``untrusted_committee``.
    """
    if not isinstance(committee, dict) or set(committee) != COMMITTEE_FIELDS:
        raise FenceStatusRefused("invalid_committee", problem="shape")
    try:
        root = committee_root(committee)
    except ValueError as error:
        raise FenceStatusRefused("invalid_committee", problem=str(error)) from None
    if type(committee["quorum"]) is not int or committee["quorum"] != expected_quorum(len(committee["validators"])):
        raise FenceStatusRefused("invalid_committee", problem="quorum")
    if committee["registry_root"] != root:
        raise FenceStatusRefused("invalid_committee", problem="registry_root")
    if committee["registry_root"] != pinned_registry_root:
        raise FenceStatusRefused("untrusted_committee")
    return {member["validator_id"]: member["public_key_hex"] for member in committee["validators"]}


def _decision_key(decision: Any) -> tuple[Any, ...] | None:
    """The signed decision as a tuple, or ``None`` when it is not one."""
    if decision is None:
        return ("none",)
    if decision == "cancelled":
        return ("cancelled",)
    if isinstance(decision, dict) and set(decision) == {"confirmed"}:
        confirmed = decision["confirmed"]
        if (
            isinstance(confirmed, dict)
            and set(confirmed) == {"order_digest", "certificate_digest"}
            and _hex96(confirmed["order_digest"])
            and _hex96(confirmed["certificate_digest"])
        ):
            return ("confirmed", confirmed["order_digest"], confirmed["certificate_digest"])
    return None


def _shape_ok(status: Any) -> bool:
    if not isinstance(status, dict) or set(status) != STATUS_FIELDS:
        return False
    if (
        status["schema"] != FENCE_STATUS_SCHEMA
        or not isinstance(status["chain_id"], str)
        or not status["chain_id"]
        or not isinstance(status["validator_id"], str)
        or not status["validator_id"]
        or not isinstance(status["signature_hex"], str)
        or not _uint(status["protocol_version"], _U32)
        or not _uint(status["committee_epoch"], _U64)
        or status["committee_epoch"] < 1
        or not _uint(status["observed_height"], _U64)
        or not all(_hex96(status[f]) for f in ("genesis_hash", "registry_root", "lock_id"))
    ):
        return False
    decision = _decision_key(status["decision"])
    if decision is None:
        return False
    terminal = [status[f] for f in ("operation", "fence_digest", "anchored_at_or_before_height")]
    if decision == ("none",):
        return all(value is None for value in terminal)
    operation, digest, height = terminal
    return (
        operation in ("transfer", "unwrap")
        and _hex96(digest)
        and _uint(height, _U64)
        and 0 < height <= status["observed_height"]
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
    decision = _decision_key(status["decision"])
    if decision[0] == "none":
        out.append(0)
    elif decision[0] == "cancelled":
        out.append(2)
    else:
        out.append(1)
        out += _text(decision[1]) + _text(decision[2])
    out.append(_OPERATIONS[status["operation"]])
    if status["fence_digest"] is None:
        out.append(0)
    else:
        out += b"\x01" + _text(status["fence_digest"])
    if status["anchored_at_or_before_height"] is None:
        out.append(0)
    else:
        out += b"\x01" + struct.pack(">Q", status["anchored_at_or_before_height"])
    out += struct.pack(">Q", status["observed_height"])
    out += _text(status["validator_id"])
    return bytes(out)


def _answer(status: dict[str, Any]) -> tuple[Any, ...]:
    """What a quorum must agree on: only signed, recognised fields."""
    return (_decision_key(status["decision"]), status["operation"], status["fence_digest"])


def _fault_bound(committee: dict[str, Any]) -> int:
    """At most this many of the committee's ``n`` members are faulty."""
    return (len(committee["validators"]) - 1) // 3


def _fault(status: Any, committee: dict[str, Any], members: dict[str, str], lock_id: str, verify) -> str | None:
    """One status's own checks: ``None`` when valid, else the fault's name."""
    try:
        message = signing_bytes(status)
    except (KeyError, TypeError, ValueError, struct.error):
        return "malformed"
    signature = status["signature_hex"]
    if not _lower_hex(signature, MAX_SIGNATURE_HEX_LEN):
        return "malformed"
    if status["lock_id"] != lock_id:
        return "wrong_lock"
    if (status["chain_id"], status["genesis_hash"], status["protocol_version"]) != (
        committee["chain_id"],
        committee["genesis_hash"],
        committee["protocol_version"],
    ):
        return "wrong_domain"
    if (status["committee_epoch"], status["registry_root"]) != (
        committee["committee_epoch"],
        committee["registry_root"],
    ):
        return "wrong_epoch"
    if status["validator_id"] not in members:
        return "non_member"
    if not verify(members[status["validator_id"]], message, signature):
        return "bad_signature"
    return None


def verify_fence_status_quorum(
    statuses: Iterable[Any],
    committee: dict[str, Any],
    lock_id: str,
    verify: Callable[[str, bytes, str], bool],
    *,
    pinned_registry_root: str,
) -> dict[str, Any]:
    """Return the proved answer, or raise ``FenceStatusRefused``.

    ``pinned_registry_root`` is required and must come from the caller's
    trusted configuration (a pinned committee or root), not from the gateway
    or RPC answer that supplied ``committee`` or ``statuses``.

    Each status is checked on its own; one that fails is listed in the
    result's ``invalid`` (``validator_id``, ``reason``) and does not affect
    the others. Valid statuses are grouped by answer, each member counted
    once per answer. A terminal answer reaching the quorum is proved, else
    "not terminal" if it does. Valid statuses that contradict the proved
    answer (another terminal answer, or "not terminal" at or above the
    quorum-supported bound; against "not terminal", a terminal answer at or
    below the quorum-supported height) are returned unchanged in
    ``contradicting``: signed evidence for operators that never changes the
    answer.

    A "not terminal" status contradicts a terminal quorum only when observed
    at or above the quorum-supported bound, the ``(f + 1)``-th smallest bound
    of its distinct signers (``f = (n - 1) // 3``): at least one honest signer
    claims a bound at or below it, so up to ``f`` false bounds cannot move it
    below the true anchoring height. A signer whose own bound is below it is
    listed in ``inconsistent_bounds``, not blamed and not counted toward a
    refusal.

    Likewise a terminal status contradicts a "not terminal" quorum only when
    anchored at or below the quorum-supported height, the ``(f + 1)``-th
    largest observed height of its distinct signers: at least one honest
    signer observed "not terminal" at or above it, so up to ``f`` false high
    heights cannot move it above the true one. A signer observed above it is
    listed in ``inconsistent_bounds`` on the same terms.

    Assumes at most the committee's fault bound, ``(n - 1) // 3`` of ``n``
    members, is faulty: a faulty minority can neither prove nor veto an
    answer. When the contradicting statuses also come from a quorum of
    distinct members, more members than that are faulty and the set is
    refused as ``conflicting``. No answer at quorum: ``below_quorum``.
    """
    statuses = list(statuses)
    members = validate_committee(committee, pinned_registry_root)
    quorum = committee["quorum"]
    invalid: list[dict[str, str]] = []
    valid: list[dict[str, Any]] = []
    groups: dict[tuple[Any, ...], list[dict[str, Any]]] = {}
    for status in statuses:
        claimed = status.get("validator_id") if isinstance(status, dict) else None
        claimed = claimed if isinstance(claimed, str) else ""
        reason = _fault(status, committee, members, lock_id, verify)
        group = groups.get(_answer(status), []) if reason is None else []
        if reason is None and any(s["validator_id"] == claimed for s in group):
            reason = "duplicate_validator"
        if reason is not None:
            invalid.append({"validator_id": claimed, "reason": reason})
            continue
        groups.setdefault(_answer(status), []).append(status)
        valid.append(status)
    reached = [group for group in groups.values() if len(group) >= quorum]
    reached.sort(key=lambda group: group[0]["decision"] is None)
    if not reached:
        agreeing = max((len(group) for group in groups.values()), default=0)
        raise FenceStatusRefused("below_quorum", agreeing=agreeing, quorum=quorum)
    best = reached[0]
    # The quorum-supported bound: the (f + 1)-th smallest of the group's
    # bounds, so at least one honest member claims one at or below it.
    bounds = sorted(s["anchored_at_or_before_height"] for s in best if s["decision"] is not None)
    supported = bounds[min(_fault_bound(committee), len(bounds) - 1)] if bounds else None
    if supported is None:
        # The quorum-supported height: the (f + 1)-th largest of the group's
        # observed heights, so at least one honest member observed "not
        # terminal" at or above it.
        heights = sorted((s["observed_height"] for s in best), reverse=True)
        seen = heights[min(_fault_bound(committee), len(heights) - 1)]
        contradicting = [
            s for s in valid if s["anchored_at_or_before_height"] is not None and s["anchored_at_or_before_height"] <= seen
        ]
        inconsistent = [s for s in best if s["observed_height"] > seen]
    else:
        contradicting = [
            s
            for s in valid
            if (_answer(s) != _answer(best[0]) if s["decision"] is not None else s["observed_height"] >= supported)
        ]
        inconsistent = [s for s in best if s["anchored_at_or_before_height"] < supported]
    if len({s["validator_id"] for s in contradicting}) >= quorum:
        raise FenceStatusRefused("conflicting")
    first = best[0]
    heights = [s["anchored_at_or_before_height"] for s in best]
    return {
        "lock_id": lock_id,
        "committee_epoch": committee["committee_epoch"],
        "decision": first["decision"],
        "operation": first["operation"],
        "fence_digest": first["fence_digest"],
        "anchored_at_or_before_height": None if None in heights else max(heights),
        "observed_height": min(s["observed_height"] for s in best),
        "signers": sorted(s["validator_id"] for s in best),
        "quorum": quorum,
        "contradicting": contradicting,
        "invalid": invalid,
        "inconsistent_bounds": inconsistent,
    }
