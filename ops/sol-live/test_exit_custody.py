import unittest
from unittest.mock import Mock, patch

from solders.keypair import Keypair
from solders.transaction import Transaction
import base64
import exit_payout as payout


class SpendGateTest(unittest.TestCase):
    def test_shortfall_defers_withdrawal_before_journal_or_send(self):
        c = Mock(payer=Keypair(), level={"commitment": "finalized"})
        c.owner = c.payer.pubkey()
        bucket = dict(
            bucket_id=payout.BUCKET,
            policy_hash=payout.POLICY,
            source_series_id=payout.SERIES,
            status="active",
            redeemable_at_par=True,
            impairment_factor_bps=10000,
            counted_value_atoms=20000,
            source_domain="sol",
            outstanding_vault_bridge_atoms=0,
            redemption_queue_atoms=20000,
            other_allocations_atoms=0,
            nav_subscription_allocations_atoms=0,
        )
        status = dict(
            buckets=[
                bucket,
                dict(bucket, bucket_id="earlier", counted_value_atoms=100000),
            ]
        )
        sent = []

        def call(method, *args):
            if method == "sendTransaction":
                sent.append(method)
                return str(
                    Transaction.from_bytes(base64.b64decode(args[0])).signatures[0]
                )
            return {
                "value": {
                    "getLatestBlockhash": {
                        "blockhash": str(payout.Hash.default()),
                        "lastValidBlockHeight": 10,
                    },
                    "getBalance": payout.FLOOR + 5000,
                    "getFeeForMessage": 5000,
                }[method]
            }

        c.call.side_effect = call
        record = Mock()
        with (
            patch.object(payout, "rpc", return_value=status),
            patch.object(
                payout,
                "inspect_vault",
                side_effect=[{"custody_atoms": 100000}, {"custody_atoms": 120000}],
            ),
        ):
            for preparatory_label in ("exit.buffer", "exit.upload", "exit.checkpoint"):
                with patch.object(payout, "check_custody", side_effect=ValueError("shortfall")):
                    with self.assertRaisesRegex(ValueError, "shortfall"):
                        payout.funded_send(c, preparatory_label, [])
                    self.assertEqual(sent, [])
            with self.assertRaisesRegex(ValueError, "Shared custody"):
                payout.funded_send(c, "exit.withdrawal", [], before_send=record)
            record.assert_not_called()
            self.assertEqual(sent, [])
            payout.funded_send(c, "exit.withdrawal", [], before_send=record)
            record.assert_called_once()
            self.assertEqual(len(sent), 1)
