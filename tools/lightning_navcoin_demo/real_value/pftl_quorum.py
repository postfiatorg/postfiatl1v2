"""Independent six-RPC observer for the persistent hardened PFTL handoff."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import json
from pathlib import Path
import sys
from typing import Any, Callable, Mapping, Sequence

from ..coordinator.protocol import SecretPreimage, encode_fulfillment
from .policy import (
    NAV_VALUATION_SCALE,
    NAV_VALUATION_UNIT,
    RealValuePolicy,
    RealValuePolicyError,
)


class PftlQuorumError(RealValuePolicyError):
    """The persistent PFTL chain is absent, divergent, or mismatched."""


@dataclass(frozen=True)
class PftlRouteSnapshot:
    height: int
    block_tip_hash: str
    state_root: str
    agreeing_validator_count: int
    validator_count: int
    build_git_revision: str
    asset_id: str
    asset_precision: int
    nav_epoch: int
    nav_per_unit: int
    nav_reserve_packet_hash: str
    coordinator_inventory_atoms: int
    coordinator_trustline_limit_atoms: int
    coordinator_receive_headroom_atoms: int
    coordinator_native_balance: int
    user_balance_atoms: int
    user_trustline_limit_atoms: int
    user_receive_headroom_atoms: int
    user_native_balance: int
    asset_freeze_enabled: bool
    asset_clawback_enabled: bool
    asset_requires_authorization: bool
    nav_valuation_unit: str = NAV_VALUATION_UNIT
    nav_valuation_scale: int = NAV_VALUATION_SCALE

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.lightning_pftl_route_snapshot.v1",
            "height": self.height,
            "block_tip_hash": self.block_tip_hash,
            "state_root": self.state_root,
            "agreeing_validator_count": self.agreeing_validator_count,
            "validator_count": self.validator_count,
            "build_git_revision": self.build_git_revision,
            "asset_id": self.asset_id,
            "asset_precision": self.asset_precision,
            "nav_epoch": self.nav_epoch,
            "nav_per_unit": self.nav_per_unit,
            "nav_per_unit_usd_e8": self.nav_per_unit,
            "nav_valuation_unit": self.nav_valuation_unit,
            "nav_valuation_scale": self.nav_valuation_scale,
            "nav_reserve_packet_hash": self.nav_reserve_packet_hash,
            "coordinator_inventory_atoms": self.coordinator_inventory_atoms,
            "coordinator_trustline_limit_atoms": self.coordinator_trustline_limit_atoms,
            "coordinator_receive_headroom_atoms": self.coordinator_receive_headroom_atoms,
            "coordinator_native_balance": self.coordinator_native_balance,
            "user_balance_atoms": self.user_balance_atoms,
            "user_trustline_limit_atoms": self.user_trustline_limit_atoms,
            "user_receive_headroom_atoms": self.user_receive_headroom_atoms,
            "user_native_balance": self.user_native_balance,
            "asset_freeze_enabled": self.asset_freeze_enabled,
            "asset_clawback_enabled": self.asset_clawback_enabled,
            "asset_requires_authorization": self.asset_requires_authorization,
        }


def default_client_factory(endpoint: str) -> Any:
    """Import the repository's stdlib RPC client without embedding credentials."""

    repo_root = Path(__file__).resolve().parents[3]
    python_root = repo_root / "python"
    if str(python_root) not in sys.path:
        sys.path.insert(0, str(python_root))
    from postfiat_rpc.client import PostFiatRpcClient

    # Audited transaction reads replay the finalized block log.  The
    # persistent six-validator demo can legitimately take longer than the
    # ordinary state-read budget once its block archive grows, so allow the
    # read to finish rather than misclassifying a committed effect as absent.
    # This changes transport patience only; all six identity, inclusion, and
    # receipt checks below remain mandatory.
    return PostFiatRpcClient(endpoint, timeout_seconds=30)


