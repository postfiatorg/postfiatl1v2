import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch


class FifthReview(unittest.TestCase):
    def test_paid_canary_survives_intervening_holder_credit(self):
        import exit_canary as c

        capture = json.loads((Path(__file__).parent / "fixtures/eth-redemption.json").read_text())
        report = capture["report"]
        row = report["redemptions"][0]
        burn = row["burn_tx_id"]
        # Adapt only the intent to the as-yet unopened Solana canary.
        row.update(owner=c.OWNER, bucket_id=c.BUCKET, destination_ref=c.DEST, amount_atoms=20000)
        transaction = capture["transaction"]
        receipt = transaction["receipt"]

        def rpc(i, method, **params):
            if method == "vault_bridge_status":
                return report
            self.assertEqual(method, "tx")
            return transaction

        response = Mock()
        response.json.return_value = dict(stage="paid", solana_tx="sig")
        with tempfile.TemporaryDirectory() as d, contextlib.ExitStack() as st:
            out = Path(d)
            (out / "signed.json").write_text("{}")
            (out / "plan.json").write_text(
                json.dumps(
                    dict(fields=dict(sequence=row["owner_sequence"]), before=dict(pfusdc_atoms=20000))
                )
            )
            st.enter_context(patch.object(c, "fleet"))
            st.enter_context(patch.object(c, "rpc", side_effect=rpc))
            st.enter_context(patch.object(c, "verified_burn_id", return_value=burn))
            st.enter_context(patch.object(c.requests, "get", return_value=response))
            st.enter_context(patch.object(c.requests, "post", return_value=response))
            st.enter_context(
                patch.object(c, "balances", return_value=dict(pfusdc_atoms=1))
            )
            st.enter_context(
                patch.object(c, "cluster")
            ).return_value.call.return_value = {"slot": 10}
            for name, val in [
                ("token_deltas", {}),
                ("signature", "sig"),
                ("finalized_reference", {}),
            ]:
                st.enter_context(patch.object(c, name, return_value=val))
            with contextlib.redirect_stdout(io.StringIO()):
                c.check(out)
            paid = json.loads((out / "paid.json").read_text())
            self.assertEqual(paid["l1_debit_atoms"], 20000)
            self.assertEqual(len(paid["burn_receipts"]), 6)
            receipt["tx_id"] = "ff" * 48
            with self.assertRaises(ValueError):
                c.check(out)
            receipt["tx_id"] = burn
            report["asset_id"] = "different"
            with self.assertRaisesRegex(ValueError, "asset"):
                c.check(out)
