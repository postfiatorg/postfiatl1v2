"""Watch a genesis external lane in both directions, reading only.

The release view (design ``docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md``,
section 5.2): every ``external_lane_release_v1`` names its Lock nonce, and the
watcher raises an alarm for a release whose nonce has no Lock, whose amount
differs from the Lock's, or whose recipient hash differs from the Lock's
``pftlRecipientHash``. It pages the lane's release records from a PFTL node
(``external_lane_info``) and checks each on Base through the lock's
``lockRecord(nonce)`` view (``crates/pft-token-contracts/src/PFTLockV1.sol``)
at the release's own source block, once that block is final and its hash is
the one the release names; a Lock made later cannot back an earlier release.

The export view (``--exports``): every ``Released`` payout of the lock in
finalized Base blocks must match a stored PFTL export packet (digest,
recipient, amount, withdrawal-id and burn commitments); every final export is
``paid`` or ``unpaid``; and the lock's and the lane's totals must agree at the
finalized block. A payout whose packet is not stored and that no earlier run
verified is an alarm; ``--accept-export-baseline`` is the recovery path past
payouts pruned before the view checked them, taken once per growth of the
lane's pruned count and atoms.

It only reads, on both chains: no key, no signature, no transaction.

    PYTHONPATH=python python -m postfiat_rpc.external_lane_watcher \\
        --pftl-rpc 127.0.0.1:27650 --lane-id base-erc20 \\
        --base-rpc http://127.0.0.1:8545 --state-file watcher-state.json --exports --once

Output is JSON lines on stdout: ``release``, ``payout``, ``export``,
``baseline_payout``, ``warning`` and ``alarm`` lines, and one ``summary`` line
per run. A one-shot run exits 0 when clean, 2 on any alarm, 3 when it accepted
an export baseline, and 1 on an input or RPC error (an ``error`` line on
stderr). With ``--interval`` it runs until stopped: it keeps running after an
alarm, so the alert signal is the ``alarm`` line itself, and it retries after
an RPC error, a malformed answer or a failed state write.

The state file keeps each view's position: the release view's checked
height and count, and the export view's scanned-through finalized Base block
with its payout count and sum and the record of paid exports (see
``export_view``). It is trusted local data, and the RPCs' answers are taken
as honest: the watcher compares two chains' reports, it does not prove them.
Each part advances only after a clean run of its view, and the file is
written before the run's summary is printed. Without it (lost or deleted),
the next run starts from height 0 and Base block ``--lock-from-block``; a
state file of an earlier schema is refused (delete it, or pass
``--from-height``, which ignores its release part and replaces it after a
clean run, but carries the export view's known payouts). An exports entry of
an earlier format is never used (see ``EXPORT_STATE_FORMAT``), and only
``--discard-export-history`` forgets the payouts a current one knows. Run the watcher at least once per release window: release records and
export packets are pruned after their retention, and the Base node must
serve each release's source block (or use an archive node).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Sequence

from . import pft_token
from .client import PostFiatRpcClient, RpcClientError

STATE_SCHEMA = "postfiat.external_lane_watcher.state.v2"
LINE_SCHEMA = "postfiat.external_lane_watcher.v1"
PAGE_SIZE = 100
MAX_STATE_BYTES = 16 * 1024 * 1024  # checked on load and before each write; ~355 bytes per retained paid export
U64 = 1 << 64
ZERO_ADDRESS = "0x" + "00" * 20
ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
HASH32 = re.compile(r"^0x[0-9a-fA-F]{64}$")
GENESIS_HASH = re.compile(r"^[0-9a-f]{96}$")
HISTORY_ROOT = re.compile(r"^0x[0-9a-f]{96}$")
U32 = 1 << 32
HEX48 = re.compile(r"^0x[0-9a-fA-F]{96}$")
LOG_CHUNK_BLOCKS = 10_000
MIN_LOG_CHUNK_BLOCKS = 16

ALARM_CODES = (
    "release_without_lock",
    "release_amount_mismatch",
    "release_recipient_mismatch",
    "release_block_not_finalized",
    "release_source_block_mismatch",
    "lock_code_hash_mismatch",
    "release_pruned_before_check",
    # The export view (--exports): Base payouts against PFTL export packets.
    "payout_without_export",
    "payout_amount_mismatch",
    "payout_recipient_mismatch",
    "payout_withdrawal_mismatch",
    "payout_burn_mismatch",
    "payout_duplicate",
    "payout_pruned_before_check",
    "payout_unknown_above_pruned",
    "total_released_mismatch",
    "lock_balance_mismatch",
    "exported_below_paid",
    "released_above_locked",
    "base_released_above_locked",
)
WARNING_CODES = ("export_unpaid_too_long",)
# Refusals of a Base answer not bound to the run's anchor block: exit 1, no
# payout or export line, the state file unchanged.
ANCHOR_CODES = ("base_anchor_unknown", "base_anchor_mismatch")


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
RELEASED_TOPIC = "0x" + keccak256(b"Released(bytes32,bytes32,bytes32,address,uint256)").hex()
EXPORT_EVM_DIGEST_DOMAIN = b"postfiat.external_lane.export_evm_digest.v1"


def export_evm_digest(packet_digest: str) -> str:
    """`external_lane_export_evm_digest`: the `packetDigest` the lock emits,
    Keccak-256 over the domain tag, a zero byte and the 48-byte packet digest."""

    if not isinstance(packet_digest, str) or not HEX48.match(packet_digest):
        raise WatcherError("export_record_malformed", "packet_digest is not 48 bytes of hex")
    raw = bytes.fromhex(packet_digest[2:])
    if len(raw) != 48:
        raise WatcherError("export_record_malformed", "packet_digest is not 48 bytes")
    return "0x" + keccak256(EXPORT_EVM_DIGEST_DOMAIN + b"\x00" + raw).hex()


def commitment_of(hex48: str) -> str:
    """keccak256 of a 48-byte PFTL commitment: what the verifier and the lock record."""

    if not isinstance(hex48, str) or not HEX48.match(hex48):
        raise WatcherError("export_record_malformed", "a commitment is not 48 bytes of hex")
    raw = bytes.fromhex(hex48[2:])
    if len(raw) != 48:
        raise WatcherError("export_record_malformed", "a commitment is not 48 bytes")
    return "0x" + keccak256(raw).hex()


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


class BaseAnchor:
    """The run's anchor on Base: the finalized block (number, hash and parent
    hash) read once after the chain id check. Every state read names it, or a
    block it binds, by hash (EIP-1898), and every log's block is bound to it
    by hash; `blocks` caches the blocks read by hash in this run."""

    def __init__(self, number: int, block_hash: str, parent_hash: str) -> None:
        self.number, self.hash, self.parent_hash = number, block_hash, parent_hash
        self.blocks: dict[str, int] = {block_hash: number}  # hash -> number, read by hash
        self.numbers: dict[int, str] = {number: block_hash}  # number -> the one hash bound to it

    @property
    def param(self) -> dict[str, Any]:
        return at_block(self.hash)


def at_block(block_hash: str) -> dict[str, Any]:
    """The EIP-1898 block parameter: this block, and only while canonical."""

    return {"blockHash": block_hash, "requireCanonical": True}


def _anchor(base: Any) -> BaseAnchor:
    block = _base(base, "eth_getBlockByNumber", ["finalized", False])
    try:
        number = pft_token.hex_quantity((block or {}).get("number"), "finalized block number")
    except (pft_token.PftTokenRefused, AttributeError) as error:
        raise WatcherError("base_answer_malformed", f"the finalized block: {error}") from None
    hashes = [block.get(key) for key in ("hash", "parentHash")]
    if not all(isinstance(value, str) and HASH32.match(value) for value in hashes):
        raise WatcherError("base_answer_malformed", "the finalized block has no 32-byte hash and parent hash")
    return BaseAnchor(number, hashes[0].lower(), hashes[1].lower())


def _bound(base: Any, method: str, params: list[Any], what: str) -> Any:
    """A read that names its block by hash: an error answer means the node does
    not know that block (another network, a reorg) and refuses the run."""

    try:
        return _base(base, method, params)
    except WatcherRpcError as error:
        raise WatcherTransientError("base_anchor_unknown",
                                    f"{what}: the Base node does not answer at the run's anchored block ({error}): "
                                    "another network, a reorg, or a failed request") from None


def _bind_block(base: Any, anchor: BaseAnchor, block_hash: str, number: int, what: str) -> None:
    """`block_hash` must name block `number` (at or below the anchor) on the
    anchored chain: read by hash once per run, and one hash per number."""

    known = anchor.blocks.get(block_hash)
    if known is None:
        block = _bound(base, "eth_getBlockByHash", [block_hash, False], what)
        if block is None:
            raise WatcherTransientError("base_anchor_unknown",
                                        f"{what}: the Base node does not know block {block_hash}: another network "
                                        "or a reorg")
        try:
            known = pft_token.hex_quantity(block.get("number") if isinstance(block, dict) else None,
                                           f"block {block_hash} number")
        except pft_token.PftTokenRefused as error:
            raise WatcherError("base_answer_malformed", str(error)) from None
        if str(block.get("hash", "")).lower() != block_hash:
            raise WatcherTransientError("base_anchor_mismatch", f"{what}: the answer for {block_hash} is another block")
        anchor.blocks[block_hash] = known
    if known != number or number > anchor.number or anchor.numbers.setdefault(number, block_hash) != block_hash:
        raise WatcherTransientError("base_anchor_mismatch",
                                    f"{what}: block {block_hash} is block {known}, not block {number} of the chain "
                                    f"anchored at {anchor.number} ({anchor.hash})")


def _check_anchor(base: Any, anchor: BaseAnchor) -> None:
    """The anchor read again by number at the end of the run: a reorg below
    it, or another network's answer to the first read, refuses the run."""

    if (found := _block_hash(base, anchor.number)) != anchor.hash:
        raise WatcherTransientError("base_anchor_mismatch",
                                    f"block {anchor.number} is now {found}, not the run's anchor {anchor.hash}")


