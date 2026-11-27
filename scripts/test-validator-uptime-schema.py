#!/usr/bin/env python3
"""Reference verifier and test for the postfiat-validator-uptime-v1 feed.

This file is the executable form of docs/specs/validator-uptime-feed-v1.md.

  python3 scripts/test-validator-uptime-schema.py                  run every check
  python3 scripts/test-validator-uptime-schema.py --write-fixture  rebuild the fixture files

What is checked:
  1. The feed and observer-keyset JSON Schemas are valid Draft 2020-12.
  2. The committed fixture equals the one this script builds (same bytes).
  3. Every observer signature is a real ML-DSA-65 signature, verified against
     the pinned observer keyset. Nothing in the feed is trusted without it.
  4. Every field of a validator row is either signed directly or recomputed
     from signed fields (derive_row), so whoever serves the feed controls none.
  5. One wrong observer does not stop the feed; two wrong observers make it
     indeterminate, which blocks new grants and never revokes anyone.
  6. The access lifecycle: pending, active, lapsed (14 days of grace), revoked.

The fixture keys come from fixed public seeds below. They are test keys.
"""

from __future__ import annotations

import copy
import datetime as dt
import hashlib
import json
from pathlib import Path
import sys
from typing import Any

from dilithium_py.ml_dsa import ML_DSA_65
import jsonschema
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "docs/specs/schemas"
SCHEMA_PATH = SCHEMA_DIR / "postfiat-validator-uptime-v1.schema.json"
FIXTURE_PATH = SCHEMA_DIR / "postfiat-validator-uptime-v1.fixture.json"
KEYSET_SCHEMA_PATH = SCHEMA_DIR / "postfiat-observer-keyset-v1.schema.json"
KEYSET_FIXTURE_PATH = SCHEMA_DIR / "postfiat-observer-keyset-v1.fixture.json"

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

# The fields two observers must share before their claims count as agreement.
AGREEMENT_FIELDS = ("ml_dsa_public_key", "registry_active", "liveness_met", "access_state")


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
    liveness_met = registry_active and (observed == 0 or mean >= LIVENESS_THRESHOLD_BPS)

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
    if not ML_DSA_65.verify(
        bytes.fromhex(pinned["public_key_hex"]), body, bytes.fromhex(att["signature_hex"]), ctx=SIGNING_CONTEXT
    ):
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
    schema: Draft202012Validator,
    keyset: dict[str, Any],
    now_utc: dt.datetime,
    last_accepted: dict[str, str] | None = None,
) -> dict[str, Any]:
    errors = sorted(schema.iter_errors(feed), key=lambda err: list(err.absolute_path))
    if errors:
        path = ".".join(str(p) for p in errors[0].absolute_path) or "<root>"
        raise FeedRejected(f"schema: {path}: {errors[0].message}")
    if feed["chain"] != keyset["chain"]:
        raise FeedRejected("feed is for another chain than the pinned keyset")
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
            check_attestation(feed, att, keyset, now_utc, last_accepted or {})
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


# --------------------------------------------------------------------------- fixture

FIXTURE_WINDOW_END = dt.date(2026, 11, 27)
FIXTURE_CHAIN = {
    "network": "l1-testnet",
    "chain_id": "postfiat-testnet-1",
    "genesis_hash": hashlib.sha3_384(b"postfiat-validator-uptime-v1 fixture genesis").hexdigest(),
}
FIXTURE_OBSERVERS = [
    ("observer-us-east-1", "us-east", "postfiat-foundation", "2026-11-27T01:38:00Z"),
    ("observer-eu-central-2", "eu-central", "independent-operator-a", "2026-11-27T01:39:00Z"),
    ("observer-ap-northeast-3", "ap-northeast", "independent-operator-b", "2026-11-27T01:40:00Z"),
]
IDLE_DAYS = {dt.date(2026, 11, 1), dt.date(2026, 11, 8)}  # the chain made no block on these days
BLOCKS_PER_BUSY_DAY = 200


def fixture_keypair(label: str) -> tuple[bytes, bytes]:
    return ML_DSA_65.key_derive(hashlib.sha256(f"postfiat-validator-uptime-v1 fixture key {label}".encode()).digest())


