"""Cross-language exact-atom vector and Open NAVCoin SDK tests (Pillar 7).

Compatible with both ``pytest`` and ``python3 -m unittest``.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import unittest

from postfiat_rpc.navcoin_sdk import (
    NAV_FEE_MAX_ANNUAL_RATE_BPS,
    NAV_FEE_MAX_FIX_FEE_CAP_BPS,
    NAV_FEE_MAX_HWM_RATE_BPS,
    NAV_FEE_MAX_SPREAD_BPS,
    NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
    PROTOCOL_BLOCKS_PER_YEAR,
    NavFeeError,
    NavFeeScheduleInputV1,
    attach_fee_schedule,
    build_nav_fee_claim_operation,
    build_nav_fee_history_rpc_request,
    build_nav_fee_schedule_cancel_operation,
    build_nav_fee_schedule_rpc_request,
    build_nav_fee_schedule_set_operation,
    cancel_pending_fee_schedule,
    canonical_json_bytes,
    compute_nav_per_unit,
    create_navcoin,
    derived_asset_id,
    evaluate_vector,
    inspect_pending_hike,
    nav_reserve_packet_id,
    parse_nav_fee_history_response,
    parse_nav_fee_schedule_response,
    propose_fee_schedule_update,
    recompute_fees,
    verify_nav_per_unit,
    verify_nav_receipt,
)

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
VECTOR_FILE = REPO_ROOT / "sdk" / "navcoin" / "vectors" / "nav_fee_vectors_v1.json"
CLI_FILE = REPO_ROOT / "sdk" / "navcoin" / "bin" / "navcoin-cli.mjs"

ISSUER = "pf1111111111111111111111111111111111111111"
RECIPIENT = "pf3333333333333333333333333333333333333333"
ASSET_ID = "99bb" * 24


def _load_vector_suite() -> dict:
    return json.loads(VECTOR_FILE.read_text(encoding="utf-8"))


class NavcoinFeeVectorSuiteTests(unittest.TestCase):
    def test_vector_suite_metadata_and_constants(self) -> None:
        suite = _load_vector_suite()
        self.assertEqual(suite["schema"], "postfiat-nav-fee-vectors-v1")
        self.assertGreaterEqual(len(suite["vectors"]), 100)
        self.assertEqual(suite["vector_count"], len(suite["vectors"]))
        expected_categories = {
            "universal_nav",
            "schedule_validation",
            "single_epoch_accrual",
            "pending_notice_and_lifecycle",
            "multi_epoch_compounding",
        }
        self.assertEqual(set(suite["category_counts"].keys()), expected_categories)
        self.assertEqual(PROTOCOL_BLOCKS_PER_YEAR, 31_536_000)
        self.assertEqual(NAV_FEE_MAX_ANNUAL_RATE_BPS, 500)
        self.assertEqual(NAV_FEE_MAX_HWM_RATE_BPS, 5_000)
        self.assertEqual(NAV_FEE_MAX_SPREAD_BPS, 500)
        self.assertEqual(NAV_FEE_MAX_FIX_FEE_CAP_BPS, 100)
        self.assertEqual(NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS, 604_800)

    def test_all_100_plus_exact_atom_vectors_in_python(self) -> None:
        suite = _load_vector_suite()
        seen_ids: set[str] = set()
        for vector in suite["vectors"]:
            vid = vector["id"]
            self.assertNotIn(vid, seen_ids, f"duplicate vector id: {vid}")
            seen_ids.add(vid)
            with self.subTest(vector_id=vid, kind=vector["kind"]):
                actual = evaluate_vector(vector)
                self.assertEqual(actual, vector["expected"])

    def test_cross_language_byte_identical_python_and_node_launch_plan(self) -> None:
        py_zero = create_navcoin(
            chain_id="postfiat-wan-devnet-2",
            issuer=ISSUER,
            code="OPENNAV",
            display_name="Open Reserve Unit",
            precision=6,
        )
        node_zero_raw = subprocess.check_output(
            [
                "node",
                str(CLI_FILE),
                "create",
                "--chain-id",
                "postfiat-wan-devnet-2",
                "--issuer",
                ISSUER,
                "--code",
                "OPENNAV",
                "--name",
                "Open Reserve Unit",
                "--precision",
                "6",
                "--canonical",
            ],
            text=True,
        ).strip()
        self.assertEqual(canonical_json_bytes(py_zero).decode("utf-8"), node_zero_raw)

        py_fee = create_navcoin(
            chain_id="postfiat-wan-devnet-2",
            issuer=ISSUER,
            code="FEENAV",
            display_name="Scheduled Reserve Unit",
            precision=8,
            fee_schedule=NavFeeScheduleInputV1(
                annual_rate_bps=100,
                hwm_rate_bps=1000,
                issue_spread_bps=50,
                redeem_spread_bps=50,
                fix_fee_cap_bps=25,
                recipient=RECIPIENT,
                increase_notice_blocks=604_800,
                accrual_cap_blocks=86_400,
            ),
        )
        node_fee_raw = subprocess.check_output(
            [
                "node",
                str(CLI_FILE),
                "create",
                "--chain-id",
                "postfiat-wan-devnet-2",
                "--issuer",
                ISSUER,
                "--code",
                "FEENAV",
                "--name",
                "Scheduled Reserve Unit",
                "--precision",
                "8",
                "--fee-recipient",
                RECIPIENT,
                "--annual-rate-bps",
                "100",
                "--hwm-rate-bps",
                "1000",
                "--issue-spread-bps",
                "50",
                "--redeem-spread-bps",
                "50",
                "--fix-fee-cap-bps",
                "25",
                "--notice-blocks",
                "604800",
                "--accrual-cap-blocks",
                "86400",
                "--canonical",
            ],
            text=True,
        ).strip()
        self.assertEqual(canonical_json_bytes(py_fee).decode("utf-8"), node_fee_raw)

    def test_zero_fee_vs_fee_enabled_sdk_and_neutral_lifecycle(self) -> None:
        zero_plan = create_navcoin(
            chain_id="postfiat-wan-devnet-2",
            issuer=ISSUER,
            code="PURE",
            display_name="Pure Reserve Unit",
        )
        self.assertFalse(zero_plan["fee_enabled"])
        self.assertEqual(
            [op["operation"] for op in zero_plan["operations"]],
            ["asset_create", "nav_asset_register"],
        )

        fee_plan = create_navcoin(
            chain_id="postfiat-wan-devnet-2",
            issuer=ISSUER,
            code="SCHED",
            display_name="Scheduled Reserve Unit",
            fee_schedule={
                "annual_rate_bps": 100,
                "hwm_rate_bps": 1000,
                "issue_spread_bps": 50,
                "redeem_spread_bps": 50,
                "fix_fee_cap_bps": 25,
                "recipient": RECIPIENT,
            },
        )
        self.assertTrue(fee_plan["fee_enabled"])
        self.assertEqual(
            [op["operation"] for op in fee_plan["operations"]],
            ["asset_create", "nav_asset_register", "nav_fee_schedule_attach"],
        )

        # Floats are strictly rejected
        with self.assertRaises(ValueError):
            compute_nav_per_unit(1.5, 1, 6)  # type: ignore[arg-type]
        with self.assertRaises(ValueError):
            compute_nav_per_unit(True, 1, 6)  # type: ignore[arg-type]

        # Inspect pending hike & cancel
        state = attach_fee_schedule(
            existing_fee_state=None,
            issuer=ISSUER,
            schedule_input=NavFeeScheduleInputV1(
                annual_rate_bps=100,
                hwm_rate_bps=1000,
                issue_spread_bps=50,
                redeem_spread_bps=50,
                fix_fee_cap_bps=25,
                recipient=RECIPIENT,
                increase_notice_blocks=604_800,
                accrual_cap_blocks=1_000_000,
            ),
            block_height=200_000,
            current_nav_per_unit=100_000_000,
        )
        self.assertFalse(inspect_pending_hike(state, 200_000)["has_pending"])
        self.assertFalse(inspect_pending_hike(None, 200_000)["has_fee_schedule"])

        state_pending = propose_fee_schedule_update(
            fee_state=state,
            issuer=ISSUER,
            schedule_input=NavFeeScheduleInputV1(
                annual_rate_bps=300,
                hwm_rate_bps=1000,
                issue_spread_bps=50,
                redeem_spread_bps=50,
                fix_fee_cap_bps=25,
                recipient=RECIPIENT,
                increase_notice_blocks=604_800,
                accrual_cap_blocks=1_000_000,
            ),
            block_height=210_000,
        )
        hike = inspect_pending_hike(state_pending, 250_000)
        self.assertTrue(hike["has_pending"])
        self.assertTrue(hike["is_increase"])
        self.assertTrue(hike["exit_capacity_locked"])
        self.assertEqual(hike["effective_at_height"], 814_800)
        self.assertEqual(hike["blocks_remaining"], 564_800)
        self.assertEqual(hike["changes"]["annual_rate_bps"], {"from": 100, "to": 300})

        state_cancelled = cancel_pending_fee_schedule(state_pending)
        self.assertIsNone(state_cancelled.pending)

        # Recompute & verify receipt
        _, accrual = recompute_fees(
            asset_id=ASSET_ID,
            epoch=12,
            block_height=515_360,
            verified_net_assets=1_100_000_000_000,
            supply_before=1_000_000_000_000,
            precision=8,
            fee_state_before=state,
        )
        self.assertEqual(accrual.minted_fee_units, 9_265_979_203)
        self.assertEqual(accrual.nav_after, 108_990_100)

        verified = verify_nav_receipt(
            accrual_receipt=accrual,
            precision=8,
            fee_state_before=state,
        )
        self.assertTrue(verified["valid"])

        tampered = accrual.to_dict()
        tampered["nav_after"] = accrual.nav_after - 1
        rejected = verify_nav_receipt(
            accrual_receipt=tampered,
            precision=8,
            fee_state_before=state,
        )
        self.assertFalse(rejected["valid"])
        self.assertIn("nav_after", rejected["mismatches"])

    def test_operation_builders_rpc_parsers_and_rust_hash_vectors(self) -> None:
        self.assertEqual(
            derived_asset_id(
                "postfiat-local", "pfissuer000000000000000000000000000000000", "USD", 1
            ),
            "7eb4ab19c010edff936edcff5e4e8c04300d15e1751c102ab91fcec8ac3e0c4"
            "9738c877dcde38c473050c2d68ecff45f",
        )
        self.assertEqual(
            nav_reserve_packet_id(
                "f0fcc338230cfbc693203d46deb36c39cfe7f492c088dbb4811b0bca54eda808"
                "2ec3d7c4bbfe0fe79fa42fe10b1713b8",
                1,
                "75c981a212c520fa45f7846c0265740a9ed9b8c7bedf9963ead6e0dd1828951c"
                "1337df22d68d2487516c68d3927a47f5",
            ),
            "69eb1f8adda23bfd3f8078d4ad76e7cca15f9d80d191e0dfb57f79e5319f63fd"
            "708b2080afae35e0fce5c9f4952dcab9",
        )

        set_op = build_nav_fee_schedule_set_operation(
            issuer=ISSUER,
            asset_id=ASSET_ID,
            schedule=NavFeeScheduleInputV1(
                annual_rate_bps=50,
                hwm_rate_bps=500,
                issue_spread_bps=10,
                redeem_spread_bps=10,
                fix_fee_cap_bps=10,
                recipient=RECIPIENT,
            ),
        )
        self.assertEqual(set_op["operation"], "nav_fee_schedule_set")
        self.assertEqual(
            build_nav_fee_schedule_cancel_operation(issuer=ISSUER, asset_id=ASSET_ID)["operation"],
            "nav_fee_schedule_cancel",
        )
        self.assertEqual(
            build_nav_fee_claim_operation(recipient=RECIPIENT, asset_id=ASSET_ID, amount=100)[
                "operation"
            ],
            "nav_fee_claim",
        )

        with self.assertRaises(NavFeeError) as ctx:
            verify_nav_per_unit(10, 3, 6, 3_333_332)
        self.assertEqual(ctx.exception.receipt_code, "nav_per_unit_formula_mismatch")

        sched_req = build_nav_fee_schedule_rpc_request(ASSET_ID)
        self.assertEqual(sched_req["method"], "nav_fee_schedule")
        parsed_sched = parse_nav_fee_schedule_response(
            {
                "result": {
                    "schema": "postfiat-nav-fee-schedule-v1",
                    "chain_id": "postfiat-wan-devnet-2",
                    "block_height": 200500,
                    "asset_id": ASSET_ID,
                    "found": True,
                    "has_fee_schedule": False,
                    "blocks_per_year": 31_536_000,
                    "fee_state": None,
                }
            }
        )
        self.assertFalse(parsed_sched["has_fee_schedule"])
        self.assertIsNone(parsed_sched["fee_state"])

        hist_req = build_nav_fee_history_rpc_request(ASSET_ID, from_epoch=1, limit=25)
        self.assertEqual(hist_req["method"], "nav_fee_history")
        parsed_hist = parse_nav_fee_history_response(
            {
                "result": {
                    "schema": "postfiat-nav-fee-history-v1",
                    "chain_id": "postfiat-wan-devnet-2",
                    "asset_id": ASSET_ID,
                    "accrual_count": 0,
                    "truncated": False,
                    "next_cursor": None,
                    "accruals": [],
                }
            }
        )
        self.assertEqual(parsed_hist["accrual_count"], 0)


if __name__ == "__main__":
    unittest.main()