def _lock_record(base: Any, lock: str, nonce: int, block: str | dict[str, Any]) -> tuple[str, int, str]:
    data = "0x" + LOCK_RECORD_SELECTOR + f"{nonce:064x}"
    try:
        answer = pft_token.hex_bytes(_bound(base, "eth_call", [{"to": lock, "data": data}, block],
                                            f"lockRecord({nonce})"), "lockRecord")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    if len(answer) != 96 or any(answer[:12]) or any(answer[32:56]):
        raise WatcherError("base_answer_malformed", f"lockRecord({nonce}) is not (address, uint64, bytes32)")
    holder = "0x" + answer[12:32].hex()
    return holder, int.from_bytes(answer[56:64], "big"), "0x" + answer[64:96].hex()


def _export(entry: Any) -> dict[str, Any]:
    """One paged export entry, validated and reduced to what the export view uses."""

    if not isinstance(entry, dict) or not isinstance(entry.get("packet"), dict):
        raise WatcherError("export_record_malformed", "an export entry has no packet")
    packet = entry["packet"]
    recipient = packet.get("base_recipient")
    if not isinstance(recipient, str) or not ADDRESS.match(recipient):
        raise WatcherError("export_record_malformed", "base_recipient is not an address")
    for field in ("packet_digest", "withdrawal_id_commitment", "burn_tx_commitment"):
        if not isinstance(entry.get(field), str) or not HEX48.match(entry[field]):
            raise WatcherError("export_record_malformed", f"{field} is not 0x and 96 hex digits")
    try:
        nonce = _u64(packet.get("export_nonce"), "export_nonce")
        amount = _u64(packet.get("amount_atoms"), "amount_atoms")
        height = _u64(packet.get("exported_at_height"), "exported_at_height")
    except WatcherError as error:
        raise WatcherError("export_record_malformed", str(error)) from None
    return {
        "export_nonce": nonce,
        "amount_atoms": amount,
        "base_recipient": recipient.lower(),
        "exported_at_height": height,
        "tx_id": packet.get("tx_id"),
        "packet_digest": entry["packet_digest"],
        "evm_digest": export_evm_digest(entry["packet_digest"]),
        "withdrawal_id_commitment": commitment_of(entry["withdrawal_id_commitment"]),
        "burn_tx_id_commitment": commitment_of(entry["burn_tx_commitment"]),
    }


def _export_snapshot(info: dict[str, Any], row: dict[str, Any]) -> dict[str, Any]:
    """The export counters every page must share. `export_count`,
    `pruned_export_count` and `state.released_atoms` are always reported;
    `exported_atoms` and `pruned_exported_atoms` are left out while zero."""

    state = row.get("state")
    if not isinstance(state, dict):
        raise WatcherError("export_record_malformed", "the lane row has no state")
    try:
        snapshot = {
            "chain_id": info.get("chain_id"),
            "genesis_hash": info.get("genesis_hash"),
            "current_height": _u64(info.get("current_height"), "current_height"),
            "export_count": _u64(row.get("export_count"), "export_count"),
            "pruned_export_count": _u64(row.get("pruned_export_count"), "pruned_export_count"),
            "export_history_root": row.get("export_history_root"),
            "released_atoms": _u64(state.get("released_atoms"), "state.released_atoms"),
            "exported_atoms": _u64(state.get("exported_atoms", 0), "state.exported_atoms"),
            "pruned_exported_atoms": _u64(state.get("pruned_exported_atoms", 0), "state.pruned_exported_atoms"),
        }
    except WatcherError as error:
        raise WatcherError("export_record_malformed", str(error)) from None
    if snapshot["pruned_export_count"] > snapshot["export_count"]:
        raise WatcherError("export_record_malformed", "more exports pruned than exported")
    return snapshot


def _run_identity(lane_id: str, snapshot: dict[str, Any], policy: dict[str, Any]) -> dict[str, Any]:
    """The identity the release view validated (against the state file too):
    lane, PFTL chain id and genesis, source chain id, lock and its code hash."""

    return {"lane_id": lane_id, "pftl_chain_id": snapshot["chain_id"], "genesis_hash": snapshot["genesis_hash"],
            "source_chain_id": policy["chain_id"], "lock": policy["lock"], "code_hash": policy["code_hash"]}


