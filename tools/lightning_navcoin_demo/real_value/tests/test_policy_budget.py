from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path

from ...coordinator.signing import sign_quote
from ..authorization import ValueAuthorizationError, verify_value_authorization
from ..budget import BudgetError, RealValueBudget
from ..policy import RealValuePolicy, RealValuePolicyError, validate_mainnet_quote
from ..pricing import FixedNavPricing
from .common import (
    QUOTE_SIGNER,
    authorization_for,
    policy,
    policy_mapping,
    price,
    quote_mapping,
    signed_quote,
)


NOW = 1_800_000_000


class RealValuePolicyTests(unittest.TestCase):
    def test_policy_hard_caps_and_nonfreezable_gate(self) -> None:
        value = policy_mapping()
        value["max_per_run_usd_e8"] = 500_000_001
        with self.assertRaisesRegex(RealValuePolicyError, "per-run cap"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["require_non_freezable"] = False
        with self.assertRaisesRegex(RealValuePolicyError, "finish-freeze"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["minimum_pftl_validators"] = 5
        with self.assertRaisesRegex(RealValuePolicyError, "six-of-six"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_rpc_endpoints"][0] = "tcp://127.0.0.1:not-a-port"
        with self.assertRaisesRegex(RealValuePolicyError, "valid endpoint"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_asset_precision"] = 19
        with self.assertRaisesRegex(RealValuePolicyError, "precision exceeds"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_build_git_revision"] = "not-a-revision"
        with self.assertRaisesRegex(RealValuePolicyError, "canonical lowercase hex"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_nav_valuation_unit"] = "USDC_E6_PER_WHOLE_ASSET_UNIT"
        with self.assertRaisesRegex(RealValuePolicyError, "valuation unit"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_nav_valuation_scale"] = 1_000_000
        with self.assertRaisesRegex(RealValuePolicyError, "valuation scale"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["pftl_nav_per_unit_usd_e8"] = 0
        with self.assertRaisesRegex(RealValuePolicyError, "pftl_nav_per_unit_usd_e8"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["max_price_age_seconds"] = 301
        with self.assertRaisesRegex(RealValuePolicyError, "price_age.*hard"):
            RealValuePolicy.from_mapping(value)
        value = policy_mapping()
        value["max_quote_lifetime_seconds"] = 301
        with self.assertRaisesRegex(RealValuePolicyError, "quote_lifetime.*hard"):
            RealValuePolicy.from_mapping(value)

    def test_proven_nav_sha384_quote_and_exact_rate_are_required(self) -> None:
        route = policy()
        view = validate_mainnet_quote(
            signed_quote(), route, price(), now_unix=NOW
        )
        self.assertEqual(view.nav_reserve_packet_hash, "44" * 48)
        malformed = quote_mapping()
        malformed["nav_reserve_packet_hash"] = "44" * 32
        with self.assertRaisesRegex(Exception, "48 bytes"):
            sign_quote(malformed, QUOTE_SIGNER)
        mismatched = quote_mapping()
        mismatched["rate_numerator"] = 2
        with self.assertRaisesRegex(RealValuePolicyError, "exactly price"):
            validate_mainnet_quote(
                sign_quote(mismatched, QUOTE_SIGNER),
                route,
                price(),
                now_unix=NOW,
            )

    def test_mainnet_identity_nav_freshness_and_dust_cap_fail_closed(self) -> None:
        route = policy()
        with self.assertRaisesRegex(RealValuePolicyError, "stale"):
            validate_mainnet_quote(
                signed_quote(),
                route,
                price(observed_at_unix=NOW - 61),
                now_unix=NOW,
            )
        wrong = quote_mapping()
        wrong["pftl_asset_id"] = "99" * 48
        with self.assertRaisesRegex(RealValuePolicyError, "pftl_asset_id"):
            validate_mainnet_quote(
                sign_quote(wrong, QUOTE_SIGNER), route, price(), now_unix=NOW
            )
        too_large = quote_mapping()
        too_large["invoice_amount_msat"] = 6_000_000
        too_large["rate_denominator"] = 6_000
        with self.assertRaisesRegex(RealValuePolicyError, "per-run"):
            validate_mainnet_quote(
                sign_quote(too_large, QUOTE_SIGNER),
                route,
                price(),
                now_unix=NOW,
            )

    def test_offramp_receiver_payee_is_not_coordinator_identity(self) -> None:
        route = policy()
        view = validate_mainnet_quote(
            signed_quote(
                direction="pftl_to_lightning",
                invoice_payee="03" + "99" * 32,
            ),
            route,
            price(),
            now_unix=NOW,
        )
        self.assertEqual(view.direction, "pftl_to_lightning")

    def test_integer_nav_pricing_rounds_against_the_coordinator(self) -> None:
        pricing = FixedNavPricing(price(), fee_bps=25)
        onramp = pricing.terms(
            direction="lightning_to_pftl",
            invoice_amount_msat=100_001,
            nav_per_unit_e8=100_000_000,
            asset_precision=6,
        )
        self.assertEqual(
            onramp.pftl_amount_atoms + onramp.coordinator_fee_atoms,
            (
                (100_001 * price().btc_usd_e8) // 100_000_000_000
            )
            * 1_000_000
            // 100_000_000,
        )
        offramp = pricing.terms(
            direction="pftl_to_lightning",
            invoice_amount_msat=100_001,
            nav_per_unit_e8=100_000_000,
            asset_precision=6,
        )
        self.assertGreaterEqual(
            offramp.pftl_amount_atoms - offramp.coordinator_fee_atoms,
            onramp.pftl_amount_atoms + onramp.coordinator_fee_atoms,
        )

    def test_proven_nav_is_per_whole_precision_scaled_unit(self) -> None:
        pricing = FixedNavPricing(price())
        onramp = pricing.terms(
            direction="lightning_to_pftl",
            invoice_amount_msat=100_000,
            nav_per_unit_e8=1_035_074_022,
            asset_precision=6,
        )
        offramp = pricing.terms(
            direction="pftl_to_lightning",
            invoice_amount_msat=100_000,
            nav_per_unit_e8=1_035_074_022,
            asset_precision=6,
        )
        # 100 sat at the pinned $100k/BTC input is $0.10. At
        # $10.35074022 per 1e6-atom coin, output rounds down and required
        # input rounds up against the coordinator.
        self.assertEqual(onramp.pftl_amount_atoms, 9_661)
        self.assertEqual(offramp.pftl_amount_atoms, 9_662)
        with self.assertRaisesRegex(RealValuePolicyError, "precision"):
            pricing.terms(
                direction="lightning_to_pftl",
                invoice_amount_msat=100_000,
                nav_per_unit_e8=1_035_074_022,
                asset_precision=1_000_000,
            )


class RealValueBudgetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def _path(self, suffix: str) -> Path:
        return Path(self.temp.name) / suffix

    def test_dry_run_cannot_reserve(self) -> None:
        route = policy()
        view = validate_mainnet_quote(
            signed_quote(), route, price(), now_unix=NOW
        )
        permit = authorization_for(view, route)
        with RealValueBudget(self._path("dry.sqlite3"), route) as budget:
            verified = verify_value_authorization(
                permit, route, quote=view, now_unix=NOW
            )
            self.assertTrue(budget.preview(verified)["would_fit"])
            with self.assertRaisesRegex(BudgetError, "DRY_RUN"):
                budget.reserve(permit, quote=view, now_unix=NOW)
            self.assertEqual(budget.summary()["reserved_count"], 0)

    def test_single_use_reserve_spend_and_irreversible_ceiling(self) -> None:
        route = policy(mode="ARMED")
        view = validate_mainnet_quote(
            signed_quote(), route, price(), now_unix=NOW
        )
        permit = authorization_for(view, route)
        with RealValueBudget(self._path("armed.sqlite3"), route) as budget:
            first = budget.reserve(permit, quote=view, now_unix=NOW)
            second = budget.reserve(permit, quote=view, now_unix=NOW)
            self.assertEqual(first, second)
            self.assertEqual(budget.summary()["reserved_count"], 1)
            spent = budget.mark_spent(
                first["authorization_id"],
                terminal_evidence={
                    "value_moved": True,
                    "payment_hash": "ab" * 32,
                    "status": "SUCCEEDED",
                },
                now_unix=NOW + 1,
            )
            self.assertEqual(spent["state"], "SPENT")
            summary = budget.summary()
            self.assertEqual(summary["reserved_count"], 0)
            self.assertEqual(summary["spent_count"], 1)
            self.assertEqual(summary["spent_usd_e8"], view.maximum_all_in_usd_e8)
            with self.assertRaisesRegex(BudgetError, "cannot be released"):
                budget.release_unspent(
                    first["authorization_id"],
                    no_value_evidence={"value_moved": False},
                    now_unix=NOW + 2,
                )

    def test_release_requires_literal_no_value_and_secret_free_spend_evidence(self) -> None:
        route = policy(mode="ARMED")
        view = validate_mainnet_quote(
            signed_quote(), route, price(), now_unix=NOW
        )
        permit = authorization_for(view, route)
        with RealValueBudget(self._path("release.sqlite3"), route) as budget:
            reservation = budget.reserve(permit, quote=view, now_unix=NOW)
            with self.assertRaisesRegex(BudgetError, "value_moved=false"):
                budget.release_unspent(
                    reservation["authorization_id"],
                    no_value_evidence={"value_moved": True},
                )
            with self.assertRaisesRegex(BudgetError, "secret-bearing"):
                budget.mark_spent(
                    reservation["authorization_id"],
                    terminal_evidence={
                        "value_moved": True,
                        "payment_preimage": "00" * 32,
                    },
                )
            with self.assertRaisesRegex(BudgetError, "secret-bearing"):
                budget.release_unspent(
                    reservation["authorization_id"],
                    no_value_evidence={
                        "value_moved": False,
                        "coordinator_secret": "00" * 32,
                    },
                )
            released = budget.release_unspent(
                reservation["authorization_id"],
                no_value_evidence={"value_moved": False, "reason": "preflight_failed"},
            )
            self.assertEqual(released["state"], "RELEASED")
            self.assertEqual(budget.summary()["remaining_usd_e8"], 2_000_000_000)

    def test_modified_authorization_is_rejected(self) -> None:
        route = policy(mode="ARMED")
        view = validate_mainnet_quote(
            signed_quote(), route, price(), now_unix=NOW
        )
        permit = authorization_for(view, route)
        modified = copy.deepcopy(permit)
        modified["authorization"]["principal_msat"] += 1
        with self.assertRaisesRegex(ValueAuthorizationError, "signature"):
            verify_value_authorization(
                modified, route, quote=view, now_unix=NOW
            )


if __name__ == "__main__":
    unittest.main()
