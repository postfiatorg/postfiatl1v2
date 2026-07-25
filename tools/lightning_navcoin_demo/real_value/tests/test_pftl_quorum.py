from __future__ import annotations

import copy
import unittest

from ..pftl_quorum import PftlQuorumError, PftlQuorumObserver
from .common import policy


class FakeClient:
    def __init__(self, index: int, route: object) -> None:
        self.index = index
        self.route = route
        self.freeze_enabled = False
        self.height = 42
        self.node_id = f"validator-{self.index}"
        self.chain_id = self.route.pftl_chain_id
        self.escrow_state = "open"
        self.receipt_height = self.height
        self.receipt_tip = "aa" * 48
        self.receipt_root = "bb" * 48

    def status(self) -> dict[str, object]:
        return {
            "chain_id": self.chain_id,
            "genesis_hash": self.route.pftl_genesis_hash,
            "validator_count": 6,
            "node_id": self.node_id,
            "status": "running",
            "mempool_pending": 0,
            "block_height": self.height,
            "block_tip_hash": "aa" * 48,
            "state_root": "bb" * 48,
            "build_git_revision": "ae3c53c9",
            "active_nav_profiles": [
                {
                    "asset_id": self.route.pftl_asset_id,
                    "profile_id": "cc" * 48,
                    "verifier_kind": "sp1_groth16",
                    "source_class": "stakehub",
                    "max_snapshot_age_blocks": 100,
                    "challenge_window_blocks": 1,
                    "max_epoch_gap_blocks": 100,
                    "settle_deadline_blocks": 10,
                    "min_attestations": 1,
                    "tolerance_bp": 0,
                    "bridge_observer_min_confirmations": 0,
                    "valuation_policy_hash": "dd" * 48,
                    "finalized_epoch": self.route.pftl_nav_epoch,
                    "nav_per_unit": self.route.pftl_nav_per_unit_usd_e8,
                    "finalized_reserve_packet_hash": (
                        self.route.pftl_nav_reserve_packet_hash
                    ),
                    "halted": False,
                }
            ],
        }

    def asset_info(self, asset_id: str) -> dict[str, object]:
        return {
            "asset": {
                "asset_id": asset_id,
                "freeze_enabled": self.freeze_enabled,
                "clawback_enabled": False,
                "requires_authorization": False,
                "precision": 6,
                "outstanding_supply": 100_000_000,
            }
        }

    def account_lines(self, *_args: object, **_kwargs: object) -> dict[str, object]:
        return {
            "lines": [
                {
                    "asset_id": self.route.pftl_asset_id,
                    "balance": 100_000_000,
                    "limit": 200_000_000,
                    "authorized": True,
                    "frozen": False,
                }
            ]
        }

    def account(self, address: str) -> dict[str, object]:
        return {"address": address, "balance": 1_000_000, "sequence": 9}

    def escrow_info(self, escrow_id: str) -> dict[str, object]:
        return {
            "escrow": {
                "escrow_id": escrow_id,
                "owner": self.route.coordinator_pftl_address,
                "recipient": "pf" + "77" * 20,
                "asset_id": self.route.pftl_asset_id,
                "amount": 1_000,
                "condition_hash": "ee" * 48,
                "finish_after": 0,
                "cancel_after": 900,
                "created_height": self.height,
                "state": self.escrow_state,
            }
        }

    def escrow_fee_quote(
        self,
        source: str,
        operation: dict[str, object],
        *,
        sequence: int,
    ) -> dict[str, object]:
        return {
            "schema": "postfiat-escrow-fee-quote-v1",
            "transaction_kind": "escrow_finish",
            "chain_id": self.route.pftl_chain_id,
            "genesis_hash": self.route.pftl_genesis_hash,
            "source": source,
            "sequence": sequence,
            "sender_balance": 1_000_000,
            "sender_sequence": 9,
            "mempool_pending_for_sender": 0,
            "minimum_fee": 23,
            "account_reserve": 10,
            "sender_meets_reserve_after_fee": True,
            "operation": operation,
        }

    def tx(
        self, tx_id: str, *, audit_block_log: bool = False
    ) -> dict[str, object]:
        assert audit_block_log is True
        return {
            "schema": "postfiat-tx-finality-v1",
            "tx_id": tx_id,
            "confirmed": True,
            "block_log_verified": True,
            "verification_mode": "full-block-replay",
            "chain_id": self.route.pftl_chain_id,
            "genesis_hash": self.route.pftl_genesis_hash,
            "protocol_version": 1,
            "receipt_count": 1,
            "receipt_index": 0,
            "receipt": {
                "tx_id": tx_id,
                "accepted": True,
                "code": "accepted",
            },
            "block": {
                "header": {
                    "height": self.receipt_height,
                    "block_hash": self.receipt_tip,
                    "state_root": self.receipt_root,
                },
                "receipt_ids": [tx_id],
            },
        }