def _exports_of(pftl: Any, lane_id: str, identity: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Every stored export packet of the lane, oldest first, and the snapshot
    it was paged from. Every page, the first one included, must carry the
    run's validated `identity` (`export_identity_mismatch`), checked before
    any of its packets is read: an endpoint that fails over between the
    release read and the export read, or between two export pages, cannot
    lend another network's exports to this one. Heights and counters may
    differ from the release view's (honest progress between the reads). As
    for releases: every page from the first page's snapshot, nonces
    contiguous from `export_count - 1` down to the pruned count, each cursor
    the page's oldest nonce and strictly decreasing."""

    exports: list[dict[str, Any]] = []
    cursor: int | None = None
    first: dict[str, Any] | None = None
    seen: set[int] = set()
    while True:
        try:
            info = pftl.external_lane_info(lane_id, export_cursor=cursor, export_limit=PAGE_SIZE)
        except RpcClientError as error:
            raise WatcherRpcError("pftl_rpc_failed", f"external_lane_info: {error}") from None
        if not isinstance(info, dict):
            raise WatcherError("export_record_malformed", "external_lane_info is not an object")
        rows = [row for row in info.get("lanes") or [] if isinstance(row, dict) and row.get("lane_id") == lane_id]
        if len(rows) != 1:
            raise WatcherError("external_lane_not_reported", f"the PFTL node reports no lane `{lane_id}`")
        row = rows[0]
        try:
            seen_identity = _run_identity(lane_id, {"chain_id": info.get("chain_id"),
                                                   "genesis_hash": info.get("genesis_hash")}, _lane_policy(row))
        except WatcherError as error:
            raise WatcherError("export_identity_mismatch", f"an export page's lane policy: {error}") from None
        if seen_identity != identity:
            changed = sorted(key for key in identity if identity[key] != seen_identity[key])
            raise WatcherError("export_identity_mismatch",
                               f"an export page is not of the release view's network, lane or policy: "
                               f"{', '.join(changed)}")
        snapshot = _export_snapshot(info, row)
        if first is None:
            first = snapshot
            expected = first["export_count"] - 1  # the next nonce, newest first
        elif snapshot != first:
            changed = sorted(key for key in first if first[key] != snapshot.get(key))
            raise WatcherTransientError("external_lane_info_inconsistent",
                                        f"the lane report changed while exports were paged: {', '.join(changed)}")
        page = [_export(entry) for entry in row.get("exports") or []]
        for entry in page:
            if entry["export_nonce"] != expected:
                raise WatcherError("export_record_malformed",
                                   f"export nonce {entry['export_nonce']} where {expected} was due")
            expected -= 1
        exports.extend(page)
        next_cursor = row.get("next_export_cursor")
        if next_cursor is None or not page:
            if expected != first["pruned_export_count"] - 1:
                raise WatcherError("export_record_malformed",
                                   f"the stored exports end at {expected + 1}, not at the pruned count "
                                   f"{first['pruned_export_count']}")
            break
        try:
            next_cursor = _u64(next_cursor, "next_export_cursor")
        except WatcherError as error:
            raise WatcherError("export_record_malformed", str(error)) from None
        if next_cursor != page[-1]["export_nonce"] or next_cursor in seen:
            raise WatcherError("export_record_malformed",
                               "next_export_cursor is not the page's oldest nonce, or repeats")
        seen.add(next_cursor)
        cursor = next_cursor
    assert first is not None
    stored = sum(entry["amount_atoms"] for entry in exports)
    if first["exported_atoms"] != first["pruned_exported_atoms"] + stored:
        raise WatcherError("export_record_malformed",
                           "exported_atoms is not the pruned atoms plus the stored packets' amounts")
    return list(reversed(exports)), first


def _released_logs(base: Any, lock: str, start: int, end: int) -> list[Any]:
    """`Released` logs of the lock in blocks `start..end`, read in chunks; a
    failed request (an RPC error or an answer above the size limit) is
    retried with half the chunk, down to `MIN_LOG_CHUNK_BLOCKS`."""

    logs: list[Any] = []
    block, size = start, LOG_CHUNK_BLOCKS
    while block <= end:
        to = min(end, block + size - 1)
        try:
            answer = _base(base, "eth_getLogs", [{"address": lock, "topics": [RELEASED_TOPIC],
                                                  "fromBlock": hex(block), "toBlock": hex(to)}])
        except WatcherRpcError:
            if size <= MIN_LOG_CHUNK_BLOCKS:
                raise
            size = max(MIN_LOG_CHUNK_BLOCKS, size // 2)
            continue
        if not isinstance(answer, list):
            raise WatcherError("base_answer_malformed", "eth_getLogs did not return a list")
        logs.extend((log, block, to) for log in answer)
        block = to + 1
    return logs


def _released_events(base: Any, lock: str, start: int, end: int) -> list[dict[str, Any]]:
    """Every `Released` event of the lock in finalized blocks `start..end`."""

    events = []
    for log, first, last in _released_logs(base, lock, start, end):
        try:
            if not isinstance(log, dict):
                raise ValueError("a log is not an object")
            topics = log.get("topics")
            data = log.get("data")
            if (not isinstance(topics, list) or len(topics) != 4
                    or not all(isinstance(topic, str) and HASH32.match(topic) for topic in topics)
                    or topics[0].lower() != RELEASED_TOPIC
                    or not isinstance(data, str) or not re.fullmatch(r"0x[0-9a-fA-F]{128}", data)
                    or str(log.get("address", "")).lower() != lock or any(int(data[2:26], 16).to_bytes(12, "big"))):
                raise ValueError("a log is not Released(3 topics, address, uint256) of the lock")
            number = pft_token.hex_quantity(log.get("blockNumber"), "log blockNumber")
            log_index = pft_token.hex_quantity(log.get("logIndex"), "log logIndex")
            transaction = log.get("transactionHash")
            if not isinstance(transaction, str) or not HASH32.match(transaction):
                raise ValueError("a log without a transaction hash")
            if not first <= number <= last:
                raise ValueError(f"a log of block {number} outside the range {first}..{last}")
        except (ValueError, pft_token.PftTokenRefused) as error:
            raise WatcherError("base_answer_malformed", f"eth_getLogs: {error}") from None
        events.append({
            "withdrawal_id_commitment": topics[1].lower(), "burn_tx_id_commitment": topics[2].lower(),
            "packet_digest": topics[3].lower(), "recipient": "0x" + data[26:66].lower(),
            "amount": int(data[66:130], 16), "transaction": transaction.lower(), "block": number,
            "log_index": log_index,
        })
    return events


def _u256_call(base: Any, to: str, data: str, block: dict[str, Any]) -> int:
    try:
        answer = pft_token.hex_bytes(_bound(base, "eth_call", [{"to": to, "data": data}, block], f"eth_call {data[:10]}"),
                                     "eth_call")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    if len(answer) != 32:
        raise WatcherError("base_answer_malformed", f"eth_call to {to} did not return one word")
    return int.from_bytes(answer, "big")


def _selector(signature: str) -> str:
    return "0x" + keccak256(signature.encode()).hex()[:8]


PAID_KEYS = ("export_nonce", "evm_digest", "block", "transaction", "log_index", "amount",
             "withdrawal_id_commitment")
BASELINE_KEYS = ("unverified_payouts", "unverified_atoms", "pruned_export_count", "pruned_exported_atoms")
# The export snapshot a clean run used: its PFTL height and counters, each a
# high-water mark (a run on a lower snapshot is refused).
SNAPSHOT_KEYS = ("height", "export_count", "exported_atoms", "pruned_export_count", "pruned_exported_atoms")
# The most recently pruned paid exports, oldest first, each with the payout
# that was verified: a different payout of one is `payout_duplicate` in every
# run, and a rescan recognises the original. 4,096 entries at u64-wide numbers
# are 1,257,472 bytes, 7.5% of MAX_STATE_BYTES (several release windows of
# pruning on a busy lane).
RECENT_PRUNED_PAID = 4096
RECENT_KEYS = ("export_nonce", "evm_digest", "block", "transaction", "log_index", "amount")
# The exports entry's format. An entry without this number is never used and
# never upgraded in place: the export view has never been released, so the
# only earlier entries are this branch's intermediate ones, and their history
# (which exports were folded, at which snapshot) cannot be recovered from
# them. The view then runs as a first run from --lock-from-block.
EXPORT_STATE_FORMAT = 1
RESCAN_REASONS = ("export_state_format_old", "lock_from_block_changed", "from_height", "export_history_discarded")


def _export_state(state: Any) -> tuple[dict[str, Any] | None, str | None]:
    """The export view's kept state and why it is not used: (None, None) when
    absent, (None, "export_state_format_old") for an entry of an earlier
    format, else (state, None) once its form is checked."""

    if state is None:
        return None, None
    if not isinstance(state, dict):
        raise WatcherError("state_file_invalid", "the state file's exports entry is malformed")
    if state.get("format") != EXPORT_STATE_FORMAT:
        return None, "export_state_format_old"

    def count(value: Any) -> bool:
        return isinstance(value, int) and not isinstance(value, bool) and value >= 0

    def hash32(value: Any) -> bool:
        return isinstance(value, str) and HASH32.match(value) is not None

    def event(entry: Any, keys: tuple[str, ...]) -> bool:
        return (isinstance(entry, dict) and set(entry) == set(keys)
                and all(count(entry[key]) for key in ("export_nonce", "block", "log_index", "amount"))
                and all(hash32(entry[key]) for key in keys if key in ("evm_digest", "transaction",
                                                                     "withdrawal_id_commitment")))

    keys = ("lock_from_block", "scanned_through_block", "payout_count", "payout_sum", "pruned_paid_count",
            "pruned_paid_atoms")
    if not all(count(state.get(key)) for key in keys):
        raise WatcherError("state_file_invalid", "the state file's exports entry is malformed")
    paid = state.get("paid")
    if (not isinstance(paid, list) or not all(event(entry, PAID_KEYS) for entry in paid)
            or len({entry["export_nonce"] for entry in paid}) != len(paid)
            or len({entry["evm_digest"] for entry in paid}) != len(paid)):
        raise WatcherError("state_file_invalid", "the state file's paid record is malformed")
    recent = state.get("pruned_paid_recent")
    if (not isinstance(recent, list) or not all(event(entry, RECENT_KEYS) for entry in recent)
            or len({entry["evm_digest"] for entry in [*recent, *paid]}) != len(recent) + len(paid)):
        raise WatcherError("state_file_invalid", "the state file's recently pruned paid exports are malformed")
    marks = state.get("snapshot")
    if (not isinstance(marks, dict) or set(marks) != set(SNAPSHOT_KEYS)
            or not all(count(value) for value in marks.values())
            or any(entry["export_nonce"] < marks["pruned_export_count"] for entry in paid)):
        # A paid entry below the pruned mark was folded already: it may not be folded twice.
        raise WatcherError("state_file_invalid", "the state file's export snapshot is malformed")
    baseline = state.get("baseline")
    if baseline is not None and (
            not isinstance(baseline, dict) or baseline.get("baseline_accepted") is not True
            or not isinstance(baseline.get("accepted_at"), str)
            or not all(count(baseline.get(key)) for key in BASELINE_KEYS)):
        raise WatcherError("state_file_invalid", "the state file's export baseline is malformed")
    return state, None


def _same_event(known: dict[str, Any], event: dict[str, Any]) -> bool:
    return all(known[key] == event[key] for key in ("block", "transaction", "log_index", "amount"))


def export_view(pftl: Any, base: Any, lane_id: str, *, identity: dict[str, Any], lock: str, finalized: int,
                current_height: int, released_atoms: int, from_block: int, state: dict[str, Any] | None = None,
                accept_baseline: bool = False, unpaid_warning_blocks: int | None, alarm: Callable[..., None],
                lines: list[dict[str, Any]], rescan_all: bool = False,
                discard_history: bool = False, anchor: BaseAnchor | None = None) -> tuple[dict[str, Any], dict[str, Any]]:
    """The outbound direction: every finalized `Released` payout must stand on a
    PFTL export packet (same digest, recipient, amount, withdrawal-id and
    burn commitments); every final export is `paid` or `unpaid`; the totals
    of the lock and the lane must agree. Returns the summary and the state
    to keep: the finalized block scanned through, the payout count and sum
    up to it, and the paid record; a run with that state reads only later
    blocks.

    The paid record holds every verified payout of a retained export (nonce,
    EVM digest, block, transaction, log index, amount, withdrawal-id
    commitment). A payout whose digest is in it, or among the last
    RECENT_PRUNED_PAID folded ones (`pruned_paid_recent`), is
    `payout_duplicate` in any run, also under `accept_baseline`; a retained
    export in it is `paid` with its reference. An
    entry is dropped only once PFTL has pruned its packet (nonce below the
    pruned count) and is then folded into `pruned_paid_count` and
    `pruned_paid_atoms`, which the bounds of unknown payouts subtract from
    the pruned count and atoms: a payout verified before pruning is never
    counted as unknown, and a new unknown one still alarms. The state also
    keeps the export snapshot used (`snapshot`: PFTL height, export count,
    exported atoms, pruned count and atoms), each a high-water mark: a lower
    snapshot, or a pruned count or atoms below the verified-then-pruned and
    accepted ones, is refused (`export_snapshot_behind_state`) before any
    payout is looked up or any entry folded.

    `released_atoms` comes from the run's first lane page, read before the
    finalized Base block was fixed, and `exported_atoms` from the export
    pages, read after it; both only grow, so `released_atoms <= totalLocked`
    and `payouts <= exported_atoms` can only err towards an alarm, never
    towards silence, when the two chains move between the reads.

    With `accept_baseline`, a payout whose packet is pruned and was never
    verified is a `baseline_payout` line instead of an alarm, as long as the
    unknown payouts stay within the pruned count and atoms left after the
    verified-then-pruned payouts and any earlier baseline; a clean run keeps
    a `baseline` entry (time, cumulative count and atoms accepted, and the
    lane's pruned count and atoms then) that every later summary repeats. A
    second acceptance is refused (`export_baseline_already_taken`) unless the
    lane has pruned more count and atoms since the last one."""

    def warning(code: str, **fields: Any) -> None:
        if code not in WARNING_CODES:
            raise AssertionError(f"undeclared warning code {code}")
        lines.append({"schema": LINE_SCHEMA, "kind": "warning", "code": code, "lane_id": lane_id, **fields})

    if anchor is None:
        anchor = _anchor(base)
    if anchor.number != finalized:
        raise WatcherTransientError("base_anchor_mismatch",
                                    f"the export view's finalized block {finalized} is not the anchor {anchor.number}")
    exports, snapshot = _exports_of(pftl, lane_id, identity)
    by_digest = {entry["evm_digest"]: entry for entry in exports}
    pruned, pruned_atoms = snapshot["pruned_export_count"], snapshot["pruned_exported_atoms"]
    # An entry of an earlier format is never used (see EXPORT_STATE_FORMAT):
    # nothing is carried, nothing trusted, and the scan starts at from_block.
    kept, rescan = _export_state(state)
    # A Base node behind the kept scan (a lagging node, a failover) could hide
    # a known payout from a rescan: nothing is discarded or rewritten on it.
    if kept is not None and kept["scanned_through_block"] > finalized:
        raise WatcherTransientError("base_finalized_behind_state",
                                    f"the Base node's finalized block {finalized} is below the state's "
                                    f"{kept['scanned_through_block']}: a lagging node or another chain")
    if discard_history:
        paid_entries = (kept or {}).get("paid", [])
        lines.append({"schema": LINE_SCHEMA, "kind": "history_discarded", "lane_id": lane_id,
                      "identities": len(paid_entries) + len((kept or {}).get("pruned_paid_recent", [])),
                      "verified_payouts": len(paid_entries) + (kept or {}).get("pruned_paid_count", 0),
                      "verified_atoms": sum(entry["amount"] for entry in paid_entries)
                      + (kept or {}).get("pruned_paid_atoms", 0)})
        kept, rescan = None, "export_history_discarded"
    # A snapshot older than one a clean run acted on (an honest node behind,
    # an RPC failover) could resurrect a folded export: nothing is judged on it.
    marks = (kept or {}).get("snapshot")
    now = {"height": snapshot["current_height"], **{key: snapshot[key] for key in SNAPSHOT_KEYS[1:]}}
    behind = [f"{key} {now[key]} < {marks[key]}" for key in SNAPSHOT_KEYS if marks and now[key] < marks[key]]
    if behind:
        raise WatcherTransientError("export_snapshot_behind_state",
                                    f"the PFTL export snapshot is older than the state's: {', '.join(behind)}")
    # A rescan rebuilds the scan and the accounting from from_block, but keeps
    # every payout identity it knew, for comparison only: the recorded event
    # is the original, any other payout of a known digest a duplicate.
    known: dict[str, dict[str, Any]] = {}
    known_paid: dict[str, dict[str, Any]] = {}
    if kept is not None and (rescan_all or kept["lock_from_block"] != from_block):
        rescan = "from_height" if rescan_all else "lock_from_block_changed"
        for entry in [*kept["pruned_paid_recent"], *kept["paid"]]:
            known[entry["evm_digest"]] = {key: entry[key] for key in RECENT_KEYS}
        known_paid = {entry["evm_digest"]: dict(entry) for entry in kept["paid"]}
        kept = None
    if kept is not None:
        start, prior_count, prior_sum = kept["scanned_through_block"] + 1, kept["payout_count"], kept["payout_sum"]
        record = {entry["evm_digest"]: dict(entry) for entry in kept["paid"]}
        pruned_paid_count, pruned_paid_atoms = kept["pruned_paid_count"], kept["pruned_paid_atoms"]
        recent = [dict(entry) for entry in kept["pruned_paid_recent"]]
        baseline = kept.get("baseline")
    else:
        start, prior_count, prior_sum, record, recent = from_block, 0, 0, {}, []
        pruned_paid_count = pruned_paid_atoms = 0
        baseline = None
    if accept_baseline and baseline is not None and not (
            pruned > baseline["pruned_export_count"] and pruned_atoms > baseline["pruned_exported_atoms"]):
        raise WatcherError("export_baseline_already_taken",
                           f"a baseline of {baseline['unverified_payouts']} payouts was accepted at "
                           f"{baseline['accepted_at']} and the lane has pruned no more since")
    # Entries whose packets PFTL has pruned: folded into the two numbers. The
    # pruned count is at or above its mark, and the record holds no nonce
    # below the mark, so each nonce is folded once.
    for digest, entry in sorted(record.items(), key=lambda item: item[1]["export_nonce"]):
        if entry["export_nonce"] < pruned:
            del record[digest]
            pruned_paid_count += 1
            pruned_paid_atoms += entry["amount"]
            recent.append({key: entry[key] for key in RECENT_KEYS})
    recently_pruned = {entry["evm_digest"]: entry for entry in recent}
    prior_accepted = (baseline["unverified_payouts"], baseline["unverified_atoms"]) if baseline else (0, 0)

    def room() -> tuple[int, int]:
        free = (pruned - pruned_paid_count - prior_accepted[0], pruned_atoms - pruned_paid_atoms - prior_accepted[1])
        if free[0] < 0 or free[1] < 0:
            # More verified-then-pruned or accepted than the node reports pruned.
            raise WatcherTransientError("export_snapshot_behind_state",
                                        f"the lane reports {pruned} pruned exports ({pruned_atoms} atoms), fewer "
                                        f"than the state's {pruned_paid_count + prior_accepted[0]} "
                                        f"({pruned_paid_atoms + prior_accepted[1]} atoms) paid or accepted")
        return free

    room()  # before any payout is read
    events = _released_events(base, lock, start, finalized)
    # On a rescan, the originals of known payouts whose packets PFTL has
    # pruned since: accounted as verified before the room is computed, never
    # judged again against a packet that is gone.
    # In any run, the recorded event of a recently pruned paid export is its
    # original too (folded already), never a duplicate of itself.
    originals: set[tuple[int, str, int]] = set()
    for event in events:
        digest = event["packet_digest"]
        if digest in by_digest:
            continue
        if digest in recently_pruned and _same_event(recently_pruned[digest], event):
            originals.add((event["block"], event["transaction"], event["log_index"]))
            continue
        first = known.get(digest)
        if first is None or not _same_event(first, event):
            continue
        if first["export_nonce"] >= pruned:
            raise WatcherTransientError("export_snapshot_behind_state",
                                        f"export {first['export_nonce']}, paid and verified earlier, is neither "
                                        f"stored nor pruned in this snapshot (pruned count {pruned})")
        originals.add((event["block"], event["transaction"], event["log_index"]))
        pruned_paid_count += 1
        pruned_paid_atoms += event["amount"]
    free_count, free_atoms = room()
    seen: set[str] = set()
    paid_total = unknown_count = unknown_sum = accepted_count = accepted_sum = 0
    # Every payout counts in the totals, matched or not: a payout without an
    # export must show in totalReleased and against exported_atoms as well.
    payout_total = prior_sum + sum(event["amount"] for event in events)
    # The totals, at the finalized block only, read before any payout is
    # judged: a scan whose payouts do not add up to totalReleased is
    # incomplete, and none of its matches is published as `backed`,
    # `verified_earlier` or `paid` (`matched_incomplete_scan`). Its alarm
    # keeps the export part of the state unchanged; a clean release view
    # still saves its own height, so the file as a whole may differ.
    tag = anchor.param
    token = "0x" + f"{_u256_call(base, lock, _selector('token()'), tag):064x}"[-40:]
    total_locked = _u256_call(base, lock, _selector("totalLocked()"), tag)
    total_released = _u256_call(base, lock, _selector("totalReleased()"), tag)
    lock_balance = _u256_call(base, token, _selector("balanceOf(address)") + lock.removeprefix("0x").rjust(64, "0"),
                              tag)
    complete = total_released == payout_total
    matched_now: set[str] = set()
    for event in events:
        result = {"schema": LINE_SCHEMA, "kind": "payout", "lane_id": lane_id, **event}
        digest = event["packet_digest"]
        first = record.get(digest) or recently_pruned.get(digest) or known.get(digest)
        if (event["block"], event["transaction"], event["log_index"]) in originals and digest not in seen:
            seen.add(digest)
            result.update(status="verified_earlier" if complete else "matched_incomplete_scan",
                          export_nonce=first["export_nonce"])
            lines.append(result)
            paid_total += event["amount"]
            continue
        # The recorded event itself is the original (judged again against a
        # retained packet); any other payout of a known digest a duplicate.
        if digest in seen or (first is not None and not _same_event(first, event)):
            # Also under --accept-export-baseline: the alarm keeps any baseline from being saved.
            result["status"] = "alarm"
            lines.append(result)
            alarm("payout_duplicate", **event,
                  **({"paid_block": first["block"], "paid_transaction": first["transaction"],
                      "paid_log_index": first["log_index"]} if first else {}),
                  **({"paid_export_pruned": True} if digest in recently_pruned else {}))
            continue
        seen.add(digest)
        packet = by_digest.get(digest)
        if packet is None:
            # Neither stored nor in the paid record: a packet pruned before
            # this watcher checked its payout, or a payout PFTL never exported.
            unknown_count += 1
            unknown_sum += event["amount"]
            if (pruned and accept_baseline and accepted_count < free_count
                    and accepted_sum + event["amount"] <= free_atoms):
                result["status"] = "baseline_unverified"
                lines.append(result)
                lines.append({"schema": LINE_SCHEMA, "kind": "baseline_payout", "lane_id": lane_id,
                              "block": event["block"], "transaction": event["transaction"],
                              "amount": event["amount"], "recipient": event["recipient"],
                              "packet_digest": digest})
                accepted_count += 1
                accepted_sum += event["amount"]
                continue
            result["status"] = "alarm"
            lines.append(result)
            if pruned:
                alarm("payout_pruned_before_check", pruned_export_count=pruned, **event)
            else:
                alarm("payout_without_export", **event)
            continue
        codes = []
        if event["amount"] != packet["amount_atoms"]:
            codes.append("payout_amount_mismatch")
        if event["recipient"] != packet["base_recipient"]:
            codes.append("payout_recipient_mismatch")
        if event["withdrawal_id_commitment"] != packet["withdrawal_id_commitment"]:
            codes.append("payout_withdrawal_mismatch")
        if event["burn_tx_id_commitment"] != packet["burn_tx_id_commitment"]:
            codes.append("payout_burn_mismatch")
        result.update(status="alarm" if codes else "backed" if complete else "matched_incomplete_scan",
                      export_nonce=packet["export_nonce"])
        lines.append(result)
        for code in codes:
            alarm(code, export_nonce=packet["export_nonce"], packet_amount_atoms=packet["amount_atoms"],
                  packet_base_recipient=packet["base_recipient"], **event)
        if not codes:
            matched_now.add(digest)
            record[digest] = {"export_nonce": packet["export_nonce"], "evm_digest": digest,
                              "block": event["block"], "transaction": event["transaction"],
                              "log_index": event["log_index"], "amount": event["amount"],
                              "withdrawal_id_commitment": event["withdrawal_id_commitment"]}
        paid_total += event["amount"]
    if pruned and (unknown_count > free_count or unknown_sum > free_atoms):
        alarm("payout_unknown_above_pruned", unknown_payouts=unknown_count, unknown_atoms=unknown_sum,
              pruned_export_count=pruned, pruned_exported_atoms=pruned_atoms,
              pruned_paid_count=pruned_paid_count, pruned_paid_atoms=pruned_paid_atoms,
              baseline_payouts=prior_accepted[0], baseline_atoms=prior_accepted[1])
    # A known payout of a retained export that this scan did not reach stays
    # in the record: it is never moved to the recent list for that.
    for digest, entry in known_paid.items():
        if digest not in record and digest not in seen and entry["export_nonce"] >= pruned:
            record[digest] = entry
    by_nonce = {entry["export_nonce"]: entry for entry in record.values()}
    unpaid = 0
    for packet in exports:
        if packet["exported_at_height"] > current_height:
            continue
        entry = by_nonce.get(packet["export_nonce"])
        line = {"schema": LINE_SCHEMA, "kind": "export", "lane_id": lane_id,
                **{key: packet[key] for key in ("export_nonce", "amount_atoms", "base_recipient",
                                                "exported_at_height", "packet_digest", "evm_digest")}}
        if entry is not None:
            line.update(status="paid" if complete or entry["evm_digest"] not in matched_now
                        else "matched_incomplete_scan", base_transaction=entry["transaction"], base_block=entry["block"],
                        base_log_index=entry["log_index"])
            lines.append(line)
            continue
        unpaid += 1
        age = current_height - packet["exported_at_height"]
        line.update(status="unpaid", age_blocks=age)
        lines.append(line)
        if unpaid_warning_blocks is not None and age > unpaid_warning_blocks:
            warning("export_unpaid_too_long", export_nonce=packet["export_nonce"], age_blocks=age,
                    threshold_blocks=unpaid_warning_blocks)
    exported_atoms = snapshot["exported_atoms"]
    # Negative only if the lock's own invariant broke; then no balance is a surplus.
    expected_balance = total_locked - total_released
    totals = {"total_locked": total_locked, "total_released": total_released, "lock_balance": lock_balance,
              "lock_surplus": max(0, lock_balance - expected_balance) if expected_balance >= 0 else 0,
              "payout_sum": payout_total,
              "paid_sum": paid_total, "exported_atoms": exported_atoms, "released_atoms": released_atoms,
              "pruned_exported_atoms": snapshot["pruned_exported_atoms"]}
    # totalReleased(finalized) - totalReleased(saved block) must be the new
    # events' sum; the saved block's value is the payout sum a clean run
    # verified against it, so no archive state is read.
    if total_released != payout_total:
        alarm("total_released_mismatch", **totals, from_block=from_block, scanned_from_block=start)
    # PFTLockV1 never releases more than it locked; if it did, the expected
    # balance is negative, no balance can match it, and both alarms are raised.
    if total_released > total_locked:
        alarm("base_released_above_locked", **totals)
    # PFTLockV1 holds at least totalLocked - totalReleased: a plain transfer
    # to the lock adds a surplus, which is reported, not raised.
    if lock_balance < expected_balance or expected_balance < 0:
        alarm("lock_balance_mismatch", **totals)
    if exported_atoms < payout_total:
        alarm("exported_below_paid", **totals)
    if released_atoms > total_locked:
        alarm("released_above_locked", **totals)
    if known:
        # Every identity carried and not in the rebuilt record stays known.
        recent = [entry for entry in known.values() if entry["evm_digest"] not in record]
    recent = recent[-RECENT_PRUNED_PAID:] if RECENT_PRUNED_PAID else []
    new_state = {"format": EXPORT_STATE_FORMAT, "lock_from_block": from_block, "scanned_through_block": finalized,
                 "payout_count": prior_count + len(events), "payout_sum": payout_total,
                 "paid": sorted(record.values(), key=lambda entry: entry["export_nonce"]),
                 "pruned_paid_count": pruned_paid_count, "pruned_paid_atoms": pruned_paid_atoms,
                 "pruned_paid_recent": recent, "snapshot": now}
    if accepted_count:
        baseline = {"baseline_accepted": True,
                    "accepted_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                    "unverified_payouts": prior_accepted[0] + accepted_count,
                    "unverified_atoms": prior_accepted[1] + accepted_sum,
                    "pruned_export_count": pruned, "pruned_exported_atoms": pruned_atoms}
    if baseline is not None:
        new_state["baseline"] = baseline
    summary = {"exports_checked": len(exports), "payouts_checked": len(events), "unpaid_exports": unpaid,
               "pruned_export_count": pruned, "scanned_from_block": start, "totals": totals,
               "paid_recorded": len(record), "pruned_paid_count": pruned_paid_count,
               "pruned_paid_atoms": pruned_paid_atoms, "rescan": rescan, "known_payouts_carried": len(known),
               "baseline_accepted_now": accepted_count}
    if baseline is not None:
        summary["baseline"] = baseline
    return summary, new_state


# --- One run. ---


def _load_state(path: Path | None, *, earlier_schema: bool = False) -> dict[str, Any] | None:
    """The state file, its form checked; its identity is checked against the
    run's own reads (`_check_identity`) before any part of it is used. With
    `earlier_schema` (--from-height), a readable file of another schema is
    no state (the run replaces it); an unreadable file is always refused."""

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

    if earlier_schema and isinstance(state, dict) and state.get("schema") != STATE_SCHEMA:
        return None
    if (
        not isinstance(state, dict)
        or state.get("schema") != STATE_SCHEMA
        or not all(isinstance(state.get(key), str) for key in ("lane_id", "pftl_chain_id", "genesis_hash",
                                                              "lock_contract_address"))
        or not all(u64(state.get(key)) for key in ("source_chain_id", "checked_through_height",
                                                   "release_count"))
    ):
        raise WatcherError("state_file_invalid", f"`{path}` is not a watcher state of this schema")
    return state


def _check_identity(state: dict[str, Any], lane_id: str, snapshot: dict[str, Any], policy: dict[str, Any]) -> None:
    """The file's identity (lane, PFTL chain id and genesis, source chain id,
    lock) must be this run's, on every path: a normal run, --from-height, a
    changed --lock-from-block and a discard alike."""

    if (state["lane_id"], state["pftl_chain_id"], state["genesis_hash"], state["lock_contract_address"].lower(),
            state["source_chain_id"]) != (lane_id, snapshot["chain_id"], snapshot["genesis_hash"], policy["lock"],
                                          policy["chain_id"]):
        raise WatcherError("state_file_invalid", "the state file belongs to another chain, genesis, lane or lock")


def _check_cursors(state: dict[str, Any], snapshot: dict[str, Any]) -> None:
    """The kept release cursor must not be ahead of the node: a regression
    means another network or a corrupt file."""

    if state["checked_through_height"] > snapshot["current_height"] or state["release_count"] > snapshot["release_count"]:
        raise WatcherError("state_file_invalid",
                           "the state file is ahead of the PFTL node: another network or a corrupt file")


def _save_state(path: Path, state: dict[str, Any]) -> None:
    """Replace the file atomically, and only with a state the next run can
    load: above MAX_STATE_BYTES the run fails and the last good file stays."""

    data = (json.dumps(state, sort_keys=True) + "\n").encode("utf-8")
    if len(data) > MAX_STATE_BYTES:
        raise WatcherTransientError("state_file_too_large",
                                    f"`{path}`: the new state is {len(data)} bytes, above {MAX_STATE_BYTES}; the "
                                    "last good file is kept")
    temporary = path.with_name(path.name + ".tmp")
    try:
        temporary.write_bytes(data)
        temporary.replace(path)
    except OSError as error:
        if temporary.parent.is_dir():
            temporary.unlink(missing_ok=True)
        raise WatcherTransientError("state_file_write_failed", f"`{path}`: {error}") from None


def watch_once(pftl: Any, base: Any, lane_id: str, *, from_height: int | None = None,
               state: dict[str, Any] | None = None, exports: bool = False, lock_from_block: int = 0,
               unpaid_warning_blocks: int | None = None, emit: Callable[[dict[str, Any]], None] | None = None,
               accept_export_baseline: bool = False, discard_export_history: bool = False,
               ) -> tuple[list[dict[str, Any]], int, dict[str, Any] | None]:
    """One run: the output lines, the alarm count and the state to keep.

    Each view's state advances only after a clean run of that view, so an
    alarm is raised again on every run until an operator starts past it
    with `from_height`. With `emit`, the release view's lines are handed to
    it before the export view starts, and only the rest is returned; a
    failure of the export view is an `error` line and `export_error` in the
    summary, never a lost release alarm. The state's identity is checked on
    every path; with `from_height` its release cursor is not used and its
    exports entry is rescanned with its known payouts carried.
    """

    row, snapshot = _lane_page(pftl, lane_id, None)
    released_atoms = None
    if exports:
        # Read before the finalized Base block is fixed (see export_view).
        try:
            released_atoms = _u64((row.get("state") or {}).get("released_atoms"), "state.released_atoms")
        except WatcherError as error:
            raise WatcherError("external_lane_info_malformed", str(error)) from None
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
    # The run's anchor: every later Base answer is bound to this block.
    anchor = _anchor(base)
    finalized = anchor.number

    lines: list[dict[str, Any]] = []

    def alarm(code: str, **fields: Any) -> None:
        if code not in ALARM_CODES:
            raise AssertionError(f"undeclared alarm code {code}")
        lines.append({"schema": LINE_SCHEMA, "kind": "alarm", "code": code, "lane_id": lane_id, **fields})

    try:
        code = pft_token.hex_bytes(_bound(base, "eth_getCode", [policy["lock"], anchor.param], "eth_getCode"),
                                   "eth_getCode")
    except pft_token.PftTokenRefused as error:
        raise WatcherError("base_answer_malformed", str(error)) from None
    observed_code_hash = "0x" + keccak256(code).hex()
    if not code or observed_code_hash != policy["code_hash"]:
        alarm("lock_code_hash_mismatch", lock_contract_address=policy["lock"], finalized_block=finalized,
              expected=policy["code_hash"], observed=observed_code_hash)

    if state is not None:
        _check_identity(state, lane_id, snapshot, policy)
        if from_height is None:
            _check_cursors(state, snapshot)
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

    release_alarms = sum(1 for line in lines if line["kind"] == "alarm")
    emitted = 0
    if emit is not None:
        for line in lines:
            emit(line)
        emitted = len(lines)
    export_summary = export_state = export_error = None
    carried = (state or {}).get("exports")
    if exports:
        assert released_atoms is not None
        try:
            export_summary, export_state = export_view(
                pftl, base, lane_id, identity=_run_identity(lane_id, snapshot, policy), lock=policy["lock"],
                finalized=finalized, current_height=current_height,
                released_atoms=released_atoms, from_block=lock_from_block,
                state=carried, accept_baseline=accept_export_baseline, unpaid_warning_blocks=unpaid_warning_blocks,
                alarm=alarm, lines=lines, rescan_all=from_height is not None,
                discard_history=discard_export_history, anchor=anchor)
        except WatcherError as error:
            if error.code in ANCHOR_CODES:
                raise  # a Base answer not bound to the anchor: the whole run is refused
            export_error = error
            lines.append({"schema": LINE_SCHEMA, "kind": "error", "code": error.code, "message": str(error)})
    _check_anchor(base, anchor)
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
        **({"exports": export_summary} if export_summary else {}),
        **({"export_error": export_error.code} if export_error else {}),
        **({"warnings": sum(1 for line in lines if line["kind"] == "warning"),
            "warning_codes": {code_: sum(1 for line in lines if line["kind"] == "warning" and line["code"] == code_)
                              for code_ in WARNING_CODES}} if exports else {}),
    })
    if any(line["kind"] == "warning" and line["code"] not in WARNING_CODES for line in lines):
        raise AssertionError("undeclared warning code")
    # The release part advances after a clean release view; the export part
    # after a clean export view, else the kept one is carried.
    new_state = None if release_alarms else {
        "schema": STATE_SCHEMA,
        "lane_id": lane_id,
        "pftl_chain_id": snapshot["chain_id"],
        "genesis_hash": snapshot["genesis_hash"],
        "lock_contract_address": policy["lock"],
        "source_chain_id": policy["chain_id"],
        "checked_through_height": current_height,
        "release_count": release_count,
    }
    if new_state is not None:
        export_clean = export_state is not None and alarms == release_alarms
        if exports and export_clean:
            new_state["exports"] = export_state
        elif carried is not None:
            new_state["exports"] = carried
    return lines[emitted:], alarms, new_state


