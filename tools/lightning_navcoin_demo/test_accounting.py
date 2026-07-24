from __future__ import annotations

import hashlib
import unittest

from tools.lightning_navcoin_demo.accounting import (
    InvariantViolation,
    LightningSettlement,
    PftlAssetState,
    assert_cancel_delta,
    assert_create_delta,
    assert_finish_delta,
    assert_lightning_settlement,
    assert_mutation_free_rejection,
    assert_terminal_conditional_atomicity,
    assert_validator_convergence,
)


class PftlAccountingTests(unittest.TestCase):
    def test_create_finish_and_cancel_conserve_exactly(self) -> None:
        initial = PftlAssetState(10_000, 1_000, 0, 11_000, None)
        open_state = PftlAssetState(8_000, 1_000, 2_000, 11_000, "OPEN")
        finished = PftlAssetState(8_000, 3_000, 0, 11_000, "FINISHED")
        assert_create_delta(initial, open_state, principal_atoms=2_000)
        assert_finish_delta(open_state, finished, principal_atoms=2_000)

        refund_open = PftlAssetState(6_500, 3_000, 1_500, 11_000, "OPEN")
        refunded = PftlAssetState(8_000, 3_000, 0, 11_000, "CANCELED")
        assert_cancel_delta(refund_open, refunded, principal_atoms=1_500)

    def test_wrong_delta_and_rejection_mutation_fail(self) -> None:
        before = PftlAssetState(10_000, 1_000, 0, 11_000, None)
        bad = PftlAssetState(8_000, 1_001, 2_000, 11_000, "OPEN")
        with self.assertRaises(InvariantViolation):
            assert_create_delta(before, bad, principal_atoms=2_000)
        with self.assertRaises(InvariantViolation):
            assert_mutation_free_rejection(before, bad)

    def test_six_and_five_view_convergence(self) -> None:
        view = {
            "validator_count": 6,
            "chain_id": "local-lightning",
            "genesis_hash": "1" * 96,
            "block_height": 12,
            "block_tip_hash": "2" * 96,
            "state_root": "3" * 96,
        }
        assert_validator_convergence([dict(view) for _ in range(6)])
        assert_validator_convergence(
            [dict(view) for _ in range(5)], required_available=5
        )
        divergent = [dict(view) for _ in range(6)]
        divergent[-1]["state_root"] = "4" * 96
        with self.assertRaises(InvariantViolation):
            assert_validator_convergence(divergent)


class CrossLedgerTests(unittest.TestCase):
    def test_lightning_linkage_and_terminal_states(self) -> None:
        preimage = bytes(range(32))
        digest = hashlib.sha256(preimage).hexdigest()
        evidence = assert_lightning_settlement(
            LightningSettlement(
                payment_hash=digest,
                payment_preimage=preimage,
                invoice_amount_msat=100_000,
                settled_amount_msat=100_000,
                fee_msat=0,
                status="SUCCEEDED",
            ),
            expected_hash=digest,
            fee_limit_msat=0,
        )
        self.assertEqual(evidence["payment_preimage"], "<redacted>")
        self.assertEqual(
            assert_terminal_conditional_atomicity(
                lightning_settled=True, pftl_escrow_state="FINISHED"
            ),
            "BOTH_SETTLED",
        )
        self.assertEqual(
            assert_terminal_conditional_atomicity(
                lightning_settled=False, pftl_escrow_state="CANCELED"
            ),
            "NEITHER_SETTLED",
        )
        with self.assertRaises(InvariantViolation):
            assert_terminal_conditional_atomicity(
                lightning_settled=True, pftl_escrow_state="CANCELED"
            )


if __name__ == "__main__":
    unittest.main()