class PftlQuorumTests(unittest.TestCase):
    def setUp(self) -> None:
        self.route = policy()
        self.clients = {
            endpoint: FakeClient(index, self.route)
            for index, endpoint in enumerate(self.route.pftl_rpc_endpoints)
        }
        self.observer = PftlQuorumObserver(
            self.route, client_factory=self.clients.__getitem__
        )

    def test_route_escrow_and_receipt_require_six_converged_views(self) -> None:
        route = self.observer.route_snapshot()
        self.assertEqual(route.agreeing_validator_count, 6)
        self.assertEqual(route.nav_epoch, 7)
        self.assertEqual(route.asset_precision, 6)
        self.assertEqual(route.coordinator_inventory_atoms, 100_000_000)
        escrow = self.observer.open_escrow(
            "ff" * 48,
            expected={
                "owner": self.route.coordinator_pftl_address,
                "recipient": "pf" + "77" * 20,
                "asset_id": self.route.pftl_asset_id,
                "amount": 1_000,
                "condition_hash": "ee" * 48,
                "finish_after": 0,
                "cancel_after": 900,
            },
        )
        self.assertEqual(escrow["agreeing_validator_count"], 6)
        capacity = self.observer.user_finish_capacity(
            "ff" * 48,
            expected={
                "owner": self.route.coordinator_pftl_address,
                "recipient": self.route.pftl_user_address,
                "asset_id": self.route.pftl_asset_id,
                "amount": 1_000,
                "condition_hash": "ee" * 48,
                "finish_after": 0,
                "cancel_after": 900,
            },
        )
        self.assertEqual(capacity["finish_minimum_fee"], 23)
        self.assertEqual(capacity["agreeing_validator_count"], 6)
        receipt = self.observer.receipt("12" * 48)
        self.assertIs(receipt["accepted"], True)
        self.assertEqual(receipt["code"], "accepted")
        self.assertEqual(receipt["verification_mode"], "full-block-replay")
        for client in self.clients.values():
            client.escrow_state = "finished"
        finished = self.observer.finished_escrow(
            "ff" * 48,
            expected={
                "owner": self.route.coordinator_pftl_address,
                "recipient": "pf" + "77" * 20,
                "asset_id": self.route.pftl_asset_id,
                "amount": 1_000,
                "condition_hash": "ee" * 48,
                "finish_after": 0,
                "cancel_after": 900,
            },
        )
        self.assertEqual(finished["state"], "finished")

    def test_receipt_identity_stays_bound_to_its_inclusion_block(self) -> None:
        for client in self.clients.values():
            client.receipt_height = 41
            client.receipt_tip = "91" * 48
            client.receipt_root = "92" * 48
        receipt = self.observer.receipt("12" * 48)
        self.assertEqual(receipt["height"], 41)
        self.assertEqual(receipt["block_tip_hash"], "91" * 48)
        self.assertEqual(receipt["state_root"], "92" * 48)
        self.assertEqual(receipt["receipt_count"], 1)

    def test_divergence_and_freeze_fail_closed(self) -> None:
        self.clients[self.route.pftl_rpc_endpoints[5]].height = 43
        with self.assertRaisesRegex(PftlQuorumError, "not converged"):
            self.observer.route_snapshot()
        self.clients[self.route.pftl_rpc_endpoints[5]].height = 42
        self.clients[self.route.pftl_rpc_endpoints[2]].freeze_enabled = True
        with self.assertRaisesRegex(PftlQuorumError, "block delivery"):
            self.observer.route_snapshot()

    def test_asset_precision_is_pinned_six_of_six(self) -> None:
        original = self.clients[self.route.pftl_rpc_endpoints[4]].asset_info

        def wrong_precision(asset_id: str) -> dict[str, object]:
            result = original(asset_id)
            result["asset"]["precision"] = 7
            return result

        self.clients[self.route.pftl_rpc_endpoints[4]].asset_info = wrong_precision
        with self.assertRaisesRegex(PftlQuorumError, "precision"):
            self.observer.route_snapshot()

    def test_fleet_wide_wrong_build_is_rejected_on_every_read_path(self) -> None:
        for client in self.clients.values():
            original = client.status

            def wrong_build(
                original_status=original,
            ) -> dict[str, object]:
                status = original_status()
                status["build_git_revision"] = "deadbee"
                return status

            client.status = wrong_build
        with self.assertRaisesRegex(PftlQuorumError, "build revision"):
            self.observer.route_snapshot()
        with self.assertRaisesRegex(PftlQuorumError, "build revision"):
            self.observer.receipt("12" * 48)
        with self.assertRaisesRegex(PftlQuorumError, "build revision"):
            self.observer.open_escrow(
                "ff" * 48,
                expected={
                    "owner": self.route.coordinator_pftl_address,
                    "recipient": "pf" + "77" * 20,
                    "asset_id": self.route.pftl_asset_id,
                    "amount": 1_000,
                    "condition_hash": "ee" * 48,
                    "finish_after": 0,
                    "cancel_after": 900,
                },
            )

    def test_fleet_wide_wrong_nav_scale_is_rejected_on_every_read_path(self) -> None:
        for client in self.clients.values():
            original = client.status

            def wrong_nav(
                original_status=original,
            ) -> dict[str, object]:
                status = original_status()
                status["active_nav_profiles"][0]["nav_per_unit"] = 1_000_000
                return status

            client.status = wrong_nav
        with self.assertRaisesRegex(PftlQuorumError, "USD-e8"):
            self.observer.route_snapshot()
        with self.assertRaisesRegex(PftlQuorumError, "USD-e8"):
            self.observer.receipt("12" * 48)
        with self.assertRaisesRegex(PftlQuorumError, "USD-e8"):
            self.observer.open_escrow(
                "ff" * 48,
                expected={
                    "owner": self.route.coordinator_pftl_address,
                    "recipient": "pf" + "77" * 20,
                    "asset_id": self.route.pftl_asset_id,
                    "amount": 1_000,
                    "condition_hash": "ee" * 48,
                    "finish_after": 0,
                    "cancel_after": 900,
                },
            )

    def test_receipt_and_escrow_reads_recheck_chain_and_validator_identity(self) -> None:
        bad = self.clients[self.route.pftl_rpc_endpoints[5]]
        bad.node_id = "validator-0"
        with self.assertRaisesRegex(PftlQuorumError, "distinct validators"):
            self.observer.receipt("12" * 48)
        bad.node_id = "validator-5"
        bad.chain_id = "wrong-chain"
        with self.assertRaisesRegex(PftlQuorumError, "chain identity"):
            self.observer.open_escrow(
                "ff" * 48,
                expected={
                    "owner": self.route.coordinator_pftl_address,
                    "recipient": "pf" + "77" * 20,
                    "asset_id": self.route.pftl_asset_id,
                    "amount": 1_000,
                    "condition_hash": "ee" * 48,
                    "finish_after": 0,
                    "cancel_after": 900,
                },
            )

    def test_every_multi_call_read_rejects_a_mid_bundle_tip_race(self) -> None:
        raced = self.clients[self.route.pftl_rpc_endpoints[5]]
        original_status = raced.status
        call_count = 0

        def moving_status() -> dict[str, object]:
            nonlocal call_count
            call_count += 1
            result = original_status()
            if call_count % 2 == 0:
                result["block_height"] = int(result["block_height"]) + 1
                result["block_tip_hash"] = "13" * 48
                result["state_root"] = "14" * 48
            return result

        raced.status = moving_status  # type: ignore[method-assign]
        expected = {
            "owner": self.route.coordinator_pftl_address,
            "recipient": self.route.pftl_user_address,
            "asset_id": self.route.pftl_asset_id,
            "amount": 1_000,
            "condition_hash": "ee" * 48,
            "finish_after": 0,
            "cancel_after": 900,
        }
        operations = (
            self.observer.route_snapshot,
            lambda: self.observer.open_escrow("ff" * 48, expected=expected),
            lambda: self.observer.user_finish_capacity(
                "ff" * 48, expected=expected
            ),
            lambda: self.observer.receipt("12" * 48),
        )
        for operation in operations:
            with self.subTest(operation=operation), self.assertRaisesRegex(
                PftlQuorumError, "finalized view changed during RPC read"
            ):
                operation()


if __name__ == "__main__":
    unittest.main()