# --- Command line. ---


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.external_lane_watcher",
        description="Watch a genesis external lane, read only: match each PFTL release against its Lock on "
                    "Base and, with --exports, each Base payout against its PFTL export packet and the totals.",
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
    parser.add_argument("--exports", action="store_true",
                        help="also check the outbound direction: every finalized Released payout of the lock "
                             "against the lane's export packets, paid/unpaid exports and the lock/lane totals")
    parser.add_argument("--lock-from-block", type=int, default=0, metavar="N",
                        help="scan Released events from this Base block; at or below the lock's deployment "
                             "block, or totalReleased cannot be matched")
    parser.add_argument("--accept-export-baseline", action="store_true",
                        help="recovery for an export view that starts late or was not run for longer than a "
                             "release window: accept the payouts whose packets were pruned before any check, "
                             "within the pruned count and atoms not already verified or accepted, list them, keep "
                             f"the scan position and exit {BASELINE_EXIT}; refused (export_baseline_already_taken) "
                             "after an earlier baseline unless the lane has pruned more since")
    parser.add_argument("--discard-export-history", action="store_true",
                        help="start the export view as a first run from --lock-from-block, forgetting every "
                             "verified payout it knew: duplicate protection for those payouts ends here; prints "
                             "how many identities and atoms it discards; one-shot runs only")
    parser.add_argument("--unpaid-warning-blocks", type=int, metavar="N",
                        help="warn when a final export stays unpaid for more than N PFTL blocks")
    return parser


