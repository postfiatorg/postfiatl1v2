#!/usr/bin/env python3
"""Read-only NAV proof cadence and freshness watchdog.

Why
---
A NAVCoin's finalized NAV age is counted in PFTL blocks. On an idle chain,
block-counted age stops moving, so a weeks-old NAV can still look "fresh".
The wallet API's own `stale` flag describes the age of the API sample, not
the age of the NAV. Without a recorded window, nobody can show that fresh
proofs keep arriving. Without an alarm, nobody notices when they stop.

What this tool does
-------------------
* ``observe``     one read-only GET of the wallet API routes endpoint
  (``--url``, required, no default); one JSONL record per route is appended
  to a hash-chained, append-only log. That log is the recorded observation
  window.
* ``evaluate``    judges the log against the operator's terms file and
  emits alarms. Every threshold comes from the terms file. The values come
  from the product owner's settlement terms; the tool has no default and
  refuses (exit 3) while a value is missing.
* ``verify-log``  recomputes the hash chain and compares it with the head
  file, so an edited, removed, reordered or truncated record is detected.
* ``repair-tail`` moves a torn final line (an append interrupted by a crash)
  aside so ``observe`` can continue. The torn bytes are kept for inspection.

What this tool never does
-------------------------
It never signs, submits, pauses, resumes, deploys or restarts anything. Its
only network I/O is the HTTP GET above (and, in ``exit_capacity.py``,
read-only ``eth_call``). It writes only inside ``--state-dir``.

Exit codes
----------
* ``0`` observation recorded / evaluation clean / log verified.
* ``1`` the local log is damaged (chain broken, truncated, torn tail).
* ``2`` evaluation completed and at least one alarm fired.
* ``3`` REFUSED: terms missing, incomplete or unable to alarm.
* ``4`` the wallet API could not be read or its answer is malformed.
* ``5`` evaluation completed without alarms, but the recorded time does not
  yet cover the observation window for every route: not clean, not an alarm.
"""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Iterator

SCHEMA_OBSERVATION = "postfiat.nav-proof-watch.observation.v2"
SCHEMA_ALARM = "postfiat.nav-proof-watch.alarm.v1"
SCHEMA_EVALUATION = "postfiat.nav-proof-watch.evaluation.v2"
SCHEMA_HEAD = "postfiat.nav-proof-watch.head.v1"
LOG_NAME = "observations.jsonl"
HEAD_NAME = "observations.head.json"
ALARM_LOG_NAME = "alarms.jsonl"
EVAL_NAME = "evaluation.json"
LOCK_NAME = ".lock"
GENESIS = "0" * 64
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
# The newest record may be at most this many sampling intervals old before
# the log counts as stopped: one missed run is tolerated, two are not.
STALE_LOG_INTERVALS = 2

EXIT_OK, EXIT_DAMAGED, EXIT_ALARM, EXIT_REFUSED, EXIT_READ, EXIT_NOT_COVERED = (
    0, 1, 2, 3, 4, 5)


class LogDamaged(Exception):
    """The local observation log cannot be trusted as it stands."""


class TornTail(LogDamaged):
    """The final line is incomplete: an append was interrupted."""


# ---------------------------------------------------------------------------
# Terms: the product owner's settlement terms, supplied by the operator
# ---------------------------------------------------------------------------

TERMS_FIELDS: dict[str, str] = {
    "max_nav_age_seconds": (
        "maximum wall-clock age of a finalized NAV for settlement"),
    "proof_interval_seconds": (
        "maximum interval between two newly finalized NAV proof epochs"),
    "observation_window_seconds": (
        "length of the recorded window over which recurrence is judged; "
        "must be longer than proof_interval_seconds"),
    "sample_interval_seconds": (
        "how often observe runs; the log counts as stopped when its newest "
        f"record is older than {STALE_LOG_INTERVALS} intervals"),
    "terms_ref": (
        "dated reference to the product owner's settlement terms that set "
        "these values"),
}
INTEGER_TERMS = ("max_nav_age_seconds", "proof_interval_seconds",
                 "observation_window_seconds", "sample_interval_seconds")


