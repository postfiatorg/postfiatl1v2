from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from ..authorization import (
    ValueAuthorizationError,
    sign_value_authorization,
    verify_value_authorization,
)
from ..budget import RealValueBudget
from ..cli import build_parser
from ..composition import CompositionError, SecureStatePaths
from ..liquidity_budget import (
    LIQUIDITY_EVIDENCE_SCHEMA,
    LIQUIDITY_MIN_REMAINING_INITIATION_SECONDS,
    LIQUIDITY_POST_RESERVATION_INITIATION_SECONDS,
    MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS,
    MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS,
    LiquidityBudgetError,
    mark_liquidity_setup_spent,
    reserve_liquidity_setup,
)
from .common import AUTH_SIGNER, policy, policy_mapping, price


NOW = 1_800_000_000


def liquidity_authorization(
    *,
    route: object,
    suffix: str = "one",
    category: str = "LIQUIDITY_SETUP",
    direction: str = "not_applicable",
    principal_msat: int = 100_000,
    max_all_in_usd_e8: int = 20_000_000,
    expires_unix: int = NOW + 30 * 60,
) -> dict[str, object]:
    setup_id = hashlib.sha256(f"liquidity-setup:{suffix}".encode()).hexdigest()
    return sign_value_authorization(
        {
            "schema": "postfiat.lightning_value_authorization.v1",
            "authorization_id": hashlib.sha256(
                f"liquidity-auth:{suffix}".encode()
            ).hexdigest(),
            "policy_id": route.policy_id,
            "category": category,
            "quote_sha256": hashlib.sha256(
                f"reviewed-liquidity-intent:{suffix}".encode()
            ).hexdigest(),
            "swap_id": setup_id,
            "direction": direction,
            "principal_msat": principal_msat,
            "max_fee_msat": 10_000,
            "max_all_in_usd_e8": max_all_in_usd_e8,
            "expires_unix": expires_unix,
            "authorized_by": "nazgul",
        },
        AUTH_SIGNER,
    )


def terminal_evidence(
    reservation: dict[str, object],
    *,
    actual_cost_msat: int = 105_000,
) -> dict[str, object]:
    return {
        "schema": LIQUIDITY_EVIDENCE_SCHEMA,
        "authorization_id": reservation["authorization_id"],
        "policy_id": reservation["policy_id"],
        "setup_id": reservation["setup_id"],
        "category": "LIQUIDITY_SETUP",
        "direction": "not_applicable",
        "provider": "operator-reviewed-test-lsp",
        "outcome": "EXTERNAL_PAYMENT_CONFIRMED_AND_CHANNEL_ACTIVE",
        "value_moved": True,
        "payment_status": "SUCCEEDED",
        "payment_hash": "ab" * 32,
        "actual_cost_msat": actual_cost_msat,
        "payment_initiated_at_unix": NOW + 10,
        "payment_settled_at_unix": NOW + 20,
        "channel_active": True,
        "channel_point": f"{'cd' * 32}:1",
        "remote_pubkey": "02" + "ef" * 32,
        "capacity_sat": 1_000_000,
        "inbound_msat": 900_000_000,
        "funding_confirmations": 3,
        "observed_at_unix": NOW + 30,
    }


class LiquidityBudgetCliTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve() / "state"
        self.paths = SecureStatePaths.under(self.root)
        self.paths.config_dir.mkdir(parents=True, mode=0o700)
        self.route = policy(mode="ARMED")
        self.policy_path = self.paths.config_dir / "policy.json"
        self.price_path = self.paths.config_dir / "btc-price.json"
        self._write(self.policy_path, policy_mapping(mode="ARMED"))
        self._write(self.price_path, price().to_dict())
        self.release_patch = patch(
            "tools.lightning_navcoin_demo.real_value.liquidity_budget."
            "validate_armed_source_release",
            return_value={"clean": True},
        )
        self.release_patch.start()
        self.addCleanup(self.release_patch.stop)

    @staticmethod
    def _write(path: Path, value: object, *, mode: int = 0o600) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="ascii",
        )
        path.chmod(mode)

    def _reserve(
        self,
        *,
        suffix: str = "one",
        authorization: dict[str, object] | None = None,
    ) -> dict[str, object]:
        permit_path = self.paths.config_dir / f"permit-{suffix}.json"
        self._write(
            permit_path,
            authorization
            if authorization is not None
            else liquidity_authorization(route=self.route, suffix=suffix),
        )
        return reserve_liquidity_setup(
            state_dir=self.root,
            policy_path=self.policy_path,
            price_path=self.price_path,
            authorization_path=permit_path,
            now_unix=NOW,
        )

    def test_reserve_then_charge_full_ceiling_without_external_clients(self) -> None:
        reservation = self._reserve()
        self.assertEqual(reservation["status"], "RESERVED")
        self.assertEqual(reservation["category"], "LIQUIDITY_SETUP")
        self.assertEqual(reservation["direction"], "not_applicable")
        self.assertFalse(reservation["order_created_by_command"])
        self.assertFalse(reservation["payment_initiated_by_command"])
        self.assertFalse(reservation["pftl_signer_loaded"])
        self.assertEqual(
            reservation["ambiguity_policy"], "KEEP_RESERVED_AND_HOLD"
        )

        evidence_path = self.paths.config_dir / "terminal.json"
        evidence = terminal_evidence(reservation)
        self._write(evidence_path, evidence)
        spent = mark_liquidity_setup_spent(
            state_dir=self.root,
            policy_path=self.policy_path,
            evidence_path=evidence_path,
            now_unix=NOW + 30,
        )
        self.assertEqual(spent["status"], "SPENT")
        self.assertEqual(
            spent["charged_ceiling_usd_e8"],
            reservation["ceiling_usd_e8"],
        )
        self.assertFalse(spent["ambiguity_release_available"])
        self.assertFalse(spent["external_api_called_by_command"])

        # Identical terminal evidence is idempotent; changed evidence is not.
        repeated = mark_liquidity_setup_spent(
            state_dir=self.root,
            policy_path=self.policy_path,
            evidence_path=evidence_path,
            now_unix=NOW + 31,
        )
        self.assertEqual(repeated["status"], "SPENT")
        evidence["funding_confirmations"] = 4
        self._write(evidence_path, evidence)
        with self.assertRaisesRegex(Exception, "evidence changed"):
            mark_liquidity_setup_spent(
                state_dir=self.root,
                policy_path=self.policy_path,
                evidence_path=evidence_path,
                now_unix=NOW + 32,
            )

    def test_reserved_liquidity_setup_is_not_swap_recovery_work(self) -> None:
        self._reserve()
        with RealValueBudget(self.paths.budget, self.route) as budget:
            self.assertEqual(budget.reserved_swap_ids(), ())

    def test_requires_armed_policy_clean_release_and_owner_only_inputs(self) -> None:
        dry_path = self.paths.config_dir / "dry-policy.json"
        self._write(dry_path, policy_mapping(mode="DRY_RUN"))
        permit_path = self.paths.config_dir / "permit.json"
        self._write(
            permit_path,
            liquidity_authorization(route=self.route),
        )
        with self.assertRaisesRegex(LiquidityBudgetError, "ARMED"):
            reserve_liquidity_setup(
                state_dir=self.root,
                policy_path=dry_path,
                price_path=self.price_path,
                authorization_path=permit_path,
                now_unix=NOW,
            )
        self.assertFalse(self.paths.budget.exists())

        permit_path.chmod(0o644)
        with self.assertRaisesRegex(LiquidityBudgetError, "mode-0600"):
            reserve_liquidity_setup(
                state_dir=self.root,
                policy_path=self.policy_path,
                price_path=self.price_path,
                authorization_path=permit_path,
                now_unix=NOW,
            )
        self.assertFalse(self.paths.budget.exists())

        permit_path.chmod(0o600)
        with patch(
            "tools.lightning_navcoin_demo.real_value.liquidity_budget."
            "validate_armed_source_release",
            side_effect=CompositionError("release dirty"),
        ):
            with self.assertRaisesRegex(CompositionError, "release dirty"):
                reserve_liquidity_setup(
                    state_dir=self.root,
                    policy_path=self.policy_path,
                    price_path=self.price_path,
                    authorization_path=permit_path,
                    now_unix=NOW,
                )
        self.assertFalse(self.paths.budget.exists())

    def test_exact_category_direction_fresh_price_and_usd_binding(self) -> None:
        swap = liquidity_authorization(
            route=self.route,
            category="SWAP",
            direction="lightning_to_pftl",
        )
        with self.assertRaisesRegex(LiquidityBudgetError, "LIQUIDITY_SETUP"):
            self._reserve(authorization=swap)
        stale = price(observed_at_unix=NOW - 61).to_dict()
        self._write(self.price_path, stale)
        with self.assertRaisesRegex(LiquidityBudgetError, "stale"):
            self._reserve()
        self._write(self.price_path, price().to_dict())
        underpriced = liquidity_authorization(
            route=self.route,
            max_all_in_usd_e8=10_000_000,
        )
        with self.assertRaisesRegex(LiquidityBudgetError, "USD ceiling"):
            self._reserve(authorization=underpriced)

    def test_liquidity_has_a_short_start_and_bounded_settlement_grace(
        self,
    ) -> None:
        self.assertEqual(MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS, 4 * 60 * 60)
        self.assertEqual(
            LIQUIDITY_MIN_REMAINING_INITIATION_SECONDS,
            30 * 60,
        )
        self.assertEqual(
            LIQUIDITY_POST_RESERVATION_INITIATION_SECONDS,
            60 * 60,
        )
        self.assertEqual(MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS, 6 * 60 * 60)
        # The swap quote limit remains independent and shorter.
        self.assertEqual(self.route.max_quote_lifetime_seconds, 120)

        start_at_liquidity_boundary = liquidity_authorization(
            route=self.route,
            expires_unix=NOW + MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS,
        )
        reserved = self._reserve(
            suffix="start-boundary",
            authorization=start_at_liquidity_boundary,
        )
        self.assertEqual(
            reserved["authorization_expires_unix"],
            NOW + MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS,
        )
        self.assertEqual(
            reserved["payment_initiation_deadline_unix"],
                NOW + LIQUIDITY_POST_RESERVATION_INITIATION_SECONDS,
        )

        late_start_authority = liquidity_authorization(
            route=self.route,
            suffix="late-start-authority",
            expires_unix=(
                NOW + MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS + 1
            ),
        )
        with self.assertRaisesRegex(
            ValueAuthorizationError, "hard initiation horizon"
        ):
            verify_value_authorization(
                late_start_authority,
                self.route,
                quote=None,
                now_unix=NOW,
            )
        with self.assertRaisesRegex(
            ValueAuthorizationError, "hard initiation horizon"
        ):
            self._reserve(
                suffix="late-start-authority",
                authorization=late_start_authority,
            )
        insufficient_start_authority = liquidity_authorization(
            route=self.route,
            suffix="insufficient-start-authority",
            expires_unix=(
                NOW + LIQUIDITY_MIN_REMAINING_INITIATION_SECONDS - 1
            ),
        )
        with self.assertRaisesRegex(
            LiquidityBudgetError, "insufficient remaining initiation time"
        ):
            self._reserve(
                suffix="insufficient-start-authority",
                authorization=insufficient_start_authority,
            )
        with RealValueBudget(self.paths.budget, self.route) as budget:
            self.assertEqual(budget.summary()["reserved_count"], 1)

    def test_confirmed_hodl_payment_may_settle_inside_grace(self) -> None:
        reserved = self._reserve()
        terminal = terminal_evidence(reserved)
        terminal["payment_initiated_at_unix"] = NOW + 10
        terminal["payment_settled_at_unix"] = (
            terminal["payment_initiated_at_unix"]
            + MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS
        )
        terminal["observed_at_unix"] = terminal["payment_settled_at_unix"]
        evidence_path = self.paths.config_dir / "delayed-terminal.json"
        self._write(evidence_path, terminal)
        spent = mark_liquidity_setup_spent(
            state_dir=self.root,
            policy_path=self.policy_path,
            evidence_path=evidence_path,
            now_unix=terminal["observed_at_unix"],
        )
        self.assertEqual(spent["status"], "SPENT")
        self.assertEqual(
            spent["charged_ceiling_usd_e8"],
            reserved["ceiling_usd_e8"],
        )

    def test_late_initiation_or_settlement_remains_reserved(self) -> None:
        initiated_before_reservation = self._reserve(
            suffix="initiated-before-reservation"
        )
        premature_initiation = terminal_evidence(initiated_before_reservation)
        premature_initiation["payment_initiated_at_unix"] = NOW - 1
        premature_initiation["payment_settled_at_unix"] = NOW + 1
        premature_initiation["observed_at_unix"] = NOW + 2
        premature_path = self.paths.config_dir / "premature-initiation.json"
        self._write(premature_path, premature_initiation)
        with self.assertRaisesRegex(
            LiquidityBudgetError, "before durable reservation"
        ):
            mark_liquidity_setup_spent(
                state_dir=self.root,
                policy_path=self.policy_path,
                evidence_path=premature_path,
                now_unix=premature_initiation["observed_at_unix"],
            )

        initiated_late = self._reserve(suffix="initiated-late")
        late_initiation = terminal_evidence(initiated_late)
        late_initiation["payment_initiated_at_unix"] = (
            initiated_late["authorization_expires_unix"] + 1
        )
        late_initiation["payment_settled_at_unix"] = late_initiation[
            "payment_initiated_at_unix"
        ]
        late_initiation["observed_at_unix"] = late_initiation[
            "payment_settled_at_unix"
        ]
        initiation_path = self.paths.config_dir / "late-initiation.json"
        self._write(initiation_path, late_initiation)
        with self.assertRaisesRegex(
            LiquidityBudgetError, "initiated after authorization expiry"
        ):
            mark_liquidity_setup_spent(
                state_dir=self.root,
                policy_path=self.policy_path,
                evidence_path=initiation_path,
                now_unix=late_initiation["observed_at_unix"],
            )

        long_authority = liquidity_authorization(
            route=self.route,
            suffix="post-reservation-late",
            expires_unix=NOW + MAX_LIQUIDITY_INITIATION_HORIZON_SECONDS,
        )
        post_reservation_late = self._reserve(
            suffix="post-reservation-late",
            authorization=long_authority,
        )
        late_after_reservation = terminal_evidence(post_reservation_late)
        late_after_reservation["payment_initiated_at_unix"] = (
            NOW + LIQUIDITY_POST_RESERVATION_INITIATION_SECONDS + 1
        )
        late_after_reservation["payment_settled_at_unix"] = (
            late_after_reservation["payment_initiated_at_unix"]
        )
        late_after_reservation["observed_at_unix"] = (
            late_after_reservation["payment_settled_at_unix"]
        )
        late_after_reservation_path = (
            self.paths.config_dir / "post-reservation-late.json"
        )
        self._write(late_after_reservation_path, late_after_reservation)
        with self.assertRaisesRegex(
            LiquidityBudgetError, "post-reservation initiation window"
        ):
            mark_liquidity_setup_spent(
                state_dir=self.root,
                policy_path=self.policy_path,
                evidence_path=late_after_reservation_path,
                now_unix=late_after_reservation["observed_at_unix"],
            )

        settled_late = self._reserve(suffix="settled-late")
        late_settlement = terminal_evidence(settled_late)
        late_settlement["payment_settled_at_unix"] = (
            late_settlement["payment_initiated_at_unix"]
            + MAX_LIQUIDITY_SETTLEMENT_GRACE_SECONDS
            + 1
        )
        late_settlement["observed_at_unix"] = late_settlement[
            "payment_settled_at_unix"
        ]
        settlement_path = self.paths.config_dir / "late-settlement.json"
        self._write(settlement_path, late_settlement)
        with self.assertRaisesRegex(
            LiquidityBudgetError, "bounded settlement grace"
        ):
            mark_liquidity_setup_spent(
                state_dir=self.root,
                policy_path=self.policy_path,
                evidence_path=settlement_path,
                now_unix=late_settlement["observed_at_unix"],
            )
        with RealValueBudget(self.paths.budget, self.route) as budget:
            self.assertEqual(budget.summary()["reserved_count"], 4)
            self.assertEqual(budget.summary()["spent_count"], 0)

    def test_lifetime_cap_and_terminal_evidence_are_fail_closed(self) -> None:
        # Four $5 reservations exhaust this fixture's configured lifetime.
        for index in range(4):
            permit = liquidity_authorization(
                route=self.route,
                suffix=str(index),
                principal_msat=4_900_000,
                max_all_in_usd_e8=500_000_000,
            )
            self._reserve(suffix=str(index), authorization=permit)
        with self.assertRaisesRegex(Exception, "remaining lifetime"):
            self._reserve(suffix="overflow")

        # Exercise terminal binding in an independent budget.
        second = Path(self.root.parent) / "state-two"
        second_paths = SecureStatePaths.under(second)
        second_paths.config_dir.mkdir(parents=True, mode=0o700)
        second_policy = second_paths.config_dir / "policy.json"
        second_price = second_paths.config_dir / "price.json"
        self._write(second_policy, policy_mapping(mode="ARMED"))
        self._write(second_price, price().to_dict())
        permit = second_paths.config_dir / "permit.json"
        self._write(permit, liquidity_authorization(route=self.route))
        reserved = reserve_liquidity_setup(
            state_dir=second,
            policy_path=second_policy,
            price_path=second_price,
            authorization_path=permit,
            now_unix=NOW,
        )
        terminal = terminal_evidence(reserved, actual_cost_msat=110_001)
        evidence_path = second_paths.config_dir / "evidence.json"
        self._write(evidence_path, terminal)
        with self.assertRaisesRegex(LiquidityBudgetError, "msat ceiling"):
            mark_liquidity_setup_spent(
                state_dir=second,
                policy_path=second_policy,
                evidence_path=evidence_path,
                now_unix=NOW + 30,
            )
        terminal["actual_cost_msat"] = 105_000
        terminal["channel_active"] = False
        self._write(evidence_path, terminal)
        with self.assertRaisesRegex(LiquidityBudgetError, "channel_active"):
            mark_liquidity_setup_spent(
                state_dir=second,
                policy_path=second_policy,
                evidence_path=evidence_path,
                now_unix=NOW + 30,
            )
        with RealValueBudget(second_paths.budget, self.route) as budget:
            self.assertEqual(budget.summary()["reserved_count"], 1)
            self.assertEqual(budget.summary()["spent_count"], 0)

    def test_cli_exposes_no_liquidity_release_command(self) -> None:
        parser = build_parser()
        reserve = parser.parse_args(
            [
                "liquidity-reserve",
                "--authorization",
                "/tmp/public-permit.json",
            ]
        )
        self.assertEqual(reserve.command, "liquidity-reserve")
        spent = parser.parse_args(
            [
                "liquidity-mark-spent",
                "--evidence",
                "/tmp/public-terminal.json",
            ]
        )
        self.assertEqual(spent.command, "liquidity-mark-spent")
        self.assertNotIn(
            "liquidity-release",
            parser._subparsers._group_actions[0].choices,
        )


if __name__ == "__main__":
    unittest.main()
