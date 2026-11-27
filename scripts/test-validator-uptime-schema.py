#!/usr/bin/env python3
"""Schema, fixture, quorum, and state-machine verifier for postfiat-validator-uptime-v1.

Validates:
  1. JSON Schema Draft 2020-12 compliance of
     docs/specs/schemas/postfiat-validator-uptime-v1.schema.json.
  2. Positive validation of the deterministic fixture
     docs/specs/schemas/postfiat-validator-uptime-v1.fixture.json.
  3. Canonical SHA-256 claim_digest_sha256 recomputation and 2-of-3 observer
     quorum reconciliation invariants (median of 3 / min of 2).
  4. Negative schema and semantic rejection cases (1 observer, bad threshold,
     revoked with active API keys, tampered digest, missing grace fields, etc.).
  5. Deterministic pending -> active -> lapsed (14d grace) -> revoked state
     machine transitions, including Day-0 ramp-in, 3-day fast recovery, and
     stale-feed non-revocation.
"""

from __future__ import annotations

import copy
import datetime as dt
import hashlib
import json
from pathlib import Path
import sys
from typing import Any

import jsonschema
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "docs/specs/schemas/postfiat-validator-uptime-v1.schema.json"
FIXTURE_PATH = ROOT / "docs/specs/schemas/postfiat-validator-uptime-v1.fixture.json"

UTC = dt.timezone.utc
BLOCKS_PER_HEIGHT_DAY = 14400
EVALUATION_HORIZON_DAYS = 30
MIN_OBSERVED_DAYS = 25
LIVENESS_THRESHOLD_BPS = 9500
GRACE_PERIOD_DAYS = 14
RECOVERY_CONSECUTIVE_DAYS = 3
STALE_AFTER_SECONDS = 3600
MAX_FUTURE_SKEW_SECONDS = 60


def parse_utc(raw: str) -> dt.datetime:
    value = dt.datetime.fromisoformat(raw.replace("Z", "+00:00"))
    if value.tzinfo is None:
        raise ValueError(f"timestamp missing UTC timezone: {raw}")
    return value.astimezone(UTC)