def fixture_days(first: dt.date, signed_for: Any, incomplete: set[dt.date]) -> list[dict[str, Any]]:
    window_start = FIXTURE_WINDOW_END - EVALUATION_HORIZON_DAYS * DAY
    days, height, day = [], 1000, window_start
    while day < FIXTURE_WINDOW_END:
        committed = 0 if day in IDLE_DAYS else BLOCKS_PER_BUSY_DAY
        from_height, to_height = (None, None) if committed == 0 else (height + 1, height + committed)
        height += committed
        if day >= first:
            complete = day not in incomplete
            signed = min(committed, signed_for(day))
            days.append(
                {
                    "date": day.isoformat(),
                    "complete": complete,
                    "from_height": from_height,
                    "to_height": to_height,
                    "committed_blocks": committed,
                    "signed_blocks": signed,
                    "participation_bps": signed * 10000 // committed if complete and committed else None,
                }
            )
        day += DAY
    return days


def build_fixture() -> tuple[dict[str, Any], dict[str, Any]]:
    window_start = FIXTURE_WINDOW_END - EVALUATION_HORIZON_DAYS * DAY
    d = dt.date
    validator_keys = [fixture_keypair(f"validator-{i}")[0].hex() for i in range(6)]

    def claims_for(observer_index: int) -> list[dict[str, Any]]:
        # Observer 3 polled across midnight once, so one day is incomplete for it.
        incomplete = {d(2026, 11, 15)} if observer_index == 2 else set()
        # Observers see the same certificates, so they count the same votes.
        specs = [
            # healthy for the whole window
            dict(state="active", first=window_start, signed=lambda day: 198, grace=None, floor=None),
            # ratified four days ago: day-0 ramp-in
            dict(state="active", first=d(2026, 11, 23), signed=lambda day: 197, grace=None, floor=None),
            # six days offline, then three passing days: recovered
            dict(
                state="active",
                first=window_start,
                signed=lambda day: 0 if d(2026, 11, 18) <= day <= d(2026, 11, 23) else 196,
                grace=None,
                floor="2026-11-24",
            ),
            # below 95% since 20 November, in grace since the 24th, one passing day so far
            dict(
                state="lapsed",
                first=window_start,
                signed=lambda day: 150 if d(2026, 11, 20) <= day <= d(2026, 11, 25) else 196,
                grace="2026-11-24T00:00:00Z",
                floor=None,
            ),
            # offline since 5 November, grace ran out, still in the registry
            dict(
                state="revoked",
                first=window_start,
                signed=lambda day: 196 if day < d(2026, 11, 5) else 0,
                grace=None,
                floor=None,
            ),
            # applied, not ratified
            dict(state="pending", first=None, signed=None, grace=None, floor=None),
        ]
        claims = []
        for index, spec in enumerate(specs):
            claims.append(
                derive_row(
                    validator_id=f"validator-{index}",
                    ml_dsa_public_key=validator_keys[index],
                    registry_active=spec["state"] != "pending",
                    access_state=spec["state"],
                    first_active_date=spec["first"].isoformat() if spec["first"] else None,
                    grace_started_at=spec["grace"],
                    recovery_floor_date=spec["floor"],
                    days=fixture_days(spec["first"], spec["signed"], incomplete) if spec["first"] else [],
                    window_end=FIXTURE_WINDOW_END,
                )
            )
        return claims

    feed: dict[str, Any] = {
        "schema": "postfiat-validator-uptime-v1",
        "schema_version": 1,
        "generated_at": "2026-11-27T01:40:10Z",
        "chain": FIXTURE_CHAIN,
        "quorum_policy": {
            "observer_keyset_id": "postfiat-observer-keyset-fixture-v1",
            "observer_count": 3,
            "quorum_threshold": QUORUM_THRESHOLD,
            "algorithm_id": "ML-DSA-65",
            "signing_context": SIGNING_CONTEXT.decode(),
            "stale_after_seconds": STALE_AFTER_SECONDS,
            "max_future_skew_seconds": MAX_FUTURE_SKEW_SECONDS,
            "indeterminate_behavior": "block_new_bindings_and_credits_preserve_existing_access",
        },
        "window_spec": {
            "metric_version": "precommit-qc-observer-brackets-v1",
            "time_basis": "observer_brackets",
            "max_observation_gap_seconds": 900,
            "evaluation_horizon_days": EVALUATION_HORIZON_DAYS,
            "min_observed_days": MIN_OBSERVED_DAYS,
            "liveness_threshold_bps": LIVENESS_THRESHOLD_BPS,
            "grace_period_days": GRACE_PERIOD_DAYS,
            "recovery_consecutive_days": RECOVERY_CONSECUTIVE_DAYS,
            "window_start": format_utc(midnight(window_start)),
            "window_end": format_utc(midnight(FIXTURE_WINDOW_END)),
        },
        "validators": [],
        "observer_attestations": [],
    }
    keyset: dict[str, Any] = {
        "schema": "postfiat.observer_keyset.v1",
        "keyset_id": "postfiat-observer-keyset-fixture-v1",
        "chain": FIXTURE_CHAIN,
        "threshold": QUORUM_THRESHOLD,
        "total_observers": 3,
        "algorithm_id": "ML-DSA-65",
        "signing_context": SIGNING_CONTEXT.decode(),
        "observers": [],
    }
    tip_height = 1000 + BLOCKS_PER_BUSY_DAY * (EVALUATION_HORIZON_DAYS - len(IDLE_DAYS))
    for index, (oid, region, group, observed_at) in enumerate(FIXTURE_OBSERVERS):
        public_key, _ = fixture_keypair(oid)
        keyset["observers"].append(
            {"observer_id": oid, "region": region, "operator_group": group, "public_key_hex": public_key.hex()}
        )
        att = {
            "observer_id": oid,
            "region": region,
            "operator_group": group,
            "observed_at": observed_at,
            "committee_epoch": 12,
            "finalized_height": tip_height,
            "finalized_block_hash": hashlib.sha3_384(f"fixture block {tip_height}".encode()).hexdigest(),
            "registry_root": hashlib.sha3_384(b"fixture registry root").hexdigest(),
            "validator_claims": claims_for(index),
            "algorithm_id": "ML-DSA-65",
            "public_key_hex": public_key.hex(),
        }
        feed["observer_attestations"].append(att)
        sign_attestation(feed, att, oid)
    feed["validators"], _ = reconcile(feed["observer_attestations"])
    return feed, keyset


