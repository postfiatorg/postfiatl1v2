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
            source_domain=payout.SOURCE_DOMAIN,
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
            patch.object(payout, "check_verifier"),
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

    def test_shared_custody_covers_all_buckets_before_tag3_signing(self):
        import exit_setup as setup

        c = Mock(owner=setup.MAINNET_PAYER)
        c.call.return_value = {"value": {"blockhash": str(setup.Hash.default())}}
        parent = dict(block_height=1759, block_tip_hash="tip", state_root="root")
        bucket = dict(
            bucket_id=payout.BUCKET, policy_hash=payout.POLICY,
            source_series_id=payout.SERIES, status="active", redeemable_at_par=True,
            impairment_factor_bps=10000, source_domain=payout.SOURCE_DOMAIN, counted_value_atoms=20000,
            outstanding_vault_bridge_atoms=10000, redemption_queue_atoms=10000,
            other_allocations_atoms=0, nav_subscription_allocations_atoms=0,
        )
        buckets = [bucket, dict(bucket, bucket_id="earlier", counted_value_atoms=100000)]
        with (
            patch.object(payout, "rpc", return_value=dict(buckets=buckets)),
            patch.object(payout, "inspect_vault", side_effect=[
                {"custody_atoms": 119999}, {"custody_atoms": 120000}]),
            patch.object(setup, "fleet", return_value=parent),
            patch.object(setup, "Transaction") as tx,
        ):
            tx.new_signed_with_payer.side_effect = RuntimeError("signing reached")
            with self.assertRaisesRegex(ValueError, "Shared custody"):
                setup.send_initialize(c, Mock(), Mock(), parent)
            tx.new_signed_with_payer.assert_not_called()
            with self.assertRaisesRegex(RuntimeError, "signing reached"):
                setup.send_initialize(c, Mock(), Mock(), parent)
            self.assertNotIn("sendTransaction", [call.args[0] for call in c.call.call_args_list])
