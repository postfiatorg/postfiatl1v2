#!/usr/bin/env python3
"""Validator uptime observer for the postfiat-validator-uptime-v1 feed.

One of three independent observers. It only reads: it holds no validator key
and no governance key. The contract is docs/specs/validator-uptime-feed-v1.md.

  run      Read the chain once, date the new blocks, count accepted votes,
           apply the access lifecycle, and write one signed attestation.
           Run it from a timer every 5 minutes (it must run at least every 15).
  bundle   Put the observers' attestations (files or URLs) into one feed.
           Needs no key. Anybody may run it.
  verify   Read a feed the way a consumer must and print the result.

How a block gets its day: block headers carry no time. Blocks that appear
between two consecutive reads on the same UTC day belong to that day. If the
head moved between two reads that straddle midnight, or two reads are more than
900 seconds apart, the blocks cannot be dated and the affected days are marked
incomplete. Nothing is inferred for the time before the observer started.

How a vote is counted: a validator is credited for a block when its vote is in
the block's precommit or prepare certificate, or when the proposer's vote
record for that block lists it as accepted (also after the quorum closed).
A vote record is unsigned, so it is taken only from the block's proposer: a
source given as `validator-id=tcp://host:port` is that validator's own RPC, and
only its records for the blocks it proposed count. A source without an id gives
blocks and certificates, never vote records.

Who is new: a validator that enters the registry while the observer runs is
new from that day (the 30-day ramp-in). A validator already in the registry
when the observer first starts is not: its activation day comes from
`--activation-file` (the ratification dates), and without one it is taken to
be older than the ramp-in, so a restarted observer never hands out a new
ramp-in.

Signing and verifying are done by `postfiat-node` (observer-uptime-sign,
observer-uptime-bundle, observer-uptime-verify).
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
from typing import Any
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parent))
import validator_uptime_contract as contract  # noqa: E402

STATE_SCHEMA = "postfiat-validator-uptime-observer-state-v1"
RPC_VERSION = "postfiat-local-rpc-v1"
MAX_OBSERVATION_GAP_SECONDS = 900
BLOCKS_PAGE = 8
VOTE_RECORDS_PAGE = 64
# Vote records are fetched for blocks of this many most recent days; a proposer
# that was unreachable gets that long to answer.
VOTE_RECORD_RETRY_DAYS = 2
MAX_REPLY_BYTES = 64 * 1024 * 1024
CREDITED_OUTCOMES = {"accepted", "late_after_deadline"}

QUORUM_POLICY = {
    "observer_count": 3,
    "quorum_threshold": contract.QUORUM_THRESHOLD,
    "algorithm_id": "ML-DSA-65",
    "signing_context": contract.SIGNING_CONTEXT.decode(),
    "stale_after_seconds": contract.STALE_AFTER_SECONDS,
    "max_future_skew_seconds": contract.MAX_FUTURE_SKEW_SECONDS,
    "indeterminate_behavior": "block_new_bindings_and_credits_preserve_existing_access",
}
WINDOW_SPEC = {
    "metric_version": "accepted-votes-observer-brackets-v1",
    "credit_sources": ["precommit_certificate", "prepare_certificate", "proposer_vote_record"],
    "time_basis": "observer_brackets",
    "max_observation_gap_seconds": MAX_OBSERVATION_GAP_SECONDS,
    "evaluation_horizon_days": contract.EVALUATION_HORIZON_DAYS,
    "min_observed_days": contract.MIN_OBSERVED_DAYS,
    "liveness_threshold_bps": contract.LIVENESS_THRESHOLD_BPS,
    "grace_period_days": contract.GRACE_PERIOD_DAYS,
    "recovery_consecutive_days": contract.RECOVERY_CONSECUTIVE_DAYS,
}


class ObserverError(Exception):
    """The observer cannot go on without writing something false. It stops instead."""


# --------------------------------------------------------------------------- reading the chain


class Source:
    """A read-only RPC endpoint: `tcp://host:port` (rpc-serve) or `http(s)://.../rpc` (the HTTP edge).

    `validator-id=<endpoint>` says the endpoint is that validator's own RPC.
    """

    def __init__(self, spec: str, timeout: float = 20.0) -> None:
        self.validator_id: str | None = None
        head, sep, rest = spec.partition("=")
        if sep and "://" not in head:
            if not head:
                raise ObserverError(f"source `{spec}` has an empty validator id")
            self.validator_id, spec = head, rest
        self.spec = spec
        self.timeout = timeout
        if spec.startswith(("http://", "https://")):
            self.kind = "http"
        elif spec.startswith("tcp://") and spec.rsplit(":", 1)[-1].isdigit():
            self.kind = "tcp"
            host, port = spec[len("tcp://") :].rsplit(":", 1)
            self.address = (host, int(port))
        else:
            raise ObserverError(f"source `{spec}` must be tcp://host:port or http(s)://host/rpc")

    def call(self, method: str, params: dict[str, Any] | None = None) -> Any:
        request = json.dumps({"version": RPC_VERSION, "id": f"observer-{method}", "method": method, "params": params or {}})
        if self.kind == "tcp":
            with socket.create_connection(self.address, timeout=self.timeout) as sock:
                sock.sendall(request.encode() + b"\n")
                buffer = b""
                while not buffer.endswith(b"\n"):
                    chunk = sock.recv(1 << 20)
                    if not chunk:
                        break
                    buffer += chunk
                    if len(buffer) > MAX_REPLY_BYTES:
                        raise ObserverError(f"{self.spec}: reply to {method} is too large")
        else:
            http_request = urllib.request.Request(
                self.spec, data=request.encode(), headers={"Content-Type": "application/json"}, method="POST"
            )
            with urllib.request.urlopen(http_request, timeout=self.timeout) as response:  # noqa: S310 (scheme checked above)
                buffer = response.read(MAX_REPLY_BYTES + 1)
        reply = json.loads(buffer)
        if not reply.get("ok"):
            error = reply.get("error") or {}
            raise ObserverError(f"{self.spec}: {method} failed: {error.get('code')}: {error.get('message')}")
        return reply["result"]


def best_source(sources: list[Source], chain_id: str | None) -> tuple[Source, dict[str, Any]]:
    """The source that is furthest ahead, among those that answer and are on this observer's chain."""
    best: tuple[Source, dict[str, Any]] | None = None
    errors = []
    for source in sources:
        try:
            status = source.call("status")
        except (OSError, ValueError, ObserverError) as exc:
            errors.append(f"{source.spec}: {exc}")
            continue
        if chain_id is not None and status.get("chain_id") != chain_id:
            errors.append(f"{source.spec}: on chain {status.get('chain_id')}")
            continue
        if best is None or status["block_height"] > best[1]["block_height"]:
            best = (source, status)
    if best is None:
        raise ObserverError("no source answers for this chain: " + "; ".join(errors))
    return best


