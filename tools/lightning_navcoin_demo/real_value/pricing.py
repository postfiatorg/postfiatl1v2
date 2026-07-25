"""Integer-only BTC/USD to proven-NAV quote arithmetic."""

from __future__ import annotations

from dataclasses import dataclass
from math import gcd

from .policy import MSAT_PER_BTC, PriceObservation, RealValuePolicyError
from .policy import MAX_ASSET_PRECISION


@dataclass(frozen=True)
class NavQuoteTerms:
    direction: str
    invoice_amount_msat: int
    pftl_amount_atoms: int
    coordinator_fee_atoms: int
    rate_numerator: int
    rate_denominator: int
    btc_usd_e8: int
    gross_value_usd_e8: int
    rounding: str


class FixedNavPricing:
    """Price against one reviewed BTC/USD observation and finalized NAV."""

    def __init__(self, observation: PriceObservation, *, fee_bps: int = 0) -> None:
        if type(fee_bps) is not int or fee_bps < 0 or fee_bps >= 10_000:
            raise ValueError("fee_bps must be within [0, 10000)")
        self.observation = observation
        self.fee_bps = fee_bps

    @staticmethod
    def _ceil_div(numerator: int, denominator: int) -> int:
        return (numerator + denominator - 1) // denominator

    def terms(
        self,
        *,
        direction: str,
        invoice_amount_msat: int,
        nav_per_unit_e8: int,
        asset_precision: int,
    ) -> NavQuoteTerms:
        if direction not in {"lightning_to_pftl", "pftl_to_lightning"}:
            raise RealValuePolicyError("unsupported pricing direction")
        if (
            type(invoice_amount_msat) is not int
            or invoice_amount_msat <= 0
            or invoice_amount_msat > (1 << 63) - 1
        ):
            raise RealValuePolicyError("invoice amount must be positive uint63")
        if (
            type(nav_per_unit_e8) is not int
            or nav_per_unit_e8 <= 0
            or nav_per_unit_e8 > (1 << 63) - 1
        ):
            raise RealValuePolicyError(
                "finalized NAV per whole asset unit must be positive uint63"
            )
        if (
            type(asset_precision) is not int
            or asset_precision < 0
            or asset_precision > MAX_ASSET_PRECISION
        ):
            raise RealValuePolicyError(
                f"asset precision must be within [0, {MAX_ASSET_PRECISION}]"
            )
        atoms_per_unit = 10**asset_precision

        value_numerator = (
            invoice_amount_msat * self.observation.btc_usd_e8
        )
        if direction == "lightning_to_pftl":
            # Output never exceeds the reviewed BTC value.
            gross_usd_e8 = value_numerator // MSAT_PER_BTC
            gross_atoms = (
                gross_usd_e8 * atoms_per_unit
            ) // nav_per_unit_e8
            if gross_atoms <= 0:
                raise RealValuePolicyError("amount rounds to zero NAVcoin atoms")
            fee_atoms = self._ceil_div(gross_atoms * self.fee_bps, 10_000)
            if fee_atoms >= gross_atoms:
                raise RealValuePolicyError("coordinator fee consumes quoted output")
            pftl_atoms = gross_atoms - fee_atoms
            priced_atoms = gross_atoms
            rounding = "btc_value_down_then_navcoin_output_down"
        else:
            # Input rounds up so the coordinator never overpays BTC.
            gross_usd_e8 = self._ceil_div(value_numerator, MSAT_PER_BTC)
            net_atoms = self._ceil_div(
                gross_usd_e8 * atoms_per_unit, nav_per_unit_e8
            )
            if net_atoms <= 0:
                raise RealValuePolicyError("amount rounds to zero NAVcoin atoms")
            pftl_atoms = self._ceil_div(net_atoms * 10_000, 10_000 - self.fee_bps)
            fee_atoms = pftl_atoms - net_atoms
            priced_atoms = net_atoms
            rounding = "btc_value_up_then_navcoin_input_up"
        common = gcd(priced_atoms, invoice_amount_msat)
        numerator = priced_atoms // common
        denominator = invoice_amount_msat // common
        if max(pftl_atoms, fee_atoms, numerator, denominator) > (1 << 63) - 1:
            raise RealValuePolicyError("quote arithmetic exceeds uint63")
        return NavQuoteTerms(
            direction=direction,
            invoice_amount_msat=invoice_amount_msat,
            pftl_amount_atoms=pftl_atoms,
            coordinator_fee_atoms=fee_atoms,
            rate_numerator=numerator,
            rate_denominator=denominator,
            btc_usd_e8=self.observation.btc_usd_e8,
            gross_value_usd_e8=gross_usd_e8,
            rounding=rounding,
        )
