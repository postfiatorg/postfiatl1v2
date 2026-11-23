import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch
from solders.keypair import Keypair
from solders.hash import Hash
import exit_canary as canary
import exit_setup as setup


class FundingTest(unittest.TestCase):
    def test_funding_timeout_waits_for_expiry_and_history_before_retry(self):
        from exit_fund import fund
        c = Mock(level={"commitment":"finalized"}, payer=Keypair())
        c.owner = c.payer.pubkey()
        with tempfile.TemporaryDirectory() as d:
            path = Path(d)/"funding.json"
            sends = []
            height, status = 50, None
            def call(method, *args):
                if method == "getSignatureStatuses":
                    self.assertTrue(args[1]["searchTransactionHistory"])
                    return {"context":{"slot":200}, "value":[status]}
                if method == "getBlockHeight":
                    return height
                if method == "getBalance":
                    return {"value":0}
                if method == "getLatestBlockhash":
                    return {"value":{"blockhash":str(Hash.new_unique()), "lastValidBlockHeight":100}}
                if method == "sendTransaction":
                    attempt = setup.read(path)
                    self.assertEqual(attempt["lastValidBlockHeight"],100)
                    sends.append(attempt["signature"])
                    raise TimeoutError()
                raise AssertionError(method)
            c.call.side_effect = call
            with self.assertRaises(TimeoutError):
                fund(c,path)
            with self.assertRaisesRegex(ValueError,"pending"):
                fund(c,path)
            self.assertEqual(len(sends),1)
            height = 101
            status = {"confirmationStatus":"confirmed","err":None}
            with self.assertRaisesRegex(ValueError,"pending"):
                fund(c,path)
            status = None
            with self.assertRaises(TimeoutError):
                fund(c,path)
            self.assertEqual(len(set(sends)),2)
            status = {"confirmationStatus":"finalized","err":None}
            self.assertEqual(fund(c,path),sends[-1])
            self.assertEqual(len(sends),2)

    def test_canary_requires_positive_native_fee_reserve_on_both_quotes(self):
        import json
        r=dict(ready=True, payout_ready=True, burns_open=False, issuer="issuer",
               epoch=8, reserve_packet_hash="hash", bucket_id=canary.BUCKET,series_asset_id=canary.SERIES)
        head=dict(chain_id="chain",genesis_hash="genesis")
        def quote(i,method,source,operation_json):
            return dict(head,source=source,operation=json.loads(operation_json), protocol_version=1,
                        minimum_fee=23, sequence=2,sender_meets_reserve_after_fee=False)
        with patch.object(canary.requests,"get") as get, patch.object(canary,"cluster"), patch.object(
                canary,"balances",return_value=dict(pfusdc_atoms=20000)), patch.object(canary,"rpc",quote), patch.object(canary,"fleet",return_value=head):
            get.return_value.json.return_value=r
            for closed in [True,False]:
                with self.assertRaisesRegex(ValueError,"fee"):
                    canary.prepare(None,allow_closed=closed)