def fetch_blocks(source: Source, from_height: int, to_height: int) -> list[dict[str, Any]]:
    blocks: list[dict[str, Any]] = []
    height = from_height
    while height <= to_height:
        page = source.call("blocks", {"from_height": height, "limit": min(BLOCKS_PAGE, to_height - height + 1)})
        if not page:
            raise ObserverError(f"{source.spec}: no block at height {height} below the reported tip {to_height}")
        for block in page:
            header = block["header"]
            if header["height"] != height:
                raise ObserverError(f"{source.spec}: asked for block {height}, got {header['height']}")
            blocks.append(header)
            height += 1
    return blocks


def block_committee(header: dict[str, Any]) -> list[str] | None:
    """The validators the block's own certificate was drawn from, when the header names them."""
    commit = header.get("consensus_v2_commit") or {}
    for holder in (commit.get("precommit_qc"), commit.get("prepare_qc"), header.get("certificate")):
        members = (holder or {}).get("validators")
        if isinstance(members, list) and members and all(isinstance(m, str) for m in members):
            return sorted(set(members))
    return None


def certificate_voters(header: dict[str, Any]) -> set[str]:
    commit = header.get("consensus_v2_commit") or {}
    voters: set[str] = set()
    for certificate in ("precommit_qc", "prepare_qc"):
        for vote in (commit.get(certificate) or {}).get("votes", []):
            if isinstance(vote, dict) and isinstance(vote.get("validator"), str):
                voters.add(vote["validator"])
    return voters


# --------------------------------------------------------------------------- state


