"""The postfiat-validator-uptime-v1 contract as code.

This is the executable form of docs/specs/validator-uptime-feed-v1.md: what an
observer signs, how a row follows from its facts, how a consumer reads a feed,
and the access lifecycle. It has no dependency of its own. The caller supplies
the ML-DSA-65 check (`verify_signature`) and the JSON Schema validator.

Used by scripts/test-validator-uptime-schema.py (the conformance test) and by
scripts/validator-uptime-observer.py (the observer).
"""

from __future__ import annotations

import copy
import datetime as dt
import hashlib
import json
from typing import Any, Callable

# (public_key_hex, message, signature_hex) -> the signature verifies under the uptime context
VerifySignature = Callable[[str, bytes, str], bool]

UTC = dt.timezone.utc
DAY = dt.timedelta(days=1)
SIGNING_CONTEXT = b"postfiat-l1-v2/validator-uptime-feed/v1"
EVALUATION_HORIZON_DAYS = 30
MIN_OBSERVED_DAYS = 25
LIVENESS_THRESHOLD_BPS = 9500
GRACE_PERIOD_DAYS = 14
RECOVERY_CONSECUTIVE_DAYS = 3
STALE_AFTER_SECONDS = 3600
MAX_FUTURE_SKEW_SECONDS = 60
QUORUM_THRESHOLD = 2

# The fields two observers must share before their claims count as agreement:
# who the validator is, where it stands, and everything a consumer acts on
# (access follows from access_state; the credit and the grace date are named).
AGREEMENT_FIELDS = (
    "ml_dsa_public_key",
    "registry_active",
    "liveness_met",
    "access_state",
    "reward_eligible",
    "grace_started_at",
)


class FeedRejected(ValueError):
    """The feed as a whole is malformed or its rows are not what the signed claims give."""


# --------------------------------------------------------------------------- time and bytes


def parse_utc(raw: str) -> dt.datetime:
    return dt.datetime.fromisoformat(raw.replace("Z", "+00:00")).astimezone(UTC)


def format_utc(value: dt.datetime) -> str:
    return value.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_date(raw: str) -> dt.date:
    return dt.date.fromisoformat(raw)


def midnight(day: dt.date) -> dt.datetime:
    return dt.datetime(day.year, day.month, day.day, tzinfo=UTC)


