"""Watch a genesis external lane: match each release on PFTL against its Base Lock.

Design ``docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`` section
5.2: every ``external_lane_release_v1`` names its Lock nonce, and the watcher
raises an alarm for a release whose nonce has no Lock, whose amount differs
from the Lock's, or whose recipient hash differs from the Lock's
``pftlRecipientHash``. It pages the lane's release records from a PFTL node
(``external_lane_info``), reads the lane policy from the same read, and checks
each release on Base through the lock contract's ``lockRecord(nonce)`` view
(``crates/pft-token-contracts/src/PFTLockV1.sol``) at the release's own source
block, once that block is final and its hash is the one the release names; a
Lock made later cannot back an earlier release.

It only reads, on both chains: no key, no signature, no transaction.

    PYTHONPATH=python python -m postfiat_rpc.external_lane_watcher \\
        --pftl-rpc 127.0.0.1:27650 --lane-id base-erc20 \\
        --base-rpc http://127.0.0.1:8545 --state-file watcher-state.json --once

Output is JSON lines on stdout: one ``release`` line per release checked, one
``alarm`` line per alarm, and a ``summary`` line per run. A one-shot run exits
0 when clean, 2 on any alarm and 1 on an input or RPC error (one ``error`` line
on stderr). With ``--interval`` it runs until stopped: it keeps running after
an alarm, so the alert signal is the ``alarm`` line itself, and it retries
after an RPC error or a failed state write.

The state file is written before a run's lines are printed. Without it (lost
or deleted), the next run starts from height 0 and re-checks every stored
release; a state file of an earlier schema is refused (delete it, or pass
``--from-height``, which does not read the file and replaces it after a clean
run). The Base node must serve each release's source block: a
node that has pruned it fails the run, so run the watcher at least once per
release window or point it at an archive node.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path
from typing import Any, Callable, Sequence

from . import pft_token
from .client import PostFiatRpcClient, RpcClientError

STATE_SCHEMA = "postfiat.external_lane_watcher.state.v2"
LINE_SCHEMA = "postfiat.external_lane_watcher.v1"
PAGE_SIZE = 100
MAX_STATE_BYTES = 64 * 1024
U64 = 1 << 64
ZERO_ADDRESS = "0x" + "00" * 20
ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
HASH32 = re.compile(r"^0x[0-9a-fA-F]{64}$")
GENESIS_HASH = re.compile(r"^[0-9a-f]{96}$")
HISTORY_ROOT = re.compile(r"^0x[0-9a-f]{96}$")
U32 = 1 << 32

ALARM_CODES = (
    "release_without_lock",
    "release_amount_mismatch",
    "release_recipient_mismatch",
    "release_block_not_finalized",
    "release_source_block_mismatch",
    "lock_code_hash_mismatch",
    "release_pruned_before_check",
)


# --- Keccak-256 (Ethereum's, padding 0x01): not in the standard library. ---

_ROUND_CONSTANTS = (
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
)
_ROTATIONS = (
    (0, 36, 3, 41, 18), (1, 44, 10, 45, 2), (62, 6, 43, 15, 61), (28, 55, 25, 21, 56), (27, 20, 39, 8, 14),
)
_MASK = (1 << 64) - 1


def _keccak_f(lanes: list[list[int]]) -> None:
    for constant in _ROUND_CONSTANTS:
        columns = [lanes[x][0] ^ lanes[x][1] ^ lanes[x][2] ^ lanes[x][3] ^ lanes[x][4] for x in range(5)]
        for x in range(5):
            mix = columns[(x - 1) % 5] ^ (((columns[(x + 1) % 5] << 1) | (columns[(x + 1) % 5] >> 63)) & _MASK)
            for y in range(5):
                lanes[x][y] ^= mix
        moved = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                shift = _ROTATIONS[x][y]
                value = lanes[x][y]
                moved[y][(2 * x + 3 * y) % 5] = ((value << shift) | (value >> (64 - shift))) & _MASK if shift else value
        for x in range(5):
            for y in range(5):
                lanes[x][y] = moved[x][y] ^ ((~moved[(x + 1) % 5][y]) & moved[(x + 2) % 5][y])
        lanes[0][0] ^= constant


def keccak256(data: bytes) -> bytes:
    """Ethereum's Keccak-256 of `data`."""

    rate = 136
    padding = bytearray(rate - len(data) % rate)
    padding[0] |= 0x01
    padding[-1] |= 0x80
    padded = bytes(data) + padding
    lanes = [[0] * 5 for _ in range(5)]
    for offset in range(0, len(padded), rate):
        block = padded[offset : offset + rate]
        for index in range(rate // 8):
            lanes[index % 5][index // 5] ^= int.from_bytes(block[8 * index : 8 * index + 8], "little")
        _keccak_f(lanes)
    return b"".join(lanes[index % 5][index // 5].to_bytes(8, "little") for index in range(4))


LOCK_RECORD_SELECTOR = keccak256(b"lockRecord(uint64)")[:4].hex()


# --- Errors and validation. ---


class WatcherError(ValueError):
    """An input error or a malformed answer: the run stops (exit 1)."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class WatcherTransientError(WatcherError):
    """A run that cannot finish now but may on retry: exit 1, or retried with
    ``--interval``."""


class WatcherRpcError(WatcherTransientError):
    """An RPC request that failed: exit 1, or retried with ``--interval``."""


def _u64(value: Any, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value < U64:
        raise WatcherError("release_record_malformed", f"{field} is not a u64")
    return value


def _lane_policy(row: dict[str, Any]) -> dict[str, Any]:
    policy = (row.get("config") or {}).get("policy") if isinstance(row.get("config"), dict) else None
    if not isinstance(policy, dict):
        raise WatcherError("external_lane_policy_malformed", "the lane row has no config.policy")
    chain_id = policy.get("chain_id")
    lock = policy.get("lock_contract_address")
    code_hash = policy.get("lock_runtime_code_hash")
    if not isinstance(chain_id, int) or isinstance(chain_id, bool) or chain_id <= 0:
        raise WatcherError("external_lane_policy_malformed", "policy.chain_id is not a positive integer")
    if not isinstance(lock, str) or not ADDRESS.match(lock):
        raise WatcherError("external_lane_policy_malformed", "policy.lock_contract_address is not an address")
    if not isinstance(code_hash, str) or not HASH32.match(code_hash):
        raise WatcherError("external_lane_policy_malformed", "policy.lock_runtime_code_hash is not 32 bytes")
    return {"chain_id": chain_id, "lock": lock.lower(), "code_hash": code_hash.lower()}


def _release(entry: Any, policy: dict[str, Any]) -> dict[str, Any]:
    """One paged release record, validated and reduced to what the check uses."""

    if not isinstance(entry, dict) or not isinstance(entry.get("record"), dict):
        raise WatcherError("release_record_malformed", "a release entry has no record")
    record = entry["record"]
    recipient = record.get("recipient")
    if not isinstance(recipient, str) or not recipient:
        raise WatcherError("release_record_malformed", "recipient is not text")
    lock = record.get("lock_contract_address")
    if not isinstance(lock, str) or lock.lower() != policy["lock"]:
        raise WatcherError("release_record_malformed", "the record names another lock contract")
    if record.get("source_chain_id") != policy["chain_id"]:
        raise WatcherError("release_record_malformed", "the record names another source chain")
    amount = _u64(record.get("amount_atoms"), "amount_atoms")
    if amount == 0:
        # Committed records refuse a zero amount and the lock makes no zero Lock.
        raise WatcherError("release_record_malformed", "amount_atoms is zero")
    source_hash = record.get("source_block_hash")
    if not isinstance(source_hash, str) or not HASH32.match(source_hash):
        raise WatcherError("release_record_malformed", "source_block_hash is not 32 bytes")
    return {
        "sequence": _u64(entry.get("sequence"), "sequence"),
        "lock_nonce": _u64(record.get("lock_nonce"), "lock_nonce"),
        "amount_atoms": amount,
        "recipient": recipient,
        "source_block_number": _u64(record.get("source_block_number"), "source_block_number"),
        "source_block_hash": source_hash.lower(),
        "released_at_height": _u64(record.get("released_at_height"), "released_at_height"),
    }


# --- PFTL and Base reads. ---


def _lane_page(pftl: Any, lane_id: str, cursor: int | None) -> tuple[dict[str, Any], dict[str, Any]]:
    """One page of the lane read and the snapshot it was served from: the
    chain identity, the height and the lane's release counters and root."""

    try:
        info = pftl.external_lane_info(lane_id, release_cursor=cursor, release_limit=PAGE_SIZE)
    except RpcClientError as error:
        raise WatcherRpcError("pftl_rpc_failed", f"external_lane_info: {error}") from None
    rows = [row for row in info.get("lanes") or [] if isinstance(row, dict) and row.get("lane_id") == lane_id]
    if len(rows) != 1:
        raise WatcherError("external_lane_not_reported", f"the PFTL node reports no lane `{lane_id}`")
    row = rows[0]
    height = info.get("current_height")
    if not isinstance(height, int) or isinstance(height, bool) or not 0 <= height < U64:
        raise WatcherError("external_lane_info_malformed", "external_lane_info has no current_height")
    chain_id, genesis = info.get("chain_id"), info.get("genesis_hash")
    if not isinstance(chain_id, str) or not chain_id or not isinstance(genesis, str) or not GENESIS_HASH.match(genesis):
        raise WatcherError("external_lane_info_malformed", "external_lane_info has no chain id or genesis hash")
    version = info.get("protocol_version")
    if not isinstance(version, int) or isinstance(version, bool) or not 0 < version < U32:
        raise WatcherError("external_lane_info_malformed", "protocol_version is not a positive integer")
    root = row.get("release_history_root")
    if root is not None and (not isinstance(root, str) or not HISTORY_ROOT.match(root)):
        raise WatcherError("external_lane_info_malformed",
                           "release_history_root is not 0x and 96 lower-case hex digits")
    try:
        count = _u64(row.get("release_count"), "release_count")
        pruned = _u64(row.get("pruned_release_count", 0), "pruned_release_count")
    except WatcherError as error:
        raise WatcherError("external_lane_info_malformed", str(error)) from None
    if pruned > count:
        raise WatcherError("external_lane_info_malformed", "more releases pruned than released")
    # The root chains every release, so it exists exactly when one does; a
    # lane with releases and no root would leave paging unbound by it.
    if (root is None) != (count == 0):
        raise WatcherError("external_lane_info_malformed",
                           "release_history_root must be present exactly when release_count is positive")
    snapshot = {
        "chain_id": chain_id,
        "genesis_hash": genesis,
        "protocol_version": version,
        "current_height": height,
        "release_count": count,
        "pruned_release_count": pruned,
        "release_history_root": root,
    }
    return row, snapshot


def _releases_from(pftl: Any, lane_id: str, first_row: dict[str, Any], first: dict[str, Any],
                   start_height: int, policy: dict[str, Any]) -> tuple[list[dict[str, Any]], bool]:
    """Every stored release at or above `start_height`, oldest first, and
    whether the stored records reach below `start_height`.

    Every page must come from the first page's snapshot, and the sequences
    must run contiguously from `release_count - 1` down to the pruned count.
    """

    releases: list[dict[str, Any]] = []
    expected = first["release_count"] - 1  # the next sequence, newest first
    row = first_row
    while True:
        page = [_release(entry, policy) for entry in row.get("releases") or []]
        for entry in page:
            if entry["sequence"] != expected:
                raise WatcherError("external_lane_info_malformed",
                                   f"release sequence {entry['sequence']} where {expected} was due")
            expected -= 1
        releases.extend(page)
        if page and page[-1]["released_at_height"] < start_height:
            reaches_below = True
            break
        cursor = row.get("next_release_cursor")
        if cursor is None or not page:
            if expected != first["pruned_release_count"] - 1:
                raise WatcherError("external_lane_info_malformed",
                                   f"the stored releases end at {expected + 1}, not at the pruned count "
                                   f"{first['pruned_release_count']}")
            reaches_below = False
            break
        if _u64(cursor, "next_release_cursor") != page[-1]["sequence"]:
            raise WatcherError("external_lane_info_malformed", "next_release_cursor is not the page's oldest sequence")
        row, snapshot = _lane_page(pftl, lane_id, cursor)
        if snapshot != first:
            changed = sorted(key for key in first if first[key] != snapshot.get(key))
            raise WatcherTransientError("external_lane_info_inconsistent",
                                        f"the lane report changed while it was paged: {', '.join(changed)}")
    return [entry for entry in reversed(releases) if entry["released_at_height"] >= start_height], reaches_below


def _base(base: Any, method: str, params: list[Any]) -> Any:
    try:
        return base.call(method, params)
    except pft_token.PftTokenError as error:
        raise WatcherRpcError("base_rpc_failed", str(error)) from None


def _block_hash(base: Any, number: int) -> str:
    """The canonical hash of Base block `number`."""

    block = _base(base, "eth_getBlockByNumber", [hex(number), False])
    if block is None:
        raise WatcherError("base_answer_malformed",
                           f"the Base node does not serve block {number}, the release's source block; "
                           "use a node that keeps it or an archive node")
    if not isinstance(block, dict):
        raise WatcherError("base_answer_malformed", f"block {number} is not an object")
    try:
        answered = pft_token.hex_quantity(block.get("number"), f"block {number} number")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    if answered != number:
        raise WatcherError("base_answer_malformed", f"the answer for block {number} is block {answered}")
    found = block.get("hash")
    if not isinstance(found, str) or not HASH32.match(found):
        raise WatcherError("base_answer_malformed", f"block {number} has no 32-byte hash")
    return found.lower()


def _lock_record(base: Any, lock: str, nonce: int, block: str) -> tuple[str, int, str]:
    data = "0x" + LOCK_RECORD_SELECTOR + f"{nonce:064x}"
    try:
        answer = pft_token.hex_bytes(_base(base, "eth_call", [{"to": lock, "data": data}, block]), "lockRecord")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    if len(answer) != 96 or any(answer[:12]) or any(answer[32:56]):
        raise WatcherError("base_answer_malformed", f"lockRecord({nonce}) is not (address, uint64, bytes32)")
    holder = "0x" + answer[12:32].hex()
    return holder, int.from_bytes(answer[56:64], "big"), "0x" + answer[64:96].hex()


# --- One run. ---


def _load_state(path: Path | None, lane_id: str) -> dict[str, Any] | None:
    if path is None or not path.exists():
        return None
    try:
        if path.stat().st_size > MAX_STATE_BYTES:
            raise WatcherError("state_file_invalid", f"`{path}` is larger than {MAX_STATE_BYTES} bytes")
        state = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise WatcherError("state_file_invalid", f"`{path}`: {error}") from None
    def u64(value: Any) -> bool:
        return isinstance(value, int) and not isinstance(value, bool) and 0 <= value < U64

    if (
        not isinstance(state, dict)
        or state.get("schema") != STATE_SCHEMA
        or state.get("lane_id") != lane_id
        or not all(isinstance(state.get(key), str) for key in ("pftl_chain_id", "genesis_hash",
                                                              "lock_contract_address"))
        or not all(u64(state.get(key)) for key in ("source_chain_id", "checked_through_height",
                                                   "release_count"))
    ):
        raise WatcherError("state_file_invalid", f"`{path}` is not this lane's watcher state")
    return state


def _check_state(state: dict[str, Any], snapshot: dict[str, Any], policy: dict[str, Any]) -> None:
    """The kept state must be this network's and lane's, and not ahead of the
    node: a regression means another network or a corrupt file."""

    if (state["pftl_chain_id"], state["genesis_hash"], state["lock_contract_address"].lower(),
            state["source_chain_id"]) != (snapshot["chain_id"], snapshot["genesis_hash"], policy["lock"],
                                          policy["chain_id"]):
        raise WatcherError("state_file_invalid", "the state file belongs to another chain, genesis or lane policy")
    if state["checked_through_height"] > snapshot["current_height"] or state["release_count"] > snapshot["release_count"]:
        raise WatcherError("state_file_invalid",
                           "the state file is ahead of the PFTL node: another network or a corrupt file")


def _save_state(path: Path, state: dict[str, Any]) -> None:
    temporary = path.with_name(path.name + ".tmp")
    try:
        temporary.write_text(json.dumps(state, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(path)
    except OSError as error:
        if temporary.parent.is_dir():
            temporary.unlink(missing_ok=True)
        raise WatcherTransientError("state_file_write_failed", f"`{path}`: {error}") from None


def watch_once(pftl: Any, base: Any, lane_id: str, *, from_height: int | None = None,
               state: dict[str, Any] | None = None) -> tuple[list[dict[str, Any]], int, dict[str, Any] | None]:
    """One run: the output lines, the alarm count and the state to keep.

    The state advances only after a clean run, so an alarm is raised again on
    every run until an operator starts past it with `from_height`.
    """

    row, snapshot = _lane_page(pftl, lane_id, None)
    current_height = snapshot["current_height"]
    policy = _lane_policy(row)
    release_count, pruned = snapshot["release_count"], snapshot["pruned_release_count"]
    try:
        chain_id = pft_token.hex_quantity(_base(base, "eth_chainId", []), "eth_chainId")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    if chain_id != policy["chain_id"]:
        raise WatcherError("base_chain_id_mismatch",
                           f"the Base RPC is on chain {chain_id}; lane `{lane_id}` names {policy['chain_id']}")
    block = _base(base, "eth_getBlockByNumber", ["finalized", False])
    try:
        finalized = pft_token.hex_quantity((block or {}).get("number"), "finalized block number")
    except (pft_token.PftTokenRefused, AttributeError) as error:
        raise WatcherError("base_answer_malformed", f"the finalized block: {error}") from None
    finalized_tag = hex(finalized)

    lines: list[dict[str, Any]] = []

    def alarm(code: str, **fields: Any) -> None:
        if code not in ALARM_CODES:
            raise AssertionError(f"undeclared alarm code {code}")
        lines.append({"schema": LINE_SCHEMA, "kind": "alarm", "code": code, "lane_id": lane_id, **fields})

    try:
        code = pft_token.hex_bytes(_base(base, "eth_getCode", [policy["lock"], finalized_tag]), "eth_getCode")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    observed_code_hash = "0x" + keccak256(code).hex()
    if not code or observed_code_hash != policy["code_hash"]:
        alarm("lock_code_hash_mismatch", lock_contract_address=policy["lock"], finalized_block=finalized,
              expected=policy["code_hash"], observed=observed_code_hash)

    if state is not None:
        _check_state(state, snapshot, policy)
    if from_height is not None:
        start_height, checked_count = from_height, 0
    elif state is not None:
        start_height, checked_count = state["checked_through_height"] + 1, state["release_count"]
    else:
        start_height, checked_count = 0, 0
    releases, reaches_below = _releases_from(pftl, lane_id, row, snapshot, start_height, policy)
    releases = [entry for entry in releases if entry["released_at_height"] <= current_height]
    # Pruned records are no longer paged: releases pruned before a run
    # checked them cannot be matched against their Locks. With a state file
    # they are the ones past its count; without one, every pruned release
    # may lie in the range unless the stored records already reach below it.
    if state is not None and from_height is None:
        unseen = pruned - checked_count
    else:
        unseen = 0 if reaches_below else pruned
    if unseen > 0:
        alarm("release_pruned_before_check", count=unseen, pruned_release_count=pruned)

    for entry in releases:
        result = {"schema": LINE_SCHEMA, "kind": "release", "lane_id": lane_id, **entry}
        codes: list[str] = []
        holder = None
        canonical_hash = None
        if entry["source_block_number"] > finalized:
            # Not final yet: re-checked by a later run, not judged now.
            codes.append("release_block_not_finalized")
        elif (canonical_hash := _block_hash(base, entry["source_block_number"])) != entry["source_block_hash"]:
            codes.append("release_source_block_mismatch")
            result["canonical_block_hash"] = canonical_hash
        else:
            # The Lock as it stood at the release's own source block: a Lock
            # made after it cannot back it.
            holder, amount, recipient_hash = _lock_record(
                base, policy["lock"], entry["lock_nonce"], hex(entry["source_block_number"]))
            expected_hash = "0x" + keccak256(entry["recipient"].encode("utf-8")).hex()
            if holder == ZERO_ADDRESS:
                codes.append("release_without_lock")
            else:
                if amount != entry["amount_atoms"]:
                    codes.append("release_amount_mismatch")
                    result["lock_amount_atoms"] = amount
                if recipient_hash != expected_hash:
                    codes.append("release_recipient_mismatch")
                    result["lock_recipient_hash"] = recipient_hash
            result["recipient_hash"] = expected_hash
        result["holder"] = holder
        result["status"] = "alarm" if codes else "backed"
        lines.append(result)
        for code_ in codes:
            extra = {"canonical_block_hash": canonical_hash} if code_ == "release_source_block_mismatch" else {}
            alarm(code_, sequence=entry["sequence"], lock_nonce=entry["lock_nonce"],
                  amount_atoms=entry["amount_atoms"], recipient=entry["recipient"],
                  released_at_height=entry["released_at_height"],
                  source_block_number=entry["source_block_number"],
                  source_block_hash=entry["source_block_hash"], finalized_block=finalized, **extra)

    alarms = sum(1 for line in lines if line["kind"] == "alarm")
    lines.append({
        "schema": LINE_SCHEMA,
        "kind": "summary",
        "lane_id": lane_id,
        "chain_id": chain_id,
        "lock_contract_address": policy["lock"],
        "lock_code_hash": observed_code_hash,
        "finalized_block": finalized,
        "current_height": current_height,
        "checked_from_height": start_height,
        "checked_through_height": current_height,
        "release_count": release_count,
        "pruned_release_count": pruned,
        "releases_checked": len(releases),
        "alarms": alarms,
        "alarm_codes": {code_: sum(1 for line in lines if line["kind"] == "alarm" and line["code"] == code_)
                        for code_ in ALARM_CODES},
    })
    new_state = None if alarms else {
        "schema": STATE_SCHEMA,
        "lane_id": lane_id,
        "pftl_chain_id": snapshot["chain_id"],
        "genesis_hash": snapshot["genesis_hash"],
        "lock_contract_address": policy["lock"],
        "source_chain_id": policy["chain_id"],
        "checked_through_height": current_height,
        "release_count": release_count,
    }
    return lines, alarms, new_state


# --- Command line. ---


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.external_lane_watcher",
        description="Match each release of a genesis external lane against its Lock on Base (read only).",
    )
    parser.add_argument("--pftl-rpc", required=True, metavar="HOST:PORT", help="a PFTL node's RPC endpoint")
    parser.add_argument("--lane-id", required=True, help="the genesis external lane, for example base-erc20")
    parser.add_argument("--base-rpc", required=True, metavar="URL", help="a Base JSON-RPC URL (http or https)")
    parser.add_argument("--from-height", type=int, metavar="N",
                        help="check releases at or above this PFTL height (overrides the state file)")
    parser.add_argument("--state-file", metavar="PATH",
                        help="keeps the last checked height between runs; without it a run re-checks every "
                             "stored release")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--once", action="store_true",
                      help="one run, then exit 0 (clean), 2 (alarm) or 1 (error); the default")
    mode.add_argument("--interval", type=float, metavar="SECONDS",
                      help="run every SECONDS until stopped; an alarm is reported as an alarm line and the "
                           "loop continues")
    parser.add_argument("--timeout", type=float, default=pft_token.TIMEOUT_SECONDS, help="seconds per RPC request")
    return parser


def _error_line(error: WatcherError) -> None:
    print(json.dumps({"schema": LINE_SCHEMA, "kind": "error", "code": error.code, "message": str(error)},
                     sort_keys=True), file=sys.stderr)


def main(argv: Sequence[str] | None = None, *, pftl: Any = None, base: Any = None,
         sleep: Callable[[float], None] = time.sleep, max_runs: int | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.interval is not None and args.interval <= 0:
        parser.error("--interval must be positive")
    if args.from_height is not None and args.from_height < 0:
        parser.error("--from-height must not be negative")
    try:
        if base is None:
            base = pft_token.JsonRpc(args.base_rpc, args.timeout)
        if pftl is None:
            pftl = PostFiatRpcClient(args.pftl_rpc, timeout_seconds=args.timeout)
    except (pft_token.PftTokenError, ValueError) as error:
        _error_line(WatcherError("input_invalid", str(error)))
        return 1
    state_path = Path(args.state_file).expanduser() if args.state_file else None
    from_height = args.from_height
    memory_state: dict[str, Any] | None = None  # the state between runs without a state file
    runs = 0
    while True:
        runs += 1
        try:
            # --from-height overrides the kept state, so the file is not read:
            # a clean run replaces it, whatever its schema.
            if from_height is not None:
                state = None
            elif state_path is not None:
                state = _load_state(state_path, args.lane_id)
            else:
                state = memory_state
            lines, alarms, new_state = watch_once(pftl, base, args.lane_id, from_height=from_height, state=state)
            # Progress is saved before the run's lines are emitted, so a clean
            # summary always means the state advanced.
            if new_state is not None and state_path is not None:
                _save_state(state_path, new_state)
        except WatcherTransientError as error:
            _error_line(error)
            if args.interval is None:
                return 1
        except WatcherError as error:
            _error_line(error)
            return 1
        else:
            for line in lines:
                print(json.dumps(line, sort_keys=True), flush=True)
            if new_state is not None:
                from_height = None  # a clean run moves the start to the kept height
                memory_state = new_state
            if args.interval is None:
                return 2 if alarms else 0
        if max_runs is not None and runs >= max_runs:
            return 0
        sleep(args.interval)


if __name__ == "__main__":
    raise SystemExit(main())