def sign_attestation(feed: dict[str, Any], att: dict[str, Any], key_label: str) -> None:
    body = canonical_json_bytes(observer_claim_body(feed, att))
    att["claim_digest_sha256"] = hashlib.sha256(body).hexdigest()
    att["signature_hex"] = ML_DSA_65.sign(fixture_keypair(key_label)[1], body, ctx=SIGNING_CONTEXT, deterministic=True).hex()


def dump_fixture(obj: Any) -> str:
    """Indented JSON with each day on one line, so the fixture stays readable."""
    text = json.dumps(obj, indent=2, ensure_ascii=False)
    marker = '"date": '
    out, lines, i = [], text.split("\n"), 0
    while i < len(lines):
        if lines[i].strip() == "{" and i + 1 < len(lines) and lines[i + 1].strip().startswith(marker):
            indent = lines[i][: len(lines[i]) - len(lines[i].lstrip())]
            j, parts = i + 1, []
            while not lines[j].strip().startswith("}"):
                parts.append(lines[j].strip())
                j += 1
            out.append(f"{indent}{{{' '.join(parts)}{lines[j].strip()}")
            i = j + 1
        else:
            out.append(lines[i])
            i += 1
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------- tests


def expect_rejected(label: str, fn: Any) -> None:
    try:
        fn()
    except FeedRejected:
        return
    raise AssertionError(f"expected the feed to be rejected: {label}")


def resign_all(feed: dict[str, Any]) -> None:
    for att in feed["observer_attestations"]:
        sign_attestation(feed, att, att["observer_id"])


