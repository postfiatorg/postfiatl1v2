"""Coordinator-enforced cross-clock timeout ordering for XRP/NAVcoin swaps.

XRPL expiration is wall-clock-like Ripple time derived from validated-ledger
close time. PFTL expiration is block height. No protocol proof binds those two
clocks, so these checks are an explicit coordinator trust boundary, not a
trustless cross-chain guarantee.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


MAX_U64 = (1 << 64) - 1


class TimingGateError(ValueError):
    """The swap cannot safely proceed under the configured timing policy."""


class Direction(str, Enum):
    XRP_TO_NAV = "xrp_to_nav"
    NAV_TO_XRP = "nav_to_xrp"


@dataclass(frozen=True)
class LedgerClocks:
    xrpl_close_time: int
    pftl_height: int
    observed_unix: int

    def __post_init__(self) -> None:
        for name, value in (
            ("xrpl_close_time", self.xrpl_close_time),
            ("pftl_height", self.pftl_height),
            ("observed_unix", self.observed_unix),
        ):
            if type(value) is not int or value < 0 or value > MAX_U64:
                raise TimingGateError(f"{name} must be uint64")


@dataclass(frozen=True)
class TimingPolicy:
    """Margins the coordinator promises to enforce operationally."""

    min_xrpl_submit_margin_seconds: int = 20
    min_pftl_submit_margin_blocks: int = 2
    cross_ledger_claim_margin_seconds: int = 30
    coordinator_seconds_per_pftl_height: int = 5
    max_clock_observation_age_seconds: int = 10

    def __post_init__(self) -> None:
        for name, value in (
            (
                "min_xrpl_submit_margin_seconds",
                self.min_xrpl_submit_margin_seconds,
            ),
            (
                "min_pftl_submit_margin_blocks",
                self.min_pftl_submit_margin_blocks,
            ),
            (
                "cross_ledger_claim_margin_seconds",
                self.cross_ledger_claim_margin_seconds,
            ),
            (
                "coordinator_seconds_per_pftl_height",
                self.coordinator_seconds_per_pftl_height,
            ),
            (
                "max_clock_observation_age_seconds",
                self.max_clock_observation_age_seconds,
            ),
        ):
            if type(value) is not int or value <= 0 or value > MAX_U64:
                raise TimingGateError(f"{name} must be a positive uint64")


@dataclass(frozen=True)
class TimeoutPlan:
    direction: Direction
    xrpl_cancel_after: int
    pftl_cancel_after: int
    first_locker: str = "user"
    second_locker: str = "coordinator"

    def __post_init__(self) -> None:
        if not isinstance(self.direction, Direction):
            raise TimingGateError("direction must be typed")
        for name, value in (
            ("xrpl_cancel_after", self.xrpl_cancel_after),
            ("pftl_cancel_after", self.pftl_cancel_after),
        ):
            if type(value) is not int or value <= 0 or value > MAX_U64:
                raise TimingGateError(f"{name} must be a positive uint64")
        if self.first_locker != "user" or self.second_locker != "coordinator":
            raise TimingGateError("demo requires user-first/coordinator-second ordering")

    @property
    def first_ledger(self) -> str:
        return "xrpl" if self.direction is Direction.XRP_TO_NAV else "pftl"

    @property
    def second_ledger(self) -> str:
        return "pftl" if self.direction is Direction.XRP_TO_NAV else "xrpl"

    def validate_second_lock(
        self,
        clocks: LedgerClocks,
        policy: TimingPolicy,
        *,
        coordinator_observed_unix: int,
    ) -> dict[str, int | str]:
        if (
            type(coordinator_observed_unix) is not int
            or coordinator_observed_unix < clocks.observed_unix
        ):
            raise TimingGateError("coordinator observation time is invalid")
        age = coordinator_observed_unix - clocks.observed_unix
        if age > policy.max_clock_observation_age_seconds:
            raise TimingGateError("cross-ledger clock observation is stale")

        xrpl_remaining = self.xrpl_cancel_after - clocks.xrpl_close_time
        pftl_remaining = self.pftl_cancel_after - clocks.pftl_height
        if xrpl_remaining < policy.min_xrpl_submit_margin_seconds:
            raise TimingGateError("XRPL cancel window is too short")
        if pftl_remaining < policy.min_pftl_submit_margin_blocks:
            raise TimingGateError("PFTL cancel window is too short")

        pftl_window_seconds = (
            pftl_remaining * policy.coordinator_seconds_per_pftl_height
        )
        if self.direction is Direction.XRP_TO_NAV:
            required = pftl_window_seconds + policy.cross_ledger_claim_margin_seconds
            if xrpl_remaining <= required:
                raise TimingGateError(
                    "first XRPL lock does not outlive second PFTL lock plus margin"
                )
        else:
            required = xrpl_remaining + policy.cross_ledger_claim_margin_seconds
            if pftl_window_seconds <= required:
                raise TimingGateError(
                    "first PFTL lock does not outlive second XRPL lock plus margin"
                )

        return {
            "direction": self.direction.value,
            "first_ledger": self.first_ledger,
            "second_ledger": self.second_ledger,
            "xrpl_remaining_seconds": xrpl_remaining,
            "pftl_remaining_blocks": pftl_remaining,
            "pftl_policy_window_seconds": pftl_window_seconds,
            "cross_ledger_claim_margin_seconds": (
                policy.cross_ledger_claim_margin_seconds
            ),
            "clock_trust": "COORDINATOR_TRUSTED",
        }

    def assert_finish_open(self, *, ledger: str, clocks: LedgerClocks) -> None:
        if ledger == "xrpl":
            if clocks.xrpl_close_time >= self.xrpl_cancel_after:
                raise TimingGateError("XRPL finish is at or after cancel_after")
            return
        if ledger == "pftl":
            if clocks.pftl_height >= self.pftl_cancel_after:
                raise TimingGateError("PFTL finish is at or after cancel_after")
            return
        raise TimingGateError("unknown ledger")

    def assert_cancel_open(self, *, ledger: str, clocks: LedgerClocks) -> None:
        if ledger == "xrpl":
            if clocks.xrpl_close_time < self.xrpl_cancel_after:
                raise TimingGateError("XRPL cancel is before cancel_after")
            return
        if ledger == "pftl":
            if clocks.pftl_height < self.pftl_cancel_after:
                raise TimingGateError("PFTL cancel is before cancel_after")
            return
        raise TimingGateError("unknown ledger")
