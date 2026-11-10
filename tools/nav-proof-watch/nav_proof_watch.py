#!/usr/bin/env python3
"""Read-only NAV proof cadence and freshness watchdog (plan 07, criterion 6).

What this is
------------
The spec (`docs/navcoins/navcoin-create-and-swap-spec.md`, gate B2.6) and
`execution/review-03-reserve-proofs-nav-mint-caps.md` (findings R3-F1/R3-F4,
10 November 2026) record two operational gaps this tool exists to close:

* nothing records a **recurring observation window** of finalized NAV proof
  epochs, so "fresh proofs recur" cannot be verified from evidence; and
* no **failure alarm** fires when proofs stop, when a proof's age cannot be
  established in wall-clock time, or when the underlying chain stops
  advancing (block-counted NAV age silently stops moving on an idle chain).

What this tool does
-------------------
* ``observe``   — one read-only GET of the wallet API routes endpoint; one
  JSONL record per route appended to a hash-chained, append-only log. This
  log *is* the recorded observation window.
* ``evaluate``  — reads the log plus a **founder-issued terms file**
  (D-SETTLEMENT, plan 07) and emits alarms. Every threshold comes from the
  terms file. If the terms file is missing or incomplete the tool REFUSES
  (exit 3) and names what is missing. No default value exists anywhere in
  this file.
* ``verify-log``— recomputes the hash chain of the observation log.

What this tool never does
-------------------------
It never signs, submits, pauses, resumes, deploys, restarts or contacts any
write path. It performs exactly one kind of network I/O: HTTP GET of the
wallet API routes endpoint (and, in ``exit_capacity.py``, read-only
``eth_call``). It is safe to run from a laptop.

Exit codes
----------
* ``0`` — observation recorded / evaluation clean / log verified.
* ``2`` — evaluation completed and at least one alarm fired (this is the
  failure signal a systemd unit's OnFailure hook or any pager consumes).
* ``3`` — REFUSED: founder-issued terms missing or incomplete (or the
  sampler reported the API sample itself stale, for ``observe``).
* ``4`` — the wallet API could not be read or its payload is malformed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

SCHEMA_OBSERVATION = "postfiat.nav-proof-watch.observation.v1"
SCHEMA_ALARM = "postfiat.nav-proof-watch.alarm.v1"
SCHEMA_EVALUATION = "postfiat.nav-proof-watch.evaluation.v1"
LOG_NAME = "observations.jsonl"
ALARM_LOG_NAME = "alarms.jsonl"
EVAL_NAME = "evaluation.json"

# The only threshold this tool reads from the environment rather than from
# founder terms is the sampler's own staleness flag, which the wallet API
# already computes (`stale` in `postfiat.navcoin.routes.v1`, 5-minute sample
# age in navcoin-indexer.js). This tool never re-derives it and never
# overrides it.

DEFAULT_ROUTES_URL = "https://5.223.45.94:5173/api/navcoin/routes"
DEFAULT_STATE_DIR = Path("/var/lib/nav-proof-watch")  # override with --state-dir

# ---------------------------------------------------------------------------
# Terms handling — D-SETTLEMENT (plan 07) is the only legitimate source.
# ---------------------------------------------------------------------------

TERMS_FIELDS: dict[str, str] = {
    "max_nav_age_seconds": (
        "Maximum wall-clock age of a finalized NAV epoch for settlement. "
        "Founder, through D-SETTLEMENT in plan 07 (the 4-hour figure in the "
        "spec is an older specification setting, not a founder requirement)."
    ),
    "proof_interval_seconds": (
        "Required maximum interval between consecutive finalized NAV proof "
        "epochs. Founder, through D-SETTLEMENT in plan 07."
    ),
    "observation_window_seconds": (
        "Length of the recorded observation window over which recurrence "
        "and alarms are judged. Founder, through D-SETTLEMENT in plan 07."
    ),
    "decisions_ref": (
        "Dated reference to the founder decision record that authorized "
        "these values (e.g. a DECISIONS.md entry). Founder, D-SETTLEMENT."
    ),
}


def load_terms(path: Path | None) -> tuple[dict[str, Any] | None, list[str]]:
    """Return (terms, missing). missing lists the human-readable refusals."""
    if path is None or not path.is_file():
        return None, [
            f"terms file not found at {path}: every threshold must come from "
            "D-SETTLEMENT (plan 07, founder). Nothing is evaluated without "
            "it.",
            *(f"MISSING: {key} — {what}"
              for key, what in TERMS_FIELDS.items()),
        ]
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return None, [f"terms file {path} is not valid JSON: {exc}"]
    if not isinstance(raw, dict):
        return None, [f"terms file {path} must be a JSON object"]
    missing: list[str] = []
    for key, what in TERMS_FIELDS.items():
        if key not in raw or raw[key] in (None, "", 0):
            missing.append(f"MISSING: {key} — {what}")
    for key in ("max_nav_age_seconds", "proof_interval_seconds",
                "observation_window_seconds"):
        if key in raw:
            value = raw[key]
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                missing.append(
                    f"INVALID: {key} must be a positive integer, got {value!r}")
    if raw.get("decisions_ref") and not isinstance(raw["decisions_ref"], str):
        missing.append("INVALID: decisions_ref must be a string")
    if missing:
        return None, missing
    return raw, []


def terms_digest(terms: dict[str, Any]) -> str:
    return hashlib.sha256(
        json.dumps(terms, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


# ---------------------------------------------------------------------------
# Read-only wallet API access
# ---------------------------------------------------------------------------


def fetch_routes(url: str, timeout: float = 20.0) -> dict[str, Any]:
    request = urllib.request.Request(
        url, headers={"Accept": "application/json",
                      "User-Agent": "nav-proof-watch/1 (read-only)"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if payload.get("schema") != "postfiat.navcoin.routes.v1":
        raise ValueError(
            f"unexpected routes schema: {payload.get('schema')!r}")
    if payload.get("ok") is not True or not isinstance(payload.get("routes"), list):
        raise ValueError("routes payload is not ok/structured as expected")
    return payload


def extract_route_facts(route: dict[str, Any]) -> dict[str, Any]:
    """Pull exactly the fields the observation log records.

    Field names mirror the wallet API's own `postfiat.navcoin.sample.v1` /
    `postfiat.navcoin.routes.v1` output. `operator_observation_unix_ms`
    being null is the R3-F1/R3-F4 condition: NAV age exists only in chain
    blocks and cannot be bounded in wall-clock time. The tool records that
    fact; it never substitutes a block-count conversion.
    """
    proof = route.get("proof") or {}
    supply = (route.get("source") or {}).get("supply") or {}
    profile = proof.get("profile") or {}
    return {
        "route_id": route.get("route_id"),
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
# Hash-chained append-only observation log
# ---------------------------------------------------------------------------


def _sha256_line(line: str) -> str:
    return hashlib.sha256(line.encode("utf-8")).hexdigest()


def read_log(state_dir: Path) -> list[dict[str, Any]]:
    log_path = state_dir / LOG_NAME
    if not log_path.is_file():
        return []
    records: list[dict[str, Any]] = []
    for number, line in enumerate(
            log_path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise SystemExit(
                f"observation log line {number} is corrupt: {exc}; "
                f"run `verify-log` to audit the chain") from exc
    return records


def verify_log_chain(records: list[dict[str, Any]]) -> tuple[bool, str]:
    previous = "0" * 64
    for number, record in enumerate(records, start=1):
        expected_previous = record.get("prev_sha256")
        if expected_previous != previous:
            return False, (
                f"record {number}: prev_sha256 {expected_previous!r} does not "
                f"chain to expected {previous!r}")
        canonical = json.dumps(
            {k: v for k, v in record.items() if k != "record_sha256"},
            sort_keys=True, separators=(",", ":"))
        digest = _sha256_line(canonical)
        if record.get("record_sha256") != digest:
            return False, f"record {number}: record_sha256 mismatch"
        previous = digest
    return True, f"OK: {len(records)} records chain intact, head {previous}"


def append_record(state_dir: Path, record: dict[str, Any]) -> dict[str, Any]:
    log_path = state_dir / LOG_NAME
    state_dir.mkdir(parents=True, exist_ok=True)
    existing = read_log(state_dir)
    previous = ("0" * 64) if not existing else existing[-1]["record_sha256"]
    stamped = dict(record)
    stamped["prev_sha256"] = previous
    canonical = json.dumps(
        {k: v for k, v in stamped.items() if k != "record_sha256"},
        sort_keys=True, separators=(",", ":"))
    stamped["record_sha256"] = _sha256_line(canonical)
    line = json.dumps(stamped, sort_keys=True, separators=(",", ":"))
    descriptor, name = tempfile.mkstemp(
        prefix=f".{LOG_NAME}.", dir=state_dir)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(line + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        # append-only: never rewrite existing bytes
        with open(log_path, "a", encoding="utf-8") as handle:
            handle.write(line + "\n")
            handle.flush()
            os.fsync(handle.fileno())
    finally:
        os.unlink(name)
    return stamped


# ---------------------------------------------------------------------------
# Evaluation over the recorded window
# ---------------------------------------------------------------------------


def alarm(code: str, severity: str, reason: str, **extra: Any) -> dict[str, Any]:
    return {
        "schema": SCHEMA_ALARM,
        "recorded_at_unix_ms": int(time.time() * 1000),
        "code": code,
        "severity": severity,
        "reason": reason,
        **extra,
    }


def evaluate(records: list[dict[str, Any]], terms: dict[str, Any],
             now_ms: int | None = None) -> dict[str, Any]:
    """Judge recurrence, freshness and chain progress over the window.

    Only records inside the last `observation_window_seconds` are judged,
    anchored at the newest record's wall-clock time (or `now_ms` in tests).
    """
    if not records:
        raise SystemExit("REFUSED: no observations recorded yet; "
                         "run `observe` first")
    window_s = terms["observation_window_seconds"]
    anchor_ms = now_ms if now_ms is not None else records[-1]["recorded_at_unix_ms"]
    window_start_ms = anchor_ms - window_s * 1000
    in_window = [r for r in records
                 if r["recorded_at_unix_ms"] >= window_start_ms]
    alarms: list[dict[str, Any]] = []
    route_ids = sorted({r["route_id"] for r in in_window} |
                       {r["route_id"] for r in records})

    per_route: dict[str, Any] = {}
    for route_id in route_ids:
        route_records = [r for r in in_window if r["route_id"] == route_id]
        facts: dict[str, Any] = {
            "samples_in_window": len(route_records),
        }
        if not route_records:
            per_route[route_id] = facts
            alarms.append(alarm(
                "route_missing_from_window", "high",
                "no observation samples for this route inside the window "
                "(sampler outage?)", route_id=route_id))
            continue

        # -- recurrence: distinct finalized epochs seen in the window -------
        epochs: list[dict[str, Any]] = []
        seen: set[tuple[Any, Any]] = set()
        for record in route_records:
            key = (record.get("proof_epoch"), record.get("reserve_packet_hash"))
            if key in seen or record.get("proof_state") != "finalized":
                continue
            seen.add(key)
            epochs.append({
                "epoch": record.get("proof_epoch"),
                "reserve_packet_hash": record.get("reserve_packet_hash"),
                "first_seen_at_unix_ms": record["recorded_at_unix_ms"],
                "operator_observation_unix_ms":
                    record.get("operator_observation_unix_ms"),
            })
        facts["finalized_epochs_in_window"] = [
            {"epoch": e["epoch"]} for e in epochs]

        # gaps between successive epoch first-seens, including window start
        # to the first epoch (an idle proof pipeline shows up here)
        stamps = [window_start_ms] + [e["first_seen_at_unix_ms"] for e in epochs]
        gaps_s = [(b - a) / 1000.0 for a, b in zip(stamps, stamps[1:])]
        if epochs:
            trailing_gap_s = (anchor_ms - epochs[-1]["first_seen_at_unix_ms"]) / 1000.0
        else:
            trailing_gap_s = (anchor_ms - window_start_ms) / 1000.0
        worst_gap_s = max(gaps_s + [trailing_gap_s])
        facts["worst_proof_gap_seconds"] = round(worst_gap_s, 3)
        facts["epochs_in_window_count"] = len(epochs)
        if worst_gap_s > terms["proof_interval_seconds"]:
            alarms.append(alarm(
                "proof_cadence_breach", "high",
                f"longest gap between finalized NAV proof epochs in the "
                f"window is {worst_gap_s:.0f}s, exceeding the founder-set "
                f"proof_interval_seconds="
                f"{terms['proof_interval_seconds']}",
                route_id=route_id, observed_gap_seconds=round(worst_gap_s, 3),
                limit_seconds=terms["proof_interval_seconds"],
                decisions_ref=terms["decisions_ref"]))
        if not epochs:
            alarms.append(alarm(
                "no_proof_finalized_in_window", "high",
                "no finalized NAV proof epoch appeared inside the recorded "
                "observation window", route_id=route_id,
                decisions_ref=terms["decisions_ref"]))

        # -- freshness: wall-clock provable? --------------------------------
        observed = [e["operator_observation_unix_ms"] for e in epochs
                    if e["operator_observation_unix_ms"]]
        latest = route_records[-1]
        if latest.get("operator_observation_unix_ms") is None:
            # R3-F1/R3-F4: block-counted age only; no clock on chain.
            alarms.append(alarm(
                "nav_age_not_wall_clock_verifiable", "high",
                "the finalized packet carries no operator observation time "
                "(age_basis="
                f"{latest.get('proof_age_basis')!r}); its NAV age cannot be "
                "bounded in wall-clock time, so freshness against "
                "max_nav_age_seconds cannot be verified. This is finding "
                "R3-F1 of execution/review-03-reserve-proofs-nav-mint-caps.md",
                route_id=route_id,
                age_blocks=latest.get("proof_age_blocks"),
                age_basis=latest.get("proof_age_basis"),
                decisions_ref=terms["decisions_ref"]))
            facts["freshness_verdict"] = "not_verifiable_no_observation_time"
        else:
            age_s = (anchor_ms - latest["operator_observation_unix_ms"]) / 1000.0
            facts["nav_age_seconds"] = round(age_s, 3)
            if age_s > terms["max_nav_age_seconds"]:
                alarms.append(alarm(
                    "nav_stale", "high",
                    f"finalized NAV is {age_s:.0f}s old, exceeding the "
                    f"founder-set max_nav_age_seconds="
                    f"{terms['max_nav_age_seconds']}",
                    route_id=route_id, observed_age_seconds=round(age_s, 3),
                    limit_seconds=terms["max_nav_age_seconds"],
                    decisions_ref=terms["decisions_ref"]))
                facts["freshness_verdict"] = "stale"
            else:
                facts["freshness_verdict"] = "fresh"

        # -- chain progress: block-counted age needs block production ------
        heights = [r.get("chain_height") for r in route_records
                   if isinstance(r.get("chain_height"), int)]
        if heights:
            facts["chain_height_first_in_window"] = heights[0]
            facts["chain_height_last_in_window"] = heights[-1]
            facts["chain_height_advanced"] = heights[-1] - heights[0]
            if heights[-1] == heights[0] and window_s > 0:
                alarms.append(alarm(
                    "chain_idle_over_window", "medium",
                    f"chain height did not advance across the whole window "
                    f"({heights[0]}→{heights[-1]}): block-counted NAV age is "
                    "not moving, so even block-based freshness limits cannot "
                    "be exercised (idle-chain trap, R3-F1)",
                    route_id=route_id,
                    chain_height=heights[-1]))

        # -- sampler health: the API sample itself -------------------------
        if route_records[-1].get("sampler_reported_stale") is True:
            alarms.append(alarm(
                "sampler_reported_stale", "medium",
                "the wallet indexer flagged its own sample as stale "
                "(sample older than the indexer's 5-minute bound). This is "
                "a sampler health alarm, NOT a NAV freshness verdict "
                "(R3-F4: the two must not be conflated)",
                route_id=route_id))
        facts["sampler_reported_stale_last"] = \
            route_records[-1].get("sampler_reported_stale")
        per_route[route_id] = facts

    return {
        "schema": SCHEMA_EVALUATION,
        "evaluated_at_unix_ms": anchor_ms,
        "window_start_unix_ms": window_start_ms,
        "observation_window_seconds": window_s,
        "log_coverage_seconds": round(
            (anchor_ms - records[0]["recorded_at_unix_ms"]) / 1000.0, 3),
        "coverage_note": (
            "cadence is judged from epochs observed inside this log; a proof "
            "finalized before logging started can only be caught through its "
            "operator observation time (nav_stale) or its absence "
            "(nav_age_not_wall_clock_verifiable), never by block-count "
            "arithmetic"),
        "terms_sha256": terms_digest(terms),
        "decisions_ref": terms["decisions_ref"],
        "samples_judged": len(in_window),
        "per_route": per_route,
        "alarm_count": len(alarms),
        "alarms": alarms,
    }


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_observe(args: argparse.Namespace) -> int:
    try:
        payload = fetch_routes(args.url, timeout=args.timeout)
    except (urllib.error.URLError, ValueError, OSError) as exc:
        print(f"ERROR: wallet API read failed: {exc}", file=sys.stderr)
        return 4
    recorded_at = int(time.time() * 1000)
    written = []
    for route in payload["routes"]:
        facts = extract_route_facts(route)
        if facts.get("route_id") is None:
            continue
        record = {
            "schema": SCHEMA_OBSERVATION,
            "recorded_at_unix_ms": recorded_at,
            "source_url": args.url,
            **facts,
        }
        stamped = append_record(Path(args.state_dir), record)
        written.append(stamped)
        stale_flag = stamped.get("sampler_reported_stale")
        print(
            f"recorded route={stamped['route_id']} epoch="
            f"{stamped.get('proof_epoch')} age_blocks="
            f"{stamped.get('proof_age_blocks')} age_ms="
            f"{stamped.get('proof_age_ms')} observation_time="
            f"{stamped.get('operator_observation_unix_ms')} "
            f"sampler_stale={stale_flag} chain_height="
            f"{stamped.get('chain_height')}")
    if not written:
        print("ERROR: routes payload contained no route records",
              file=sys.stderr)
        return 4
    print(f"appended {len(written)} record(s) to "
          f"{Path(args.state_dir) / LOG_NAME}")
    sampler_stale = [w["route_id"] for w in written
                     if w.get("sampler_reported_stale") is True]
    if sampler_stale:
        # The sampler itself reported staleness: record it as an alarm now,
        # because a wedged sampler would otherwise silently stop the log.
        state = Path(args.state_dir)
        for route_id in sampler_stale:
            record = append_alarm(state, alarm(
                "sampler_reported_stale", "medium",
                "the wallet indexer flagged its own sample as stale during "
                "observe; the observation log may be missing live samples",
                route_id=route_id))
            print(f"ALARM sampler_reported_stale route={route_id} "
                  f"alarm_sha256={record['record_sha256'][:16]}…")
    return 0


def append_alarm(state_dir: Path, alarm_record: dict[str, Any]) -> dict[str, Any]:
    existing: list[dict[str, Any]] = []
    alarm_path = state_dir / ALARM_LOG_NAME
    if alarm_path.is_file():
        for line in alarm_path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                existing.append(json.loads(line))
    previous = ("0" * 64) if not existing else existing[-1]["record_sha256"]
    stamped = dict(alarm_record)
    stamped["prev_sha256"] = previous
    canonical = json.dumps(
        {k: v for k, v in stamped.items() if k != "record_sha256"},
        sort_keys=True, separators=(",", ":"))
    stamped["record_sha256"] = _sha256_line(canonical)
    line = json.dumps(stamped, sort_keys=True, separators=(",", ":"))
    state_dir.mkdir(parents=True, exist_ok=True)
    with open(alarm_path, "a", encoding="utf-8") as handle:
        handle.write(line + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    return stamped


def cmd_evaluate(args: argparse.Namespace) -> int:
    state_dir = Path(args.state_dir)
    terms_path = Path(args.terms) if args.terms else state_dir / "terms.json"
    terms, missing = load_terms(terms_path)
    if terms is None:
        print("REFUSED: no evaluation performed. Founder-set value(s) "
              "missing; this tool invents none.")
        for line in missing:
            print(f"  {line}")
        print("  who supplies them: the founder, through D-SETTLEMENT "
              "(plan 07 `Decisions only the founder can make`).")
        return 3
    records = read_log(state_dir)
    if not records:
        print("REFUSED: no observations recorded yet; run `observe` first")
        return 3
    ok, message = verify_log_chain(records)
    if not ok:
        print(f"REFUSED: observation log chain failed verification: {message}")
        return 3
    result = evaluate(records, terms)
    state_dir.mkdir(parents=True, exist_ok=True)
    for alarm_record in result["alarms"]:
        stamped = append_alarm(state_dir, alarm_record)
        print(f"ALARM [{alarm_record['severity']}] "
              f"{alarm_record['code']} route={alarm_record.get('route_id')} "
              f"alarm_sha256={stamped['record_sha256'][:16]}…")
    evaluation_path = state_dir / EVAL_NAME
    evaluation_path.write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"window {result['window_start_unix_ms']}→"
          f"{result['evaluated_at_unix_ms']} "
          f"({result['observation_window_seconds']}s) "
          f"samples={result['samples_judged']} "
          f"alarms={result['alarm_count']}")
    for route_id, facts in sorted(result["per_route"].items()):
        print(f"  route={route_id} epochs_in_window="
              f"{facts.get('epochs_in_window_count')} freshness="
              f"{facts.get('freshness_verdict')} chain_advanced="
              f"{facts.get('chain_height_advanced')}")
    print(f"evaluation written to {evaluation_path}")
    return 2 if result["alarm_count"] else 0


def cmd_verify_log(args: argparse.Namespace) -> int:
    try:
        records = read_log(Path(args.state_dir))
    except SystemExit as exc:
        print(f"CORRUPT: {exc}")
        return 1
    ok, message = verify_log_chain(records)
    print(message)
    return 0 if ok else 1


def cmd_show_terms_template(args: argparse.Namespace) -> int:
    print(json.dumps({
        "_source": ("Every value here is set by the founder through "
                    "D-SETTLEMENT (plan 07). This template ships with "
                    "every value deliberately unset; the tool refuses to "
                    "evaluate until a completed, dated file replaces it."),
        **{key: None for key in TERMS_FIELDS},
    }, indent=2))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[1] if __doc__ else None,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)

    observe = sub.add_parser(
        "observe", help="record one read-only observation of every route")
    observe.add_argument("--url", default=DEFAULT_ROUTES_URL)
    observe.add_argument("--state-dir", default=str(DEFAULT_STATE_DIR))
    observe.add_argument("--timeout", type=float, default=20.0)
    observe.set_defaults(func=cmd_observe)

    evaluate_p = sub.add_parser(
        "evaluate", help="judge the recorded window against founder terms")
    evaluate_p.add_argument("--state-dir", default=str(DEFAULT_STATE_DIR))
    evaluate_p.add_argument("--terms", default=None,
                            help="path to the D-SETTLEMENT terms JSON "
                                 "(default: <state-dir>/terms.json)")
    evaluate_p.set_defaults(func=cmd_evaluate)

    verify = sub.add_parser("verify-log", help="audit the observation log chain")
    verify.add_argument("--state-dir", default=str(DEFAULT_STATE_DIR))
    verify.set_defaults(func=cmd_verify_log)

    template = sub.add_parser(
        "terms-template", help="print the terms file template (all unset)")
    template.set_defaults(func=cmd_show_terms_template)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