def load_terms(path: Path | None) -> tuple[dict[str, Any] | None, list[str]]:
    """Return (terms, problems). Any problem means REFUSED."""
    if path is None or not path.is_file():
        return None, [
            f"terms file not found at {path}: every threshold comes from the "
            "product owner's settlement terms; nothing is evaluated without "
            "them.",
            *(f"MISSING: {key} ({what})" for key, what in TERMS_FIELDS.items()),
        ]
    try:
        raw = json.loads(path.read_text(encoding="utf-8"),
                         object_pairs_hook=_no_duplicate_keys)
    except (json.JSONDecodeError, ValueError) as exc:
        return None, [f"terms file {path} is not valid JSON: {exc}"]
    if not isinstance(raw, dict):
        return None, [f"terms file {path} must be a JSON object"]
    problems: list[str] = []
    for key, what in TERMS_FIELDS.items():
        if key not in raw or raw[key] in (None, ""):
            problems.append(f"MISSING: {key} ({what})")
    for key in INTEGER_TERMS:
        value = raw.get(key)
        if value is not None and (not isinstance(value, int)
                                  or isinstance(value, bool) or value <= 0):
            problems.append(
                f"INVALID: {key} must be a positive integer, got {value!r}")
    if raw.get("terms_ref") is not None and not isinstance(raw["terms_ref"], str):
        problems.append("INVALID: terms_ref must be a string")
    if not problems:
        window = raw["observation_window_seconds"]
        if window <= raw["proof_interval_seconds"]:
            problems.append(
                "INVALID: observation_window_seconds must be longer than "
                "proof_interval_seconds; otherwise no gap inside the window "
                "can exceed the interval and proof_cadence_breach could "
                "never fire")
        if raw["sample_interval_seconds"] * STALE_LOG_INTERVALS >= window:
            problems.append(
                "INVALID: sample_interval_seconds is too long for the "
                "observation window; a stopped log would not be noticed "
                "inside the window")
    if problems:
        return None, problems
    return raw, []