def load_state(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    state = json.loads(path.read_text(encoding="utf-8"))
    if state.get("schema") != STATE_SCHEMA:
        raise ObserverError(f"{path} is not an observer state file")
    return state


def atomic_write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, tmp = tempfile.mkstemp(dir=path.parent, prefix=path.name + ".", suffix=".tmp")
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as out:
            json.dump(payload, out, sort_keys=True, separators=(",", ":"))
            out.write("\n")
            out.flush()
            os.fsync(out.fileno())
        os.replace(tmp, path)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise


def day_entry(state: dict[str, Any], date: dt.date) -> dict[str, Any]:
    """Every key of the state is a string; a JSON round trip changes nothing."""
    return state["days"].setdefault(date.isoformat(), {"complete": True, "blocks": {}, "counts": None})


def mark_incomplete(state: dict[str, Any], first: dt.date, last: dt.date) -> None:
    day = first
    while day <= last:
        day_entry(state, day)["complete"] = False
        day += contract.DAY


# --------------------------------------------------------------------------- one read


def poll(state: dict[str, Any] | None, sources: list[Source], network: str, now: dt.datetime,
         activation: dict[str, str] | None = None) -> dict[str, Any]:
    """`activation` (validator id -> first active date) is read only when the observer first starts."""
    source, status = best_source(sources, state["chain"]["chain_id"] if state else None)
    chain = {"network": network, "chain_id": status["chain_id"], "genesis_hash": status["genesis_hash"]}
    registry_reply = source.call("validators")
    registry = {v["node_id"]: v["public_key_hex"] for v in registry_reply["validators"]}
    tip_height = status["block_height"]
    today = now.date()

    if state is None:
        # The observer starts here. Its first day is not a whole day, and no
        # block before this moment is ever dated.
        state = {
            "schema": STATE_SCHEMA,
            "chain": chain,
            "started_at": contract.format_utc(now),
            "days": {},
            "validators": {},
            # Validators that were in the registry before this observer saw
            # anything. They did not become active today.
            "bootstrap": {},
        }
        unknown = (today - contract.EVALUATION_HORIZON_DAYS * contract.DAY).isoformat()
        for vid, key in registry.items():
            first = (activation or {}).get(vid, unknown)
            if contract.parse_date(first) > today:
                raise ObserverError(f"activation date of {vid} is after today")
            state["bootstrap"][vid] = {"ml_dsa_public_key": key, "first_active_date": first}
        day_entry(state, today)["complete"] = False
    else:
        if state["chain"] != chain:
            raise ObserverError(f"the source is on another chain than this observer's state: {chain} != {state['chain']}")
        last_poll = contract.parse_utc(state["last_poll_at"])
        if now < last_poll:
            raise ObserverError("the clock went backwards since the last read")
        if tip_height < state["last_height"]:
            raise ObserverError(f"the source is behind this observer: height {tip_height} < {state['last_height']}")
        gap_too_long = (now - last_poll).total_seconds() > MAX_OBSERVATION_GAP_SECONDS
        if gap_too_long:
            mark_incomplete(state, last_poll.date(), today)
        if tip_height > state["last_height"]:
            headers = fetch_blocks(source, state["last_height"] + 1, tip_height)
            parent = state["last_block_hash"]
            for header in headers:
                if header["parent_hash"] != parent:
                    raise ObserverError(f"block {header['height']} does not follow the block this observer saw last")
                parent = header["block_hash"]
            if last_poll.date() != today:
                # The head moved across midnight: these blocks cannot be dated.
                mark_incomplete(state, last_poll.date(), today)
            elif not gap_too_long:
                day = day_entry(state, today)
                for header in headers:
                    # A block is scored against the validators of its own
                    # certificate. Several blocks of one read may fall on
                    # either side of a registry change.
                    members = block_committee(header) or sorted(registry)
                    day["blocks"][str(header["height"])] = {
                        "hash": header["block_hash"],
                        "proposer": header["proposer"],
                        "registry": members,
                        "credited": sorted(certificate_voters(header) & set(members)),
                        "record": False,
                    }
        else:
            day_entry(state, today)

    tip = fetch_blocks(source, tip_height, tip_height)[0] if tip_height >= 1 else None
    state["last_poll_at"] = contract.format_utc(now)
    state["last_height"] = tip_height
    state["last_block_hash"] = tip["block_hash"] if tip else ""
    removed = state.setdefault("removed_since", {})
    for vid in list(removed):
        if vid in registry:
            del removed[vid]
    for vid in state["validators"]:
        if vid not in registry:
            removed.setdefault(vid, today.isoformat())
    state["registry"] = registry
    state["registry_root"] = registry_reply["registry_root"]
    domain = (((tip or {}).get("consensus_v2_commit") or {}).get("precommit_qc") or {}).get("domain") or {}
    state["committee_epoch"] = domain.get("committee_epoch", state.get("committee_epoch", 0))

    read_vote_records(state, sources, today)
    close_old_days(state, today)
    return state


def read_vote_records(state: dict[str, Any], sources: list[Source], today: dt.date) -> None:
    """Add the votes the certificates left out.

    A record is unsigned. It is taken only from the source that is the block's
    proposer, so one validator can speak for the blocks it proposed and for no
    others.
    """
    wanted: dict[int, dict[str, Any]] = {}
    for date_text, day in state["days"].items():
        if (today - contract.parse_date(date_text)).days > VOTE_RECORD_RETRY_DAYS:
            continue
        for height, block in day["blocks"].items():
            if not block["record"]:
                wanted[int(height)] = block
    if not wanted:
        return
    for source in sources:
        proposer = getattr(source, "validator_id", None)
        if proposer is None:
            continue
        heights = sorted(h for h, block in wanted.items() if not block["record"] and block["proposer"] == proposer)
        index = 0
        while index < len(heights):
            start = heights[index]
            try:
                reply = source.call("block_vote_records", {"from_height": start, "limit": VOTE_RECORDS_PAGE})
            except (OSError, ValueError, KeyError, ObserverError):
                break  # unreachable, or a release without the method: certificates only for its blocks
            for row in reply.get("records", []):
                block = wanted.get(row.get("height"))
                record = row.get("record")
                if block is None or not record:
                    continue
                # The record must be about this very block, and this source must be its proposer.
                if block["proposer"] != proposer or block["record"]:
                    continue
                if record.get("block_hash") != block["hash"] or record.get("proposer") != proposer:
                    continue
                voted = {
                    entry["validator"]
                    for entry in record.get("entries", [])
                    if entry.get("outcome") in CREDITED_OUTCOMES and entry.get("validator") in block["registry"]
                }
                block["credited"] = sorted(set(block["credited"]) | voted)
                block["record"] = True
            while index < len(heights) and heights[index] < start + VOTE_RECORDS_PAGE:
                index += 1


def day_counts(day: dict[str, Any]) -> dict[str, Any]:
    if day.get("counts") is not None:
        return day["counts"]
    heights = sorted(int(h) for h in day["blocks"])
    validators: dict[str, dict[str, int]] = {}
    for block in day["blocks"].values():
        for vid in block["registry"]:
            entry = validators.setdefault(vid, {"committed": 0, "signed": 0})
            entry["committed"] += 1
            if vid in block["credited"]:
                entry["signed"] += 1
    return {
        "from_height": heights[0] if heights else None,
        "to_height": heights[-1] if heights else None,
        "validators": validators,
    }


def close_old_days(state: dict[str, Any], today: dt.date) -> None:
    """Keep per-block detail while a vote record may still arrive, then only the counts. Drop what left the window."""
    for date_text in sorted(state["days"]):
        age = (today - contract.parse_date(date_text)).days
        day = state["days"][date_text]
        if age > contract.EVALUATION_HORIZON_DAYS + 1:
            del state["days"][date_text]
        elif age > VOTE_RECORD_RETRY_DAYS and day.get("counts") is None:
            day["counts"] = day_counts(day)
            day["blocks"] = {}


# --------------------------------------------------------------------------- the attestation


def days_for(state: dict[str, Any], vid: str, first_active: dt.date, today: dt.date) -> list[dict[str, Any]]:
    window_start = today - contract.EVALUATION_HORIZON_DAYS * contract.DAY
    out, date = [], max(first_active, window_start)
    while date < today:
        day = state["days"].get(date.isoformat())
        if day is None:
            # The observer was not running: it knows nothing about this day.
            out.append({"date": date.isoformat(), "complete": False, "from_height": None, "to_height": None,
                        "committed_blocks": 0, "signed_blocks": 0, "participation_bps": None})
        else:
            counts = day_counts(day)
            mine = counts["validators"].get(vid, {"committed": 0, "signed": 0})
            complete = bool(day["complete"])
            committed, signed = mine["committed"], mine["signed"]
            out.append({
                "date": date.isoformat(),
                "complete": complete,
                "from_height": counts["from_height"],
                "to_height": counts["to_height"],
                "committed_blocks": committed,
                "signed_blocks": signed,
                "participation_bps": signed * 10000 // committed if complete and committed else None,
            })
        date += contract.DAY
    return out


FACT_KEYS = ("validator_id", "ml_dsa_public_key", "registry_active", "access_state", "first_active_date",
             "grace_started_at", "recovery_floor_date", "days")


def build_rows(state: dict[str, Any], now: dt.datetime) -> list[dict[str, Any]]:
    today = now.date()
    rows = []
    for vid in sorted(set(state["registry"]) | set(state["validators"])):
        previous = state["validators"].get(vid)
        if previous and vid not in state["registry"] and previous["access_state"] == "revoked":
            since = state.get("removed_since", {}).get(vid)
            if since and (today - contract.parse_date(since)).days > contract.EVALUATION_HORIZON_DAYS:
                # Out of the registry for longer than the window and revoked:
                # there is nothing left to say about it.
                del state["validators"][vid]
                state["removed_since"].pop(vid, None)
                continue
        key = state["registry"].get(vid) or previous["ml_dsa_public_key"]
        if previous and previous["ml_dsa_public_key"] != key:
            # A validator id under a new key is a new validator.
            previous = None
        known = state.get("bootstrap", {}).get(vid)
        if previous is None and known and known["ml_dsa_public_key"] == key:
            # In the registry before this observer started: active since its
            # activation day, not since today.
            previous = {"ml_dsa_public_key": key, "access_state": "active", "first_active_date": known["first_active_date"],
                        "grace_started_at": None, "recovery_floor_date": None}
        first_text = previous["first_active_date"] if previous else None
        first = contract.parse_date(first_text) if first_text else today
        row = contract.next_row(
            previous,
            validator_id=vid,
            ml_dsa_public_key=key,
            registry_active=vid in state["registry"],
            days=days_for(state, vid, first, today) if first_text else [],
            window_end=today,
            observed_at=now,
        )
        window_start = today - contract.EVALUATION_HORIZON_DAYS * contract.DAY
        contract.check_row(row, window_start, today)
        rows.append(row)
        state["validators"][vid] = {k: row[k] for k in FACT_KEYS if k != "days"}
    return rows


def attest(state: dict[str, Any], rows: list[dict[str, Any]], keyset_id: str, key_file: Path, node_bin: str,
           now: dt.datetime) -> dict[str, Any]:
    today = now.date()
    window_spec = {
        **WINDOW_SPEC,
        "window_start": contract.format_utc(contract.midnight(today - contract.EVALUATION_HORIZON_DAYS * contract.DAY)),
        "window_end": contract.format_utc(contract.midnight(today)),
    }
    quorum_policy = {"observer_keyset_id": keyset_id, **QUORUM_POLICY}
    unsigned = {
        "chain": state["chain"],
        "quorum_policy": quorum_policy,
        "window_spec": window_spec,
        "observed_at": contract.format_utc(now),
        "committee_epoch": state["committee_epoch"],
        "finalized_height": state["last_height"],
        "finalized_block_hash": state["last_block_hash"],
        "registry_root": state["registry_root"],
        "validator_facts": [{k: row[k] for k in FACT_KEYS} for row in rows],
    }
    with tempfile.TemporaryDirectory() as tmp:
        input_file = Path(tmp) / "unsigned.json"
        input_file.write_text(json.dumps(unsigned), encoding="utf-8")
        signed = subprocess.run(
            [node_bin, "observer-uptime-sign", "--key-file", str(key_file), "--input-file", str(input_file)],
            capture_output=True, text=True, check=False,
        )
    if signed.returncode != 0:
        raise ObserverError(f"signing failed: {signed.stderr.strip() or signed.stdout.strip()}")
    attestation = json.loads(signed.stdout)
    # Two implementations of the contract must give the same rows.
    if attestation["validator_claims"] != rows:
        raise ObserverError("the node derived other rows than this observer; nothing is published")
    return {"chain": state["chain"], "quorum_policy": quorum_policy, "window_spec": window_spec, "attestation": attestation}


def refuse_validator_data_dir(path: Path) -> None:
    for parent in [path.resolve(), *path.resolve().parents]:
        if (parent / "validator_keys.json").exists() or (parent / "consensus-v2-safety").exists():
            raise ObserverError(f"`{path}` is inside a validator data directory; an observer writes nothing there")


# --------------------------------------------------------------------------- commands


def parse_now(raw: str | None) -> dt.datetime:
    now = contract.parse_utc(raw) if raw else dt.datetime.now(contract.UTC)
    return now.replace(microsecond=0)


def cmd_run(args: argparse.Namespace) -> int:
    state_file, output = Path(args.state_file), Path(args.output)
    for path in (state_file, output):
        refuse_validator_data_dir(path.parent)
    now = parse_now(args.now)
    activation = None
    if args.activation_file:
        activation = json.loads(Path(args.activation_file).read_text(encoding="utf-8"))
        if not isinstance(activation, dict) or not all(isinstance(v, str) for v in activation.values()):
            raise ObserverError("--activation-file is a JSON object of validator id to YYYY-MM-DD")
    state = poll(load_state(state_file), [Source(spec) for spec in args.source], args.network, now, activation)
    rows = build_rows(state, now)
    if rows:
        envelope = attest(state, rows, args.keyset_id, Path(args.key_file), args.node_bin, now)
        atomic_write_json(output, envelope)
    # The state is written last: a run that failed above is repeated from the same state.
    atomic_write_json(state_file, state)
    print(json.dumps({
        "observed_at": contract.format_utc(now),
        "height": state["last_height"],
        "validators": {row["validator_id"]: [row["access_state"], row["access_state_reason"]] for row in rows},
    }))
    return 0


def cmd_bundle(args: argparse.Namespace) -> int:
    with tempfile.TemporaryDirectory() as tmp:
        command = [args.node_bin, "observer-uptime-bundle", "--keyset-file", args.keyset_file, "--output-file", args.output]
        if args.now:
            command += ["--now-unix", str(int(parse_now(args.now).timestamp()))]
        for index, origin in enumerate(args.attestation):
            if origin.startswith("https://"):
                local = Path(tmp) / f"attestation-{index}.json"
                try:
                    with urllib.request.urlopen(origin, timeout=20) as response:  # noqa: S310 (https only)
                        local.write_bytes(response.read(MAX_REPLY_BYTES))
                except OSError as exc:
                    print(f"{origin}: {exc}", file=sys.stderr)
                    continue
                origin = str(local)
            command += ["--attestation-file", origin]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    return result.returncode


def cmd_verify(args: argparse.Namespace) -> int:
    command = [args.node_bin, "observer-uptime-verify", "--feed-file", args.feed_file, "--keyset-file", args.keyset_file]
    if args.now:
        command += ["--now-unix", str(int(parse_now(args.now).timestamp()))]
    if args.last_accepted_file:
        command += ["--last-accepted-file", args.last_accepted_file]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    return result.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="read the chain once and write one signed attestation")
    run.add_argument("--source", action="append", required=True,
                     help="read-only RPC: tcp://host:port or https://host/rpc. Write validator-id=tcp://host:port for a validator's own RPC: only then do its vote records count, and only for the blocks it proposed. Repeat for every validator; the source furthest ahead gives the blocks")
    run.add_argument("--activation-file",
                     help="JSON object, validator id to first active date (YYYY-MM-DD), for validators already in the registry. Read only on the observer's first run")
    run.add_argument("--network", required=True, choices=["l1-devnet", "l1-testnet", "l1-mainnet"])
    run.add_argument("--keyset-id", required=True, help="keyset_id of the pinned observer keyset this observer is in")
    run.add_argument("--key-file", required=True, help="this observer's key file (observer-key-create)")
    run.add_argument("--state-file", required=True)
    run.add_argument("--output", required=True, help="where the signed attestation is written")
    run.add_argument("--node-bin", default="postfiat-node")
    run.add_argument("--now", help=argparse.SUPPRESS)  # tests
    run.set_defaults(handler=cmd_run)

    bundle = sub.add_parser("bundle", help="put attestations into one feed")
    bundle.add_argument("--attestation", action="append", required=True, help="attestation file or https URL; repeat")
    bundle.add_argument("--keyset-file", required=True)
    bundle.add_argument("--output", required=True)
    bundle.add_argument("--node-bin", default="postfiat-node")
    bundle.add_argument("--now", help=argparse.SUPPRESS)
    bundle.set_defaults(handler=cmd_bundle)

    verify = sub.add_parser("verify", help="read a feed the way a consumer must")
    verify.add_argument("--feed-file", required=True)
    verify.add_argument("--keyset-file", required=True)
    verify.add_argument("--last-accepted-file")
    verify.add_argument("--node-bin", default="postfiat-node")
    verify.add_argument("--now", help=argparse.SUPPRESS)
    verify.set_defaults(handler=cmd_verify)

    args = parser.parse_args(argv)
    try:
        return args.handler(args)
    except ObserverError as exc:
        print(f"validator-uptime-observer: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
