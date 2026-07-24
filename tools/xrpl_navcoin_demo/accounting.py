"""Exact principal conservation checks (network fees are separate)."""

from __future__ import annotations

from dataclasses import dataclass


class ConservationError(AssertionError):
    pass


@dataclass(frozen=True)
class PrincipalState:
    user: int
    coordinator: int
    locked: int

    @property
    def total(self) -> int:
        return self.user + self.coordinator + self.locked


def assert_principal_conserved(before: PrincipalState, after: PrincipalState) -> None:
    if before.total != after.total:
        raise ConservationError(
            f"principal changed: before={before.total}, after={after.total}"
        )


def assert_xrp_conserved_with_fees(
    before: PrincipalState, after: PrincipalState, fees_drops: int
) -> None:
    if before.total - after.total != fees_drops:
        raise ConservationError(
            "XRP account-plus-escrow principal delta does not equal validated fees"
        )