def canonical_json_bytes(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def observer_claim_body(feed: dict[str, Any], att: dict[str, Any]) -> dict[str, Any]:
    """The exact object an observer signs. Everything a consumer acts on is in here."""
    return {
        "chain": feed["chain"],
        "committee_epoch": att["committee_epoch"],
        "finalized_block_hash": att["finalized_block_hash"],
        "finalized_height": att["finalized_height"],
        "observed_at": att["observed_at"],
        "observer_id": att["observer_id"],
        "operator_group": att["operator_group"],
        "quorum_policy": feed["quorum_policy"],
        "region": att["region"],
        "registry_root": att["registry_root"],
        "schema": feed["schema"],
        "schema_version": feed["schema_version"],
        "validator_claims": att["validator_claims"],
        "window_spec": feed["window_spec"],
    }


def claim_digest(feed: dict[str, Any], att: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json_bytes(observer_claim_body(feed, att))).hexdigest()


# --------------------------------------------------------------------------- one validator row


def scoring_days(row: dict[str, Any], window_end: dt.date) -> list[dict[str, Any]]:
    """Complete days with at least one committed block. After a recovery, only days from the floor on."""
    days = [d for d in row["days"] if d["complete"] and d["committed_blocks"] > 0]
    floor = row["recovery_floor_date"]
    if floor is not None and window_end < parse_date(floor) + EVALUATION_HORIZON_DAYS * DAY:
        days = [d for d in days if parse_date(d["date"]) >= parse_date(floor)]
    return days


def derive_row(
    *,
    validator_id: str,
    ml_dsa_public_key: str,
    registry_active: bool,
    access_state: str,
    first_active_date: str | None,
    grace_started_at: str | None,
    recovery_floor_date: str | None,
    days: list[dict[str, Any]],
    window_end: dt.date,
) -> dict[str, Any]:
    """Build the full row from the facts an observer holds. Every other field follows from these."""
    row: dict[str, Any] = {
        "validator_id": validator_id,
        "ml_dsa_public_key": ml_dsa_public_key,
        "registry_active": registry_active,
        "access_state": access_state,
        "first_active_date": first_active_date,
        "grace_started_at": grace_started_at,
        "recovery_floor_date": recovery_floor_date,
        "days": days,
    }
    scored = scoring_days(row, window_end)
    observed = len(scored)
    mean = sum(d["participation_bps"] for d in scored) // observed if observed else None
    streak = 0
    for day in reversed(scored):
        if day["participation_bps"] < LIVENESS_THRESHOLD_BPS:
            break
        streak += 1
    ramp_in = (
        first_active_date is not None
        and window_end < parse_date(first_active_date) + EVALUATION_HORIZON_DAYS * DAY
    )
    in_recovery = (
        recovery_floor_date is not None
        and window_end < parse_date(recovery_floor_date) + EVALUATION_HORIZON_DAYS * DAY
    )
    # Outside the ramp-in and a recovery, fewer than MIN_OBSERVED_DAYS scored
    # days are too few to judge: liveness is not failed on them. Access stays
    # and the credit waits (`insufficient_observed_days`). Inside the ramp-in
    # or a recovery every scored day counts from the first.
    too_few_to_judge = not ramp_in and not in_recovery and observed < MIN_OBSERVED_DAYS
    liveness_met = registry_active and (observed == 0 or too_few_to_judge or mean >= LIVENESS_THRESHOLD_BPS)

    if access_state == "pending":
        reason, hub, reward, removal = "awaiting_ratification", False, False, False
    elif access_state == "active":
        hub, removal = True, False
        if ramp_in:
            reason, reward = "day0_ramp_in", True
        elif in_recovery:
            reason, reward = "recovered_3day_streak", True
        elif observed >= MIN_OBSERVED_DAYS:
            reason, reward = "healthy_liveness", True
        else:
            # Too few days with blocks to judge. Access stays, the credit waits.
            reason, reward = "insufficient_observed_days", False
    elif access_state == "lapsed":
        reason = "below_liveness_threshold" if registry_active else "removed_from_registry"
        hub, reward, removal = True, False, False
    elif access_state == "revoked":
        reason = "grace_expired" if registry_active else "removed_from_registry"
        hub, reward, removal = False, False, registry_active
    else:
        raise ValueError(f"unknown access_state {access_state}")

    grace_expires_at = None
    if grace_started_at is not None:
        grace_expires_at = format_utc(parse_utc(grace_started_at) + GRACE_PERIOD_DAYS * DAY)

    row.update(
        {
            "ramp_in_active": ramp_in,
            "observed_days": observed,
            "mean_daily_participation_bps": mean,
            "consecutive_passing_days": streak,
            "liveness_met": liveness_met,
            "access_state_reason": reason,
            "grace_expires_at": grace_expires_at,
            "removal_proposal_eligible": removal,
            "reward_eligible": reward,
            "finance_hub_access": hub,
            "research_api_keys_active": hub,
        }
    )
    return row


def check_row(row: dict[str, Any], window_start: dt.date, window_end: dt.date) -> None:
    """Refuse a signed claim row that does not follow from its own facts."""
    vid = row["validator_id"]
    state = row["access_state"]
    first = row["first_active_date"]
    if (state == "pending") != (first is None):
        raise ValueError(f"{vid}: first_active_date is null exactly while pending")
    if state == "pending" and row["registry_active"]:
        raise ValueError(f"{vid}: a pending validator is not in the registry")
    if state == "active" and not row["registry_active"]:
        raise ValueError(f"{vid}: an active validator is in the registry")
    if (state == "lapsed") != (row["grace_started_at"] is not None):
        raise ValueError(f"{vid}: grace_started_at is set exactly while lapsed")
    if row["grace_started_at"] is not None:
        started = parse_utc(row["grace_started_at"])
        if started != midnight(started.date()) or started.date() > window_end:
            raise ValueError(f"{vid}: grace starts at a UTC midnight, not after the evaluation day")
    floor = row["recovery_floor_date"]
    if floor is not None:
        if state != "active" or not window_end < parse_date(floor) + EVALUATION_HORIZON_DAYS * DAY:
            raise ValueError(f"{vid}: recovery_floor_date outlived its 30 days or its active state")

    expected_dates: list[dt.date] = []
    if first is not None:
        day = max(parse_date(first), window_start)
        while day < window_end:
            expected_dates.append(day)
            day += DAY
    if [parse_date(d["date"]) for d in row["days"]] != expected_dates:
        raise ValueError(f"{vid}: days must list every window day from first_active_date, once, in order")
    last_to = 0
    for day in row["days"]:
        label = f"{vid} {day['date']}"
        if day["signed_blocks"] > day["committed_blocks"]:
            raise ValueError(f"{label}: signed_blocks exceeds committed_blocks")
        if (day["from_height"] is None) != (day["to_height"] is None):
            raise ValueError(f"{label}: from_height and to_height are set together")
        if day["from_height"] is None:
            if day["committed_blocks"] != 0:
                raise ValueError(f"{label}: blocks without a height range")
        else:
            if day["from_height"] <= last_to or day["to_height"] < day["from_height"]:
                raise ValueError(f"{label}: height ranges must ascend")
            if day["committed_blocks"] > day["to_height"] - day["from_height"] + 1:
                raise ValueError(f"{label}: more blocks than heights")
            last_to = day["to_height"]
        expected_bps = None
        if day["complete"] and day["committed_blocks"] > 0:
            expected_bps = day["signed_blocks"] * 10000 // day["committed_blocks"]
        if day["participation_bps"] != expected_bps:
            raise ValueError(f"{label}: participation_bps is {day['participation_bps']}, expected {expected_bps}")

    derived = derive_row(
        validator_id=vid,
        ml_dsa_public_key=row["ml_dsa_public_key"],
        registry_active=row["registry_active"],
        access_state=state,
        first_active_date=first,
        grace_started_at=row["grace_started_at"],
        recovery_floor_date=floor,
        days=row["days"],
        window_end=window_end,
    )
    for key, value in derived.items():
        if row[key] != value:
            raise ValueError(f"{vid}: {key} is {row[key]!r}, the signed facts give {value!r}")
    if state == "active" and not row["liveness_met"]:
        raise ValueError(f"{vid}: active without liveness")


# --------------------------------------------------------------------------- the feed


def check_attestation(
    feed: dict[str, Any],
    att: dict[str, Any],
    keyset: dict[str, Any],
    now_utc: dt.datetime,
    last_accepted: dict[str, str],
    verify_signature: VerifySignature,
) -> None:
    """Raise ValueError with the reason when one observer's attestation must not count."""
    pinned = {o["observer_id"]: o for o in keyset["observers"]}.get(att["observer_id"])
    if pinned is None:
        raise ValueError("observer is not in the pinned keyset")
    for field in ("public_key_hex", "region", "operator_group"):
        if att[field] != pinned[field]:
            raise ValueError(f"{field} differs from the pinned keyset")
    body = canonical_json_bytes(observer_claim_body(feed, att))
    if att["claim_digest_sha256"] != hashlib.sha256(body).hexdigest():
        raise ValueError("claim_digest_sha256 does not match the claim body")
    if not verify_signature(pinned["public_key_hex"], body, att["signature_hex"]):
        raise ValueError("ML-DSA-65 signature does not verify")

    observed_at = parse_utc(att["observed_at"])
    if (observed_at - now_utc).total_seconds() > MAX_FUTURE_SKEW_SECONDS:
        raise ValueError("observed_at is in the future")
    if (now_utc - observed_at).total_seconds() > STALE_AFTER_SECONDS:
        raise ValueError("stale")
    previous = last_accepted.get(att["observer_id"])
    if previous is not None and observed_at < parse_utc(previous):
        raise ValueError("older than the last attestation accepted from this observer")
    window_start = parse_utc(feed["window_spec"]["window_start"])
    window_end = parse_utc(feed["window_spec"]["window_end"])
    if not window_end <= observed_at < window_end + DAY:
        raise ValueError("observed_at is outside the evaluation day of this window")

    ids = [c["validator_id"] for c in att["validator_claims"]]
    if ids != sorted(set(ids)):
        raise ValueError("validator_claims must be sorted by validator_id without repeats")
    keys = [c["ml_dsa_public_key"] for c in att["validator_claims"]]
    if len(set(keys)) != len(keys):
        raise ValueError("two validators share one key")
    for claim in att["validator_claims"]:
        check_row(claim, window_start.date(), window_end.date())


def reconcile(attestations: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[str]]:
    """Rows from the verified attestations, and the validators that have no quorum.

    Each row is one observer's signed claim, copied whole. With three agreeing
    observers it is the one with the middle score, with two the lower score;
    ties go to the lower observer_id. An unknown score counts as the lowest.
    """
    by_validator: dict[str, list[tuple[str, dict[str, Any]]]] = {}
    for att in attestations:
        for claim in att["validator_claims"]:
            by_validator.setdefault(claim["validator_id"], []).append((att["observer_id"], claim))
    rows, no_quorum = [], []
    for vid in sorted(by_validator):
        groups: dict[str, list[tuple[str, dict[str, Any]]]] = {}
        for oid, claim in by_validator[vid]:
            groups.setdefault(json.dumps([claim[f] for f in AGREEMENT_FIELDS]), []).append((oid, claim))
        group = max(groups.values(), key=len)
        if len(group) < QUORUM_THRESHOLD:
            no_quorum.append(vid)
            continue
        ordered = sorted(
            group,
            key=lambda item: (
                -1 if item[1]["mean_daily_participation_bps"] is None else item[1]["mean_daily_participation_bps"],
                item[0],
            ),
        )
        source_id, source = ordered[1] if len(ordered) == 3 else ordered[0]
        rows.append(
            {**copy.deepcopy(source), "source_observer_id": source_id, "agreeing_observer_ids": sorted(o for o, _ in group)}
        )
    return rows, no_quorum


def verify_feed(
    feed: dict[str, Any],
    *,
    schema: Any,
    keyset: dict[str, Any],
    now_utc: dt.datetime,
    verify_signature: VerifySignature,
    last_accepted: dict[str, str] | None = None,
) -> dict[str, Any]:
    """Section 4 of the spec. `schema` is a jsonschema Draft202012Validator for the feed schema."""
    errors = sorted(schema.iter_errors(feed), key=lambda err: list(err.absolute_path))
    if errors:
        path = ".".join(str(p) for p in errors[0].absolute_path) or "<root>"
        raise FeedRejected(f"schema: {path}: {errors[0].message}")
    if feed["chain"] != keyset["chain"]:
        raise FeedRejected("feed is for another chain than the pinned keyset")
    if feed["quorum_policy"]["observer_keyset_id"] != keyset["keyset_id"]:
        raise FeedRejected("feed names another observer keyset than the pinned one")
    window_start = parse_utc(feed["window_spec"]["window_start"])
    window_end = parse_utc(feed["window_spec"]["window_end"])
    if window_end != midnight(window_end.date()) or window_end - window_start != EVALUATION_HORIZON_DAYS * DAY:
        raise FeedRejected("the window is 30 whole UTC days")
    seen: set[str] = set()
    for att in feed["observer_attestations"]:
        if att["observer_id"] in seen:
            raise FeedRejected(f"observer {att['observer_id']} appears twice")
        seen.add(att["observer_id"])

    verified, rejected = [], {}
    for att in feed["observer_attestations"]:
        try:
            check_attestation(feed, att, keyset, now_utc, last_accepted or {}, verify_signature)
            verified.append(att)
        except ValueError as exc:
            rejected[att["observer_id"]] = str(exc)

    indeterminate = len(verified) < QUORUM_THRESHOLD
    result = {
        "verified_observers": sorted(a["observer_id"] for a in verified),
        "rejected_observers": rejected,
        "indeterminate": indeterminate,
        # An indeterminate feed blocks new bindings and new credit. It revokes nobody.
        "allow_new_bindings": not indeterminate,
        "allow_credit_grants": not indeterminate,
        "revoke_existing_access": False,
        "validators": [],
        "validators_without_quorum": [],
        "accepted": {a["observer_id"]: a["observed_at"] for a in verified},
    }
    if indeterminate:
        return result
    rows, no_quorum = reconcile(verified)
    if feed["validators"] != rows:
        raise FeedRejected("the validators array is not what the verified signed claims give")
    result["validators"] = rows
    result["validators_without_quorum"] = no_quorum
    return result


# --------------------------------------------------------------------------- lifecycle


def next_row(
    previous: dict[str, Any] | None,
    *,
    validator_id: str,
    ml_dsa_public_key: str,
    registry_active: bool,
    days: list[dict[str, Any]],
    window_end: dt.date,
    observed_at: dt.datetime,
) -> dict[str, Any]:
    """What one observer writes for a validator, given what it wrote before and what it sees now."""
    state = previous["access_state"] if previous else "pending"
    first = previous["first_active_date"] if previous else None
    grace = previous["grace_started_at"] if previous else None
    floor = previous["recovery_floor_date"] if previous else None
    if floor is not None and not window_end < parse_date(floor) + EVALUATION_HORIZON_DAYS * DAY:
        floor = None

    def build(new_state: str, new_grace: str | None, new_floor: str | None) -> dict[str, Any]:
        return derive_row(
            validator_id=validator_id,
            ml_dsa_public_key=ml_dsa_public_key,
            registry_active=registry_active,
            access_state=new_state,
            first_active_date=first,
            grace_started_at=new_grace,
            recovery_floor_date=new_floor,
            days=days,
            window_end=window_end,
        )

    if state == "pending":
        if not registry_active:
            return build("pending", None, None)
        first = window_end.isoformat()
        return build("active", None, None)

    if state == "active":
        row = build("active", None, floor)
        if row["liveness_met"]:
            return row
        return build("lapsed", format_utc(midnight(window_end)), None)

    # lapsed or revoked
    probe = build(state, grace, None)
    if registry_active and probe["consecutive_passing_days"] >= RECOVERY_CONSECUTIVE_DAYS:
        streak_start = scoring_days(probe, window_end)[-RECOVERY_CONSECUTIVE_DAYS]["date"]
        return build("active", None, streak_start)
    if state == "lapsed" and observed_at >= parse_utc(probe["grace_expires_at"]):
        return build("revoked", None, None)
    return probe
