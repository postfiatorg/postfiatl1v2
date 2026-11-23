import base64
import unittest
from copy import deepcopy
from pathlib import Path
from functools import partial
from unittest.mock import Mock, patch
import base58
import exit_canary as c
import exit_payout as p


class SettlementTest(unittest.TestCase):
    def test_finalized_ordinal_accounts_and_draft_match_paid_record(self):
        capture = c.read(Path(__file__).parent / "fixtures/agave-withdrawal.json")
        tx, paid = capture["transaction"], capture["paid"]
        keys = tx["transaction"]["message"]["accountKeys"]
        owner = p.pk(capture["job"]["recipient"])
        # The private bank used its own fee payer and recipient.
        with (patch.object(c, "PHANTOM", owner), patch.object(c, "FEE_PAYER", p.pk(keys[0])),
              patch.object(c, "token_deltas", partial(c.token_deltas, owner=owner))):
            ref = c.finalized_reference(paid, tx)
            bank = Mock(level={"commitment": "finalized"})
            bank.call.return_value = tx
            acc = capture["marker_account"]
            bank.account.return_value = dict(acc, data=base64.b64decode(acc["data"][0]))
            status = c.read(Path(__file__).parent / "fixtures/fork-burn-status.json")
            row, = status["redemptions"]
            with (patch.object(c, "cluster", return_value=bank), patch.object(c, "rpc", return_value=status),
                  patch.object(c, "OWNER", row["owner"]), patch.object(c, "DEST", row["destination_ref"])):
                self.assertEqual(c.review_paid(paid), ref)
            self.assertEqual(ref["ordinal"], 0)
            self.assertEqual(ref["slot"], tx["slot"])
            self.assertEqual(ref["signature_hex"], base58.b58decode(paid["solana_tx"]).hex())
            for field, value in [("finalized_slot", tx["slot"] + 1), ("solana_tx", "different")]:
                with self.assertRaises(ValueError):
                    c.finalized_reference(dict(paid, **{field: value}), tx)
            for case in ["duplicate", "inner", "wrong owner"]:
                altered = deepcopy(tx)
                instructions = altered["transaction"]["message"]["instructions"]
                ix = next(ix for ix in instructions if keys[ix["programIdIndex"]] == str(p.VERIFIER))
                if case == "duplicate":
                    instructions.append(ix)
                elif case == "inner":
                    altered["meta"]["innerInstructions"] = [dict(index=0, instructions=[ix])]
                else:
                    ix["accounts"][12] = 0
                with self.assertRaises(ValueError):
                    c.finalized_reference(paid, altered)
            with self.assertRaises(ValueError):
                c.match_reference(dict(kind=ref["kind"] + "0"), ref)

    def test_accounting_reconciles_concurrent_burn_deposit_and_settlement(self):
        status = c.read(Path(__file__).parent / "fixtures/fork-burn-status.json")
        red, = status["redemptions"]
        paid = {k: red[k] for k in ("burn_tx_id", "withdrawal_packet_hash")}
        bucket, = status["buckets"]
        alloc = status["allocations"][0]
        receipt = status["receipts"][0]
        other = dict(red, burn_tx_id="b", withdrawal_packet_hash="second", amount_atoms=10000)
        before = c.settlement_state(status, paid)
        red.update(state="settled", settled_atoms=20000, settlement_receipt_hash="id")
        status["redemptions"].append(other)
        bucket.update(
            counted_value_atoms=before["counted"] - 20000,
            redemption_queue_atoms=10000,
            outstanding_vault_bridge_atoms=before["outstanding"] - 10000,
        )
        alloc["remaining_atoms"] -= 20000
        c.check_settlement(before, c.settlement_state(status, paid), "id")
        status["allocations"].append(
            dict(alloc, amount_atoms=5000, remaining_atoms=5000)
        )
        status["receipts"].append(dict(receipt, counted_value_atoms=5000))
        bucket.update(
            counted_value_atoms=before["counted"] - 25000,
            redemption_queue_atoms=0,
            outstanding_vault_bridge_atoms=before["outstanding"] - 5000,
        )
        other.update(
            state="settled", settled_atoms=10000, settlement_receipt_hash="id2"
        )
        alloc["remaining_atoms"] -= 10000
        after = c.settlement_state(status, paid)
        c.check_settlement(before, after, "id")
        for k in ("counted", "supply", "outstanding", "queue"):
            with self.assertRaises(ValueError):
                c.check_settlement(before, dict(after, **{k: after[k] + 1}), "id")

    def test_settlement_cli_retains_first_baseline_and_checks_six_receipts(self):
        from pathlib import Path
        import tempfile
        from unittest.mock import patch

        status = c.read(Path(__file__).parent / "fixtures/fork-burn-status.json")
        red, = status["redemptions"]
        paid = {k: red[k] for k in ("burn_tx_id", "withdrawal_packet_hash")}
        bucket, = status["buckets"]
        alloc = status["allocations"][0]
        fixture = c.read(Path(__file__).parent / "fixtures/validator-governance.json")
        batch, tx = fixture["batch"], fixture["transaction"]
        height = tx["block"]["header"]["height"]
        amendment = tx["receipt"]["tx_id"]
        decision, = c.json.loads(batch["payload_json"])["amendments"]
        ref = dict(kind=decision["kind"])
        archived = []

        def rpc(i, method, **kwargs):
            if method == "vault_bridge_status":
                return status
            if method == "tx":
                self.assertEqual(kwargs["tx_id"], amendment)
                return tx
            self.assertEqual(method, "batch_archive")
            self.assertEqual(kwargs["batch_id"], batch["batch_id"])
            archived.append(i)
            return [batch]

        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(c, "rpc", side_effect=rpc),
            patch.object(c, "review_paid", return_value=ref),
            patch("live.receipt_at", return_value=[{"accepted": True}] * 6) as receipts,
        ):
            out = Path(directory)
            c.save(out / "paid.json", paid)
            c.settlement(out)
            baseline = (out / "settlement-before.json").read_bytes()
            status["redemptions"].append(
                dict(red, burn_tx_id="bb" * 48, amount_atoms=10000)
            )
            bucket.update(
                redemption_queue_atoms=30000, outstanding_vault_bridge_atoms=bucket["outstanding_vault_bridge_atoms"] - 10000
            )
            c.settlement(out)
            self.assertEqual((out / "settlement-before.json").read_bytes(), baseline)
            red.update(
                state="settled", settled_atoms=20000, settlement_receipt_hash=amendment
            )
            bucket.update(redemption_queue_atoms=10000, counted_value_atoms=bucket["counted_value_atoms"] - 20000)
            alloc["remaining_atoms"] -= 20000
            c.settlement(out, amendment, height)
            self.assertEqual(sorted(archived), list(range(6)))
            receipts.assert_called_once_with(amendment, height)
            self.assertEqual(c.read(out / "settled.json")["state"]["queue"], 10000)