def terms_digest(terms: dict[str, Any]) -> str:
    return hashlib.sha256(
        json.dumps(terms, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


# ---------------------------------------------------------------------------
# Read-only wallet API access
# ---------------------------------------------------------------------------


def fetch_routes(url: str, timeout: float = 20.0) -> dict[str, Any]:
    """One GET. TLS certificates are verified (the default ssl context)."""
    request = urllib.request.Request(
        url, headers={"Accept": "application/json",
                      "User-Agent": "nav-proof-watch/2 (read-only)"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        body = response.read(MAX_RESPONSE_BYTES + 1)
    if len(body) > MAX_RESPONSE_BYTES:
        raise ValueError("routes answer exceeds the size bound")
    payload = json.loads(body.decode("utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("routes answer is not a JSON object")
    if payload.get("schema") != "postfiat.navcoin.routes.v1":
        raise ValueError(
            f"unexpected routes schema: {payload.get('schema')!r}")
    routes = payload.get("routes")
    if payload.get("ok") is not True or not isinstance(routes, list) or not all(
            isinstance(route, dict) for route in routes):
        raise ValueError("routes answer is not ok/structured as expected")
    return payload


def _object(value: Any, name: str) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ValueError(f"route field {name} is not an object")
    return value


def extract_route_facts(route: dict[str, Any]) -> dict[str, Any]:
    """Pull exactly the fields the observation log records.

    A null `operator_observation_unix_ms` means the NAV's age exists only as
    a block count and cannot be bounded in wall-clock time. The tool records
    that fact; it never substitutes a block-count conversion.
    """
    if not isinstance(route, dict):
        raise ValueError("route entry is not an object")
    proof = _object(route.get("proof"), "proof")
    source = _object(route.get("source"), "source")
    supply = _object(source.get("supply"), "source.supply")
    profile = _object(proof.get("profile"), "proof.profile")
    route_id = route.get("route_id")
    if route_id is not None and not isinstance(route_id, str):
        raise ValueError("route_id is not a string")
    return {
        "route_id": route_id,
        "name": route.get("name"),
        "chain_height": route.get("chain_height"),
        "chain_state_root": route.get("chain_state_root"),
        "sample_observed_at_ms": route.get("observed_at_ms"),
        "sampler_reported_stale": route.get("stale"),
        "state": route.get("state"),
        "proof_epoch": proof.get("epoch"),
        "proof_state": proof.get("state"),
        "proof_submitted_at_height": proof.get("submitted_at_height"),
        "proof_age_blocks": proof.get("age_blocks"),
        "proof_age_basis": proof.get("age_basis"),
        "proof_age_ms": proof.get("age_ms"),
        "operator_observation_unix_ms": proof.get("operator_observation_unix_ms"),
        "reserve_packet_hash": proof.get("reserve_packet_hash"),
        "profile_max_snapshot_age_blocks": profile.get("max_snapshot_age_blocks"),
        "settlement_reserve_atoms": supply.get("settlement_reserve_atoms"),
        "reserve_backed_redeem_atoms": supply.get("reserve_backed_redeem_atoms"),
    }


# ---------------------------------------------------------------------------
# Hash-chained append-only log with a head file
# ---------------------------------------------------------------------------


def _no_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key {key!r}")
        result[key] = value
    return result


def _canonical(value: dict[str, Any]) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _record_digest(record: dict[str, Any]) -> str:
    body = {k: v for k, v in record.items() if k != "record_sha256"}
    return hashlib.sha256(_canonical(body).encode("utf-8")).hexdigest()


@contextlib.contextmanager
def state_lock(state_dir: Path) -> Iterator[None]:
    """Exclusive lock: one writer at a time (timer run vs. a manual run)."""
    state_dir.mkdir(parents=True, exist_ok=True)
    with open(state_dir / LOCK_NAME, "a", encoding="utf-8") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def _fsync_dir(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write_atomic(path: Path, text: str) -> None:
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with open(temporary, "w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    _fsync_dir(path.parent)


def parse_log_lines(path: Path) -> list[dict[str, Any]]:
    """Parse a JSONL log strictly: canonical lines, no duplicate keys."""
    if not path.is_file():
        return []
    data = path.read_bytes()
    if not data:
        return []
    torn = not data.endswith(b"\n")  # every complete record ends with one
    lines = data.split(b"\n")
    if not torn:
        lines = lines[:-1]
    records: list[dict[str, Any]] = []
    for number, raw in enumerate(lines, start=1):
        last = number == len(lines)
        try:
            text = raw.decode("utf-8")
            record = json.loads(text, object_pairs_hook=_no_duplicate_keys)
        except (UnicodeDecodeError, ValueError) as exc:
            if last:
                raise TornTail(
                    f"{path.name} line {number} is incomplete ({exc}); an "
                    "append was interrupted. Repair with: python3 "
                    "nav_proof_watch.py repair-tail --state-dir "
                    f"{path.parent}") from exc
            raise LogDamaged(f"{path.name} line {number}: {exc}") from exc
        if not isinstance(record, dict) or _canonical(record) != text:
            raise LogDamaged(
                f"{path.name} line {number} is not a canonical record")
        if torn and last:
            raise TornTail(
                f"{path.name} line {number} has no final newline; an append "
                "was interrupted. Repair with: python3 nav_proof_watch.py "
                f"repair-tail --state-dir {path.parent}")
        records.append(record)
    return records


def read_log(state_dir: Path) -> list[dict[str, Any]]:
    return parse_log_lines(state_dir / LOG_NAME)


def read_head(state_dir: Path) -> dict[str, Any] | None:
    path = state_dir / HEAD_NAME
    if not path.is_file():
        return None
    try:
        head = json.loads(path.read_text(encoding="utf-8"),
                          object_pairs_hook=_no_duplicate_keys)
    except ValueError as exc:
        raise LogDamaged(f"{HEAD_NAME} is not valid JSON: {exc}") from exc
    if (not isinstance(head, dict) or head.get("schema") != SCHEMA_HEAD
            or not isinstance(head.get("record_count"), int)
            or not isinstance(head.get("head_sha256"), str)):
        raise LogDamaged(f"{HEAD_NAME} is malformed")
    return head


def verify_log_chain(records: list[dict[str, Any]],
                     head: dict[str, Any] | None = None) -> tuple[bool, str]:
    previous = GENESIS
    for number, record in enumerate(records, start=1):
        if record.get("prev_sha256") != previous:
            return False, (
                f"record {number}: prev_sha256 {record.get('prev_sha256')!r} "
                f"does not chain to expected {previous!r}")
        digest = _record_digest(record)
        if record.get("record_sha256") != digest:
            return False, f"record {number}: record_sha256 mismatch"
        previous = digest
    note = ""
    if head is not None:
        count = head["record_count"]
        if len(records) < count:
            return False, (
                f"log holds {len(records)} records but the head file "
                f"recorded {count}: the tail was truncated")
        at_head = records[count - 1]["record_sha256"] if count else GENESIS
        if at_head != head["head_sha256"]:
            return False, (
                f"record {count} does not match the head file's hash: the "
                "log was rewritten")
        if len(records) > count:
            note = (f" (head file {len(records) - count} record(s) behind: an "
                    "append was interrupted before the head update)")
    elif records:
        note = " (no head file: truncation cannot be checked)"
    return True, (f"OK: {len(records)} records chain intact, head "
                  f"{previous}{note}")


def _append_chained(path: Path, existing: list[dict[str, Any]],
                    record: dict[str, Any]) -> dict[str, Any]:
    stamped = dict(record)
    stamped["prev_sha256"] = existing[-1]["record_sha256"] if existing else GENESIS
    stamped["record_sha256"] = _record_digest(stamped)
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(_canonical(stamped) + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    existing.append(stamped)
    return stamped


def append_records(state_dir: Path,
                   records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Append under the lock, then advance the head file atomically."""
    with state_lock(state_dir):
        existing = read_log(state_dir)
        ok, message = verify_log_chain(existing, read_head(state_dir))
        if not ok:
            raise LogDamaged(message)
        new_log = not (state_dir / LOG_NAME).exists()
        stamped = [_append_chained(state_dir / LOG_NAME, existing, record)
                   for record in records]
        if new_log:
            _fsync_dir(state_dir)
        _write_atomic(state_dir / HEAD_NAME, json.dumps({
            "schema": SCHEMA_HEAD,
            "record_count": len(existing),
            "head_sha256": existing[-1]["record_sha256"] if existing else GENESIS,
        }, sort_keys=True) + "\n")
        return stamped


def append_record(state_dir: Path, record: dict[str, Any]) -> dict[str, Any]:
    return append_records(state_dir, [record])[0]


def append_alarms(state_dir: Path,
                  alarms: list[dict[str, Any]]) -> list[dict[str, Any]]:
    with state_lock(state_dir):
        path = state_dir / ALARM_LOG_NAME
        existing = parse_log_lines(path)
        ok, message = verify_log_chain(existing)
        if not ok:
            raise LogDamaged(f"{ALARM_LOG_NAME}: {message}")
        return [_append_chained(path, existing, alarm_record)
                for alarm_record in alarms]


def repair_tail(state_dir: Path) -> str:
    """Move a torn final line aside; never touch complete records."""
    with state_lock(state_dir):
        path = state_dir / LOG_NAME
        try:
            read_log(state_dir)
            return "nothing to repair: the log has no torn final line"
        except TornTail:
            pass
        data = path.read_bytes()
        complete = data if not data.endswith(b"\n") else data[:-1]
        cut = complete.rfind(b"\n") + 1
        aside = state_dir / f"{LOG_NAME}.torn-{int(time.time() * 1000)}"
        _write_atomic(aside, data[cut:].decode("utf-8", "replace"))
        with open(path, "r+b") as handle:
            handle.truncate(cut)
            handle.flush()
            os.fsync(handle.fileno())
        read_log(state_dir)  # the remainder must now parse
        return f"moved {len(data) - cut} torn byte(s) to {aside.name}"


# ---------------------------------------------------------------------------
# Evaluation over the recorded window
# ---------------------------------------------------------------------------


def now_ms() -> int:
    return int(time.time() * 1000)


def alarm(code: str, severity: str, reason: str, **extra: Any) -> dict[str, Any]:
    return {
        "schema": SCHEMA_ALARM,
        "recorded_at_unix_ms": now_ms(),
        "code": code,
        "severity": severity,
        "reason": reason,
        **extra,
    }


def _epoch_key(record: dict[str, Any]) -> tuple[Any, Any]:
    return (record.get("proof_epoch"), record.get("reserve_packet_hash"))


def evaluate(records: list[dict[str, Any]], terms: dict[str, Any],
             now: int | None = None) -> dict[str, Any]:
    """Judge recurrence, freshness and chain progress over the window.

    The window ends at the current time, not at the newest record, so a log
    that stopped is judged as stopped. Time before a route's first record is
    not recorded time and never counts as a proof gap.
    """
    if not records:
        raise ValueError("no observations recorded yet")
    anchor = now_ms() if now is None else now
    window_s = terms["observation_window_seconds"]
    window_start = anchor - window_s * 1000
    ref = terms["terms_ref"]
    alarms: list[dict[str, Any]] = []

    newest = max(r["recorded_at_unix_ms"] for r in records)
    log_age_s = (anchor - newest) / 1000.0
    stale_bound_s = terms["sample_interval_seconds"] * STALE_LOG_INTERVALS
    if log_age_s > stale_bound_s:
        alarms.append(alarm(
            "observation_log_stale", "high",
            f"the newest observation is {log_age_s:.0f}s old, more than "
            f"{STALE_LOG_INTERVALS} sampling intervals ({stale_bound_s}s): "
            "observe has stopped or keeps failing, so nothing below "
            "describes the present", newest_record_unix_ms=newest,
            limit_seconds=stale_bound_s, terms_ref=ref))

    route_ids = sorted({r["route_id"] for r in records})
    per_route: dict[str, Any] = {}
    uncovered: list[str] = []
    for route_id in route_ids:
        history = [r for r in records if r["route_id"] == route_id]
        in_window = [r for r in history
                     if window_start <= r["recorded_at_unix_ms"] <= anchor]
        first_ms = history[0]["recorded_at_unix_ms"]
        coverage_start = max(window_start, first_ms)
        covered = first_ms <= window_start
        facts: dict[str, Any] = {
            "samples_in_window": len(in_window),
            "first_recorded_unix_ms": first_ms,
            "window_covered": covered,
            "recorded_seconds_in_window": round((anchor - coverage_start) / 1000.0, 3),
        }
        per_route[route_id] = facts
        if not in_window:
            alarms.append(alarm(
                "route_missing_from_window", "high",
                "no observation of this route inside the window although "
                "it was observed before", route_id=route_id))
            continue

        # Epochs newly finalized while the log was watching: an epoch already
        # shown in the route's first record was finalized before logging.
        first_seen: dict[tuple[Any, Any], int] = {}
        for record in history:
            if record.get("proof_state") == "finalized":
                first_seen.setdefault(_epoch_key(record),
                                      record["recorded_at_unix_ms"])
        initial = {_epoch_key(history[0])}
        new_epochs = sorted(
            (stamp, key) for key, stamp in first_seen.items()
            if key not in initial and coverage_start <= stamp <= anchor)
        facts["new_epochs_in_window"] = [
            {"epoch": key[0], "first_seen_unix_ms": stamp}
            for stamp, key in new_epochs]
        stamps = [coverage_start] + [stamp for stamp, _ in new_epochs] + [anchor]
        worst_gap_s = max((b - a) / 1000.0 for a, b in zip(stamps, stamps[1:]))
        facts["worst_proof_gap_seconds"] = round(worst_gap_s, 3)

        if not covered:
            uncovered.append(route_id)
            facts["cadence_verdict"] = "window_not_yet_covered"
        else:
            facts["cadence_verdict"] = "judged"
            if worst_gap_s > terms["proof_interval_seconds"]:
                alarms.append(alarm(
                    "proof_cadence_breach", "high",
                    f"the longest recorded stretch without a newly finalized "
                    f"NAV proof epoch is {worst_gap_s:.0f}s, more than "
                    f"proof_interval_seconds={terms['proof_interval_seconds']}",
                    route_id=route_id, observed_gap_seconds=round(worst_gap_s, 3),
                    limit_seconds=terms["proof_interval_seconds"], terms_ref=ref))
            if not new_epochs:
                alarms.append(alarm(
                    "no_proof_finalized_in_window", "high",
                    "no NAV proof epoch was first seen inside the window; "
                    "the epoch still displayed was finalized earlier",
                    route_id=route_id,
                    displayed_epoch=in_window[-1].get("proof_epoch"),
                    terms_ref=ref))

        latest = in_window[-1]
        observation = latest.get("operator_observation_unix_ms")
        if observation is None:
            alarms.append(alarm(
                "nav_age_not_wall_clock_verifiable", "high",
                "the finalized packet carries no operator observation time "
                f"(age_basis={latest.get('proof_age_basis')!r}); its NAV age "
                "exists only as a block count, which stops moving on an idle "
                "chain, so freshness against max_nav_age_seconds cannot be "
                "verified", route_id=route_id,
                age_blocks=latest.get("proof_age_blocks"),
                age_basis=latest.get("proof_age_basis"), terms_ref=ref))
            facts["freshness_verdict"] = "not_verifiable_no_observation_time"
        elif not isinstance(observation, int) or isinstance(observation, bool):
            facts["freshness_verdict"] = "not_verifiable_malformed_time"
            alarms.append(alarm(
                "nav_age_not_wall_clock_verifiable", "high",
                f"operator observation time {observation!r} is not an integer",
                route_id=route_id, terms_ref=ref))
        else:
            age_s = (anchor - observation) / 1000.0
            facts["nav_age_seconds"] = round(age_s, 3)
            if age_s > terms["max_nav_age_seconds"]:
                alarms.append(alarm(
                    "nav_stale", "high",
                    f"finalized NAV is {age_s:.0f}s old, more than "
                    f"max_nav_age_seconds={terms['max_nav_age_seconds']}",
                    route_id=route_id, observed_age_seconds=round(age_s, 3),
                    limit_seconds=terms["max_nav_age_seconds"], terms_ref=ref))
                facts["freshness_verdict"] = "stale"
            else:
                facts["freshness_verdict"] = "fresh"

        heights = [r.get("chain_height") for r in in_window
                   if isinstance(r.get("chain_height"), int)
                   and not isinstance(r.get("chain_height"), bool)]
        if heights:
            facts["chain_height_first_in_window"] = heights[0]
            facts["chain_height_last_in_window"] = heights[-1]
            facts["chain_height_advanced"] = heights[-1] - heights[0]
            if covered and len(heights) >= 2 and heights[-1] == heights[0]:
                alarms.append(alarm(
                    "chain_idle_over_window", "medium",
                    f"chain height did not advance across the window "
                    f"({heights[0]} to {heights[-1]}): block-counted NAV age "
                    "is not moving, so block-based freshness limits cannot "
                    "be exercised", route_id=route_id,
                    chain_height=heights[-1]))

        if latest.get("sampler_reported_stale") is True:
            alarms.append(alarm(
                "sampler_reported_stale", "medium",
                "the wallet indexer flagged its own sample as stale; this is "
                "sampler health, not a NAV freshness verdict",
                route_id=route_id))
        facts["sampler_reported_stale_last"] = latest.get("sampler_reported_stale")

    if alarms:
        outcome = "alarm"
    elif uncovered:
        outcome = "window_not_yet_covered"
    else:
        outcome = "clean"
    return {
        "schema": SCHEMA_EVALUATION,
        "evaluated_at_unix_ms": anchor,
        "window_start_unix_ms": window_start,
        "observation_window_seconds": window_s,
        "newest_record_unix_ms": newest,
        "outcome": outcome,
        "routes_not_yet_covered": uncovered,
        "terms_sha256": terms_digest(terms),
        "terms_ref": ref,
        "samples_judged": sum(f["samples_in_window"] for f in per_route.values()),
        "per_route": per_route,
        "alarm_count": len(alarms),
        "alarms": alarms,
    }


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_observe(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir)
    try:
        payload = fetch_routes(args.url, timeout=args.timeout)
        facts = [extract_route_facts(route) for route in payload["routes"]]
    except (urllib.error.URLError, ValueError, OSError) as exc:
        print(f"ERROR: wallet API read failed: {exc}", file=sys.stderr)
        return EXIT_READ
    recorded_at = now_ms()
    records = [{"schema": SCHEMA_OBSERVATION, "recorded_at_unix_ms": recorded_at,
                **fact} for fact in facts if fact.get("route_id") is not None]
    if not records:
        print("ERROR: routes answer contained no route records", file=sys.stderr)
        return EXIT_READ
    try:
        written = append_records(state_dir, records)
    except LogDamaged as exc:
        print(f"LOG DAMAGED: {exc}", file=sys.stderr)
        return EXIT_DAMAGED
    for stamped in written:
        print(
            f"recorded route={stamped['route_id']} epoch="
            f"{stamped.get('proof_epoch')} age_blocks="
            f"{stamped.get('proof_age_blocks')} observation_time="
            f"{stamped.get('operator_observation_unix_ms')} "
            f"sampler_stale={stamped.get('sampler_reported_stale')} "
            f"chain_height={stamped.get('chain_height')}")
    print(f"appended {len(written)} record(s) to {state_dir / LOG_NAME}")
    return EXIT_OK


def cmd_evaluate(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir)
    terms_path = Path(args.terms) if args.terms else state_dir / "terms.json"
    terms, problems = load_terms(terms_path)
    if terms is None:
        print("REFUSED: no evaluation performed. The tool has no default "
              "for any threshold.")
        for line in problems:
            print(f"  {line}")
        print("  who supplies them: the operator, from the product owner's "
              "settlement terms.")
        return EXIT_REFUSED
    try:
        records = read_log(state_dir)
        ok, message = verify_log_chain(records, read_head(state_dir))
    except LogDamaged as exc:
        print(f"LOG DAMAGED: {exc}")
        return EXIT_DAMAGED
    if not records:
        print("REFUSED: no observations recorded yet; run `observe` first")
        return EXIT_REFUSED
    if not ok:
        print(f"LOG DAMAGED: observation log failed verification: {message}")
        return EXIT_DAMAGED
    result = evaluate(records, terms)
    try:
        stamped = append_alarms(state_dir, result["alarms"])
    except LogDamaged as exc:
        print(f"LOG DAMAGED: {exc}")
        return EXIT_DAMAGED
    for record in stamped:
        print(f"ALARM [{record['severity']}] {record['code']} "
              f"route={record.get('route_id')}")
    _write_atomic(state_dir / EVAL_NAME,
                  json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"window {result['window_start_unix_ms']} to "
          f"{result['evaluated_at_unix_ms']} "
          f"({result['observation_window_seconds']}s) outcome="
          f"{result['outcome']} alarms={result['alarm_count']}")
    for route_id, facts in sorted(result["per_route"].items()):
        print(f"  route={route_id} cadence={facts.get('cadence_verdict')} "
              f"freshness={facts.get('freshness_verdict')} "
              f"chain_advanced={facts.get('chain_height_advanced')}")
    if result["alarm_count"]:
        return EXIT_ALARM
    if result["outcome"] == "window_not_yet_covered":
        print("WINDOW NOT YET COVERED: recorded time is shorter than the "
              "observation window for: " + ", ".join(result["routes_not_yet_covered"]))
        return EXIT_NOT_COVERED
    return EXIT_OK


def cmd_verify_log(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir)
    try:
        ok, message = verify_log_chain(read_log(state_dir), read_head(state_dir))
    except LogDamaged as exc:
        print(f"LOG DAMAGED: {exc}")
        return EXIT_DAMAGED
    print(message)
    return EXIT_OK if ok else EXIT_DAMAGED


def cmd_repair_tail(args: argparse.Namespace) -> int:
    try:
        print(repair_tail(Path(args.state_dir)))
    except LogDamaged as exc:
        print(f"LOG DAMAGED beyond the final line: {exc}")
        return EXIT_DAMAGED
    return EXIT_OK


def cmd_show_terms_template(args: argparse.Namespace) -> int:
    print(json.dumps({key: None for key in TERMS_FIELDS}, indent=2))
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0] if __doc__ else None)
    sub = parser.add_subparsers(dest="command", required=True)

    observe = sub.add_parser("observe", help="record one observation of every route")
    observe.add_argument("--url", required=True,
                         help="wallet API routes endpoint (no default)")
    observe.add_argument("--state-dir", required=True)
    observe.add_argument("--timeout", type=float, default=20.0)
    observe.set_defaults(func=cmd_observe)

    evaluate_p = sub.add_parser("evaluate", help="judge the recorded window")
    evaluate_p.add_argument("--state-dir", required=True)
    evaluate_p.add_argument("--terms", default=None,
                            help="terms JSON (default: <state-dir>/terms.json)")
    evaluate_p.set_defaults(func=cmd_evaluate)

    verify = sub.add_parser("verify-log", help="audit the observation log")
    verify.add_argument("--state-dir", required=True)
    verify.set_defaults(func=cmd_verify_log)

    repair = sub.add_parser("repair-tail", help="move a torn final line aside")
    repair.add_argument("--state-dir", required=True)
    repair.set_defaults(func=cmd_repair_tail)

    template = sub.add_parser("terms-template", help="print the empty terms file")
    template.set_defaults(func=cmd_show_terms_template)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
