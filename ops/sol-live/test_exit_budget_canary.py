from pathlib import Path
import unittest
from unittest.mock import Mock
import exit_payout as p
import exit_canary as canary


class BudgetCanaryTest(unittest.TestCase):
    def test_shared_custody_reserves_and_fee_floor(self):
        b = dict(
            source_domain="same",
            outstanding_vault_bridge_atoms=20000,
            redemption_queue_atoms=20000,
            nav_subscription_allocations_atoms=0,
            other_allocations_atoms=0,
            counted_value_atoms=40000,
        )
        self.assertEqual(
            p.shared_liabilities(
                [
                    b,
                    dict(b, redemption_queue_atoms=0, counted_value_atoms=20000),
                    dict(b, source_domain="other"),
                ],
                "same",
            ),
            60000,
        )
        c = Mock(owner=p.FEE_PAYER)
        c.rent.side_effect = lambda n: 100 + n
        c.call.side_effect = lambda method, *args: (
            {"value": {"blockhash": str(p.Hash.default())}}
            if method == "getLatestBlockhash"
            else {"value": 10000 if method == "getFeeForMessage" else p.FLOOR}
        )
        debit = p.payout_budget(c, 1)
        self.assertGreater(debit, 10000 + c.rent(2048))
        with self.assertRaises(ValueError):
            p.balance(c, debit)

    def test_complete_signed_fields_bind_fee_sequence_and_domain(self):
        quote = canary.read(Path(__file__).parent / "fixtures/validator-canary-quote.json")
        plan = {k: quote[k] for k in ("chain_id", "genesis_hash", "source", "sequence",
                                      "protocol_version", "operation")}
        plan["fee"] = quote["minimum_fee"]
        u = canary.expected_unsigned(plan)
        self.assertEqual(u["amount_atoms"], 20000)
        for k in u:
            with self.assertRaises(ValueError):
                canary.check_unsigned(dict(u, **{k: "different"}), plan)

    def test_canary_uses_transaction_local_token_deltas(self):
        capture = canary.read(Path(__file__).parent / "fixtures/agave-withdrawal.json")
        tx = capture["transaction"]
        owner = p.pk(capture["job"]["recipient"])
        self.assertEqual(
            canary.token_deltas(tx, 20000, owner), {"custody": -20000, "recipient": 20000}
        )
        # The planned (governed) amount must be the amount the payout moved.
        with self.assertRaises(ValueError):
            canary.token_deltas(tx, 300000, owner)
        custody = str(p.pda(p.VAULT, b"custody"))
        index = tx["transaction"]["message"]["accountKeys"].index(custody)
        row = next(r for r in tx["meta"]["postTokenBalances"] if r["accountIndex"] == index)
        row["uiTokenAmount"]["amount"] = "1"
        with self.assertRaises(ValueError):
            canary.token_deltas(tx, 20000, owner)

    def test_budget_uses_selected_segments_and_peak_buffer_rent(self):
        from unittest.mock import patch

        c = Mock(owner=p.FEE_PAYER)
        c.rent.side_effect = lambda size: 100 + size
        c.call.side_effect = [
            {"value": {"blockhash": str(p.Hash.default())}},
            {"value": 7},
        ]
        with patch.object(p, "select_target", return_value={"height": 1818}) as select:
            count = p.proof_count(1753, 1883, 1882)
            self.assertEqual(count, 2)
            select.assert_called_once()
            self.assertEqual(
                p.payout_budget(c, count), 2 * (6 * 7 + 148) + 2148 + 101 + 265
            )

    def test_partial_system_accounts_need_rent_topups(self):
        c = Mock()
        c.rent.return_value = 2000
        partial = dict(owner=str(p.SYSTEM), executable=False, data=b"", lamports=800)
        for size, program in [(48, p.VERIFIER), (165, p.TOKEN), (1, p.VERIFIER)]:
            self.assertEqual(p.rent_shortfall(c, partial, size, program), 1200)
            self.assertEqual(p.rent_shortfall(c, None, size, program), 2000)
            self.assertEqual(
                p.rent_shortfall(c, dict(partial, lamports=3000), size, program), 0
            )

    def test_nonwithdrawal_sends_the_message_used_for_fee_quote(self):
        import base64
        from solders.keypair import Keypair
        from solders.transaction import Transaction

        c = Mock()
        c.payer = Keypair()
        c.owner = c.payer.pubkey()
        quoted = []

        def call(method, *args):
            if method == "getLatestBlockhash":
                return {
                    "value": {
                        "blockhash": str(p.Hash.default()),
                        "lastValidBlockHeight": 50,
                    }
                }
            if method == "getFeeForMessage":
                quoted.append(base64.b64decode(args[0]))
                return {"value": 5000}
            if method == "getBalance":
                return {"value": p.FLOOR + 6200}
            tx = Transaction.from_bytes(base64.b64decode(args[0]))
            self.assertEqual(bytes(tx.message), quoted[0])
            return str(tx.signatures[0])

        c.call.side_effect = call
        from unittest.mock import patch
        with patch.object(p, "check_custody"), patch.object(p, "check_verifier"):
            p.funded_send(c, "exit.checkpoint", [], rent=1200)
        c.confirm.assert_called_once()

    def test_recover_resubmits_only_saved_envelope_after_six_status_checks(self):
        import tempfile
        from pathlib import Path
        from unittest.mock import patch

        burn = "ad" * 48
        head = dict(block_height=40, block_tip_hash="tip", state_root="root")
        fields = dict(sequence=7)
        plan = dict(parent=head, fields=fields)
        with tempfile.TemporaryDirectory() as d:
            out = Path(d)
            canary.save(out / "plan.json", plan)
            canary.save(out / "sign-request.json", dict(kind="asset", fields=fields))
            raw = ' { "example" : "original" }\n'
            (out / "signed.json").write_text(raw)
            seen = set()
            accepted = False

            def rpc(i, method, **params):
                if method == "tx":
                    self.assertEqual(params["tx_id"], burn)
                    seen.add(i)
                    if not accepted:
                        raise RuntimeError("rpc_tx_not_found")
                    return dict(confirmed=True, receipt=dict(tx_id=burn, accepted=True))
                self.assertEqual(seen, set(range(6)))
                self.assertEqual(params["signed_asset_transaction_json"], raw)
                return dict(
                    finality=dict(
                        confirmed=True, receipt=dict(accepted=True, tx_id=burn)
                    )
                )

            with (
                patch.object(canary, "verified_burn_id", return_value=burn),
                patch.object(canary, "prepare", return_value=plan),
                patch.object(canary, "rpc", side_effect=rpc) as calls,
                patch.object(
                    canary.subprocess,
                    "run",
                    side_effect=AssertionError("must not re-sign"),
                ),
            ):
                canary.recover(out)
                self.assertEqual(calls.call_count, 7)
                accepted = True
                canary.recover(out)
                self.assertEqual(calls.call_count, 13)
                calls.reset_mock()
                (out / "signed.json").write_text(raw + " ")
                with self.assertRaises(ValueError):
                    canary.recover(out)
                calls.assert_not_called()
                (out / "signed.json").write_text(raw)
                canary.save(
                    out / "sign-request.json",
                    dict(kind="asset", fields=dict(sequence=8)),
                )
                with self.assertRaises(ValueError):
                    canary.recover(out)
                calls.assert_not_called()
