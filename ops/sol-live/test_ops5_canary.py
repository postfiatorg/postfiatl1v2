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

        burn = "ab" * 48
        row = dict(
            owner=c.OWNER,
            owner_sequence=7,
            bucket_id=c.BUCKET,
            amount_atoms=20000,
            destination_ref=c.DEST,
            burn_tx_id=burn,
            created_at_height=9,
            withdrawal_packet_hash="cd" * 48,
            asset_id=c.ASSET,
        )
        receipt = dict(tx_id=burn, accepted=True)
        h = dict(
            height=9, block_hash="ab" * 48, state_root="ef" * 48, batch_kind="asset"
        )

        def rpc(i, method, **params):
            if method == "vault_bridge_status":
                return dict(redemptions=[row])
            self.assertEqual(method, "tx")
            return dict(confirmed=True, receipt=receipt, block=dict(header=h))

        response = Mock()
        response.json.return_value = dict(stage="paid", solana_tx="sig")
        with tempfile.TemporaryDirectory() as d, contextlib.ExitStack() as st:
            out = Path(d)
            (out / "signed.json").write_text("{}")
            (out / "plan.json").write_text(
                json.dumps(
                    dict(fields=dict(sequence=7), before=dict(pfusdc_atoms=20000))
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