def run_feed_tests(schema: Draft202012Validator, feed: dict[str, Any], keyset: dict[str, Any]) -> int:
    now = parse_utc(feed["generated_at"]) + dt.timedelta(seconds=30)
    check = lambda f, **kw: verify_feed(f, schema=schema, keyset=keyset, now_utc=kw.pop("now", now), **kw)  # noqa: E731
    cases = 0

    ok = check(feed)
    assert ok["verified_observers"] == sorted(o[0] for o in FIXTURE_OBSERVERS) and not ok["indeterminate"]
    assert ok["allow_new_bindings"] and ok["allow_credit_grants"] and not ok["validators_without_quorum"]
    states = {v["validator_id"]: (v["access_state"], v["access_state_reason"]) for v in ok["validators"]}
    assert states == {
        "validator-0": ("active", "healthy_liveness"),
        "validator-1": ("active", "day0_ramp_in"),
        "validator-2": ("active", "recovered_3day_streak"),
        "validator-3": ("lapsed", "below_liveness_threshold"),
        "validator-4": ("revoked", "grace_expired"),
        "validator-5": ("pending", "awaiting_ratification"),
    }, states
    cases += 1

    # Stale: valid, indeterminate, blocks new grants, revokes nobody.
    stale = check(feed, now=now + dt.timedelta(seconds=3700))
    assert stale["indeterminate"] and not stale["allow_new_bindings"] and not stale["allow_credit_grants"]
    assert stale["revoke_existing_access"] is False and stale["validators"] == []
    cases += 1

    # --- the signature layer
    def one_bad(mutate: Any, reason: str) -> None:
        """One observer is wrong: the served rows no longer match, and a rebuilt 2-of-3 feed passes."""
        bad = copy.deepcopy(feed)
        mutate(bad)
        expect_rejected(reason, lambda: check(bad))
        rebuilt = copy.deepcopy(bad)
        good = [a for a in rebuilt["observer_attestations"] if a["observer_id"] != FIXTURE_OBSERVERS[0][0]]
        rebuilt["validators"], _ = reconcile(good)
        result = check(rebuilt)
        assert list(result["rejected_observers"]) == [FIXTURE_OBSERVERS[0][0]], result["rejected_observers"]
        assert reason in result["rejected_observers"][FIXTURE_OBSERVERS[0][0]], result["rejected_observers"]
        assert not result["indeterminate"] and len(result["validators"]) == 6

    first = lambda f: f["observer_attestations"][0]  # noqa: E731
    one_bad(lambda f: first(f).update(signature_hex="00" * 3309), "signature does not verify")
    one_bad(lambda f: first(f).update(signature_hex=f["observer_attestations"][1]["signature_hex"]), "signature does not verify")
    one_bad(lambda f: first(f).update(claim_digest_sha256="0" * 64), "claim_digest_sha256")
    one_bad(lambda f: first(f).update(region="eu-west"), "region differs")
    cases += 4

    def self_signed_key(f: dict[str, Any]) -> None:
        # A validly signed attestation under a key that is not the pinned one.
        first(f)["public_key_hex"] = fixture_keypair("intruder")[0].hex()
        sign_attestation(f, first(f), "intruder")

    one_bad(self_signed_key, "public_key_hex differs")
    cases += 1

    def unsigned_change(field: str, value: Any) -> None:
        # Change a signed fact in one claim without re-signing.
        def mutate(f: dict[str, Any]) -> None:
            first(f)["validator_claims"][3][field] = value

        one_bad(mutate, "claim_digest_sha256")

    unsigned_change("access_state", "active")
    unsigned_change("grace_started_at", "2026-11-26T00:00:00Z")
    unsigned_change("removal_proposal_eligible", True)
    unsigned_change("consecutive_passing_days", 3)
    cases += 4

    # Three invented observers, each with a valid signature of its own: nothing counts.
    bad = copy.deepcopy(feed)
    for index, att in enumerate(bad["observer_attestations"]):
        att["observer_id"] = f"invented-observer-{index}"
        att["public_key_hex"] = fixture_keypair(att["observer_id"])[0].hex()
        sign_attestation(bad, att, att["observer_id"])
    result = check(bad)
    assert result["indeterminate"] and result["verified_observers"] == [] and not result["allow_credit_grants"]
    cases += 1

    # --- the served rows are covered by the signatures
    for field, value in (
        ("removal_proposal_eligible", True),
        ("grace_started_at", "2026-11-26T00:00:00Z"),
        ("consecutive_passing_days", 3),
        ("finance_hub_access", False),
        ("reward_eligible", True),
        ("source_observer_id", "observer-us-east-1"),
    ):
        bad = copy.deepcopy(feed)
        bad["validators"][3][field] = value
        expect_rejected(f"served row changed: {field}", lambda: check(bad))
        cases += 1
    bad = copy.deepcopy(feed)
    bad["validators"] = bad["validators"][:-1]
    expect_rejected("served row dropped", lambda: check(bad))
    cases += 1

    # --- a signed claim that does not follow from its own facts is one wrong observer
    def signed_nonsense(mutate_claim: Any, reason: str) -> None:
        def mutate(f: dict[str, Any]) -> None:
            mutate_claim(first(f)["validator_claims"])
            sign_attestation(f, first(f), first(f)["observer_id"])

        one_bad(mutate, reason)

    signed_nonsense(lambda c: c[4].update(finance_hub_access=True), "finance_hub_access")
    signed_nonsense(lambda c: c[4].update(research_api_keys_active=True), "research_api_keys_active")
    signed_nonsense(lambda c: c[3].update(reward_eligible=True), "reward_eligible")
    signed_nonsense(lambda c: c[0].update(mean_daily_participation_bps=10000), "mean_daily_participation_bps")
    signed_nonsense(lambda c: c[0]["days"][0].update(participation_bps=10000), "participation_bps")
    signed_nonsense(lambda c: c[0]["days"].pop(5), "days must list every window day")
    signed_nonsense(lambda c: c[3].update(grace_expires_at="2026-12-31T00:00:00Z"), "grace_expires_at")
    signed_nonsense(lambda c: c[1].update(first_active_date="2026-10-01"), "days must list every window day")
    cases += 8

    # --- schema: the fixed numbers cannot be moved
    for path, value in (
        (("window_spec", "liveness_threshold_bps"), 9400),
        (("window_spec", "grace_period_days"), 7),
        (("window_spec", "metric_version"), "latency-fair-round-window-v1"),
        (("quorum_policy", "quorum_threshold"), 1),
        (("undeclared_extra",), True),
    ):
        bad = copy.deepcopy(feed)
        target = bad
        for key in path[:-1]:
            target = target[key]
        target[path[-1]] = value
        expect_rejected(f"schema: {'.'.join(path)}", lambda: check(bad))
        cases += 1
    bad = copy.deepcopy(feed)
    first(bad)["signature_hex"] = "aa" * 100
    expect_rejected("schema: truncated signature", lambda: check(bad))
    bad = copy.deepcopy(feed)
    bad["observer_attestations"].append(copy.deepcopy(first(bad)))
    expect_rejected("schema: four attestations", lambda: check(bad))
    bad = copy.deepcopy(feed)
    bad["observer_attestations"][1] = copy.deepcopy(first(bad))
    expect_rejected("one observer twice", lambda: check(bad))
    cases += 3

    # --- time
    def reobserved(observer_index: int, when: str) -> dict[str, Any]:
        other = copy.deepcopy(feed)
        att = other["observer_attestations"][observer_index]
        att["observed_at"] = when
        sign_attestation(other, att, att["observer_id"])
        return other

    result_holder: dict[str, Any] = {}

    def served_mismatch(f: dict[str, Any], reason: str) -> None:
        f["validators"], _ = reconcile(f["observer_attestations"][1:])
        result_holder["r"] = check(f)
        assert reason in result_holder["r"]["rejected_observers"][FIXTURE_OBSERVERS[0][0]], result_holder["r"]

    served_mismatch(reobserved(0, format_utc(now + dt.timedelta(seconds=120))), "in the future")
    served_mismatch(reobserved(0, "2026-11-26T23:59:00Z"), "stale")
    cases += 2
    # An older feed cannot replace a newer one.
    accepted = check(feed)["accepted"]
    newer = {oid: format_utc(parse_utc(ts) + dt.timedelta(seconds=5)) for oid, ts in accepted.items()}
    replay = check(feed, last_accepted=newer)
    assert replay["indeterminate"] and all("older than" in r for r in replay["rejected_observers"].values())
    assert not check(feed, last_accepted=accepted)["indeterminate"]
    cases += 1
    # Another chain.
    other_keyset = copy.deepcopy(keyset)
    other_keyset["chain"] = {**keyset["chain"], "chain_id": "postfiat-other-1"}
    expect_rejected("other chain", lambda: verify_feed(feed, schema=schema, keyset=other_keyset, now_utc=now))
    cases += 1

    # --- disagreement
    # One observer disagrees on a state: the other two carry the row.
    split = copy.deepcopy(feed)
    claims = first(split)["validator_claims"]
    claims[3] = derive_row(
        **{k: claims[3][k] for k in ("validator_id", "ml_dsa_public_key", "registry_active", "first_active_date", "days")},
        access_state="revoked",
        grace_started_at=None,
        recovery_floor_date=None,
        window_end=FIXTURE_WINDOW_END,
    )
    sign_attestation(split, first(split), first(split)["observer_id"])
    split["validators"], no_quorum = reconcile(split["observer_attestations"])
    result = check(split)
    row = result["validators"][3]
    assert no_quorum == [] and row["access_state"] == "lapsed"
    assert row["agreeing_observer_ids"] == ["observer-ap-northeast-3", "observer-eu-central-2"]
    cases += 1
    # With only two observers and a disagreement, that validator has no quorum; the rest are unaffected.
    split["observer_attestations"] = split["observer_attestations"][:2]
    split["validators"], no_quorum = reconcile(split["observer_attestations"])
    result = check(split)
    assert no_quorum == ["validator-3"] == result["validators_without_quorum"] and len(result["validators"]) == 5
    cases += 1
    # Middle score of three, lower score of two.
    scores = copy.deepcopy(feed)
    for index, signed in enumerate((199, 197, 198)):
        claim = scores["observer_attestations"][index]["validator_claims"][0]
        for day in claim["days"]:
            if day["committed_blocks"]:
                day["signed_blocks"] = signed
                if day["complete"]:
                    day["participation_bps"] = signed * 10000 // day["committed_blocks"]
        scores["observer_attestations"][index]["validator_claims"][0] = derive_row(
            **{
                k: claim[k]
                for k in (
                    "validator_id",
                    "ml_dsa_public_key",
                    "registry_active",
                    "access_state",
                    "first_active_date",
                    "grace_started_at",
                    "recovery_floor_date",
                    "days",
                )
            },
            window_end=FIXTURE_WINDOW_END,
        )
    resign_all(scores)
    scores["validators"], _ = reconcile(scores["observer_attestations"])
    assert check(scores)["validators"][0]["source_observer_id"] == "observer-ap-northeast-3"
    scores["observer_attestations"] = scores["observer_attestations"][:2]
    scores["validators"], _ = reconcile(scores["observer_attestations"])
    assert check(scores)["validators"][0]["source_observer_id"] == "observer-eu-central-2"
    cases += 1
    return cases


