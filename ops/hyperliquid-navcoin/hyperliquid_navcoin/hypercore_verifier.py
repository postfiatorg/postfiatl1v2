"""HyperCoreReserveReader ABI decoding, commitment verification, and NAV accounting.

Mirrors ``crates/ethereum-contracts/src/HyperCoreReserveReader.sol`` and
``tools/nav-reserve-proof/crates/reserve-proof-types/src/hyperliquid_receipt.rs``
with exact integer arithmetic and conservative floor rounding.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any

from eth_abi import decode as abi_decode
from eth_abi import encode as abi_encode
from eth_utils import keccak as _eth_keccak

HL_RECEIPT_EVENT_SIGNATURE = b"HyperCoreSnapshot(bytes32,uint64,bytes)"
HL_RECEIPT_EVENT_TOPIC0_HEX = _eth_keccak(HL_RECEIPT_EVENT_SIGNATURE).hex()

HYPERCORE_PAYLOAD_ABI_TYPES = [
    "address",
    "(int64,uint64,uint64,int64)",
    "uint64",
    "(uint32,int64,uint64)[]",
    "(uint64,uint64,uint64,uint8,uint64)[]",
]

SUPPORTED_SPOT_TOKENS = {150: 8, 404: 8}  # HYPE (150), XMR1 (404)
_HEX_RE = re.compile(r"^[0-9a-f]+$")
_ETH_ADDR_RE = re.compile(r"^0x[0-9a-fA-F]{40}$")
_B256_RE = re.compile(r"^0x[0-9a-fA-F]{64}$")


def keccak256(data: bytes) -> bytes:
    return bytes(_eth_keccak(data))


def _norm_eth_address(label: str, value: str) -> str:
    if not isinstance(value, str) or not _ETH_ADDR_RE.match(value):
        raise ValueError(f"{label} must be a 0x-prefixed 20-byte hex address")
    if value.lower() == "0x" + "00" * 20:
        raise ValueError(f"{label} cannot be the zero address")
    return value.lower()


def _norm_b256(label: str, value: str, *, allow_zero: bool = False) -> str:
    if not isinstance(value, str) or not _B256_RE.match(value):
        raise ValueError(f"{label} must be a 0x-prefixed 32-byte hex hash")
    norm = value.lower()
    if not allow_zero and norm == "0x" + "00" * 32:
        raise ValueError(f"{label} cannot be zero")
    return norm


@dataclass(frozen=True)
class SpotTokenPolicy:
    token: int
    wei_decimals: int


@dataclass(frozen=True)
class HyperCorePolicy:
    source_domain: str
    aggregate_position_id: str
    hyperevm_chain_id: int
    reader_contract: str
    reader_code_hash: str
    required_perps: tuple[int, ...]
    allowed_spot_tokens: tuple[SpotTokenPolicy, ...]
    expected_account: str

    def validate(self) -> None:
        if self.hyperevm_chain_id <= 0:
            raise ValueError("hyperevm_chain_id must be positive")
        if self.source_domain != f"eip155:{self.hyperevm_chain_id}":
            raise ValueError("source_domain must equal eip155:<hyperevm_chain_id>")
        if not self.aggregate_position_id:
            raise ValueError("aggregate_position_id must be non-empty")
        _norm_eth_address("reader_contract", self.reader_contract)
        _norm_b256("reader_code_hash", self.reader_code_hash)
        _norm_eth_address("expected_account", self.expected_account)
        prev_perp = -1
        for perp in self.required_perps:
            if perp < 0 or perp > 0xFFFF or perp <= prev_perp:
                raise ValueError("required_perps must be strictly increasing uint16 indices")
            prev_perp = perp
        if not self.allowed_spot_tokens:
            raise ValueError("allowed_spot_tokens must be non-empty")
        prev_tok = -1
        for spot in self.allowed_spot_tokens:
            if spot.token <= prev_tok:
                raise ValueError("allowed_spot_tokens must be strictly increasing")
            if SUPPORTED_SPOT_TOKENS.get(spot.token) != spot.wei_decimals:
                raise ValueError(
                    f"unsupported spot token policy: token={spot.token}, wei_decimals={spot.wei_decimals}"
                )
            prev_tok = spot.token

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> HyperCorePolicy:
        return cls(
            source_domain=str(data["source_domain"]),
            aggregate_position_id=str(data["aggregate_position_id"]),
            hyperevm_chain_id=int(data["hyperevm_chain_id"]),
            reader_contract=str(data["reader_contract"]).lower(),
            reader_code_hash=str(data["reader_code_hash"]).lower(),
            required_perps=tuple(int(p) for p in data["required_perps"]),
            allowed_spot_tokens=tuple(
                SpotTokenPolicy(token=int(item["token"]), wei_decimals=int(item["wei_decimals"]))
                for item in data["allowed_spot_tokens"]
            ),
            expected_account=str(data["expected_account"]).lower(),
        )


@dataclass(frozen=True)
class HyperCoreMarginSummary:
    account_value: int  # int64 usd_e6
    margin_used: int    # uint64 usd_e6
    ntl_pos: int        # uint64 usd_e6
    raw_usd: int        # int64 usd_e6


@dataclass(frozen=True)
class HyperCorePerpSnapshot:
    perp: int
    szi: int
    mark_px: int


@dataclass(frozen=True)
class HyperCoreSpotSnapshot:
    token: int
    total: int
    hold: int
    wei_decimals: int
    price_usd_e8: int


@dataclass(frozen=True)
class HyperCoreSnapshotPayload:
    account: str
    margin_summary: HyperCoreMarginSummary
    withdrawable_usd_e6: int
    perps: tuple[HyperCorePerpSnapshot, ...]
    spots: tuple[HyperCoreSpotSnapshot, ...]


@dataclass(frozen=True)
class HyperCoreVerificationResult:
    reader_contract: str
    reader_code_hash: str
    block_number: int
    block_time_ms: int
    commitment: str
    event_topic0: str
    payload: HyperCoreSnapshotPayload
    spot_locked_usd_e8: int
    spot_unlocked_usd_e8: int
    cash_locked_usd_e8: int
    cash_unlocked_usd_e8: int
    perp_notional_usd_e8: int
    gross_assets_usd_e8: int
    total_liabilities_usd_e8: int
    verified_net_assets_usd_e6: int
    observation_root: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "reader_contract": self.reader_contract,
            "reader_code_hash": self.reader_code_hash,
            "block_number": self.block_number,
            "block_time_ms": self.block_time_ms,
            "commitment": self.commitment,
            "event_topic0": self.event_topic0,
            "account": self.payload.account,
            "spot_locked_usd_e8": self.spot_locked_usd_e8,
            "spot_unlocked_usd_e8": self.spot_unlocked_usd_e8,
            "cash_locked_usd_e8": self.cash_locked_usd_e8,
            "cash_unlocked_usd_e8": self.cash_unlocked_usd_e8,
            "perp_notional_usd_e8": self.perp_notional_usd_e8,
            "gross_assets_usd_e8": self.gross_assets_usd_e8,
            "total_liabilities_usd_e8": self.total_liabilities_usd_e8,
            "verified_net_assets_usd_e6": self.verified_net_assets_usd_e6,
            "observation_root": self.observation_root,
        }


def encode_hypercore_payload(payload: HyperCoreSnapshotPayload) -> bytes:
    """ABI-encode payload identically to ``HyperCoreReserveReader.sol`` line 73."""
    account = _norm_eth_address("payload.account", payload.account)
    margin = (
        int(payload.margin_summary.account_value),
        int(payload.margin_summary.margin_used),
        int(payload.margin_summary.ntl_pos),
        int(payload.margin_summary.raw_usd),
    )
    perps = [(int(p.perp), int(p.szi), int(p.mark_px)) for p in payload.perps]
    spots = [
        (int(s.token), int(s.total), int(s.hold), int(s.wei_decimals), int(s.price_usd_e8))
        for s in payload.spots
    ]
    return abi_encode(
        HYPERCORE_PAYLOAD_ABI_TYPES,
        [account, margin, int(payload.withdrawable_usd_e6), perps, spots],
    )


def decode_hypercore_payload(payload_bytes: bytes) -> HyperCoreSnapshotPayload:
    """Decode ABI payload emitted by ``HyperCoreReserveReader.HyperCoreSnapshot``."""
    account, margin_raw, withdrawable, perps_raw, spots_raw = abi_decode(
        HYPERCORE_PAYLOAD_ABI_TYPES,
        payload_bytes,
    )
    margin = HyperCoreMarginSummary(
        account_value=int(margin_raw[0]),
        margin_used=int(margin_raw[1]),
        ntl_pos=int(margin_raw[2]),
        raw_usd=int(margin_raw[3]),
    )
    perps = tuple(
        HyperCorePerpSnapshot(perp=int(p[0]), szi=int(p[1]), mark_px=int(p[2]))
        for p in perps_raw
    )
    spots = tuple(
        HyperCoreSpotSnapshot(
            token=int(s[0]),
            total=int(s[1]),
            hold=int(s[2]),
            wei_decimals=int(s[3]),
            price_usd_e8=int(s[4]),
        )
        for s in spots_raw
    )
    return HyperCoreSnapshotPayload(
        account=str(account).lower(),
        margin_summary=margin,
        withdrawable_usd_e6=int(withdrawable),
        perps=perps,
        spots=spots,
    )


def compute_snapshot_commitment(payload_bytes: bytes, salt_hex: str) -> str:
    """Compute ``keccak256(abi.encodePacked(payload, salt))`` matching Solidity."""
    salt_norm = _norm_b256("salt", salt_hex, allow_zero=True)
    salt_bytes = bytes.fromhex(salt_norm[2:])
    return "0x" + keccak256(payload_bytes + salt_bytes).hex()


def _token_units_to_usd_e8_floor(amount: int, price_usd_e8: int, wei_decimals: int) -> int:
    unit = 10**wei_decimals
    return (amount * price_usd_e8) // unit


def _snapshot_observation_root(
    *,
    policy: HyperCorePolicy,
    commitment: str,
    block_number: int,
    block_time_ms: int,
    gross_assets_usd_e8: int,
    total_liabilities_usd_e8: int,
) -> str:
    canonical = json.dumps(
        {
            "source_domain": policy.source_domain,
            "reader_contract": policy.reader_contract.lower(),
            "reader_code_hash": policy.reader_code_hash.lower(),
            "account": policy.expected_account.lower(),
            "block_number": block_number,
            "block_time_ms": block_time_ms,
            "commitment": commitment.lower(),
            "gross_assets_usd_e8": gross_assets_usd_e8,
            "total_liabilities_usd_e8": total_liabilities_usd_e8,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.hypercore_snapshot_observation.v1")
    hasher.update(b"\x00")
    hasher.update(canonical)
    return hasher.hexdigest()


def verify_hypercore_snapshot(
    snapshot_receipt: dict[str, Any],
    policy: HyperCorePolicy,
    *,
    now_unix: int | None = None,
    max_snapshot_age_seconds: int = 300,
) -> HyperCoreVerificationResult:
    """Verify a HyperCoreReserveReader receipt log against governed policy.

    Enforces every fail-closed invariant from ``hyperliquid_receipt.rs``:
    - Reader contract address, bytecode hash, and event topic0 match policy.
    - ``keccak256(payload || salt) == commitment``.
    - Account matches policy owner; ``account_value >= 0`` and ``>= withdrawable``.
    - Exact ordered perp set and ``sum(|szi| * mark_px * 100) == ntl_pos * 100``.
    - Exact ordered spot token set, ``hold <= total``, non-zero prices, floor USD-e8 math.
    - Snapshot timestamp freshness when ``now_unix`` is provided.
    """
    policy.validate()

    reader_contract = _norm_eth_address("receipt.reader_contract", snapshot_receipt["reader_contract"])
    if reader_contract != policy.reader_contract.lower():
        raise ValueError("wrong_reader_contract: receipt reader does not match policy")

    reader_code_hash = _norm_b256("receipt.reader_code_hash", snapshot_receipt["reader_code_hash"])
    if reader_code_hash != policy.reader_code_hash.lower():
        raise ValueError("reader_code_hash_mismatch: deployed bytecode hash does not match policy")

    topic0 = _norm_b256("receipt.event_topic0", snapshot_receipt["event_topic0"])
    if topic0[2:] != HL_RECEIPT_EVENT_TOPIC0_HEX:
        raise ValueError("wrong_event_topic: event signature does not match HyperCoreSnapshot")

    commitment = _norm_b256("receipt.commitment", snapshot_receipt["commitment"])
    salt = _norm_b256("receipt.salt", snapshot_receipt["salt"], allow_zero=True)
    payload_hex = str(snapshot_receipt["payload_hex"])
    if payload_hex.startswith("0x"):
        payload_hex = payload_hex[2:]
    if not payload_hex or len(payload_hex) % 2 != 0 or not _HEX_RE.match(payload_hex.lower()):
        raise ValueError("bad_abi: payload_hex must be valid even-length hex")
    payload_bytes = bytes.fromhex(payload_hex)

    recomputed = compute_snapshot_commitment(payload_bytes, salt)
    if recomputed != commitment:
        raise ValueError(f"bad_commitment: expected {recomputed}, got {commitment}")

    block_number = int(snapshot_receipt["block_number"])
    block_time_ms = int(snapshot_receipt["block_time_ms"])
    if block_number <= 0 or block_time_ms <= 0:
        raise ValueError("receipt block_number and block_time_ms must be positive")

    if now_unix is not None:
        snap_unix = block_time_ms // 1000
        if snap_unix > now_unix + 30:
            raise ValueError("snapshot_in_future: block_time_ms exceeds current clock")
        age = now_unix - snap_unix
        if age > max_snapshot_age_seconds:
            raise ValueError(f"snapshot_stale: age {age}s > max {max_snapshot_age_seconds}s")

    payload = decode_hypercore_payload(payload_bytes)
    if payload.account != policy.expected_account.lower():
        raise ValueError("account_mismatch: snapshot account does not match governed reserve owner")
    if payload.margin_summary.account_value < 0:
        raise ValueError("negative_account_value: HyperCore margin accountValue < 0")
    if payload.margin_summary.account_value < payload.withdrawable_usd_e6:
        raise ValueError("withdrawable_exceeds_account_value")

    if len(payload.perps) != len(policy.required_perps):
        raise ValueError("bad_perp_position: perp row count does not match required_perps")
    perp_notional = 0
    for perp_row, expected_perp in zip(payload.perps, policy.required_perps):
        if perp_row.perp != expected_perp:
            raise ValueError(f"bad_perp_position: expected perp {expected_perp}, got {perp_row.perp}")
        if perp_row.szi == 0:
            continue
        if perp_row.mark_px == 0:
            raise ValueError(f"bad_price: zero mark_px on open perp {perp_row.perp}")
        perp_notional += abs(perp_row.szi) * perp_row.mark_px * 100
    account_perp_notional = payload.margin_summary.ntl_pos * 100
    if perp_notional != account_perp_notional:
        raise ValueError(
            f"perp_notional_mismatch: sum={perp_notional} != marginSummary.ntlPos*100={account_perp_notional}"
        )

    if len(payload.spots) != len(policy.allowed_spot_tokens):
        raise ValueError("bad_spot_token: spot row count does not match allowed_spot_tokens")
    seen_tokens: set[int] = set()
    spot_locked = 0
    spot_unlocked = 0
    for spot_row, expected_spot in zip(payload.spots, policy.allowed_spot_tokens):
        if spot_row.token in seen_tokens:
            raise ValueError(f"duplicate_row: duplicate spot token {spot_row.token}")
        seen_tokens.add(spot_row.token)
        if spot_row.token != expected_spot.token or spot_row.wei_decimals != expected_spot.wei_decimals:
            raise ValueError(f"bad_spot_token: token {spot_row.token} violates policy")
        if spot_row.hold > spot_row.total:
            raise ValueError(f"bad_spot_balance: hold {spot_row.hold} > total {spot_row.total}")
        if spot_row.total == 0 and spot_row.hold == 0:
            continue
        if spot_row.price_usd_e8 == 0:
            raise ValueError(f"bad_price: zero price_usd_e8 on non-zero spot token {spot_row.token}")
        unlocked_units = spot_row.total - spot_row.hold
        spot_locked += _token_units_to_usd_e8_floor(spot_row.hold, spot_row.price_usd_e8, spot_row.wei_decimals)
        spot_unlocked += _token_units_to_usd_e8_floor(unlocked_units, spot_row.price_usd_e8, spot_row.wei_decimals)

    cash_unlocked = payload.withdrawable_usd_e6 * 100
    cash_locked = (payload.margin_summary.account_value - payload.withdrawable_usd_e6) * 100
    gross_assets = spot_locked + spot_unlocked + cash_locked + cash_unlocked
    total_liabilities = 0
    verified_net_assets_usd_e6 = (gross_assets - total_liabilities) // 100

    obs_root = _snapshot_observation_root(
        policy=policy,
        commitment=commitment,
        block_number=block_number,
        block_time_ms=block_time_ms,
        gross_assets_usd_e8=gross_assets,
        total_liabilities_usd_e8=total_liabilities,
    )

    return HyperCoreVerificationResult(
        reader_contract=reader_contract,
        reader_code_hash=reader_code_hash,
        block_number=block_number,
        block_time_ms=block_time_ms,
        commitment=commitment,
        event_topic0=topic0,
        payload=payload,
        spot_locked_usd_e8=spot_locked,
        spot_unlocked_usd_e8=spot_unlocked,
        cash_locked_usd_e8=cash_locked,
        cash_unlocked_usd_e8=cash_unlocked,
        perp_notional_usd_e8=perp_notional,
        gross_assets_usd_e8=gross_assets,
        total_liabilities_usd_e8=total_liabilities,
        verified_net_assets_usd_e6=verified_net_assets_usd_e6,
        observation_root=obs_root,
    )


def compute_nav_per_unit(
    verified_net_assets: int,
    circulating_supply: int,
    asset_precision: int,
) -> int:
    """Universal Pillar 7 / C4 exact floor formula: ``floor(V * 10^precision / S)``."""
    if verified_net_assets <= 0:
        raise ValueError("verified_net_assets must be positive")
    if circulating_supply <= 0:
        raise ValueError("circulating_supply must be positive")
    if asset_precision < 0 or asset_precision > 18:
        raise ValueError("asset_precision must be in 0..=18")
    return (verified_net_assets * (10**asset_precision)) // circulating_supply


def nav_reserve_packet_id(asset_id: str, epoch: int, reserve_packet_hash: str) -> str:
    """Mirror of ``postfiat_types::nav_reserve_packet_id``."""
    hasher = hashlib.sha3_384()
    hasher.update(b"postfiat.nav_reserve_packet_id.v1")
    hasher.update(b"\x00")
    hasher.update(
        f"asset_id={asset_id}\nepoch={epoch}\nreserve_packet_hash={reserve_packet_hash}\n".encode("utf-8")
    )
    return hasher.hexdigest()