def _uint(value: Any, name: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise PftlQuorumError(f"{name} must be uint63 >= {minimum}")
    return value


def _text(value: Any, name: str) -> str:
    if type(value) is not str or not value or len(value) > 4096:
        raise PftlQuorumError(f"{name} must be a nonempty bounded string")
    return value


def _asset_body(report: Any) -> Mapping[str, Any]:
    if not isinstance(report, Mapping):
        raise PftlQuorumError("asset_info response is not an object")
    body = report.get("asset")
    if not isinstance(body, Mapping):
        raise PftlQuorumError("asset_info has no asset")
    return body


def _matching_line(report: Any, asset_id: str) -> Mapping[str, Any]:
    if not isinstance(report, Mapping) or not isinstance(report.get("lines"), list):
        raise PftlQuorumError("account_lines response is malformed")
    lines = [
        line
        for line in report["lines"]
        if isinstance(line, Mapping) and line.get("asset_id") == asset_id
    ]
    if len(lines) != 1:
        raise PftlQuorumError("coordinator must have exactly one NAVcoin trustline")
    return lines[0]


class PftlQuorumObserver:
    """Read only.  Signing/submission lives behind a separate backend boundary."""

    def __init__(
        self,
        policy: RealValuePolicy,
        *,
        client_factory: Callable[[str], Any] = default_client_factory,
    ) -> None:
        self.policy = policy
        self._clients = tuple(
            client_factory(endpoint) for endpoint in policy.pftl_rpc_endpoints
        )

    def _read_route(self, client: Any) -> dict[str, Any]:
        status_before = client.status()
        asset = client.asset_info(self.policy.pftl_asset_id)
        lines = client.account_lines(
            self.policy.coordinator_pftl_address,
            asset_id=self.policy.pftl_asset_id,
            limit=8,
        )
        native = client.account(self.policy.coordinator_pftl_address)
        user_lines = client.account_lines(
            self.policy.pftl_user_address,
            asset_id=self.policy.pftl_asset_id,
            limit=8,
        )
        user_native = client.account(self.policy.pftl_user_address)
        status_after = client.status()
        return {
            "status_before": status_before,
            "status_after": status_after,
            "asset": asset,
            "lines": lines,
            "native": native,
            "user_lines": user_lines,
            "user_native": user_native,
        }

    def _validate_nav_profile(self, status: Mapping[str, Any]) -> Mapping[str, Any]:
        profiles = status.get("active_nav_profiles")
        if not isinstance(profiles, list):
            raise PftlQuorumError("PFTL status has no active NAV profiles")
        matches = [
            profile
            for profile in profiles
            if isinstance(profile, Mapping)
            and profile.get("asset_id") == self.policy.pftl_asset_id
        ]
        if len(matches) != 1:
            raise PftlQuorumError("PFTL status has no unique pinned NAV profile")
        profile = matches[0]
        raw_nav = _uint(profile.get("nav_per_unit"), "nav_per_unit", minimum=1)
        if (
            self.policy.pftl_nav_valuation_unit != NAV_VALUATION_UNIT
            or self.policy.pftl_nav_valuation_scale != NAV_VALUATION_SCALE
            or raw_nav != self.policy.pftl_nav_per_unit_usd_e8
            or profile.get("finalized_epoch") != self.policy.pftl_nav_epoch
            or profile.get("finalized_reserve_packet_hash")
            != self.policy.pftl_nav_reserve_packet_hash
            or profile.get("halted") is not False
        ):
            raise PftlQuorumError(
                "proven NAV value, USD-e8 unit, epoch, or hash does not match policy"
            )
        return profile

    def _validate_status_identity(
        self,
        status: Any,
        node_ids: set[str],
    ) -> dict[str, Any]:
        if not isinstance(status, Mapping):
            raise PftlQuorumError("PFTL status is malformed")
        if (
            status.get("chain_id") != self.policy.pftl_chain_id
            or status.get("genesis_hash") != self.policy.pftl_genesis_hash
            or status.get("validator_count") != 6
        ):
            raise PftlQuorumError("PFTL chain identity does not match policy")
        if (
            status.get("build_git_revision")
            != self.policy.pftl_build_git_revision
        ):
            raise PftlQuorumError(
                "PFTL build revision does not match the pinned hardened release"
            )
        node_id = _text(status.get("node_id"), "PFTL node_id")
        if node_id in node_ids:
            raise PftlQuorumError("PFTL views are not from distinct validators")
        node_ids.add(node_id)
        if str(status.get("status", "")).lower() not in {
            "running",
            "active",
            "validator",
        }:
            raise PftlQuorumError("PFTL validator service is not active")
        profile = self._validate_nav_profile(status)
        return {
            "height": _uint(status.get("block_height"), "block_height"),
            "tip": _text(status.get("block_tip_hash"), "block_tip_hash"),
            "root": _text(status.get("state_root"), "state_root"),
            "node_id": node_id,
            "build_git_revision": self.policy.pftl_build_git_revision,
            "nav_profile": dict(profile),
        }

    def _validate_status_sandwich(
        self,
        status_before: Any,
        status_after: Any,
        node_ids: set[str],
        *,
        read_label: str,
    ) -> dict[str, Any]:
        """Bind a bundle of state reads to one unchanged finalized view."""

        before = self._validate_status_identity(status_before, node_ids)
        after = self._validate_status_identity(status_after, set())
        stable_fields = (
            "height",
            "tip",
            "root",
            "node_id",
            "build_git_revision",
            "nav_profile",
        )
        if any(before[field] != after[field] for field in stable_fields):
            raise PftlQuorumError(
                f"PFTL {read_label} finalized view changed during RPC read"
            )
        return before

    @staticmethod
    def _canonical_route_view(row: Mapping[str, Any]) -> str:
        return json.dumps(
            row,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )

    def route_snapshot(self) -> PftlRouteSnapshot:
        """Require exact six-of-six identity, root, asset, NAV, and inventory."""

        try:
            rows = [self._read_route(client) for client in self._clients]
        except Exception as error:
            raise PftlQuorumError("one or more PFTL RPC reads failed") from error
        if len(rows) != self.policy.minimum_pftl_validators:
            raise PftlQuorumError("PFTL observer is not configured for six validators")

        node_ids: set[str] = set()
        normalized: list[dict[str, Any]] = []
        for row in rows:
            status_before = row["status_before"]
            status_after = row["status_after"]
            identity = self._validate_status_sandwich(
                status_before,
                status_after,
                node_ids,
                read_label="route",
            )
            if (
                _uint(
                    status_before.get("mempool_pending", 0),
                    "mempool_pending",
                )
                != 0
                or _uint(
                    status_after.get("mempool_pending", 0),
                    "mempool_pending",
                )
                != 0
            ):
                raise PftlQuorumError("PFTL mempool must be empty for real-value preflight")

            profile = identity["nav_profile"]

            asset = _asset_body(row["asset"])
            if asset.get("asset_id") != self.policy.pftl_asset_id:
                raise PftlQuorumError("asset_info returned a different asset")
            asset_precision = _uint(asset.get("precision"), "asset precision")
            if asset_precision != self.policy.pftl_asset_precision:
                raise PftlQuorumError(
                    "asset precision does not match pinned policy"
                )
            flags = (
                asset.get("freeze_enabled"),
                asset.get("clawback_enabled"),
                asset.get("requires_authorization"),
            )
            if self.policy.require_non_freezable and flags != (False, False, False):
                raise PftlQuorumError(
                    "asset controls could block delivery after Lightning settlement"
                )

            line = _matching_line(row["lines"], self.policy.pftl_asset_id)
            if line.get("authorized") is not True or line.get("frozen") is not False:
                raise PftlQuorumError("coordinator NAVcoin trustline is not movable")
            inventory = _uint(line.get("balance"), "coordinator inventory", minimum=1)
            trustline_limit = _uint(
                line.get("limit"), "coordinator trustline limit", minimum=inventory
            )
            native = row["native"]
            if not isinstance(native, Mapping):
                raise PftlQuorumError("coordinator native account is malformed")
            native_balance = _uint(
                native.get("balance"), "coordinator native balance", minimum=1
            )
            user_line = _matching_line(
                row["user_lines"], self.policy.pftl_asset_id
            )
            if (
                user_line.get("authorized") is not True
                or user_line.get("frozen") is not False
            ):
                raise PftlQuorumError("pinned user NAVcoin trustline is not movable")
            user_balance = _uint(
                user_line.get("balance"), "pinned user asset balance"
            )
            user_limit = _uint(
                user_line.get("limit"),
                "pinned user trustline limit",
                minimum=user_balance,
            )
            user_native = row["user_native"]
            if (
                not isinstance(user_native, Mapping)
                or user_native.get("address") != self.policy.pftl_user_address
            ):
                raise PftlQuorumError("pinned user native account is malformed")
            user_native_balance = _uint(
                user_native.get("balance"),
                "pinned user native balance",
                minimum=1,
            )

            normalized.append(
                {
                    "height": identity["height"],
                    "tip": identity["tip"],
                    "root": identity["root"],
                    "build_git_revision": identity["build_git_revision"],
                    "profile": dict(profile),
                    "asset": dict(asset),
                    "asset_precision": asset_precision,
                    "line": dict(line),
                    "native": dict(native),
                    "user_line": dict(user_line),
                    "user_native": dict(user_native),
                    "inventory": inventory,
                    "trustline_limit": trustline_limit,
                    "receive_headroom": trustline_limit - inventory,
                    "native_balance": native_balance,
                    "user_balance": user_balance,
                    "user_trustline_limit": user_limit,
                    "user_receive_headroom": user_limit - user_balance,
                    "user_native_balance": user_native_balance,
                }
            )

        canonical = [self._canonical_route_view(row) for row in normalized]
        if len(set(canonical)) != 1:
            raise PftlQuorumError("PFTL route views are not converged six-of-six")
        agreed = normalized[0]
        profile = agreed["profile"]
        asset = agreed["asset"]
        return PftlRouteSnapshot(
            height=agreed["height"],
            block_tip_hash=agreed["tip"],
            state_root=agreed["root"],
            agreeing_validator_count=6,
            validator_count=6,
            build_git_revision=agreed["build_git_revision"],
            asset_id=self.policy.pftl_asset_id,
            asset_precision=agreed["asset_precision"],
            nav_epoch=profile["finalized_epoch"],
            nav_per_unit=profile["nav_per_unit"],
            nav_reserve_packet_hash=profile["finalized_reserve_packet_hash"],
            coordinator_inventory_atoms=agreed["inventory"],
            coordinator_trustline_limit_atoms=agreed["trustline_limit"],
            coordinator_receive_headroom_atoms=agreed["receive_headroom"],
            coordinator_native_balance=agreed["native_balance"],
            user_balance_atoms=agreed["user_balance"],
            user_trustline_limit_atoms=agreed["user_trustline_limit"],
            user_receive_headroom_atoms=agreed["user_receive_headroom"],
            user_native_balance=agreed["user_native_balance"],
            asset_freeze_enabled=bool(asset["freeze_enabled"]),
            asset_clawback_enabled=bool(asset["clawback_enabled"]),
            asset_requires_authorization=bool(asset["requires_authorization"]),
        )

    def _escrow_state(
        self,
        escrow_id: str,
        *,
        expected: Mapping[str, Any],
        required_state: str,
    ) -> dict[str, Any]:
        """Require one exact escrow state from all six finalized views."""

        required = frozenset(
            {
                "owner",
                "recipient",
                "asset_id",
                "amount",
                "condition_hash",
                "finish_after",
                "cancel_after",
            }
        )
        if frozenset(expected.keys()) != required:
            raise PftlQuorumError("escrow expectation field set mismatch")
        if required_state not in {"open", "finished", "canceled"}:
            raise PftlQuorumError("unsupported escrow state expectation")
        try:
            rows = [
                {
                    "status_before": client.status(),
                    "escrow": client.escrow_info(escrow_id),
                    "status_after": client.status(),
                }
                for client in self._clients
            ]
        except Exception as error:
            raise PftlQuorumError("PFTL escrow quorum read failed") from error
        views: list[dict[str, Any]] = []
        node_ids: set[str] = set()
        for row in rows:
            report = row["escrow"]
            if (
                not isinstance(row["status_before"], Mapping)
                or not isinstance(row["status_after"], Mapping)
                or not isinstance(report, Mapping)
            ):
                raise PftlQuorumError("PFTL escrow quorum response is malformed")
            identity = self._validate_status_sandwich(
                row["status_before"],
                row["status_after"],
                node_ids,
                read_label=f"{required_state} escrow",
            )
            escrow = report.get("escrow")
            if not isinstance(escrow, Mapping) or escrow.get("escrow_id") != escrow_id:
                raise PftlQuorumError("open escrow is absent")
            if escrow.get("state") != required_state:
                raise PftlQuorumError(
                    f"escrow is not {required_state}"
                )
            for field, value in expected.items():
                if escrow.get(field) != value:
                    raise PftlQuorumError(f"escrow {field} does not match quote")
            views.append(
                {
                    "height": identity["height"],
                    "tip": identity["tip"],
                    "root": identity["root"],
                    "escrow": dict(escrow),
                }
            )
        if len({self._canonical_route_view(view) for view in views}) != 1:
            raise PftlQuorumError("open escrow is not converged six-of-six")
        return {
            "schema": "postfiat.lightning_pftl_escrow_quorum.v1",
            "escrow_id": escrow_id,
            "state": required_state,
            "height": views[0]["height"],
            "block_tip_hash": views[0]["tip"],
            "state_root": views[0]["root"],
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "escrow": views[0]["escrow"],
        }

    def open_escrow(
        self,
        escrow_id: str,
        *,
        expected: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Require the exact open lock from all six finalized views."""

        return self._escrow_state(
            escrow_id,
            expected=expected,
            required_state="open",
        )

    def finished_escrow(
        self,
        escrow_id: str,
        *,
        expected: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Require the exact finished lock from all six finalized views."""

        return self._escrow_state(
            escrow_id,
            expected=expected,
            required_state="finished",
        )

    def canceled_escrow(
        self,
        escrow_id: str,
        *,
        expected: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Require the exact canceled lock from all six finalized views."""

        return self._escrow_state(
            escrow_id,
            expected=expected,
            required_state="canceled",
        )

    def user_finish_capacity(
        self,
        escrow_id: str,
        *,
        expected: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Pin the open escrow and the user's exact finish capacity six-of-six."""

        if expected.get("recipient") != self.policy.pftl_user_address:
            raise PftlQuorumError("escrow recipient is not the pinned demo user")
        dummy_fulfillment = encode_fulfillment(SecretPreimage(bytes(32)))
        operation = {
            "operation": "escrow_finish",
            "escrow_id": escrow_id,
            "owner": expected.get("owner"),
            "recipient": self.policy.pftl_user_address,
            # PREIMAGE-SHA-256 fulfillment size is fixed. This known dummy is
            # used only to quote byte weight; it cannot satisfy the escrow.
            "fulfillment": dummy_fulfillment,
        }
        rows: list[dict[str, Any]] = []
        node_ids: set[str] = set()
        try:
            for client in self._clients:
                status_before = client.status()
                escrow_report = client.escrow_info(escrow_id)
                lines = client.account_lines(
                    self.policy.pftl_user_address,
                    asset_id=self.policy.pftl_asset_id,
                    limit=8,
                )
                native = client.account(self.policy.pftl_user_address)
                if not isinstance(native, Mapping):
                    raise PftlQuorumError("pinned user native account is malformed")
                sequence = _uint(
                    native.get("sequence"), "pinned user sequence"
                )
                if sequence == (1 << 63) - 1:
                    raise PftlQuorumError("pinned user sequence is exhausted")
                fee_quote = client.escrow_fee_quote(
                    self.policy.pftl_user_address,
                    operation,
                    sequence=sequence + 1,
                )
                status_after = client.status()
                identity = self._validate_status_sandwich(
                    status_before,
                    status_after,
                    node_ids,
                    read_label="finish capacity",
                )
                escrow = (
                    escrow_report.get("escrow")
                    if isinstance(escrow_report, Mapping)
                    else None
                )
                if (
                    not isinstance(escrow, Mapping)
                    or escrow.get("escrow_id") != escrow_id
                    or escrow.get("state") != "open"
                ):
                    raise PftlQuorumError("pinned user escrow is not open")
                for field, value in expected.items():
                    if escrow.get(field) != value:
                        raise PftlQuorumError(
                            f"pinned user escrow {field} does not match quote"
                        )
                line = _matching_line(lines, self.policy.pftl_asset_id)
                if (
                    line.get("authorized") is not True
                    or line.get("frozen") is not False
                ):
                    raise PftlQuorumError(
                        "pinned user NAVcoin trustline is not movable"
                    )
                balance = _uint(line.get("balance"), "pinned user asset balance")
                limit = _uint(
                    line.get("limit"),
                    "pinned user trustline limit",
                    minimum=balance,
                )
                amount = _uint(expected.get("amount"), "escrow amount", minimum=1)
                if limit - balance < amount:
                    raise PftlQuorumError(
                        "pinned user lacks NAVcoin finish headroom"
                    )
                native_balance = _uint(
                    native.get("balance"),
                    "pinned user native balance",
                )
                if native.get("address") != self.policy.pftl_user_address:
                    raise PftlQuorumError("pinned user native address mismatches")
                if not isinstance(fee_quote, Mapping):
                    raise PftlQuorumError("PFTL finish fee quote is malformed")
                minimum_fee = _uint(
                    fee_quote.get("minimum_fee"),
                    "PFTL finish minimum fee",
                    minimum=1,
                )
                account_reserve = _uint(
                    fee_quote.get("account_reserve"),
                    "PFTL account reserve",
                )
                if (
                    fee_quote.get("schema") != "postfiat-escrow-fee-quote-v1"
                    or fee_quote.get("chain_id") != self.policy.pftl_chain_id
                    or fee_quote.get("genesis_hash")
                    != self.policy.pftl_genesis_hash
                    or fee_quote.get("source") != self.policy.pftl_user_address
                    or fee_quote.get("transaction_kind") != "escrow_finish"
                    or fee_quote.get("sequence") != sequence + 1
                    or fee_quote.get("sender_sequence") != sequence
                    or fee_quote.get("sender_balance") != native_balance
                    or fee_quote.get("mempool_pending_for_sender") != 0
                    or fee_quote.get("sender_meets_reserve_after_fee") is not True
                    or fee_quote.get("operation") != operation
                    or native_balance < minimum_fee + account_reserve
                ):
                    raise PftlQuorumError(
                        "pinned user lacks an exact executable finish fee quote"
                    )
                rows.append(
                    {
                        "height": identity["height"],
                        "tip": identity["tip"],
                        "root": identity["root"],
                        "escrow": dict(escrow),
                        "line": dict(line),
                        "native": dict(native),
                        "fee_quote": dict(fee_quote),
                    }
                )
        except PftlQuorumError:
            raise
        except Exception as error:
            raise PftlQuorumError(
                "pinned user finish-capacity quorum read failed"
            ) from error
        if len(rows) != 6 or len(
            {self._canonical_route_view(row) for row in rows}
        ) != 1:
            raise PftlQuorumError(
                "pinned user finish capacity is not converged six-of-six"
            )
        agreed = rows[0]
        return {
            "schema": "postfiat.lightning_pftl_finish_capacity.v1",
            "escrow_id": escrow_id,
            "state": "open",
            "height": agreed["height"],
            "block_tip_hash": agreed["tip"],
            "state_root": agreed["root"],
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "escrow": agreed["escrow"],
            "recipient_asset_balance": agreed["line"]["balance"],
            "recipient_asset_headroom": (
                agreed["line"]["limit"] - agreed["line"]["balance"]
            ),
            "recipient_native_balance": agreed["native"]["balance"],
            "finish_minimum_fee": agreed["fee_quote"]["minimum_fee"],
            "account_reserve": agreed["fee_quote"]["account_reserve"],
        }

    def receipt(self, tx_id: str) -> dict[str, Any]:
        """Read one inclusion-bound finalized receipt, six-of-six.

        The receipts RPC is a current-state sidecar and does not identify the
        block that applied the transaction.  The audited tx RPC replays the
        block log and returns the transaction's inclusion block.  Use that
        block identity here so a later, unrelated finalized transition cannot
        silently rebind an old receipt to the current state root.
        """

        try:
            def read_finality(client: Any) -> dict[str, Any]:
                return {
                    "status_before": client.status(),
                    "finality": client.tx(tx_id, audit_block_log=True),
                    "status_after": client.status(),
                }

            with ThreadPoolExecutor(
                max_workers=len(self._clients),
                thread_name_prefix="pftl-receipt",
            ) as executor:
                rows = list(executor.map(read_finality, self._clients))
        except Exception as error:
            raise PftlQuorumError("PFTL receipt quorum read failed") from error
        normalized: list[dict[str, Any]] = []
        node_ids: set[str] = set()
        for row in rows:
            identity = self._validate_status_sandwich(
                row["status_before"],
                row["status_after"],
                node_ids,
                read_label="receipt",
            )
            finality = row["finality"]
            if not isinstance(finality, Mapping):
                raise PftlQuorumError("PFTL transaction finality is malformed")
            block = finality.get("block")
            header = block.get("header") if isinstance(block, Mapping) else None
            receipt_ids = (
                block.get("receipt_ids") if isinstance(block, Mapping) else None
            )
            receipt = finality.get("receipt")
            if (
                finality.get("schema") != "postfiat-tx-finality-v1"
                or finality.get("tx_id") != tx_id
                or finality.get("confirmed") is not True
                or finality.get("block_log_verified") is not True
                or finality.get("verification_mode") != "full-block-replay"
                or finality.get("chain_id") != self.policy.pftl_chain_id
                or finality.get("genesis_hash") != self.policy.pftl_genesis_hash
                or finality.get("protocol_version") != 1
                or not isinstance(header, Mapping)
                or not isinstance(receipt, Mapping)
                or receipt.get("tx_id") != tx_id
                or not isinstance(receipt_ids, Sequence)
                or isinstance(receipt_ids, (str, bytes, bytearray))
                or list(receipt_ids).count(tx_id) != 1
                or finality.get("receipt_count") != len(receipt_ids)
                or finality.get("receipt_index") != list(receipt_ids).index(tx_id)
            ):
                raise PftlQuorumError(
                    "PFTL transaction lacks exact full-block finality"
                )
            inclusion_height = _uint(
                header.get("height"), "receipt inclusion height", minimum=1
            )
            inclusion_tip = _text(
                header.get("block_hash"), "receipt inclusion block hash"
            )
            inclusion_root = _text(
                header.get("state_root"), "receipt inclusion state root"
            )
            if (
                inclusion_height > identity["height"]
                or (
                    inclusion_height == identity["height"]
                    and (
                        inclusion_tip != identity["tip"]
                        or inclusion_root != identity["root"]
                    )
                )
            ):
                raise PftlQuorumError(
                    "PFTL transaction inclusion is not on the finalized view"
                )
            normalized.append(
                {
                    "height": inclusion_height,
                    "tip": inclusion_tip,
                    "root": inclusion_root,
                    "accepted": receipt.get("accepted"),
                    "code": receipt.get("code"),
                    "receipt_count": len(receipt_ids),
                }
            )
        if len({self._canonical_route_view(row) for row in normalized}) != 1:
            raise PftlQuorumError(
                "PFTL receipt inclusion views are not converged six-of-six"
            )
        return {
            "schema": "postfiat.lightning_pftl_receipt_quorum.v1",
            "tx_id": tx_id,
            "accepted": normalized[0]["accepted"],
            "code": normalized[0]["code"],
            "height": normalized[0]["height"],
            "block_tip_hash": normalized[0]["tip"],
            "state_root": normalized[0]["root"],
            "receipt_count": normalized[0]["receipt_count"],
            "verification_mode": "full-block-replay",
            "agreeing_validator_count": 6,
            "validator_count": 6,
        }
