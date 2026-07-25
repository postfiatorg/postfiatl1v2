"""Coordinator timing policy for unrelated Bitcoin and PFTL block clocks."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class TimingError(ValueError):
    """A two-ledger timeout plan violates the configured safety margin."""


class Direction(str, Enum):
    BTC_TO_NAV = "btc_to_nav"
    NAV_TO_BTC = "nav_to_btc"


@dataclass(frozen=True)
class TimingPolicy:
    bitcoin_seconds_per_block: int = 600
    pftl_seconds_per_block: int = 5
    cross_ledger_margin_seconds: int = 600
    minimum_remaining_blocks: int = 1

    def __post_init__(self) -> None:
        for name, value in self.__dict__.items():
            if type(value) is not int or value <= 0:
                raise TimingError(f"{name} must be a positive integer")


def validate_second_lock(
    *,
    direction: Direction,
    bitcoin_height: int,
    bitcoin_cancel_height: int,
    pftl_height: int,
    pftl_cancel_height: int,
    policy: TimingPolicy = TimingPolicy(),
) -> dict[str, int | str]:
    if not isinstance(direction, Direction):
        raise TimingError("direction must be typed")
    for name, value in (
        ("bitcoin_height", bitcoin_height),
        ("bitcoin_cancel_height", bitcoin_cancel_height),
        ("pftl_height", pftl_height),
        ("pftl_cancel_height", pftl_cancel_height),
    ):
        if type(value) is not int or value < 0:
            raise TimingError(f"{name} must be a non-negative integer")
    bitcoin_blocks = bitcoin_cancel_height - bitcoin_height
    pftl_blocks = pftl_cancel_height - pftl_height
    if (
        bitcoin_blocks < policy.minimum_remaining_blocks
        or pftl_blocks < policy.minimum_remaining_blocks
    ):
        raise TimingError("a timeout is already mature or lacks submit margin")
    bitcoin_window = bitcoin_blocks * policy.bitcoin_seconds_per_block
    pftl_window = pftl_blocks * policy.pftl_seconds_per_block
    first_window = (
        bitcoin_window if direction is Direction.BTC_TO_NAV else pftl_window
    )
    second_window = (
        pftl_window if direction is Direction.BTC_TO_NAV else bitcoin_window
    )
    if first_window <= second_window + policy.cross_ledger_margin_seconds:
        raise TimingError("first lock does not outlive second lock plus margin")
    return {
        "direction": direction.value,
        "first_ledger": (
            "bitcoin" if direction is Direction.BTC_TO_NAV else "pftl"
        ),
        "second_ledger": (
            "pftl" if direction is Direction.BTC_TO_NAV else "bitcoin"
        ),
        "bitcoin_remaining_blocks": bitcoin_blocks,
        "pftl_remaining_blocks": pftl_blocks,
        "bitcoin_policy_window_seconds": bitcoin_window,
        "pftl_policy_window_seconds": pftl_window,
        "cross_ledger_margin_seconds": policy.cross_ledger_margin_seconds,
        "clock_relationship": "COORDINATOR_POLICY_ESTIMATE",
    }