def run_lifecycle_tests() -> int:
    end = dt.date(2026, 11, 27)
    start = end - EVALUATION_HORIZON_DAYS * DAY
    key = "ab" * 1952

    def days(first: dt.date, until: dt.date, signed: Any, idle: Any = lambda day: False) -> list[dict[str, Any]]:
        out, day, height = [], max(first, until - EVALUATION_HORIZON_DAYS * DAY), 0
        while day < until:
            committed = 0 if idle(day) else 100
            out.append(
                {
                    "date": day.isoformat(),
                    "complete": True,
                    "from_height": height + 1 if committed else None,
                    "to_height": height + committed if committed else None,
                    "committed_blocks": committed,
                    "signed_blocks": signed(day) if committed else 0,
                    "participation_bps": signed(day) * 100 if committed else None,
                }
            )
            height += committed
            day += DAY
        return out

    def step(previous: Any, registry_active: bool, day_list: list[dict[str, Any]], today: dt.date = end, at: Any = None) -> dict[str, Any]:
        row = next_row(
            previous,
            validator_id="v",
            ml_dsa_public_key=key,
            registry_active=registry_active,
            days=day_list,
            window_end=today,
            observed_at=at or midnight(today) + dt.timedelta(hours=1),
        )
        check_row(row, today - EVALUATION_HORIZON_DAYS * DAY, today)
        return row

    checks = 0
    # 1. Pending stays pending until ratified.
    row = step(None, False, [])
    assert row["access_state"] == "pending" and not row["finance_hub_access"]
    checks += 1
    # 2. Ratified: active at once, with access and credit, before any day is observed.
    row = step(row, True, [])
    assert (row["access_state"], row["access_state_reason"]) == ("active", "day0_ramp_in")
    assert row["first_active_date"] == "2026-11-27" and row["reward_eligible"] and row["finance_hub_access"]
    checks += 1
    # 3. Ramp-in falls into grace when the observed days are below 95%.
    ramp = {**row, "first_active_date": "2026-11-22"}
    row = step(ramp, True, days(dt.date(2026, 11, 22), end, lambda day: 94))
    assert (row["access_state"], row["access_state_reason"]) == ("lapsed", "below_liveness_threshold")
    assert row["grace_started_at"] == "2026-11-27T00:00:00Z" and row["grace_expires_at"] == "2026-12-11T00:00:00Z"
    assert row["finance_hub_access"] and row["research_api_keys_active"] and not row["reward_eligible"]
    checks += 1
    # 4. Steady state at the exact boundary: 25 observed days at 95%.
    steady = derive_row(
        validator_id="v", ml_dsa_public_key=key, registry_active=True, access_state="active",
        first_active_date="2026-01-01", grace_started_at=None, recovery_floor_date=None, days=[], window_end=end,
    )
    five_idle = lambda day: day < start + 5 * DAY  # noqa: E731
    row = step(steady, True, days(start, end, lambda day: 95, five_idle))
    assert (row["access_state_reason"], row["observed_days"], row["reward_eligible"]) == ("healthy_liveness", 25, True)
    checks += 1
    # 5. A quiet chain (24 days with blocks) holds the credit and keeps the access. Nobody is punished for it.
    six_idle = lambda day: day < start + 6 * DAY  # noqa: E731
    row = step(steady, True, days(start, end, lambda day: 100, six_idle))
    assert (row["access_state"], row["access_state_reason"]) == ("active", "insufficient_observed_days")
    assert row["finance_hub_access"] and not row["reward_eligible"]
    checks += 1
    # 6. A chain with no blocks at all leaves everyone as they were.
    row = step(steady, True, days(start, end, lambda day: 0, lambda day: True))
    assert row["access_state"] == "active" and row["observed_days"] == 0 and row["liveness_met"]
    checks += 1
    # 7. Steady state below 95% enters grace.
    outage = lambda day: 0 if day >= end - 6 * DAY else 100  # noqa: E731
    lapsed = step(steady, True, days(start, end, outage))
    assert lapsed["access_state"] == "lapsed" and lapsed["mean_daily_participation_bps"] == 8000
    checks += 1
    # 8. Removal from the registry enters grace too.
    row = step(steady, False, days(start, end, lambda day: 100))
    assert (row["access_state"], row["access_state_reason"]) == ("lapsed", "removed_from_registry")
    checks += 1
    # 9. Two passing days are not a recovery.
    def after(n: int) -> tuple[dt.date, list[dict[str, Any]]]:
        today = end + n * DAY
        return today, days(start, today, lambda day: 0 if end - 6 * DAY <= day < end else 100)

    today, day_list = after(2)
    row = step(lapsed, True, day_list, today)
    assert row["access_state"] == "lapsed" and row["consecutive_passing_days"] == 2
    checks += 1
    # 10. Three passing days recover at once, though the 30-day mean is still below 95%.
    today, day_list = after(3)
    recovered = step(row, True, day_list, today)
    assert (recovered["access_state"], recovered["access_state_reason"]) == ("active", "recovered_3day_streak")
    assert recovered["recovery_floor_date"] == "2026-11-27" and recovered["reward_eligible"]
    assert recovered["mean_daily_participation_bps"] == 10000 and recovered["observed_days"] == 3
    checks += 1
    # 11. The recovered validator stays active through the next days, and the floor ends after 30 days.
    today, day_list = after(10)
    row = step(recovered, True, day_list, today)
    assert row["access_state"] == "active" and row["recovery_floor_date"] == "2026-11-27"
    today = dt.date(2026, 12, 27)
    row = step(row, True, days(today - EVALUATION_HORIZON_DAYS * DAY, today, lambda day: 100), today)
    assert (row["recovery_floor_date"], row["access_state_reason"]) == (None, "healthy_liveness")
    checks += 1
    # 12. Grace runs out by the clock: one second before, still lapsed; at the instant, revoked.
    today, day_list = end + 14 * DAY, days(end - 16 * DAY, end + 14 * DAY, lambda day: 0)
    at_edge = parse_utc(lapsed["grace_expires_at"])
    row = step(lapsed, True, days(end - 17 * DAY, end + 13 * DAY, lambda day: 0), end + 13 * DAY, at_edge - dt.timedelta(seconds=1))
    assert row["access_state"] == "lapsed"
    revoked = step(row, True, day_list, today, at_edge)
    assert (revoked["access_state"], revoked["access_state_reason"]) == ("revoked", "grace_expired")
    assert not revoked["finance_hub_access"] and not revoked["research_api_keys_active"]
    assert not revoked["reward_eligible"] and revoked["removal_proposal_eligible"]
    checks += 1
    # 13. Revoked and already out of the registry: nothing left to propose.
    row = step(revoked, False, day_list, today)
    assert row["access_state_reason"] == "removed_from_registry" and not row["removal_proposal_eligible"]
    checks += 1
    # 14. A revoked validator comes back with three passing days.
    today = end + 20 * DAY
    row = step(revoked, True, days(today - EVALUATION_HORIZON_DAYS * DAY, today, lambda day: 100 if day >= today - 3 * DAY else 0), today)
    assert (row["access_state"], row["access_state_reason"]) == ("active", "recovered_3day_streak")
    checks += 1
    # 15. An idle chain during grace does not stop the clock and does not count as recovery.
    row = step(lapsed, True, days(end - 17 * DAY, end + 13 * DAY, lambda day: 0, lambda day: day >= end), end + 13 * DAY)
    assert row["access_state"] == "lapsed"
    checks += 1
    return checks


