#!/usr/bin/env python3
"""Multi-observer signed validator uptime collector & verifier (`postfiat-validator-uptime-v1`).

Implements frozen Phase 0 Contract C2:
  - Latency-fair round-window participation metric (`prepare_vote`, `precommit_vote`,
    `timeout_vote`, `consensus_heartbeat`) over deterministic block-height windows
    (zero wall-clock drift; exact integer basis points 0..=10000; 9500 bps threshold).
  - Access lifecycle state machine: `pending -> active -> lapsed (14d grace) -> revoked`
    with Day-0 ramp-in, 3-consecutive-height-day fast recovery, stale-feed (>3600s)
    non-revocation invariant, and Research API key invalidation on `revoked`.
  - 3-observer (2-of-3 quorum) pure FIPS 204 ML-DSA-65 signing and verification under
    context `postfiat-l1-v2/validator-uptime-feed/v1` (reconciling median of 3 / min of 2).
  - Strictly read-only towards validator RPC endpoints and refuses writing inside
    any validator data directory.

Subcommands:
  collect-observer   Query read-only RPC(s) (or offline input), compute deterministic
                     height-day observations + state transitions, and emit a signed
                     observer attestation JSON using `postfiat-node observer-uptime-sign`.
  reconcile          Combine 2 or 3 signed observer attestations into a canonical
                     `postfiat-validator-uptime-v1` quorum feed and verify it.
  verify             Verify a `postfiat-validator-uptime-v1` feed against a pinned
                     3-observer `postfiat.observer_keyset.v1` trust store (both
                     cryptographic ML-DSA-65 via `postfiat-node observer-uptime-verify`
                     and Contract C2 schema/semantic/freshness rules).
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
from typing import Any

UTC = dt.timezone.utc
RPC_VERSION = "postfiat-local-rpc-v1"
FEED_SCHEMA = "postfiat-validator-uptime-v1"
FEED_SCHEMA_VERSION = 1
SIGNING_CONTEXT = "postfiat-l1-v2/validator-uptime-feed/v1"
METRIC_VERSION = "latency-fair-round-window-v1"
CREDIT_SOURCES = [
    "prepare_vote",
    "precommit_vote",
    "timeout_vote",
    "consensus_heartbeat",
]
EVALUATION_HORIZON_DAYS = 30
MIN_OBSERVED_DAYS = 25
LIVENESS_THRESHOLD_BPS = 9500
GRACE_PERIOD_DAYS = 14
RECOVERY_CONSECUTIVE_DAYS = 3
STALE_AFTER_SECONDS = 3600
MAX_FUTURE_SKEW_SECONDS = 60
STALE_BEHAVIOR = "block_new_bindings_and_credits_preserve_existing_access"


def format_utc(value: dt.datetime) -> str:
    return value.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_utc(raw: str) -> dt.datetime:
    parsed = dt.datetime.fromisoformat(raw.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError(f"UTC timestamp required: {raw}")
    return parsed.astimezone(UTC)


def canonical_json_bytes(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def observer_claim_body(feed_chain: dict[str, Any], window_spec: dict[str, Any], att: dict[str, Any]) -> dict[str, Any]:
    return {
        "chain_id": feed_chain["chain_id"],
        "finalized_block_hash": att["finalized_block_hash"],
        "finalized_height": att["finalized_height"],
        "genesis_hash": feed_chain["genesis_hash"],
        "network": feed_chain["network"],
        "observed_at": att["observed_at"],
        "observer_id": att["observer_id"],
        "operator_group": att["operator_group"],
        "region": att["region"],
        "registry_root": att["registry_root"],
        "schema": FEED_SCHEMA,
        "schema_version": FEED_SCHEMA_VERSION,
        "validator_claims": att["validator_claims"],
        "window_spec": window_spec,
    }


def compute_claim_digest_sha256(
    feed_chain: dict[str, Any], window_spec: dict[str, Any], att: dict[str, Any]
) -> str:
    return hashlib.sha256(
        canonical_json_bytes(observer_claim_body(feed_chain, window_spec, att))
    ).hexdigest()


def reconcile_bps(scores: list[int | None]) -> int | None:
    non_null = sorted(s for s in scores if s is not None)
    if not non_null:
        return None
    if len(non_null) == 3:
        return non_null[1]  # deterministic median of 3
    if len(non_null) == 2:
        return non_null[0]  # conservative min of 2 when 1 observer is down
    raise ValueError(f"2-of-3 quorum requires 2 or 3 scores, got {len(non_null)}")


def refuse_validator_data_dir(path: Path) -> None:
    resolved = path.resolve()
    for directory in (resolved, *resolved.parents):
        if (directory / "node_state.json").exists() or (directory / "guard.state").exists():
            raise ValueError(
                f"refusing to write inside validator data or signer-guard directory: {path}"
            )


def atomic_write_json(path: Path, payload: Any) -> None:
    refuse_validator_data_dir(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(tmp_name, 0o644)
        os.replace(tmp_name, path)
    finally:
        if os.path.exists(tmp_name):
            os.unlink(tmp_name)


def read_only_rpc(host: str, port: int, method: str, params: dict[str, Any] | None = None, timeout: float = 15.0) -> Any:
    if method not in {"status", "validators", "blocks", "metrics"}:
        raise ValueError(f"non-read RPC method refused: {method}")
    req_id = f"uptime-observer-{method}"
    payload = {
        "version": RPC_VERSION,
        "id": req_id,
        "method": method,
        "params": params or {},
    }
    with socket.create_connection((host, port), timeout) as stream:
        stream.settimeout(timeout)
        stream.sendall((json.dumps(payload) + "\n").encode("utf-8"))
        raw = stream.makefile("rb").readline(8 * 1024 * 1024 + 1)
    if len(raw) > 8 * 1024 * 1024 or not raw.endswith(b"\n"):
        raise ValueError(f"RPC `{method}` frame truncated or oversized")
    resp = json.loads(raw.decode("utf-8"))
    if resp.get("version") != RPC_VERSION or resp.get("id") != req_id or resp.get("ok") is not True:
        raise ValueError(f"RPC `{method}` failed or protocol mismatch: {resp.get('error')}")
    return resp["result"]


def compute_height_day_observations(
    *,
    eligible_heights_by_day: dict[int, int],
    credited_heights_by_day: dict[int, int],
    precommit_qc_heights_by_day: dict[int, int],
    complete_days: set[int],
    blocks_per_height_day: int,
    finalized_height: int,
    recovery_floor_height: int | None = None,
) -> tuple[list[dict[str, Any]], int, int | None, int]:
    """Compute trailing height-day rows, observed_height_days, mean bps, and streak."""
    latest_complete_day = (finalized_height // blocks_per_height_day) - 1
    if latest_complete_day < 0:
        return [], 0, None, 0

    first_day = max(0, latest_complete_day - EVALUATION_HORIZON_DAYS + 1)
    rows: list[dict[str, Any]] = []
    scored_days: list[int] = []

    for day_index in range(first_day, latest_complete_day + 1):
        from_h = day_index * blocks_per_height_day + 1
        to_h = (day_index + 1) * blocks_per_height_day
        if recovery_floor_height is not None and from_h < recovery_floor_height:
            continue
        eligible = eligible_heights_by_day.get(day_index, 0)
        credited = min(credited_heights_by_day.get(day_index, 0), eligible)
        qc_heights = min(precommit_qc_heights_by_day.get(day_index, 0), credited)
        complete = day_index in complete_days
        if complete and eligible > 0:
            bps: int | None = (credited * 10_000) // eligible
            scored_days.append(bps)
        else:
            bps = None
        rows.append(
            {
                "day_index": day_index,
                "from_height": from_h,
                "to_height": to_h,
                "complete": complete,
                "eligible_heights": eligible,
                "credited_heights": credited,
                "precommit_qc_heights": qc_heights,
                "participation_bps": bps,
            }
        )

    observed_days = len(scored_days)
    mean_bps = (sum(scored_days) // observed_days) if observed_days > 0 else None

    streak = 0
    for row in reversed(rows):
        if row["complete"] and row["eligible_heights"] > 0:
            if row["participation_bps"] is not None and row["participation_bps"] >= LIVENESS_THRESHOLD_BPS:
                streak += 1
            else:
                break
        else:
            break

    return rows, observed_days, mean_bps, streak


def evaluate_validator_lifecycle(
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
    blocks_per_height_day: int = 14400,
) -> dict[str, Any]:
    """Deterministic Contract C2 pending -> active -> lapsed (14d) -> revoked transition."""
    if previous_state == "pending":
        if not registry_active:
            return {
                "first_active_height": None,
                "ramp_in_active": False,
                "liveness_met": False,
                "access_state": "pending",
                "access_state_reason": "awaiting_ratification",
                "grace_started_at": None,
                "grace_expires_at": None,
                "grace_started_height": None,
                "grace_expires_height": None,
                "recovery_floor_height": None,
                "removal_proposal_eligible": False,
                "reward_eligible": False,
                "finance_hub_access": False,
                "research_api_keys_active": False,
            }
        first_h = first_active_height or finalized_height
        return {
            "first_active_height": first_h,
            "ramp_in_active": True,
            "liveness_met": True,
            "access_state": "active",
            "access_state_reason": "day0_ramp_in",
            "grace_started_at": None,
            "grace_expires_at": None,
            "grace_started_height": None,
            "grace_expires_height": None,
            "recovery_floor_height": None,
            "removal_proposal_eligible": False,
            "reward_eligible": True,
            "finance_hub_access": True,
            "research_api_keys_active": True,
        }

    first_h = first_active_height or 1
    ramp_in_active = finalized_height < first_h + EVALUATION_HORIZON_DAYS * blocks_per_height_day
    in_recovery = (
        recovery_floor_height is not None
        and finalized_height < recovery_floor_height + EVALUATION_HORIZON_DAYS * blocks_per_height_day
    )

    def enter_lapsed(reason: str) -> dict[str, Any]:
        return {
            "first_active_height": first_h,
            "ramp_in_active": ramp_in_active,
            "liveness_met": False,
            "access_state": "lapsed",
            "access_state_reason": reason,
            "grace_started_at": format_utc(observed_at),
            "grace_expires_at": format_utc(observed_at + dt.timedelta(days=GRACE_PERIOD_DAYS)),
            "grace_started_height": finalized_height,
            "grace_expires_height": finalized_height + GRACE_PERIOD_DAYS * blocks_per_height_day,
            "recovery_floor_height": recovery_floor_height,
            "removal_proposal_eligible": False,
            "reward_eligible": False,
            "finance_hub_access": True,
            "research_api_keys_active": True,
        }

    if previous_state == "active":
        if not registry_active:
            return enter_lapsed("removed_from_registry")
        if ramp_in_active or in_recovery:
            if observed_height_days == 0 or (
                mean_daily_participation_bps is not None
                and mean_daily_participation_bps >= LIVENESS_THRESHOLD_BPS
            ):
                return {
                    "first_active_height": first_h,
                    "ramp_in_active": ramp_in_active,
                    "liveness_met": True,
                    "access_state": "active",
                    "access_state_reason": "day0_ramp_in" if ramp_in_active else "recovered_3day_streak",
                    "grace_started_at": None,
                    "grace_expires_at": None,
                    "grace_started_height": None,
                    "grace_expires_height": None,
                    "recovery_floor_height": recovery_floor_height if in_recovery else None,
                    "removal_proposal_eligible": False,
                    "reward_eligible": True,
                    "finance_hub_access": True,
                    "research_api_keys_active": True,
                }
            return enter_lapsed("below_liveness_threshold")
        if observed_height_days < MIN_OBSERVED_DAYS:
            return enter_lapsed("insufficient_observed_days")
        if mean_daily_participation_bps is None or mean_daily_participation_bps < LIVENESS_THRESHOLD_BPS:
            return enter_lapsed("below_liveness_threshold")
        return {
            "first_active_height": first_h,
            "ramp_in_active": False,
            "liveness_met": True,
            "access_state": "active",
            "access_state_reason": "healthy_liveness",
            "grace_started_at": None,
            "grace_expires_at": None,
            "grace_started_height": None,
            "grace_expires_height": None,
            "recovery_floor_height": None,
            "removal_proposal_eligible": False,
            "reward_eligible": True,
            "finance_hub_access": True,
            "research_api_keys_active": True,
        }

    if previous_state == "lapsed":
        g_start_h = grace_started_height or finalized_height
        g_start_at = grace_started_at or observed_at
        g_exp_h = g_start_h + GRACE_PERIOD_DAYS * blocks_per_height_day
        g_exp_at = g_start_at + dt.timedelta(days=GRACE_PERIOD_DAYS)
        if registry_active and consecutive_passing_days >= RECOVERY_CONSECUTIVE_DAYS:
            streak_floor = max(
                1, finalized_height - RECOVERY_CONSECUTIVE_DAYS * blocks_per_height_day + 1
            )
            return {
                "first_active_height": first_h,
                "ramp_in_active": ramp_in_active,
                "liveness_met": True,
                "access_state": "active",
                "access_state_reason": "recovered_3day_streak",
                "grace_started_at": None,
                "grace_expires_at": None,
                "grace_started_height": None,
                "grace_expires_height": None,
                "recovery_floor_height": streak_floor,
                "removal_proposal_eligible": False,
                "reward_eligible": True,
                "finance_hub_access": True,
                "research_api_keys_active": True,
            }
        if finalized_height >= g_exp_h:
            return {
                "first_active_height": first_h,
                "ramp_in_active": ramp_in_active,
                "liveness_met": False,
                "access_state": "revoked",
                "access_state_reason": "grace_expired" if registry_active else "removed_from_registry",
                "grace_started_at": None,
                "grace_expires_at": None,
                "grace_started_height": None,
                "grace_expires_height": None,
                "recovery_floor_height": None,
                "removal_proposal_eligible": registry_active,
                "reward_eligible": False,
                "finance_hub_access": False,
                "research_api_keys_active": False,
            }
        return {
            "first_active_height": first_h,
            "ramp_in_active": ramp_in_active,
            "liveness_met": False,
            "access_state": "lapsed",
            "access_state_reason": "removed_from_registry" if not registry_active else "below_liveness_threshold",
            "grace_started_at": format_utc(g_start_at),
            "grace_expires_at": format_utc(g_exp_at),
            "grace_started_height": g_start_h,
            "grace_expires_height": g_exp_h,
            "recovery_floor_height": recovery_floor_height,
            "removal_proposal_eligible": False,
            "reward_eligible": False,
            "finance_hub_access": True,
            "research_api_keys_active": True,
        }

    if previous_state == "revoked":
        if registry_active and consecutive_passing_days >= RECOVERY_CONSECUTIVE_DAYS:
            streak_floor = max(
                1, finalized_height - RECOVERY_CONSECUTIVE_DAYS * blocks_per_height_day + 1
            )
            return {
                "first_active_height": first_h,
                "ramp_in_active": ramp_in_active,
                "liveness_met": True,
                "access_state": "active",
                "access_state_reason": "recovered_3day_streak",
                "grace_started_at": None,
                "grace_expires_at": None,
                "grace_started_height": None,
                "grace_expires_height": None,
                "recovery_floor_height": streak_floor,
                "removal_proposal_eligible": False,
                "reward_eligible": True,
                "finance_hub_access": True,
                "research_api_keys_active": True,
            }
        return {
            "first_active_height": first_h,
            "ramp_in_active": ramp_in_active,
            "liveness_met": False,
            "access_state": "revoked",
            "access_state_reason": "grace_expired" if registry_active else "removed_from_registry",
            "grace_started_at": None,
            "grace_expires_at": None,
            "grace_started_height": None,
            "grace_expires_height": None,
            "recovery_floor_height": None,
            "removal_proposal_eligible": registry_active,
            "reward_eligible": False,
            "finance_hub_access": False,
            "research_api_keys_active": False,
        }

    raise ValueError(f"unknown previous_state: {previous_state}")


def sign_attestation_via_node(
    node_bin: str,
    key_file: Path,
    chain: dict[str, Any],
    window_spec: dict[str, Any],
    observed_at: str,
    validator_claims: list[dict[str, Any]],
) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as tmp:
        json.dump(
            {
                "chain": chain,
                "window_spec": window_spec,
                "observed_at": observed_at,
                "validator_claims": validator_claims,
            },
            tmp,
        )
        tmp_path = tmp.name
    try:
        proc = subprocess.run(
            [
                node_bin,
                "observer-uptime-sign",
                "--key-file",
                str(key_file),
                "--input-file",
                tmp_path,
            ],
            check=True,
            text=True,
            capture_output=True,
        )
        return json.loads(proc.stdout)
    finally:
        if os.path.exists(tmp_path):
            os.unlink(tmp_path)


def verify_feed_via_node(
    node_bin: str, feed_file: Path, keyset_file: Path, now_unix: int | None = None
) -> dict[str, Any]:
    cmd = [
        node_bin,
        "observer-uptime-verify",
        "--feed-file",
        str(feed_file),
        "--keyset-file",
        str(keyset_file),
    ]
    if now_unix is not None:
        cmd.extend(["--now-unix", str(now_unix)])
    proc = subprocess.run(cmd, check=False, text=True, capture_output=True)
    if proc.returncode != 0:
        raise ValueError(f"ML-DSA-65 verification failed: {proc.stderr.strip() or proc.stdout.strip()}")
    return json.loads(proc.stdout)


def reconcile_observer_bundles(
    bundles: list[dict[str, Any]],
    *,
    generated_at: str | None = None,
) -> dict[str, Any]:
    """Reconcile 2 or 3 signed observer bundles into a canonical postfiat-validator-uptime-v1 feed."""
    if not (2 <= len(bundles) <= 3):
        raise ValueError(f"2-of-3 quorum reconciliation requires 2 or 3 observer bundles, got {len(bundles)}")

    chain = bundles[0]["chain"]
    window_spec = bundles[0]["window_spec"]
    for idx, bundle in enumerate(bundles[1:], start=1):
        if bundle["chain"] != chain:
            raise ValueError(f"observer bundle #{idx} chain identity disagrees with bundle #0")
        if bundle["window_spec"] != window_spec:
            raise ValueError(f"observer bundle #{idx} window_spec disagrees with bundle #0")

    attestations = [b["attestation"] for b in bundles]
    # Collect validator rows across observers and enforce 2-of-3 agreement.
    rows_by_vid: dict[str, list[tuple[str, dict[str, Any], dict[str, Any]]]] = {}
    for bundle in bundles:
        oid = bundle["attestation"]["observer_id"]
        claim_by_id = {c["validator_id"]: c for c in bundle["attestation"]["validator_claims"]}
        for row in bundle["validators"]:
            vid = row["validator_id"]
            if vid not in claim_by_id:
                raise ValueError(f"observer {oid} missing claim for validator {vid}")
            rows_by_vid.setdefault(vid, []).append((oid, row, claim_by_id[vid]))

    reconciled_validators: list[dict[str, Any]] = []
    for vid in sorted(rows_by_vid):
        entries = rows_by_vid[vid]
        # Group agreeing observers by (ml_dsa_public_key, registry_active, liveness_met, access_state)
        buckets: dict[tuple[Any, ...], list[tuple[str, dict[str, Any], dict[str, Any]]]] = {}
        for item in entries:
            _, _, claim = item
            sig = (
                claim["ml_dsa_public_key"],
                claim["registry_active"],
                claim["liveness_met"],
                claim["access_state"],
            )
            buckets.setdefault(sig, []).append(item)
        winner = max(buckets.values(), key=len)
        if len(winner) < 2:
            raise ValueError(f"validator {vid} lacks 2-of-3 observer agreement on access_state")

        agreeing_ids = [oid for oid, _, _ in winner]
        scores = [claim["mean_daily_participation_bps"] for _, _, claim in winner]
        reconciled_mean = reconcile_bps(scores)

        # Pick deterministic template row from the first agreeing observer (sorted by observer_id).
        base_row = dict(winner[0][1])
        base_row["agreeing_observer_ids"] = agreeing_ids
        base_row["mean_daily_participation_bps"] = reconciled_mean
        reconciled_validators.append(base_row)

    feed = {
        "schema": FEED_SCHEMA,
        "schema_version": FEED_SCHEMA_VERSION,
        "generated_at": generated_at or format_utc(dt.datetime.now(UTC)),
        "chain": chain,
        "quorum_policy": {
            "observer_count": 3,
            "quorum_threshold": 2,
            "algorithm_id": "ML-DSA-65",
            "signing_context": SIGNING_CONTEXT,
            "stale_after_seconds": STALE_AFTER_SECONDS,
            "max_future_skew_seconds": MAX_FUTURE_SKEW_SECONDS,
            "stale_behavior": STALE_BEHAVIOR,
        },
        "window_spec": window_spec,
        "validators": reconciled_validators,
        "observer_attestations": attestations,
    }
    return feed


def build_observer_bundle(
    *,
    chain: dict[str, Any],
    blocks_per_height_day: int,
    validator_inputs: list[dict[str, Any]],
    observed_at: dt.datetime,
    key_file: Path,
    node_bin: str,
) -> dict[str, Any]:
    """Build and sign a single observer's uptime bundle from observed validator telemetry."""
    finalized_height = int(chain["finalized_height"])
    latest_complete_day = (finalized_height // blocks_per_height_day) - 1
    window_start_height = (
        max(0, latest_complete_day - EVALUATION_HORIZON_DAYS + 1) * blocks_per_height_day + 1
        if latest_complete_day >= 0
        else 1
    )
    window_end_height = (
        (latest_complete_day + 1) * blocks_per_height_day
        if latest_complete_day >= 0
        else finalized_height
    )
    window_spec = {
        "metric_version": METRIC_VERSION,
        "credit_sources": list(CREDIT_SOURCES),
        "blocks_per_height_day": blocks_per_height_day,
        "evaluation_horizon_days": EVALUATION_HORIZON_DAYS,
        "min_observed_days": MIN_OBSERVED_DAYS,
        "liveness_threshold_bps": LIVENESS_THRESHOLD_BPS,
        "grace_period_days": GRACE_PERIOD_DAYS,
        "recovery_consecutive_days": RECOVERY_CONSECUTIVE_DAYS,
        "window_start_height": window_start_height,
        "window_end_height": window_end_height,
    }

    validators_out: list[dict[str, Any]] = []
    claims_out: list[dict[str, Any]] = []

    for item in sorted(validator_inputs, key=lambda v: v["validator_id"]):
        vid = item["validator_id"]
        pk = item["ml_dsa_public_key"]
        if not re.fullmatch(r"[0-9a-f]{3904}", pk):
            raise ValueError(f"validator {vid} has invalid ML-DSA-65 public key hex")

        eligible_map = {int(k): int(v) for k, v in item.get("eligible_heights_by_day", {}).items()}
        credited_map = {int(k): int(v) for k, v in item.get("credited_heights_by_day", {}).items()}
        qc_map = {int(k): int(v) for k, v in item.get("precommit_qc_heights_by_day", {}).items()}
        complete_days = {int(d) for d in item.get("complete_days", eligible_map.keys())}

        height_days, obs_days, mean_bps, streak = compute_height_day_observations(
            eligible_heights_by_day=eligible_map,
            credited_heights_by_day=credited_map,
            precommit_qc_heights_by_day=qc_map,
            complete_days=complete_days,
            blocks_per_height_day=blocks_per_height_day,
            finalized_height=finalized_height,
            recovery_floor_height=item.get("recovery_floor_height"),
        )

        lifecycle = evaluate_validator_lifecycle(
            previous_state=item.get("previous_state", "pending"),
            registry_active=bool(item["registry_active"]),
            finalized_height=finalized_height,
            observed_at=observed_at,
            first_active_height=item.get("first_active_height"),
            observed_height_days=obs_days,
            mean_daily_participation_bps=mean_bps,
            consecutive_passing_days=streak,
            grace_started_height=item.get("grace_started_height"),
            grace_started_at=(
                parse_utc(item["grace_started_at"]) if item.get("grace_started_at") else None
            ),
            recovery_floor_height=item.get("recovery_floor_height"),
            blocks_per_height_day=blocks_per_height_day,
        )

        validators_out.append(
            {
                "validator_id": vid,
                "ml_dsa_public_key": pk,
                "operator_identity_id": item.get("operator_identity_id"),
                "public_rpc_url": item.get("public_rpc_url"),
                "registry_active": bool(item["registry_active"]),
                "first_active_height": lifecycle["first_active_height"],
                "ramp_in_active": lifecycle["ramp_in_active"],
                "observed_height_days": obs_days,
                "mean_daily_participation_bps": mean_bps,
                "consecutive_passing_days": streak,
                "liveness_met": lifecycle["liveness_met"],
                "access_state": lifecycle["access_state"],
                "access_state_reason": lifecycle["access_state_reason"],
                "grace_started_at": lifecycle["grace_started_at"],
                "grace_expires_at": lifecycle["grace_expires_at"],
                "grace_started_height": lifecycle["grace_started_height"],
                "grace_expires_height": lifecycle["grace_expires_height"],
                "recovery_floor_height": lifecycle["recovery_floor_height"],
                "removal_proposal_eligible": lifecycle["removal_proposal_eligible"],
                "reward_eligible": lifecycle["reward_eligible"],
                "finance_hub_access": lifecycle["finance_hub_access"],
                "research_api_keys_active": lifecycle["research_api_keys_active"],
                "agreeing_observer_ids": [],
                "height_days": height_days,
            }
        )
        claims_out.append(
            {
                "validator_id": vid,
                "ml_dsa_public_key": pk,
                "registry_active": bool(item["registry_active"]),
                "observed_height_days": obs_days,
                "mean_daily_participation_bps": mean_bps,
                "consecutive_passing_days": streak,
                "liveness_met": lifecycle["liveness_met"],
                "access_state": lifecycle["access_state"],
            }
        )

    observed_at_str = format_utc(observed_at)
    attestation = sign_attestation_via_node(
        node_bin=node_bin,
        key_file=key_file,
        chain=chain,
        window_spec=window_spec,
        observed_at=observed_at_str,
        validator_claims=claims_out,
    )
    return {
        "schema": "postfiat-validator-uptime-observer-bundle-v1",
        "chain": chain,
        "window_spec": window_spec,
        "validators": validators_out,
        "attestation": attestation,
    }


def collect_live_rpc_observation(
    *,
    rpc_host: str,
    rpc_port: int,
    network: str,
    blocks_per_height_day: int,
    max_blocks_scan: int,
    state: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any]]:
    """Query a read-only RPC node for chain tip, validator registry, and recent block votes."""
    status_before = read_only_rpc(rpc_host, rpc_port, "status")
    validators_resp = read_only_rpc(rpc_host, rpc_port, "validators")
    status_after = read_only_rpc(rpc_host, rpc_port, "status")
    for field in ("chain_id", "genesis_hash", "block_height", "block_tip_hash"):
        if status_before.get(field) != status_after.get(field):
            raise ValueError(f"chain moved during read snapshot on `{field}`")

    height = int(status_before["block_height"])
    chain = {
        "network": network,
        "chain_id": status_before["chain_id"],
        "genesis_hash": status_before["genesis_hash"],
        "committee_epoch": int(state.get("committee_epoch", 0)),
        "registry_root": validators_resp["registry_root"],
        "finalized_height": max(1, height),
        "finalized_block_hash": status_before["block_tip_hash"],
    }

    records = validators_resp["validators"]
    day_credit: dict[str, dict[int, int]] = state.get("day_credit", {})
    day_qc: dict[str, dict[int, int]] = state.get("day_qc", {})
    day_eligible: dict[str, dict[int, int]] = state.get("day_eligible", {})
    last_scanned = int(state.get("last_scanned_height", 0))
    start_h = max(1, last_scanned + 1)
    if height - start_h + 1 > max_blocks_scan:
        start_h = max(1, height - max_blocks_scan + 1)

    if height >= start_h:
        for batch_start in range(start_h, height + 1, 16):
            limit = min(16, height + 1 - batch_start)
            batch = read_only_rpc(
                rpc_host,
                rpc_port,
                "blocks",
                {"from_height": batch_start, "limit": limit},
            )
            for block in batch:
                header = block.get("header", {})
                bh = int(header.get("height", 0))
                if bh < 1:
                    continue
                day_idx = (bh - 1) // blocks_per_height_day
                qc = header.get("consensus_v2_commit", {}).get("precommit_qc") or {}
                qc_signers = {
                    v.get("validator") for v in qc.get("votes", []) if isinstance(v, dict)
                }
                for rec in records:
                    vid = rec["node_id"]
                    day_eligible.setdefault(vid, {})
                    day_eligible[vid][day_idx] = day_eligible[vid].get(day_idx, 0) + 1
                    if vid in qc_signers:
                        day_credit.setdefault(vid, {})
                        day_credit[vid][day_idx] = day_credit[vid].get(day_idx, 0) + 1
                        day_qc.setdefault(vid, {})
                        day_qc[vid][day_idx] = day_qc[vid].get(day_idx, 0) + 1

    prev_meta = state.get("validator_state", {})
    validator_inputs: list[dict[str, Any]] = []
    for rec in records:
        vid = rec["node_id"]
        meta = prev_meta.get(vid, {})
        validator_inputs.append(
            {
                "validator_id": vid,
                "ml_dsa_public_key": rec["public_key_hex"],
                "operator_identity_id": meta.get("operator_identity_id"),
                "public_rpc_url": meta.get("public_rpc_url"),
                "registry_active": True,
                "previous_state": meta.get("access_state", "pending"),
                "first_active_height": meta.get("first_active_height", max(1, height)),
                "grace_started_height": meta.get("grace_started_height"),
                "grace_started_at": meta.get("grace_started_at"),
                "recovery_floor_height": meta.get("recovery_floor_height"),
                "eligible_heights_by_day": day_eligible.get(vid, {}),
                "credited_heights_by_day": day_credit.get(vid, {}),
                "precommit_qc_heights_by_day": day_qc.get(vid, {}),
                "complete_days": [
                    d
                    for d, cnt in day_eligible.get(vid, {}).items()
                    if cnt >= blocks_per_height_day
                ],
            }
        )

    new_state = {
        "last_scanned_height": height,
        "committee_epoch": chain["committee_epoch"],
        "day_credit": day_credit,
        "day_qc": day_qc,
        "day_eligible": day_eligible,
        "validator_state": prev_meta,
    }
    return chain, validator_inputs, new_state


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    p_collect = sub.add_parser("collect-observer", help="Collect and sign a single observer bundle")
    p_collect.add_argument("--node-bin", default="postfiat-node")
    p_collect.add_argument("--key-file", required=True)
    p_collect.add_argument("--output", required=True)
    p_collect.add_argument("--input-observations", help="Offline observation JSON file (bypasses live RPC)")
    p_collect.add_argument("--rpc-host", default="127.0.0.1")
    p_collect.add_argument("--rpc-port", type=int, default=27650)
    p_collect.add_argument("--network", default="l1-testnet", choices=["l1-devnet", "l1-testnet", "l1-mainnet"])
    p_collect.add_argument("--blocks-per-height-day", type=int, default=14400)
    p_collect.add_argument("--max-blocks-scan", type=int, default=1000)
    p_collect.add_argument("--state-file", help="Persistent observer state JSON path")
    p_collect.add_argument("--observed-at", help="Optional RFC3339 UTC timestamp override")

    p_rec = sub.add_parser("reconcile", help="Reconcile 2-of-3 signed observer bundles into feed")
    p_rec.add_argument("--bundles", nargs="+", required=True)
    p_rec.add_argument("--output", required=True)
    p_rec.add_argument("--keyset-file", help="Optional pinned observer keyset to verify immediately")
    p_rec.add_argument("--node-bin", default="postfiat-node")
    p_rec.add_argument("--generated-at")
    p_rec.add_argument("--now-unix", type=int)

    p_ver = sub.add_parser("verify", help="Verify signed postfiat-validator-uptime-v1 feed")
    p_ver.add_argument("--feed-file", required=True)
    p_ver.add_argument("--keyset-file", required=True)
    p_ver.add_argument("--node-bin", default="postfiat-node")
    p_ver.add_argument("--schema-file", help="Optional JSON Schema Draft 2020-12 file")
    p_ver.add_argument("--now-unix", type=int)

    args = parser.parse_args(argv)

    if args.command == "collect-observer":
        obs_at = parse_utc(args.observed_at) if args.observed_at else dt.datetime.now(UTC)
        if args.input_observations:
            raw = json.loads(Path(args.input_observations).read_text(encoding="utf-8"))
            chain = raw["chain"]
            bpd = int(raw.get("blocks_per_height_day", args.blocks_per_height_day))
            validators = raw["validators"]
            new_state = None
        else:
            state_path = Path(args.state_file) if args.state_file else None
            state = json.loads(state_path.read_text(encoding="utf-8")) if state_path and state_path.exists() else {}
            chain, validators, new_state = collect_live_rpc_observation(
                rpc_host=args.rpc_host,
                rpc_port=args.rpc_port,
                network=args.network,
                blocks_per_height_day=args.blocks_per_height_day,
                max_blocks_scan=args.max_blocks_scan,
                state=state,
            )
            bpd = args.blocks_per_height_day
        bundle = build_observer_bundle(
            chain=chain,
            blocks_per_height_day=bpd,
            validator_inputs=validators,
            observed_at=obs_at,
            key_file=Path(args.key_file),
            node_bin=args.node_bin,
        )
        atomic_write_json(Path(args.output), bundle)
        if new_state is not None and args.state_file:
            new_state["validator_state"] = {
                v["validator_id"]: {
                    "access_state": v["access_state"],
                    "first_active_height": v["first_active_height"],
                    "grace_started_height": v["grace_started_height"],
                    "grace_started_at": v["grace_started_at"],
                    "recovery_floor_height": v["recovery_floor_height"],
                    "operator_identity_id": v["operator_identity_id"],
                    "public_rpc_url": v["public_rpc_url"],
                }
                for v in bundle["validators"]
            }
            atomic_write_json(Path(args.state_file), new_state)
        print(
            json.dumps(
                {
                    "ok": True,
                    "observer_id": bundle["attestation"]["observer_id"],
                    "finalized_height": chain["finalized_height"],
                    "claim_digest_sha256": bundle["attestation"]["claim_digest_sha256"],
                    "output": args.output,
                }
            )
        )
        return 0

    if args.command == "reconcile":
        bundles = [json.loads(Path(p).read_text(encoding="utf-8")) for p in args.bundles]
        feed = reconcile_observer_bundles(bundles, generated_at=args.generated_at)
        out_path = Path(args.output)
        atomic_write_json(out_path, feed)
        report: dict[str, Any] = {
            "ok": True,
            "schema": feed["schema"],
            "validators": len(feed["validators"]),
            "observers": len(feed["observer_attestations"]),
            "output": str(out_path),
        }
        if args.keyset_file:
            verified = verify_feed_via_node(
                args.node_bin, out_path, Path(args.keyset_file), args.now_unix
            )
            report["verification"] = verified
        print(json.dumps(report))
        return 0

    if args.command == "verify":
        feed_path = Path(args.feed_file)
        feed = json.loads(feed_path.read_text(encoding="utf-8"))
        if args.schema_file:
            import jsonschema

            schema = json.loads(Path(args.schema_file).read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator(schema).validate(feed)
        verified = verify_feed_via_node(
            args.node_bin, feed_path, Path(args.keyset_file), args.now_unix
        )
        print(json.dumps(verified, indent=2))
        return 0

    return 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"validator-uptime-observer error: {exc}", file=sys.stderr)
        raise SystemExit(1)