BASELINE_EXIT = 3  # a baseline was accepted: neither clean (0) nor alarm (2)
RETRIED_CODES = ("base_answer_malformed", "export_record_malformed", "external_lane_info_malformed")


def _print_line(line: dict[str, Any]) -> None:
    print(json.dumps(line, sort_keys=True), flush=True)


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
    if args.accept_export_baseline and (not args.exports or args.interval is not None):
        parser.error("--accept-export-baseline needs --exports and a one-shot run, not --interval")
    if args.discard_export_history and (not args.exports or args.interval is not None):
        parser.error("--discard-export-history needs --exports and a one-shot run, not --interval")
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
            # --from-height overrides the kept release cursor, and a clean run
            # replaces a file of an earlier schema; a current file's identity
            # is checked first and its export history carried, so the rescan
            # forgets no payout it knew.
            if state_path is not None:
                state = _load_state(state_path, earlier_schema=from_height is not None)
            else:
                state = memory_state
            lines, alarms, new_state = watch_once(pftl, base, args.lane_id, from_height=from_height, state=state,
                                                  exports=args.exports, lock_from_block=args.lock_from_block,
                                                  unpaid_warning_blocks=args.unpaid_warning_blocks,
                                                  emit=_print_line if args.exports else None,
                                                  accept_export_baseline=args.accept_export_baseline,
                                                  discard_export_history=args.discard_export_history)
            # Progress is saved before the summary is printed, so a clean
            # summary always means the state advanced.
            if new_state is not None and state_path is not None:
                _save_state(state_path, new_state)
        except WatcherTransientError as error:
            _error_line(error)
            if args.interval is None:
                return 1
        except WatcherError as error:
            _error_line(error)
            # A malformed answer may be a node's passing fault: --interval keeps watching.
            if args.interval is None or error.code not in RETRIED_CODES:
                return 1
        else:
            failed = False
            for line in lines:
                if line["kind"] == "error":
                    failed = True
                    print(json.dumps(line, sort_keys=True), file=sys.stderr, flush=True)
                else:
                    _print_line(line)
            if new_state is not None:
                from_height = None  # a clean run moves the start to the kept height
                memory_state = new_state
            if args.interval is None:
                if alarms:
                    return 2
                if failed:
                    return 1
                summary = lines[-1].get("exports") or {}
                return BASELINE_EXIT if summary.get("baseline_accepted_now") and new_state is not None else 0
        if max_runs is not None and runs >= max_runs:
            return 0
        sleep(args.interval)


if __name__ == "__main__":
    raise SystemExit(main())
