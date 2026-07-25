"""Bind unitless PFTL RPC NAV integers to six local finalized ledgers.

The current RPC status exposes ``nav_per_unit`` but not its valuation unit.
For this persistent local handoff, ARMED pricing therefore also reads the
same six validator data directories and requires their finalized NAV asset
records to say ``valuation_unit=usd_1e8`` at the exact RPC height/root/tip.
"""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import threading
from typing import Any, Callable, Mapping

from .pftl_handoff import PersistentPftlHandoff
from .policy import NAV_VALUATION_SCALE, NAV_VALUATION_UNIT


LEDGER_VALUATION_UNIT = "usd_1e8"
MAX_LOCAL_STATE_BYTES = 16 * 1024 * 1024
MAX_STATE_VERIFICATION_BYTES = 256 * 1024
STATE_VERIFICATION_TIMEOUT_SECONDS = 30


class PftlValuationBindingError(ValueError):
    """Local finalized ledger metadata does not bind the RPC NAV semantics."""


def _require_local_directory(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise PftlValuationBindingError(f"{label} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid not in {0, os.geteuid()}
        or metadata.st_mode & stat.S_IWOTH
    ):
        raise PftlValuationBindingError(
            f"{label} must be canonical, trusted, and non-world-writable"
        )


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise PftlValuationBindingError(
                f"duplicate local state JSON field: {key}"
            )
        result[key] = value
    return result


@dataclass(frozen=True)
class _ReadDocument:
    value: Mapping[str, Any]
    sha256: str
    inode: int
    size: int
    mtime_ns: int


def _read_local_state(path: Path, label: str) -> _ReadDocument:
    try:
        before = path.lstat()
    except OSError as error:
        raise PftlValuationBindingError(f"{label} is unavailable") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise PftlValuationBindingError(f"{label} must be regular and non-symlink")
    if before.st_uid not in {0, os.geteuid()}:
        raise PftlValuationBindingError(f"{label} has an untrusted owner")
    if before.st_mode & stat.S_IWOTH:
        raise PftlValuationBindingError(f"{label} must not be world writable")
    if before.st_size < 2 or before.st_size > MAX_LOCAL_STATE_BYTES:
        raise PftlValuationBindingError(f"{label} size is invalid")
    try:
        encoded = path.read_bytes()
        after = path.lstat()
    except OSError as error:
        raise PftlValuationBindingError(f"{label} could not be read") from error
    identity_before = (
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    )
    identity_after = (
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    if identity_before != identity_after or len(encoded) != before.st_size:
        raise PftlValuationBindingError(f"{label} changed while being read")
    try:
        value = json.loads(
            encoded.decode("utf-8"),
            object_pairs_hook=_reject_duplicates,
            parse_constant=lambda item: (_ for _ in ()).throw(
                PftlValuationBindingError(
                    f"non-finite local state JSON value: {item}"
                )
            ),
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise PftlValuationBindingError(f"{label} is not valid JSON") from error
    if not isinstance(value, Mapping):
        raise PftlValuationBindingError(f"{label} must contain a JSON object")
    return _ReadDocument(
        value=value,
        sha256=hashlib.sha256(encoded).hexdigest(),
        inode=before.st_ino,
        size=before.st_size,
        mtime_ns=before.st_mtime_ns,
    )


def _uint(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise PftlValuationBindingError(f"{label} must be uint63 >= {minimum}")
    return value


def _text(value: Any, label: str, *, maximum: int = 4096) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
    ):
        raise PftlValuationBindingError(f"{label} must be bounded ASCII")
    return value


@dataclass(frozen=True)
class PftlValuationEvidence:
    height: int
    block_tip_hash: str
    state_root: str
    asset_id: str
    nav_epoch: int
    nav_per_unit_usd_e8: int
    reserve_packet_hash: str
    valuation_unit: str
    valuation_scale: int
    validator_count: int
    ledger_sha256: tuple[str, ...]
    chain_tip_sha256: tuple[str, ...]
    state_verification_sha256: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.lightning_pftl_valuation_binding.v1",
            "height": self.height,
            "block_tip_hash": self.block_tip_hash,
            "state_root": self.state_root,
            "asset_id": self.asset_id,
            "nav_epoch": self.nav_epoch,
            "nav_per_unit_usd_e8": self.nav_per_unit_usd_e8,
            "reserve_packet_hash": self.reserve_packet_hash,
            "valuation_unit": self.valuation_unit,
            "valuation_scale": self.valuation_scale,
            "validator_count": self.validator_count,
            "ledger_sha256": list(self.ledger_sha256),
            "chain_tip_sha256": list(self.chain_tip_sha256),
            "state_verification_sha256": list(
                self.state_verification_sha256
            ),
        }


class PftlValuationBinding:
    """Read and cryptographically verify six exact finalized validator ledgers."""

    def __init__(
        self,
        handoff: PersistentPftlHandoff,
        *,
        state_verifier: Callable[[int, Path, Any], str] | None = None,
    ) -> None:
        self.handoff = handoff
        self.nodes_root = handoff.data_root / "nodes"
        self._state_verifier = state_verifier or self._verify_node_state
        self._uses_pinned_binary = state_verifier is None
        self._verification_lock = threading.RLock()
        self._cached_key: tuple[Any, ...] | None = None
        self._cached_evidence: PftlValuationEvidence | None = None
        _require_local_directory(handoff.data_root, "PFTL data root")
        _require_local_directory(self.nodes_root, "PFTL nodes root")

    def _verify_binary(self) -> None:
        binary = self.handoff.binary_path
        try:
            metadata = binary.lstat()
            resolved = binary.resolve(strict=True)
        except OSError as error:
            raise PftlValuationBindingError(
                "pinned PFTL verifier binary is unavailable"
            ) from error
        if (
            resolved != binary
            or stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid not in {0, os.geteuid()}
            or metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            or not metadata.st_mode & stat.S_IXUSR
        ):
            raise PftlValuationBindingError(
                "pinned PFTL verifier binary metadata is unsafe"
            )
        digest = hashlib.sha256()
        try:
            with binary.open("rb") as handle:
                while chunk := handle.read(1024 * 1024):
                    digest.update(chunk)
        except OSError as error:
            raise PftlValuationBindingError(
                "pinned PFTL verifier binary could not be read"
            ) from error
        if digest.hexdigest() != self.handoff.binary_sha256:
            raise PftlValuationBindingError(
                "pinned PFTL verifier binary SHA-256 mismatch"
            )

    def _verify_node_state(self, index: int, node: Path, snapshot: Any) -> str:
        """Recompute the ledger root with the exact pinned hardened binary."""

        del index
        try:
            completed = subprocess.run(
                [
                    str(self.handoff.binary_path),
                    "verify-state",
                    "--data-dir",
                    str(node),
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=STATE_VERIFICATION_TIMEOUT_SECONDS,
                env={
                    "PATH": "/usr/bin:/bin",
                    "LANG": "C",
                    "LC_ALL": "C",
                },
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise PftlValuationBindingError(
                "pinned PFTL state verification failed to run"
            ) from error
        if (
            completed.returncode != 0
            or not completed.stdout
            or len(completed.stdout) > MAX_STATE_VERIFICATION_BYTES
            or len(completed.stderr) > MAX_STATE_VERIFICATION_BYTES
        ):
            raise PftlValuationBindingError(
                "pinned PFTL state verification did not succeed"
            )
        try:
            report = json.loads(
                completed.stdout.decode("utf-8"),
                object_pairs_hook=_reject_duplicates,
                parse_constant=lambda item: (_ for _ in ()).throw(
                    PftlValuationBindingError(
                        f"non-finite state verification JSON value: {item}"
                    )
                ),
            )
        except (UnicodeError, json.JSONDecodeError) as error:
            raise PftlValuationBindingError(
                "pinned PFTL state verification returned invalid JSON"
            ) from error
        if not isinstance(report, Mapping):
            raise PftlValuationBindingError(
                "pinned PFTL state verification returned no object"
            )
        block_log = report.get("block_log")
        if (
            report.get("schema") != "postfiat-state-verification-v1"
            or report.get("verified") is not True
            or report.get("chain_id") != self.handoff.chain_id
            or report.get("genesis_hash") != self.handoff.genesis_hash
            or report.get("protocol_version") != 1
            or not isinstance(block_log, Mapping)
            or block_log.get("verified") is not True
            or _uint(block_log.get("block_count"), "verified block count")
            != snapshot.height
            or block_log.get("tip_hash") != snapshot.block_tip_hash
            or block_log.get("state_root") != snapshot.state_root
        ):
            raise PftlValuationBindingError(
                "pinned PFTL state verification does not match the six-RPC snapshot"
            )
        return hashlib.sha256(completed.stdout).hexdigest()

    def _node_view(self, index: int, snapshot: Any) -> dict[str, Any]:
        node = self.nodes_root / f"validator-{index}"
        _require_local_directory(node, f"validator-{index} data directory")
        tip_path = node / "chain_tip.json"
        ledger_path = node / "ledger.json"
        tip_before = _read_local_state(
            tip_path,
            f"validator-{index} chain tip",
        )
        ledger = _read_local_state(
            ledger_path,
            f"validator-{index} ledger",
        )
        tip_after = _read_local_state(
            tip_path,
            f"validator-{index} chain tip",
        )
        if tip_before.sha256 != tip_after.sha256:
            raise PftlValuationBindingError(
                f"validator-{index} advanced while valuation metadata was read"
            )
        tip = tip_before.value
        if (
            tip.get("schema") != "postfiat-chain-tip-v1"
            or tip.get("chain_id") != self.handoff.chain_id
            or tip.get("genesis_hash") != self.handoff.genesis_hash
            or _uint(tip.get("height"), "chain tip height") != snapshot.height
            or tip.get("block_hash") != snapshot.block_tip_hash
            or tip.get("state_root") != snapshot.state_root
        ):
            raise PftlValuationBindingError(
                f"validator-{index} local tip does not match the six-RPC snapshot"
            )
        nav_assets = ledger.value.get("nav_assets")
        if not isinstance(nav_assets, list):
            raise PftlValuationBindingError(
                f"validator-{index} ledger has no NAV asset registry"
            )
        matches = [
            value
            for value in nav_assets
            if isinstance(value, Mapping)
            and value.get("asset_id") == self.handoff.asset_id
        ]
        if len(matches) != 1:
            raise PftlValuationBindingError(
                f"validator-{index} ledger lacks one pinned NAV asset"
            )
        asset = matches[0]
        if (
            asset.get("issuer") != self.handoff.asset_issuer
            or asset.get("proof_profile") != self.handoff.profile_id
            or asset.get("valuation_unit") != LEDGER_VALUATION_UNIT
            or _uint(asset.get("finalized_epoch"), "NAV finalized epoch", minimum=1)
            != self.handoff.nav_epoch
            or _uint(asset.get("nav_per_unit"), "NAV per unit", minimum=1)
            != self.handoff.nav_per_unit_usd_e8
            or asset.get("finalized_reserve_packet_hash")
            != self.handoff.reserve_packet_hash
            or _uint(
                asset.get("circulating_supply"),
                "NAV circulating supply",
                minimum=1,
            )
            != self.handoff.circulating_supply_atoms
            or asset.get("halted") is not False
            or _uint(
                asset.get("finalized_at_height"),
                "NAV finalized height",
                minimum=1,
            )
            > snapshot.height
        ):
            raise PftlValuationBindingError(
                f"validator-{index} NAV valuation metadata mismatches the USD-e8 pin"
            )
        return {
            "height": snapshot.height,
            "tip": snapshot.block_tip_hash,
            "root": snapshot.state_root,
            "asset": {
                "asset_id": self.handoff.asset_id,
                "issuer": self.handoff.asset_issuer,
                "proof_profile": self.handoff.profile_id,
                "valuation_unit": LEDGER_VALUATION_UNIT,
                "finalized_epoch": self.handoff.nav_epoch,
                "nav_per_unit": self.handoff.nav_per_unit_usd_e8,
                "reserve_packet_hash": self.handoff.reserve_packet_hash,
                "circulating_supply": self.handoff.circulating_supply_atoms,
                "halted": False,
            },
            "ledger_sha256": ledger.sha256,
            "tip_sha256": tip_before.sha256,
        }

    def verify(self, snapshot: Any) -> PftlValuationEvidence:
        with self._verification_lock:
            if (
                getattr(snapshot, "agreeing_validator_count", None) != 6
                or getattr(snapshot, "validator_count", None) != 6
                or getattr(snapshot, "asset_id", None) != self.handoff.asset_id
                or getattr(snapshot, "nav_epoch", None) != self.handoff.nav_epoch
                or getattr(snapshot, "nav_per_unit", None)
                != self.handoff.nav_per_unit_usd_e8
                or getattr(snapshot, "nav_reserve_packet_hash", None)
                != self.handoff.reserve_packet_hash
            ):
                raise PftlValuationBindingError(
                    "six-RPC snapshot does not match the pinned USD-e8 NAV"
                )
            rows = [self._node_view(index, snapshot) for index in range(6)]
            canonical = {
                json.dumps(
                    {
                        "height": row["height"],
                        "tip": row["tip"],
                        "root": row["root"],
                        "asset": row["asset"],
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=True,
                    allow_nan=False,
                )
                for row in rows
            }
            if len(canonical) != 1:
                raise PftlValuationBindingError(
                    "six local valuation ledgers are not converged"
                )
            cache_key = (
                snapshot.height,
                snapshot.block_tip_hash,
                snapshot.state_root,
                *(row["ledger_sha256"] for row in rows),
                *(row["tip_sha256"] for row in rows),
            )
            if (
                cache_key == self._cached_key
                and self._cached_evidence is not None
            ):
                return self._cached_evidence
            if self._uses_pinned_binary:
                self._verify_binary()
            nodes = [
                self.nodes_root / f"validator-{index}" for index in range(6)
            ]
            with ThreadPoolExecutor(
                max_workers=6,
                thread_name_prefix="pftl-state-verify",
            ) as executor:
                futures = [
                    executor.submit(
                        self._state_verifier,
                        index,
                        node,
                        snapshot,
                    )
                    for index, node in enumerate(nodes)
                ]
                state_verification_sha256 = tuple(
                    future.result() for future in futures
                )
            for index, row in enumerate(rows):
                tip = _read_local_state(
                    nodes[index] / "chain_tip.json",
                    f"validator-{index} chain tip",
                )
                ledger = _read_local_state(
                    nodes[index] / "ledger.json",
                    f"validator-{index} ledger",
                )
                if (
                    tip.sha256 != row["tip_sha256"]
                    or ledger.sha256 != row["ledger_sha256"]
                ):
                    raise PftlValuationBindingError(
                        f"validator-{index} advanced during state verification"
                    )
            evidence = PftlValuationEvidence(
                height=snapshot.height,
                block_tip_hash=snapshot.block_tip_hash,
                state_root=snapshot.state_root,
                asset_id=self.handoff.asset_id,
                nav_epoch=self.handoff.nav_epoch,
                nav_per_unit_usd_e8=self.handoff.nav_per_unit_usd_e8,
                reserve_packet_hash=self.handoff.reserve_packet_hash,
                valuation_unit=NAV_VALUATION_UNIT,
                valuation_scale=NAV_VALUATION_SCALE,
                validator_count=6,
                ledger_sha256=tuple(
                    row["ledger_sha256"] for row in rows
                ),
                chain_tip_sha256=tuple(
                    row["tip_sha256"] for row in rows
                ),
                state_verification_sha256=state_verification_sha256,
            )
            self._cached_key = cache_key
            self._cached_evidence = evidence
            return evidence