def main() -> int:
    feed_schema_json = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    keyset_schema_json = json.loads(KEYSET_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(feed_schema_json)
    Draft202012Validator.check_schema(keyset_schema_json)
    schema = Draft202012Validator(feed_schema_json)

    feed, keyset = build_fixture()
    if "--write-fixture" in sys.argv[1:]:
        FIXTURE_PATH.write_text(dump_fixture(feed), encoding="utf-8")
        KEYSET_FIXTURE_PATH.write_text(dump_fixture(keyset), encoding="utf-8")
        print(f"wrote {FIXTURE_PATH.relative_to(ROOT)} and {KEYSET_FIXTURE_PATH.relative_to(ROOT)}")
        return 0
    if FIXTURE_PATH.read_text(encoding="utf-8") != dump_fixture(feed):
        raise AssertionError("the committed feed fixture is not the one this script builds; run --write-fixture")
    if KEYSET_FIXTURE_PATH.read_text(encoding="utf-8") != dump_fixture(keyset):
        raise AssertionError("the committed keyset fixture is not the one this script builds; run --write-fixture")
    Draft202012Validator(keyset_schema_json).validate(keyset)

    feed_cases = run_feed_tests(schema, feed, keyset)
    lifecycle_checks = run_lifecycle_tests()
    print(
        f"validator_uptime_schema=ok fixture_validators={len(feed['validators'])} "
        f"observers={len(feed['observer_attestations'])} signatures=verified "
        f"feed_cases={feed_cases} lifecycle_checks={lifecycle_checks}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, ValueError, jsonschema.ValidationError, jsonschema.SchemaError) as exc:
        print(f"validator_uptime_schema=FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