def format_utc(value: dt.datetime) -> str:
    return value.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def canonical_json_bytes(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def observer_claim_body(feed: dict[str, Any], attestation: dict[str, Any]) -> dict[str, Any]:
    return {
        "chain_id": feed["chain"]["chain_id"],
        "finalized_block_hash": attestation["finalized_block_hash"],
        "finalized_height": attestation["finalized_height"],
        "genesis_hash": feed["chain"]["genesis_hash"],
        "network": feed["chain"]["network"],
        "observed_at": attestation["observed_at"],
        "observer_id": attestation["observer_id"],
        "operator_group": attestation["operator_group"],
        "region": attestation["region"],
        "registry_root": attestation["registry_root"],
        "schema": feed["schema"],
        "schema_version": feed["schema_version"],
        "validator_claims": attestation["validator_claims"],
        "window_spec": feed["window_spec"],
    }


def compute_claim_digest_sha256(feed: dict[str, Any], attestation: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json_bytes(observer_claim_body(feed, attestation))).hexdigest()


def reconcile_bps(scores: list[int | None]) -> int | None:
    non_null = sorted(s for s in scores if s is not None)
    if not non_null:
        return None
    if len(non_null) == 3:
        return non_null[1]  # deterministic median of 3
    if len(non_null) == 2:
        return non_null[0]  # conservative min of 2 when 1 observer is down
    raise ValueError(f"quorum requires 2 or 3 scores, got {len(non_null)}")


def verify_feed_semantics(
    feed: dict[str, Any],
    *,
    validator: Draft202012Validator,
    now_utc: dt.datetime,
) -> dict[str, Any]:
    """Validate JSON Schema + all cryptographic-framing, freshness, and 2-of-3 quorum rules."""
    errors = sorted(validator.iter_errors(feed), key=lambda err: list(err.absolute_path))
    if errors:
        first = errors[0]
        path = ".".join(str(p) for p in first.absolute_path) or "<root>"
        raise ValueError(f"schema validation failed at {path}: {first.message}")

    chain = feed["chain"]
    window = feed["window_spec"]
    if window["window_end_height"] < window["window_start_height"]:
        raise ValueError("window_end_height must be >= window_start_height")
    if window["window_end_height"] > chain["finalized_height"]:
        raise ValueError("window_end_height cannot exceed chain.finalized_height")

    # Observer uniqueness, digest integrity, chain match, and freshness.
    seen_ids: set[str] = set()
    seen_regions: set[str] = set()
    seen_groups: set[str] = set()
    seen_keys: set[str] = set()
    fresh_observers: list[dict[str, Any]] = []
    stale_observers: list[str] = []

    for att in feed["observer_attestations"]:
        oid = att["observer_id"]
        if oid in seen_ids:
            raise ValueError(f"duplicate observer_id: {oid}")
        if att["region"] in seen_regions:
            raise ValueError(f"duplicate observer region: {att['region']}")
        if att["operator_group"] in seen_groups:
            raise ValueError(f"duplicate observer operator_group: {att['operator_group']}")
        if att["public_key_hex"] in seen_keys:
            raise ValueError(f"duplicate observer public_key_hex for {oid}")
        seen_ids.add(oid)
        seen_regions.add(att["region"])
        seen_groups.add(att["operator_group"])
        seen_keys.add(att["public_key_hex"])

        # Claims strictly sorted by validator_id with unique keys.
        claim_ids = [c["validator_id"] for c in att["validator_claims"]]
        if claim_ids != sorted(claim_ids) or len(set(claim_ids)) != len(claim_ids):
            raise ValueError(f"observer {oid} validator_claims not strictly sorted or unique")
        claim_pks = [c["ml_dsa_public_key"] for c in att["validator_claims"]]
        if len(set(claim_pks)) != len(claim_pks):
            raise ValueError(f"observer {oid} has duplicate validator public keys")

        expected_digest = compute_claim_digest_sha256(feed, att)
        if att["claim_digest_sha256"] != expected_digest:
            raise ValueError(
                f"observer {oid} claim_digest_sha256 mismatch: "
                f"expected {expected_digest}, got {att['claim_digest_sha256']}"
            )

        if (
            att["finalized_height"] != chain["finalized_height"]
            or att["finalized_block_hash"] != chain["finalized_block_hash"]
            or att["registry_root"] != chain["registry_root"]
        ):
            raise ValueError(f"observer {oid} chain identity disagrees with feed.chain")

        observed_at = parse_utc(att["observed_at"])
        skew = (observed_at - now_utc).total_seconds()
        if skew > MAX_FUTURE_SKEW_SECONDS:
            raise ValueError(f"observer {oid} timestamp exceeds max future skew ({skew:.1f}s)")
        age = (now_utc - observed_at).total_seconds()
        if age > STALE_AFTER_SECONDS:
            stale_observers.append(oid)
        else:
            fresh_observers.append(att)

    feed_stale = len(fresh_observers) < feed["quorum_policy"]["quorum_threshold"]

    # Validators strictly sorted and unique.
    val_ids = [v["validator_id"] for v in feed["validators"]]
    if val_ids != sorted(val_ids) or len(set(val_ids)) != len(val_ids):
        raise ValueError("validators array must be strictly sorted with unique validator_id")
    val_pks = [v["ml_dsa_public_key"] for v in feed["validators"]]
    if len(set(val_pks)) != len(val_pks):
        raise ValueError("validators array contains duplicate ml_dsa_public_key")

    att_by_id = {att["observer_id"]: att for att in feed["observer_attestations"]}
    bpd = window["blocks_per_height_day"]

    for v in feed["validators"]:
        vid = v["validator_id"]
        agreeing_ids = v["agreeing_observer_ids"]
        if len(agreeing_ids) < 2:
            raise ValueError(f"validator {vid} has fewer than 2 agreeing observers")
        matching_scores: list[int | None] = []
        for oid in agreeing_ids:
            if oid not in att_by_id:
                raise ValueError(f"validator {vid} references unknown observer {oid}")
            claims = {c["validator_id"]: c for c in att_by_id[oid]["validator_claims"]}
            if vid not in claims:
                raise ValueError(f"observer {oid} missing claim for validator {vid}")
            claim = claims[vid]
            for field in (
                "ml_dsa_public_key",
                "registry_active",
                "liveness_met",
                "access_state",
            ):
                if claim[field] != v[field]:
                    raise ValueError(
                        f"observer {oid} disagrees with validator {vid} on {field}: "
                        f"{claim[field]!r} != {v[field]!r}"
                    )
            matching_scores.append(claim["mean_daily_participation_bps"])

        expected_mean = reconcile_bps(matching_scores)
        if v["mean_daily_participation_bps"] != expected_mean:
            raise ValueError(
                f"validator {vid} mean_daily_participation_bps {v['mean_daily_participation_bps']} "
                f"!= quorum reconciled {expected_mean}"
            )

        # Height-day arithmetic checks.
        prev_day_index = -1
        for day in v["height_days"]:
            if day["day_index"] <= prev_day_index:
                raise ValueError(f"validator {vid} height_days not strictly ascending")
            prev_day_index = day["day_index"]
            expected_from = day["day_index"] * bpd + 1
            expected_to = (day["day_index"] + 1) * bpd
            if day["from_height"] != expected_from or day["to_height"] != expected_to:
                raise ValueError(
                    f"validator {vid} day {day['day_index']} height bounds mismatch"
                )
            if day["precommit_qc_heights"] > day["credited_heights"]:
                raise ValueError(
                    f"validator {vid} day {day['day_index']} precommit_qc_heights > credited_heights"
                )
            if day["credited_heights"] > day["eligible_heights"]:
                raise ValueError(
                    f"validator {vid} day {day['day_index']} credited_heights > eligible_heights"
                )
            if day["complete"] and day["eligible_heights"] > 0:
                expected_bps = (day["credited_heights"] * 10000) // day["eligible_heights"]
                if day["participation_bps"] != expected_bps:
                    raise ValueError(
                        f"validator {vid} day {day['day_index']} participation_bps "
                        f"{day['participation_bps']} != {expected_bps}"
                    )
            elif day["participation_bps"] is not None:
                raise ValueError(
                    f"validator {vid} day {day['day_index']} must have null participation_bps"
                )

        # Grace period arithmetic checks for lapsed state.
        if v["access_state"] == "lapsed":
            expected_grace_blocks = window["grace_period_days"] * bpd
            if v["grace_expires_height"] - v["grace_started_height"] != expected_grace_blocks:
                raise ValueError(f"validator {vid} grace height span != 14 height-days")
            g_start = parse_utc(v["grace_started_at"])
            g_end = parse_utc(v["grace_expires_at"])
            if g_end - g_start != dt.timedelta(days=window["grace_period_days"]):
                raise ValueError(f"validator {vid} grace timestamp span != 14 days")

    return {
        "valid": True,
        "feed_stale": feed_stale,
        "fresh_observers": len(fresh_observers),
        "stale_observers": stale_observers,
        "allow_new_bindings": not feed_stale,
        "allow_credit_grants": not feed_stale,
        "revoke_existing_on_stale": False,
    }


def evaluate_state_transition(
    *,
    previous_state: str,
    registry_active: bool,
    finalized_height: int,
    observed_at: dt.datetime,
    first_active_height: int | None,
    observed_height_days: int,
    mean_daily_participation_bps: int | None,
    consecutive_passing_days: int,
    grace_started_height: int | None = None,
    grace_started_at: dt.datetime | None = None,
    recovery_floor_height: int | None = None,
    feed_stale: bool = False,
    blocks_per_height_day: int = BLOCKS_PER_HEIGHT_DAY,
) -> dict[str, Any]:
    """Reference deterministic pending -> active -> lapsed (14d) -> revoked transition."""
    if feed_stale:
        # Stale feed NEVER falsely revokes existing access, but blocks new bindings/credits.
        return {
            "access_state": previous_state,
            "allow_new_bindings": False,
            "allow_credit_grants": False,
            "finance_hub_access": previous_state in {"active", "lapsed"},
            "research_api_keys_active": previous_state in {"active", "lapsed"},
            "reward_eligible": False,
            "removal_proposal_eligible": False,
            "stale_preserved": True,
        }

    if previous_state == "pending":
        if not registry_active:
            return {
                "access_state": "pending",
                "access_state_reason": "awaiting_ratification",
                "finance_hub_access": False,
                "research_api_keys_active": False,
                "reward_eligible": False,
                "removal_proposal_eligible": False,
            }
        # Day-0 ramp-in: immediately active upon registry ratification.
        return {
            "access_state": "active",
            "access_state_reason": "day0_ramp_in",
            "first_active_height": first_active_height or finalized_height,
            "ramp_in_active": True,
            "finance_hub_access": True,
            "research_api_keys_active": True,
            "reward_eligible": True,
            "removal_proposal_eligible": False,
        }

    assert first_active_height is not None
    ramp_in_active = finalized_height < first_active_height + EVALUATION_HORIZON_DAYS * blocks_per_height_day
    in_recovery_window = (
        recovery_floor_height is not None
        and finalized_height < recovery_floor_height + EVALUATION_HORIZON_DAYS * blocks_per_height_day
    )

    if previous_state == "active":
        if not registry_active:
            return _enter_lapsed("removed_from_registry", finalized_height, observed_at, blocks_per_height_day)
        if ramp_in_active or in_recovery_window:
            if observed_height_days == 0 or (
                mean_daily_participation_bps is not None
                and mean_daily_participation_bps >= LIVENESS_THRESHOLD_BPS
            ):
                reason = "day0_ramp_in" if ramp_in_active else "recovered_3day_streak"
                return {
                    "access_state": "active",
                    "access_state_reason": reason,
                    "finance_hub_access": True,
                    "research_api_keys_active": True,
                    "reward_eligible": True,
                    "removal_proposal_eligible": False,
                }
            return _enter_lapsed("below_liveness_threshold", finalized_height, observed_at, blocks_per_height_day)

        # Steady state: requires >= 25 observed days and >= 9500 bps.
        if observed_height_days < MIN_OBSERVED_DAYS:
            return _enter_lapsed("insufficient_observed_days", finalized_height, observed_at, blocks_per_height_day)
        if mean_daily_participation_bps is None or mean_daily_participation_bps < LIVENESS_THRESHOLD_BPS:
            return _enter_lapsed("below_liveness_threshold", finalized_height, observed_at, blocks_per_height_day)
        return {
            "access_state": "active",
            "access_state_reason": "healthy_liveness",
            "finance_hub_access": True,
            "research_api_keys_active": True,
            "reward_eligible": True,
            "removal_proposal_eligible": False,
        }

    if previous_state == "lapsed":
        assert grace_started_height is not None and grace_started_at is not None
        grace_expires_height = grace_started_height + GRACE_PERIOD_DAYS * blocks_per_height_day
        grace_expires_at = grace_started_at + dt.timedelta(days=GRACE_PERIOD_DAYS)

        # Fast recovery: 3 consecutive passing height-days while active in registry.
        if registry_active and consecutive_passing_days >= RECOVERY_CONSECUTIVE_DAYS:
            streak_start_height = max(1, finalized_height - RECOVERY_CONSECUTIVE_DAYS * blocks_per_height_day + 1)
            return {
                "access_state": "active",
                "access_state_reason": "recovered_3day_streak",
                "recovery_floor_height": streak_start_height,
                "finance_hub_access": True,
                "research_api_keys_active": True,
                "reward_eligible": True,
                "removal_proposal_eligible": False,
            }

        if finalized_height >= grace_expires_height:
            return {
                "access_state": "revoked",
                "access_state_reason": "grace_expired" if registry_active else "removed_from_registry",
                "finance_hub_access": False,
                "research_api_keys_active": False,
                "reward_eligible": False,
                "removal_proposal_eligible": registry_active,
            }

        return {
            "access_state": "lapsed",
            "access_state_reason": "removed_from_registry" if not registry_active else "below_liveness_threshold",
            "grace_started_height": grace_started_height,
            "grace_expires_height": grace_expires_height,
            "grace_started_at": format_utc(grace_started_at),
            "grace_expires_at": format_utc(grace_expires_at),
            "finance_hub_access": True,
            "research_api_keys_active": True,
            "reward_eligible": False,
            "removal_proposal_eligible": False,
        }

    if previous_state == "revoked":
        if registry_active and consecutive_passing_days >= RECOVERY_CONSECUTIVE_DAYS:
            streak_start_height = max(1, finalized_height - RECOVERY_CONSECUTIVE_DAYS * blocks_per_height_day + 1)
            return {
                "access_state": "active",
                "access_state_reason": "recovered_3day_streak",
                "recovery_floor_height": streak_start_height,
                "finance_hub_access": True,
                "research_api_keys_active": True,
                "reward_eligible": True,
                "removal_proposal_eligible": False,
            }
        return {
            "access_state": "revoked",
            "access_state_reason": "grace_expired" if registry_active else "removed_from_registry",
            "finance_hub_access": False,
            "research_api_keys_active": False,
            "reward_eligible": False,
            "removal_proposal_eligible": registry_active,
        }

    raise ValueError(f"unknown previous_state: {previous_state}")


def _enter_lapsed(
    reason: str,
    finalized_height: int,
    observed_at: dt.datetime,
    blocks_per_height_day: int,
) -> dict[str, Any]:
    return {
        "access_state": "lapsed",
        "access_state_reason": reason,
        "grace_started_height": finalized_height,
        "grace_expires_height": finalized_height + GRACE_PERIOD_DAYS * blocks_per_height_day,
        "grace_started_at": format_utc(observed_at),
        "grace_expires_at": format_utc(observed_at + dt.timedelta(days=GRACE_PERIOD_DAYS)),
        "finance_hub_access": True,
        "research_api_keys_active": True,
        "reward_eligible": False,
        "removal_proposal_eligible": False,
    }


def expect_failure(label: str, fn: Any) -> None:
    try:
        fn()
    except ValueError:
        return
    raise AssertionError(f"expected failure for negative test case: {label}")


def run_negative_tests(validator: Draft202012Validator, fixture: dict[str, Any], now: dt.datetime) -> int:
    cases = 0

    # 1. Fewer than 2 observers (breaks 2-of-3 quorum)
    bad = copy.deepcopy(fixture)
    bad["observer_attestations"] = bad["observer_attestations"][:1]
    expect_failure("only-1-observer", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 2. Wrong liveness threshold (9400 instead of locked 9500)
    bad = copy.deepcopy(fixture)
    bad["window_spec"]["liveness_threshold_bps"] = 9400
    expect_failure("wrong-liveness-threshold", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 3. Wrong grace period (7 days instead of locked 14)
    bad = copy.deepcopy(fixture)
    bad["window_spec"]["grace_period_days"] = 7
    expect_failure("wrong-grace-period", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 4. Revoked validator with active Research API keys
    bad = copy.deepcopy(fixture)
    revoked = next(v for v in bad["validators"] if v["access_state"] == "revoked")
    revoked["research_api_keys_active"] = True
    expect_failure("revoked-with-active-research-keys", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 5. Revoked validator with Finance Hub access still enabled
    bad = copy.deepcopy(fixture)
    revoked = next(v for v in bad["validators"] if v["access_state"] == "revoked")
    revoked["finance_hub_access"] = True
    expect_failure("revoked-with-finance-hub-access", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 6. Lapsed validator missing grace_expires_at
    bad = copy.deepcopy(fixture)
    lapsed = next(v for v in bad["validators"] if v["access_state"] == "lapsed")
    lapsed["grace_expires_at"] = None
    expect_failure("lapsed-missing-grace-expiry", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 7. Lapsed validator with reward_eligible=True
    bad = copy.deepcopy(fixture)
    lapsed = next(v for v in bad["validators"] if v["access_state"] == "lapsed")
    lapsed["reward_eligible"] = True
    expect_failure("lapsed-with-reward-eligible", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 8. Truncated ML-DSA-65 signature hex
    bad = copy.deepcopy(fixture)
    bad["observer_attestations"][0]["signature_hex"] = "aa" * 100
    expect_failure("truncated-mldsa65-signature", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 9. Tampered claim_digest_sha256
    bad = copy.deepcopy(fixture)
    bad["observer_attestations"][0]["claim_digest_sha256"] = "0" * 64
    expect_failure("tampered-claim-digest", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 10. Undeclared extra property (strict additionalProperties=false)
    bad = copy.deepcopy(fixture)
    bad["undeclared_extra"] = True
    expect_failure("undeclared-extra-property", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 11. Duplicate observer region (violates geographic independence)
    bad = copy.deepcopy(fixture)
    bad["observer_attestations"][1]["region"] = bad["observer_attestations"][0]["region"]
    bad["observer_attestations"][1]["claim_digest_sha256"] = compute_claim_digest_sha256(
        bad, bad["observer_attestations"][1]
    )
    expect_failure("duplicate-observer-region", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 12. Observer future clock skew > 60s
    bad = copy.deepcopy(fixture)
    future_ts = format_utc(now + dt.timedelta(seconds=120))
    bad["observer_attestations"][0]["observed_at"] = future_ts
    bad["observer_attestations"][0]["claim_digest_sha256"] = compute_claim_digest_sha256(
        bad, bad["observer_attestations"][0]
    )
    expect_failure("observer-future-skew", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 13. Observer disagrees on access_state listed in agreeing_observer_ids
    bad = copy.deepcopy(fixture)
    bad["observer_attestations"][0]["validator_claims"][0]["access_state"] = "lapsed"
    bad["observer_attestations"][0]["claim_digest_sha256"] = compute_claim_digest_sha256(
        bad, bad["observer_attestations"][0]
    )
    expect_failure("observer-access-state-disagreement", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    # 14. Quorum-reconciled mean_daily_participation_bps mismatch
    bad = copy.deepcopy(fixture)
    bad["validators"][0]["mean_daily_participation_bps"] = 9999
    expect_failure("quorum-bps-mismatch", lambda: verify_feed_semantics(bad, validator=validator, now_utc=now))
    cases += 1

    return cases


def run_state_machine_tests() -> int:
    t0 = dt.datetime(2026, 11, 27, 0, 0, 0, tzinfo=UTC)
    checks = 0

    # 1. Pending stays pending before ratification
    r = evaluate_state_transition(
        previous_state="pending",
        registry_active=False,
        finalized_height=1000,
        observed_at=t0,
        first_active_height=None,
        observed_height_days=0,
        mean_daily_participation_bps=None,
        consecutive_passing_days=0,
    )
    assert r["access_state"] == "pending" and not r["finance_hub_access"]
    checks += 1

    # 2. Day-0 ramp-in: immediate active on ratification (0 observed days)
    r = evaluate_state_transition(
        previous_state="pending",
        registry_active=True,
        finalized_height=1000,
        observed_at=t0,
        first_active_height=1000,
        observed_height_days=0,
        mean_daily_participation_bps=None,
        consecutive_passing_days=0,
    )
    assert r["access_state"] == "active" and r["access_state_reason"] == "day0_ramp_in"
    assert r["finance_hub_access"] and r["research_api_keys_active"] and r["reward_eligible"]
    checks += 1

    # 3. Day-0 ramp-in drops to lapsed if observed participation < 9500 bps
    r = evaluate_state_transition(
        previous_state="active",
        registry_active=True,
        finalized_height=1000 + 5 * BLOCKS_PER_HEIGHT_DAY,
        observed_at=t0,
        first_active_height=1000,
        observed_height_days=5,
        mean_daily_participation_bps=9499,
        consecutive_passing_days=0,
    )
    assert r["access_state"] == "lapsed" and r["finance_hub_access"] and not r["reward_eligible"]
    checks += 1

    # 4. Steady-state exact boundary pass (25 days, 9500 bps)
    r = evaluate_state_transition(
        previous_state="active",
        registry_active=True,
        finalized_height=500000,
        observed_at=t0,
        first_active_height=1,
        observed_height_days=25,
        mean_daily_participation_bps=9500,
        consecutive_passing_days=5,
    )
    assert r["access_state"] == "active" and r["access_state_reason"] == "healthy_liveness"
    checks += 1

    # 5. Steady-state below 25 observed days enters 14d grace
    r = evaluate_state_transition(
        previous_state="active",
        registry_active=True,
        finalized_height=500000,
        observed_at=t0,
        first_active_height=1,
        observed_height_days=24,
        mean_daily_participation_bps=9900,
        consecutive_passing_days=24,
    )
    assert r["access_state"] == "lapsed" and r["access_state_reason"] == "insufficient_observed_days"
    assert r["grace_expires_height"] - r["grace_started_height"] == 14 * BLOCKS_PER_HEIGHT_DAY
    checks += 1

    # 6. Lapsed stays lapsed on 2 consecutive passing days (needs 3)
    r = evaluate_state_transition(
        previous_state="lapsed",
        registry_active=True,
        finalized_height=500000 + 5 * BLOCKS_PER_HEIGHT_DAY,
        observed_at=t0 + dt.timedelta(days=5),
        first_active_height=1,
        observed_height_days=25,
        mean_daily_participation_bps=8200,
        consecutive_passing_days=2,
        grace_started_height=500000,
        grace_started_at=t0,
    )
    assert r["access_state"] == "lapsed" and r["finance_hub_access"] and r["research_api_keys_active"]
    checks += 1

    # 7. Fast recovery: lapsed -> active on 3 consecutive passing days even with 30d mean < 9500 bps
    r = evaluate_state_transition(
        previous_state="lapsed",
        registry_active=True,
        finalized_height=500000 + 6 * BLOCKS_PER_HEIGHT_DAY,
        observed_at=t0 + dt.timedelta(days=6),
        first_active_height=1,
        observed_height_days=27,
        mean_daily_participation_bps=7780,
        consecutive_passing_days=3,
        grace_started_height=500000,
        grace_started_at=t0,
    )
    assert r["access_state"] == "active" and r["access_state_reason"] == "recovered_3day_streak"
    assert r["reward_eligible"] and r["recovery_floor_height"] is not None
    checks += 1

    # 8. Grace expiry: lapsed -> revoked disables Finance Hub, invalidates API keys, enables removal proposal
    r = evaluate_state_transition(
        previous_state="lapsed",
        registry_active=True,
        finalized_height=500000 + 14 * BLOCKS_PER_HEIGHT_DAY,
        observed_at=t0 + dt.timedelta(days=14),
        first_active_height=1,
        observed_height_days=25,
        mean_daily_participation_bps=7000,
        consecutive_passing_days=0,
        grace_started_height=500000,
        grace_started_at=t0,
    )
    assert r["access_state"] == "revoked" and r["access_state_reason"] == "grace_expired"
    assert not r["finance_hub_access"]
    assert not r["research_api_keys_active"]
    assert not r["reward_eligible"]
    assert r["removal_proposal_eligible"]
    checks += 1

    # 9. Stale feed (> 1h) blocks new bindings/credits but NEVER revokes active access
    r = evaluate_state_transition(
        previous_state="active",
        registry_active=False,
        finalized_height=600000,
        observed_at=t0,
        first_active_height=1,
        observed_height_days=0,
        mean_daily_participation_bps=0,
        consecutive_passing_days=0,
        feed_stale=True,
    )
    assert r["access_state"] == "active"
    assert r["finance_hub_access"] and r["research_api_keys_active"]
    assert not r["allow_new_bindings"] and not r["allow_credit_grants"] and not r["reward_eligible"]
    checks += 1

    return checks


def main() -> int:
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)

    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    now = parse_utc(fixture["generated_at"]) + dt.timedelta(seconds=30)

    # Positive check (fresh feed)
    fresh_result = verify_feed_semantics(fixture, validator=validator, now_utc=now)
    assert fresh_result["valid"] is True
    assert fresh_result["feed_stale"] is False
    assert fresh_result["fresh_observers"] == 3
    assert fresh_result["allow_new_bindings"] is True
    assert fresh_result["allow_credit_grants"] is True

    # Stale feed check (> 3600s later): valid schema, marked stale, blocks new grants, never revokes
    stale_result = verify_feed_semantics(
        fixture, validator=validator, now_utc=now + dt.timedelta(seconds=3700)
    )
    assert stale_result["valid"] is True
    assert stale_result["feed_stale"] is True
    assert stale_result["allow_new_bindings"] is False
    assert stale_result["allow_credit_grants"] is False
    assert stale_result["revoke_existing_on_stale"] is False

    # 2-of-3 quorum degradation check (1 observer offline, 2 remaining)
    two_of_three = copy.deepcopy(fixture)
    two_of_three["observer_attestations"] = two_of_three["observer_attestations"][:2]
    for v in two_of_three["validators"]:
        v["agreeing_observer_ids"] = [
            oid for oid in v["agreeing_observer_ids"] if oid != "observer-ap-northeast-3"
        ]
        # Recompute 2-observer conservative minimum score
        scores = []
        for att in two_of_three["observer_attestations"]:
            claim = next(c for c in att["validator_claims"] if c["validator_id"] == v["validator_id"])
            scores.append(claim["mean_daily_participation_bps"])
        v["mean_daily_participation_bps"] = reconcile_bps(scores)
    degraded_result = verify_feed_semantics(two_of_three, validator=validator, now_utc=now)
    assert degraded_result["valid"] is True and degraded_result["fresh_observers"] == 2

    neg_count = run_negative_tests(validator, fixture, now)
    sm_count = run_state_machine_tests()

    print(
        f"validator_uptime_schema=ok fixture_validators={len(fixture['validators'])} "
        f"observers={len(fixture['observer_attestations'])} "
        f"negative_cases={neg_count} state_machine_checks={sm_count}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, ValueError, jsonschema.SchemaError) as exc:
        print(f"validator_uptime_schema=FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
